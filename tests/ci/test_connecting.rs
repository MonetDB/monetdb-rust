// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use crate::{get_server, AResult};
use monetdb::{Connection, Parameters};

#[test]
fn test_connect() -> AResult<()> {
    let ctx = get_server();
    let parms: Parameters = ctx.parms();
    let conn = Connection::new(parms)?;
    conn.close();
    Ok(())
}
