#![allow(dead_code)]

use std::{
    result,
    sync::{Arc, Mutex},
};

use crate::{
    framing::{
        connecting::{establish_connection, ConnResult},
        ServerSock, ServerState,
    },
    parms::Parameters,
};

pub struct Connection(Arc<Conn>);

struct Conn {
    locked: Mutex<Locked>,
}

struct Locked {
    state: ServerState,
    sock: Option<ServerSock>,
}

impl Connection {
    pub fn new(parameters: Parameters) -> ConnResult<Connection> {
        let (sock, state) = establish_connection(parameters)?;
        let connection = Self::from_parts(sock, state);
        Ok(connection)
    }

    pub fn connect_url(url: impl AsRef<str>) -> ConnResult<Connection> {
        let parms = Parameters::from_url(url.as_ref())?;
        Self::new(parms)
    }

    pub(crate) fn from_parts(sock: ServerSock, state: ServerState) -> Self {
        let locked = Locked {
            state,
            sock: Some(sock),
        };
        let conn = Conn {
            locked: Mutex::new(locked),
        };
        Connection(Arc::new(conn))
    }
}

impl Conn {
    fn run_locked<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'x> FnOnce(ServerSock, &'x mut ServerState) -> result::Result<(ServerSock, T), E>,
    {
        let mut guard = self.locked.lock().unwrap();
        let Some(sock) = guard.sock.take() else {
            panic!("connection has been closed"); // this should really be an Error
        };
        match f(sock, &mut guard.state) {
            Ok((sock, value)) => {
                guard.sock = Some(sock);
                Ok(value)
            }
            Err(e) => Err(e),
        }
    }
}
