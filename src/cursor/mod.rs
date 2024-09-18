#![allow(dead_code)]

use std::{io, sync::Arc};

use crate::conn::Conn;
use crate::framing::reading::MapiReader;
use crate::framing::writing::MapiBuf;
use crate::{framing::FramingError, IoError};

#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum CursorError {
    #[error("connection has been closed")]
    Closed,
    #[error(transparent)]
    IO(#[from] IoError),
    #[error(transparent)]
    Framing(#[from] FramingError),
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
    replies: Vec<u8>,
}

impl Cursor {
    pub(crate) fn new(conn: Arc<Conn>) -> Self {
        Cursor {
            conn,
            buf: MapiBuf::new(),
            replies: Vec::default(),
        }
    }

    pub fn execute(&mut self, statements: &str) -> CursorResult<&str> {
        self.replies.clear();
        self.buf.append("s");
        self.buf.append(statements);
        self.buf.append("\n;1");
        let () = self.conn.run_locked(|_state, mut sock| {
            self.replies.truncate(0);
            sock = self.buf.write_reset(sock)?;
            sock = MapiReader::to_end(sock, &mut self.replies)?;
            Ok((sock, ()))
        })?;

        match std::str::from_utf8(&self.replies) {
            Ok(s) => Ok(s),
            Err(_) => Err(FramingError::Unicode.into()),
        }
    }
}
