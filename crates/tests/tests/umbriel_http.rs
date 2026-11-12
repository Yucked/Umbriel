//! `http::*` (`orm/src/functions/http.rs`): rendering only, since unit tests make no network
//! requests.

use std::collections::BTreeMap;

use umbriel::{Render, functions::http};
use umbriel_tests::*;

#[test]
fn rendering() {
    calls!(
        (http::head!("https://example.com"), "http::head('https://example.com')"),
        (http::get!("https://example.com"), "http::get('https://example.com')"),
        (http::delete!("https://example.com"), "http::delete('https://example.com')"),
    );
    let body = BTreeMap::from([("name".to_owned(), "Alice".to_owned())]);
    let posted = http::post!("https://example.com", body.clone()).to_debug_sql();
    assert!(posted.starts_with("http::post('https://example.com', {"), "{posted}");
    assert!(posted.ends_with('}') || posted.ends_with("})"), "{posted}");
    assert!(!posted.contains(", { })"), "no empty headers object: {posted}");
    assert_parses(&format!("RETURN {posted};"));
    for function in [
        http::get!("https://example.com", body.clone()),
        http::put!("https://example.com", body.clone()),
        http::patch!("https://example.com", body.clone(), body),
    ] {
        assert_parses(&format!("RETURN {};", function.to_debug_sql()));
    }
}
