// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

pub mod raw_decimal;

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

#[cfg(test)]
mod tests {
    use claims::{assert_err, assert_matches};

    use crate::{
        cursor::{replies::ReplyBuf, rowset::RowSet},
        MonetType, ResultColumn,
    };

    use super::*;

    fn extract_from_fake_resultset<T: FromMonet + fmt::Debug>(
        coltype: MonetType,
        field: &str,
    ) -> CursorResult<Option<T>> {
        let columns = vec![
            ResultColumn::new("%0", coltype),
            ResultColumn::new("%1", coltype),
        ];
        let body = format!("[ NULL,\t{field}\t]\n");
        let replybuf = ReplyBuf::new(body.into());
        let mut row_set = RowSet::new(replybuf, columns.len());
        row_set.advance().unwrap();

        let rs = ResultSet {
            result_id: 0,
            next_row: 0,
            total_rows: 1,
            columns,
            row_set,
            stashed: None,
            to_close: None,
        };

        let col0 = T::extract(&rs, 0);
        assert_matches!(col0, Ok(None));

        T::extract(&rs, 1)
    }

    #[track_caller]
    fn assert_parses<T>(field: &str, value: T)
    where
        T: FromMonet,
        T: fmt::Debug + PartialEq,
    {
        let parsed = extract_from_fake_resultset(MonetType::Inet, field);
        assert_eq!(parsed, Ok(Some(value)));
    }

    #[track_caller]
    fn assert_parse_fails<T>(field: &str, _dummy: T)
    where
        T: FromMonet,
        T: fmt::Debug + PartialEq,
    {
        let parsed = extract_from_fake_resultset::<T>(MonetType::Inet, field);
        assert_err!(parsed);
    }

    #[test]
    fn test_floats() {
        assert_parses("1.23", 1.23);
        assert_parses("-1e-3", -0.001);
    }

    #[test]
    fn test_ints() {
        assert_parses("9", 9i8);
        assert_parse_fails("87654", 0i8);
        assert_parse_fails("-87654", 0i8);
        assert_parses("9", 9u8);
        assert_parse_fails("87654", 0u8);
        assert_parse_fails("-87654", 0u8);

        assert_parses("9", 9i16);
        assert_parse_fails("87654", 0i16);
        assert_parse_fails("-87654", 0i16);
        assert_parses("9", 9u16);
        assert_parse_fails("87654", 0u16);
        assert_parse_fails("-87654", 0u16);

        assert_parses("9", 9i32);
        assert_parses("87654", 87654i32);
        assert_parses("-87654", -87654i32);
        assert_parses("9", 9u32);
        assert_parses("87654", 87654u32);
        assert_parse_fails("-87654", 0u32);

        assert_parses("9", 9i64);
        assert_parses("87654", 87654i64);
        assert_parses("-87654", -87654i64);
        assert_parses("9", 9u64);
        assert_parses("87654", 87654u64);
        assert_parse_fails("-87654", 0u64);

        assert_parses("9", 9i128);
        assert_parses("87654", 87654i128);
        assert_parses("-87654", -87654i128);
        assert_parses("9", 9u128);
        assert_parses("87654", 87654u128);
        assert_parse_fails("-87654", 0u128);

        assert_parses("9", 9isize);
        assert_parses("87654", 87654isize);
        assert_parses("-87654", -87654isize);
        assert_parses("9", 9usize);
        assert_parses("87654", 87654usize);
        assert_parse_fails("-87654", 0usize);
    }

    #[test]
    fn test_rawdecimal() {
        assert_parses("1.23", RawDecimal(123i32, 2));
        assert_parses("1.20", RawDecimal(120i32, 2));
        assert_parses("-1.23", RawDecimal(-123i32, 2));

        assert_parses("1.23", RawDecimal(123u32, 2));
        assert_parses("1.20", RawDecimal(120u32, 2));
        assert_parse_fails("-1.23", RawDecimal(0u32, 0));

        assert_parses("1.23", RawDecimal(123i8, 2));
        assert_parses("1.27", RawDecimal(127i8, 2));
        assert_parse_fails("1.28", RawDecimal(123i8, 2));

        assert_parses("-1.23", RawDecimal(-123i8, 2));
        assert_parses("-1.27", RawDecimal(-127i8, 2));
        assert_parse_fails("-1.29", RawDecimal(123i8, 2));

        // If scale is 0, MonetDB omits the period as well

        assert_parses("1", RawDecimal(1, 0));
        assert_parses("10", RawDecimal(10, 0));
        assert_parses("-1", RawDecimal(-1, 0));
        assert_parses("-10", RawDecimal(-10, 0));
    }

    #[test]
    fn test_bool() {
        assert_parses("true", true);
        assert_parses("false", false);

        assert_parse_fails("True", true);
    }

    #[test]
    fn test_blob() {
        assert_parses("466f6f", Vec::from(b"Foo"));
    }

    #[test]
    #[cfg(feature = "uuid")]
    fn test_uuid() {
        let expected = uuid::Uuid::from_str("444fcb84-9a7d-4fe1-adfa-7eae290328c3").unwrap();
        assert_parses("444fcb84-9a7d-4fe1-adfa-7eae290328c3", expected);
    }

    #[test]
    #[cfg(feature = "rust_decimal")]
    fn test_rust_decimal() {
        use rust_decimal::Decimal;
        let s = "-123.45";
        let d = Decimal::from_str(s).unwrap();
        assert_parses(s, d);
    }

    #[test]
    #[cfg(feature = "decimal-rs")]
    fn test_decimal_rs() {
        use decimal_rs::Decimal;
        let s = "-123.45";
        let d = Decimal::from_str(s).unwrap();
        assert_parses(s, d);
    }
}
