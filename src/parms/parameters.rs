// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation
use array_macro::array;
use std::mem;

use urlparser::{is_our_url, parse_any_url, url_from_parms};

use super::*;

type Cowstr = Cow<'static, str>;

/// Identifies all things that can be configured when connecting to MonetDB, for
/// example [`Host`][`Parm::Host`], [`Port`][`Parm::Port`] and
/// [`Password`][`Parm::Password`].
///
/// Note: Rustdoc displays numeric values for the enum variants but these must
/// not be considered part of the API. For a stable way to obtain a numeric value for
/// a Parm, consider [`Parm::index`].
#[derive(
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Clone,
    Copy,
    enum_utils::IterVariants,
    enum_utils::FromStr,
)]
#[repr(u8)]
#[enumeration(rename_all = "lowercase")]
pub enum Parm {
    Database,
    Host,
    Port,
    Tls,
    User,
    Password,

    Autocommit,
    Binary,
    Cert,
    CertHash,
    ClientCert,
    ClientKey,
    Language,
    #[enumeration(alias = "fetchsize")]
    ReplySize,
    Schema,
    Sock,
    SockDir,
    Timezone,

    // Specific to this crate
    #[enumeration(rename = "client_info")]
    ClientInfo,
    #[enumeration(rename = "client_application")]
    ClientApplication,
    #[enumeration(rename = "client_remark")]
    ClientRemark,

    // Unused but recognized to pass the tests
    TableSchema,
    Table,
    Hash,
    Debug,
    Logfile,
    MaxPrefetch,
}

impl Parm {
    /// Return the name of this parameter when used in a URL.
    pub fn as_str(&self) -> &'static str {
        match self {
            Parm::Database => "database",
            Parm::Host => "host",
            Parm::Port => "port",
            Parm::Tls => "tls",
            Parm::User => "user",
            Parm::Password => "password",
            Parm::Autocommit => "autocommit",
            Parm::Binary => "binary",
            Parm::Cert => "cert",
            Parm::CertHash => "certhash",
            Parm::ClientCert => "clientcert",
            Parm::ClientKey => "clientkey",
            Parm::Language => "language",
            Parm::ReplySize => "replysize",
            Parm::Schema => "schema",
            Parm::Sock => "sock",
            Parm::SockDir => "sockdir",
            Parm::Timezone => "timezone",
            Parm::ClientInfo => "client_info",
            Parm::ClientApplication => "client_application",
            Parm::ClientRemark => "client_remark",
            Parm::TableSchema => "tableschema",
            Parm::Table => "table",
            Parm::Hash => "hash",
            Parm::Debug => "debug",
            Parm::Logfile => "logfile",
            Parm::MaxPrefetch => "maxprefetch",
        }
    }

    /// Convert the parameter into a number that can be used to index
    /// an array of values.
    pub const fn index(&self) -> usize {
        let idx = unsafe {
            // SAFETY: Self is repr(u8) so it will fit and be one-to-one
            mem::transmute::<Self, u8>(*self) as usize
        };
        // Theoretically, the compiler could assign any index whatover to the Parms.
        // However, most likely they will be consecutive starting at or near 0.
        // The compioler will then optimize this away.
        // If we ever find a compiler which does assign high numbers we can
        // get around it by simply setting PARM_TABLE_SIZE to 256.
        assert!(idx < PARM_TABLE_SIZE);
        idx
    }

    /// Returns whether the parameter is a core parameter. There are six core
    /// parameters: tls, host, port, database, tableschema and table. The core
    /// parameters are not allowed to occur in the query string of a URL.
    pub fn is_core(&self) -> bool {
        use Parm::*;
        matches!(self, Tls | Host | Port | Database | TableSchema | Table)
    }

    /// Returns whether the parameter must be suppressed when parameters
    /// are for example logged. Currently true for User and Password.
    pub fn is_sensitive(&self) -> bool {
        matches!(self, Parm::User | Parm::Password)
    }

    /// If `Parm::from_str` fails, this method determines whether this
    /// should be ignored (true) or considered an error (false).
    pub fn ignored(name: &str) -> bool {
        name.contains('_')
    }

    #[allow(dead_code)]
    pub(crate) fn parm_type(&self) -> ParmType {
        use Parm::*;
        use ParmType::*;
        match self {
            Tls | Autocommit | ClientInfo => Bool,
            Port | ReplySize | Timezone | MaxPrefetch => Int,
            _ => Str,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn require_bool(&self) -> bool {
        matches!(self, Parm::Tls | Parm::Autocommit)
    }

    #[allow(dead_code)]
    pub(crate) fn require_int(&self) -> bool {
        matches!(self, Parm::Port | Parm::ReplySize | Parm::Timezone)
    }
}

impl fmt::Display for Parm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

#[test]
fn test_parm_names() {
    assert_eq!(Parm::from_str("database"), Ok(Parm::Database));
    assert_eq!(Parm::from_str("host"), Ok(Parm::Host));
    assert_eq!(Parm::from_str("port"), Ok(Parm::Port));
    assert_eq!(Parm::from_str("tls"), Ok(Parm::Tls));
    assert_eq!(Parm::from_str("user"), Ok(Parm::User));
    assert_eq!(Parm::from_str("password"), Ok(Parm::Password));
    assert_eq!(Parm::from_str("autocommit"), Ok(Parm::Autocommit));
    assert_eq!(Parm::from_str("binary"), Ok(Parm::Binary));
    assert_eq!(Parm::from_str("cert"), Ok(Parm::Cert));
    assert_eq!(Parm::from_str("certhash"), Ok(Parm::CertHash));
    assert_eq!(Parm::from_str("clientcert"), Ok(Parm::ClientCert));
    assert_eq!(Parm::from_str("clientkey"), Ok(Parm::ClientKey));
    assert_eq!(Parm::from_str("language"), Ok(Parm::Language));
    assert_eq!(Parm::from_str("replysize"), Ok(Parm::ReplySize));
    assert_eq!(Parm::from_str("schema"), Ok(Parm::Schema));
    assert_eq!(Parm::from_str("sock"), Ok(Parm::Sock));
    assert_eq!(Parm::from_str("sockdir"), Ok(Parm::SockDir));
    assert_eq!(Parm::from_str("timezone"), Ok(Parm::Timezone));
    assert_eq!(Parm::from_str("client_info"), Ok(Parm::ClientInfo));
    assert_eq!(
        Parm::from_str("client_application"),
        Ok(Parm::ClientApplication)
    );
    assert_eq!(Parm::from_str("client_remark"), Ok(Parm::ClientRemark));
    // special case
    assert_eq!(Parm::from_str("fetchsize"), Ok(Parm::ReplySize));

    for parm in Parm::iter() {
        assert_eq!(Parm::from_str(parm.as_str()), Ok(parm), "parm {parm:?}");
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ParmType {
    Bool,
    Int,
    Str,
}

/// Try to convert a string to a boolean.
///
/// Case insensitive.  The strings "yes", "true" and "on"
/// map to `true` and the strings "no", "false" and "off"
/// map to `false`.
pub fn parse_bool(s: &str) -> Option<bool> {
    for yes in ["yes", "true", "on"] {
        if yes.eq_ignore_ascii_case(s) {
            return Some(true);
        }
    }
    for no in ["no", "false", "off"] {
        if no.eq_ignore_ascii_case(s) {
            return Some(false);
        }
    }
    None
}

pub fn render_bool(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

/// Type [`Value`] can hold the possible values for these parameters, glossing over
/// the distinction between strings, numbers and booleans.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(Cowstr),
}

impl Value {
    /// Construct a `Value` from a `&str` without copying.
    /// If you use `Value::from_str`, the `from_str` cannot notice
    /// that the lifetime is static so it would copy the string
    /// instead of putting the static reference into a `Cow::Borrowed`.
    pub const fn from_static(s: &'static str) -> Value {
        Value::Str(Cow::Borrowed(s))
    }

    /// Try to convert the value to a `bool`
    pub fn bool_value(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(_) => None,
            Value::Str(s) => parse_bool(s),
        }
    }

    /// Try to convert the value to an `bool`
    pub fn int_value(&self) -> Option<i64> {
        match self {
            Value::Bool(_) => None,
            Value::Int(i) => Some(*i),
            Value::Str(s) => s.parse().ok(),
        }
    }

    pub(crate) fn binary_value(&self) -> Option<u16> {
        match self.bool_value() {
            Some(false) => Some(0),
            Some(true) => Some(65535),
            None => u16::try_from(self.int_value()?).ok(),
        }
    }

    /// Render the value as a string. This yields a `Cow::Borrowed` value
    /// if it's set as a string or bool but it must allocate a new `Cow::Owned`
    /// value if it's a number.
    pub fn str_value(&self) -> Cow<'_, str> {
        match self {
            Value::Bool(b) => Cow::Borrowed(render_bool(*b)),
            Value::Int(i) => i.to_string().into(),
            Value::Str(cow) => Cow::Borrowed(cow),
        }
    }

    /// Like [`str_value`], but takes ownership of the value.
    pub fn into_str(self) -> Cowstr {
        match self {
            Value::Bool(b) => render_bool(b).into(),
            Value::Int(i) => i.to_string().into(),
            Value::Str(cow) => cow,
        }
    }

    /// Verify if the Value can be assigned to the given Parm.
    ///
    /// For example, it can only be assigned to [`Parm::Autocommit`]
    /// if it's a boolean or can be converted to a boolean.
    pub fn verify_assign(&self, parm: Parm) -> ParmResult<()> {
        let parm_type = parm.parm_type();
        // in most cases we check if the value can be converted,
        // but for strings we check if it's the actual variant
        match parm_type {
            ParmType::Bool => {
                self.bool_value().ok_or(ParmError::InvalidBool(parm))?;
            }
            ParmType::Int => {
                self.int_value().ok_or(ParmError::InvalidInt(parm))?;
            }
            ParmType::Str => {
                let Value::Str(_) = self else {
                    return Err(ParmError::MustBeString(parm));
                };
            }
        }
        Ok(())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.str_value().fmt(f)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Value {
        Value::Str(value.to_string().into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Value {
        Value::Str(value.into())
    }
}

impl<'a> From<Cow<'a, str>> for Value {
    fn from(value: Cow<'a, str>) -> Value {
        let s = match value {
            Cow::Owned(s) => s,
            Cow::Borrowed(s) => s.to_string(),
        };
        Value::Str(s.into())
    }
}

impl From<i8> for Value {
    fn from(value: i8) -> Self {
        Value::Int(value.into())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Value::Int(value.into())
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Value::Int(value.into())
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Value::Int(value.into())
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Int(value.into())
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Value::Int(value.into())
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<isize> for Value {
    fn from(value: isize) -> Self {
        Value::Int(value.try_into().unwrap())
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value::Int(value.try_into().unwrap())
    }
}

/// If you want to create a table indexed by [`Parm`], the table must
/// have at least this number of elements. Use [`Parm::index`] to convert
/// Parms to usizes.
pub const PARM_TABLE_SIZE: usize = 27;

#[test]
fn test_parm_table_size() {
    for p in Parm::iter() {
        // this will already panic:
        let idx = p.index();
        // but pretend we use the value
        assert!(idx < PARM_TABLE_SIZE);
    }
}

/// Holds unvalidated connection parameters.
///
/// This is basically a mapping from [`Parm`] to [`Value`] with lots of helper
/// methods. Call [`Parameters::validate`] to validate and interpret them.
///
/// This type also keeps track of when user name and password have last been
/// set. When [`Parameters::boundary`] is called and only one has been touched,
/// the other is cleared. This happens for example before and after parsing a
/// URL.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Parameters {
    parms: [Value; PARM_TABLE_SIZE],
    user_changed: bool,
    password_changed: bool,
    timezone_set: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        DEFAULT_PARAMETERS
    }
}

/// A constant holding the default values of the parameters.
/// Most are clear. Can be used in a const context.
/// See also [`THE_DEFAULT_PARAMETERS`].
pub const DEFAULT_PARAMETERS: Parameters = {
    let parms = array![i => default_parameter_value_by_index(i); PARM_TABLE_SIZE];
    Parameters {
        parms,
        user_changed: false,
        password_changed: false,
        timezone_set: false,
    }
};

/// A static value containing the default parameters.
/// You can take `&static` references of it.
/// See also [`DEFAULT_PARAMETERS`].
static THE_DEFAULT_PARAMETERS: Parameters = DEFAULT_PARAMETERS;

// This function is only used in the definition of DEFAULT_PARAMETERS. It's the
// source of truth for the default parameter values.
//
// It takes usize rather than Parm because we need some trickery due to the
// const context it will be evaluated in.
const fn default_parameter_value_by_index(idx: usize) -> Value {
    use Parm::*;
    if idx == Tls.index() {
        Value::Bool(false)
    } else if idx == Port.index() {
        Value::Int(-1)
    } else if idx == SockDir.index() {
        Value::from_static("/tmp")
    } else if idx == Language.index() {
        Value::from_static("sql")
    } else if idx == Autocommit.index() {
        Value::Bool(true) // arbitrary choice
    } else if idx == Timezone.index() {
        Value::Int(0)
    } else if idx == ReplySize.index() {
        Value::Int(200)
    } else if idx == Binary.index() {
        Value::from_static("on") // we can't yet, but we'd like to
    } else if idx == ClientInfo.index() {
        Value::Bool(true)
    } else {
        Value::from_static("")
    }
}

impl Parameters {
    /// Create a new Parameters object with database, user name and password
    /// initialized to the given values.
    pub fn basic(database: &str, user: &str, password: &str) -> ParmResult<Parameters> {
        use Parm::*;
        let mut parms = Parameters::default();
        if is_our_url(database) {
            parms.apply_url(database)?;
        } else {
            parms.set(Database, database)?;
        }
        if !user.is_empty() {
            parms.set(User, user)?;
        }
        if !password.is_empty() {
            parms.set(Password, password)?;
        }
        parms.boundary();
        Ok(parms)
    }

    /// Create a new Parameters object with database, user name and password
    /// initialized from the given URL.
    pub fn from_url(url: &str) -> ParmResult<Parameters> {
        let mut parms = Parameters::default();
        parms.apply_url(url)?;
        Ok(parms)
    }

    /// Replace the existing value of a Parm with a new value.
    ///
    /// Primitive on which all setters and [`Parameters::take`] are based.
    pub fn replace(&mut self, parm: Parm, value: impl Into<Value>) -> ParmResult<Value> {
        match parm {
            Parm::User => self.user_changed = true,
            Parm::Password => self.password_changed = true,
            Parm::Timezone => self.timezone_set = true,
            _ => {}
        }

        let mut value: Value = value.into();
        value.verify_assign(parm)?;
        mem::swap(&mut self.parms[parm.index()], &mut value);
        Ok(value)
    }

    /// Set a Parm to a new value.
    pub fn set(&mut self, parm: Parm, value: impl Into<Value>) -> ParmResult<()> {
        self.replace(parm, value)?;
        Ok(())
    }

    /// Set a Parm to its default value.
    pub fn reset(&mut self, parm: Parm) {
        self.set(parm, THE_DEFAULT_PARAMETERS.get(parm).clone())
            .unwrap();
    }

    /// Retrieve the value of a Parm as a [`Value`].
    pub fn get(&self, parm: Parm) -> &Value {
        &self.parms[parm.index()]
    }

    /// Retrieve the value of a Parm as a `bool`.
    pub fn get_bool(&self, parm: Parm) -> ParmResult<bool> {
        self.get(parm)
            .bool_value()
            .ok_or(ParmError::InvalidBool(parm))
    }

    /// Retrieve the value of a Parm as an `i64`.
    pub fn get_int(&self, parm: Parm) -> ParmResult<i64> {
        self.get(parm)
            .int_value()
            .ok_or(ParmError::InvalidInt(parm))
    }

    /// Retrieve the value of a Parm as a `&str`.
    pub fn get_str(&self, parm: Parm) -> ParmResult<Cow<'_, str>> {
        Ok(self.get(parm).str_value())
    }

    /// Take the value of the Parm out of this Parameters object, replacing it with its
    /// default value. Can sometimes be used to save an allocation.
    pub fn take(&mut self, parm: Parm) -> Value {
        self.replace(parm, THE_DEFAULT_PARAMETERS.get(parm).clone())
            .unwrap()
    }

    /// Set the value of a Parm which is specified by name, as a `&str` rather
    /// than a `Parm`. If the name is not known, [`Parm::ignored`] is used to
    /// decide whether that's an error or a no-op.
    pub fn set_named(&mut self, parm_name: &str, value: impl Into<Value>) -> ParmResult<()> {
        let Ok(parm) = Parm::from_str(parm_name) else {
            if Parm::ignored(parm_name) {
                return Ok(());
            } else {
                return Err(ParmError::UnknownParameter(parm_name.to_string()));
            }
        };
        self.set(parm, value)
    }

    /// Returns whether the given Parm currently has its default value.
    pub fn is_default(&self, parm: Parm) -> bool {
        let value = self.get(parm);
        let default_value = THE_DEFAULT_PARAMETERS.get(parm);
        match default_value {
            Value::Bool(b) => value.bool_value() == Some(*b),
            Value::Int(i) => value.int_value() == Some(*i),
            Value::Str(s) => {
                let left: &str = s;
                let right: &str = &value.str_value();
                left == right
            }
        }
    }

    /// If exactly one of user name and password has been set since
    /// the previous call to this method, clear the other.
    pub fn boundary(&mut self) {
        match (self.user_changed, self.password_changed) {
            (true, false) => self.reset(Parm::Password),
            (false, true) => self.reset(Parm::User),
            _ => {}
        }
        self.user_changed = false;
        self.password_changed = false;
    }

    /// Overwrite Parms with values found in the given URL.
    ///
    /// Supports `monetdb://`, `monetdbs://` and `mapi:monetdb://` URLs.
    pub fn apply_url(&mut self, url: &str) -> ParmResult<()> {
        self.boundary();
        parse_any_url(self, url)?;
        self.boundary();
        Ok(())
    }

    /// Check if the parameters have sensible values and if so,
    /// return a [`Validated`] object for them.
    pub fn validate(&self) -> ParmResult<Validated<'_>> {
        Validated::new(self)
    }
}

// Builder pattern
impl Parameters {
    pub fn set_database(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::Database, value)
    }

    pub fn with_database(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_database(value)?;
        Ok(self)
    }

    pub fn set_host(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::Host, value)
    }

    pub fn with_host(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_host(value)?;
        Ok(self)
    }

    pub fn set_port(&mut self, value: u16) -> ParmResult<()> {
        self.set(Parm::Port, value)
    }

    pub fn with_port(mut self, value: u16) -> ParmResult<Parameters> {
        self.set_port(value)?;
        Ok(self)
    }

    pub fn set_tls(&mut self, value: bool) -> ParmResult<()> {
        self.set(Parm::Tls, value)
    }

    pub fn with_tls(mut self, value: bool) -> ParmResult<Parameters> {
        self.set_tls(value)?;
        Ok(self)
    }

    pub fn set_user(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::User, value)
    }

    pub fn with_user(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_user(value)?;
        Ok(self)
    }

    pub fn set_password(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::Password, value)
    }

    pub fn with_password(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_password(value)?;
        Ok(self)
    }

    pub fn set_autocommit(&mut self, value: bool) -> ParmResult<()> {
        self.set(Parm::Autocommit, value)
    }

    pub fn with_autocommit(mut self, value: bool) -> ParmResult<Parameters> {
        self.set_autocommit(value)?;
        Ok(self)
    }

    pub fn set_binary(&mut self, value: impl Into<Value>) -> ParmResult<()> {
        self.set(Parm::Binary, value)
    }

    pub fn with_binary(mut self, value: impl Into<Value>) -> ParmResult<Parameters> {
        self.set_binary(value)?;
        Ok(self)
    }

    pub fn set_cert(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::Cert, value)
    }

    pub fn with_cert(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_cert(value)?;
        Ok(self)
    }

    pub fn set_certhash(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::CertHash, value)
    }

    pub fn with_certhash(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_certhash(value)?;
        Ok(self)
    }

    pub fn set_clientcert(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::ClientCert, value)
    }

    pub fn with_clientcert(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_clientcert(value)?;
        Ok(self)
    }

    pub fn set_clientkey(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::ClientKey, value)
    }

    pub fn with_clientkey(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_clientkey(value)?;
        Ok(self)
    }

    pub fn set_language(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::Language, value)
    }

    pub fn with_language(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_language(value)?;
        Ok(self)
    }

    pub fn set_replysize(&mut self, value: impl Into<i64>) -> ParmResult<()> {
        self.set(Parm::ReplySize, value.into())
    }

    pub fn with_replysize(mut self, value: i64) -> ParmResult<Parameters> {
        self.set_replysize(value)?;
        Ok(self)
    }

    pub fn set_schema(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::Schema, value)
    }

    pub fn with_schema(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_schema(value)?;
        Ok(self)
    }

    pub fn set_sock(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::Sock, value)
    }

    pub fn with_sock(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_sock(value)?;
        Ok(self)
    }

    pub fn set_sockdir(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::SockDir, value)
    }

    pub fn with_sockdir(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_sockdir(value)?;
        Ok(self)
    }

    pub fn set_timezone(&mut self, value: impl Into<i64>) -> ParmResult<()> {
        self.set(Parm::Timezone, value.into())
    }

    pub fn with_timezone(mut self, value: impl Into<i64>) -> ParmResult<Parameters> {
        self.set_timezone(value)?;
        Ok(self)
    }

    pub fn set_client_info(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::ClientInfo, value)
    }

    pub fn with_client_info(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_client_info(value)?;
        Ok(self)
    }

    pub fn set_client_application(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::ClientApplication, value)
    }

    pub fn with_client_application(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_client_application(value)?;
        Ok(self)
    }

    pub fn set_client_remark(&mut self, value: &str) -> ParmResult<()> {
        self.set(Parm::ClientRemark, value)
    }

    pub fn with_client_remark(mut self, value: &str) -> ParmResult<Parameters> {
        self.set_client_remark(value)?;
        Ok(self)
    }
}

/// Indicates how the TLS certificate of the server must be verified.
#[derive(Debug, PartialEq, Eq)]
pub enum TlsVerify {
    /// No verification.
    Off,
    /// Compute the SHA-256 hash of the DER form of the leave certificate and check if it starts
    /// with the hexadecimal digits given by [`Parm::CertHash`].
    Hash,
    /// Verify that the server certificate is signed by the certificate given by [`Parm::Cert`].
    Cert,
    /// Use the certificates in the system certificate store to determine if the
    /// server certificate is valid.
    System,
}

/// Derived from a [`Parameters`], holds validated and processed connection
/// parameters.
///
/// For example, based on the combination of `host`, `port`, `database` and
/// `sock` it knows whether a connection must be made to a Unix Domain socket, a
/// TCP socket or both.
#[derive(Debug)]
pub struct Validated<'a> {
    pub database: Cow<'a, str>,
    pub tls: bool,
    pub user: Cow<'a, str>,
    pub password: Cow<'a, str>,
    pub autocommit: bool,
    pub cert: Cow<'a, str>,
    pub language: Cow<'a, str>,
    pub replysize: usize,
    pub schema: Cow<'a, str>,
    pub client_info: bool,
    pub client_application: Cow<'a, str>,
    pub client_remark: Cow<'a, str>,
    pub connect_timezone_seconds: Option<i32>,
    pub connect_scan: bool,
    pub connect_unix: Cow<'a, str>,
    pub connect_tcp: Cow<'a, str>,
    pub connect_port: u16,
    pub connect_tls_verify: TlsVerify,
    pub connect_certhash_digits: String,
    pub connect_clientkey: Cow<'a, str>,
    pub connect_clientcert: Cow<'a, str>,
    pub connect_binary: u16,
}

impl Validated<'_> {
    #[allow(unused_variables)]
    fn new(parms: &Parameters) -> ParmResult<Validated> {
        use Parm::*;
        use ParmError::*;

        // First extract all members, type checking them in the process
        let raw_database: Cow<str> = parms.get_str(Database)?;
        let raw_host: Cow<str> = parms.get_str(Host)?;
        let raw_port: i64 = parms.get_int(Port)?;
        let raw_tls: bool = parms.get_bool(Tls)?;
        let raw_user: Cow<str> = parms.get_str(User)?;
        let raw_password: Cow<str> = parms.get_str(Password)?;
        let raw_autocommit: bool = parms.get_bool(Autocommit)?;
        let raw_cert: Cow<str> = parms.get_str(Cert)?;
        let raw_certhash: Cow<str> = parms.get_str(CertHash)?;
        let raw_clientcert: Cow<str> = parms.get_str(ClientCert)?;
        let raw_clientkey: Cow<str> = parms.get_str(ClientKey)?;
        let raw_language: Cow<str> = parms.get_str(Language)?;
        let raw_replysize: i64 = parms.get_int(ReplySize)?;
        let raw_schema: Cow<str> = parms.get_str(Schema)?;
        let raw_sock: Cow<str> = parms.get_str(Sock)?;
        let raw_sockdir: Cow<str> = parms.get_str(SockDir)?;

        let raw_timezone: i64 = parms.get_int(Timezone)?;
        let raw_binary: &Value = parms.get(Binary);

        let raw_client_info = parms.get_bool(ClientInfo)?;
        let raw_client_application = parms.get_str(ClientApplication)?;
        let raw_client_remark = parms.get_str(ClientRemark)?;

        let raw_tableschema: Cow<str> = parms.get_str(TableSchema)?;
        let raw_table: Cow<str> = parms.get_str(Table)?;

        // 1. The parameters have the types listed in the table in Section
        //    Parameters.
        //
        // Checked during extraction

        // 2. At least one of sock and host must be empty.
        if !raw_host.is_empty() && !raw_sock.is_empty() {
            return Err(HostSockConflict);
        }

        // 3. The string parameter binary must either parse as a boolean or as a
        //    non-negative integer.
        let Some(connect_binary) = raw_binary.binary_value() else {
            return Err(ParmError::InvalidBinary);
        };

        // 4. If sock is not empty, tls must be 'off'.
        if !raw_sock.is_empty() && raw_tls {
            return Err(OnlyWithTls(Sock));
        }

        // 5. If certhash is not empty, it must be of the form sha256:hexdigits
        //    where hexdigits is a non-empty sequence of 0-9, a-f, A-F and
        //    colons.
        let connect_certhash_digits = if raw_certhash.is_empty() {
            String::new()
        } else {
            Self::valid_certhash(&raw_certhash)?
        };

        // 6. If tls is 'off', cert and certhash must be 'off' as well.
        if !raw_tls {
            if !raw_cert.is_empty() {
                return Err(OnlyWithTls(Cert));
            }
            if !raw_certhash.is_empty() {
                return Err(OnlyWithTls(CertHash));
            }
        }

        // 7. Parameters database, tableschema and table must consist only of
        //    upper- and lowercase letters, digits, periods, dashes and
        //    underscores. They must not start with a dash. If table is not
        //    empty, tableschema must also not be empty. If tableschema is not
        //    empty, database must also not be empty.
        let database = Self::valid_name(Database, raw_database)?;
        let _tableschema = Self::valid_name(TableSchema, raw_tableschema)?;
        let _table = Self::valid_name(Schema, raw_table)?;

        // 8. Parameter port must be -1 or in the range 1-65535.
        let connect_port = match raw_port {
            -1 => 50000,
            1..=65535 => raw_port as u16,
            _ => return Err(InvalidValue(Port)),
        };

        // 9. If clientcert is set, clientkey must also be set.
        if !raw_clientcert.is_empty() && raw_clientkey.is_empty() {
            return Err(ClientCertRequiresKey);
        }

        // Specific to this crate
        if raw_client_info && raw_client_application.contains('\n') {
            return Err(ClientInfoNewline(ClientApplication));
        }
        if raw_client_info && raw_client_remark.contains('\n') {
            return Err(ClientInfoNewline(ClientRemark));
        }
        // Virtual parameters

        // connect_port and connect_binary have already been determined above

        let connect_scan = !database.is_empty()
            && raw_sock.is_empty()
            && raw_host.is_empty()
            && raw_port == -1
            && !raw_tls;

        let host_empty = raw_host.is_empty();
        let sock_empty = raw_sock.is_empty();

        let connect_unix = if !sock_empty {
            raw_sock
        } else if raw_tls {
            "".into()
        } else if host_empty {
            format!("{dir}/.s.monetdb.{connect_port}", dir = raw_sockdir).into()
        } else {
            "".into()
        };

        let connect_tcp = if !sock_empty {
            "".into()
        } else if host_empty {
            "localhost".into()
        } else {
            raw_host
        };

        let connect_tls_verify = if !raw_tls {
            TlsVerify::Off
        } else if !connect_certhash_digits.is_empty() {
            TlsVerify::Hash
        } else if !raw_cert.is_empty() {
            TlsVerify::Cert
        } else {
            TlsVerify::System
        };

        let connect_clientkey = raw_clientkey;
        let connect_clientcert = if !raw_clientcert.is_empty() {
            raw_clientcert
        } else {
            connect_clientkey.clone()
        };

        let connect_timezone_seconds = if parms.timezone_set {
            Some(raw_timezone as i32 * 60)
        } else {
            None
        };

        let Ok(replysize) = raw_replysize.try_into() else {
            return Err(ParmError::InvalidInt(Parm::ReplySize));
        };

        // Construct object

        let validated = Validated {
            database,
            tls: raw_tls,
            user: raw_user,
            password: raw_password,
            autocommit: raw_autocommit,
            cert: raw_cert,
            language: raw_language,
            replysize,
            schema: raw_schema,
            client_info: raw_client_info,
            client_application: raw_client_application,
            client_remark: raw_client_remark,
            connect_scan,
            connect_unix,
            connect_tcp,
            connect_port,
            connect_tls_verify,
            connect_certhash_digits,
            connect_clientkey,
            connect_clientcert,
            connect_timezone_seconds,
            connect_binary,
        };

        Ok(validated)
    }

    fn valid_name<T: AsRef<str>>(parm: Parm, name: T) -> ParmResult<T> {
        let the_error = Err(ParmError::InvalidValue(parm));

        let valid = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_';

        let s = name.as_ref();
        if !s.chars().all(valid) {
            return the_error;
        }
        if s.starts_with('-') {
            return the_error;
        }

        Ok(name)
    }

    fn valid_certhash(certhash: &str) -> ParmResult<String> {
        let Some(fingerprint) = certhash.strip_prefix("sha256:") else {
            return Err(ParmError::InvalidValue(Parm::CertHash));
        };
        let mut digits = String::with_capacity(fingerprint.len());
        for c in fingerprint.chars() {
            match c {
                '0'..='9' | 'a'..='f' => digits.push(c),
                'A'..='F' => digits.push(c.to_ascii_lowercase()),
                ':' => continue,
                _ => return Err(ParmError::InvalidValue(Parm::CertHash)),
            }
        }
        Ok(digits)
    }
}

impl Parameters {
    /// Convert the Parameters into a URL including user name and password.
    pub fn url_with_credentials(&self) -> ParmResult<String> {
        url_from_parms(self, Parm::iter())
    }

    /// Convert the Parameters into a URL not including user name and password.
    pub fn url_without_credentials(&self) -> ParmResult<String> {
        let selection = Parm::iter().filter(|p| !p.is_sensitive());
        url_from_parms(self, selection)
    }
}
