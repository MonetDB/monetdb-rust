#![allow(dead_code)]

pub mod delayed;
pub mod replies;
pub mod rowset;

use std::mem;
use std::{io, sync::Arc};

use delayed::DelayedCommands;
use replies::{BadReply, ReplyBuf, ReplyParser, ResultColumn, ResultSet};
use rowset::RowSet;

use crate::conn::Conn;
use crate::framing::reading::MapiReader;
use crate::framing::writing::MapiBuf;
use crate::framing::{ServerSock, ServerState};
use crate::{framing::FramingError, IoError};

#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum CursorError {
    #[error("{0}")]
    Server(String),
    #[error("connection has been closed")]
    Closed,
    #[error(transparent)]
    IO(#[from] IoError),
    #[error(transparent)]
    Framing(#[from] FramingError),
    #[error(transparent)]
    BadReply(#[from] BadReply),
    #[error("there is no result set")]
    NoResultSet,
    #[error("could not convert column {0} to {1}: {2}")]
    Conversion(usize, &'static str, String),
    #[error("server unexpectedly returned no rows")]
    NoRows,
}

pub type CursorResult<T> = Result<T, CursorError>;

impl From<io::Error> for CursorError {
    fn from(value: io::Error) -> Self {
        IoError::from(value).into()
    }
}

pub struct Cursor {
    conn: Arc<Conn>,
    buf: MapiBuf,
    replies: ReplyParser,
    reply_size: usize,
}

impl Cursor {
    pub(crate) fn new(conn: Arc<Conn>) -> Self {
        Cursor {
            buf: MapiBuf::new(),
            replies: ReplyParser::default(),
            reply_size: conn.reply_size,
            conn,
        }
    }

    pub fn execute(&mut self, statements: &str) -> CursorResult<()> {
        self.exhaust()?;

        let mut vec = self.replies.take_buffer();
        let command = &[b"s", statements.as_bytes(), b"\n;"];

        self.command(command, &mut vec)?;

        let error = ReplyParser::detect_errors(&vec);

        // Always create and install a replyparser, even if an error occurred.
        // We need to make sure all result sets are being released etc.
        self.replies = ReplyParser::new(vec)?;

        if let Err(err) = error {
            self.exhaust()?;
            return Err(err);
        }

        Ok(())
    }

    fn command(&mut self, command: &[&[u8]], vec: &mut Vec<u8>) -> Result<(), CursorError> {
        self.conn.run_locked(
            |_state: &mut ServerState,
             delayed: &mut DelayedCommands,
             mut sock: ServerSock|
             -> CursorResult<ServerSock> {
                sock = delayed.send_delayed_plus(sock, command)?;
                sock = delayed.recv_delayed(sock, vec)?;
                vec.clear();
                sock = MapiReader::to_end(sock, vec)?;
                Ok(sock)
            },
        )?;
        Ok(())
    }

    pub fn affected_rows(&self) -> Option<i64> {
        self.replies.affected_rows()
    }

    pub fn has_result_set(&self) -> bool {
        self.replies.at_result_set()
    }

    pub fn next_reply(&mut self) -> CursorResult<bool> {
        // todo: close server side result set if necessary
        let old = mem::take(&mut self.replies);
        let new = old.into_next_reply()?;
        self.switch_to_reply(new)
    }

    fn switch_to_reply(&mut self, replies: ReplyParser) -> CursorResult<bool> {
        self.replies = replies;
        let have_next = !matches!(self.replies, ReplyParser::Exhausted(..));
        Ok(have_next)
    }

    fn exhaust(&mut self) -> CursorResult<()> {
        loop {
            if let ReplyParser::Exhausted(..) = self.replies {
                return Ok(());
            }
            self.next_reply()?;
        }
    }

    pub fn metadata(&self) -> &[ResultColumn] {
        if let ReplyParser::Data(ResultSet { columns, .. }) = &self.replies {
            &columns[..]
        } else {
            &[]
        }
    }

    pub fn next_row(&mut self) -> CursorResult<bool> {
        self.skip_to_result_set()?;

        loop {
            let ResultSet {
                row_set,
                next_row,
                total_rows,
                ..
            } = self.result_set_mut();

            if row_set.advance()? {
                *next_row += 1;
                return Ok(true);
            }
            if next_row == total_rows {
                return Ok(false);
            }
            self.fetch_more_rows()?;
        }
    }

    fn result_set(&self) -> &ResultSet {
        let ReplyParser::Data(rs) = &self.replies else {
            unreachable!("skip_to_result_set() should have ensured a result set");
        };
        rs
    }

    fn result_set_mut(&mut self) -> &mut ResultSet {
        let ReplyParser::Data(rs) = &mut self.replies else {
            unreachable!("skip_to_result_set() should have ensured a result set");
        };
        rs
    }

    fn skip_to_result_set(&mut self) -> CursorResult<()> {
        loop {
            match &mut self.replies {
                ReplyParser::Data(_) => return Ok(()),
                ReplyParser::Exhausted(_) => return Err(CursorError::NoResultSet),
                _ => self.next_reply()?,
            };
        }
    }

    fn decide_next_fetch(&self) -> (u64, u64, usize) {
        let ResultSet {
            result_id,
            next_row,
            total_rows,
            ..
        } = self.result_set();

        let n = (total_rows - *next_row).min(self.reply_size as u64) as usize;
        (*result_id, *next_row, n)
    }

    fn fetch_more_rows(&mut self) -> CursorResult<()> {
        let (res_id, start, n) = self.decide_next_fetch();
        let cmd = format!("Xexport {res_id} {start} {n}");

        // scratch vector. TODO re-use this
        let mut vec = vec![];

        // execute the command
        self.command(&[cmd.as_bytes()], &mut vec)?;
        ReplyParser::detect_errors(&vec)?;

        // parse it into a rowset
        let mut buf = ReplyBuf::new(vec);
        let mut fields = [0u64; 4];
        ReplyParser::parse_header(&mut buf, &mut fields)?;
        let ncol = fields[1];
        let mut new_row_set = RowSet::new(buf, ncol as usize);

        // If we were reading the initial response, save it.
        // Then install the new rowset, saving the old one if it's the primary.
        // We know it's the primary when stashed_primary_row_set is still None.
        let ResultSet {
            row_set,
            stashed: stashed_primary_row_set,
            ..
        } = self.result_set_mut();
        mem::swap(row_set, &mut new_row_set);
        if stashed_primary_row_set.is_none() {
            // new_row_set is actually the old row set now
            *stashed_primary_row_set = Some(new_row_set);
        }

        // Now the new rows are in place!
        Ok(())
    }

    fn row_set(&self) -> CursorResult<&RowSet> {
        if let ReplyParser::Data(ResultSet { row_set, .. }) = &self.replies {
            Ok(row_set)
        } else {
            Err(CursorError::NoResultSet)
        }
    }
}

macro_rules! getter {
    ($method:ident, $type:ty) => {
        pub fn $method(&self, col: usize) -> CursorResult<Option<$type>> {
            self.row_set()?.$method(col)
        }
    };
}

impl Cursor {
    getter!(get_str, &str);
    getter!(get_i8, i8);
    getter!(get_u8, u8);
    getter!(get_i16, i16);
    getter!(get_u16, u16);
    getter!(get_i32, i32);
    getter!(get_u32, u32);
    getter!(get_i64, i64);
    getter!(get_u64, u64);
    getter!(get_i128, i128);
    getter!(get_u128, u128);
    getter!(get_isize, isize);
    getter!(get_usize, usize);
    getter!(get_f32, f32);
    getter!(get_f64, f64);
}

fn find_response_line(marker: u8, response: &[u8]) -> Option<usize> {
    if response.is_empty() {
        None
    } else if response[0] == marker {
        Some(0)
    } else {
        memchr::memmem::find(response, &[b'\n', marker]).map(|idx| idx + 1)
    }
}

pub fn from_utf8(bytes: &[u8]) -> CursorResult<&str> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(_) => Err(FramingError::Unicode.into()),
    }
}
