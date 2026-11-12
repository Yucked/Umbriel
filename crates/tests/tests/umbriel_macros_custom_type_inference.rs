//! `umbriel-macros` `custom_type_inference`: the stored type each Rust field type is given, and
//! that a value of every SurrealQL kind round-trips through a model field on the in-memory
//! engine.

use std::{collections::BTreeMap, ops::Bound, time::Duration};

use chrono::{DateTime, Utc};
use umbriel::{
    Model, Nullable, RecordId, SurrealCrudNode, SurrealRead,
    types::{self, SurrealValue, Value},
};
use umbriel_tests::*;

fn int(value: i64) -> Value {
    value.into_value()
}

fn text(value: &str) -> Value {
    value.to_owned().into_value()
}

fn point() -> geo::Point {
    geo::Point::new(1.0, 2.0)
}

fn square() -> geo::Polygon {
    geo::Polygon::new(
        geo::LineString::from(vec![
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (0.0, 0.0),
        ]),
        vec![],
    )
}

/// A record holding a typical value of every kind.
fn value_kind(key: &str) -> ValueKind {
    let mut shape = types::Object::new();
    shape.insert("kind", "a");
    shape.insert("n", 1);
    ValueKind {
        id: ValueKind::id(key),
        flag: true,
        count: 42,
        ratio: 0.5,
        price: types::Decimal::new(1999, 2),
        amount: types::Number::Float(2.5),
        label: "Alice".to_owned(),
        created: DateTime::<Utc>::UNIX_EPOCH,
        elapsed: Duration::from_secs(90),
        token: types::Uuid::from(uuid::Uuid::from_u128(
            0x0190_7a2e_4c1d_7b3a_8f00_0000_0000_0001,
        )),
        payload: types::Bytes::from(vec![1, 2, 3]),
        members: types::Set::from(vec![text("alice"), text("bob")]),
        scores: BTreeMap::from([("alice".to_owned(), 3), ("bob".to_owned(), 5)]),
        window: types::Range::from((Bound::Included(int(1)), Bound::Excluded(int(5)))),
        source: types::Table::new("user"),
        pattern: Value::Regex("^a.*e$".parse().expect("a regex")),
        triple: [1, 2, 3],
        point: point(),
        line: geo::LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]),
        polygon: square(),
        points: geo::MultiPoint::from(vec![point(), geo::Point::new(3.0, 4.0)]),
        lines: geo::MultiLineString::new(vec![geo::LineString::from(vec![(0.0, 0.0), (1.0, 1.0)])]),
        polygons: geo::MultiPolygon::new(vec![square()]),
        collection: types::Geometry::Collection(vec![
            types::Geometry::Point(point()),
            types::Geometry::Polygon(square()),
        ]),
        maybe: Some(7),
        nullable: Nullable::Value(8),
        status: MessageStatus::Sent,
        level: 2,
        shape: Value::Object(shape),
        either: int(7),
        maybe_either: Some(text("seven")),
        owner: User::id("alice"),
        reviewer: Some(User::id("bob")),
        watchers: vec![User::id("alice"), User::id("bob")],
    }
}

async fn stored(db: &surrealdb::Surreal<surrealdb::engine::local::Db>, record: ValueKind) {
    let created = record.clone().create(db).await.expect("created");
    assert_eq!(created, record, "create returns the stored record");
    let found = ValueKind::get(db, &record.id)
        .await
        .expect("read")
        .expect("exists");
    assert_eq!(found, record);
}

#[test]
fn every_field_kind_is_inferred() {
    assert_eq!(
        joined(&ValueKind::define_fields()),
        "DEFINE FIELD flag ON TABLE value_kind TYPE bool;
DEFINE FIELD count ON TABLE value_kind TYPE int;
DEFINE FIELD ratio ON TABLE value_kind TYPE float;
DEFINE FIELD price ON TABLE value_kind TYPE decimal;
DEFINE FIELD amount ON TABLE value_kind TYPE number;
DEFINE FIELD label ON TABLE value_kind TYPE string;
DEFINE FIELD created ON TABLE value_kind TYPE datetime;
DEFINE FIELD elapsed ON TABLE value_kind TYPE duration;
DEFINE FIELD token ON TABLE value_kind TYPE uuid;
DEFINE FIELD payload ON TABLE value_kind TYPE bytes;
DEFINE FIELD members ON TABLE value_kind TYPE set<any>;
DEFINE FIELD scores ON TABLE value_kind TYPE object;
DEFINE FIELD window ON TABLE value_kind TYPE range;
DEFINE FIELD source ON TABLE value_kind TYPE table<user>;
DEFINE FIELD pattern ON TABLE value_kind TYPE regex;
DEFINE FIELD triple ON TABLE value_kind TYPE array<int, 3>;
DEFINE FIELD point ON TABLE value_kind TYPE geometry<point>;
DEFINE FIELD line ON TABLE value_kind TYPE geometry<line>;
DEFINE FIELD polygon ON TABLE value_kind TYPE geometry<polygon>;
DEFINE FIELD points ON TABLE value_kind TYPE geometry<multipoint>;
DEFINE FIELD lines ON TABLE value_kind TYPE geometry<multiline>;
DEFINE FIELD polygons ON TABLE value_kind TYPE geometry<multipolygon>;
DEFINE FIELD collection ON TABLE value_kind TYPE geometry<collection>;
DEFINE FIELD maybe ON TABLE value_kind TYPE none | int;
DEFINE FIELD nullable ON TABLE value_kind TYPE null | int;
DEFINE FIELD status ON TABLE value_kind TYPE 'draft' | 'sent';
DEFINE FIELD level ON TABLE value_kind TYPE 1 | 2 | 3;
DEFINE FIELD shape ON TABLE value_kind TYPE {kind: 'a', n: int};
DEFINE FIELD either ON TABLE value_kind TYPE int | string;
DEFINE FIELD maybe_either ON TABLE value_kind TYPE none | int | string;
DEFINE FIELD owner ON TABLE value_kind TYPE record<user>;
DEFINE FIELD reviewer ON TABLE value_kind TYPE none | record<user>;
DEFINE FIELD watchers ON TABLE value_kind TYPE array<record<user>>;"
    );
}

#[tokio::test]
async fn every_field_round_trips() {
    let db = db().await;
    apply_definitions::<ValueKind>(&db).await;
    stored(&db, value_kind("alice")).await;
}

#[tokio::test]
async fn edge_values_round_trip() {
    let db = db().await;
    apply_definitions::<ValueKind>(&db).await;
    stored(
        &db,
        ValueKind {
            payload: types::Bytes::from(Vec::new()),
            members: types::Set::new(),
            maybe: None,
            nullable: Nullable::Null,
            maybe_either: None,
            reviewer: None,
            watchers: vec![],
            ..value_kind("bob")
        },
    )
    .await;

    let stored: Vec<bool> = read(
        &db,
        "RETURN [type::is_none(value_kind:bob.maybe), type::is_null(value_kind:bob.nullable)];",
    )
    .await;
    assert_eq!(stored, [true, true], "NONE and NULL stay distinct");

    // 3.3.4 fixed fractional decimals losing their fraction through `option<decimal>`.
    assert!(
        succeeds(
            &db,
            "DEFINE FIELD discount ON TABLE value_kind TYPE option<decimal>;
             UPDATE value_kind:bob SET discount = 1.25dec;"
        )
        .await
    );
    let discount: types::Decimal = read(&db, "RETURN value_kind:bob.discount;").await;
    assert_eq!(discount, types::Decimal::new(125, 2));
}

#[tokio::test]
async fn each_union_variant_round_trips() {
    let db = db().await;
    apply_definitions::<ValueKind>(&db).await;
    stored(
        &db,
        ValueKind {
            either: int(7),
            maybe_either: Some(int(8)),
            status: MessageStatus::Draft,
            level: 1,
            ..value_kind("alice")
        },
    )
    .await;
    stored(
        &db,
        ValueKind {
            either: text("seven"),
            maybe_either: Some(text("eight")),
            status: MessageStatus::Sent,
            level: 3,
            ..value_kind("bob")
        },
    )
    .await;
}

/// Each case sets `field` on a stored record: the first value fits its type, the second does not.
async fn assert_accepts_then_refuses(field: &str, accepted: &str, refused: &str) {
    let db = db().await;
    apply_definitions::<ValueKind>(&db).await;
    value_kind("alice").create(&db).await.expect("created");
    assert!(
        succeeds(
            &db,
            &format!("UPDATE value_kind:alice SET {field} = {accepted};")
        )
        .await,
        "`{field}` accepts {accepted}"
    );
    assert!(
        !succeeds(
            &db,
            &format!("UPDATE value_kind:alice SET {field} = {refused};")
        )
        .await,
        "`{field}` refuses {refused}"
    );
}

#[tokio::test]
async fn union_and_literal_fields_refuse_other_values() {
    assert_accepts_then_refuses("either", "'seven'", "true").await;
    assert_accepts_then_refuses("level", "3", "4").await;
    assert_accepts_then_refuses("status", "'draft'", "'gone'").await;
    assert_accepts_then_refuses("shape", "{ kind: 'a', n: 2 }", "{ kind: 'b', n: 2 }").await;
}

#[tokio::test]
async fn fixed_arrays_refuse_shorter_and_longer() {
    assert_accepts_then_refuses("triple", "[4, 5, 6]", "[1, 2]").await;
    assert_accepts_then_refuses("triple", "[4, 5, 6]", "[1, 2, 3, 4]").await;
}

/// Runs `sql` with `$source` bound to the table `table`, as the SDK writes a `types::Table`.
async fn with_table(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    sql: &str,
    table: &str,
) -> surrealdb::Result<()> {
    db.query(sql.to_owned())
        .bind(("source", types::Table::new(table)))
        .await
        .and_then(surrealdb::IndexedResults::check)
        .map(drop)
}

#[tokio::test]
async fn table_fields_refuse_other_tables() {
    let db = db().await;
    apply_definitions::<ValueKind>(&db).await;
    value_kind("alice").create(&db).await.expect("created");
    let sql = "UPDATE value_kind:alice SET source = $source;";
    with_table(&db, sql, "user")
        .await
        .expect("`source` accepts `user`");
    assert!(
        with_table(&db, sql, "post").await.is_err(),
        "`source` refuses `post`"
    );
}

/// SurrealDB 3.3.4 coerces a plain `table` field through `string`: it takes a table name written
/// as a string and refuses a table value, which is why `types::Table` needs `ty = "table<…>"`.
/// When this fails, SurrealDB accepts table values and `types::Table` can be inferred again.
#[tokio::test]
async fn a_plain_table_field_refuses_table_values() {
    let db = db().await;
    assert!(
        succeeds(
            &db,
            "DEFINE TABLE probe; DEFINE FIELD source ON probe TYPE table;"
        )
        .await
    );
    assert!(succeeds(&db, "CREATE probe:a SET source = 'user';").await);
    let error = with_table(&db, "CREATE probe:b SET source = $source;", "user")
        .await
        .expect_err("a table value is refused");
    assert!(
        error
            .to_string()
            .contains("Expected `string` but found `user`"),
        "refused by the string coercion: {error}"
    );
}

#[tokio::test]
async fn record_fields_refuse_other_tables() {
    assert_accepts_then_refuses("owner", "user:carol", "value_kind:alice").await;
    assert_accepts_then_refuses("watchers", "[user:carol]", "[value_kind:alice]").await;
}

#[tokio::test]
async fn every_key_kind_round_trips_in_a_record_field() {
    let db = db().await;
    apply_definitions::<ValueKind>(&db).await;
    let mut object = types::Object::new();
    object.insert("region", "eu");
    object.insert("seq", 7);
    let owners: [RecordId<User>; 5] = [
        User::id(42),
        User::id("alice"),
        User::id(uuid::Uuid::from_u128(
            0x0190_7a2e_4c1d_7b3a_8f00_0000_0000_0002,
        )),
        User::id(vec![1i64, 2]),
        User::id(object),
    ];
    for (index, owner) in owners.into_iter().enumerate() {
        stored(
            &db,
            ValueKind {
                owner,
                ..value_kind(&format!("key{index}"))
            },
        )
        .await;
    }
}
