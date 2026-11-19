//! The integration tests, compiled into one binary so SurrealDB is linked once.
//!
//! The tree mirrors the sources: the tests of `crates/<crate>/src/<path>.rs` are in
//! `<crate>/<path>.rs`, with `orm` for `umbriel` and `macros` for `umbriel-macros`. A module's
//! own tests are in the file named after it, which declares its children. `server` covers the
//! remote transports.

mod macros;
mod orm;
mod server;
