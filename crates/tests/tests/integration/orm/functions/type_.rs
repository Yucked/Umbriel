//! `type::*` (`orm/src/functions/type_.rs`): every function and its method form on every value,
//! and the conversions and checks SurrealDB computes.

use std::time::Duration;

use umbriel::{
    Field, Model, Param, Render, StrandLike, Table, ValueLike, functions::type_, types::Value,
};
use umbriel_tests::*;

/// The forms no server test calls.
#[test]
fn rendering() {
    calls!(
        (type_::field("name"), "type::field('name')"),
        (
            type_::fields(vec!["name", "age"]),
            "type::fields(['name', 'age'])"
        ),
    );
}

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (type_::array(type_::set(vec![2, 1])), "[1, 2]"),
            (type_::bool("true"), "true"),
            (type_::bytes("hi"), "<bytes> 'hi'"),
            (type_::decimal("1.5"), "1.5dec"),
            (type_::duration("1h"), "1h"),
            (type_::float("1.5"), "1.5"),
            (type_::int("5"), "5"),
            (type_::number("5"), "5"),
            (type_::of(5), "'int'"),
            (type_::of("a"), "'string'"),
            (type_::point(vec![1, 2]), "(1, 2)"),
            (type_::record!("user", "alice"), "user:alice"),
            (type_::record!("user:alice"), "user:alice"),
            (type_::set(vec![1, 1, 2]), "<set> [1, 2]"),
            (
                type_::datetime("2021-11-01T08:30:17Z"),
                "d'2021-11-01T08:30:17Z'"
            ),
            (type_::string(5), "'5'"),
            (type_::is_string(5), "false"),
            (type_::is_int("5"), "false"),
            (type_::is_array(vec![1]), "true"),
            (type_::is_bool(true), "true"),
            (type_::is_datetime(datetime()), "true"),
            (type_::is_duration(Duration::from_secs(60)), "true"),
            (type_::is_float(1.5), "true"),
            (type_::is_int(1), "true"),
            (type_::is_none(umbriel::NONE), "true"),
            (type_::is_null(umbriel::NULL), "true"),
            (type_::is_number(1), "true"),
            (type_::is_point(geo::Point::new(1.0, 2.0)), "true"),
            (type_::is_record!(User::id("alice")), "true"),
            (type_::is_record!(User::id("alice"), "user"), "true"),
            (type_::is_record!(User::id("alice"), "post"), "false"),
            (type_::is_set(type_::set(vec![1])), "true"),
            (type_::is_string("a"), "true"),
            (
                type_::is_uuid(Field::raw("u'018a6680-bef9-701b-9025-e1754f296a0f'")),
                "true"
            ),
            (ValueLike::from(5).to_string_value(), "'5'"),
            (ValueLike::from("5").to_int(), "5"),
            (
                ValueLike::from("user").to_record(Some("alice")),
                "user:alice"
            ),
            (ValueLike::from(1).is_int(), "true"),
        ]
    );
    let record: Option<umbriel::RecordId<User>> =
        bound(&db, &type_::record!("user", "alice")).await;
    assert_eq!(record, Some(User::id("alice")));
    let table: Option<Value> = bound(&db, &type_::table("user")).await;
    assert_eq!(table, Some(Value::Table(Table::from("user"))));
}

#[tokio::test]
async fn string_receivers_dispatch_to_string_checks() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (
                ValueLike::from("018a6680-bef9-701b-9025-e1754f296a0f").is_uuid(),
                "true"
            ),
            (
                type_::is_uuid("018a6680-bef9-701b-9025-e1754f296a0f"),
                "false"
            ),
            (
                StrandLike::from("2021-11-01T08:30:17Z").is_datetime(None::<&str>),
                "true"
            ),
        ]
    );
}

#[test]
fn generated_bindings() {
    let nested = type_::int(type_::string(Param::new("count")));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "type::int(type::string($count))");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = call_failure(&db, &type_::int("five")).await;
    assert!(
        error.contains("five") || error.contains("type::int"),
        "{error}"
    );
}

/// Every mapped method runs on a value and evaluates like its function form. Checks run on a
/// number: on a string, SurrealDB dispatches `.is_datetime()`, `.is_uuid()` and `.is_record()` to
/// the `string::is_*` functions (`string_receivers_dispatch_to_string_checks`).
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let v = |value: ValueLike| value;
    let one = || ValueLike::from(1);
    let set = || Field::raw("(<set> [2, 1])");
    let range = || Field::raw("(1..3)");
    let uuid = "018a6680-bef9-701b-9025-e1754f296a0f";
    let point = geo::Point::new(1.0, 2.0);
    same_all!(
        &db,
        [
            (v(set().into()).to_array(), type_::array(set())),
            (v("true".into()).to_bool(), type_::bool("true")),
            (v("hi".into()).to_bytes(), type_::bytes("hi")),
            (
                v("2021-11-01T08:30:17Z".into()).to_datetime(),
                type_::datetime("2021-11-01T08:30:17Z")
            ),
            (v("1.5".into()).to_decimal(), type_::decimal("1.5")),
            (v("1h".into()).to_duration(), type_::duration("1h")),
            (v("1.5".into()).to_float(), type_::float("1.5")),
            (v(point.into()).to_geometry(), type_::geometry(point)),
            (v("5".into()).to_int(), type_::int("5")),
            (v("5".into()).to_number(), type_::number("5")),
            (v(1.5.into()).type_of(), type_::of(1.5)),
            (v(vec![1, 2].into()).to_point(), type_::point(vec![1, 2])),
            (v(range().into()).to_range(), type_::range(range())),
            (
                v("user".into()).to_record(Some("alice")),
                type_::record!("user", "alice")
            ),
            (v(vec![1, 1].into()).to_set(), type_::set(vec![1, 1])),
            (one().to_string_value(), type_::string(1)),
            (
                v(type_::bytes("hi").into()).to_string_lossy(),
                type_::string_lossy(type_::bytes("hi"))
            ),
            (v(uuid.into()).to_uuid(), type_::uuid(uuid)),
            (one().is_array(), type_::is_array(1)),
            (one().is_bool(), type_::is_bool(1)),
            (one().is_bytes(), type_::is_bytes(1)),
            (one().is_collection(), type_::is_collection(1)),
            (one().is_datetime(), type_::is_datetime(1)),
            (one().is_decimal(), type_::is_decimal(1)),
            (one().is_duration(), type_::is_duration(1)),
            (one().is_float(), type_::is_float(1)),
            (one().is_geometry(), type_::is_geometry(1)),
            (one().is_int(), type_::is_int(1)),
            (one().is_line(), type_::is_line(1)),
            (one().is_multiline(), type_::is_multiline(1)),
            (one().is_multipoint(), type_::is_multipoint(1)),
            (one().is_multipolygon(), type_::is_multipolygon(1)),
            (one().is_none(), type_::is_none(1)),
            (one().is_null(), type_::is_null(1)),
            (one().is_number(), type_::is_number(1)),
            (one().is_object(), type_::is_object(1)),
            (one().is_point(), type_::is_point(1)),
            (one().is_polygon(), type_::is_polygon(1)),
            (one().is_range(), type_::is_range(1)),
            (one().is_record(None::<&str>), type_::is_record!(1)),
            (one().is_set(), type_::is_set(1)),
            (one().is_string(), type_::is_string(1)),
            (one().is_uuid(), type_::is_uuid(1)),
        ]
    );
}
