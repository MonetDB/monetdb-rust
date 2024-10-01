// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MonetKind {
    /// A common integer, that is, up to i64.
    Integer,
    /// Huge integers are sometimes treated differently
    HugeInteger,
    /// Decimals have precision and scale and need to be extracted differently
    Decimal,
    /// UTF-8 encoded text without NUL bytes
    Text,
}

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
        }
    }
}

impl MonetType {
    pub fn kind(&self) -> MonetKind {
        use MonetType::*;
        match self {
            Bool | TinyInt | SmallInt | Int | BigInt | Oid => MonetKind::Integer,
            HugeInt => MonetKind::HugeInteger,
            Decimal(_, _) => MonetKind::Decimal,
            Varchar(_) => MonetKind::Text,
        }
    }

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
            _ => return None,
        };
        Some(typ)
    }
}
