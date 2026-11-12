//! `parse::*` (`orm/src/functions/parse.rs`): every function rendered and the parts SurrealDB
//! extracts. None of them has a method form.

use umbriel::{Param, Render, functions::parse};
use umbriel_tests::*;

const URL: &str = "https://example.com:443/topics?arg=value#fragment";

#[test]
fn rendering() {
    calls!(
        (
            parse::email::host!("alice@example.com"),
            "parse::email::host('alice@example.com')"
        ),
        (
            parse::email::user!("alice@example.com"),
            "parse::email::user('alice@example.com')"
        ),
    );
    calls!(
        (
            parse::url::domain!(URL),
            &format!("parse::url::domain('{URL}')")
        ),
        (
            parse::url::fragment!(URL),
            &format!("parse::url::fragment('{URL}')")
        ),
        (
            parse::url::host!(URL),
            &format!("parse::url::host('{URL}')")
        ),
        (
            parse::url::path!(URL),
            &format!("parse::url::path('{URL}')")
        ),
        (
            parse::url::port!(URL),
            &format!("parse::url::port('{URL}')")
        ),
        (
            parse::url::query!(URL),
            &format!("parse::url::query('{URL}')")
        ),
        (
            parse::url::scheme!(URL),
            &format!("parse::url::scheme('{URL}')")
        ),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(
        &db,
        [
            (parse::email::host!("alice@example.com"), "'example.com'"),
            (parse::email::user!("alice@example.com"), "'alice'"),
            (parse::url::domain!(URL), "'example.com'"),
            (parse::url::fragment!(URL), "'fragment'"),
            (parse::url::host!(URL), "'example.com'"),
            (parse::url::path!(URL), "'/topics'"),
            (parse::url::port!(URL), "443"),
            (parse::url::query!(URL), "'arg=value'"),
            (parse::url::scheme!(URL), "'https'"),
            (parse::url::fragment!("https://example.com"), "NONE"),
        ]
    );
}

#[test]
fn generated_bindings() {
    let call = parse::url::host!(Param::new("link"));
    let (sql, vars) = call.to_query().expect("renders").into_parts();
    assert_eq!(sql, "parse::url::host($link)");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = failure(&db, "RETURN parse::url::host(1);").await;
    assert!(error.contains("parse::url::host"), "{error}");
}
