// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
#![allow(dead_code)]

use std::{error, iter, mem, str::FromStr};

use bstr::{BStr, BString, ByteSlice};
use memchr::memmem;

use crate::monettypes::MonetType;

use super::{rowset::RowSet, CursorError, CursorResult};

#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum BadReply {
    #[error("invalid utf-8 encoding in {0}")]
    Unicode(&'static str),
    #[error("unknown server response: {0}")]
    UnknownResponse(BString),
    #[error("expected separator {:?} not found", *.0 as char)]
    SepNotFound(u8),
    #[error("invalid reply header: {0}")]
    InvalidHeader(String),
    #[error("unexpected reply header: {0}")]
    UnexpectedHeader(BString),
    #[error("unexpected end of server response")]
    UnexpectedEnd,
    #[error("too many columns in result set: {0}")]
    TooManyColumns(u64),
    #[error("too few columns in result set: {0}")]
    TooFewColumns(usize),
    #[error("invalid backslash escape in result set")]
    InvalidBackslashEscape,
    #[error("column index {0} out of bounds, have only {1} columns")]
    ColumnIndexOutOfBounds(usize, usize),
}

pub type RResult<T> = Result<T, BadReply>;

#[derive(Debug)]
pub struct ReplyBuf {
    data: Vec<u8>,
    pos: usize,
}

impl ReplyBuf {
    pub fn new(vec: Vec<u8>) -> Self {
        ReplyBuf { data: vec, pos: 0 }
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }

    pub fn mut_vec(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    pub fn peek(&self) -> &[u8] {
        &self.data[self.pos..]
    }

    pub fn is_empty(&self) -> bool {
        self.peek().is_empty()
    }

    pub fn consume(&mut self, nbytes: usize) -> &mut [u8] {
        assert!(nbytes <= self.data.len() - self.pos);
        let newpos = self.pos + nbytes;
        let ret = &mut self.data[self.pos..newpos];
        self.pos = newpos;
        ret
    }

    pub fn find(&self, byte: u8) -> Option<usize> {
        memchr::memchr(byte, self.peek())
    }

    pub fn find2(&self, byte1: u8, byte2: u8) -> Option<(usize, u8)> {
        let haystack = self.peek();
        memchr::memchr2(byte1, byte2, haystack).map(|i| (i, haystack[i]))
    }

    pub fn find_line(&mut self, first: u8) -> Option<usize> {
        let haystack = self.peek();
        if haystack.is_empty() {
            None
        } else if haystack[0] == first {
            Some(0)
        } else {
            memmem::find(haystack, &[b'\n', first]).map(|idx| idx + 1)
        }
    }

    pub fn split(&mut self, sep: u8) -> RResult<&'_ mut [u8]> {
        let Some(end) = self.find(sep) else {
            if self.is_empty() {
                return Err(BadReply::UnexpectedEnd);
            } else {
                return Err(BadReply::SepNotFound(sep));
            }
        };
        let ret = self.consume(end + 1);
        Ok(&mut ret[..end])
    }

    pub fn split_str(&mut self, sep: u8, context: &'static str) -> RResult<&str> {
        let head = self.split(sep)?;
        from_utf8(context, head)
    }

    pub fn convert_backslashes(&mut self, skip: usize) -> RResult<&'_ mut [u8]> {
        let start_offset = self.pos + skip;
        let start = self.data.as_mut_ptr().wrapping_add(start_offset);
        let end = self.data.as_mut_ptr().wrapping_add(self.data.len());
        assert!(start <= end);

        let mut wr = start;
        let mut rd = start as *const u8;

        // wr <= rd <= end
        loop {
            if rd == end {
                return Err(BadReply::UnexpectedEnd); // end quote missing
            }
            // Here, wr <= rd < end

            let b = unsafe { rd.read() };
            rd = rd.wrapping_add(1);
            // Now, wr < rd <= end

            let unescaped = if b == b'\\' {
                // avail is nr of bytes available AFTER the backslash
                let avail = unsafe { end.offset_from(rd) };
                if avail < 1 {
                    return Err(BadReply::InvalidBackslashEscape);
                }
                let chr = unsafe { rd.read() };
                rd = rd.wrapping_add(1);
                match chr {
                    b't' => b'\t',
                    b'n' => b'\n',
                    b'f' => 0x0C,
                    b'r' => b'\r',
                    b'\\' => b'\\',
                    b'"' => b'"',
                    b'0'..=b'3' => {
                        // octal escape
                        let e1 = chr.wrapping_sub(b'0');
                        if avail < 3 {
                            return Err(BadReply::UnexpectedEnd);
                        }
                        let e2 = unsafe { rd.read().wrapping_sub(b'0') };
                        rd = rd.wrapping_add(1);
                        let e3 = unsafe { rd.read().wrapping_sub(b'0') };
                        rd = rd.wrapping_add(1);
                        if ((e2 | e3) & 0b1111_1000) != 0 {
                            return Err(BadReply::InvalidBackslashEscape);
                        }
                        (e1 << 6) | (e2 << 3) | e3
                    }
                    _ => return Err(BadReply::InvalidBackslashEscape),
                }
            } else if b == b'"' {
                break;
            } else {
                // nothing to unescape
                b
            };
            // rd may have moved but still, wr < rd <= end

            unsafe { wr.write(unescaped) };
            wr = wr.wrapping_add(1);
            // wr <= rd <= end
        }

        let rd_offset = unsafe { rd.offset_from(self.data.as_mut_ptr()) as usize };
        let wr_offset = unsafe { wr.offset_from(self.data.as_mut_ptr()) as usize };

        let old_pos = self.pos;
        self.pos = rd_offset;
        Ok(&mut self.data[old_pos..wr_offset])
    }
}

#[test]
fn test_convert_backslashes() {
    #[track_caller]
    fn f(s: &str, skip: usize, expected: RResult<&str>) {
        let Some(opening_quote_idx) = s.find('"') else {
            panic!("test data must have opening quote");
        };
        let mut buf = ReplyBuf::new(s.into());
        buf.consume(opening_quote_idx);
        assert!(buf.peek().starts_with(b"\""), "{}", BStr::new(buf.peek()));
        buf.consume(1);
        // now we have the buf where we want it, right after the opening quote.

        let actual: Result<&BStr, BadReply> = buf.convert_backslashes(skip).map(|x| BStr::new(x));
        let expected: Result<&BStr, BadReply> = expected.map(|t| t.into());

        assert_eq!(actual, expected);
    }

    f(r#"foo"banana""#, 0, Ok("banana"));
    f(r#"foo"banana"#, 0, Err(BadReply::UnexpectedEnd));

    f(r#"foo"""#, 0, Ok(""));
    f(r#"foo""#, 0, Err(BadReply::UnexpectedEnd));

    f(r#"foo"bana\na""#, 0, Ok("bana\na"));
    f(r#"foo"bana\ta""#, 0, Ok("bana\ta"));
    f(r#"foo"bana\fa""#, 0, Ok("bana\x0Ca"));
    f(r#"foo"bana\ra""#, 0, Ok("bana\ra"));
    f(r#"foo"bana\"a""#, 0, Ok("bana\"a"));
    f(r#"foo"bana\xa""#, 0, Err(BadReply::InvalidBackslashEscape));

    f(r#"foo"bana\Na""#, 0, Err(BadReply::InvalidBackslashEscape));
    f(r#"foo"bana\Ta""#, 0, Err(BadReply::InvalidBackslashEscape));
    f(r#"foo"bana\Fa""#, 0, Err(BadReply::InvalidBackslashEscape));
    f(r#"foo"bana\Ra""#, 0, Err(BadReply::InvalidBackslashEscape));

    f(r#"foo"bana\R4""#, 0, Err(BadReply::InvalidBackslashEscape));
    f(r#"foo"bana\R5""#, 0, Err(BadReply::InvalidBackslashEscape));
    f(r#"foo"bana\R6""#, 0, Err(BadReply::InvalidBackslashEscape));
    f(r#"foo"bana\R7""#, 0, Err(BadReply::InvalidBackslashEscape));

    f(r#"foo"bana\r""#, 0, Ok("bana\r"));
    f(r#"foo"\tbanana""#, 0, Ok("\tbanana"));

    // LATIN SMALL LETTER A == a == oct utf-8 \141
    f(r#"foo"b\141nana""#, 0, Ok("banana"));
    // LATIN SMALL LETTER A WITH DIAERESIS == \u{e4} == oct utf-8 \303\244
    f(r#"foo"b\303\244nana""#, 0, Ok("b\u{e4}nana"));

    // Test the skip. 4 skips the bana but it's still included in the result
    f(r#"foo"bana\na""#, 4, Ok("bana\na"));
}

#[derive(Debug)]
pub enum ReplyParser {
    Exhausted(Vec<u8>),
    Error(ReplyBuf),
    Success {
        buf: ReplyBuf,
        affected: Option<i64>,
    },
    Data(ResultSet),
    Tx {
        buf: ReplyBuf,
        auto_commit: bool,
    },
}

#[derive(Debug)]
pub struct ResultSet {
    pub result_id: u64,
    pub next_row: u64,
    pub total_rows: u64,
    pub columns: Vec<ResultColumn>,
    pub row_set: RowSet,
    pub stashed: Option<RowSet>,
    pub to_close: Option<u64>,
}

impl Default for ReplyParser {
    fn default() -> Self {
        ReplyParser::Exhausted(vec![])
    }
}

impl ReplyParser {
    pub fn new(mut vec: Vec<u8>) -> RResult<Self> {
        let min_cap = 8192;
        if vec.capacity() < min_cap {
            vec.reserve(min_cap - vec.capacity());
        }
        let buf = ReplyBuf::new(vec);
        Self::parse(buf)
    }

    pub fn take_buffer(&mut self) -> Vec<u8> {
        if let ReplyParser::Exhausted(vec) = self {
            mem::take(vec)
        } else {
            panic!("cannot call ReplyParser::take_buffer() when parser is not exhausted");
        }
    }

    pub fn affected_rows(&self) -> Option<i64> {
        match self {
            ReplyParser::Success { affected, .. } => *affected,
            ReplyParser::Data(ResultSet {
                total_rows: nrows, ..
            }) => Some(*nrows as i64),
            _ => None,
        }
    }

    pub fn at_result_set(&self) -> bool {
        matches!(self, ReplyParser::Data { .. })
    }

    pub fn into_next_reply(self) -> RResult<(ReplyParser, Option<u64>)> {
        let mut return_to_close = None;
        use ReplyParser::*;
        let buf = match self {
            Exhausted(vec) => ReplyBuf::new(vec),
            Error(buf) | Success { buf, .. } | Tx { buf, .. } => buf,
            Data(
                ResultSet {
                    stashed: Some(row_set),
                    to_close,
                    ..
                }
                | ResultSet {
                    stashed: None,
                    row_set,
                    to_close,
                    ..
                },
            ) => {
                return_to_close = to_close;
                row_set.finish()
            }
        };

        ReplyParser::parse(buf).map(|parser| (parser, return_to_close))
    }

    pub fn detect_errors(response: &[u8]) -> CursorResult<()> {
        let start = if response.is_empty() {
            return Ok(());
        } else if response[0] == b'!' {
            1
        } else if let Some(pos) = memmem::find(response, b"\n!") {
            pos + 1
        } else {
            return Ok(());
        };

        let mut bytes = &response[start..];
        if let Some(idx) = bytes.find_byte(b'\n') {
            bytes = &bytes[..idx];
        }
        let message = std::str::from_utf8(bytes)
            .unwrap_or("server sent an error message but it can't be decoded");

        Err(CursorError::Server(message.to_string()))
    }

    fn parse(buf: ReplyBuf) -> RResult<ReplyParser> {
        let ahead = buf.peek();
        match ahead {
            [] => {
                let mut vec = buf.into_vec();
                vec.clear();
                Ok(ReplyParser::Exhausted(vec))
            }
            [b'&', b'1', ..] => Self::parse_data(buf),
            [b'&', b'2', ..] => Self::parse_successful_update(buf),
            [b'&', b'3', ..] => Self::parse_successful_other(buf),
            [b'&', b'4', ..] => Self::parse_autocommit_status(buf),
            [b'!', ..] => Self::parse_error(buf),
            _ => {
                let line = ahead.as_bstr().lines().next().unwrap();
                Err(BadReply::UnknownResponse(line.into()))
            }
        }
    }

    fn parse_successful_update(mut buf: ReplyBuf) -> RResult<ReplyParser> {
        let mut fields = [0]; // don't care about the other fields yet
        Self::parse_header(&mut buf, &mut fields)?;
        Ok(ReplyParser::Success {
            buf,
            affected: Some(fields[0]),
        })
    }

    fn parse_successful_other(mut buf: ReplyBuf) -> RResult<ReplyParser> {
        let mut fields: [i64; 0] = [];
        Self::parse_header(&mut buf, &mut fields)?;
        Ok(ReplyParser::Success {
            buf,
            affected: None,
        })
    }

    pub(crate) fn parse_header<T: FromStr>(buf: &mut ReplyBuf, dest: &mut [T]) -> RResult<()> {
        let line = buf.split_str(b'\n', "header line")?.trim_ascii();
        let mut parts = line[3..].split(' ');
        for (i, d) in dest.iter_mut().enumerate() {
            let Some(p) = parts.next() else {
                return Err(BadReply::InvalidHeader(format!(
                    "not enough header items, expected {n}: {line}",
                    n = dest.len()
                )));
            };
            let Ok(value) = p.parse() else {
                return Err(BadReply::InvalidHeader(format!(
                    "cannot parse header item {i}: {line}"
                )));
            };
            *d = value;
        }
        Ok(())
    }

    fn parse_autocommit_status(mut buf: ReplyBuf) -> RResult<ReplyParser> {
        let line = buf.split_str(b'\n', "header line")?.trim_ascii();
        let auto_commit = if line.starts_with("&4 f") {
            false
        } else if line.starts_with("&4 t") {
            true
        } else {
            return Err(BadReply::InvalidHeader(format!(
                "invalid autocommit header: {line}"
            )));
        };
        Ok(ReplyParser::Tx { buf, auto_commit })
    }

    fn parse_error(mut buf: ReplyBuf) -> RResult<ReplyParser> {
        // for now, .execute() has already returned the error, no reason to hold on to it
        let _line = buf.split_str(b'\n', "error header")?.trim_ascii();
        Ok(ReplyParser::Error(buf))
    }

    fn parse_data(mut buf: ReplyBuf) -> RResult<ReplyParser> {
        let mut fields = [0; 4];
        Self::parse_header(&mut buf, &mut fields)?;
        let [result_id, rows_total, ncols, rows_included] = fields;
        if ncols > usize::MAX as u64 {
            return Err(BadReply::TooManyColumns(ncols));
        }
        let ncols = ncols as usize;
        let to_close = (rows_included < rows_total).then_some(result_id);

        let mut columns: Vec<ResultColumn> =
            iter::repeat(ResultColumn::empty()).take(ncols).collect();

        // parse the table_name header
        Self::parse_data_header(&mut buf, "table_name", &mut columns, &|col, s| {
            col.name.push_str(s);
            Ok(())
        })?;

        // parse the name header
        Self::parse_data_header(&mut buf, "name", &mut columns, &|col, s| {
            col.name.push('.');
            col.name.push_str(s);
            Ok(())
        })?;

        // parse the type header
        Self::parse_data_header(&mut buf, "type", &mut columns, &|col, s| {
            let Some(typ) = MonetType::prototype(s) else {
                return Err(format!("unknown column type: {s}").into());
            };
            col.typ = typ;
            Ok(())
        })?;

        // parse the length header
        Self::parse_data_header(&mut buf, "length", &mut columns, &|col, s| {
            if let MonetType::Varchar(n) = &mut col.typ {
                *n = u32::from_str(s)?
            };
            Ok(())
        })?;

        // parse the typesizes header
        Self::parse_data_header(&mut buf, "typesizes", &mut columns, &|col, s| {
            if let MonetType::Decimal(precision, scale) = &mut col.typ {
                let Some((pr, sc)) = s.split_once(' ') else {
                    return Err("expect typesizes to be PRECISION <space> SCALE".into());
                };
                *precision = pr.parse()?;
                *scale = sc.parse()?;
            };
            Ok(())
        })?;

        let row_set = RowSet::new(buf, columns.len());
        Ok(ReplyParser::Data(ResultSet {
            result_id,
            next_row: 0,
            total_rows: rows_total,
            columns,
            row_set,
            to_close,
            stashed: None,
        }))
    }

    fn parse_data_header<'a>(
        buf: &'a mut ReplyBuf,
        expected_kind: &str,
        columns: &'a mut [ResultColumn],
        f: ResultColumnUpdater<'_, 'a>,
    ) -> RResult<()> {
        let line: &[u8] = buf.split(b'\n')?;
        let line = from_utf8("data header line", line)?;
        let Some(line) = line.strip_prefix("% ") else {
            return Err(BadReply::UnexpectedHeader(line.into()));
        };
        let Some((body, kind)) = line.split_once(" # ") else {
            return Err(BadReply::InvalidHeader(
                "expected '# ' in data header".into(),
            ));
        };
        if kind != expected_kind {
            return Err(BadReply::InvalidHeader(format!(
                "expected '{expected_kind}' header, found {}",
                BStr::new(kind)
            )));
        }

        let mut columns = columns.iter_mut();
        for (i, part) in body.split(",\t").enumerate() {
            let Some(col) = columns.next() else {
                return Err(BadReply::InvalidHeader(
                    "too many columns in data header".into(),
                ));
            };
            if let Err(e) = f(col, part) {
                return Err(BadReply::InvalidHeader(format!("col {i}: {e}")));
            }
        }
        if columns.next().is_some() {
            return Err(BadReply::InvalidHeader(
                "too few columns in data header".into(),
            ));
        }
        Ok(())
    }
}

/// Holds information about a column of a result set.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ResultColumn {
    name: String,
    typ: MonetType,
}

impl ResultColumn {
    fn empty() -> Self {
        ResultColumn {
            name: "".into(),
            typ: MonetType::Bool,
        }
    }

    /// Return the name of the column.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the type of the column.
    pub fn sql_type(&self) -> &MonetType {
        &self.typ
    }
}

type ResultColumnUpdater<'x, 'a> =
    &'x dyn Fn(&'a mut ResultColumn, &'a str) -> Result<(), Box<dyn error::Error>>;

pub fn from_utf8<'a>(context: &'static str, bytes: &'a [u8]) -> RResult<&'a str> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(_) => Err(BadReply::Unicode(context)),
    }
}
