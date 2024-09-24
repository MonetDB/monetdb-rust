#![allow(dead_code)]

use std::{error, iter, mem, str::FromStr};

use bstr::{BStr, BString, ByteSlice};
use memchr::memmem;

use crate::monettypes::MonetType;

#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum BadReply {
    #[error("invalid utf-8 encoding in {0}")]
    Unicode(&'static str),
    #[error("unknown server response: {0}")]
    UnknownResponse(BString),
    #[error("expected separator {0:?} not found")]
    SepNotFound(u8),
    #[error("invalid reply header: {0}")]
    InvalidHeader(String),
    #[error("unexpected reply header: {0}")]
    UnexpectedHeader(BString),
    #[error("unexpected end of server response")]
    UnexpectedEnd,
    #[error("too many columns in result set: {0}")]
    TooManyColumns(u64),
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
}

#[derive(Debug)]
pub enum ReplyParser {
    Exhausted(Vec<u8>),
    Error(ReplyBuf),
    Success {
        buf: ReplyBuf,
        affected: Option<i64>,
    },
    Data {
        buf: ReplyBuf,
        result_id: u64,
        cur_row: u64,
        nrows: u64,
        columns: Vec<ResultColumn>,
        reply_size: u64,
        byte_size: usize,
    },
    Tx {
        buf: ReplyBuf,
        auto_commit: bool,
    },
}

impl Default for ReplyParser {
    fn default() -> Self {
        ReplyParser::Exhausted(Vec::with_capacity(8192))
    }
}

impl ReplyParser {
    pub fn new(vec: Vec<u8>) -> RResult<Self> {
        let buf = ReplyBuf::new(vec);
        Self::parse(buf)
    }

    pub fn take_buffer(&mut self) -> Vec<u8> {
        let vec = match self {
            ReplyParser::Exhausted(vec) => mem::take(vec),
            ReplyParser::Error(buf) => mem::take(&mut buf.data),
            ReplyParser::Success { buf, .. } => mem::take(&mut buf.data),
            ReplyParser::Data { buf, .. } => mem::take(&mut buf.data),
            ReplyParser::Tx { buf, .. } => mem::take(&mut buf.data),
        };
        *self = ReplyParser::Exhausted(vec![]);
        vec
    }

    pub fn affected_rows(&self) -> Option<i64> {
        match self {
            ReplyParser::Success { affected, .. } => *affected,
            ReplyParser::Data { nrows, .. } => Some(*nrows as i64),
            _ => None,
        }
    }

    pub fn at_result_set(&self) -> bool {
        matches!(self, ReplyParser::Data { .. })
    }

    pub fn remaining_rows(&self) -> RResult<Option<&str>> {
        if let ReplyParser::Data { buf, byte_size, .. } = self {
            let bytes = &buf.peek()[..*byte_size];
            let s = from_utf8("temporary_get_rows", bytes)?;
            Ok(Some(s))
        } else {
            Ok(None)
        }
    }

    pub fn into_next_reply(self) -> RResult<ReplyParser> {
        match self {
            ReplyParser::Exhausted(vec) => Self::parse(ReplyBuf::new(vec)),
            ReplyParser::Error(buf) => Self::parse(buf),
            ReplyParser::Success { buf, .. } | ReplyParser::Tx { buf, .. } => Self::parse(buf),
            ReplyParser::Data {
                mut buf, byte_size, ..
            } => {
                buf.consume(byte_size);
                Self::parse(buf)
            }
        }
    }

    pub fn detect_errors(response: &[u8]) -> Option<&str> {
        let start = if response.is_empty() {
            return None;
        } else if response[0] == b'!' {
            1
        } else if let Some(pos) = memmem::find(response, b"\n!") {
            pos + 1
        } else {
            return None;
        };

        let mut bytes = &response[start..];
        if let Some(idx) = bytes.find_byte(b'\n') {
            bytes = &bytes[..idx];
        }
        let message = std::str::from_utf8(bytes)
            .unwrap_or("server sent an error message but it can't be decoded");

        Some(message)
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

    fn parse_header<T: FromStr>(buf: &mut ReplyBuf, dest: &mut [T]) -> RResult<()> {
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
        let [result_id, nrows, ncols, reply_size] = fields;
        if ncols > usize::MAX as u64 {
            return Err(BadReply::TooManyColumns(ncols));
        }
        let ncols = ncols as usize;
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

        let bytes_to_end = if let Some((pos, _)) = buf.find2(b'&', b'!') {
            pos
        } else {
            buf.peek().len()
        };
        Ok(ReplyParser::Data {
            buf,
            result_id,
            cur_row: 0,
            nrows,
            columns,
            reply_size,
            byte_size: bytes_to_end,
        })
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
}

type ResultColumnUpdater<'x, 'a> =
    &'x dyn Fn(&'a mut ResultColumn, &'a str) -> Result<(), Box<dyn error::Error>>;

pub fn from_utf8<'a>(context: &'static str, bytes: &'a [u8]) -> RResult<&'a str> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(_) => Err(BadReply::Unicode(context)),
    }
}
