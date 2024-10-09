// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
pub mod blockstate;
pub mod connecting;
pub mod reading;
pub mod tls;
pub mod writing;

use std::{error, fmt, io, net::TcpStream, os::unix::net::UnixStream, sync::Arc};

use crate::conn::InnerServerMetadata;

pub const BLOCKSIZE: usize = 8190;

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

#[derive(Debug, Clone)]
pub struct ServerState {
    pub initial_auto_commit: bool,
    pub reply_size: usize,
    pub time_zone_seconds: i32,
    pub sql_metadata: Option<Arc<InnerServerMetadata>>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            initial_auto_commit: true,
            reply_size: 100,
            time_zone_seconds: 0,
            sql_metadata: None,
        }
    }
}

trait ServerSockTrait: fmt::Debug + io::Read + io::Write + Send + 'static {}

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
