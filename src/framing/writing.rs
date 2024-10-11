// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

#![allow(
    dead_code,
    unused_imports,
    unused_import_braces,
    unused_variables,
    unused_assignments
)]

use std::{fmt, io};

use super::{
    blockstate::{BlockState, Header},
    BLOCKSIZE,
};

pub struct MapiBuf {
    buffer: Vec<u8>,
    block_left: usize,
}

impl Default for MapiBuf {
    fn default() -> Self {
        Self::with_buf(Vec::with_capacity(2 * (BLOCKSIZE + 2)))
    }
}

impl MapiBuf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_buf(mut buffer: Vec<u8>) -> Self {
        buffer.clear();
        let mut me = MapiBuf {
            buffer,
            block_left: 0,
        };
        // obvious dummy header
        me.buffer.push(0xFF);
        me.buffer.push(0xFF);
        me.block_left = BLOCKSIZE;
        me
    }

    pub fn append(&mut self, data: impl AsRef<[u8]>) {
        let data = data.as_ref();
        if data.len() <= self.block_left {
            // happy path
            self.buffer.extend_from_slice(data);
            self.block_left -= data.len();
        } else {
            self.append_long(data)
        }
    }

    fn append_long(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            if self.block_left == 0 {
                self.finish_block(false);
            }
            let n = data.len().min(self.block_left);
            let (chunk, rest) = data.split_at(n);
            self.buffer.extend_from_slice(chunk);
            self.block_left -= n;
            data = rest;
        }
    }

    fn finish_block(&mut self, end: bool) {
        let used = BLOCKSIZE - self.block_left;
        let header = Header::new(used, end);
        let start = self.buffer.len() - used - 2;
        let dest = &mut self.buffer[start..start + 2];
        let dest: &mut [u8; 2] = dest.try_into().unwrap();
        *dest = *header.as_bytes();
        // make room for next header
        self.buffer.reserve(BLOCKSIZE + 2);
        // obvious dummy header
        self.buffer.push(0xFF);
        self.buffer.push(0xFF);
        self.block_left = BLOCKSIZE;
    }

    pub fn end(&mut self) {
        self.finish_block(true);
    }

    pub fn reset(&mut self) -> &[u8] {
        let raw_base = self.buffer.as_ptr();
        let mut raw_len = self.buffer.len();
        if self.block_left == BLOCKSIZE {
            raw_len -= 2;
        }
        // now reset the buffer but make sure not to overwrite the initial
        // header yet
        self.buffer.truncate(2);
        self.block_left = BLOCKSIZE;
        unsafe { std::slice::from_raw_parts(raw_base, raw_len) }
    }

    pub fn end_reset(&mut self) -> &[u8] {
        self.end();
        self.reset()
    }

    pub fn write_reset<W: io::Write>(&mut self, mut wr: W) -> io::Result<W> {
        let data = self.end_reset();
        wr.write_all(data)?;
        Ok(wr)
    }

    pub fn write_reset_plus<W: io::Write>(&mut self, wr: W, extra: &[&[u8]]) -> io::Result<W> {
        // we should really use a vectored write here
        for chunk in extra {
            self.append(chunk);
        }
        self.write_reset(wr)
    }

    pub fn peek(&self) -> &[u8] {
        &self.buffer
    }
}

impl fmt::Write for MapiBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.append(s);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::iter::{self, Map};

    use crate::util::referencedata::ReferenceData;

    use super::*;

    #[test]
    fn test_simple_append() {
        let mut mb = MapiBuf::new();

        mb.append([]);
        mb.append([]);
        mb.append(b"AAA");
        assert_eq!(mb.end_reset(), &[7, 0, b'A', b'A', b'A']);
    }

    #[test]
    fn test_complex() {
        let aaa: Vec<u8> = iter::repeat(b'A').take(BLOCKSIZE).collect();

        let mut mb = MapiBuf::new();
        mb.append(b"12345");
        mb.append(&aaa);
        let actual = mb.end_reset();

        let mut refd = ReferenceData::new();
        refd.data(Header::new(BLOCKSIZE, false));
        refd.data(b"12345".as_slice());
        refd.data(&aaa[..BLOCKSIZE - 5]);
        refd.mark("second block");
        refd.data(Header::new(5, true));
        refd.data(b"AAAAA".as_slice());

        let mut verifier = refd.verifier();
        verifier.assert(actual);
        verifier.assert_end();
    }
}
