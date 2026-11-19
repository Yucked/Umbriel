//! USE (`orm/src/statements/use_.rs`): the typed `UseResult` of a `USE` in a chain (#29, Q33),
//! the selection lasting for its request only, and the #12 verification of `USE DEFAULT`, which
//! the 3.3.4 parser accepts (`parser/stmt/mod.rs`) but the docs do not list. `USE DEFAULT` has no
//! builder; #29 deferred it.

use std::collections::BTreeMap;

use surrealdb::types::Value;
use umbriel::{
    Raw, Runnable, chain,
    statements::{UseResult, use_},
};
use umbriel_tests::*;

#[tokio::test]
async fn each_use_returns_its_selection_for_the_request_only() {
    let (db, guard) = db().await;
    // The `USE` selects a namespace of the test's own, so nothing outside it is touched.
    let (_root, scratch) = namespace().await;
    let ns = scratch.namespace();
    let mut results = chain(use_().namespace(ns).database("sales"))
        .chain(use_().database("support"))
        .run(&db)
        .await
        .expect("selected");

    let both: UseResult = results
        .take::<Option<UseResult>>(0)
        .expect("decodes")
        .expect("a selection");
    assert_eq!(both.namespace.as_deref(), Some(ns));
    assert_eq!(both.database.as_deref(), Some("sales"));
    // A database alone keeps the namespace, and the result says so.
    let database_only: UseResult = results
        .take::<Option<UseResult>>(1)
        .expect("decodes")
        .expect("a selection");
    assert_eq!(database_only.namespace.as_deref(), Some(ns));
    assert_eq!(database_only.database.as_deref(), Some("support"));

    // The handle's own session is unchanged: SurrealDB keeps a `USE` statement's selection in the
    // request's options and never writes it back (`dbs/executor.rs`).
    let session: Vec<String> = read(&db, "RETURN [session::ns(), session::db()];").await;
    assert_eq!(session, [guard.namespace(), guard.database()]);
}

/// `USE DEFAULT` selects the namespace and database of `DEFINE CONFIG DEFAULT` (a 3.x server
/// starts with one), and like every `USE` since 3.0.0 it returns the selection it ends with
/// (3.3.4 `dbs/executor.rs`), for its request only. The server's default config is shared state,
/// so the test reads it rather than defining one.
#[tokio::test]
async fn use_default_returns_the_selection_it_ends_with() {
    let (db, guard) = db().await;
    let mut results = Raw::new("USE DEFAULT; RETURN [session::ns(), session::db()];")
        .run(&db)
        .await
        .expect("USE DEFAULT runs");
    // `take` decodes into `Option`, `Vec` or `Value`, so a single object is taken as an `Option`.
    let selected: BTreeMap<String, Value> = results
        .take::<Option<_>>(0)
        .expect("decodes")
        .expect("an object");
    let session: Vec<String> = results.take(1).expect("the session");
    let [namespace, database] = [&session[0], &session[1]].map(|name| Value::String(name.clone()));
    assert_eq!(selected.get("namespace"), Some(&namespace));
    assert_eq!(selected.get("database"), Some(&database));

    let after: Vec<String> = read(&db, "RETURN [session::ns(), session::db()];").await;
    assert_eq!(after, [guard.namespace(), guard.database()]);
}
