//! SurrealML calls (`orm/src/functions/ml.rs`, `MlVersion` in `orm/src/query/function.rs`):
//! rendering, version parsing, path checks, and SurrealDB refusing the call without its `ml`
//! feature. Running a model is not covered: it needs SurrealDB's `ml` feature and a `.surml`
//! fixture, neither of which this crate has.

use umbriel::{Field, MlVersion, Render, functions::ml, kinds};
use umbriel_tests::*;

fn version() -> MlVersion {
    MlVersion::new(1, 0, 0)
}

#[test]
fn calls_render() {
    let call = ml::call::<kinds::Float, _>(
        "insurance::prediction",
        version(),
        [Field::new("age"), Field::new("income")],
    );
    assert_eq!(call.to_debug_sql(), "ml::insurance::prediction<1.0.0>(age, income)");
    assert_parses(&format!("RETURN {};", call.to_debug_sql()));
    let bound = ml::call::<kinds::Any, _>("insurance", MlVersion::new(2, 10, 3), [42]);
    let query = bound.to_query().expect("renders");
    assert_eq!(query.sql(), "ml::insurance<2.10.3>($_p0)");
    assert_eq!("2.10.3".parse::<MlVersion>(), Ok(MlVersion::new(2, 10, 3)));
    assert_eq!(MlVersion::new(2, 10, 3).to_string(), "2.10.3");
}

#[test]
fn malformed_versions_are_refused() {
    for text in ["1.0", "1.0.0.0", "a.b.c", "-1.0.0", "1.0.0-beta", "", "1..0", "+1.0.0", "4294967296.0.0"] {
        let error = text.parse::<MlVersion>().expect_err(text);
        assert!(error.to_string().contains(&format!("`{text}`")), "{error}");
    }
}

#[test]
fn bad_paths_are_render_errors() {
    for path in ["", "a::", "::a", "a::1b", "a;b", "a b"] {
        let call = ml::call::<kinds::Any, _>(path, version(), [1]);
        assert_render_error(&call, "invalid function name");
    }
}

#[tokio::test]
async fn calls_without_the_feature_are_refused() {
    let db = db().await;
    let call = ml::call::<kinds::Float, _>("insurance::prediction", version(), [18]);
    let error = call_failure(&db, &call).await;
    assert!(!error.is_empty());
    println!("SurrealDB refuses `ml::` without its `ml` feature: {error}");
}
