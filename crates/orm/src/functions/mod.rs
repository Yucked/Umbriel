//! SurrealDB's built-in functions, grouped by namespace as SurrealQL groups them.
//!
//! Each function `ns::name` has three forms, all rendering the same call:
//! - `ns::name!(args)`, a macro that also takes its optional arguments left out;
//! - `ns::name_fn(args)`, the function the macro calls;
//! - where SurrealDB 3.3.4 dispatches the function as a method, `receiver.name(args)` on the
//!   receiver's typed expression, e.g. `StrandLike::from(name).len()` renders `name.len()`.
//!
//! Every call is a [`Function<K>`](crate::Function) typed by its documented result kind (see
//! [`kinds`](crate::kinds)). The namespaces `api`, `file`, `search`, `sequence` and `vector`
//! belong to other work; `search` and `vector` keep their earlier wrappers.
//!
//! [See the SurrealDB documentation](https://surrealdb.com/docs/surrealql/functions/database)
//! for what each function does.

#[macro_use]
mod catalogue;

/// Array functions.
pub mod array;

/// Bytes functions.
pub mod bytes;

// `count()`, re-exported here.
mod count;
pub use count::*;

/// Hashing, password hashing and comparison functions.
pub mod crypto;

/// Duration functions and the `duration::MAX` constant.
pub mod duration;

/// Encoding and decoding functions.
pub mod encoding;

/// `eval::*` functions.
pub mod eval;

/// Geospatial functions.
pub mod geo;

/// HTTP request functions.
pub mod http;

/// Math functions and constants.
pub mod math;

/// SurrealML model calls, `ml::name<version>(…)`.
pub mod ml;

/// Surrealism module calls, `mod::name(…)`.
pub mod module;

/// Object functions.
pub mod object;

/// Parsing functions.
pub mod parse;

/// Random value functions.
pub mod rand;
pub use self::rand::rand;

/// Embedded JavaScript functions.
mod script;
pub use script::*;

/// Session functions.
pub mod session;

/// Set functions.
pub mod set;

/// `sleep()`.
pub mod sleep;

/// String functions.
pub mod string;

/// Time functions and constants.
pub mod time;

/// Type checking and conversion functions.
pub mod type_;

/// Record id functions.
pub mod record;

/// Value functions.
pub mod value;

/// Vector functions.
pub mod vector;

/// Search functions.
pub mod search;
