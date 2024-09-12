use std::{borrow::Borrow, ops::Range};

use super::BLOCKSIZE;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Header([u8; 2]);

impl Header {
    pub fn new(size: usize, last: bool) -> Self {
        assert!(size <= BLOCKSIZE);
        let n = 2 * size as u16 + last as u16;
        let bytes = u16::to_le_bytes(n);
        Header(bytes)
    }

    pub fn from_bytes(bytes: [u8; 2]) -> Self {
        let header = Header(bytes);
        assert!(header.size() <= BLOCKSIZE);
        header
    }

    pub fn from_slice(slice: &[u8]) -> Self {
        let bytes = slice.try_into().unwrap();
        Self::from_bytes(bytes)
    }

    pub fn size(&self) -> usize {
        let n = u16::from_le_bytes(self.0);
        n as usize / 2
    }

    pub fn is_last(&self) -> bool {
        (self.0[0] & 1) > 0
    }

    pub fn as_bytes(&self) -> &[u8; 2] {
        &self.0
    }
}

impl Borrow<[u8]> for Header {
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum BlockState {
    #[default]
    Start,
    PartialHeader(u8),
    Body {
        remaining: usize,
        last: bool,
    },
    End,
}

impl BlockState {
    pub fn new(remaining: usize, last: bool) -> Self {
        match (remaining, last) {
            (0, false) => BlockState::Start,
            (0, true) => BlockState::End,
            (1.., _) => BlockState::Body { remaining, last },
        }
    }

    fn from_header(header: Header) -> Self {
        Self::new(header.size(), header.is_last())
    }

    pub fn skip_headers(&self, data: &[u8]) -> (Range<usize>, BlockState) {
        use BlockState::*;

        let end = data.len();
        let mut pos = 0;
        let mut st = *self;

        while pos < end {
            let avail = end - pos;
            match st {
                Body { remaining, last } if remaining > avail => {
                    // body extends beyond available data, return smaller Body
                    return (pos..pos + avail, Self::new(remaining - avail, last));
                }

                Body { remaining, last } => {
                    // body ends somewhere in the buffer, new block starts there
                    assert_ne!(remaining, 0);
                    return (pos..pos + remaining, Self::new(0, last));
                }

                Start if avail >= 2 => {
                    let header = Header::from_slice(&data[pos..pos + 2]);
                    st = Self::from_header(header);
                    pos += 2;
                }

                Start => {
                    assert_eq!(avail, 1);
                    assert_eq!(pos, data.len() - 1);
                    let lo = data[pos];
                    return (end..end, PartialHeader(lo));
                }

                PartialHeader(lo) => {
                    assert_ne!(avail, 0);
                    let header = Header::from_bytes([lo, data[pos]]);
                    pos += 1;
                    st = Self::from_header(header);
                }

                End => {
                    panic!("cannot continue in End state");
                }
            }
        }

        (end..end, st)
    }

    pub fn interpret(&mut self, data: impl AsRef<[u8]>) -> Range<usize> {
        let (range, new) = self.skip_headers(data.as_ref());
        *self = new;
        range
    }
}

#[cfg(test)]
mod tests {
    use crate::util::referencedata::ReferenceData;

    use super::*;
    use BlockState::*;

    #[test]
    fn test_interpret1() {
        let mut bs = BlockState::default();
        assert_eq!(bs, Start);

        bs.interpret(b"");
        assert_eq!(bs, Start);

        bs.interpret([0, 0]);
        assert_eq!(bs, Start);

        bs.interpret([1, 0]);
        assert_eq!(bs, End);
    }

    fn head(remaining: usize, last: bool) -> [u8; 2] {
        *Header::new(remaining, last).as_bytes()
    }

    fn step<'a>(bs: &mut BlockState, data: &mut &'a [u8]) -> &'a [u8] {
        let range = bs.interpret(*data);
        let new_start = range.end;
        let extracted = &data[range];
        *data = &data[new_start..];
        extracted
    }

    #[test]
    fn test_interpret2() {
        let mut orig = ReferenceData::default();
        orig.data(head(0, false));
        orig.data(Header::new(0, false));
        orig.mark_data("name_header", Header::new(5, true));
        orig.mark_data("name_body", "joeri".as_bytes());

        let bs = &mut BlockState::default();
        let mut data = orig.as_slice();
        assert_eq!(step(bs, &mut data), b"joeri");
        assert_eq!(data, b"");
        assert_eq!(*bs, End);

        let bs = &mut BlockState::default();
        let n = orig.lookup("name_body") + 3;
        let mut data = &orig.as_slice()[..n];
        assert_eq!(step(bs, &mut data), b"joe");
        assert_eq!(data, b"");
        data = &orig.as_slice()[n..];
        assert_eq!(step(bs, &mut data), b"ri");
        assert_eq!(data, b"");
        assert_eq!(*bs, End);

        let bs = &mut BlockState::default();
        let n = orig.lookup("name_header") + 1;
        let mut data = &orig.as_slice()[..n];
        assert_eq!(step(bs, &mut data), b"");
        data = &orig.as_slice()[n..];
        assert_eq!(step(bs, &mut data), b"joeri");
    }
}
