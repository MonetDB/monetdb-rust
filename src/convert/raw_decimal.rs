// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

use std::{any::type_name, ops::Sub, str::FromStr};

use num::{CheckedAdd, CheckedMul};

/// Representation of a Decimal value from Monet.
/// `RawDecimal(n, s)` ist to be interpreted as `n * 10^(-s)`.
#[derive(Debug, Clone, Copy)]
pub struct RawDecimal<T>(pub T, pub u8);

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidDecimal {
    #[error("value doesn't fit {}", type_name::<Self>())]
    OutOfRange,
    #[error("unexpected character in decimal: {0:?}")]
    UnexpectedCharacter(char),
}

impl<T> RawDecimal<T> {
    fn parse_signed(digits: &[u8]) -> Result<RawDecimal<T>, InvalidDecimal>
    where
        T: CheckedAdd + CheckedMul + Sub<Output = T> + TryFrom<u8>,
    {
        if let Some(digits) = digits.strip_prefix(b"-") {
            let RawDecimal(value, scale) = Self::parse_unsigned(digits)?;
            Ok(RawDecimal(Self::small_constant(0) - value, scale))
        } else {
            Self::parse_unsigned(digits)
        }
    }

    fn parse_unsigned(digits: &[u8]) -> Result<RawDecimal<T>, InvalidDecimal>
    where
        T: CheckedAdd + CheckedMul + TryFrom<u8>,
    {
        let mut scale = 0;
        let mut saw_period = false;
        let mut acc = Self::small_constant(0);

        for &d in digits {
            match d {
                b'0'..=b'9' => {
                    if let Some(new) = Self::multiply_accumulate(acc, d - b'0') {
                        acc = new;
                        scale += 1;
                    } else {
                        return Err(InvalidDecimal::OutOfRange);
                    }
                }
                b'.' => {
                    scale = 0;
                    saw_period = true;
                }
                _ => return Err(InvalidDecimal::UnexpectedCharacter(d as char)),
            }
        }

        if !saw_period {
            scale = 0;
        }
        Ok(RawDecimal(acc, scale))
    }

    fn multiply_accumulate(acc: T, digit: u8) -> Option<T>
    where
        T: CheckedAdd + CheckedMul + TryFrom<u8>,
    {
        acc.checked_mul(&Self::small_constant(10u8))?
            .checked_add(&Self::small_constant(digit))
    }

    fn small_constant(num: u8) -> T
    where
        T: TryFrom<u8>,
    {
        if let Ok(n) = T::try_from(num) {
            n
        } else {
            panic!("invalid small constant {num}");
        }
    }
}

macro_rules! raw_decimal {
    ($type:ty, $parser:ident) => {
        impl FromStr for RawDecimal<$type> {
            type Err = InvalidDecimal;
            fn from_str(s: &str) -> Result<Self, InvalidDecimal> {
                Self::$parser(s.as_bytes())
            }
        }

        impl RawDecimal<$type> {
            pub fn at_scale(&self, s: u8) -> Option<$type> {
                if s < self.1 {
                    // fractional part not completely cleared
                    return None;
                }
                let sc = <$type>::scale10(s - self.1);
                self.0.checked_mul(sc)
            }
        }

        impl PartialEq for RawDecimal<$type> {
            fn eq(&self, other: &Self) -> bool {
                let highest = self.1.max(other.1);
                let Some(left) = self.at_scale(highest) else {
                    return false;
                };
                let Some(right) = other.at_scale(highest) else {
                    return false;
                };
                left == right
            }
        }

        impl Eq for RawDecimal<$type> {}
    };
}

raw_decimal!(i8, parse_signed);
raw_decimal!(u8, parse_unsigned);
raw_decimal!(i16, parse_signed);
raw_decimal!(u16, parse_unsigned);
raw_decimal!(i32, parse_signed);
raw_decimal!(u32, parse_unsigned);
raw_decimal!(i64, parse_signed);
raw_decimal!(u64, parse_unsigned);
raw_decimal!(i128, parse_signed);
raw_decimal!(u128, parse_unsigned);

#[test]
fn test_fromstr() {
    let expected = Ok(RawDecimal(-123i32, 2));
    let actual = "-1.23".parse::<RawDecimal<i32>>();
    assert_eq!(actual, expected);
}

#[test]
fn test_fromstr_no_period() {
    let expected = Ok(RawDecimal(-123, 0));
    let actual = "-123".parse::<RawDecimal<i32>>();
    assert_eq!(actual, expected);
}

#[test]
fn test_at_scale() {
    assert_eq!(RawDecimal(123i32, 2).at_scale(0), None);
    assert_eq!(RawDecimal(123i32, 2).at_scale(1), None);
    assert_eq!(RawDecimal(123i32, 2).at_scale(2), Some(123));
    assert_eq!(RawDecimal(123i32, 2).at_scale(3), Some(1230));
    assert_eq!(RawDecimal(123i32, 2).at_scale(4), Some(12300));
}

#[test]
fn test_eq() {
    assert_eq!(RawDecimal(10, 0), RawDecimal(10, 0));
    assert_eq!(RawDecimal(100, 1), RawDecimal(100, 1));
    assert_eq!(RawDecimal(10, 0), RawDecimal(100, 1));
}

pub trait Scale10
where
    Self: Clone + Copy,
{
    const SCALE10: [Self; 256];

    fn scale10(s: u8) -> Self {
        let table = &Self::SCALE10;
        table[s as usize]
    }
}

impl Scale10 for i8 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table
    };
}

impl Scale10 for u8 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table
    };
}

impl Scale10 for i16 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table[3] = 1000;
        table[4] = 10000;
        table
    };
}

impl Scale10 for u16 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table[3] = 1000;
        table[4] = 10000;
        table
    };
}

impl Scale10 for i32 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table[3] = 1000;
        table[4] = 10000;
        table[5] = 100000;
        table[6] = 1000000;
        table[7] = 10000000;
        table[8] = 100000000;
        table[9] = 1000000000;
        table
    };
}

impl Scale10 for u32 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table[3] = 1000;
        table[4] = 10000;
        table[5] = 100000;
        table[6] = 1000000;
        table[7] = 10000000;
        table[8] = 100000000;
        table[9] = 1000000000;
        table
    };
}

impl Scale10 for i64 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table[3] = 1000;
        table[4] = 10000;
        table[5] = 100000;
        table[6] = 1000000;
        table[7] = 10000000;
        table[8] = 100000000;
        table[9] = 1000000000;
        table[10] = 10000000000;
        table[11] = 100000000000;
        table[12] = 1000000000000;
        table[13] = 10000000000000;
        table[14] = 100000000000000;
        table[15] = 1000000000000000;
        table[16] = 10000000000000000;
        table[17] = 100000000000000000;
        table[18] = 1000000000000000000;
        table
    };
}

impl Scale10 for u64 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table[3] = 1000;
        table[4] = 10000;
        table[5] = 100000;
        table[6] = 1000000;
        table[7] = 10000000;
        table[8] = 100000000;
        table[9] = 1000000000;
        table[10] = 10000000000;
        table[11] = 100000000000;
        table[12] = 1000000000000;
        table[13] = 10000000000000;
        table[14] = 100000000000000;
        table[15] = 1000000000000000;
        table[16] = 10000000000000000;
        table[17] = 100000000000000000;
        table[18] = 1000000000000000000;
        table[19] = 10000000000000000000;
        table
    };
}

impl Scale10 for i128 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table[3] = 1000;
        table[4] = 10000;
        table[5] = 100000;
        table[6] = 1000000;
        table[7] = 10000000;
        table[8] = 100000000;
        table[9] = 1000000000;
        table[10] = 10000000000;
        table[11] = 100000000000;
        table[12] = 1000000000000;
        table[13] = 10000000000000;
        table[14] = 100000000000000;
        table[15] = 1000000000000000;
        table[16] = 10000000000000000;
        table[17] = 100000000000000000;
        table[18] = 1000000000000000000;
        table[19] = 10000000000000000000;
        table[20] = 100000000000000000000;
        table[21] = 1000000000000000000000;
        table[22] = 10000000000000000000000;
        table[23] = 100000000000000000000000;
        table[24] = 1000000000000000000000000;
        table[25] = 10000000000000000000000000;
        table[26] = 100000000000000000000000000;
        table[27] = 1000000000000000000000000000;
        table[28] = 10000000000000000000000000000;
        table[29] = 100000000000000000000000000000;
        table[30] = 1000000000000000000000000000000;
        table[31] = 10000000000000000000000000000000;
        table[32] = 100000000000000000000000000000000;
        table[33] = 1000000000000000000000000000000000;
        table[34] = 10000000000000000000000000000000000;
        table[35] = 100000000000000000000000000000000000;
        table[36] = 1000000000000000000000000000000000000;
        table[37] = 10000000000000000000000000000000000000;
        table[38] = 100000000000000000000000000000000000000;
        table
    };
}

impl Scale10 for u128 {
    const SCALE10: [Self; 256] = {
        let mut table = [Self::MAX; 256];
        table[0] = 1;
        table[1] = 10;
        table[2] = 100;
        table[3] = 1000;
        table[4] = 10000;
        table[5] = 100000;
        table[6] = 1000000;
        table[7] = 10000000;
        table[8] = 100000000;
        table[9] = 1000000000;
        table[10] = 10000000000;
        table[11] = 100000000000;
        table[12] = 1000000000000;
        table[13] = 10000000000000;
        table[14] = 100000000000000;
        table[15] = 1000000000000000;
        table[16] = 10000000000000000;
        table[17] = 100000000000000000;
        table[18] = 1000000000000000000;
        table[19] = 10000000000000000000;
        table[20] = 100000000000000000000;
        table[21] = 1000000000000000000000;
        table[22] = 10000000000000000000000;
        table[23] = 100000000000000000000000;
        table[24] = 1000000000000000000000000;
        table[25] = 10000000000000000000000000;
        table[26] = 100000000000000000000000000;
        table[27] = 1000000000000000000000000000;
        table[28] = 10000000000000000000000000000;
        table[29] = 100000000000000000000000000000;
        table[30] = 1000000000000000000000000000000;
        table[31] = 10000000000000000000000000000000;
        table[32] = 100000000000000000000000000000000;
        table[33] = 1000000000000000000000000000000000;
        table[34] = 10000000000000000000000000000000000;
        table[35] = 100000000000000000000000000000000000;
        table[36] = 1000000000000000000000000000000000000;
        table[37] = 10000000000000000000000000000000000000;
        table[38] = 100000000000000000000000000000000000000;
        table
    };
}
