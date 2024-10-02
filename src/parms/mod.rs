// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

//! Helpers for working with connection parameters.
//!
//! Enum [`Parm`] identifies all things that can be configured, for example
//! [`Parm::Host`], [`Parm::Port`] and [`Parm::Password`].
//!
//! Type [`Value`] can hold the possible values for these parameters. It doesn't
//! really care whether these are specified as strings or typed values such as
//! `bool` or `i32`.
//!
//! Type [`Parameters`] holds an almost arbitray mapping between Parms and
//! Values. The values can be invalid or inconsistent. It has a getter and
//! setter for the Value of a Parm, and a large number of typed setters such as
//! `set_autocommit`. These setters come in two varieties: `set_autocommit(&mut
//! self, bool)` and `with_autocommit(self, bool)`.
//!
//! Type [`Validated`] is created by calling the
//! [`validate()`][`Parameters::validate`] method on a Parameters object.
//! If this succeeds, the values of the parameters are sensible and the Validated object
//! knows how to make a number of policy decisions, such as whether to connect to
//! a Unix Domain socket, a TCP socket or both.
mod parameters;
mod urlparser;
#[cfg(test)]
mod urltests;

use std::{borrow::Cow, fmt, str::FromStr};

pub use parameters::{parse_bool, Parameters, Parm, TlsVerify, Validated, Value, PARM_TABLE_SIZE};

/// An error that occurs while dealing with [`Parameters`].
#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum ParmError {
    /// No parameter with the given name known.
    #[error("unknown parameter '{0}'")]
    UnknownParameter(String),
    /// The given parameter has an invalid value.
    #[error("invalid value for parameter '{0}'")]
    InvalidValue(Parm),
    /// The given parameter is invalid as a boolean.
    #[error("parameter '{0}': invalid boolean value")]
    InvalidBool(Parm),
    /// The given parameter is invalid as an integer.
    #[error("parameter '{0}': invalid integer value")]
    InvalidInt(Parm),
    /// The given parameter must be a string, cannot be set to an integer, bool or something else.
    #[error("parameter '{0}': must be string")]
    MustBeString(Parm),
    /// Invalid value for [`Parm::Binary`], must be on, off, true, false, yes, no or 0..65535.
    #[error("parameter 'binary' must be on, off, true, false, yes, no or 0..65535")]
    InvalidBinary,
    /// An URL was invalid for the given reason
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("invalid percent encoding in url")]
    /// URL percent encoding was invalid
    InvalidPercentEncoding,
    /// After percent decoding the result was not valid UTF-8.
    #[error("invalid utf-8 after percent decoding url")]
    InvalidPercentUtf8,
    /// [`Parm::Host`] and [`Parm::Sock`] cannot be combined
    #[error("cannot combine 'host' and 'sock'")]
    HostSockConflict,
    /// The given parameter is only valid when TLS is enabled
    #[error("parameter '{0}' is only valid with TLS is enabled")]
    OnlyWithTls(Parm),
    /// The [`Parm::ClientCert`] parameter requires [`Parm::ClientKey`] as well.
    #[error("parameter 'clientcert' requires 'clientkey' as well")]
    ClientCertRequiresKey,
    /// The given parameter is not allowed as a query parameter.
    #[error("parameter '{0}' is not allowed as query parameter")]
    NotAllowedAsQuery(Parm),
    /// The given parameter is not allowed to contain newlines.
    #[error("parameter: '{0}': must not contain newlines")]
    ClientInfoNewline(Parm),
}

pub type ParmResult<T> = Result<T, ParmError>;
