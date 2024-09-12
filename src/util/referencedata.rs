#![allow(dead_code)]

use core::fmt;
use std::{borrow::Borrow, collections::HashMap, io};

use bstr::BStr;
use itertools::Itertools;

#[derive(Debug, Default)]
pub struct ReferenceData {
    data: Vec<u8>,
    positions: HashMap<String, usize>,
}

impl ReferenceData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pos(&self) -> usize {
        self.data.len()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn data(&mut self, data: impl Borrow<[u8]>) -> usize {
        let pos = self.pos();
        self.data.extend_from_slice(data.borrow());
        pos
    }

    pub fn set_mark(&mut self, name: &str, pos: usize) {
        if let Some(prev) = self.positions.insert(name.to_string(), pos) {
            panic!("cannot define '{name}'={pos}, already have '{name}'={prev}");
        }
    }

    #[allow(dead_code)]
    pub fn mark(&mut self, name: &str) -> usize {
        let pos = self.pos();
        self.set_mark(name, pos);
        pos
    }

    pub fn mark_data(&mut self, name: &str, data: impl Borrow<[u8]>) -> usize {
        let pos = self.data(data);
        self.set_mark(name, pos);
        pos
    }

    pub fn lookup(&self, name: &str) -> usize {
        if let Some(pos) = self.positions.get(name) {
            *pos
        } else {
            let positions = self.positions.keys().join(", ");
            panic!("unknown position '{name}'. known: {positions}")
        }
    }

    pub fn verifier(&self) -> Verifier {
        Verifier {
            expected: self.data.clone(),
            positions: self.positions.clone(),
            pos: 0,
        }
    }
}

// impl Deref for ReferenceData {
//     type Target = [u8];

//     fn deref(&self) -> &Self::Target {
//         &self.data
//     }
// }

impl io::Write for ReferenceData {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct Verifier {
    pub expected: Vec<u8>,
    pub pos: usize,
    positions: HashMap<String, usize>,
}

impl Verifier {
    pub fn verify(&mut self, data: &[u8]) -> Result<(), String> {
        let start_pos = self.pos;
        let expected = &self.expected[self.pos..];

        if let Some(difference) = expected.iter().zip(data).position(|(e, d)| e != d) {
            return Err(self.report(start_pos, data, difference));
        }

        if data.len() > expected.len() {
            return Err(self.report(start_pos, data, expected.len()));
        }

        self.pos += data.len();
        Ok(())
    }

    fn report(&self, start_pos: usize, data: &[u8], difference: usize) -> String {
        use fmt::Write;
        let data_len = data.len();
        let (name, delta) = self.describe(start_pos);
        let after_this = Bin::cut(&self.expected, start_pos, 5, 0);
        let show_start = Bin::cut(data, 0, 0, 5);
        let show_expected = Bin::cut(&self.expected, start_pos + difference, 0, 5);
        let show_data = Bin::cut(data, difference, 0, 5);
        let mut message = String::new();
        writeln!(message, "Write differs from reference data").unwrap();
        writeln!(message, "  writing {data_len} bytes starting at offset {start_pos} (position '{name}' + {delta}): {show_start}").unwrap();
        writeln!(message, "  right after {after_this}").unwrap();
        writeln!(message, "  difference at byte {difference}:").unwrap();
        writeln!(message, "    expected {show_expected}").unwrap();
        writeln!(message, "    found    {show_data}").unwrap();

        message
    }

    pub fn verify_end(&self) -> Result<(), String> {
        let all = self.expected.len();
        let unused = &self.expected[self.pos..];
        if unused.is_empty() {
            return Ok(());
        }

        let used = self.pos;
        let (name, delta) = self.describe(used);
        let unused_len = unused.len();
        let show_unused = Bin::cut(unused, 0, 0, 5);
        let msg = format!("only {used} ('{name}' + {delta}) of {all} bytes of reference data were used, {unused_len} remain: {show_unused}");
        Err(msg)
    }

    #[track_caller]
    pub fn assert_end(&self) {
        if let Err(msg) = self.verify_end() {
            panic!("{msg}")
        }
    }

    fn describe(&self, pos: usize) -> (&str, usize) {
        let mut base = "<start>";
        let mut dist = pos;
        for (n, &p) in &self.positions {
            if pos < p {
                continue;
            }
            let new_dist = pos - p;
            if new_dist < dist {
                base = n;
                dist = new_dist;
            }
        }
        (base, dist)
    }
}

impl io::Write for Verifier {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Err(msg) = self.verify(buf) {
            panic!("{msg}");
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Bin<'a>(&'a [u8]);

impl<'a> Bin<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Bin(bytes)
    }

    fn cut(bytes: &'a [u8], pos: usize, prefix: usize, len: usize) -> Self {
        #[allow(clippy::manual_saturating_arithmetic)]
        let start = pos.checked_sub(prefix).unwrap_or(0);
        let end = bytes.len().min(pos + len);
        Self::new(&bytes[start..end])
    }
}

impl fmt::Display for Bin<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str_like = BStr::new(self.0);
        let arr_like = self.0;
        write!(f, "{str_like:?} {arr_like:?}")
    }
}

fn aaabbbccc() -> ReferenceData {
    let mut refd = ReferenceData::new();
    refd.data(b"AAA".as_slice());
    refd.mark_data("b", b"BBB".as_slice());
    refd.mark_data("c", b"CCC".as_slice());
    refd
}

#[test]
fn test_verifier1() {
    let refd = aaabbbccc();
    let mut ver = refd.verifier();
    ver.verify(b"AAAB").unwrap();
    ver.verify(b"BBCCC").unwrap();
    ver.verify_end().unwrap();
}

const TEST_VERIFIER2_OUTPUT: &str = r##"
Write differs from reference data
  writing 5 bytes starting at offset 4 (position 'b' + 1): "BBXXX" [66, 66, 88, 88, 88]
  right after "AAAB" [65, 65, 65, 66]
  difference at byte 2:
    expected "CCC" [67, 67, 67]
    found    "XXX" [88, 88, 88]
"##;

#[test]
fn test_verifier2() {
    let expected_error = TEST_VERIFIER2_OUTPUT.trim_start().into();
    let refd = aaabbbccc();
    let mut ver = refd.verifier();
    assert_eq!(ver.verify(b"AAAB"), Ok(()));
    assert_eq!(ver.verify(b"BBXXX"), Err(expected_error));
}

const TEST_VERIFIER3_OUTPUT: &str = r##"
Write differs from reference data
  writing 8 bytes starting at offset 4 (position 'b' + 1): "BBCCC" [66, 66, 67, 67, 67]
  right after "AAAB" [65, 65, 65, 66]
  difference at byte 5:
    expected "" []
    found    "XYZ" [88, 89, 90]
"##;

#[test]
fn test_verifier3() {
    let expected_error = TEST_VERIFIER3_OUTPUT.trim_start().into();
    let refd = aaabbbccc();
    let mut ver = refd.verifier();
    assert_eq!(ver.verify(b"AAAB"), Ok(()));
    assert_eq!(ver.verify(b"BBCCCXYZ"), Err(expected_error));
}
