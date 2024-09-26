use std::{io, sync::Arc};

use rustls::{pki_types::ServerName, ClientConnection, StreamOwned};

use crate::{
    framing::{
        connecting::{ConnResult, ConnectError},
        ServerSock, ServerSockTrait,
    },
    parms::Validated,
};

pub fn wrap_with_rustls(parms: &Validated, sock: ServerSock) -> ConnResult<ServerSock> {
    wrap_inner(parms, sock).map_err(|e| ConnectError::TlsError(e.to_string()))
}

fn wrap_inner(
    parms: &Validated,
    sock: ServerSock,
) -> Result<ServerSock, Box<dyn std::error::Error>> {
    // we should really cache this
    let config = Arc::new(rustls_platform_verifier::tls_config());

    let server_name = parms.connect_tcp.to_string();
    let server_name = ServerName::try_from(server_name)?;

    let client = ClientConnection::new(config, server_name)?;

    let stream = StreamOwned::new(client, sock);
    let wrapped = StreamWrapper(stream);

    Ok(ServerSock::new(wrapped))
}

/// We need to wrap the rustls::Stream so we can make it implement ServerSockTrait.
#[derive(Debug)]
struct StreamWrapper(pub StreamOwned<ClientConnection, ServerSock>);

impl io::Read for StreamWrapper {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl io::Write for StreamWrapper {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl ServerSockTrait for StreamWrapper {}
