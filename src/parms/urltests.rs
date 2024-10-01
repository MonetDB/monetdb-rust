// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use std::fs;

use parameters::TlsVerify;

use super::*;

const IDENTIFIER: &str = "monetdb-rs";

#[derive(Debug, PartialEq, Eq, Clone)]
struct Failure(String);

type TestResult<T> = Result<T, Failure>;

impl From<ParmError> for Failure {
    fn from(value: ParmError) -> Self {
        Failure::new(format!("validation failed: {value}"))
    }
}

impl Failure {
    fn new(msg: impl Into<String>) -> Self {
        Failure(msg.into())
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

macro_rules! fail {
    ($($toks:tt),*) => {
        return Err(Failure::new(format!( $($toks),* )))
    }
}

struct UrlTester {
    state: State,
    active: bool,
    section: String,
}

impl UrlTester {
    fn new() -> Self {
        UrlTester {
            state: State::default(),
            active: false,
            section: "".into(),
        }
    }

    fn process_line(&mut self, line: &str) -> TestResult<()> {
        if !self.active {
            if line == "```test" {
                self.state = State::default();
                self.active = true;
            } else if line.starts_with('#') {
                self.section = line.to_string();
            }
            return Ok(());
        }
        if line == "```" {
            self.active = false;
            return Ok(());
        }

        self.state.process_line(line)
    }

    fn process_text(&mut self, file: &str, text: &str) {
        let mut lineno = 0;
        self.section = "".into();
        for line in text.lines() {
            lineno += 1;
            if let Err(failure) = self.process_line(line) {
                panic!(
                    "{file}:{lineno}: in section {sec:?}: {failure}",
                    sec = self.section,
                )
            }
        }
    }
}

#[derive(Debug, Default)]
struct State {
    parms: Parameters,
    disabled: bool,
}

impl State {
    fn process_line(&mut self, line: &str) -> TestResult<()> {
        if self.disabled {
            return Ok(());
        }
        let line = line.trim();
        if line.is_empty() {
            return Ok(());
        }

        if let Some(url) = line.strip_prefix("PARSE ") {
            self.process_parse(url.trim_ascii_start(), false)
        } else if let Some(url) = line.strip_prefix("ACCEPT ") {
            self.process_parse(url.trim_ascii_start(), true)
        } else if let Some(url) = line.strip_prefix("REJECT ") {
            self.process_reject(url.trim_ascii_start())
        } else if let Some(assign) = line.strip_prefix("SET ") {
            let (key, value) = self.parse_assign(assign)?;
            self.process_set(key, value)
        } else if let Some(assign) = line.strip_prefix("EXPECT ") {
            let (key, value) = self.parse_assign(assign)?;
            self.process_expect(key, value)
        } else if let Some(libname) = line.strip_prefix("ONLY ") {
            self.process_only(libname.trim(), true)
        } else if let Some(libname) = line.strip_prefix("NOT ") {
            self.process_only(libname.trim(), false)
        } else {
            Err(Failure::new(format!("syntax error: {line}")))
        }
    }

    fn parse_assign<'a>(&self, assignment: &'a str) -> TestResult<(&'a str, &'a str)> {
        assert_eq!(assignment, assignment.trim());

        let Some((key, value)) = assignment.split_once('=') else {
            fail!("expected KEY=VALUE, found {assignment}")
        };
        Ok((key, value))
    }

    fn validate(&self) -> TestResult<()> {
        self.parms.clone().validate()?;
        Ok(())
    }

    fn lookup(&self, key: &str) -> TestResult<Value> {
        if let Ok(parm) = Parm::from_str(key) {
            let value = self.parms.get(parm);
            return Ok(value.clone());
        }

        let validated = self.parms.validate();
        let valid = validated.is_ok();

        match key {
            "valid" => Ok(valid.into()),
            "connect_scan" => Ok(validated?.connect_scan.into()),
            "connect_unix" => Ok(validated?.connect_unix.into()),
            "connect_tcp" => Ok(validated?.connect_tcp.into()),
            "connect_port" => Ok(validated?.connect_port.into()),
            "connect_tls_verify" => match validated?.connect_tls_verify {
                TlsVerify::Off => Ok("".into()),
                TlsVerify::Hash => Ok("hash".into()),
                TlsVerify::Cert => Ok("cert".into()),
                TlsVerify::System => Ok("system".into()),
            },
            "connect_certhash_digits" => Ok(validated?.connect_certhash_digits.into()),
            "connect_clientkey" => Ok(validated?.connect_clientkey.into()),
            "connect_clientcert" => Ok(validated?.connect_clientcert.into()),
            "connect_timezone" => Ok(validated?.connect_timezone_seconds.unwrap_or(42).into()),
            "connect_binary" => Ok(validated?.connect_binary.into()),
            _ => fail!("unknown key: {key}"),
        }
    }

    fn process_parse(&mut self, url: &str, validate: bool) -> TestResult<()> {
        self.parms.apply_url(url)?;
        if validate {
            self.validate()?;
        }
        Ok(())
    }

    fn process_reject(&mut self, url: &str) -> TestResult<()> {
        if self.process_parse(url, true).is_err() {
            Ok(())
        } else {
            fail!("this url should have been rejected");
        }
    }

    fn process_set(&mut self, key: &str, value: &str) -> TestResult<()> {
        self.parms.set_named(key, value)?;
        Ok(())
    }

    fn process_expect(&mut self, key: &str, expected: &str) -> TestResult<()> {
        let found = self.lookup(key)?;

        // deal with boolean names, for example true == on
        let found_bool = found.bool_value();
        let expected_bool = Value::from(expected).bool_value();

        if expected_bool.is_some() && expected_bool == found_bool {
            return Ok(());
        }

        let found_str = found.str_value();
        if found_str == expected {
            return Ok(());
        }
        fail!("expected {key}={expected:?}, found {found_str:?}")
    }

    fn process_only(&mut self, libname: &str, must_be_same: bool) -> TestResult<()> {
        let ok = if must_be_same {
            libname == IDENTIFIER
        } else {
            libname != IDENTIFIER
        };
        self.disabled = !ok;
        Ok(())
    }
}

#[test]
fn test_urlspec_adhoc() {
    use std::io::ErrorKind;
    let source = "adhoctest.md";
    let test_cases = match fs::read_to_string(source) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => return,
        Err(e) => panic!("cannot open {source}: {e}"),
    };
    let mut urltester = UrlTester::new();
    urltester.process_text(source, &test_cases);
}

#[test]
fn test_urlspec_test_cases() {
    let source = "src/parms/tests.md";
    let test_cases = fs::read_to_string(source).unwrap();

    let mut urltester = UrlTester::new();
    urltester.process_text(source, &test_cases);
}
