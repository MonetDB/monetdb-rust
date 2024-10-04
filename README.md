MonetDB bindings for Rust
=========================

Rust client for the [MonetDB](https://www.monetdb.org/) analytics database.

Note: this crate is in its early stages. The basics seem to work but a lot has
not been implemented yet and the API may change in incompatible ways at any
time.

Examples
--------

```rust
use std::error::Error;
use monetdb::Connection;

fn main() -> Result<(), Box<dyn Error>> {
    let url = "monetdb:///demo?user=monetdb&password=monetdb";
    let conn = Connection::connect_url(url)?;
    let mut cursor = conn.cursor();

    cursor.execute("SELECT hostname, clientpid, client, remark FROM sys.sessions")?;
    while cursor.next_row()? {
        // getters return Option< >, None means NULL
        let hostname: Option<&str> = cursor.get_str(0)?;
        let clientpid: Option<u32> = cursor.get_u32(1)?;
        let client: Option<&str> = cursor.get_str(2)?;
        let remark: Option<&str> = cursor.get_str(3)?; // usually NULL
        println!("host={hostname:?} clientpid={clientpid:?} client={client:?} remark={remark:?}",);
    }
    Ok(())
}

// Example output:
// host=Some("totoro") clientpid=Some(1895691) client=Some("libmapi 11.51.4") remark=None
// host=Some("totoro") clientpid=Some(1914127) client=Some("monetdb-rust 0.1.1") remark=None
```

You can also use a [`Parameters`] object to fine tune the connection parameters:

```rust
# use std::error::Error;
use monetdb::{Parameters, Connection};
# fn main() -> Result<(), Box<dyn Error>> {
let parms = Parameters::basic("demo", "monetdb", "monetdb")? // database / user / password
    .with_autocommit(false)?;
let conn = Connection::new(parms)?;
# Ok(())
# }
```

Current status
--------------

* Support for MonetDB Jun2020 (11.37.7) and higher. Older versions are highly
  likely to work but haven't been tested. If you need this, just ask.

* Rust 1.81 and higher. (TODO: check this)

* The full `monetdb://` connection URL syntax is supported, though not all features have been implemented.

* Most data types can be retrieved in string form using `get_str()`.
  Exception: blobs

* The primitive types bool, i8/u8, i16/u16, i32/u32, i64/u64, i128/u128,
  isize/usize, f32/f64 have typed getters, for example `get_i8()`.

* A single call to `Cursor::execute()` can return multiple result sets.

* extremely basic and untested TLS (`monetdbs://`) support can optionally be
  compiled in.

Not implemented yet but planned:

* parametrized queries

* start transaction / commit / rollback

* typed getters for decimal and temporal types

* BLOB support

* Full TLS support

* file transfers

* Binary result set

* Adaptive paging window sizes

* scanning /tmp for Unix Domain sockets

* Non-SQL, for example language=mal for MonetDB's tracing / profiling API

* PREPARE STATEMENT

* Async, seems to be needed for [sqlx]

* Integration with database frameworks such as [sqlx] and [Diesel].
  There does not seem to be a JDBC equivalent for Rust.

[sqlx]: https://crates.io/crates/sqlx

[Diesel]: https://crates.io/crates/diesel

Optional features
-----------------

The `monetdb` crate currently defines one optional feature:

* **rustls** Enable a first stab at supporting TLS connections using
  [rustls](https://crates.io/crates/rustls/). The TLS-related configuration
  parameters such as `cert=` and `clientkey=` aren't supported yet and there is
  no testing, but a basic `monetdbs://` URL seems to work.
  To try it, pass it on the command line like this:
  ```plain
  cargo run --features=rustls --example testconnect -- monetdbs://my.tls.host/demo
  ```
  or enable it in your application's Cargo.toml like this:
  ```plain
  [dependencies]
  monetdb = { version="0.1.1", features=["rustls"]}
  ```