//! The operators added in `orm/src/traits/operation.rs`: `??`, `?:`, `%`, `!`, `!!` and unary
//! `-`, rendered and evaluated by SurrealDB, and kept in their precedence.

use std::collections::BTreeMap;

use umbriel::{
    Field, NONE, NULL, Operatable, Render, ValueLike, negate, not, truthy, types::Value,
};
use umbriel_tests::*;

/// A field standing for `value`, so operators can be applied to it.
fn literal(value: impl Into<ValueLike>) -> Field {
    Field::from_expr(value.into())
}

/// Whether SurrealDB evaluates `rendered` to a value equal to the SurrealQL `expected`.
async fn equals(rendered: &str, expected: &str) {
    let db = db().await;
    let equal: bool = read(&db, &format!("RETURN ({rendered}) = {expected};")).await;
    if !equal {
        let actual: Value = read(&db, &format!("RETURN {rendered};")).await;
        panic!("`{rendered}` returned {actual:?}, expected {expected}");
    }
}

/// Asserts the debug SQL of `expression`, that it parses, and its value.
async fn evaluates(expression: impl Render, rendered: &str, expected: &str) {
    assert_eq!(expression.to_debug_sql(), rendered);
    assert_parses(&format!("RETURN {rendered};"));
    equals(rendered, expected).await;
}

#[tokio::test]
async fn new_operators_render_and_evaluate() {
    evaluates(literal(NONE).coalesce("x"), "(NONE ?? 'x')", "'x'").await;
    evaluates(literal(NULL).coalesce("x"), "(NULL ?? 'x')", "'x'").await;
    evaluates(literal(0).coalesce("x"), "(0 ?? 'x')", "0").await;
    evaluates(literal(0).or_else_truthy("x"), "(0 ?: 'x')", "'x'").await;
    evaluates(literal("").or_else_truthy("x"), "('' ?: 'x')", "'x'").await;
    evaluates(literal("a").or_else_truthy("x"), "('a' ?: 'x')", "'a'").await;
    evaluates(literal(7).modulo(3), "7 % 3", "1").await;
    evaluates(literal(-7).modulo(3), "-7 % 3", "-1").await;
    evaluates(literal(7).modulo(-3), "7 % -3", "1").await;
    evaluates(truthy(literal(1)), "!!1", "true").await;
    evaluates(negate(literal(5)), "-5", "-5").await;

    let falsy = [
        ValueLike::from(0),
        ValueLike::from(""),
        ValueLike::from(Vec::<i64>::new()),
        ValueLike::from(BTreeMap::<String, i64>::new()),
        ValueLike::from(NONE),
        ValueLike::from(NULL),
        ValueLike::from(false),
    ];
    for value in falsy {
        let as_bool = truthy(literal(value.clone())).to_debug_sql();
        assert!(as_bool.starts_with("!!"), "{as_bool}");
        equals(&as_bool, "false").await;
        let negated = not(literal(value)).to_debug_sql();
        assert!(negated.starts_with('!') && !negated.starts_with("!!"), "{negated}");
        equals(&negated, "true").await;
    }

    let db = db().await;
    assert!(succeeds(&db, "CREATE account:alice SET balance = 10;").await);
    let negated: i64 = read(
        &db,
        &format!(
            "RETURN (SELECT VALUE {} FROM ONLY account:alice);",
            negate(Field::new("balance")).to_debug_sql()
        ),
    )
    .await;
    assert_eq!(negated, -10);
}

#[tokio::test]
async fn precedence_is_kept() {
    evaluates(literal(NONE).coalesce(2).add(1), "(NONE ?? 2) + 1", "3").await;
    evaluates(literal(1).equal(literal(NONE).coalesce(1)), "1 = (NONE ?? 1)", "true").await;
    evaluates(
        literal(1).or_else_truthy(literal(2).add(3)),
        "(1 ?: (2 + 3))",
        "1",
    )
    .await;
    evaluates(negate(literal(2).add(3)), "-(2 + 3)", "-5").await;
    evaluates(not(literal(1).equal(2)), "!(1 = 2)", "true").await;
    evaluates(negate(literal(2).add(3)).multiply(2), "-(2 + 3) * 2", "-10").await;
}
