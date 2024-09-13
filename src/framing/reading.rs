#![allow(dead_code)]

use std::io::{self, ErrorKind, Read};

use super::{blockstate::BlockState, BLOCKSIZE};

pub struct MapiReadStream<R> {
    inner: R,
    state: BlockState,
}

impl<R: Read> MapiReadStream<R> {
    pub fn new(inner: R) -> Self {
        MapiReadStream {
            inner,
            state: BlockState::Start,
        }
    }

    fn do_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            match self.state {
                BlockState::Body { remaining, last } => {
                    return self.read_body(remaining, last, buf)
                }

                BlockState::Start => self.read_header(&mut [0, 0])?,

                BlockState::PartialHeader(_) => self.read_header(&mut [0])?,

                BlockState::End => return Ok(0),
            }
        }
    }

    fn read_header(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read_exact(buf)?;
        self.state.interpret(buf);
        Ok(())
    }

    fn read_body(&mut self, remaining: usize, last: bool, buf: &mut [u8]) -> io::Result<usize> {
        assert!(remaining > 0);

        let ideal_read = if last {
            // last block of the message, do not read beyond message end
            remaining
        } else {
            // try to read the next header as well
            remaining + 2
        };
        let n = ideal_read.min(buf.len());
        let nread = self.read_some(&mut buf[..n])?;
        let range = self.state.interpret(&buf[..nread]);
        assert_eq!(range.start, 0); // we were in state Body or we wouldn't have got here

        if range.end < nread {
            // we succeeded in reading (part of) the next header
            let tail = &buf[range.end..nread];
            let next_range = self.state.interpret(tail);
            assert!(next_range.is_empty());
            assert_eq!(next_range.end, tail.len());
        }

        Ok(range.end)
    }

    fn read_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let nread = self.inner.read(buf)?;
        if nread == 0 {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(nread)
    }

    pub fn finish(mut self) -> io::Result<R> {
        if !matches!(self.state, BlockState::End) {
            self.skip_to_end()?;
        }
        Ok(self.inner)
    }

    fn skip_to_end(&mut self) -> io::Result<()> {
        let mut buf = [0u8; BLOCKSIZE + 2];
        while !matches!(self.state, BlockState::End) {
            let _ = self.do_read(&mut buf)?;
        }
        Ok(())
    }

    pub fn read_max(&mut self, mut buffer: &mut [u8]) -> io::Result<usize> {
        let orig_len = buffer.len();
        while !buffer.is_empty() {
            let n = self.read(buffer)?;
            if n == 0 {
                break;
            }
            buffer = &mut buffer[n..];
        }
        Ok(orig_len - buffer.len())
    }
}

impl<R: Read> Read for MapiReadStream<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        MapiReadStream::do_read(self, buf)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use crate::{framing::blockstate::Header, util::referencedata::ReferenceData};

    use super::MapiReadStream;

    #[test]
    fn test_read() {
        // two concatenated messages, reading should stop exactly at the
        // boundary
        let mut refd = ReferenceData::new();

        let content1 = b"monet";
        refd.data(Header::new(content1.len(), false));
        refd.data(content1.as_slice());

        let content2 = b"db";
        refd.data(Header::new(content2.len(), true));
        // let pos1 = refd.pos();
        refd.data(content2.as_slice());
        // let pos2 = refd.pos();

        let content3 = b"yeah";
        refd.data(Header::new(content3.len(), true));
        refd.data(content3.as_slice());
        // let pos3 = refd.pos();

        let master_cursor = Cursor::new(Vec::from(refd.as_slice()));

        // try reading block by block

        let mut rd = MapiReadStream::new(master_cursor.clone());
        let mut buffer = [0u8; 10];

        assert_eq!(rd.do_read(&mut buffer).unwrap(), 5);
        assert_eq!(&buffer[..5], b"monet");

        assert_eq!(rd.do_read(&mut buffer).unwrap(), 2);
        assert_eq!(&buffer[..2], b"db");

        assert_eq!(rd.do_read(&mut buffer).unwrap(), 0);
        assert_eq!(rd.do_read(&mut buffer).unwrap(), 0);
        assert_eq!(rd.do_read(&mut buffer).unwrap(), 0);

        // start reading next message
        let cursor = rd.finish().unwrap();
        let mut rd = MapiReadStream::new(cursor);

        assert_eq!(rd.do_read(&mut buffer).unwrap(), 4);
        assert_eq!(&buffer[..4], b"yeah");
        assert_eq!(rd.do_read(&mut buffer).unwrap(), 0);

        // if we just read from the stream we don't notice the block boundaries.
        let mut rd = MapiReadStream::new(master_cursor.clone());
        let mut message = String::new();
        rd.read_to_string(&mut message).unwrap();
        assert_eq!(message, "monetdb");
    }
}
