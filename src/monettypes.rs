// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use std::fmt;

pub type Precision = u8;

pub type Scale = u8;

pub type Width = u32;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MonetType {
    Bool,
    TinyInt,
    SmallInt,
    Int,
    BigInt,
    HugeInt,
    Oid,
    Decimal(Precision, Scale),
    Varchar(Width),
    Real,
    Double,
    MonthInterval,
    DayInterval,
    SecInterval,
    Time,
    TimeTz,
    Date,
    Timestamp,
    TimestampTz,
    // Blob,
    Url,
    Inet,
    Json,
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
            Url => f.write_str("URL"),
            Inet => f.write_str("INET"),
            Json => f.write_str("JSON"),
            Uuid => f.write_str("UUID"),
        }
    }
}

impl MonetType {

    pub fn prototype(code: &str) -> Option<Self> {
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
            "url" => Url,
            "inet" => Inet,
            "json" => Json,
            "uuid" => Uuid,
            _ => return None,
        };
        Some(typ)
    }
}
