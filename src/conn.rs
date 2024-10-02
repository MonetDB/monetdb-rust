// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

use std::sync::{
    atomic::{self, AtomicBool},
    Arc, Mutex, TryLockError,
};

use crate::{
    cursor::{delayed::DelayedCommands, Cursor, CursorError, CursorResult},
    framing::{
        connecting::{establish_connection, ConnectResult},
        ServerSock, ServerState,
    },
    parms::Parameters,
};

/// A connection to MonetDB.
///
/// The [top-level documentation](`super#examples`) contains some examples of how a
/// connection can be created.
///
/// Executing queries on a connection is done with a [`Cursor`] object, which
/// can be obtained using the [`cursor()`](`Connection::cursor`) method.
pub struct Connection(Arc<Conn>);

pub(crate) struct Conn {
    pub(crate) reply_size: usize,
    locked: Mutex<Locked>,
    closing: AtomicBool,
}

struct Locked {
    state: ServerState,
    sock: Option<ServerSock>,
    delayed: DelayedCommands,
}

impl Connection {
    /// Create a new connection based on the given [`Parameters`] object.
    pub fn new(parameters: Parameters) -> ConnectResult<Connection> {
        let (sock, state, delayed) = establish_connection(parameters)?;

        let reply_size = state.reply_size;

        let locked = Locked {
            state,
            sock: Some(sock),
            delayed,
        };
        let conn = Conn {
            locked: Mutex::new(locked),
            closing: AtomicBool::new(false),
            reply_size,
        };
        let connection = Connection(Arc::new(conn));

        Ok(connection)
    }

    /// Create a new connection based on the given URL.
    pub fn connect_url(url: &str) -> ConnectResult<Connection> {
        let parms = Parameters::from_url(url)?;
        Self::new(parms)
    }

    /// Create a new [`Cursor`] for this connection
    pub fn cursor(&self) -> Cursor {
        Cursor::new(Arc::clone(&self.0))
    }

    /// Close the connection.
    ///
    /// Any remaining cursors will not be able to fetch new data.
    /// They may still be able to return some already retrieved data but
    /// you shouldn't count on that.
    pub fn close(self) {
        drop(self);
    }

    fn close_connection(&mut self) {
        let conn = self.0.as_ref();
        conn.closing.store(true, atomic::Ordering::SeqCst);
        match conn.locked.try_lock() {
            Ok(mut locked) => locked.sock = None,
            Err(TryLockError::Poisoned(mut poisoned)) => poisoned.get_mut().sock = None,
            Err(TryLockError::WouldBlock) => {}
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.close_connection();
    }
}

impl Conn {
    pub(crate) fn run_locked<F>(&self, f: F) -> CursorResult<()>
    where
        F: for<'x> FnOnce(
            &'x mut ServerState,
            &'x mut DelayedCommands,
            ServerSock,
        ) -> CursorResult<ServerSock>,
    {
        let mut guard = self.locked.lock().unwrap();
        let Some(sock) = guard.sock.take() else {
            return Err(CursorError::Closed);
        };
        let Locked { state, delayed, .. } = &mut *guard;
        match f(state, delayed, sock) {
            Ok(sock) => {
                guard.sock = Some(sock);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
