use std::io::{self, IoSlice};

use crate::framing::blockstate::Header;

use super::BLOCKSIZE;

pub struct MapiWriteStream<W> {
    inner: W,
}

impl<W: io::Write> MapiWriteStream<W> {
    pub fn new(inner: W) -> Self {
        MapiWriteStream { inner }
    }

    fn write_block(&mut self, data: &[u8], last: bool) -> io::Result<()> {
        assert!(data.len() <= BLOCKSIZE);
        let header = Header::new(data.len(), last);
        let mut ioslices = [IoSlice::new(header.as_bytes()), IoSlice::new(data)];
        write_all_vectored(&mut self.inner, &mut ioslices)
    }

    pub fn write_data(&mut self, data: &[u8], last: bool) -> io::Result<()> {
        if data.len() <= BLOCKSIZE {
            return self.write_block(data, last);
        }
        let mut last_flag = false;
        for chunk in data.chunks(BLOCKSIZE) {
            last_flag = last && chunk.len() < BLOCKSIZE; // < BLOCKSIZE can only happen in the last chunk
            self.write_block(chunk, last_flag)?;
        }
        if last && !last_flag {
            // make sure flag is sent when len is a multiple of BLOCKSIZE, in
            // particular 0.
            self.write_block(&[], true)?;
        }
        Ok(())
    }

    pub fn inner(&self) -> &W {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.write_block(&[], true)?;
        self.inner.flush()?;
        Ok(self.inner)
    }
}

impl<W: io::Write> io::Write for MapiWriteStream<W> {
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_data(buf, false).map(|()| buf.len())
    }
}

// copy pasted from the standard library where it's still unstable
fn write_all_vectored(mut wr: impl io::Write, mut bufs: &mut [io::IoSlice<'_>]) -> io::Result<()> {
    // Guarantee that bufs is empty if it contains no data,
    // to avoid calling write_vectored if there is no data to be written.
    io::IoSlice::advance_slices(&mut bufs, 0);
    while !bufs.is_empty() {
        match wr.write_vectored(bufs) {
            Ok(0) => {
                return Err(io::ErrorKind::WriteZero.into());
            }
            Ok(n) => io::IoSlice::advance_slices(&mut bufs, n),
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use io::{Cursor, Write};

    use super::*;
    use crate::{framing::blockstate::BlockState, util::referencedata::ReferenceData};
    use std::iter;

    #[test]
    fn test_write() {
        let aaa: Vec<u8> = iter::repeat(b'A').take(9000).collect();
        let mut refd = ReferenceData::new();
        refd.mark("block1");
        refd.data(Header::new(7, true));
        refd.data(b"monetdb".as_slice());
        refd.mark("data1");
        refd.data(Header::new(8190, false));
        refd.data(&aaa[..8190]);
        refd.mark("data2");
        refd.data(Header::new(9000 - 8190, true));
        refd.data(&aaa[8190..]);

        let mut verifier = refd.verifier();
        let mut wr = MapiWriteStream::new(&mut verifier);
        wr.write_block(b"monetdb", true).unwrap();
        wr.write_data(&aaa, true).unwrap();

        verifier.assert_end();
    }

    #[test]
    fn test_into_inner_ends_message() {
        fn state(bytes: &[u8]) -> BlockState {
            let mut bs = BlockState::default();
            let mut todo = bytes;
            while !todo.is_empty() {
                let range = bs.interpret(todo);
                todo = &todo[range.end..];
            }
            bs
        }

        let buffer: Vec<u8> = vec![];
        let cursor = Cursor::new(buffer);
        let mut wr = MapiWriteStream::new(cursor);

        assert_eq!(state(wr.inner().get_ref()), BlockState::Start);

        assert_eq!(wr.write(b"monetdb").unwrap(), 7);
        assert_eq!(state(wr.inner().get_ref()), BlockState::Start);

        let cursor = wr.finish().unwrap();
        assert_eq!(state(cursor.get_ref()), BlockState::End);
    }
}
