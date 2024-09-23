#![allow(dead_code)]

pub mod delayed;
pub mod replies;

use std::mem;
use std::{io, sync::Arc};

use delayed::DelayedCommands;
use replies::{BadReply, ReplyParser};

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
            |_state: &mut ServerState, delayed: &mut DelayedCommands,  mut sock: ServerSock| -> CursorResult<ServerSock> {
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

    pub fn temporary_get_result_set(&self) -> CursorResult<Option<&str>> {
        let x = self.replies.remaining_rows()?;
        Ok(x)
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
