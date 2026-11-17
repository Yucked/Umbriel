//! `api::*` (`orm/src/functions/api.rs`): `api::invoke` rendered and calling a defined API,
//! through the typed middleware of `ApiConfig`.

use surrealdb::types::{Number, Object, Value};
use umbriel::{
    Runnable, ValueLike,
    functions::api,
    statements::{ApiMethod, api_action, api_config, define_api},
};
use umbriel_tests::*;

#[test]
fn rendering() {
    let mut request = Object::new();
    request.insert("method", "post".to_owned());
    calls!(
        (api::invoke!("/status"), "api::invoke('/status')"),
        (
            api::invoke!("/status", request),
            "api::invoke('/status', { method: 'post' })"
        ),
    );
}

/// The response object of `api::invoke(path)`.
async fn invoke(db: &surrealdb::Surreal<surrealdb::engine::local::Db>, path: &str) -> Object {
    bound::<Object, _>(db, &api::invoke!(path))
        .await
        .expect("a response")
}

#[tokio::test]
async fn invoke_answers_with_the_action_and_its_middleware() {
    let db = db().await;
    let mut answer = Object::new();
    answer.insert("status", 200_i64);
    answer.insert("body", "ok".to_owned());
    define_api("/status")
        .action(
            api_action([ApiMethod::Get], ValueLike::value(answer))
                .config(api_config().res_status(201).res_header("x-acme", "Acme Corp")),
        )
        .run(&db)
        .await
        .expect("defined");
    let response = invoke(&db, "/status").await;
    assert_eq!(
        response.get("status"),
        Some(&Value::Number(Number::Int(201))),
        "{response:?}"
    );
    let headers = match response.get("headers") {
        Some(Value::Object(headers)) => headers.clone(),
        other => panic!("headers: {other:?}"),
    };
    assert_eq!(
        headers.get("x-acme"),
        Some(&Value::String("Acme Corp".to_owned())),
        "{headers:?}"
    );
}

#[tokio::test]
async fn an_undefined_path_fails() {
    let db = db().await;
    let error = call_failure(&db, &api::invoke!("/missing")).await;
    assert!(!error.is_empty());
}
