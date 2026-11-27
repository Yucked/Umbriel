//! `bytes::*` (`orm/src/functions/bytes.rs`): every function and method form run, and the
//! results SurrealDB computes.

use umbriel::{BytesLike, functions::bytes};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (bytes::len(vec![1u8, 2, 3]), "3"),
            (bytes::len(Vec::<u8>::new()), "0"),
            (BytesLike::from(vec![1u8]).len(), "1"),
        ]
    );
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = failure(&db, "RETURN bytes::len('text');").await;
    assert!(error.contains("bytes::len"), "{error}");
}

/// The mapped method runs on bytes and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    same(
        &db,
        &BytesLike::from(vec![1u8, 2, 3]).len(),
        &bytes::len(vec![1u8, 2, 3]),
    )
    .await;
}
