// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use crate::{get_server, AResult};
use claims::assert_some;
use monetdb::{parms::Parm, Connection, Parameters};

#[test]
fn test_connect() -> AResult<()> {
    let ctx = get_server();
    let parms: Parameters = ctx.parms();
    let conn = Connection::new(parms)?;
    conn.close();
    Ok(())
}

#[test]
fn test_metadata() -> AResult<()> {
    let ctx = get_server();
    let parms: Parameters = ctx.parms();
    let mut conn = Connection::new(parms)?;
    let metadata = conn.metadata()?;
    let version = metadata.version();
    assert!(version >= (11, 3, 3));
    assert!(version.0 >= 11);
    assert!(version.1 >= 1);
    assert!(version.2 >= 1);
    assert_some!(metadata.env("monet_release"));
    Ok(())
}

#[test]
fn test_hashed_password() -> AResult<()> {
    let ctx = get_server();
    let mut parms: Parameters = ctx.parms();
    let user = parms.get_str(Parm::User)?.to_string();
    let password = parms.get_str(Parm::Password)?.to_string();

    // connect to learn hash algorithm used by server
    let mut conn = Connection::new(parms.clone())?;
    let metadata = conn.metadata()?;
    conn.close();

    // hash the password
    let hash_algo = metadata.password_prehash_algo();
    let mut hasher: Box<dyn digest::DynDigest> = match hash_algo {
        "SHA512" => Box::new(sha2::Sha512::default()),
        _ => {
            panic!("this test is not yet suitable for password hash {hash_algo}, please extend it")
        }
    };
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    let hexdigits = hex::encode(digest);
    let prehashed_password = format!("\u{0001}{hexdigits}");

    // Set the hashed password. Parameters requires us to also set the user
    // when we change the password
    parms.set_user(&user)?;
    parms.set_password(&prehashed_password)?;

    // try to connect
    if let Err(e) = Connection::new(parms) {
        panic!("While trying to connect with prehashed password {prehashed_password:?}: {e}");
    }

    Ok(())
}
