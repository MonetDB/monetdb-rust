// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
#![doc = include_str!("toplevel.md")]

#[macro_use]
mod our_logger;

mod conn;
mod cursor;
mod framing;
pub mod monettypes;
pub mod parms;
mod util;

pub use conn::Connection;
pub use cursor::{replies::ResultColumn, Cursor, CursorError, CursorResult};
pub use framing::connecting::{ConnectError, ConnectResult};
pub use monettypes::MonetType;
pub use parms::Parameters;

/// The version number of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The identification string of this MonetDB client.
///
/// Used for example to set the value in the 'client' column of MonetDB's
/// `sys.sessions` table.
pub const PUBLIC_NAME: &str = concat!("monetdb-rust ", env!("CARGO_PKG_VERSION"));
