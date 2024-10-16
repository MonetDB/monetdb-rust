// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

use std::ops::Sub;

use atoi::FromRadix10Checked;
use bstr::BStr;
use num::Zero;

use crate::{cursor::replies::ResultSet, CursorResult};

use super::{conversion_error, raw_decimal::RawDecimal, FromMonet};

/// Representation of a DATE value from MonetDB
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RawDate {
    pub day: u8,
    pub month: u8,
    pub year: i16,
}

impl RawDate {
    fn parse(field: &mut &[u8]) -> CursorResult<RawDate> {
        let year = take_signed::<i16, RawDate>(field)?;
        take_sep::<RawDate>(field, b"-")?;
        let month = take_unsigned::<u8, RawDate>(field)?;
        take_sep::<RawDate>(field, b"-")?;
        let day = take_unsigned::<u8, RawDate>(field)?;
        let date = RawDate { day, month, year };
        Ok(date)
    }
}

#[test]
fn test_parse_date() {
    let mut s: &[u8] = b"2014-02-14xyz";
    assert_eq!(
        RawDate::parse(&mut s),
        Ok(RawDate {
            day: 14,
            month: 2,
            year: 2014
        })
    );
    assert_eq!(s, b"xyz");

    s = b"123-4-5";
    assert_eq!(
        RawDate::parse(&mut s),
        Ok(RawDate {
            day: 5,
            month: 4,
            year: 123
        })
    );
    s = b"-123-4-5";
    assert_eq!(
        RawDate::parse(&mut s),
        Ok(RawDate {
            day: 5,
            month: 4,
            year: -123
        })
    );
}

impl FromMonet for RawDate {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(mut field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        let date = RawDate::parse(&mut field)?;
        expect_end::<Self>(field)?;
        Ok(Some(date))
    }
}

/// Representation of a TIME value from MonetDB.
/// Also used in [`RawTimeTz`], [`RawTimestamp`] and [`RawTimestampTz`].
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RawTime {
    pub microseconds: u32,
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
}

impl RawTime {
    fn parse(field: &mut &[u8]) -> CursorResult<RawTime> {
        let hours = take_unsigned::<u8, RawTime>(field)?;
        take_sep::<RawTime>(field, b":")?;
        let minutes = take_unsigned::<u8, RawTime>(field)?;
        take_sep::<RawTime>(field, b":")?;
        let seconds = take_unsigned::<u8, RawTime>(field)?;
        let fractional_part: RawDecimal<u32> = if field.starts_with(b".") {
            match RawDecimal::parse_unsigned(field) {
                Ok((dec, rest)) => {
                    *field = rest;
                    dec
                }
                Err(e) => {
                    return Err(conversion_error::<Self>(format!(
                        "server sent invalid TIME: {}",
                        e
                    )))
                }
            }
        } else {
            RawDecimal(0, 6)
        };

        let Some(microseconds) = fractional_part.at_scale(6) else {
            return Err(conversion_error::<Self>(
                "server sent too many decimal digits",
            ));
        };

        let time = RawTime {
            microseconds,
            seconds,
            minutes,
            hours,
        };
        Ok(time)
    }

    pub fn microseconds(&self) -> u32 {
        self.microseconds + 1_000_000 * self.seconds as u32
    }
}

#[test]
fn test_parse_time() {
    let mut s: &[u8] = b"12:34:56xyz";
    assert_eq!(
        RawTime::parse(&mut s),
        Ok(RawTime {
            microseconds: 0,
            seconds: 56,
            minutes: 34,
            hours: 12,
        })
    );
    assert_eq!(s, b"xyz");

    s = b"01:02:03";
    assert_eq!(
        RawTime::parse(&mut s),
        Ok(RawTime {
            microseconds: 0,
            seconds: 3,
            minutes: 2,
            hours: 1,
        })
    );

    s = b"1:2:3";
    assert_eq!(
        RawTime::parse(&mut s),
        Ok(RawTime {
            microseconds: 0,
            seconds: 3,
            minutes: 2,
            hours: 1,
        })
    );

    s = b"12:34:56.789";
    assert_eq!(
        RawTime::parse(&mut s),
        Ok(RawTime {
            microseconds: 789000,
            seconds: 56,
            minutes: 34,
            hours: 12,
        })
    );

    s = b"12:34:56.123456";
    assert_eq!(
        RawTime::parse(&mut s),
        Ok(RawTime {
            microseconds: 123456,
            seconds: 56,
            minutes: 34,
            hours: 12,
        })
    );

    // too many digits
    s = b"12:34:56.1234567";
    claims::assert_err!(RawTime::parse(&mut s));
}

impl FromMonet for RawTime {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(mut field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        let time = RawTime::parse(&mut field)?;
        expect_end::<Self>(field)?;
        Ok(Some(time))
    }
}

/// Representation of a TIMESTAMP value from MonetDB.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RawTimestamp {
    pub date: RawDate,
    pub time: RawTime,
}

impl RawTimestamp {
    fn parse(field: &mut &[u8]) -> CursorResult<RawTimestamp> {
        let date = RawDate::parse(field)?;
        take_sep::<Self>(field, b" ")?;
        let time = RawTime::parse(field)?;
        Ok(RawTimestamp { date, time })
    }
}

#[test]
fn test_parse_timestamp() {
    let mut s: &[u8] = b"2024-10-16 10:32:59.12xyz";
    assert_eq!(
        RawTimestamp::parse(&mut s),
        Ok(RawTimestamp {
            date: RawDate {
                day: 16,
                month: 10,
                year: 2024,
            },
            time: RawTime {
                microseconds: 120_000,
                seconds: 59,
                minutes: 32,
                hours: 10,
            }
        })
    );
    assert_eq!(s, b"xyz");
}

impl FromMonet for RawTimestamp {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(mut field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        let timestamp = RawTimestamp::parse(&mut field)?;
        expect_end::<Self>(field)?;
        Ok(Some(timestamp))
    }
}

/// Representation of the UTC offset of a time zone as included in MonetDB's
/// TIME WITH TIMEZONE (TIMETZ) and TIMESTAMP WITH TIMEZONE (TIMESTAMPTZ).
/// Contains the offset in seconds east of UTC.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RawTz {
    pub seconds_east: i32,
}

impl RawTz {
    fn parse(field: &mut &[u8]) -> CursorResult<RawTz> {
        let (sign, ref mut hr_ms) = match field {
            [b'+', rest @ ..] => (1, rest),
            [b'-', rest @ ..] => (-1, rest),
            [] => return Err(conversion_error::<Self>("missing timezone")),
            _ => {
                return Err(conversion_error::<Self>(format!(
                    "invalid timezone: {:?}",
                    BStr::new(field)
                )))
            }
        };
        let hours = take_unsigned::<u8, Self>(hr_ms)?;
        take_sep::<Self>(hr_ms, b":")?;
        let mins: u8 = take_unsigned::<u8, Self>(hr_ms)?;
        let seconds_east = sign * (3600 * hours as i32 + 60 * mins as i32);
        *field = *hr_ms;
        Ok(RawTz { seconds_east })
    }
}

#[test]
fn test_parse_tz() {
    let mut s: &[u8] = b"+00:00xyz";
    assert_eq!(RawTz::parse(&mut s), Ok(RawTz { seconds_east: 0 }));
    assert_eq!(s, b"xyz");
    s = b"-00:00";
    assert_eq!(RawTz::parse(&mut s), Ok(RawTz { seconds_east: 0 }));
    s = b"+01:00";
    assert_eq!(RawTz::parse(&mut s), Ok(RawTz { seconds_east: 3600 }));
    s = b"+07:30";
    assert_eq!(
        RawTz::parse(&mut s),
        Ok(RawTz {
            seconds_east: 7 * 3600 + 30 * 60
        })
    );
    s = b"-07:30";
    assert_eq!(
        RawTz::parse(&mut s),
        Ok(RawTz {
            seconds_east: -(7 * 3600 + 30 * 60)
        })
    );

    s = b"*00:00";
    claims::assert_err!(RawTz::parse(&mut s));
    s = b"00:00";
    claims::assert_err!(RawTz::parse(&mut s));
    s = b"+00";
    claims::assert_err!(RawTz::parse(&mut s));
}

/// Representation of a TIME WITH TIMEZONE (TIMETZ) value from MonetDB
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RawTimeTz {
    pub time: RawTime,
    pub tz: RawTz,
}

impl RawTimeTz {
    fn parse(field: &mut &[u8]) -> CursorResult<RawTimeTz> {
        let time = RawTime::parse(field)?;
        let tz = RawTz::parse(field)?;
        Ok(RawTimeTz { time, tz })
    }
}

#[test]
fn test_parse_timetz() {
    let mut s: &[u8] = b"10:32:59.12+00:00xyz";
    assert_eq!(
        RawTimeTz::parse(&mut s),
        Ok(RawTimeTz {
            time: RawTime {
                microseconds: 120_000,
                seconds: 59,
                minutes: 32,
                hours: 10,
            },
            tz: RawTz { seconds_east: 0 }
        })
    );
    assert_eq!(s, b"xyz");

    s = b"10:32:59.12-05:00";
    assert_eq!(
        RawTimeTz::parse(&mut s),
        Ok(RawTimeTz {
            time: RawTime {
                microseconds: 120_000,
                seconds: 59,
                minutes: 32,
                hours: 10,
            },
            tz: RawTz {
                seconds_east: -5 * 3600
            }
        })
    );
}

impl FromMonet for RawTimeTz {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(mut field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        let timetz = RawTimeTz::parse(&mut field)?;
        expect_end::<Self>(field)?;
        Ok(Some(timetz))
    }
}

/// Representation of a TIMESTAMP WITH TIMEZONE (TIMESTAMPTZ) value from MonetDB
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RawTimestampTz {
    pub date: RawDate,
    pub time: RawTime,
    pub tz: RawTz,
}

impl RawTimestampTz {
    fn parse(field: &mut &[u8]) -> CursorResult<RawTimestampTz> {
        let RawTimestamp { date, time } = RawTimestamp::parse(field)?;
        let tz = RawTz::parse(field)?;
        let timestamptz = RawTimestampTz { date, time, tz };
        Ok(timestamptz)
    }
}

#[test]
fn test_parse_timestamptz() {
    let mut s: &[u8] = b"2024-10-16 10:32:59.12+00:00xyz";
    assert_eq!(
        RawTimestampTz::parse(&mut s),
        Ok(RawTimestampTz {
            date: RawDate {
                day: 16,
                month: 10,
                year: 2024,
            },
            time: RawTime {
                microseconds: 120_000,
                seconds: 59,
                minutes: 32,
                hours: 10,
            },
            tz: RawTz { seconds_east: 0 }
        })
    );
    assert_eq!(s, b"xyz");

    s = b"2024-10-16 10:32:59.12-05:00";
    assert_eq!(
        RawTimestampTz::parse(&mut s),
        Ok(RawTimestampTz {
            date: RawDate {
                day: 16,
                month: 10,
                year: 2024,
            },
            time: RawTime {
                microseconds: 120_000,
                seconds: 59,
                minutes: 32,
                hours: 10,
            },
            tz: RawTz {
                seconds_east: -5 * 3600
            }
        })
    );
}

impl FromMonet for RawTimestampTz {
    fn extract(rs: &ResultSet, colnr: usize) -> CursorResult<Option<Self>> {
        let Some(mut field) = rs.row_set.get_field_raw(colnr) else {
            return Ok(None);
        };
        let timestamptz = RawTimestampTz::parse(&mut field)?;
        expect_end::<Self>(field)?;
        Ok(Some(timestamptz))
    }
}

fn take_unsigned<V, T>(data: &mut &[u8]) -> CursorResult<V>
where
    T: 'static,
    V: FromRadix10Checked,
{
    match V::from_radix_10_checked(data) {
        (Some(v), n @ 1..) => {
            *data = &data[n..];
            Ok(v)
        }
        _ => Err(conversion_error::<T>(format_args!(
            "invalid integer {:?}",
            BStr::new(data)
        ))),
    }
}

#[test]
fn test_take_unsigned() {
    let s = &mut b"123".as_slice();
    assert_eq!(take_unsigned::<i16, i16>(s), Ok(123i16));
    assert_eq!(*s, b"");

    let s = &mut b"-123".as_slice();
    claims::assert_err!(take_unsigned::<i16, i16>(s));

    let s = &mut b"".as_slice();
    claims::assert_err!(take_unsigned::<i16, i16>(s));
}

fn take_signed<V, T>(data: &mut &[u8]) -> CursorResult<V>
where
    T: 'static,
    V: FromRadix10Checked,
    V: Sub<Output = V> + Zero,
{
    if let Some(mut digits) = data.strip_prefix(b"-") {
        let value = take_unsigned::<V, T>(&mut digits)?;
        *data = digits;
        let negated = V::zero() - value;
        Ok(negated)
    } else {
        take_unsigned::<V, T>(data)
    }
}

#[test]
fn test_take_signed() {
    let s = &mut b"123".as_slice();
    assert_eq!(take_signed::<i16, i16>(s), Ok(123i16));
    assert_eq!(*s, b"");

    let s = &mut b"-123".as_slice();
    assert_eq!(take_signed::<i16, i16>(s), Ok(-123i16));
    assert_eq!(*s, b"");

    let s = &mut b"".as_slice();
    claims::assert_err!(take_signed::<i16, i16>(s));
}

fn take_sep<T: 'static>(data: &mut &[u8], delimiter: &[u8]) -> CursorResult<()> {
    if let Some(rest) = data.strip_prefix(delimiter) {
        *data = rest;
        Ok(())
    } else {
        Err(conversion_error::<T>(format!(
            "expected delimiter {:?}",
            BStr::new(delimiter)
        )))
    }
}

fn expect_end<T: 'static>(data: &[u8]) -> CursorResult<()> {
    if data.is_empty() {
        Ok(())
    } else {
        Err(conversion_error::<T>(format!(
            "unexpected data: {:?}",
            BStr::new(data)
        )))
    }
}
