First steps
===========

2024-09

There is no globally accepted API to comply to, like Java has with JDBC and
Python has with DBAPI. So we implement an API roughly similar to the Python
API. At a later stage we can then create wrappers for sql, Diesel, and whatnot.

For the time being the API is not async because that makes it easier to get
started. However, some of the bindings above are async so we'll have to add it
at a later stage.


Requirements
------------

1. Support MonetDB versions Jun2020 and higher

2. Support Rust 1.80 and higher. in the early stages we can still increase this
   number.

3. Support SQL and MAL. The latter primarily for tracing / profiling

4. Support all data types needed to run the monetdb-client-bench.

5. Sync API, possibly later on async API.

6. Support pipelining for things like Xclose and setting the reply size.

7. Small amount of unsafe code is allowed.


Design notes
------------

We'll have a high level `Connection` object which hands out `Cursor`s and
`PreparedStatement`s. They share a single `Mapi` object which holds the socket
and deals with framing.

With pipelining there must also be an object that keeps track of which pipelined
commands still need to be sent, and which pipelined responses still need to be handled.
Not quite sure where we'll put that. Let's ignore it for now.

Of course we'll fully follow the URL spec. We'll have a Parameters object holding
the properties and a Validated object that holds them after validation.

Note: Parameters and Validated now implemented.


