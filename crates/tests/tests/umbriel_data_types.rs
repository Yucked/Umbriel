//! Typed expressions (`orm/src/query/data_types.rs`): sets and bytes bind as their own kinds, and
//! nested calls and closures bind their values once each, in order.

use std::collections::{BTreeSet, HashSet};

use umbriel::{
    BytesLike, Field, Operatable, Param, Render, SetLike, closure,
    functions::{array, string},
    types::{SurrealValue, Value},
};
use umbriel_tests::*;

/// Whether the value bound for `expression` is of `kind` on the engine (`type::is_<kind>`).
async fn binds_as(expression: impl Render, kind: &str) -> bool {
    let db = db().await;
    let (sql, vars) = expression.to_query().expect("renders").into_parts();
    db.query(format!("RETURN type::is_{kind}({sql});"))
        .bind(vars)
        .await
        .expect("runs")
        .take::<Option<bool>>(0)
        .expect("a boolean")
        .expect("not NONE")
}

#[tokio::test]
async fn sets_and_bytes_bind_as_their_kind() {
    let hash: HashSet<i64> = HashSet::from([1, 2]);
    let tree: BTreeSet<&str> = BTreeSet::from(["a", "b"]);
    assert!(binds_as(SetLike::from(hash), "set").await);
    assert!(binds_as(SetLike::from(tree), "set").await);
    assert!(binds_as(BytesLike::from(vec![1u8, 2, 3]), "bytes").await);
    assert!(binds_as(BytesLike::from(&[1u8, 2][..]), "bytes").await);
    assert!(!binds_as(umbriel::ArrayLike::from(vec![1u8, 2]), "bytes").await);

    let db = db().await;
    let length: Option<i64> = bound(&db, &umbriel::functions::bytes::len!(vec![1u8, 2, 3])).await;
    assert_eq!(length, Some(3));
    let size: Option<i64> =
        bound(&db, &umbriel::functions::set::len!(BTreeSet::from([1, 2, 2]))).await;
    assert_eq!(size, Some(2));
}

#[test]
fn generated_bindings_are_ordered_through_nested_calls() {
    let nested = string::len!(string::concat!(Param::new("a"), "x"));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "string::len(string::concat($a, $_p0))");
    assert_eq!(vars.len(), 1);
    assert_eq!(vars.get("_p0"), Some(&Value::String("x".to_owned())));

    let mapped = array::map!(
        vec![1, 2],
        closure(["n"], |[n]| n.multiply(10).add(Field::new("offset")))
    );
    let (sql, vars) = mapped.to_query().expect("renders").into_parts();
    assert_eq!(sql, "array::map($_p0, |$n| $n * $_p1 + offset)");
    assert_eq!(vars.len(), 2);
    assert_eq!(vars.get("_p0"), Some(&vec![1i64, 2].into_value()));
    assert_eq!(vars.get("_p1"), Some(&10i64.into_value()));
}
