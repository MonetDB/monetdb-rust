# Change Log

## mapiproxy NEXTVERSION - YYYY-MM-DD

New features:

- Add Connection::metadata() method to information about the server.

- Add connect_timeout setting.

Bug fixes:

- Fix build issue on Windows, Unix domain sockets are not supported there.

Other:

- Add integration tests, by default they try to connect to
  `monetdb:///test-monetdb-rust`.


## mapiproxy 0.2.0 - 2024-10-04

First public release.

- This version can be used to connect to MonetDB and execute queries.
  The API is subject to change.

- There are typed getters for boolean and the various integer types.
  Other types, including decimals and temporal types, must be retrieved
  as strings and converted manually.

- Understands the full MonetDB URL syntax, though not all features have been
  implemented.

- There is a demo program and a number of unit tests but this release has
  not seen much testing.

- Has been tested mostly with MonetDB versions Aug2024 (11.51.3) and
  Jun2020 (11.37.13) but older versions are believed to work fine.

- Works with Rust 1.80.0, the exact minimum supported Rust version yet to be
  decided.

- Extremely basic and untested TLS support can optionally be compiled in
  be enabling the `rustls` Cargo feature.
