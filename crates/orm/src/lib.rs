// Shared lints live in the workspace manifest; only the public crate requires docs.
#![warn(missing_docs)]
#![recursion_limit = "256"]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! This library includes augmented surrealdb types, custom types, statements, functions, operators, castings, and other utilities to
//! to make working with surrealdb a joy.

mod errors;
mod execution;

/// Casts and SurrealQL constants.
mod data_model;
pub mod functions;
mod helpers;
pub mod kinds;
mod operators_macros;
mod query;
mod render;
/// This module contains the different types of statements that can be used to query the
/// database.
pub mod statements;
mod traits;
/// For compile time validations
pub mod validators;

pub use data_model::*;
pub use errors::*;
pub use execution::Executor;
pub use functions::count;
pub use helpers::*;
pub use query::*;
pub use render::{Fragment, GENERATED_PARAM_PREFIX, Query, Render, Renderer};
pub use statements::select::CanOrder;
pub use statements::utils::*;
pub use statements::{Models, WritableModels};
pub use traits::*;

/// SurrealDB's value types (`Value`, `RecordId`, `Datetime`, `Kind`, …) and the `SurrealValue`
/// conversion trait models derive.
pub use surrealdb::types;
pub use surrealdb::types::Table;

/// The SDK's client, connection, engines and options, re-exported so `umbriel` is the only
/// dependency a consumer needs to connect: `umbriel::engine::any::connect("mem://")`, a
/// `Surreal<Any>`, `opt::Config`. Engines and transports are enabled through Umbriel's features
/// of the same names (`kv-mem`, `protocol-ws`, …). Credential types are in `opt::auth`.
pub use surrealdb::{Connection, IndexedResults, Surreal, engine, opt};

/// An open SDK transaction from [`Surreal::begin`], and the per-statement statistics of
/// [`Runnable::run_with_stats`].
///
/// A [`Transaction`] is an [`Executor`]: statements and model methods run inside it. The SDK's
/// `begin` takes the client by value, so a transaction from a shared client starts from a
/// `clone()`, which is a new session. `commit` and `cancel` consume the handle and return the
/// client. A handle dropped without either commits nothing and reports nothing; its session
/// ends, and the SDK source releases the open transaction with it, which SDK 3.3.4 gives no way
/// to observe. Transactions are available on the WebSocket and embedded engines; the HTTP engine
/// refuses `begin`.
///
/// A commit conflict is [`Error::Conflict`] ([`Retry::Safe`]) over WebSocket, but on the
/// embedded engine the handle's commit error carries no structured kind, so it is
/// [`Error::Query`] with [`Retry::Uncertain`].
pub use surrealdb::method::{Stats, Transaction, WithStats};

/// Parses SurrealQL value text without a database: `parse::value("{ name: 'Alice' }")` returns a
/// [`types::Value`], or an error for text that is not one value.
#[cfg(feature = "parse")]
pub use surrealdb::parse;

/// Embedded and file-based schema migrations, generated from model definitions.
#[cfg(feature = "migrations")]
pub mod migrations;

#[doc = include_str!("docs/model.md")]
pub use umbriel_macros::model;

pub use umbriel_macros::{Models, Pickable, TableResources, pick};

/// # Example
///
/// ```
/// use umbriel::query_raw;
///
/// let query = query_raw!("SELECT name, age FROM user WHERE name = $name");
/// assert_eq!(query, "SELECT name, age FROM user WHERE name = $name");
/// ```
pub use umbriel_macros::query_raw;

/// # Examples
///
/// ```no_run
/// # async fn run(db: umbriel::Surreal<umbriel::engine::any::Any>) -> umbriel::Result<()> {
/// use umbriel::query;
///
/// let mut response = query!(db, "RETURN $name", { name: "Alice" }).await?;
/// let name: Option<String> = response.take(0)?;
///
/// // Several queries share one set of bindings.
/// let mut response = query!(
///     db,
///     [
///         "CREATE user:alice SET name = $name, skills = $skills",
///         "SELECT VALUE skills FROM ONLY user:alice"
///     ],
///     { name: "Alice", skills: vec!["Rust", "SurrealQL"] }
/// )
/// .await?;
/// let skills: Vec<String> = response.take(1)?;
/// # Ok(())
/// # }
/// ```
pub use umbriel_macros::query;

/// # Examples
///
/// ```
/// use umbriel::{All, Field, Operatable, Render, Table, chain, statements::select};
///
/// let user = &Table::from("user");
/// let query = chain! {
///     let name = "Alice";
///     select(All).from(user);
/// };
/// assert_eq!(query.to_debug_sql(), "LET $name = 'Alice';\n\nSELECT * FROM user;");
///
/// // Branches and loops are written as in Rust.
/// let age = Field::new("age");
/// let query = chain! {
///     if age.gte(18) {
///         select(All).from(Table::from("adult"));
///     } else {
///         select(All).from(Table::from("child"));
///     };
/// };
/// assert_eq!(
///     query.to_debug_sql(),
///     "IF age >= 18 \n\t{ SELECT * FROM adult; }\nELSE\n\t{ SELECT * FROM child; };"
/// );
/// ```
pub use umbriel_macros::chain;

/// # Example
///
/// ```
/// use umbriel::{All, Render, Table, block, statements::select};
///
/// let user = &Table::from("user");
/// let query = block! {
///     let name = "Alice";
///     select(All).from(user);
///     return name;
/// };
/// assert_eq!(
///     query.to_debug_sql(),
///     "{\nLET $name = 'Alice';\n\nSELECT * FROM user;\n\nRETURN $name;\n}"
/// );
/// ```
pub use umbriel_macros::block;

/// # Example
///
/// ```
/// use umbriel::{All, Render, Table, statements::select, transaction};
///
/// let user = &Table::from("user");
/// let query = transaction! {
///     BEGIN TRANSACTION;
///     select(All).from(user);
///     COMMIT TRANSACTION;
/// };
/// assert!(query.to_debug_sql().starts_with("BEGIN TRANSACTION;"));
/// ```
pub use umbriel_macros::transaction;

/// # Example
///
/// Defines `fn::double`: `double_statement()` is its `DEFINE FUNCTION`, and `double!` and
/// `double_fn` call it.
///
/// ```
/// use umbriel::*;
///
/// define_function!(double(amount: int) {
///     return amount.multiply(2);
/// });
///
/// fn main() {
///     assert_eq!(
///         double_statement().to_debug_sql(),
///         "DEFINE FUNCTION fn::double($amount: int) {\nRETURN $amount * 2;\n};"
///     );
///     assert_eq!(double!(21).to_debug_sql(), "fn::double(21)");
/// }
/// ```
pub use umbriel_macros::define_function;

/// # Example
///
/// ```
/// use umbriel::{All, Field, Operatable, Render, Table, for_, statements::select};
///
/// let person = &Table::from("person");
/// let user_name = &Field::new("user_name");
/// let for_loop = for_!((name in vec!["Alice", "Bob"]) {
///     select(All).from(person).where_(user_name.equal(&name));
/// };);
/// assert_eq!(
///     for_loop.to_debug_sql(),
///     "FOR $name IN ['Alice', 'Bob'] {\nSELECT * FROM person WHERE user_name = $name;\n};"
/// );
/// ```
pub use umbriel_macros::for_;

/// Internals exercised directly by `crates/tests`; not part of the public API.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    pub use crate::execution::{classify, select_failure};
    #[cfg(feature = "migrations")]
    pub use crate::migrations::{
        database::{
            db_runner::apply_after_reading,
            file_generator::{generate_from_schemas, init_comparison_with, remove_comparison},
            migration_model::history_or_empty,
        },
        resources::bucket_backend::credential_in_backend,
        schema::{add_table_info, database_schema, field_definition},
    };
}

#[doc(hidden)]
pub mod internal_tools {
    pub use paste::paste;
    pub use typed_builder;
}

/// Items `#[model]` expands to; not part of the public API.
#[doc(hidden)]
pub mod __private {
    pub use crate::statements::any::overlapping_members;
    pub use umbriel_macros::ModelCodegen;

    /// Implemented by `#[model]` for every field wrapper it generates: `M` is the model or object
    /// that declares the field, and `V` is the field's Rust type.
    ///
    /// Builders use it to type what a field stands for: `return_value(field)` yields `V`. The model
    /// and value are trait parameters rather than associated types because a generic model's field
    /// types name the model's parameters, which the wrapper type itself does not.
    ///
    /// The derive is its only supported implementor. A derive expands in the crate that declares
    /// the model, so this trait cannot be sealed: a hand-written implementation can claim any
    /// types, and the checks it enables then no longer hold.
    pub trait ModelField<M, V>: std::ops::Deref<Target = crate::Field> {}

    /// Glob-imported by `#[model]`: the `SurrealValue` derive names the trait unqualified, and
    /// `Value` too in the code it generates for `#[surreal(flatten)]` fields.
    pub mod surreal_value {
        pub use surrealdb::types::{SurrealValue, Value};
    }
}
