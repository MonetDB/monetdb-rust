pub mod blockstate;
pub mod connecting;
pub mod reading;
pub mod writing;

use std::{error, fmt, io, net::TcpStream, os::unix::net::UnixStream};

pub const BLOCKSIZE: usize = 8190;

// pub use connecting::connect;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FramingError {
    InvalidBlockSize,
    Unicode,
    TooLong,
}

impl FramingError {
    fn to_str(&self) -> &'static str {
        match self {
            FramingError::InvalidBlockSize => {
                "network layer: invalid block; network byte stream out of sync?"
            }
            FramingError::Unicode => {
                "network layer: invalid utf-8 encoding, block was expected to contain text"
            }
            FramingError::TooLong => "network layer: message too long",
        }
    }
}

impl fmt::Display for FramingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_str().fmt(f)
    }
}

pub type FramingResult<T> = Result<T, FramingError>;

impl From<FramingError> for io::Error {
    fn from(value: FramingError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, value.to_str())
    }
}

impl error::Error for FramingError {}

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
