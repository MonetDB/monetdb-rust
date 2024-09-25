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
            Decimal(p, s) => write!(f, "DECIMAL({p}, {s})"),
            Varchar(n) => write!(f, "VARCHAR({n})"),
        }
    }
}

impl MonetType {
    pub fn kind(&self) -> MonetKind {
        use MonetType::*;
        match self {
            Bool | TinyInt | SmallInt | Int | BigInt => MonetKind::Integer,
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
            "varchar" => Varchar(0),
            "decimal" => Decimal(0, 0),
            _ => return None,
        };
        Some(typ)
    }
}
