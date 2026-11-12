//! Shared test support: the fixture models, an in-memory database and SurrealQL assertions.

mod models;

pub mod migrations;
pub mod migrator_support;

pub use models::*;

use surrealdb::{
    Surreal,
    engine::local::{Db, Mem},
    types::{SurrealValue, Value},
};
use umbriel::{Function, Model, Raw, Render, Runnable, kinds::KindOf};

/// Asserts a call's SurrealQL and that SurrealDB's parser accepts it.
#[track_caller]
pub fn call<K: KindOf>(function: &Function<K>, expected: &str) {
    pretty_assertions::assert_eq!(function.to_debug_sql(), expected);
    assert_parses(&format!("RETURN {expected};"));
}

/// Asserts that SurrealDB evaluates the call to a value equal to the SurrealQL `expected`.
pub async fn holds<K: KindOf>(db: &Surreal<Db>, function: &Function<K>, expected: &str) {
    let call = function.to_debug_sql();
    let equal: bool = read(db, &format!("RETURN {call} = {expected};")).await;
    if !equal {
        let actual: Value = read(db, &format!("RETURN {call};")).await;
        panic!("`{call}` returned {actual:?}, expected {expected}");
    }
}

/// Asserts that SurrealDB evaluates the call to a value of the given type, e.g. `float`.
pub async fn is_a<K: KindOf>(db: &Surreal<Db>, function: &Function<K>, kind: &str) {
    let call = function.to_debug_sql();
    let matches: bool = read(db, &format!("RETURN type::is_{kind}({call});")).await;
    assert!(matches, "`{call}` is not a {kind}");
}

/// The error SurrealDB gives for the call, as text. Panics when it succeeds.
pub async fn call_failure<K: KindOf>(db: &Surreal<Db>, function: &Function<K>) -> String {
    failure(db, &format!("RETURN {};", function.to_debug_sql())).await
}

/// Runs the call with its values bound, through [`Function::run`], and decodes the result.
pub async fn bound<T: SurrealValue, K: KindOf>(db: &Surreal<Db>, function: &Function<K>) -> Option<T> {
    function.run(db.clone()).await.expect("ran")
}

/// 2021-11-01T08:30:17Z, a Monday.
pub fn datetime() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_635_755_417, 0).expect("valid timestamp")
}

/// `holds` for each `(call, expected)` pair, which may be calls of different kinds.
#[macro_export]
macro_rules! holds_all {
    ($db:expr, [$(($function:expr, $expected:expr $(,)?)),* $(,)?]) => {{
        $($crate::holds($db, &$function, $expected).await;)*
    }};
}

/// `is_a` for each `(call, kind)` pair, which may be calls of different kinds.
#[macro_export]
macro_rules! is_each {
    ($db:expr, [$(($function:expr, $kind:expr $(,)?)),* $(,)?]) => {{
        $($crate::is_a($db, &$function, $kind).await;)*
    }};
}

/// `call` for each `(call, expected)` pair, which may be calls of different kinds.
#[macro_export]
macro_rules! calls {
    ($(($function:expr, $expected:expr $(,)?)),* $(,)?) => {{
        $($crate::call(&$function, $expected);)*
    }};
}

/// Definitions joined one per line.
pub fn joined(definitions: &[Raw]) -> String {
    definitions
        .iter()
        .map(Raw::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Defines `M`'s table and fields on `db`. Panics when SurrealDB refuses a definition.
pub async fn apply_definitions<M: Model>(db: &Surreal<Db>) {
    for definition in std::iter::once(M::define_table()).chain(M::define_fields()) {
        definition
            .run(db.clone())
            .await
            .unwrap_or_else(|error| panic!("`{definition}` is accepted: {error}"));
    }
}

/// Asserts a statement's debug SQL, and that both it and its bound form parse as SurrealQL with
/// SurrealDB's own parser, so syntax SurrealDB no longer accepts fails even without a database.
#[track_caller]
pub fn assert_sql(statement: &impl Render, expected: &str) {
    let debug = statement.to_debug_sql();
    pretty_assertions::assert_eq!(debug, expected);
    assert_parses(&debug);
    let query = statement
        .to_query()
        .unwrap_or_else(|error| panic!("`{debug}` renders: {error}"));
    assert_parses(query.sql());
}

/// Asserts that `sql` parses as SurrealQL.
#[track_caller]
pub fn assert_parses(sql: &str) {
    if let Err(error) = surrealdb_syn::parse(sql) {
        panic!("`{sql}` is not valid SurrealQL: {error}");
    }
}

/// Asserts that rendering `statement` fails with an error containing `message`.
#[track_caller]
pub fn assert_render_error(statement: &impl Render, message: &str) {
    match statement.to_query() {
        Ok(query) => panic!("`{}` rendered, expected an error", query.sql()),
        Err(error) => assert!(
            error.to_string().contains(message),
            "`{error}` does not mention `{message}`"
        ),
    }
}

/// The failed statement's index, its error and the number of other failed statements of a run
/// expected to fail.
#[track_caller]
pub fn statement_error(
    result: umbriel::Result<surrealdb::IndexedResults>,
) -> (usize, surrealdb::Error, usize) {
    match result {
        Err(umbriel::Error::Statement {
            index,
            source,
            related,
        }) => (index, *source, related.len()),
        Err(other) => panic!("expected a statement error, got {other}"),
        Ok(_) => panic!("expected a statement error, got success"),
    }
}

/// A fresh in-memory database, with a namespace and database selected.
pub async fn db() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.expect("in-memory engine");
    db.use_ns("test")
        .use_db("test")
        .await
        .expect("namespace and database");
    db
}

/// Runs `sql` and decodes the result of its first statement. Used to check what a test stored
/// independently of the API under test.
pub async fn read<T: SurrealValue>(db: &Surreal<impl surrealdb::Connection>, sql: &str) -> T {
    let value: Value = db
        .query(sql.to_owned())
        .await
        .and_then(surrealdb::IndexedResults::check)
        .expect("query succeeds")
        .take(0)
        .expect("a result");
    T::from_value(value).expect("value decodes")
}

/// The `DEFINE` statement that `info` (e.g. `INFO FOR DB;`) lists for `name` under `kind`
/// (e.g. `tables`). Panics when the shape differs or the definition is missing.
pub async fn stored_definition(
    db: &Surreal<impl surrealdb::Connection>,
    info: &str,
    kind: &str,
    name: &str,
) -> String {
    let Value::Object(info) = read::<Value>(db, info).await else {
        panic!("`{info}` returns an object");
    };
    let Some(Value::Object(definitions)) = info.get(kind) else {
        panic!("`{kind}` lists definitions");
    };
    match definitions.get(name) {
        Some(Value::String(sql)) => sql.clone(),
        other => panic!("`{name}` is not defined: {other:?}"),
    }
}

/// Runs `sql` and reports whether every statement in it succeeded. Used where a test checks only
/// that SurrealDB accepts or refuses something.
pub async fn succeeds(db: &Surreal<impl surrealdb::Connection>, sql: &str) -> bool {
    db.query(sql.to_owned())
        .await
        .and_then(surrealdb::IndexedResults::check)
        .is_ok()
}

/// The error SurrealDB gives for `sql`, as text. Panics when every statement succeeds, so a test
/// can assert why a statement is refused rather than only that it is.
pub async fn failure(db: &Surreal<impl surrealdb::Connection>, sql: &str) -> String {
    match db
        .query(sql.to_owned())
        .await
        .and_then(surrealdb::IndexedResults::check)
    {
        Ok(_) => panic!("`{sql}` succeeded, expected an error"),
        Err(error) => error.to_string(),
    }
}
