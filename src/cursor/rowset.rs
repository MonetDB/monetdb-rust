use crate::cursor::replies::BadReply;

use super::replies::{from_utf8, RResult, ReplyBuf};

#[derive(Debug)]
pub struct RowSet {
    buf: ReplyBuf,
    ncols: usize,
    fields: Vec<Option<(*const u8, usize)>>,
}

// [ 1,→"one"→]↵
// [ 42,→"forty-two"→]↵
// [ -1,→"a\\\"b"→]↵

impl RowSet {
    pub fn new(buf: ReplyBuf, ncols: usize) -> Self {
        let fields = vec![None; ncols];
        RowSet { buf, ncols, fields }
    }

    pub fn advance(&mut self) -> RResult<bool> {
        let ret = self.do_advance();
        if ret.is_err() {
            self.fields.clear();
        }
        ret
    }

    fn do_advance(&mut self) -> RResult<bool> {
        if !self.buf.peek().starts_with(b"[") {
            self.fields.fill(None);
            return Ok(false);
        }
        self.buf.consume(2);
        for (i, field) in self.fields.iter_mut().enumerate() {
            let comma_skip = (i + 1 < self.ncols) as usize;
            let Some(first) = self.buf.peek().first() else {
                return Err(BadReply::UnexpectedEnd);
            };
            match first {
                b']' => {
                    return Err(BadReply::TooFewColumns(i));
                }
                b'"' => {
                    // skip it
                    self.buf.consume(1);
                    let Some((pos, char)) = self.buf.find2(b'"', b'\\') else {
                        return Err(BadReply::UnexpectedEnd);
                    };
                    if char == b'"' {
                        // no backslashes
                        *field = Some((self.buf.peek().as_ptr(), pos));
                        // skip the data, the quote, possibly the comma and the tab
                        self.buf.consume(pos + 1 + comma_skip + 1);
                    } else {
                        let unescaped = self.buf.convert_backslashes(pos)?;
                        *field = Some((unescaped.as_ptr(), unescaped.len()));
                        // buf has already skipped the quote, skip comma and tab
                        self.buf.consume(comma_skip + 1);
                    }
                }
                _ => {
                    let rough: &[u8] = self.buf.split(b'\t')?;
                    let adjusted = &rough[..rough.len() - comma_skip];
                    *field = if adjusted == b"NULL" {
                        None
                    } else {
                        Some((adjusted.as_ptr(), adjusted.len()))
                    };
                }
            }
        }

        // now we should be looking at the trailing ]
        if !self.buf.peek().starts_with(b"]\n") {
            return Err(BadReply::SepNotFound(b']'));
        }
        self.buf.consume(2);
        Ok(true)
    }

    pub fn finish(mut self) -> ReplyBuf {
        if let Some(idx) = self.buf.find_line(b'&') {
            self.buf.consume(idx);
        } else {
            self.buf.consume(self.buf.peek().len());
        }
        self.buf
    }

    pub fn get_field_raw(&self, idx: usize) -> Option<&[u8]> {
        // index out of bounds -> None
        let field = *self.fields.get(idx)?;
        // NULL -> None
        let field = field?;
        let slice = unsafe { std::slice::from_raw_parts(field.0, field.1) };
        Some(slice)
    }

    pub fn get_field_str(&self, idx: usize) -> RResult<Option<&str>> {
        let Some(bytes) = self.get_field_raw(idx) else {
            return Ok(None);
        };
        let str = from_utf8("result set field", bytes)?;
        Ok(Some(str))
    }
}

#[test]
fn test_rowset_int_and_null() {
    let testdata = "[ 11,\tNULL,\t33\t]\n";
    let mut rs = RowSet::new(ReplyBuf::new(testdata.into()), 3);

    assert_eq!(rs.get_field_str(0), Ok(None));
    assert_eq!(rs.get_field_str(1), Ok(None));
    assert_eq!(rs.get_field_str(2), Ok(None));
    assert_eq!(rs.get_field_str(3), Ok(None));

    let have_row = rs.advance().unwrap();
    assert!(have_row);

    assert_eq!(rs.get_field_str(0), Ok(Some("11")));
    assert_eq!(rs.get_field_str(1), Ok(None)); // was NULL
    assert_eq!(rs.get_field_str(2), Ok(Some("33")));
    assert_eq!(rs.get_field_str(3), Ok(None));

    let have_row = rs.advance().unwrap();
    assert!(!have_row);
}

#[test]
fn test_rowset_strings() {
    let testdata = "[ \"\",\t\"MonetDB\",\t\"NULL\"\t]\n";
    let mut rs = RowSet::new(ReplyBuf::new(testdata.into()), 3);

    let have_row = rs.advance().unwrap();
    assert!(have_row);

    assert_eq!(rs.get_field_str(0), Ok(Some("")));
    assert_eq!(rs.get_field_str(1), Ok(Some("MonetDB")));
    assert_eq!(rs.get_field_str(2), Ok(Some("NULL")));
    assert_eq!(rs.get_field_str(3), Ok(None));

    let have_row = rs.advance().unwrap();
    assert!(!have_row);
}

#[test]
fn test_rowset_escaped_strings() {
    use std::fmt::Write;

    fn escape(s: &str) -> String {
        let mut answer = String::new();
        answer.push('"');
        for &b in s.as_bytes() {
            match b {
                b'\t' => write!(answer, "\\t").unwrap(),
                b'\n' => write!(answer, "\\n").unwrap(),
                b'\r' => write!(answer, "\\r").unwrap(),
                b'\\' => write!(answer, "\\\\").unwrap(),
                b'"' => write!(answer, "\\\"").unwrap(),
                ..=31 | 127.. => write!(answer, "\\{b:03o}").unwrap(),
                _ => answer.push(b as char),
            }
        }
        answer.push('"');
        answer
    }

    let expected = [
        ["", "FOO", "TAB\tTAB"],
        ["CR\rLF\n", "FF\u{C}", "BACK\\SLASH"],
        ["DOUBLE\"QUOTE", "B\u{c4}NANA", "SMILEY\u{263A}SMILEY"],
    ];

    let mut testdata = String::new();
    for row in expected {
        write!(testdata, "[ ").unwrap();
        for (i, field) in row.iter().enumerate() {
            testdata.push_str(&escape(field));
            if i + 1 < row.len() {
                testdata.push(',');
            }
            testdata.push('\t');
        }
        testdata.push_str("]\n");
    }

    let mut rs = RowSet::new(ReplyBuf::new(testdata.into()), 3);

    for (row_nr, expected_row) in expected.iter().enumerate() {
        let advance = rs.advance();
        assert_eq!(advance, Ok(true), "advancing to row {row_nr}");
        for (col_nr, &expected_field) in expected_row.iter().enumerate() {
            let field = rs.get_field_str(col_nr);
            assert_eq!(field, Ok(Some(expected_field)), "row {row_nr} col {col_nr}");
        }
    }
    assert!(!rs.advance().unwrap());
}

#[test]
fn test_single_column() {
    // multiple types in one column shouldn't happen but we're
    // not going to notice that here
    let testdata = "[ 1\t]\n[ NULL\t]\n[ \"foo\\\"bar\"\t]\n";
    let mut rs = RowSet::new(ReplyBuf::new(testdata.into()), 1);

    assert_eq!(rs.advance(), Ok(true));
    assert_eq!(rs.get_field_str(0), Ok(Some("1")));

    assert_eq!(rs.advance(), Ok(true));
    assert_eq!(rs.get_field_str(0), Ok(None));

    assert_eq!(rs.advance(), Ok(true));
    assert_eq!(rs.get_field_str(0), Ok(Some(r#"    foo"bar     "#.trim())));

    assert_eq!(rs.advance(), Ok(false));
}

#[test]
fn test_finish() {
    use bstr::BStr;
    let testdata = "[ 1,\t2\t]\n[ 3,\t4\t]\n[ 5,\t6\t]\n&lalala\n";

    // .finish() works after we've consumed three rows
    let mut rs = RowSet::new(ReplyBuf::new(testdata.into()), 2);
    assert_eq!(rs.advance(), Ok(true));
    assert_eq!(rs.get_field_str(0), Ok(Some("1")));
    assert_eq!(rs.get_field_str(1), Ok(Some("2")));
    assert_eq!(rs.advance(), Ok(true));
    assert_eq!(rs.get_field_str(0), Ok(Some("3")));
    assert_eq!(rs.get_field_str(1), Ok(Some("4")));
    assert_eq!(rs.advance(), Ok(true));
    assert_eq!(rs.get_field_str(0), Ok(Some("5")));
    assert_eq!(rs.get_field_str(1), Ok(Some("6")));
    let buf = rs.finish();
    assert_eq!(BStr::new(buf.peek()), BStr::new("&lalala\n"));

    // .finish() works after we've consumed only two rows
    let mut rs = RowSet::new(ReplyBuf::new(testdata.into()), 2);
    assert_eq!(rs.advance(), Ok(true));
    assert_eq!(rs.advance(), Ok(true));
    let buf = rs.finish();
    assert_eq!(BStr::new(buf.peek()), BStr::new("&lalala\n"));

    // .finish() works after we've consumed only one rows
    let mut rs = RowSet::new(ReplyBuf::new(testdata.into()), 2);
    assert_eq!(rs.advance(), Ok(true));
    let buf = rs.finish();
    assert_eq!(BStr::new(buf.peek()), BStr::new("&lalala\n"));

    // .finish() works after we've consumed no rows at all
    let rs = RowSet::new(ReplyBuf::new(testdata.into()), 2);
    let buf = rs.finish();
    assert_eq!(BStr::new(buf.peek()), BStr::new("&lalala\n"));
}
