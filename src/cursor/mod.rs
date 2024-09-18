#![allow(dead_code)]

use std::{io, sync::Arc};

use crate::conn::Conn;
use crate::framing::reading::MapiReader;
use crate::framing::writing::MapiBuf;
use crate::{framing::FramingError, IoError};

#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum CursorError {
    #[error("server: {0}")]
    Server(String),
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
        let () = self.conn.run_locked(|_state, mut sock| {
            self.replies.clear();
            sock = self.buf.write_reset_plus(sock, &[b"s", statements.as_bytes(), b"\n;"])?;
            sock = MapiReader::to_end(sock, &mut self.replies)?;
            Ok((sock, ()))
        })?;

        // Quickly check for errors.
        if let Some(idx) = find_response_line(b'!', &self.replies) {
            let error_line = self.replies[idx + 1..].split(|&b| b == b'\n').next().unwrap();
            let message = from_utf8(error_line)?;
            return Err(CursorError::Server(message.to_string()));
        }

        from_utf8(&self.replies)
    }
}

fn find_response_line(marker: u8, response: &[u8]) -> Option<usize> {
    if response.is_empty() {
        None
    } else if response[0] == marker {
        Some(0)
    } else if let Some(idx) = memchr::memmem::find(response, &[b'\n', marker]) {
        Some(idx + 1)
    } else {
        None
    }
}


pub fn from_utf8(bytes: &[u8]) -> CursorResult<&str> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(_) => Err(FramingError::Unicode.into()),
    }
}