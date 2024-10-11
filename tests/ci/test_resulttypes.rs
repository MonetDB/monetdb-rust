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
};

use monetdb::{convert::FromMonet, CursorResult};

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
