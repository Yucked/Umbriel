//! The names the query-building test files share. They import it with
//! `use umbriel_tests::support::query::*;`, so each file uses only what it needs.

pub use std::time::Duration;

pub use surrealdb::types::{self, SurrealValue, Value};
pub use umbriel::{
    AliasName, Aliasable, All, CrudType, Database, E, EdgeDirection, Event, Field, Fragment,
    GENERATED_PARAM_PREFIX, Ident, Interval, JwtAlgorithm, Last, Model, NONE, NULL, Namespace,
    Node, NumberLike, Operatable, Param, ParamNameError, Raw, Render, Runnable, SchemaGetter,
    Table, TableIndex, ValueLike, any_other_edges, arr, bracket, chain, cond, index,
    statements::{let_, return_, select, select_value},
    this, value, where_,
};
