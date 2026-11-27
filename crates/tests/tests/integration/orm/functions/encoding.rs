//! `encoding::*` (`orm/src/functions/encoding.rs`): every function run, and the results
//! SurrealDB computes.

use umbriel::{
    Param, Render,
    functions::{encoding, type_},
};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (
                encoding::base64::encode!(type_::bytes("hi"), true),
                "'aGk='"
            ),
            (encoding::base64::decode("aGk="), "<bytes> 'hi'"),
            (
                encoding::base64::decode(encoding::base64::encode!(type_::bytes("hi"))),
                "<bytes> 'hi'"
            ),
            (
                encoding::json::decode(encoding::json::encode(vec![1, 2])),
                "[1, 2]"
            ),
            (
                encoding::json::decode("{\"name\":\"Alice\"}"),
                "{ name: 'Alice' }"
            ),
            (
                encoding::cbor::decode(encoding::cbor::encode(vec![1, 2])),
                "[1, 2]"
            ),
        ]
    );
}

#[test]
fn generated_bindings() {
    let nested = encoding::base64::encode!(encoding::cbor::encode(Param::new("value")));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(
        sql,
        "encoding::base64::encode(encoding::cbor::encode($value))"
    );
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = call_failure(&db, &encoding::base64::decode("not base64!")).await;
    assert!(!error.is_empty());
}
