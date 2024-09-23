#![allow(dead_code)]

use std::io::Write;

use crate::framing::{reading::MapiReader, writing::MapiBuf, ServerSock};

use super::CursorResult;

pub struct ExpectedResponse {
    pub description: String,
}

pub struct DelayedCommands {
    pub buffer: MapiBuf,
    pub responses: Vec<ExpectedResponse>,
}

impl Default for DelayedCommands {
    fn default() -> Self {
        Self::new()
    }
}
impl DelayedCommands {
    pub fn new() -> Self {
        DelayedCommands {
            buffer: MapiBuf::default(),
            responses: Vec::default(),
        }
    }

    pub fn add(&mut self, cmd: String) {
        self.buffer.append(&cmd);
        if !cmd.ends_with('\n') {
            self.buffer.append("\n");
        }
        self.buffer.end();
        self.responses.push(ExpectedResponse { description: cmd })
    }

    pub fn send_delayed(&mut self, mut conn: ServerSock) -> CursorResult<ServerSock> {
        let raw = self.buffer.reset();
        conn.write_all(raw)?;
        Ok(conn)
    }

    pub fn send_delayed_plus(&mut self, mut conn: ServerSock, extra: &[&[u8]]) -> CursorResult<ServerSock> {
        conn = self.buffer.write_reset_plus(conn, extra)?;
        Ok(conn)
    }

    pub fn recv_delayed(
        &mut self,
        conn: ServerSock,
        buffer: &mut Vec<u8>,
    ) -> CursorResult<ServerSock> {
        let res = self.recv_delayed_inner(conn, buffer);
        buffer.clear();
        res
    }

    pub fn recv_delayed_inner(
        &mut self,
        mut conn: ServerSock,
        buffer: &mut Vec<u8>,
    ) -> CursorResult<ServerSock> {
        for resp in self.responses.drain(..) {
            buffer.clear();
            conn = MapiReader::to_end(conn, buffer)?;
            if let Some(err_msg) = buffer.strip_prefix(b"!") {
                let msg = String::from_utf8_lossy(err_msg);
                let description = &resp.description;
                return Err(super::CursorError::Server(format!(
                    "delayed {description}: {msg}"
                )));
            }
        }
        Ok(conn)
    }
}
