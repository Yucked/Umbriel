//! `bytes::*` and `encoding::*` (`orm/src/functions/{bytes, encoding}.rs`): every function
//! rendered, and the results SurrealDB computes.

use umbriel::{
    BytesLike, Field, Param, Render,
    functions::{bytes, encoding, type_},
};
use umbriel_tests::*;

#[test]
fn rendering() {
    let payload = Field::new("payload");
    calls!(
        (bytes::len!(payload.clone()), "bytes::len(payload)"),
        (BytesLike::from(payload.clone()).len(), "payload.len()"),
        (encoding::base64::decode!("aGk="), "encoding::base64::decode('aGk=')"),
        (encoding::base64::encode!(payload.clone()), "encoding::base64::encode(payload)"),
        (encoding::base64::encode!(payload.clone(), false), "encoding::base64::encode(payload, false)"),
        (encoding::cbor::decode!(payload.clone()), "encoding::cbor::decode(payload)"),
        (encoding::cbor::encode!(Field::new("input")), "encoding::cbor::encode(input)"),
        (encoding::json::decode!("[1]"), "encoding::json::decode('[1]')"),
        (encoding::json::encode!(Field::new("input")), "encoding::json::encode(input)"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(&db, [
        (bytes::len!(vec![1u8, 2, 3]), "3"),
        (bytes::len!(Vec::<u8>::new()), "0"),
        (BytesLike::from(vec![1u8]).len(), "1"),
        (encoding::base64::encode!(type_::bytes!("hi"), true), "'aGk='"),
        (encoding::base64::decode!("aGk="), "<bytes> 'hi'"),
        (encoding::base64::decode!(encoding::base64::encode!(type_::bytes!("hi"))), "<bytes> 'hi'"),
        (encoding::json::decode!(encoding::json::encode!(vec![1, 2])), "[1, 2]"),
        (encoding::json::decode!("{\"name\":\"Alice\"}"), "{ name: 'Alice' }"),
        (encoding::cbor::decode!(encoding::cbor::encode!(vec![1, 2])), "[1, 2]"),
    ]);
}

#[test]
fn generated_bindings() {
    let nested = encoding::base64::encode!(encoding::cbor::encode!(Param::new("value")));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "encoding::base64::encode(encoding::cbor::encode($value))");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = failure(&db, "RETURN bytes::len('text');").await;
    assert!(error.contains("bytes::len"), "{error}");
    let error = call_failure(&db, &encoding::base64::decode!("not base64!")).await;
    assert!(!error.is_empty());
}
