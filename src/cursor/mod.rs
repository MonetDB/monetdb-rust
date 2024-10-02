// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
#![allow(dead_code)]

pub(crate) mod delayed;
pub(crate) mod replies;
pub(crate) mod rowset;

use std::mem;
use std::{io, sync::Arc};

use delayed::DelayedCommands;
use replies::{BadReply, ReplyBuf, ReplyParser, ResultColumn, ResultSet};
use rowset::RowSet;

use crate::conn::Conn;
use crate::framing::reading::MapiReader;
use crate::framing::writing::MapiBuf;
use crate::framing::FramingError;
use crate::framing::{ServerSock, ServerState};
use crate::util::ioerror::IoError;

/// An error that occurs while accessing data with a [`Cursor`].
#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum CursorError {
    /// The server returned an error.
    #[error("{0}")]
    Server(String),
    /// The connection has been closed.
    #[error("connection has been closed")]
    Closed,
    /// An IO Error occurred.
    #[error(transparent)]
    IO(#[from] IoError),
    #[error(transparent)]
    /// Something went wrong in the communication with the server.
    Framing(#[from] FramingError),
    /// The server sent a response that we do not understand.
    #[error(transparent)]
    BadReply(#[from] BadReply),
    /// [`next_row()`](`Cursor::next_row`) or [`next_reply()`](`Cursor::next_reply`)
    /// was called but the server did not send a result set.
    #[error("there is no result set")]
    NoResultSet,
    /// The user called the wrong typed getter, for example
    /// [`get_bool()`](`Cursor::get_bool`) on an INT column.
    #[error("could not convert column {colnr} to {expected_type}: {message}")]
    Conversion {
        colnr: usize,
        expected_type: &'static str,
        message: String,
    },
}

pub type CursorResult<T> = Result<T, CursorError>;

impl From<io::Error> for CursorError {
    fn from(value: io::Error) -> Self {
        IoError::from(value).into()
    }
}

/// Executes queries on a connection and manages retrieval of the
/// results. It can be obtained using the
/// [`cursor()`](`super::conn::Connection::cursor`) method on the connection.
///
/// The method [`execute()`][`Cursor::execute`] can be used to send SQL
/// statements to the server. The server will return zero or more replies,
/// usually one per statement. A reply may be an error, an acknowledgement such
/// as "your UPDATE statement affected 1001 rows", or a result set. This method
/// will immediately abort with `Err(CursorError::Server(_))` if *any* of the
/// replies is an error message, not just the first reply.
///
/// Most retrieval methods on a cursor operate on the *current reply*. To move
/// on to the next reply, call [`next_reply()`][`Cursor::next_reply`]. The only
/// exception is [`next_row()`][`Cursor::next_row`], which will automatically
/// try to skip to the next result set reply if the current reply is not a
/// result set. This is useful because people often write things like
/// ```sql
/// CREATE TABLE foo(..);
/// INSERT INTO foo SELECT .. FROM other_table;
/// INSERT INTO foo SELECT .. FROM yet_another_table;
/// SELECT COUNT(*) FROM foo;
/// ```
/// and they expect to be able to directly retrieve the count, not get an error
/// message "CREATE TABLE did not return a result set". Note that
/// [`next_row()`][`Cursor::next_row`] will *not* automatically skip to the next
/// result set if the current result set is exhausted.
///
/// To retrieve data from a result set, first call
/// [`next_row()`][`Cursor::next_row`]. This tries to move the cursor to the
/// next row and returns a boolean indicating if a new row was found. if so,
/// methods like [`get_str(colnr)`][`Cursor::get_str`] and
/// [`get_i32(colnr)`][`Cursor::get_i32`] can be used to retrieve individual
/// fields from this row.
/// Note that you **must** call [`next_row()`][`Cursor::next_row`] before you
/// call a getter. Before the first call to [`next_row()`][`Cursor::next_row`],
/// the cursor is *before* the first row, not *at* the first row. This behaviour
/// is convenient because it allows to write things like
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let mut cursor: monetdb::Cursor = todo!();
/// cursor.execute("SELECT * FROM mytable")?;
/// while cursor.next_row()? {
///     let value: Option<&str> = cursor.get_str(0)?;
///     println!("{}", value.unwrap());
/// }
/// # Ok(())
/// # }
/// ```
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

    /// Execute the given SQL statements and place the cursor at the first
    /// reply. The results of any earlier queries on this cursor are discarded.

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

    /// Retrieve the number of affected rows from the current reply. INSERT,
    /// UPDATE and SELECT statements provide the number of affected rows, but
    /// for example CREATE TABLE doesn't. Returns a signed value because we're
    /// not entirely sure whether the server ever sends negative values to indicate
    /// exceptional conditions.
    ///
    /// TODO figure this out and deal with it.
    pub fn affected_rows(&self) -> Option<i64> {
        self.replies.affected_rows()
    }

    /// Return `true` if the current reply is a result set.
    pub fn has_result_set(&self) -> bool {
        self.replies.at_result_set()
    }

    /// Try to move the cursor to the next reply.
    pub fn next_reply(&mut self) -> CursorResult<bool> {
        // todo: close server side result set if necessary
        let old = mem::take(&mut self.replies);
        let (new, to_close) = old.into_next_reply()?;
        if let Some(res_id) = to_close {
            self.queue_close(res_id)?;
        }
        self.switch_to_reply(new)
    }

    fn switch_to_reply(&mut self, replies: ReplyParser) -> CursorResult<bool> {
        self.replies = replies;
        let have_next = !matches!(self.replies, ReplyParser::Exhausted(..));
        Ok(have_next)
    }

    fn queue_close(&mut self, res_id: u64) -> CursorResult<()> {
        self.conn.run_locked(|_, delayed, sock| {
            delayed.add_xcommand("close", res_id);
            Ok(sock)
        })?;
        Ok(())
    }

    fn exhaust(&mut self) -> CursorResult<()> {
        loop {
            if let ReplyParser::Exhausted(..) = self.replies {
                return Ok(());
            }
            self.next_reply()?;
        }
    }

    /// Destroy the cursor, discarding all results. This may need to communicate with the server
    /// to release resources there.
    pub fn close(mut self) -> CursorResult<()> {
        self.do_close()?;
        Ok(())
    }

    fn do_close(&mut self) -> CursorResult<()> {
        self.exhaust()?;
        let mut vec = self.replies.take_buffer();
        self.conn.run_locked(|_state, delayed, mut sock| {
            if !delayed.responses.is_empty() {
                sock = delayed.send_delayed(sock)?;
                sock = delayed.recv_delayed(sock, &mut vec)?;
            }
            Ok(sock)
        })
    }

    /// Return information about the columns of the current result set.
    pub fn column_metadata(&self) -> &[ResultColumn] {
        if let ReplyParser::Data(ResultSet { columns, .. }) = &self.replies {
            &columns[..]
        } else {
            &[]
        }
    }

    /// Advance the cursor to the next available row in the result set,
    /// returning a boolean that indicates whether such a row was present.
    ///
    /// When the cursor enters a new result set after
    /// [`execute()`][`Cursor::execute`] or
    /// [`next_reply()`][`Cursor::next_reply`], it is initially positioned
    /// *before* the first row, and the first call to this method will advance
    /// it to be *at* the first row. This means you always have to call this method
    /// before calling getters.
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

/// These getters can be called to retrieve values from the current row, after
/// [`next_row()`][`Cursor::next_row`] has confirmed that that row exists.
/// They return None if the value is NULL.
impl Cursor {
    getter!(get_str, &str);
    getter!(get_bool, bool);
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

impl Drop for Cursor {
    fn drop(&mut self) {
        let _ = self.do_close();
    }
}
