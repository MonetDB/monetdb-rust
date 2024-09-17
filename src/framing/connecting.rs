#![allow(dead_code)]

use core::{fmt, str};
use std::{
    borrow::Cow,
    io::{self, ErrorKind, Write},
    net::{TcpStream, ToSocketAddrs},
    os::unix::net::UnixStream,
    str::Utf8Error,
};

use crate::{
    framing::{reading::MapiReader, writing::MapiBuf},
    parms::{Parameters, ParmError, Validated},
    util::hash_algorithms,
    IoError,
};

use super::{ServerSock, ServerState};

#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum ConnectError {
    #[error("{0}")]
    Parm(#[from] ParmError),
    #[error("{0}")]
    IO(#[from] IoError),
    #[error("invalid utf-8 sequence")]
    UTF(#[from] Utf8Error),
    #[error("{0} in server challenge")]
    InvalidChallenge(String),
    #[error("server requested unsupported hash algorithm: {0}")]
    UnsupportedHashAlgo(String),
    #[error("TLS (monetdbs://) has not been enabled")]
    TlsNotSupported,
    #[error("too many redirects")]
    TooManyRedirects,
    #[error("login rejected: {0}")]
    Rejected(String),
    #[error("unexpected server response: {0:?}")]
    UnexpectedResponse(String),
}

pub type ConnResult<T> = Result<T, ConnectError>;

impl From<io::Error> for ConnectError {
    fn from(value: io::Error) -> Self {
        IoError::from(value).into()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Endian {
    Big,
    Lit,
}

impl Endian {
    #[cfg(target_endian = "little")]
    pub const NATIVE: Endian = Endian::Lit;

    #[cfg(target_endian = "big")]
    pub const NATIVE: Endian = Endian::Big;
}

impl fmt::Display for Endian {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Endian::Big => "BIG",
            Endian::Lit => "LIT",
        };
        s.fmt(f)
    }
}

fn connect_unix_socket(parms: &Validated) -> io::Result<ServerSock> {
    let path = parms.connect_unix.as_ref();
    match UnixStream::connect(path) {
        Ok(mut s) => {
            debug!("connected to {path}");
            s.write_all(b"0")?;
            Ok(ServerSock::new(s))
        }
        Err(e) => {
            debug!("{path}: {e}");
            Err(e)
        }
    }
}

fn connect_tcp_socket(parms: &Validated) -> io::Result<ServerSock> {
    let host = parms.connect_tcp.as_ref();
    let port = parms.connect_port;

    let mut err = None;
    for a in (host, port).to_socket_addrs()? {
        match TcpStream::connect(a) {
            Err(e) => {
                debug!("{a}: {e}");
                err = Some(e);
                continue;
            }
            Ok(sock) => {
                debug!("connected to {a}");
                if let Err(e) = sock.set_nodelay(true) {
                    debug!("failed to set nodelay: {e}");
                }
                return Ok(ServerSock::new(sock));
            }
        }
    }
    if let Some(e) = err {
        Err(e)
    } else {
        // unlikely, but apparently .to_sock_addrs returned an empty set and not an error.
        debug!("no ip addresses found for '{host}'");
        let err = io::Error::new(ErrorKind::NotFound, format!("no ip addresses for '{host}'"));
        Err(err)
    }
}

fn connect_socket(parms: &Validated) -> ConnResult<ServerSock> {
    let mut err = None;

    if parms.tls {
        return Err(ConnectError::TlsNotSupported);
    }

    if !parms.connect_unix.is_empty() {
        match connect_unix_socket(parms) {
            Ok(s) => return Ok(s),
            Err(e) => err = Some(e),
        }
    }
    if !parms.connect_tcp.is_empty() {
        match connect_tcp_socket(parms) {
            Ok(s) => return Ok(s),
            Err(e) => err = Some(e),
        }
    }
    Err(err.unwrap().into())
}

#[derive(Debug)]
enum Login {
    Redirect(String),
    Restart(ServerSock),
    Complete(ServerSock, ServerState),
}

pub fn establish_connection(mut parms: Parameters) -> ConnResult<(ServerSock, ServerState)> {
    'redirect: for _ in 0..10 {
        let validated = parms.validate()?;
        if log_enabled!(log::Level::Debug) {
            if let Ok(url) = parms.url_without_credentials() {
                debug!("connecting to {url}");
            }
        }
        let mut sock = connect_socket(&validated)?;
        'restart: loop {
            match login(&validated, sock)? {
                Login::Complete(sock, state) => {
                    return Ok((sock, state));
                }
                Login::Redirect(url) => {
                    debug!("redirected to {url}");
                    parms.apply_url(&url)?;
                    continue 'redirect;
                }
                Login::Restart(s) => {
                    debug!("local redirect, restarting authentication");
                    sock = s;
                    continue 'restart;
                }
            }
        }
    }
    Err(ConnectError::TooManyRedirects)
}

fn login(parms: &Validated, sock: ServerSock) -> ConnResult<Login> {
    let mut server_message = String::with_capacity(1000);
    let mut mbuf = MapiBuf::new();

    // read the challenge
    let sock = MapiReader::to_limited_string(sock, &mut server_message, 5000)?;

    // determine the response
    let chal = Challenge::new(&server_message)?;
    let mut response = String::with_capacity(500);
    let state = challenge_response(parms, &chal, &mut response)?;

    // send the response
    mbuf.append(response);
    let sock = mbuf.write_reset(sock)?;

    // read the server response
    server_message.clear();
    let sock = MapiReader::to_limited_string(sock, &mut server_message, 5000)?;

    // process the server
    process_redirects(sock, state, &server_message)
}

fn challenge_response(
    parms: &Validated,
    chal: &Challenge,
    response: &mut String,
) -> ConnResult<ServerState> {
    use fmt::Write;

    let my_endian = Endian::NATIVE;
    let (user, password) = if chal.server_type == "merovingian" {
        ("merovingian", "")
    } else {
        (&*parms.user, &*parms.password)
    };

    let prehashed_password = if password.starts_with('\u{0001}') {
        Cow::Borrowed(password)
    } else {
        let algo_name = chal.prehash_algo;
        let Some((_, algo)) = hash_algorithms::find_algo(algo_name) else {
            return Err(ConnectError::UnsupportedHashAlgo(algo_name.to_string()));
        };
        let mut hasher = algo();
        hasher.update(password.as_bytes());
        let bindigest = hasher.finalize();
        let hexdigest = hex::encode(bindigest);
        Cow::Owned(hexdigest)
    };

    let response_algos = chal.response_algos;
    let Some((algo_name, algo)) = hash_algorithms::find_algo(response_algos) else {
        return Err(ConnectError::UnsupportedHashAlgo(
            response_algos.to_string(),
        ));
    };
    let mut hasher = algo();
    let ph = prehashed_password.as_bytes();
    hasher.update(ph);
    let salt = chal.salt.as_bytes();
    hasher.update(salt);
    let hashed_password = hex::encode(hasher.finalize());

    let language = &*parms.language;
    let database = &*parms.database;

    write!(
        response,
        "{my_endian}:{user}:{{{algo_name}}}{hashed_password}:{language}:{database}:FILETRANS:"
    )
    .unwrap();

    let state = ServerState::default();
    Ok(state)
}

fn process_redirects(sock: ServerSock, state: ServerState, reply: &str) -> ConnResult<Login> {
    let reply = reply.trim_ascii();

    if reply.is_empty() || reply.starts_with("=OK") {
        debug!("login complete");
    } else if reply.starts_with('^') {
        // we only want the first one
        let first_line = reply.split('\n').next().unwrap();
        let redirect = &first_line[1..];
        if redirect.starts_with("mapi:merovingian://proxy") {
            return Ok(Login::Restart(sock));
        } else {
            return Ok(Login::Redirect(redirect.to_string()));
        }
    } else if let Some(message) = reply.strip_prefix('!') {
        debug!("login rejected: {message}");
        return Err(ConnectError::Rejected(message.to_string()));
    } else if let Some(message) = reply.strip_prefix('#') {
        debug!("login complete with welcome message {message:?}");
    } else {
        debug!("unexpected response: {reply:?}");
        return Err(ConnectError::UnexpectedResponse(reply.to_string()));
    }
    Ok(Login::Complete(sock, state))
}

#[derive(Debug)]
struct Challenge<'a> {
    salt: &'a str,
    server_type: &'a str,
    protocol: u8,
    response_algos: &'a str,
    endian: Endian,
    prehash_algo: &'a str,
    sql_handshake_option_level: u8,
    binary: u16,
    oobintr: u16,
    clientinfo: bool,
}

impl<'a> Challenge<'a> {
    fn new(line: &'a str) -> ConnResult<Self> {
        // trace!("parsing challenge {line:?}");
        let mut parts = line.trim_end_matches(':').split(':');

        let err = |msg: &str| ConnectError::InvalidChallenge(msg.to_string());

        let Some(salt) = parts.next() else {
            return Err(err("salt missing"));
        };

        let Some(server_type) = parts.next() else {
            return Err(err("server_type missing"));
        };

        let protocol = match parts.next() {
            Some("9") => 9,
            Some(_) => return Err(err("unknown protocol")),
            None => return Err(err("protocol missing")),
        };

        let Some(response_algos) = parts.next() else {
            return Err(err("hashes missing"));
        };

        let endian = match parts.next() {
            Some("BIG") => Endian::Big,
            Some("LIT") => Endian::Lit,
            Some(_) => return Err(err("invalid endian")),
            None => return Err(err("endian missing")),
        };

        let Some(prehash_algo) = parts.next() else {
            return Err(err("password hash algo missing"));
        };

        let mut sql_handshake_option_level = 0;
        if let Some(optlevels) = parts.next() {
            for optlevel in optlevels.split(',') {
                if let Some(lvl) = optlevel.strip_prefix("sql=") {
                    sql_handshake_option_level = lvl
                        .parse()
                        .map_err(|_| err("invalid handshake options level"))?;
                }
            }
        }

        let binary = if let Some(s) = parts.next() {
            let Some(n) = s.strip_prefix("BINARY=") else {
                return Err(err("invalid binary level"));
            };
            n.parse().map_err(|_| err("invalid binary level"))?
        } else {
            0
        };

        let oobintr = if let Some(s) = parts.next() {
            let Some(n) = s.strip_prefix("OOBINTR=") else {
                return Err(err("invalid binary level"));
            };
            n.parse().map_err(|_| err("invalid oobintr level"))?
        } else {
            0
        };

        let clientinfo = if let Some(s) = parts.next() {
            match s {
                "CLIENTINFO" => true,
                _ => return Err(err("invalid clientinfo")),
            }
        } else {
            false
        };

        let challenge = Challenge {
            salt,
            server_type,
            protocol,
            response_algos,
            endian,
            prehash_algo,
            sql_handshake_option_level,
            binary,
            oobintr,
            clientinfo,
        };
        Ok(challenge)
    }
}
