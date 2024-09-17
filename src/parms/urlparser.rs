use super::*;

use url::{Host, Url};

pub fn is_our_url(url: &str) -> bool {
    url.starts_with("monetdb://")
        || url.starts_with("monetdbs://")
        || url.starts_with("mapi:monetdb://")
}

pub fn parse_any_url(parms: &mut Parameters, url: &str) -> ParmResult<()> {
    if url.starts_with("monetdb://") {
        parse_monetdb_url(parms, false, url)
    } else if url.starts_with("monetdbs://") {
        parse_monetdb_url(parms, true, url)
    } else if url.starts_with("mapi:monetdb://") {
        parse_legacy_url(parms, url)
    } else {
        let msg = "must start with monetdb://, monetdbs:// or mapi:monetdb://";
        Err(ParmError::InvalidUrl(msg.to_string()))
    }
}

fn parse_monetdb_url(parms: &mut Parameters, use_tls: bool, url: &str) -> ParmResult<()> {
    let parsed = Url::parse(url).map_err(|e| ParmError::InvalidUrl(e.to_string()))?;

    parms.set_tls(use_tls)?;

    let host: Cow<'static, str> = match parsed.host() {
        None => "".into(),
        Some(Host::Domain(dom)) => match &*percent_decode(dom)? {
            "localhost" => "".into(),
            "localhost." => "localhost".into(),
            other => other.to_string().into(),
        },
        Some(Host::Ipv4(ip)) => ip.to_string().into(),
        Some(Host::Ipv6(ip)) => ip.to_string().into(),
    };
    parms.set(Parm::Host, host)?;

    if let Some(port) = parsed.port() {
        parms.set(Parm::Port, port)?;
    } else {
        parms.set(Parm::Port, -1)?;
    }

    if let Some(mut path_segments) = parsed.path_segments() {
        if let Some(database) = path_segments.next() {
            parms.set(Parm::Database, percent_decode(database)?)?;
        }
        if let Some(tableschema) = path_segments.next() {
            parms.set(Parm::TableSchema, percent_decode(tableschema)?)?;
        }
        if let Some(table) = path_segments.next() {
            parms.set(Parm::Table, percent_decode(table)?)?;
        }
        if let Some(unexpected) = path_segments.next() {
            return Err(ParmError::InvalidUrl(format!(
                "invalid path component {unexpected:?}"
            )));
        }
    }

    for (k, v) in parsed.query_pairs() {
        // k and v have already been percentdecoded
        let k = k.as_ref();
        let v = v.as_ref();
        let parm = match Parm::from_str(k) {
            Ok(p) => p,
            Err(()) if Parm::ignored(k) => continue,
            Err(()) => return Err(ParmError::UnknownParameter(k.to_string())),
        };
        if parm.is_core() {
            return Err(ParmError::NotAllowedAsQuery(parm));
        }
        parms.set(parm, v)?;
    }

    Ok(())
}

fn percent_decode(s: &str) -> ParmResult<Cow<'_, str>> {
    let data = s.as_bytes();

    let Some(idx) = data.iter().position(|c| *c == b'%') else {
        return Ok(Cow::Borrowed(s));
    };

    let mut buf = Vec::with_capacity(data.len());
    buf.extend_from_slice(&data[..idx]);

    fn unhex(digit: u8) -> ParmResult<u8> {
        match digit {
            b'0'..=b'9' => Ok(digit - b'0'),
            b'a'..=b'f' => Ok(digit - b'a' + 10),
            b'A'..=b'F' => Ok(digit - b'A' + 10),
            _ => Err(ParmError::InvalidPercentEncoding),
        }
    }

    let mut iter = data[idx..].iter();
    while let Some(&b) = iter.next() {
        if b != b'%' {
            buf.push(b);
            continue;
        }
        let Some(&hi) = iter.next() else {
            return Err(ParmError::InvalidPercentEncoding);
        };
        let Some(&lo) = iter.next() else {
            return Err(ParmError::InvalidPercentEncoding);
        };
        let byte = 16 * unhex(hi)? + unhex(lo)?;
        buf.push(byte);
    }

    match String::from_utf8(buf) {
        Ok(s) => Ok(Cow::Owned(s)),
        Err(_) => Err(ParmError::InvalidPercentUtf8),
    }
}

fn percent_encode(buffer: &mut String, s: &str) {
    for &byte in s.as_bytes() {
        let safe = matches!(
            byte,
            b'a' ..= b'z' | b'A' ..= b'Z' | b'0' ..= b'9'
            | b'-' | b'.' | b'_' | b'~' | b'!'
            | b'#' | b'$' | b'&' | b'\'' | b'('
            | b')' | b'*' | b'+' | b',' | b'/'
            | b':' | b';' | b'=' | b'?' | b'@'
            | b'[' | b']'
        );
        if safe {
            buffer.push(byte as char);
        } else {
            const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
            buffer.push('%');
            buffer.push(HEX_DIGITS[(byte / 16) as usize] as char);
            buffer.push(HEX_DIGITS[(byte % 16) as usize] as char);
        }
    }
}

#[test]
fn test_percent_decode() {
    #[track_caller]
    fn check(s: &str, expected: ParmResult<&str>) {
        let owned = percent_decode(s);
        let result = match &owned {
            Ok(s) => Ok(s.as_ref()),
            Err(e) => Err(e.clone()),
        };
        assert_eq!(result, expected)
    }

    check("", Ok(""));
    check("FOO", Ok("FOO"));
    check("%46OO", Ok("FOO"));
    check("F%4FO", Ok("FOO"));
    check("FO%4F", Ok("FOO"));
    check("F%4fO", Ok("FOO"));

    check("F%%O", Err(ParmError::InvalidPercentEncoding));
    check("F%4gO", Err(ParmError::InvalidPercentEncoding));
    check("F%g4O", Err(ParmError::InvalidPercentEncoding));

    check("F%", Err(ParmError::InvalidPercentEncoding));
    check("F%7", Err(ParmError::InvalidPercentEncoding));
    check("F%f", Err(ParmError::InvalidPercentEncoding));
    check("F%F", Err(ParmError::InvalidPercentEncoding));

    check("F%80O", Err(ParmError::InvalidPercentUtf8));
}

fn parse_legacy_url(parms: &mut Parameters, url: &str) -> ParmResult<()> {
    let parsed = Url::parse(&url[5..]).map_err(|e| ParmError::InvalidUrl(e.to_string()))?;

    if !parsed.username().is_empty() || parsed.password().is_some() {
        let msg =
            "no user name or password allowed after the ://, use ?user= and password= instead";
        return Err(ParmError::InvalidUrl(msg.to_string()));
    }

    if let Some(host) = parsed.host_str() {
        parms.set(Parm::Host, host)?;
        parms.set(Parm::Sock, "")?;
        // first path component is database name

        let mut database = "";
        if let Some(mut path_segments) = parsed.path_segments() {
            if let Some(db) = path_segments.next() {
                database = db;
                if let Some(unexpected) = path_segments.next() {
                    return Err(ParmError::InvalidUrl(format!(
                        "invalid path component {unexpected:?}"
                    )));
                }
            }
        };
        parms.set(Parm::Database, database)?;
    } else {
        parms.set(Parm::Host, "")?;
        parms.set(Parm::Sock, parsed.path())?;
        parms.set(Parm::Database, "")?; // can be overridden with query parameter
    }

    if let Some(port) = parsed.port() {
        parms.set(Parm::Port, port)?;
    } else {
        parms.set(Parm::Port, -1)?;
    }

    // Do not use parsed.query_pairs because it percent-decodes and
    // mapi:monetdb:// urls don't do that
    if let Some(query) = parsed.query() {
        for x in query.split('&') {
            if let Some((k, v)) = x.split_once('=') {
                match k {
                    "language" => parms.set(Parm::Language, v)?,
                    "database" => parms.set(Parm::Database, v)?,
                    _other => {}
                }
            }
        }
    }

    Ok(())
}

pub fn url_from_parms(
    parms: &Parameters,
    selection: impl IntoIterator<Item = Parm>,
) -> ParmResult<String> {
    use fmt::Write;
    use Parm::*;
    let mut url = String::with_capacity(80);

    let scheme = if parms.get_bool(Tls)? {
        "monetdbs"
    } else {
        "monetdb"
    };
    url.push_str(scheme);
    url.push_str("://");

    let port = match parms.get_int(Port)? {
        -1 | 50000 => None,
        p => Some(p),
    };
    let host = parms.get_str(Host)?;
    let mut host: &str = &host;
    if host == "localhost" {
        host = "";
    }
    if port.is_some() && host.is_empty() {
        host = "localhost";
    }
    percent_encode(&mut url, host);
    if let Some(p) = port {
        write!(url, ":{p}").unwrap();
    }

    let database = parms.get_str(Database)?;
    let tableschema = parms.get_str(TableSchema)?;
    let table = parms.get_str(Table)?;
    if !database.is_empty() || !tableschema.is_empty() || !table.is_empty() {
        url.push('/');
        percent_encode(&mut url, &database);
    }
    if !tableschema.is_empty() || !table.is_empty() {
        url.push('/');
        percent_encode(&mut url, &tableschema);
    }
    if !table.is_empty() {
        url.push('/');
        percent_encode(&mut url, &table);
    }

    let mut sep = '?';
    for p in selection {
        if p.is_core() {
            continue;
        }
        if !parms.is_default(p) {
            url.push(sep);
            url.push_str(p.as_str());
            url.push('=');
            let value = parms.get_str(p)?;
            percent_encode(&mut url, &value);
            sep = '&';
        }
    }

    Ok(url)
}
