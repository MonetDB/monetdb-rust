// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

use std::{
    any::{type_name, Any},
    fmt,
    str::FromStr,
};

use crate::{cursor::replies::BadReply, CursorError, CursorResult};

/// A type that can be extracted from `&'a [u8]`.
pub trait FromMonet
where
    Self: Sized,
{
    fn from_monet(bytes: &[u8]) -> CursorResult<Self>;
}

macro_rules! fromstr_frommonet {
    ($type:ty) => {
        impl FromMonet for $type {
            fn from_monet(bytes: &[u8]) -> CursorResult<Self> {
                let x: $type = transform_fromstr(bytes)?;
                Ok(x)
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

/// BLOB
impl FromMonet for Vec<u8> {
    fn from_monet(field: &[u8]) -> CursorResult<Self> {
        match hex::decode(field) {
            Ok(vec) => Ok(vec),
            Err(e) => Err(conversion_error::<Self>(e)),
        }
    }
}

/// Verify correct UTF-8, return [`CursorError`] if this fails.
pub(crate) fn from_utf8(field: &[u8]) -> CursorResult<&str> {
    match std::str::from_utf8(field) {
        Ok(s) => Ok(s),
        Err(_) => Err(CursorError::BadReply(BadReply::Unicode("result set"))),
    }
}

/// Apply the function to the raw result set field, converting any errors to [`CursorError`].
pub(crate) fn transform<F, T, E>(field: &[u8], f: F) -> CursorResult<T>
where
    F: for<'x> FnOnce(&'x str) -> Result<T, E>,
    E: fmt::Display,
    T: Any,
{
    let s = from_utf8(field)?;
    match f(s) {
        Ok(value) => Ok(value),
        Err(e) => Err(conversion_error::<T>(e)),
    }
}

/// Convert raw result set field to a value using [`FromStr`].
pub(crate) fn transform_fromstr<T>(field: &[u8]) -> CursorResult<T>
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

#[cfg(test)]
mod tests {
    use claims::assert_err;

    use super::*;

    #[track_caller]
    fn assert_parses<T>(field: &[u8], value: T)
    where
        T: FromMonet,
        T: fmt::Debug + PartialEq,
    {
        let parsed = T::from_monet(field);
        assert_eq!(parsed, Ok(value));
    }

    #[track_caller]
    fn assert_parse_fails<T>(field: &[u8], _dummy: T)
    where
        T: FromMonet,
        T: fmt::Debug + PartialEq,
    {
        let parsed = T::from_monet(field);
        assert_err!(parsed);
    }

    #[test]
    fn test_floats() {
        assert_parses(b"1.23", 1.23);
        assert_parses(b"-1e-3", -0.001);
    }

    #[test]
    fn test_ints() {
        assert_parses(b"9", 9i8);
        assert_parse_fails(b"87654", 0i8);
        assert_parse_fails(b"-87654", 0i8);
        assert_parses(b"9", 9u8);
        assert_parse_fails(b"87654", 0u8);
        assert_parse_fails(b"-87654", 0u8);

        assert_parses(b"9", 9i16);
        assert_parse_fails(b"87654", 0i16);
        assert_parse_fails(b"-87654", 0i16);
        assert_parses(b"9", 9u16);
        assert_parse_fails(b"87654", 0u16);
        assert_parse_fails(b"-87654", 0u16);

        assert_parses(b"9", 9i32);
        assert_parses(b"87654", 87654i32);
        assert_parses(b"-87654", -87654i32);
        assert_parses(b"9", 9u32);
        assert_parses(b"87654", 87654u32);
        assert_parse_fails(b"-87654", 0u32);

        assert_parses(b"9", 9i64);
        assert_parses(b"87654", 87654i64);
        assert_parses(b"-87654", -87654i64);
        assert_parses(b"9", 9u64);
        assert_parses(b"87654", 87654u64);
        assert_parse_fails(b"-87654", 0u64);

        assert_parses(b"9", 9i128);
        assert_parses(b"87654", 87654i128);
        assert_parses(b"-87654", -87654i128);
        assert_parses(b"9", 9u128);
        assert_parses(b"87654", 87654u128);
        assert_parse_fails(b"-87654", 0u128);

        assert_parses(b"9", 9isize);
        assert_parses(b"87654", 87654isize);
        assert_parses(b"-87654", -87654isize);
        assert_parses(b"9", 9usize);
        assert_parses(b"87654", 87654usize);
        assert_parse_fails(b"-87654", 0usize);
    }

    #[test]
    fn test_bool() {
        assert_parses(b"true", true);
        assert_parses(b"false", false);

        assert_parse_fails(b"True", true);
    }
}
