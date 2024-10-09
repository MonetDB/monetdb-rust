// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

use std::{
    collections::HashMap,
    sync::{
        atomic::{self, AtomicBool},
        Arc, Mutex, TryLockError,
    },
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

    pub fn metadata(&mut self) -> CursorResult<ServerMetadata> {
        let mut inner = None;
        self.0.run_locked(|state, _delayed, sock| {
            inner = state.sql_metadata.clone();
            Ok(sock)
        })?;
        if let Some(md) = inner {
            return Ok(ServerMetadata(md));
        }

        // create it and put it in the state
        // (ignore harmless race condition)
        let new_metadata = ServerMetadata::new(self)?;
        self.0.run_locked(|state, _delayed, sock| {
            state.sql_metadata = Some(Arc::clone(&new_metadata.0));
            Ok(sock)
        })?;
        Ok(new_metadata)
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

#[derive(Debug, Clone)]
pub struct ServerMetadata(Arc<InnerServerMetadata>);

#[derive(Debug, Clone)]
pub struct InnerServerMetadata {
    environment: HashMap<String, String>,
    version: (u16, u16, u16),
    prehash_algo: &'static str,
}

impl ServerMetadata {
    fn new(conn: &mut Connection) -> CursorResult<Self> {
        let mut cursor = conn.cursor();
        cursor.execute("SELECT name, value FROM sys.environment")?;
        let mut environment = HashMap::new();
        while cursor.next_row()? {
            let name = cursor
                .get_str(0)?
                .expect("sys.environment.name should not be null");
            let value = cursor.get_str(1)?.unwrap_or("");
            environment.insert(name.to_string(), value.to_string());
        }

        // parse version
        let Some(v) = environment.get("monet_version") else {
            return Err(CursorError::Metadata(
                "'monet_version' not found in environment",
            ));
        };
        let mut parts = v.split('.');
        let mut next_part = || -> CursorResult<u16> {
            let Some(s) = parts.next() else {
                return Err(CursorError::Metadata(
                    "'monet_version' does not have 3 components",
                ));
            };
            s.parse()
                .map_err(|_| CursorError::Metadata("invalid int component in 'monet_release'"))
        };
        let major = next_part()?;
        let minor = next_part()?;
        let patch = next_part()?;
        if parts.next().is_some() {
            return Err(CursorError::Metadata(
                "'monet_version' has more than 3 components",
            ));
        }
        let version = (major, minor, patch);

        let mut prehash_algo: &'static str = "";
        conn.0.run_locked(|state, _delayed, sock| {
            prehash_algo = state.prehash_algo;
            Ok(sock)
        })?;

        let inner = InnerServerMetadata {
            environment,
            version,
            prehash_algo,
        };
        let metadata = ServerMetadata(Arc::new(inner));
        Ok(metadata)
    }

    pub fn env(&self, key: &str) -> Option<&str> {
        self.0.environment.get(key).map(String::as_ref)
    }

    pub fn version(&self) -> (u16, u16, u16) {
        self.0.version
    }

    pub fn password_prehash_algo(&self) -> &str {
        self.0.prehash_algo
    }
}
