// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use digest::{Digest, DynDigest};

// https://github.com/RustCrypto/hashes?tab=readme-ov-file#supported-algorithms

// "RIPEMD160",
// "SHA512",
// "SHA384",
// "SHA256",
// "SHA224",
// "SHA1",

fn new_hasher<T: Digest + DynDigest + Default + 'static>() -> Box<dyn DynDigest> {
    Box::new(T::default())
}

type Algo = fn() -> Box<dyn DynDigest>;

pub fn find_algo(comma_separated_names: &str) -> Option<(&str, Algo)> {
    for name in comma_separated_names.split(',') {
        let constructor = match name {
            "RIPEMD160" => new_hasher::<ripemd::Ripemd160>,
            "SHA512" => new_hasher::<sha2::Sha512>,
            "SHA384" => new_hasher::<sha2::Sha384>,
            "SHA256" => new_hasher::<sha2::Sha256>,
            "SHA224" => new_hasher::<sha2::Sha224>,
            // "SHA1" => new_hasher::<Sha1>,
            _ => continue,
        };
        return Some((name, constructor));
    }
    None
}
