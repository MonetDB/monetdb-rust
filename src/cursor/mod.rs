#![allow(dead_code)]

pub mod delayed;
pub mod replies;
pub mod rowset;

use std::mem;
use std::{io, sync::Arc};

use delayed::DelayedCommands;
use replies::{BadReply, ReplyParser, ResultColumn};

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
}

impl Cursor {
    pub(crate) fn new(conn: Arc<Conn>) -> Self {
        Cursor {
            conn,
            buf: MapiBuf::new(),
            replies: ReplyParser::default(),
        }
    }

    pub fn execute(&mut self, statements: &str) -> CursorResult<()> {
        self.exhaust()?;

        let mut vec = self.replies.take_buffer();

        self.conn.run_locked(
            |_state: &mut ServerState,
             delayed: &mut DelayedCommands,
             mut sock: ServerSock|
             -> CursorResult<ServerSock> {
                let command = &[b"s", statements.as_bytes(), b"\n;"];
                sock = delayed.send_delayed_plus(sock, command)?;
                sock = delayed.recv_delayed(sock, &mut vec)?;
                vec.clear();
                sock = MapiReader::to_end(sock, &mut vec)?;
                Ok(sock)
            },
        )?;

        let error =
            ReplyParser::detect_errors(&vec).map(|msg| CursorError::Server(msg.to_string()));

        // Always create and install a replyparser, even if an error occurred.
        // We need to make sure all result sets are being released etc.
        self.replies = ReplyParser::new(vec)?;

        if let Some(err) = error {
            self.exhaust()?;
            return Err(err);
        }

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
        if let ReplyParser::Data { columns, .. } = &self.replies {
            &columns[..]
        } else {
            &[]
        }
    }

    pub fn next_row(&mut self) -> CursorResult<bool> {
        // Skip forward to the next result set if we're not currently on one
        loop {
            match &mut self.replies {
                ReplyParser::Data { row_set, .. } => {
                    let x = row_set.advance()?;
                    return Ok(x);
                }
                ReplyParser::Exhausted(_) => return Err(CursorError::NoResultSet),
                _ => {
                    self.next_reply()?;
                }
            }
        }
    }

    pub fn get_str(&self, col: usize) -> CursorResult<Option<&str>> {
        let ReplyParser::Data { row_set, .. } = &self.replies else {
            return Err(CursorError::NoResultSet);
        };
        let value = row_set.get_field_str(col)?;
        Ok(value)
    }
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
