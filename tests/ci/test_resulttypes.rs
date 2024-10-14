// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

#![allow(dead_code, unused_imports)]
use std::{
    any::{type_name, type_name_of_val, Any},
    fmt::{self, Debug},
    str::FromStr,
};

use monetdb::{
    convert::{raw_decimal::RawDecimal, FromMonet},
    CursorResult,
};

use crate::context::with_shared_cursor;

fn check<T>(sql_repr: &str, expected: T)
where
    T: FromMonet + PartialEq + Debug + Clone + Any,
{
    with_shared_cursor(|cursor| {
        cursor.execute(&format!("SELECT {sql_repr}"))?;
        assert!(cursor.next_row()?);
        let value: Option<T> = cursor.get(0)?;
        assert_eq!(
            value,
            Some(expected.clone()),
            "for type {}",
            type_name_of_val(&expected)
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn test_varchar() {
    with_shared_cursor(|cursor| {
        cursor.execute(r##" SELECT 'mo"ne\\t''db' "##)?;
        assert!(cursor.next_row()?);
        let value: Option<&str> = cursor.get_str(0)?;
        assert_eq!(value, Some(r##"mo"ne\t'db"##));
        Ok(())
    })
    .unwrap()
}

#[test]
fn test_ints() {
    for &value in &[0i8, 10, -10] {
        check(&value.to_string(), value);
    }

    for &value in &[0u8, 10] {
        check(&value.to_string(), value);
    }

    for &value in &[0i16, 10, -10] {
        check(&value.to_string(), value);
    }

    for &value in &[0u16, 10] {
        check(&value.to_string(), value);
    }

    for &value in &[0i32, 10, -10] {
        check(&value.to_string(), value);
    }

    for &value in &[0u32, 10] {
        check(&value.to_string(), value);
    }

    for &value in &[0i64, 10, -10] {
        check(&value.to_string(), value);
    }

    for &value in &[0u64, 10] {
        check(&value.to_string(), value);
    }

    for &value in &[0i128, 10, -10] {
        check(&value.to_string(), value);
    }

    for &value in &[0u128, 10] {
        check(&value.to_string(), value);
    }

    for &value in &[0isize, 10, -10] {
        check(&value.to_string(), value);
    }

    for &value in &[0usize, 10] {
        check(&value.to_string(), value);
    }
}

#[test]
fn test_blob() {
    check(r#" BLOB '414243' "#, Vec::from("ABC"));
}

#[test]
#[cfg(feature = "uuid")]
fn test_uuid() {
    let u = uuid::Uuid::parse_str("7b4dcdd0-e0f2-4d05-a81b-599f445843b6").unwrap();

    check(r#"  UUID '7b4dcdd0-e0f2-4d05-a81b-599f445843b6'  "#, u);
    check(r#"  UUID '7b4dcdd0e0f24d05a81b599f445843b6'  "#, u);
    check(r#"  UUID '7B4DCDD0E0F24D05A81B599F445843B6'  "#, u);
}

#[test]
fn test_rawdecimal() {
    check("CAST( 12.34 AS DECIMAL(7,3))", RawDecimal(12340i32, 3));
    check("CAST( -12.34 AS DECIMAL(7,3))", RawDecimal(-12340i32, 3));

    check("CAST( 12.34 AS DECIMAL(7,0))", RawDecimal(12, 0));
    check("CAST( -12.34 AS DECIMAL(7,0))", RawDecimal(-12, 0));
}

#[test]
fn test_decimal_as_float() {
    check("CAST( 12.34 AS DECIMAL(7,3))", 12.34f32);
    check("CAST( 12.34 AS DECIMAL(7,3))", 12.34f64);
    check("CAST( -12.34 AS DECIMAL(7,3))", -12.34f32);
    check("CAST( -12.34 AS DECIMAL(7,3))", -12.34f64);

    check("CAST( 12.34 AS DECIMAL(7,0))", 12.0f32);
    check("CAST( 12.34 AS DECIMAL(7,0))", 12.0f64);
    check("CAST( -12.34 AS DECIMAL(7,0))", -12.0f32);
    check("CAST( -12.34 AS DECIMAL(7,0))", -12.0f64);
}

#[cfg(feature = "rust_decimal")]
#[test]
fn test_rust_decimal() {
    use rust_decimal::Decimal;

    let d2 = Decimal::from_str("12.34").unwrap();
    assert_eq!(d2.scale(), 2);

    check("CAST( 12.34 AS DECIMAL(7,3))", d2);
    check("CAST( -12.34 AS DECIMAL(7,3))", -d2);

    check("CAST( 12.34 AS DECIMAL(7,0))", Decimal::from(12));
    check("CAST( -12.34 AS DECIMAL(7,0))", Decimal::from(-12));
}

#[cfg(feature = "decimal-rs")]
#[test]
fn test_decimal_rs() {
    use decimal_rs::Decimal;

    let d2 = Decimal::from_str("12.34").unwrap();
    assert_eq!(d2.scale(), 2);

    check("CAST( 12.34 AS DECIMAL(7,3))", d2);
    check("CAST( -12.34 AS DECIMAL(7,3))", -d2);

    check("CAST( 12.34 AS DECIMAL(7,0))", Decimal::from(12));
    check("CAST( -12.34 AS DECIMAL(7,0))", Decimal::from(-12));
}
