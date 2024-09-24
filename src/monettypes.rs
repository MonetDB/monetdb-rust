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
