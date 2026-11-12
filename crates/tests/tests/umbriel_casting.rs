//! Casts (`orm/src/data_model/casting.rs`): `cast::<K>` renders `<kind> value` for every marker,
//! `cast_dynamic` for kinds no marker names, and SurrealDB converts the values.

use umbriel::{
    Field, NumberLike, Operatable, Render, Runnable, StrandLike, cast, cast_dynamic, kinds,
    types::{Kind, KindLiteral},
};
use umbriel_tests::*;

#[test]
fn typed_casts_render() {
    let value = Field::new("input");
    let cases = [
        (cast::<kinds::Bool>("true").to_debug_sql(), "<bool> 'true'"),
        (cast::<kinds::Int>("5").to_debug_sql(), "<int> '5'"),
        (cast::<kinds::Float>("1.5").to_debug_sql(), "<float> '1.5'"),
        (
            cast::<kinds::Decimal>("1.5").to_debug_sql(),
            "<decimal> '1.5'",
        ),
        (cast::<kinds::Number>("5").to_debug_sql(), "<number> '5'"),
        (cast::<kinds::String>(5).to_debug_sql(), "<string> 5"),
        (
            cast::<kinds::Duration>("1h").to_debug_sql(),
            "<duration> '1h'",
        ),
        (
            cast::<kinds::Datetime>("2020-01-01T00:00:00Z").to_debug_sql(),
            "<datetime> '2020-01-01T00:00:00Z'",
        ),
        (
            cast::<kinds::Uuid>(value.clone()).to_debug_sql(),
            "<uuid> input",
        ),
        (
            cast::<kinds::Bytes>(value.clone()).to_debug_sql(),
            "<bytes> input",
        ),
        (
            cast::<kinds::Point>(value.clone()).to_debug_sql(),
            "<geometry<point>> input",
        ),
        (
            cast::<kinds::Record<User>>(value.clone()).to_debug_sql(),
            "<record<user>> input",
        ),
        (
            cast::<kinds::Array<kinds::Record<User>>>(value.clone()).to_debug_sql(),
            "<array<record<user>>> input",
        ),
        (
            cast::<kinds::Set<kinds::String>>(value.clone()).to_debug_sql(),
            "<set<string>> input",
        ),
        (
            cast::<kinds::Int>(value.add(1)).to_debug_sql(),
            "<int> (input + 1)",
        ),
    ];
    for (rendered, expected) in cases {
        assert_eq!(rendered, expected);
        assert_parses(&format!("RETURN {expected};"));
    }
}

#[test]
fn dynamic_casts_render() {
    let value = Field::new("input");
    let cases = [
        (
            cast_dynamic(Kind::Either(vec![Kind::Int, Kind::String]), value.clone()),
            "<int | string> input",
        ),
        (
            cast_dynamic(
                Kind::Literal(KindLiteral::String("draft".to_owned())),
                value.clone(),
            ),
            "<'draft'> input",
        ),
        (
            cast_dynamic(Kind::Array(Box::new(Kind::Int), Some(3)), value),
            "<array<int, 3>> input",
        ),
    ];
    for (cast, expected) in cases {
        assert_eq!(cast.to_debug_sql(), expected);
        assert_parses(&format!("RETURN {expected};"));
    }
}

#[test]
fn casts_enter_only_the_expressions_of_their_kind() {
    let length = NumberLike::from(cast::<kinds::Int>("5"));
    assert_eq!(length.to_debug_sql(), "<int> '5'");
    let name = StrandLike::from(cast::<kinds::String>(5));
    assert_eq!(name.to_debug_sql(), "<string> 5");
    let untyped = cast_dynamic(Kind::Int, "5");
    assert_eq!(
        NumberLike::from(untyped.clone()).to_debug_sql(),
        "<int> '5'"
    );
    assert_eq!(StrandLike::from(untyped).to_debug_sql(), "<int> '5'");
}

#[tokio::test]
async fn casts_convert_on_the_engine() {
    let db = db().await;
    let int: Option<i64> =
        umbriel::statements::return_(Field::from_expr(cast::<kinds::Int>("5")).add(1))
            .get_data(db.clone())
            .await
            .expect("ran");
    assert_eq!(int, Some(6));
    let string: Option<String> = umbriel::statements::return_(cast::<kinds::String>(5))
        .get_data(db.clone())
        .await
        .expect("ran");
    assert_eq!(string.as_deref(), Some("5"));

    let union = Kind::Either(vec![Kind::Int, Kind::String]);
    let first: bool = read(
        &db,
        &format!(
            "RETURN type::is_int({});",
            cast_dynamic(union.clone(), "5").to_debug_sql()
        ),
    )
    .await;
    assert!(first, "a union cast tries `int` first");
    let second: bool = read(
        &db,
        &format!(
            "RETURN type::is_string({});",
            cast_dynamic(union, "five").to_debug_sql()
        ),
    )
    .await;
    assert!(second, "then `string`");

    let error = failure(
        &db,
        &format!("RETURN {};", cast::<kinds::Int>("five").to_debug_sql()),
    )
    .await;
    assert!(
        error.contains("five"),
        "the failed cast names the value: {error}"
    );
}
