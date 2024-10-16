// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

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
fn assert_parse_fails<T>(field: &str)
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
    assert_parse_fails::<i8>("87654");
    assert_parse_fails::<i8>("-87654");
    assert_parses("9", 9u8);
    assert_parse_fails::<u8>("87654");
    assert_parse_fails::<u8>("-87654");

    assert_parses("9", 9i16);
    assert_parse_fails::<i16>("87654");
    assert_parse_fails::<i16>("-87654");
    assert_parses("9", 9u16);
    assert_parse_fails::<u16>("87654");
    assert_parse_fails::<u16>("-87654");

    assert_parses("9", 9i32);
    assert_parses("87654", 87654i32);
    assert_parses("-87654", -87654i32);
    assert_parses("9", 9u32);
    assert_parses("87654", 87654u32);
    assert_parse_fails::<u32>("-87654");

    assert_parses("9", 9i64);
    assert_parses("87654", 87654i64);
    assert_parses("-87654", -87654i64);
    assert_parses("9", 9u64);
    assert_parses("87654", 87654u64);
    assert_parse_fails::<u64>("-87654");

    assert_parses("9", 9i128);
    assert_parses("87654", 87654i128);
    assert_parses("-87654", -87654i128);
    assert_parses("9", 9u128);
    assert_parses("87654", 87654u128);
    assert_parse_fails::<u128>("-87654");

    assert_parses("9", 9isize);
    assert_parses("87654", 87654isize);
    assert_parses("-87654", -87654isize);
    assert_parses("9", 9usize);
    assert_parses("87654", 87654usize);
    assert_parse_fails::<usize>("-87654");
}

#[test]
fn test_rawdecimal() {
    assert_parses("1.23", RawDecimal(123i32, 2));
    assert_parses("1.20", RawDecimal(120i32, 2));
    assert_parses("-1.23", RawDecimal(-123i32, 2));

    assert_parses("1.23", RawDecimal(123u32, 2));
    assert_parses("1.20", RawDecimal(120u32, 2));
    assert_parse_fails::<RawDecimal<u32>>("-1.23");

    assert_parses("1.23", RawDecimal(123i8, 2));
    assert_parses("1.27", RawDecimal(127i8, 2));
    assert_parse_fails::<RawDecimal<i8>>("1.28");

    assert_parses("-1.23", RawDecimal(-123i8, 2));
    assert_parses("-1.27", RawDecimal(-127i8, 2));
    assert_parse_fails::<RawDecimal<i8>>("-1.29");

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

    assert_parse_fails::<bool>("True");
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

#[test]
fn test_std_duration() {
    use std::time::Duration;
    assert_parses("86400.000", Duration::from_secs(24 * 3600));
    // Negative durations are not supported
    assert_parse_fails::<Duration>("-86400.000");
}
