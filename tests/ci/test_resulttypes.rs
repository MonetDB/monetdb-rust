// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

#![allow(dead_code, unused_imports)]

use anyhow::Result as AResult;
use std::{
    any::{type_name, type_name_of_val, Any},
    fmt::{self, Debug},
    str::FromStr,
};

use monetdb::{
    convert::{
        raw_decimal::RawDecimal,
        raw_temporal::{RawDate, RawTime, RawTimeTz, RawTimestamp, RawTimestampTz},
        FromMonet,
    },
    Connection, Cursor, CursorResult, Parameters,
};

use crate::context::{get_server, with_shared_cursor};

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

#[test]
fn test_std_duration() {
    use std::time::Duration;

    let second = Duration::from_secs(1);
    let minute = 60 * second;
    let hour = 60 * minute;
    let day = 24 * hour;
    check("CAST('10' AS INTERVAL DAY)", 10 * day);
    check("CAST('10' AS INTERVAL HOUR)", 10 * hour);
    check("CAST('10' AS INTERVAL MINUTE)", 10 * minute);
    check("CAST('10' AS INTERVAL SECOND)", 10 * second);
    check(
        "CAST('10' AS INTERVAL SECOND) / 4",
        Duration::from_millis(2500),
    );
}

fn check_temporal<F, T>(it_sql: &str, expected_sql: &str, extractor: F)
where
    F: Fn(&T) -> String,
    T: FromMonet,
{
    with_shared_cursor(|cursor| {
        let query = format!("WITH mapped AS (SELECT tsz, {it_sql} AS it FROM temporal) SELECT tsz, it, {expected_sql} AS expected FROM mapped");
        cursor.execute(&query)?;
        let mut i = 0;
        while cursor.next_row()? {
            let value = cursor.get::<T>(1)?;
            let expected = cursor.get_str(2)?.map(String::from);
            let actual = value.map(|x| extractor(&x));
            assert_eq!(expected, actual, "row {i} with tsz = {:?}", cursor.get_str(0));
            i += 1;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn test_date_year() {
    check_temporal(
        "CAST(tsz AS DATE)",
        "EXTRACT(YEAR FROM it)",
        |d: &RawDate| d.year.to_string(),
    );
}

#[test]
fn test_date_month() {
    check_temporal(
        "CAST(tsz AS DATE)",
        "EXTRACT(MONTH FROM it)",
        |d: &RawDate| d.month.to_string(),
    );
}

#[test]
fn test_date_day() {
    check_temporal(
        "CAST(tsz AS DATE)",
        "EXTRACT(DAY FROM it)",
        |d: &RawDate| d.day.to_string(),
    );
}

#[test]
fn test_time_hour() {
    check_temporal(
        "CAST(tsz AS TIME)",
        "EXTRACT(HOUR FROM it)",
        |t: &RawTime| t.hours.to_string(),
    );
}

#[test]
fn test_time_minute() {
    check_temporal(
        "CAST(tsz AS TIME)",
        "EXTRACT(MINUTE FROM it)",
        |t: &RawTime| t.minutes.to_string(),
    );
}

#[test]
fn test_time_second() {
    check_temporal(
        "CAST(tsz AS TIME)",
        "EXTRACT(SECOND FROM it)",
        |t: &RawTime| format!("{:.6}", t.microseconds() as f64 / 1e6),
    );
}

#[test]
fn test_timestamp_year() {
    check_temporal(
        "CAST(tsz AS TIMESTAMP)",
        "EXTRACT(YEAR FROM it)",
        |ts: &RawTimestamp| ts.date.year.to_string(),
    );
}

#[test]
fn test_timestamp_month() {
    check_temporal(
        "CAST(tsz AS TIMESTAMP)",
        "EXTRACT(MONTH FROM it)",
        |ts: &RawTimestamp| ts.date.month.to_string(),
    );
}

#[test]
fn test_timestamp_day() {
    check_temporal(
        "CAST(tsz AS TIMESTAMP)",
        "EXTRACT(DAY FROM it)",
        |ts: &RawTimestamp| ts.date.day.to_string(),
    );
}

#[test]
fn test_timestamp_hour() {
    check_temporal(
        "CAST(tsz AS TIMESTAMP)",
        "EXTRACT(HOUR FROM it)",
        |ts: &RawTimestamp| ts.time.hours.to_string(),
    );
}

#[test]
fn test_timestamp_minute() {
    check_temporal(
        "CAST(tsz AS TIMESTAMP)",
        "EXTRACT(MINUTE FROM it)",
        |ts: &RawTimestamp| ts.time.minutes.to_string(),
    );
}

#[test]
fn test_timestamp_second() {
    check_temporal(
        "CAST(tsz AS TIMESTAMP)",
        "EXTRACT(SECOND FROM it)",
        |ts: &RawTimestamp| format!("{:.6}", ts.time.microseconds() as f64 / 1e6),
    );
}

#[test]
fn test_timetz_hour() {
    check_temporal(
        "CAST(tsz AS TIMETZ)",
        "EXTRACT(HOUR FROM it)",
        |t: &RawTimeTz| t.time.hours.to_string(),
    );
}

#[test]
fn test_timetz_minute() {
    check_temporal(
        "CAST(tsz AS TIMETZ)",
        "EXTRACT(MINUTE FROM it)",
        |t: &RawTimeTz| t.time.minutes.to_string(),
    );
}

#[test]
fn test_timetz_second() {
    check_temporal(
        "CAST(tsz AS TIMETZ)",
        "EXTRACT(SECOND FROM it)",
        |t: &RawTimeTz| format!("{:.6}", t.time.microseconds() as f64 / 1e6),
    );
}

#[test]
fn test_timestamptz_year() {
    check_temporal("tsz", "EXTRACT(YEAR FROM it)", |tsz: &RawTimestampTz| {
        tsz.date.year.to_string()
    });
}

#[test]
fn test_timestamptz_month() {
    check_temporal("tsz", "EXTRACT(MONTH FROM it)", |tsz: &RawTimestampTz| {
        tsz.date.month.to_string()
    });
}

#[test]
fn test_timestamptz_day() {
    check_temporal("tsz", "EXTRACT(DAY FROM it)", |tsz: &RawTimestampTz| {
        tsz.date.day.to_string()
    });
}

#[test]
fn test_timestamptz_hour() {
    check_temporal("tsz", "EXTRACT(HOUR FROM it)", |tsz: &RawTimestampTz| {
        tsz.time.hours.to_string()
    });
}

#[test]
fn test_timestamptz_minute() {
    check_temporal("tsz", "EXTRACT(MINUTE FROM it)", |tsz: &RawTimestampTz| {
        tsz.time.minutes.to_string()
    });
}

#[test]
fn test_timestamptz_second() {
    check_temporal("tsz", "EXTRACT(SECOND FROM it)", |tsz: &RawTimestampTz| {
        format!("{:.6}", tsz.time.microseconds() as f64 / 1e6)
    });
}

#[test]
fn test_rawtz() -> AResult<()> {
    let ctx = get_server();
    let parms: Parameters = ctx.parms();
    let conn = Connection::new(parms)?;
    let mut cursor = conn.cursor();

    for offset_hours in [0i32, 2, -6] {
        let sign = if offset_hours >= 0 { '+' } else { '-' };
        let abs = offset_hours.abs();
        let seconds_east = offset_hours * 3600;

        cursor.execute(&format!(
            "SET TIME ZONE INTERVAL '{sign}{abs:02}:00' HOUR TO MINUTE"
        ))?;
        cursor.execute("SELECT MAX(tsz), CAST(MAX(tsz) AS TIMETZ) as tz FROM temporal")?;
        assert!(cursor.next_row()?);

        let tsz: RawTimestampTz = cursor.get(0)?.expect("tsz should not be null");
        assert_eq!(tsz.tz.seconds_east, seconds_east);

        let tz: RawTimeTz = cursor.get(1)?.expect("tsz should not be null");
        assert_eq!(tz.tz.seconds_east, seconds_east);
    }
    Ok(())
}
