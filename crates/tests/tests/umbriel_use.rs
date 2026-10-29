//! USE (`orm/src/statements/use_.rs`): the #12 verification of `USE DEFAULT`, which the 3.3.4
//! parser accepts (`parser/stmt/mod.rs`) but the docs do not list. A builder for it belongs to
//! #29 (Q33).

use std::collections::BTreeMap;

use surrealdb::types::Value;
use umbriel::{Raw, Runnable};
use umbriel_tests::*;

#[tokio::test]
async fn use_default_selects_a_namespace_and_database() {
    let db = db().await;
    let mut results = Raw::new("USE DEFAULT; RETURN [session::ns(), session::db()];")
        .run(db.clone())
        .await
        .expect("USE DEFAULT runs on the embedded engine");
    // Since 3.0.0 `USE` returns the namespace and database it selected.
    let selected: BTreeMap<String, Value> = results.take(0).expect("an object");
    assert!(
        selected.contains_key("namespace") && selected.contains_key("database"),
        "{selected:?}"
    );
    let session: Vec<Value> = results.take(1).expect("the session");
    assert_eq!(session.len(), 2, "{session:?}");
}
