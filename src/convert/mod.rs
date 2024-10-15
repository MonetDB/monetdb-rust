// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

pub mod raw_decimal;

#[cfg(test)]
mod tests;

use std::{
    any::{type_name, Any},
    fmt,
    str::FromStr,
};

use raw_decimal::RawDecimal;

use crate::{
    cursor::replies::{BadReply, ResultSet},
    CursorError, CursorResult,
};

/// A type that can be extracted from a result set.
pub trait FromMonet
where
    Self: Sized,
{
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>>;
}

macro_rules! fromstr_frommonet {
    ($type:ty) => {
        impl FromMonet for $type {
            fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
                let Some(field) = rs.row_set.get_field_raw(colnr) else {
                    return Ok(None);
                };
                transform_fromstr(field)
            }
        }
    };
}

fromstr_frommonet!(bool);
fromstr_frommonet!(i8);
fromstr_frommonet!(u8);
fromstr_frommonet!(i16);
fromstr_frommonet!(u16);
fromstr_frommonet!(i32);
fromstr_frommonet!(u32);
fromstr_frommonet!(i64);
fromstr_frommonet!(u64);
fromstr_frommonet!(i128);
fromstr_frommonet!(u128);
fromstr_frommonet!(isize);
fromstr_frommonet!(usize);
fromstr_frommonet!(f32);
fromstr_frommonet!(f64);

fromstr_frommonet!(RawDecimal<i8>);
fromstr_frommonet!(RawDecimal<u8>);
fromstr_frommonet!(RawDecimal<i16>);
fromstr_frommonet!(RawDecimal<u16>);
fromstr_frommonet!(RawDecimal<i32>);
fromstr_frommonet!(RawDecimal<u32>);
fromstr_frommonet!(RawDecimal<i64>);
fromstr_frommonet!(RawDecimal<u64>);
fromstr_frommonet!(RawDecimal<i128>);
fromstr_frommonet!(RawDecimal<u128>);

/// BLOB
impl FromMonet for Vec<u8> {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        match hex::decode(field) {
            Ok(vec) => Ok(Some(vec)),
            Err(e) => Err(conversion_error::<Self>(e)),
        }
    }
}

/// UUID
#[cfg(feature = "uuid")]
impl FromMonet for uuid::Uuid {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        match uuid::Uuid::try_parse_ascii(field) {
            Ok(u) => Ok(Some(u)),
            Err(e) => Err(conversion_error::<Self>(e)),
        }
    }
}

/// RUST_DECIMAL
#[cfg(feature = "rust_decimal")]
impl FromMonet for rust_decimal::Decimal {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        transform(field, rust_decimal::Decimal::from_str)
    }
}

/// DECIMAL-RS
#[cfg(feature = "decimal-rs")]
impl FromMonet for decimal_rs::Decimal {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        transform(field, decimal_rs::Decimal::from_str)
    }
}

impl FromMonet for std::time::Duration {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(decimal) = <RawDecimal<u64> as FromMonet>::extract(rs, colnr)? else {
            return Ok(None);
        };
        let milliseconds = decimal.at_scale(3).expect(
            "expect server to send day_interval and second_interval with milliseconds precision",
        ); // it's always milliseconds
        let duration = std::time::Duration::from_millis(milliseconds);
        Ok(Some(duration))
    }
}

/////////////////////////////////////////////////////////////////////////////////////////

/// Verify correct UTF-8, return [`CursorError`] if this fails.
pub(crate) fn from_utf8(field: &[u8]) -> CursorResult<&str> {
    match std::str::from_utf8(field) {
        Ok(s) => Ok(s),
        Err(_) => Err(CursorError::BadReply(BadReply::Unicode("result set"))),
    }
}

/// Apply the function to the raw result set field, converting any errors to [`CursorError`].
pub(crate) fn transform<F, T, E>(field: &[u8], f: F) -> CursorResult<Option<T>>
where
    F: for<'x> FnOnce(&'x str) -> Result<T, E>,
    E: fmt::Display,
    T: Any,
{
    let s = from_utf8(field)?;
    match f(s) {
        Ok(value) => Ok(Some(value)),
        Err(e) => Err(conversion_error::<T>(e)),
    }
}

/// Convert raw result set field to a value using [`FromStr`].
pub(crate) fn transform_fromstr<T>(field: &[u8]) -> CursorResult<Option<T>>
where
    T: FromStr + Any,
    <T as FromStr>::Err: fmt::Display,
{
    transform(field, |s| s.parse())
}

fn conversion_error<T: Any>(e: impl fmt::Display) -> CursorError {
    CursorError::Conversion {
        expected_type: type_name::<T>(),
        message: e.to_string().into(),
    }
}
