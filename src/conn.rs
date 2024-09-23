#![allow(dead_code)]

use std::sync::{
    atomic::{self, AtomicBool},
    Arc, Mutex, TryLockError,
};

use crate::{
    cursor::{delayed::DelayedCommands, Cursor, CursorError, CursorResult},
    framing::{
        connecting::{establish_connection, ConnResult},
        ServerSock, ServerState,
    },
    parms::Parameters,
};

pub struct Connection(Arc<Conn>);

pub(crate) struct Conn {
    locked: Mutex<Locked>,
    closing: AtomicBool,
}

struct Locked {
    state: ServerState,
    sock: Option<ServerSock>,
    delayed: DelayedCommands,
}

impl Connection {
    pub fn new(parameters: Parameters) -> ConnResult<Connection> {
        let (sock, state, delayed) = establish_connection(parameters)?;
        let connection = Self::from_parts(sock, state, delayed);
        Ok(connection)
    }

    pub fn connect_url(url: impl AsRef<str>) -> ConnResult<Connection> {
        let parms = Parameters::from_url(url.as_ref())?;
        Self::new(parms)
    }

    pub(crate) fn from_parts(sock: ServerSock, state: ServerState, delayed: DelayedCommands) -> Self {
        let locked = Locked {
            state,
            sock: Some(sock),
            delayed,
        };
        let conn = Conn {
            locked: Mutex::new(locked),
            closing: AtomicBool::new(false),
        };
        Connection(Arc::new(conn))
    }

    pub fn cursor(&self) -> Cursor {
        Cursor::new(Arc::clone(&self.0))
    }

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
        F: for<'x> FnOnce(&'x mut ServerState, &'x mut DelayedCommands, ServerSock) -> CursorResult<ServerSock>,
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
