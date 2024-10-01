// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
mod parameters;
mod urlparser;
#[cfg(test)]
mod urltests;

use std::{borrow::Cow, fmt, str::FromStr};

pub use parameters::{parse_bool, Parameters, Parm, Validated, Value, PARM_TABLE_SIZE};

#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum ParmError {
    #[error("unknown parameter '{0}'")]
    UnknownParameter(String),
    #[error("invalid value for parameter '{0}'")]
    InvalidValue(Parm),
    #[error("parameter '{0}': invalid boolean value")]
    InvalidBool(Parm),
    #[error("parameter '{0}': invalid integer value")]
    InvalidInt(Parm),
    #[error("parameter '{0}': must be string")]
    MustBeString(Parm),
    #[error("parameter 'binary' must be on, off, true, false, yes, no or 0..65545")]
    InvalidBinary,
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("invalid percent encoding in url")]
    InvalidPercentEncoding,
    #[error("invalid utf-8 after percent decoding url")]
    InvalidPercentUtf8,
    #[error("cannot combine 'host' and 'sock'")]
    HostSockConflict,
    #[error("parameter '{0}' is only valid with TLS is enabled")]
    OnlyWithTls(Parm),
    #[error("parameter 'clientcert' requires 'clientkey' as well")]
    ClientCertRequiresKey,
    #[error("parameter '{0}' is not allowed as query parameter")]
    NotAllowedAsQuery(Parm),
    #[error("parameter: '{0}': must not contain newlines")]
    ClientInfoNewline(Parm),
}

pub type ParmResult<T> = Result<T, ParmError>;
