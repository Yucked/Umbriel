//! Calls (`orm/src/query/function.rs`): `Function::run` decodes typed results, custom functions
//! run, and method forms render and dispatch as SurrealDB 3.3.4 does.

use std::{collections::BTreeSet, time::Duration};

use umbriel::{
    ArrayLike, BytesLike, DatetimeLike, DurationLike, Field, GeometryLike, Model, NumberLike,
    ObjectLike, Operatable, Param, RecordIdLike, Render, Runnable, SetLike, StrandLike, ValueLike,
    closure, define_function,
    functions::{array, function, string, type_},
    kinds,
};
use umbriel_tests::*;

define_function!(greet(name: string) {
    return name;
});

#[tokio::test]
async fn run_returns_typed_results() {
    let db = db().await;
    let length: Option<i64> = string::len!("Alice").run(db.clone()).await.expect("ran");
    assert_eq!(length, Some(5));
    let distinct: Option<Vec<i64>> = array::distinct!(vec![1, 1, 2])
        .run(db.clone())
        .await
        .expect("ran");
    assert_eq!(distinct, Some(vec![1, 2]));
    let owner: Option<umbriel::RecordId<User>> = type_::record!("user", "alice")
        .run(db.clone())
        .await
        .expect("ran");
    assert_eq!(owner, Some(User::id("alice")));
    let nothing: Option<i64> = array::first!(Vec::<i64>::new()).run(db).await.expect("ran");
    assert_eq!(nothing, None, "`NONE` decodes as `None`");
}

#[tokio::test]
async fn custom_function_runs() {
    let db = db().await;
    greet_statement().run(db.clone()).await.expect("defined");
    let greeting: Option<String> = greet!("Alice")
        .typed::<kinds::String>()
        .run(db)
        .await
        .expect("ran");
    assert_eq!(greeting.as_deref(), Some("Alice"));
}

#[tokio::test]
async fn method_calls_render_and_run() {
    let db = db().await;
    let name = Field::new("name");
    calls!(
        (StrandLike::from(name.clone()).len(), "name.len()"),
        (
            StrandLike::from(name.clone()).slice(Some(1), None::<i64>),
            "name.slice(1)"
        ),
        (
            StrandLike::from(name).concat(["b", "c"]),
            "name.concat('b', 'c')"
        ),
        (NumberLike::from(Field::new("score")).abs(), "score.abs()"),
        (ArrayLike::from(Field::new("tags")).len(), "tags.len()"),
        (ArrayLike::from(Field::new("tags")).add(1), "tags.add(1)"),
        (SetLike::from(Field::new("tags")).len(), "tags.len()"),
        (
            BytesLike::from(Field::new("payload")).len(),
            "payload.len()"
        ),
        (
            ObjectLike::from(Field::new("settings")).keys(),
            "settings.keys()"
        ),
        (GeometryLike::from(Field::new("area")).area(), "area.area()"),
        (
            DatetimeLike::from(Field::new("created")).year(),
            "created.year()"
        ),
        (
            DurationLike::from(Field::new("elapsed")).days(),
            "elapsed.days()"
        ),
        (RecordIdLike::from(Field::new("owner")).tb(), "owner.tb()"),
        (
            ValueLike::from(Field::new("input")).type_of(),
            "input.type_of()"
        ),
        (
            ValueLike::from(Field::new("input")).to_string_value(),
            "input.to_string()"
        ),
        (
            ValueLike::from(Field::new("input")).chain(closure(["v"], |[v]| v.add(1))),
            "input.chain(|$v| $v + 1)",
        ),
        (StrandLike::from("a").len(), "('a').len()"),
        (NumberLike::from(-5).abs(), "(-5).abs()"),
    );
    holds_all!(
        &db,
        [
            (StrandLike::from("Alice").len(), "5"),
            (StrandLike::from("Alice").uppercase(), "'ALICE'"),
            (StrandLike::from(", ").join(["a", "b"]), "'a, b'"),
            (NumberLike::from(-5).abs(), "5"),
            (ArrayLike::from(vec![1, 1, 2]).distinct(), "[1, 2]"),
            (SetLike::from(BTreeSet::from([1, 2])).len(), "2"),
            (BytesLike::from(vec![1u8, 2]).len(), "2"),
            (DatetimeLike::from(datetime()).year(), "2021"),
            (
                DurationLike::from(Duration::from_secs(2 * 86_400)).days(),
                "2"
            ),
            (RecordIdLike::from(User::id("alice")).tb(), "'user'"),
            (ValueLike::from(1).type_of(), "'int'"),
            (
                ValueLike::from(2).chain(closure(["v"], |[v]| v.multiply(3))),
                "6"
            ),
        ]
    );
}

#[tokio::test]
async fn method_eligibility_follows_dispatch() {
    let db = db().await;
    holds_all!(
        &db,
        [
            (NumberLike::from(-2.5).abs(), "2.5"),
            (
                ValueLike::from("user").to_record(Some("alice")),
                "user:alice"
            ),
            (ValueLike::from(1.5).type_of(), "'float'"),
            (
                StrandLike::from("alice").similarity_smithwaterman("alicia"),
                "string::similarity::smithwaterman('alice', 'alicia')"
            ),
        ]
    );
}

#[tokio::test]
async fn an_unmapped_method_is_refused_by_surrealdb() {
    let db = db().await;
    let error = failure(&db, "RETURN 1.fixed(2);").await;
    assert!(
        error.contains("no such method found for the int type"),
        "{error}"
    );
}

#[tokio::test]
async fn a_method_on_the_wrong_runtime_type_errors() {
    let db = db().await;
    let call = StrandLike::from(Param::new("x")).uppercase();
    assert_eq!(call.to_debug_sql(), "$x.uppercase()");
    let error = failure(&db, &format!("LET $x = 5; RETURN {};", call.to_debug_sql())).await;
    assert!(
        error.contains("no such method found for the int type"),
        "{error}"
    );
}

#[test]
fn script_typed_result() {
    let script =
        function!((Field::new("score")), "{ return arguments[0] * 2; }").typed::<kinds::Int>();
    let doubled = NumberLike::from(script.clone()).to_debug_sql();
    assert_eq!(doubled, "function(score) { return arguments[0] * 2; }");
    assert_parses(&format!("RETURN {doubled};"));
    let script = function!((Param::new("value")), "{ return arguments[0] * 2; }");
    assert!(script.to_debug_sql().starts_with("function($value) "));
    assert_parses(&format!("RETURN {};", script.to_debug_sql()));
    assert_eq!(
        string::len!(StrandLike::from(Field::new("name"))).to_debug_sql(),
        "string::len(name)"
    );
}
