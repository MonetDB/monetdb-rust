use std::{io, net::TcpListener};

// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use crate::{get_server, AResult};
use claims::assert_some;
use monetdb::{parms::Parm, Connection, CursorResult, Parameters};

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

#[test]
fn test_redirect() -> AResult<()> {
    fn get_server_fingerprint(conn: &mut Connection) -> CursorResult<(String, String)> {
        let md = conn.metadata()?;
        let pid = md.env("monet_pid").unwrap().to_string();
        let dir = md.env("gdk_dbpath").unwrap().to_string();
        Ok((pid, dir))
    }

    let ctx = get_server();
    let parms: Parameters = ctx.parms();
    let real_server_url = parms.url_with_credentials()?;
    let user = parms.get_str(Parm::User)?;
    let password = parms.get_str(Parm::Password)?;

    // Connect to the real server and extract a fingerprint we can check later.
    let mut conn = Connection::new(parms.clone())?;
    let expected_fingerprint = get_server_fingerprint(&mut conn)?;
    conn.close();

    // Spawn a fake server that redirects to the real one.
    let host = "127.0.0.1";
    let listener = TcpListener::bind((host, 0))?;
    let port = listener.local_addr()?.port();
    std::thread::spawn(|| run_redirect_server(listener, real_server_url));

    // Connect to the fake server
    let redirect_server_parms = Parameters::default()
        .with_host(host)?
        .with_port(port)?
        .with_user(&user)?
        .with_password(&password)?;
    let mut conn = Connection::new(redirect_server_parms)?;
    let fingerprint_found = get_server_fingerprint(&mut conn)?;
    conn.close();

    assert_eq!(fingerprint_found, expected_fingerprint);
    Ok(())
}

fn run_redirect_server(listener: TcpListener, redirect_to: String) {
    loop {
        let (mut conn, _peer) = listener.accept().unwrap();
        send_msg(
            &mut conn,
            "BANANA:merovingian:9:RIPEMD160,SHA512,SHA384,SHA256,SHA224,SHA1:LIT:SHA512:",
        )
        .unwrap();
        let _ = recv_msg(&mut conn).unwrap();
        send_msg(&mut conn, &format!("^{redirect_to}")).unwrap();
    }
}

fn send_msg(mut conn: impl io::Write, msg: &str) -> io::Result<()> {
    assert!(msg.len() < 8190);
    let hdr_val = 2 * msg.len() as u16 + 1;
    let hdr: [u8; 2] = hdr_val.to_le_bytes();
    conn.write_all(&hdr)?;
    conn.write_all(msg.as_bytes())?;
    Ok(())
}

fn recv_msg(mut conn: impl io::Read) -> io::Result<String> {
    let mut buffer = vec![];
    loop {
        let mut hdr = [0u8; 2];
        conn.read_exact(&mut hdr)?;
        let hdr_val = u16::from_le_bytes(hdr);
        let len = hdr_val as usize / 2;
        let last = (hdr_val & 1) > 0;

        let cur_end = buffer.len();
        buffer.resize(cur_end + len, 0u8);
        conn.read_exact(&mut buffer[cur_end..])?;

        if last {
            break;
        }
    }

    let s: String = String::from_utf8_lossy(&buffer).into();
    Ok(s)
}
