// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use std::{error, fmt, io};

#[macro_use]
pub mod our_logger;

pub mod conn;
pub mod cursor;
pub mod framing;
pub mod monettypes;
pub mod parms;
pub mod util;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const PUBLIC_NAME: &str = concat!("monetdb-rust ", env!("CARGO_PKG_VERSION"));

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
