//! `eval::*` (`orm/src/functions/eval.rs`): rendering, and the embedded engine refusing them,
//! since the SDK's `Capabilities` cannot allow evaluated queries in 3.3.4. Running them needs a
//! server started with `--allow-eval-query`.

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
async fn eval_is_refused_on_the_embedded_engine() {
    let db = db().await;
    let error = call_failure(&db, &eval::surql!("RETURN 1")).await;
    assert!(error.contains("eval::surql"), "{error}");
}
