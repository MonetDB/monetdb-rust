// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

use anyhow::{bail, Context, Result as AResult};

use monetdb::{parms::Parm, ConnectResult, Connection, Cursor, Parameters};
use std::{
    env::{self, VarError},
    mem,
    sync::{LazyLock, Mutex, MutexGuard},
};

const SERVER_URL_ENV_VAR: &str = "CI_SERVER_URL";
const DEFAULT_SERVER_URL: &str = "monetdb:///test-monetdb-rust";
const DEFAULT_USER: &str = "monetdb";
const DEFAULT_PASSWORD: &str = "monetdb";

/// This static either holds a mutex-protected Server Context or
/// the error message we got when we tried to create one.
static SERVER: LazyLock<AResult<Mutex<Server>>> = LazyLock::new(find_and_initialize_server);

/// Get an exclusive handle on the server context, initializing if not already there.
pub fn get_server() -> MutexGuard<'static, Server> {
    match &*SERVER {
        Err(e) => panic!("{e:#}"),
        Ok(srv) => match srv.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        },
    }
}

pub struct Server {
    parms: Parameters,
    shared: Option<Connection>,
}

impl Server {
    pub fn parms(&self) -> Parameters {
        self.parms.clone()
    }

    pub fn connect(&self) -> ConnectResult<Connection> {
        Connection::new(self.parms())
    }
}

pub fn with_shared_server(f: impl FnOnce(Connection) -> AResult<Connection>) -> AResult<()> {
    let mut server = get_server();
    let conn = match mem::take(&mut server.shared) {
        Some(c) => c,
        None => server.connect()?,
    };

    let used_conn = f(conn)?;
    server.shared = Some(used_conn);
    Ok(())
}

pub fn with_shared_cursor(f: impl FnOnce(&mut Cursor) -> AResult<()>) -> AResult<()> {
    with_shared_server(|conn| {
        f(&mut conn.cursor())?;
        Ok(conn)
    })
}

fn find_and_initialize_server() -> AResult<Mutex<Server>> {
    match parms_from_env(SERVER_URL_ENV_VAR, Some(DEFAULT_SERVER_URL)) {
        Ok(parms) => {
            let mut conn = Connection::new(parms.clone())?;
            initialize_server(&mut conn).context("Could not initialize test database")?;
            let server = Server {
                parms,
                shared: Some(conn),
            };
            Ok(Mutex::new(server))
        }
        Err(e) => bail!("{SERVER_URL_ENV_VAR}: {e}"),
    }
}

const SQL: &str = include_str!("schema.sql");

fn initialize_server(conn: &mut Connection) -> AResult<()> {
    let mut cursor = conn.cursor();
    cursor.execute(SQL)?;
    cursor.close()?;
    Ok(())
}

/// Extract connection parameters from an environment variable
fn parms_from_env(env_var: &str, default_url: Option<&str>) -> AResult<Parameters> {
    let url = match env::var(env_var) {
        Ok(u) => u,
        Err(VarError::NotPresent) => {
            if let Some(u) = default_url {
                u.to_owned()
            } else {
                bail!("environment variable not set");
            }
        }
        Err(e) => return Err(e.into()),
    };

    let mut parms = Parameters::default()
        .with_user(DEFAULT_USER)?
        .with_password(DEFAULT_PASSWORD)?;
    parms.apply_url(&url)?;

    if parms.is_default(Parm::ConnectTimeout) {
        parms.set_connect_timeout(2)?;
    }

    parms.validate()?;
    Ok(parms)
}
