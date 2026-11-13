//! `type::*` (`orm/src/functions/type_.rs`): every function and its method form on every value,
//! and the conversions and checks SurrealDB computes.

use std::time::Duration;

use umbriel::{
    Field, Model, Param, Render, StrandLike, Table, ValueLike, functions::type_, types::Value,
};
use umbriel_tests::*;

#[test]
fn rendering() {
    let value = Field::new("input");
    calls!(
        (type_::array!(value.clone()), "type::array(input)"),
        (type_::bool!("true"), "type::bool('true')"),
        (type_::bytes!(value.clone()), "type::bytes(input)"),
        (
            type_::datetime!(Field::new("created")),
            "type::datetime(created)"
        ),
        (
            type_::datetime!("2021-11-01T08:30:17Z"),
            "type::datetime('2021-11-01T08:30:17Z')"
        ),
        (type_::decimal!(value.clone()), "type::decimal(input)"),
        (
            type_::duration!(Duration::from_secs(3600)),
            "type::duration(1h)"
        ),
        (type_::field!("name"), "type::field('name')"),
        (
            type_::fields!(vec!["name", "age"]),
            "type::fields(['name', 'age'])"
        ),
        (type_::float!(1), "type::float(1)"),
        (type_::float!("1.5"), "type::float('1.5')"),
        (type_::geometry!(value.clone()), "type::geometry(input)"),
        (type_::int!(1), "type::int(1)"),
        (type_::number!(1), "type::number(1)"),
        (type_::of!(value.clone()), "type::of(input)"),
        (type_::point!(vec![1, 2]), "type::point([1, 2])"),
        (type_::range!(value.clone()), "type::range(input)"),
        (
            type_::record!("user", "alice"),
            "type::record('user', 'alice')"
        ),
        (type_::record!("user:alice"), "type::record('user:alice')"),
        (type_::set!(value.clone()), "type::set(input)"),
        (type_::string!(5), "type::string(5)"),
        (
            type_::string_lossy!(value.clone()),
            "type::string_lossy(input)"
        ),
        (type_::table!("user"), "type::table('user')"),
        (type_::uuid!(value.clone()), "type::uuid(input)"),
        (type_::is_array!(Field::new("tags")), "type::is_array(tags)"),
        (type_::is_bool!(true), "type::is_bool(true)"),
        (type_::is_bytes!(Field::new("data")), "type::is_bytes(data)"),
        (
            type_::is_collection!(Field::new("area")),
            "type::is_collection(area)"
        ),
        (
            type_::is_datetime!(Field::new("created")),
            "type::is_datetime(created)"
        ),
        (
            type_::is_decimal!(Field::new("price")),
            "type::is_decimal(price)"
        ),
        (
            type_::is_duration!(Duration::from_secs(60)),
            "type::is_duration(1m)"
        ),
        (type_::is_float!(1.5), "type::is_float(1.5f)"),
        (
            type_::is_geometry!(Field::new("area")),
            "type::is_geometry(area)"
        ),
        (type_::is_int!(1), "type::is_int(1)"),
        (type_::is_line!(Field::new("area")), "type::is_line(area)"),
        (
            type_::is_multiline!(Field::new("area")),
            "type::is_multiline(area)"
        ),
        (
            type_::is_multipoint!(Field::new("area")),
            "type::is_multipoint(area)"
        ),
        (
            type_::is_multipolygon!(Field::new("area")),
            "type::is_multipolygon(area)"
        ),
        (type_::is_none!(Field::new("bio")), "type::is_none(bio)"),
        (type_::is_null!(Field::new("bio")), "type::is_null(bio)"),
        (type_::is_number!(1), "type::is_number(1)"),
        (
            type_::is_object!(Field::new("settings")),
            "type::is_object(settings)"
        ),
        (type_::is_point!(Field::new("area")), "type::is_point(area)"),
        (
            type_::is_polygon!(Field::new("area")),
            "type::is_polygon(area)"
        ),
        (type_::is_range!(Field::new("span")), "type::is_range(span)"),
        (
            type_::is_record!(Field::new("owner")),
            "type::is_record(owner)"
        ),
        (
            type_::is_record!(Field::new("owner"), "user"),
            "type::is_record(owner, 'user')"
        ),
        (type_::is_set!(Field::new("tags")), "type::is_set(tags)"),
        (type_::is_string!("a"), "type::is_string('a')"),
        (type_::is_uuid!(Field::new("key")), "type::is_uuid(key)"),
    );
}

#[test]
fn method_rendering() {
    let value = ValueLike::from(Field::new("input"));
    calls!(
        (value.to_array(), "input.to_array()"),
        (value.to_bool(), "input.to_bool()"),
        (value.to_bytes(), "input.to_bytes()"),
        (value.to_datetime(), "input.to_datetime()"),
        (value.to_decimal(), "input.to_decimal()"),
        (value.to_duration(), "input.to_duration()"),
        (value.to_float(), "input.to_float()"),
        (value.to_geometry(), "input.to_geometry()"),
        (value.to_int(), "input.to_int()"),
        (value.to_number(), "input.to_number()"),
        (value.type_of(), "input.type_of()"),
        (value.to_point(), "input.to_point()"),
        (value.to_range(), "input.to_range()"),
        (value.to_record(Some("alice")), "input.to_record('alice')"),
        (value.to_set(), "input.to_set()"),
        (value.to_string_value(), "input.to_string()"),
        (value.to_string_lossy(), "input.to_string_lossy()"),
        (value.to_uuid(), "input.to_uuid()"),
        (value.is_array(), "input.is_array()"),
        (value.is_bool(), "input.is_bool()"),
        (value.is_bytes(), "input.is_bytes()"),
        (value.is_collection(), "input.is_collection()"),
        (value.is_datetime(), "input.is_datetime()"),
        (value.is_decimal(), "input.is_decimal()"),
        (value.is_duration(), "input.is_duration()"),
        (value.is_float(), "input.is_float()"),
        (value.is_geometry(), "input.is_geometry()"),
        (value.is_int(), "input.is_int()"),
        (value.is_line(), "input.is_line()"),
        (value.is_multiline(), "input.is_multiline()"),
        (value.is_multipoint(), "input.is_multipoint()"),
        (value.is_multipolygon(), "input.is_multipolygon()"),
        (value.is_none(), "input.is_none()"),
        (value.is_null(), "input.is_null()"),
        (value.is_number(), "input.is_number()"),
        (value.is_object(), "input.is_object()"),
        (value.is_point(), "input.is_point()"),
        (value.is_polygon(), "input.is_polygon()"),
        (value.is_range(), "input.is_range()"),
        (value.is_record(None::<&str>), "input.is_record()"),
        (value.is_set(), "input.is_set()"),
        (value.is_string(), "input.is_string()"),
        (value.is_uuid(), "input.is_uuid()"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(
        &db,
        [
            (type_::array!(type_::set!(vec![2, 1])), "[1, 2]"),
            (type_::bool!("true"), "true"),
            (type_::bytes!("hi"), "<bytes> 'hi'"),
            (type_::decimal!("1.5"), "1.5dec"),
            (type_::duration!("1h"), "1h"),
            (type_::float!("1.5"), "1.5"),
            (type_::int!("5"), "5"),
            (type_::number!("5"), "5"),
            (type_::of!(5), "'int'"),
            (type_::of!("a"), "'string'"),
            (type_::point!(vec![1, 2]), "(1, 2)"),
            (type_::record!("user", "alice"), "user:alice"),
            (type_::record!("user:alice"), "user:alice"),
            (type_::set!(vec![1, 1, 2]), "<set> [1, 2]"),
            (
                type_::datetime!("2021-11-01T08:30:17Z"),
                "d'2021-11-01T08:30:17Z'"
            ),
            (type_::string!(5), "'5'"),
            (type_::is_string!(5), "false"),
            (type_::is_int!("5"), "false"),
            (type_::is_array!(vec![1]), "true"),
            (type_::is_bool!(true), "true"),
            (type_::is_datetime!(datetime()), "true"),
            (type_::is_duration!(Duration::from_secs(60)), "true"),
            (type_::is_float!(1.5), "true"),
            (type_::is_int!(1), "true"),
            (type_::is_none!(umbriel::NONE), "true"),
            (type_::is_null!(umbriel::NULL), "true"),
            (type_::is_number!(1), "true"),
            (type_::is_point!(geo::Point::new(1.0, 2.0)), "true"),
            (type_::is_record!(User::id("alice")), "true"),
            (type_::is_record!(User::id("alice"), "user"), "true"),
            (type_::is_record!(User::id("alice"), "post"), "false"),
            (type_::is_set!(type_::set!(vec![1])), "true"),
            (type_::is_string!("a"), "true"),
            (
                type_::is_uuid!(Field::raw("u'018a6680-bef9-701b-9025-e1754f296a0f'")),
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
    let table: Option<Value> = bound(&db, &type_::table!("user")).await;
    assert_eq!(table, Some(Value::Table(Table::from("user"))));
}

#[tokio::test]
async fn string_receivers_dispatch_to_string_checks() {
    let db = db().await;
    holds_all!(
        &db,
        [
            (
                ValueLike::from("018a6680-bef9-701b-9025-e1754f296a0f").is_uuid(),
                "true"
            ),
            (
                type_::is_uuid!("018a6680-bef9-701b-9025-e1754f296a0f"),
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
    let nested = type_::int!(type_::string!(Param::new("count")));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "type::int(type::string($count))");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = call_failure(&db, &type_::int!("five")).await;
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
    let db = db().await;
    let v = |value: ValueLike| value;
    let one = || ValueLike::from(1);
    let set = || Field::raw("(<set> [2, 1])");
    let range = || Field::raw("(1..3)");
    let uuid = "018a6680-bef9-701b-9025-e1754f296a0f";
    let point = geo::Point::new(1.0, 2.0);
    same_all!(
        &db,
        [
            (v(set().into()).to_array(), type_::array!(set())),
            (v("true".into()).to_bool(), type_::bool!("true")),
            (v("hi".into()).to_bytes(), type_::bytes!("hi")),
            (
                v("2021-11-01T08:30:17Z".into()).to_datetime(),
                type_::datetime!("2021-11-01T08:30:17Z")
            ),
            (v("1.5".into()).to_decimal(), type_::decimal!("1.5")),
            (v("1h".into()).to_duration(), type_::duration!("1h")),
            (v("1.5".into()).to_float(), type_::float!("1.5")),
            (v(point.into()).to_geometry(), type_::geometry!(point)),
            (v("5".into()).to_int(), type_::int!("5")),
            (v("5".into()).to_number(), type_::number!("5")),
            (v(1.5.into()).type_of(), type_::of!(1.5)),
            (v(vec![1, 2].into()).to_point(), type_::point!(vec![1, 2])),
            (v(range().into()).to_range(), type_::range!(range())),
            (
                v("user".into()).to_record(Some("alice")),
                type_::record!("user", "alice")
            ),
            (v(vec![1, 1].into()).to_set(), type_::set!(vec![1, 1])),
            (one().to_string_value(), type_::string!(1)),
            (
                v(type_::bytes!("hi").into()).to_string_lossy(),
                type_::string_lossy!(type_::bytes!("hi"))
            ),
            (v(uuid.into()).to_uuid(), type_::uuid!(uuid)),
            (one().is_array(), type_::is_array!(1)),
            (one().is_bool(), type_::is_bool!(1)),
            (one().is_bytes(), type_::is_bytes!(1)),
            (one().is_collection(), type_::is_collection!(1)),
            (one().is_datetime(), type_::is_datetime!(1)),
            (one().is_decimal(), type_::is_decimal!(1)),
            (one().is_duration(), type_::is_duration!(1)),
            (one().is_float(), type_::is_float!(1)),
            (one().is_geometry(), type_::is_geometry!(1)),
            (one().is_int(), type_::is_int!(1)),
            (one().is_line(), type_::is_line!(1)),
            (one().is_multiline(), type_::is_multiline!(1)),
            (one().is_multipoint(), type_::is_multipoint!(1)),
            (one().is_multipolygon(), type_::is_multipolygon!(1)),
            (one().is_none(), type_::is_none!(1)),
            (one().is_null(), type_::is_null!(1)),
            (one().is_number(), type_::is_number!(1)),
            (one().is_object(), type_::is_object!(1)),
            (one().is_point(), type_::is_point!(1)),
            (one().is_polygon(), type_::is_polygon!(1)),
            (one().is_range(), type_::is_range!(1)),
            (one().is_record(None::<&str>), type_::is_record!(1)),
            (one().is_set(), type_::is_set!(1)),
            (one().is_string(), type_::is_string!(1)),
            (one().is_uuid(), type_::is_uuid!(1)),
        ]
    );
}
