// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use crate::{get_server, AResult};
use claims::assert_some;
use monetdb::{Connection, Parameters};

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
