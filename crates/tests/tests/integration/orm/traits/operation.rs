//! The operators added in `orm/src/traits/operation.rs`: `??`, `?:`, `%`, `!`, `!!` and unary
//! `-`, rendered and evaluated by SurrealDB, and kept in their precedence.

use std::collections::BTreeMap;

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{
    Field, Match, NONE, NULL, Operatable, Render, ValueLike, negate, not, statements::Distance,
    truthy, types::Value,
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
    evaluates(
        negate(literal(2).add(3)).multiply(2),
        "(-(2 + 3)) * 2",
        "-10",
    )
    .await;
}

/// Named arithmetic groups a nested operation the way SurrealDB would read it, as the `std::ops`
/// operators do.
#[tokio::test]
async fn named_arithmetic_preserves_grouping() {
    evaluates(literal(2).add(3).multiply(2), "(2 + 3) * 2", "10").await;
    evaluates(literal(10).subtract(4).divide(2), "(10 - 4) / 2", "3").await;
    evaluates(literal(1).add(2).power(2), "(1 + 2) ** 2", "9").await;
    evaluates(literal(2).multiply(literal(3).add(1)), "2 * (3 + 1)", "8").await;
    evaluates(
        literal(20).divide(literal(2).multiply(5)),
        "20 / (2 * 5)",
        "2",
    )
    .await;
    evaluates(literal(7).add(3).modulo(4), "(7 + 3) % 4", "2").await;
    // A comparison over arithmetic keeps its operands as they are.
    evaluates(literal(2).add(3).equal(5), "2 + 3 = 5", "true").await;

    // The named methods and the Rust operators write the same text.
    let age = Field::new("age");
    assert_eq!(
        age.add(1).multiply(2).to_debug_sql(),
        ((age.clone() + 1) * 2).to_debug_sql()
    );
    assert_eq!(
        age.subtract(1).divide(2).to_debug_sql(),
        ((&age - 1) / 2).to_debug_sql()
    );

    // Bindings keep the order the operands were written in.
    let query = Field::new("price")
        .add(Field::new("tax"))
        .multiply(ValueLike::value(3))
        .to_query()
        .expect("valid");
    assert_eq!(query.sql(), "(price + tax) * $_p0");
}

mod operators {
    use super::*;

    #[test]
    fn comparison_and_logic() {
        let age = Field::new("age");
        let cases = [
            (age.equal(25), "age = 25"),
            (age.not_equal(25), "age != 25"),
            (age.exactly_equal(25), "age == 25"),
            (age.any_equal(25), "age ?= 25"),
            (age.all_equal(25), "age *= 25"),
            (age.less_than(25), "age < 25"),
            (age.less_than_or_equal(25), "age <= 25"),
            (age.greater_than(25), "age > 25"),
            (age.greater_than_or_equal(25), "age >= 25"),
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
            (age.subtract(5), "age - 5"),
            (age.multiply(5), "age * 5"),
            (age.divide(5), "age / 5"),
            (age.power(2), "age ** 2"),
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
        let with_ref = Field::new("content").matches_with(Match::reference(1), "hello world");
        assert_eq!(
            with_ref.to_query().expect("valid").sql(),
            "content @1@ $_p0"
        );
    }

    #[test]
    fn matches_forms_render_each_reference_and_boolean_combination() {
        let content = Field::new("content");
        let cases = [
            (content.matches("a b"), "content @@ 'a b'"),
            (
                content.matches_with(Match::default(), "a b"),
                "content @@ 'a b'",
            ),
            (
                content.matches_with(Match::reference(1), "a b"),
                "content @1@ 'a b'",
            ),
            (content.matches_with(Match::OR, "a b"), "content @OR@ 'a b'"),
            (
                content.matches_with(Match::reference(1).or(), "a b"),
                "content @1,OR@ 'a b'",
            ),
        ];
        for (operation, expected) in cases {
            assert_eq!(operation.to_debug_sql(), expected);
            assert_parses(&format!("SELECT * FROM post WHERE {expected};"));
        }
    }

    #[test]
    fn knn_forms_render() {
        let embedding = Field::new("embedding");
        let cases = [
            (embedding.knn_ef(2, 40, [1, 2]), "embedding <|2,40|> [1, 2]"),
            (
                embedding.knn_metric(2, Distance::Euclidean, [1, 2]),
                "embedding <|2,EUCLIDEAN|> [1, 2]",
            ),
            (
                embedding.knn_metric(2, Distance::Minkowski(3.0), [1, 2]),
                "embedding <|2,MINKOWSKI 3|> [1, 2]",
            ),
        ];
        for (operation, expected) in cases {
            assert_eq!(operation.to_debug_sql(), expected);
            assert_parses(&format!("SELECT * FROM doc WHERE {expected};"));
        }
        assert!(
            embedding
                .knn_metric(2, Distance::Minkowski(f64::NAN), [1, 2])
                .to_query()
                .is_err()
        );
    }

    /// Two notes, one holding both terms and one holding only `world`, under a full-text index.
    async fn notes(db: &Surreal<Any>) {
        let setup = "
            DEFINE ANALYZER words TOKENIZERS blank FILTERS lowercase;
            DEFINE INDEX note_text ON note FIELDS text FULLTEXT ANALYZER words BM25 HIGHLIGHTS;
            CREATE note:both SET text = 'hello world';
            CREATE note:one SET text = 'world';
        ";
        assert!(succeeds(db, setup).await, "the notes are defined");
    }

    /// The ids of the notes matching `condition`, in order.
    async fn matching(db: &Surreal<Any>, condition: &umbriel::Operation) -> Vec<String> {
        read(
            db,
            &format!(
                "RETURN (SELECT VALUE record::id(id) FROM note WHERE {} ORDER BY id);",
                condition.to_debug_sql()
            ),
        )
        .await
    }

    #[tokio::test]
    async fn or_matches_any_term_and_and_matches_all() {
        let (db, _guard) = db().await;
        notes(&db).await;
        let text = Field::new("text");
        assert_eq!(matching(&db, &text.matches("hello world")).await, ["both"]);
        assert_eq!(
            matching(&db, &text.matches_with(Match::OR, "hello world")).await,
            ["both", "one"]
        );
    }

    #[tokio::test]
    async fn a_referenced_or_match_scores_and_highlights() {
        let (db, _guard) = db().await;
        notes(&db).await;
        let condition = Field::new("text")
            .matches_with(Match::reference(1).or(), "hello world")
            .to_debug_sql();
        let selected = |call: String| {
            format!("RETURN (SELECT VALUE {call} FROM note WHERE {condition} ORDER BY id);")
        };
        let highlights: Vec<String> = read(
            &db,
            &selected(umbriel::functions::search::highlight!("<b>", "</b>", 1).to_debug_sql()),
        )
        .await;
        assert_eq!(highlights, ["<b>hello</b> <b>world</b>", "<b>world</b>"]);
        // BM25 scores a term found in every note at or below zero, so only their presence is
        // checked.
        let scores: Vec<f64> = read(
            &db,
            &selected(umbriel::functions::search::score!(1).to_debug_sql()),
        )
        .await;
        assert_eq!(scores.len(), 2, "{scores:?}");
    }

    #[tokio::test]
    async fn knn_brute_force_ranks_by_the_metric() {
        let (db, _guard) = db().await;
        let setup = "
            CREATE doc:near SET embedding = [1, 1];
            CREATE doc:middle SET embedding = [3, 3];
            CREATE doc:far SET embedding = [9, 9];
        ";
        assert!(succeeds(&db, setup).await, "the documents are created");
        let condition = Field::new("embedding").knn_metric(2, Distance::Euclidean, [0, 0]);
        let found: Vec<String> = read(
            &db,
            &format!(
                "RETURN (SELECT VALUE record::id(id) FROM doc WHERE {} ORDER BY id);",
                condition.to_debug_sql()
            ),
        )
        .await;
        assert_eq!(found, ["middle", "near"]);
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
