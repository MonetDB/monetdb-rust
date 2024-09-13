pub mod blockstate;
pub mod connecting;
pub mod reading;
pub mod writing;

use std::{fmt, io, net::TcpStream, os::unix::net::UnixStream};

use crate::IoError;

pub const BLOCKSIZE: usize = 8190;

// pub use connecting::connect;

#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum MapiError {
    #[error("{0}")]
    IO(#[from] IoError),
}

pub type MapiResult<T> = Result<T, MapiError>;

impl From<io::Error> for MapiError {
    fn from(value: io::Error) -> Self {
        IoError::from(value).into()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ServerState {
    pub reply_size: i64,
}

impl Default for ServerState {
    fn default() -> Self {
        Self { reply_size: 100 }
    }
}

trait ServerSockTrait: fmt::Debug + io::Read + io::Write + 'static {}

impl ServerSockTrait for UnixStream {}

impl ServerSockTrait for TcpStream {}

#[derive(Debug)]
pub struct ServerSock(Box<dyn ServerSockTrait>);

impl ServerSock {
    fn new(sock: impl ServerSockTrait) -> Self {
        ServerSock(Box::new(sock))
    }
}

impl io::Read for ServerSock {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }
}

impl io::Write for ServerSock {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }
}
