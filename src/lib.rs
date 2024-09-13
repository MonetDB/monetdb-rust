use std::{error, fmt, io};

#[macro_use]
pub mod our_logger;

pub mod framing;
pub mod parms;
pub mod util;

/// Variant of std::io::Error that implements PartialEq, Eq and Clone.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct IoError(io::ErrorKind, String);

impl error::Error for IoError {}

impl IoError {
    pub fn kind(&self) -> io::ErrorKind {
        self.0
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

impl From<io::Error> for IoError {
    fn from(value: io::Error) -> Self {
        IoError(value.kind(), value.to_string())
    }
}
