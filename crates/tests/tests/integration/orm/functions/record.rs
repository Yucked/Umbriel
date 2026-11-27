//! `record::*` (`orm/src/functions/record.rs`): every function and method form run, and the
//! results SurrealDB computes.

use umbriel::{Field, Model, Param, RecordIdLike, Render, functions::record};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (record::id(User::id("alice")), "'alice'"),
            (record::tb(User::id("alice")), "'user'"),
            (record::table(User::id("alice")), "'user'"),
            (record::exists(User::id("alice")), "false"),
            (RecordIdLike::from(User::id("alice")).id(), "'alice'"),
        ]
    );
    assert!(
        succeeds(
            &db,
            "CREATE user:alice; CREATE user:bob; RELATE user:alice->follows:x->user:bob;"
        )
        .await
    );
    holds_all!(
        &db,
        [
            (record::exists(User::id("alice")), "true"),
            (record::is_edge(User::id("alice")), "false"),
            (record::is_edge(Field::raw("follows:x")), "true"),
        ]
    );
}

#[test]
fn generated_bindings() {
    let call = record::tb(Param::new("id"));
    let (sql, vars) = call.to_query().expect("renders").into_parts();
    assert_eq!(sql, "record::tb($id)");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = failure(&db, "RETURN record::tb('user');").await;
    assert!(error.contains("record::tb"), "{error}");
}

/// Every mapped method runs on a record id and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    assert!(succeeds(&db, "CREATE user:alice;").await);
    let r = || RecordIdLike::from(User::id("alice"));
    same_all!(
        &db,
        [
            (r().exists(), record::exists(User::id("alice"))),
            (r().id(), record::id(User::id("alice"))),
            (r().is_edge(), record::is_edge(User::id("alice"))),
            (r().table(), record::table(User::id("alice"))),
            (r().tb(), record::tb(User::id("alice"))),
        ]
    );
}
