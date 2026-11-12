//! `record::*` (`orm/src/functions/record.rs`): every function and method form rendered, and the
//! results SurrealDB computes.

use umbriel::{Field, Model, Param, RecordIdLike, Render, functions::record};
use umbriel_tests::*;

#[test]
fn rendering() {
    calls!(
        (record::exists!(User::id("alice")), "record::exists(user:alice)"),
        (record::id!(User::id("alice")), "record::id(user:alice)"),
        (record::is_edge!(Field::new("id")), "record::is_edge(id)"),
        (record::table!(Field::new("id")), "record::table(id)"),
        (record::tb!(Field::new("id")), "record::tb(id)"),
    );
    let owner = RecordIdLike::from(Field::new("owner"));
    calls!(
        (owner.exists(), "owner.exists()"),
        (owner.id(), "owner.id()"),
        (owner.is_edge(), "owner.is_edge()"),
        (owner.table(), "owner.table()"),
        (owner.tb(), "owner.tb()"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(&db, [
        (record::id!(User::id("alice")), "'alice'"),
        (record::tb!(User::id("alice")), "'user'"),
        (record::table!(User::id("alice")), "'user'"),
        (record::exists!(User::id("alice")), "false"),
        (RecordIdLike::from(User::id("alice")).id(), "'alice'"),
    ]);
    assert!(succeeds(&db, "CREATE user:alice; CREATE user:bob; RELATE user:alice->follows:x->user:bob;").await);
    holds_all!(&db, [
        (record::exists!(User::id("alice")), "true"),
        (record::is_edge!(User::id("alice")), "false"),
        (record::is_edge!(Field::raw("follows:x")), "true"),
    ]);
}

#[test]
fn generated_bindings() {
    let call = record::tb!(Param::new("id"));
    let (sql, vars) = call.to_query().expect("renders").into_parts();
    assert_eq!(sql, "record::tb($id)");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = failure(&db, "RETURN record::tb('user');").await;
    assert!(error.contains("record::tb"), "{error}");
}
