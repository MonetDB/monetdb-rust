// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

//! Representation of MonetDB's type system.
//!
//! In particular, the SQL type system, not the MAL/GDK type system.

use std::fmt;

/// Type alias for the precision (number of digits) of DECIMAL types.
pub type Precision = u8;

/// Type alias for the scale (number of digits after the decimal point) of
/// DECIMAL types.
pub type Scale = u8;

/// Type alias for the width of for example CHAR/VARCHAR types.
pub type Width = u32;

/// Denotes the various types table- or result set column can have in MonetDB.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MonetType {
    /// The BOOLEAN type: false and true.
    Bool,
    /// 8 bit signed integer between -127 and 127.
    TinyInt,
    /// 16 bit signed integer between -32767 and 32767.
    SmallInt,
    /// 32 bit signed integer between -2147483647 and 2147483647
    Int,
    /// 64 bit signed integer between -9223372036854775807 and 9223372036854775807.
    BigInt,
    /// 128 bit signed integer between -2^127 +1 and +2^127 -1 (±170141183460469231731687303715884105727).
    /// Not all servers support this.
    HugeInt,
    /// 64 bits unsigned object (row) identifier, for internal use
    Oid,
    /// Exact signed decimal number with specified Precision and Scale.
    /// Precision is between 1 and 18 if the server does not support HUGEINT, and between 1 and 38 if it does.
    /// Scale is between 0 and Precision.
    Decimal(Precision, Scale),
    /// CHAR or VARCHAR column with the given maximum width. Width 0 means 'unspecified'.
    Varchar(Width),
    /// 32 bit signed floating point number
    Real,
    /// 64 bit signed floating point number
    Double,
    /// 32 bit signed number of months.
    MonthInterval,
    /// 64 bit signed number of days.
    DayInterval,
    /// 64 bit signed number of milliseconds.
    SecInterval,
    /// 24-hour time of day HH:MM:SS.sss with varying number of decimals,
    /// independent of time zone.
    /// (Nr of decimals currently unimplemented.)
    Time,
    /// 24-hour time of day HH:MM:SS.sss with varying number of decimals,
    /// expressed in the connections current timezone.
    /// (Nr of decimals currently unimplemented.)
    TimeTz,
    /// Gregorian calendar date YYYY-MM-DD
    Date,
    /// Timestamp YYYY-MM-DD HH:MM:SS.sss with varying number of decimals,
    /// expressed in the connections current timezone.
    /// (Nr of decimals currently unimplemented.)
    Timestamp,
    /// Timestamp YYYY-MM-DD HH:MM:SS.sss with varying number of decimals,
    /// independent of time zone.
    /// (Nr of decimals currently unimplemented.)
    TimestampTz,
    Blob,
    /// A URL.
    Url,
    Inet,
    /// Valid string representation of a JSON object.
    Json,
    /// A UUID.
    Uuid,
}

impl fmt::Display for MonetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use MonetType::*;
        match self {
            Bool => f.write_str("BOOL"),
            TinyInt => f.write_str("TINYINT"),
            SmallInt => f.write_str("SMALLINT"),
            Int => f.write_str("INT"),
            BigInt => f.write_str("BIGINT"),
            HugeInt => f.write_str("HUGEINT"),
            Oid => f.write_str("OID"),
            Decimal(p, s) => write!(f, "DECIMAL({p}, {s})"),
            Varchar(n) => write!(f, "VARCHAR({n})"),
            Real => f.write_str("REAL"),
            Double => f.write_str("DOUBLE"),
            MonthInterval => f.write_str("MONTH_INTERVAL"),
            DayInterval => f.write_str("DAY_INTERVAL"),
            SecInterval => f.write_str("SEC_INTERVAL"),
            Time => f.write_str("TIME"),
            TimeTz => f.write_str("TIMETZ"),
            Date => f.write_str("DATE"),
            Timestamp => f.write_str("TIMESTAMP"),
            TimestampTz => f.write_str("TIMESTAMPTZ"),
            Blob => f.write_str("BLOB"),
            Url => f.write_str("URL"),
            Inet => f.write_str("INET"),
            Json => f.write_str("JSON"),
            Uuid => f.write_str("UUID"),
        }
    }
}

impl MonetType {
    /// Used while parsing result sets. Based on the name
    /// create a MonetType instance with parameters
    /// set to a dummy value.
    pub(crate) fn prototype(code: &str) -> Option<Self> {
        use MonetType::*;
        let typ = match code {
            "boolean" => Bool,
            "tinyint" => TinyInt,
            "smallint" => SmallInt,
            "int" => Int,
            "bigint" => BigInt,
            "hugeint" => HugeInt,
            "oid" => Oid,
            "varchar" => Varchar(0),
            "decimal" => Decimal(0, 0),
            "real" => Real,
            "double" => Double,
            "month_interval" => MonthInterval,
            "day_interval" => DayInterval,
            "sec_interval" => SecInterval,
            "time" => Time,
            "timetz" => TimeTz,
            "date" => Date,
            "timestamp" => Timestamp,
            "timestamptz" => TimestampTz,
            "blob" => Blob,
            "url" => Url,
            "inet" => Inet,
            "json" => Json,
            "uuid" => Uuid,
            _ => return None,
        };
        Some(typ)
    }
}
