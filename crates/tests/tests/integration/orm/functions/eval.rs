//! `eval::*` (`orm/src/functions/eval.rs`): rendering, and a server refusing them unless started
//! with `--allow-eval-query`. Running them is covered in `server.rs` (`eval_surql_runs`).

use std::collections::BTreeMap;

use umbriel::{Render, functions::eval};
use umbriel_tests::*;

#[test]
fn rendering() {
    let bindings = BTreeMap::from([("n".to_owned(), 1)]);
    calls!(
        (eval::surql!("RETURN 1"), "eval::surql('RETURN 1')"),
        (
            eval::gql!("{ user { id } }"),
            "eval::gql('{ user { id } }')"
        ),
    );
    let with_bindings = eval::surql!("RETURN $n", bindings).to_debug_sql();
    assert!(
        with_bindings.starts_with("eval::surql('RETURN $n', {"),
        "{with_bindings}"
    );
    assert_parses(&format!("RETURN {with_bindings};"));
}

#[tokio::test]
async fn eval_is_refused_without_eval_queries_allowed() {
    let (db, _guard) = restricted_db().await;
    let error = call_failure(&db, &eval::surql!("RETURN 1")).await;
    assert!(error.contains("eval::surql"), "{error}");
}
