//! What the test files of the exported macros share: the names they import and their fixtures.
//! They import it with `use umbriel_tests::support::macros::*;`, so each file uses only what it
//! needs.

pub use surrealdb::{Surreal, engine::any::Any};
pub use umbriel::{
    All, E, Field, Model, NONE, Operatable, Param, Pickable, Render, Runnable, SchemaGetter,
    SetterAssignable, SetterNumeric, Table, block, chain, define_function, for_,
    functions::math,
    object, object_partial, pick, query, query_raw,
    statements::{
        CreateTarget, Returning, create, create_only, insert, return_, select, select_value, update,
    },
    transaction,
};

use crate::*;

pub async fn ships(db: &Surreal<Any>) -> Vec<String> {
    read(db, "SELECT VALUE name FROM space_ship ORDER BY name;").await
}
