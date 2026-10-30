//! USE (`orm/src/statements/use_.rs`): the #12 verification of `USE DEFAULT`, which the 3.3.4
//! parser accepts (`parser/stmt/mod.rs`) but the docs do not list. A builder for it belongs to
//! #29 (Q33).

use std::collections::BTreeMap;

use surrealdb::types::Value;
use umbriel::{Raw, Runnable};
use umbriel_tests::*;

/// `USE DEFAULT` selects the namespace and database of `DEFINE CONFIG DEFAULT`; with none
/// defined it changes nothing, and like every `USE` since 3.0.0 it returns the selection it ends
/// with (3.3.4 `dbs/executor.rs`).
#[tokio::test]
async fn use_default_without_a_default_keeps_the_selection() {
    let db = db().await;
    let mut results = Raw::new("USE DEFAULT; RETURN [session::ns(), session::db()];")
        .run(db.clone())
        .await
        .expect("USE DEFAULT runs on the embedded engine");
    // `take` decodes into `Option`, `Vec` or `Value`, so a single object is taken as an `Option`.
    let selected: BTreeMap<String, Value> = results
        .take::<Option<_>>(0)
        .expect("decodes")
        .expect("an object");
    assert_eq!(
        selected.get("namespace"),
        Some(&Value::String("test".to_owned()))
    );
    assert_eq!(
        selected.get("database"),
        Some(&Value::String("test".to_owned()))
    );
    let session: Vec<String> = results.take(1).expect("the session");
    assert_eq!(session, ["test", "test"]);
}
