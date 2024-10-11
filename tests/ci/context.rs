// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

use anyhow::{bail, Result as AResult};

use monetdb::{Connection, Parameters};
use std::{
    env::{self, VarError},
    sync::{LazyLock, Mutex, MutexGuard},
};

const SERVER_URL_ENV_VAR: &str = "CI_SERVER_URL";
const DEFAULT_SERVER_URL: &str = "monetdb:///test-monetdb-rust";
const DEFAULT_USER: &str = "monetdb";
const DEFAULT_PASSWORD: &str = "monetdb";

/// This static either holds a mutex-protected Server Context or
/// the error message we got when we tried to create one.
static SERVER: LazyLock<Result<Mutex<Server>, String>> = LazyLock::new(initialize_server);

/// Get an exclusive handle on the server context, initializing if not already there.
pub fn get_server() -> MutexGuard<'static, Server> {
    match &*SERVER {
        Err(e) => panic!("{e}"),
        Ok(srv) => match srv.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        },
    }
}

pub struct Server {
    parms: Parameters,
}

impl Server {
    pub fn parms(&self) -> Parameters {
        self.parms.clone()
    }
}

fn initialize_server() -> Result<Mutex<Server>, String> {
    match parms_from_env(SERVER_URL_ENV_VAR, Some(DEFAULT_SERVER_URL)) {
        Ok(parms) => {
            let server = Server { parms };
            Ok(Mutex::new(server))
        }
        Err(e) => Err(format!("{SERVER_URL_ENV_VAR}: {e}")),
    }
}

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
    parms.validate()?;

    let test_parms = parms.clone().with_connect_timeout(2)?;
    let conn = Connection::new(test_parms)?;
    conn.close();

    Ok(parms)
}
