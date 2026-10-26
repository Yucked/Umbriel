// Shared lints live in the workspace manifest; only the public crate requires docs.
#![warn(missing_docs)]
#![recursion_limit = "256"]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! This library includes augmented surrealdb types, custom types, statements, functions, operators, castings, and other utilities to
//! to make working with surrealdb a joy.

mod errors;
mod execution;

/// Contains math constants and all the casting functions.
mod data_model;
pub mod functions;
mod helpers;
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
pub use helpers::*;
pub use query::*;
pub use render::{Fragment, GENERATED_PARAM_PREFIX, Query, Render, Renderer};
pub use statements::select::CanOrder;
pub use statements::utils::*;
pub use traits::*;

/// SurrealDB's value types (`Value`, `RecordId`, `Datetime`, `Kind`, …) and the `SurrealValue`
/// conversion trait models derive.
pub use surrealdb::types;
pub use surrealdb::types::Table;

/// Embedded and file-based schema migrations, generated from model definitions.
#[cfg(feature = "migrations")]
pub mod migrations;

#[doc = include_str!("docs/model.md")]
pub use umbriel_macros::model;

pub use umbriel_macros::{
    Pickable, TableResources, block, chain, define_function, for_, pick, query, query_raw,
    transaction,
};

/// Internals exercised directly by `crates/tests`; not part of the public API.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    pub use crate::execution::{classify, select_failure};
    #[cfg(feature = "migrations")]
    pub use crate::migrations::{
        database::migration_model::history_or_empty, schema::field_definition,
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
