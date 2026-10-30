//! Parameters (`orm/src/query/param.rs`): how long a `LET` parameter lives (Q36), and the session
//! values reserved parameters stand for (Q35).

use surrealdb::types::Value;
use umbriel::{
    Param, Runnable,
    statements::{let_, return_},
};
use umbriel_tests::*;

/// A `LET` parameter lives for its own query: 3.3.4 binds a top-level `LET` into the
/// request-scoped context (`dbs/executor.rs`), and the embedded engine merges only the
/// connection's `set` values into each query (`surrealdb-engine-local` `engine.rs`).
#[tokio::test]
async fn a_let_parameter_lives_for_its_query() {
    let db = db().await;
    let_("greeting")
        .equal_to("hello")
        .run(db.clone())
        .await
        .expect("set");
    let mut results = return_(Param::new("greeting"))
        .run(db.clone())
        .await
        .expect("ran");
    let greeting: Option<String> = results.take(0).expect("a value");
    assert_eq!(greeting, None);

    db.set("greeting", "hello")
        .await
        .expect("set on the connection");
    let mut results = return_(Param::new("greeting"))
        .run(db.clone())
        .await
        .expect("ran");
    let greeting: Option<String> = results.take(0).expect("a value");
    assert_eq!(greeting.as_deref(), Some("hello"));
}

/// An embedded session that never signed in has no record access and no token: the session
/// fills `rd` and `tk` only from a signed-in record and its token (3.3.4 `dbs/session.rs`).
#[tokio::test]
async fn session_record_and_token_without_sign_in() {
    let db = db().await;
    let rd: Value = read(&db, "RETURN session::rd();").await;
    let token: Value = read(&db, "RETURN session::token();").await;
    assert_eq!(rd, Value::None);
    assert_eq!(token, Value::None);
}
