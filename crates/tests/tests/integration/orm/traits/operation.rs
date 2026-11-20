//! The operators added in `orm/src/traits/operation.rs`: `??`, `?:`, `%`, `!`, `!!` and unary
//! `-`, rendered and evaluated by SurrealDB, and kept in their precedence.

use std::collections::BTreeMap;

use umbriel::{
    Field, NONE, NULL, Operatable, Render, ValueLike, negate, not, truthy, types::Value,
};
use umbriel_tests::{support::query::*, *};

/// A field standing for `value`, so operators can be applied to it.
fn literal(value: impl Into<ValueLike>) -> Field {
    Field::from_expr(value.into())
}

/// Whether SurrealDB evaluates `rendered` to a value equal to the SurrealQL `expected`.
async fn equals(rendered: &str, expected: &str) {
    let (db, _guard) = db().await;
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
        assert!(
            negated.starts_with('!') && !negated.starts_with("!!"),
            "{negated}"
        );
        equals(&negated, "true").await;
    }

    let (db, _guard) = db().await;
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
    evaluates(
        literal(1).equal(literal(NONE).coalesce(1)),
        "1 = (NONE ?? 1)",
        "true",
    )
    .await;
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

mod operators {
    use super::*;

    #[test]
    fn comparison_and_logic() {
        let age = Field::new("age");
        let cases = [
            (age.equal(25), "age = 25"),
            (age.not_equal(25), "age != 25"),
            (age.neq(25), "age != 25"),
            (age.exactly_equal(25), "age == 25"),
            (age.any_equal(25), "age ?= 25"),
            (age.all_equal(25), "age *= 25"),
            (age.less_than(25), "age < 25"),
            (age.lt(25), "age < 25"),
            (age.less_than_or_equal(25), "age <= 25"),
            (age.lte(25), "age <= 25"),
            (age.greater_than(25), "age > 25"),
            (age.gt(25), "age > 25"),
            (age.greater_than_or_equal(25), "age >= 25"),
            (age.gte(25), "age >= 25"),
            (age.truthy_and(25), "age && 25"),
            (age.truthy_or(25), "age || 25"),
            (age.and(25), "age AND 25"),
            (age.or(25), "age OR 25"),
            (age.is(25), "age IS 25"),
            (age.is_not(NONE), "age IS NOT NONE"),
            (age.is(NULL), "age IS NULL"),
            (age.equal(Field::new("limit")), "age = limit"),
            (age.equal(Param::new("limit")), "age = $limit"),
        ];
        for (operation, expected) in cases {
            assert_eq!(operation.to_debug_sql(), expected);
            assert_parses(&operation.to_debug_sql());
        }
    }

    #[test]
    fn arithmetic_operators() {
        let age = Field::new("age");
        let cases = [
            (age.add(5), "age + 5"),
            (age.plus(5), "age + 5"),
            (age.subtract(5), "age - 5"),
            (age.minus(5), "age - 5"),
            (age.multiply(5), "age * 5"),
            (age.mul(5), "age * 5"),
            (age.divide(5), "age / 5"),
            (age.div(5), "age / 5"),
            (age.power(2), "age ** 2"),
            (age.pow(2), "age ** 2"),
        ];
        for (operation, expected) in cases {
            assert_eq!(operation.to_debug_sql(), expected);
        }
    }

    #[test]
    fn containment_and_geometry() {
        let tags = Field::new("tags");
        let cases = [
            (tags.contains("rust"), "tags CONTAINS 'rust'"),
            (tags.contains_not("go"), "tags CONTAINSNOT 'go'"),
            (
                tags.contains_all(vec!["rust", "sql"]),
                "tags CONTAINSALL ['rust', 'sql']",
            ),
            (
                tags.contains_any(vec!["rust", "sql"]),
                "tags CONTAINSANY ['rust', 'sql']",
            ),
            (tags.contains_none(vec!["go"]), "tags CONTAINSNONE ['go']"),
            (tags.inside(vec!["rust"]), "tags INSIDE ['rust']"),
            (tags.in_(vec!["rust"]), "tags INSIDE ['rust']"),
            (tags.not_inside(vec!["rust"]), "tags NOTINSIDE ['rust']"),
            (tags.all_inside([1, 2]), "tags ALLINSIDE [1, 2]"),
            (tags.any_inside([1, 2]), "tags ANYINSIDE [1, 2]"),
            (tags.none_inside(vec![1, 2]), "tags NONEINSIDE [1, 2]"),
        ];
        for (operation, expected) in cases {
            assert_eq!(operation.to_debug_sql(), expected);
            assert_parses(&operation.to_debug_sql());
        }

        let location = Field::new("location");
        let area = geo::Polygon::new(
            geo::LineString::from(vec![
                (0.0, 0.0),
                (2.0, 0.0),
                (2.0, 2.0),
                (0.0, 2.0),
                (0.0, 0.0),
            ]),
            vec![],
        );
        for operation in [location.outside(area.clone()), location.intersects(area)] {
            assert_parses(&operation.to_debug_sql());
            assert_eq!(operation.to_query().expect("valid").vars().len(), 1);
        }
        assert!(
            location
                .outside(geo::Point::new(1.0, 1.0))
                .to_debug_sql()
                .starts_with("location OUTSIDE ")
        );
    }

    #[test]
    fn ranges_and_full_text_matches() {
        let age = Field::new("age");
        assert_eq!(
            age.between(18, 30).to_debug_sql(),
            "(18 < age AND age < 30)"
        );
        assert_eq!(
            age.within(18, 30).to_debug_sql(),
            "(18 <= age AND age <= 30)"
        );
        assert_eq!(
            age.between(Field::new("min"), Field::new("max"))
                .to_debug_sql(),
            "(min < age AND age < max)"
        );
        assert_parses(&age.between(18, 30).to_debug_sql());
        let content = Field::new("content");
        assert_eq!(
            content.matches("hello world").to_debug_sql(),
            "content @@ 'hello world'"
        );
        let with_ref = content.matches_with_ref(1, "hello world");
        assert_eq!(with_ref.to_debug_sql(), "content @1@ 'hello world'");
        assert_eq!(
            with_ref.to_query().expect("valid").sql(),
            "content @1@ $_p0"
        );
    }

    #[tokio::test]
    async fn operators_evaluate_as_surrealdb_defines_them() {
        let (db, _guard) = db().await;
        let tags = Field::from(Param::new("tags"));
        let value = |value: &'static str| Field::from_expr(ValueLike::value(value));
        let checks: Vec<(umbriel::Operation, bool)> = vec![
            (tags.contains("rust"), true),
            (tags.contains_not("go"), true),
            (tags.contains_all(vec!["rust", "sql"]), true),
            (tags.contains_any(vec!["go", "sql"]), true),
            (tags.contains_none(vec!["go"]), true),
            (tags.any_equal("rust"), true),
            (tags.all_equal("rust"), false),
            (value("rust").inside(tags.clone()), true),
            (Field::from_expr(ValueLike::value(5)).between(1, 10), true),
            (Field::from_expr(ValueLike::value(10)).within(1, 10), true),
            (Field::from_expr(ValueLike::value(10)).between(1, 10), false),
            (
                Field::from_expr(ValueLike::from(umbriel::GeometryLike::from(
                    geo::Point::new(1.0, 1.0),
                )))
                .inside(geo::Polygon::new(
                    geo::LineString::from(vec![
                        (0.0, 0.0),
                        (2.0, 0.0),
                        (2.0, 2.0),
                        (0.0, 2.0),
                        (0.0, 0.0),
                    ]),
                    vec![],
                )),
                true,
            ),
        ];
        for (operation, expected) in checks {
            let sql = format!(
                "LET $tags = ['rust', 'sql']; RETURN {};",
                operation.to_debug_sql()
            );
            let mut response = db
                .query(sql.clone())
                .await
                .and_then(surrealdb::IndexedResults::check)
                .expect("ran");
            let result: Option<bool> = response.take(1).expect("bool");
            assert_eq!(result, Some(expected), "{sql}");
        }
    }
}
