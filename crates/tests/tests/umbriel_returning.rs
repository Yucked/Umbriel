//! Return modes (`orm/src/statements/returning.rs`): `DiffOp` decodes every JSON Patch operation
//! by its `op` tag, and `ONLY` turns a list result into a single optional one, keeping a result
//! that is itself an array whole.

use std::collections::BTreeMap;

use surrealdb::{
    Surreal,
    engine::local::Mem,
    opt::auth::Record,
    types::{Object, SurrealNull, SurrealValue, Value},
};
use umbriel::{
    Field, Model, Operatable, SchemaGetter, SetterAssignable, SurrealCrudNode,
    statements::{
        CreateTarget, DiffOp, Returning, create, create_only, delete, delete_only, insert, update,
        update_only,
    },
};
use umbriel_tests::*;

fn op(fields: &[(&str, Value)]) -> Value {
    let mut object = Object::new();
    for (key, value) in fields {
        object.insert((*key).to_owned(), value.clone());
    }
    Value::Object(object)
}

fn string(text: &str) -> Value {
    Value::String(text.to_owned())
}

#[test]
fn every_operation_decodes_by_its_tag() {
    let cases = [
        (
            op(&[
                ("op", string("add")),
                ("path", string("/a")),
                ("value", 1_i64.into_value()),
            ]),
            DiffOp::Add {
                path: "/a".to_owned(),
                value: 1_i64.into_value(),
            },
        ),
        (
            op(&[("op", string("remove")), ("path", string("/a"))]),
            DiffOp::Remove {
                path: "/a".to_owned(),
            },
        ),
        // Same shape as `add`: only the tag tells them apart.
        (
            op(&[
                ("op", string("replace")),
                ("path", string("/a")),
                ("value", 1_i64.into_value()),
            ]),
            DiffOp::Replace {
                path: "/a".to_owned(),
                value: 1_i64.into_value(),
            },
        ),
        (
            op(&[
                ("op", string("change")),
                ("path", string("/name")),
                ("value", string("@@ -1 +1 @@\n-a\n+b\n")),
            ]),
            DiffOp::Change {
                path: "/name".to_owned(),
                value: "@@ -1 +1 @@\n-a\n+b\n".to_owned(),
            },
        ),
    ];
    for (value, expected) in cases {
        assert!(DiffOp::is_value(&value), "{value:?}");
        let decoded = DiffOp::from_value(value).expect("decodes");
        assert_eq!(decoded, expected);
        assert_eq!(
            DiffOp::from_value(decoded.clone().into_value()).expect("round trips"),
            decoded
        );
    }
}

#[test]
fn unknown_or_incomplete_operations_fail() {
    for value in [
        op(&[("op", string("copy")), ("path", string("/a"))]),
        op(&[("op", string("add")), ("path", string("/a"))]),
        op(&[("path", string("/a"))]),
        op(&[("op", string("remove"))]),
        string("remove"),
    ] {
        assert!(DiffOp::from_value(value.clone()).is_err(), "{value:?}");
    }
}

#[tokio::test]
async fn only_returns_one_optional_result() {
    let db = db().await;
    let ship = SpaceShip {
        id: SpaceShip::id("apollo"),
        name: "Apollo".to_owned(),
        ..Default::default()
    };
    let created: Option<SpaceShip> = create_only::<SpaceShip>(CreateTarget::table())
        .content(ship.clone())
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(created.map(|ship| ship.id), Some(ship.id.clone()));

    let many: Vec<SpaceShip> = create::<SpaceShip>(SpaceShip::table())
        .content(SpaceShip {
            id: SpaceShip::id("gemini"),
            ..ship.clone()
        })
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(many.len(), 1);

    let missing: Option<SpaceShip> = update_only::<SpaceShip>(SpaceShip::id("missing"))
        .execute(&db)
        .await
        .expect("updated nothing");
    assert!(missing.is_none());

    let deleted = delete_only::<SpaceShip>(SpaceShip::id("apollo"))
        .return_before()
        .execute(&db)
        .await
        .expect("deleted");
    assert_eq!(deleted.map(|ship| ship.name), Some("Apollo".to_owned()));
}

/// `RETURN NULL` under select permissions: a record user on an isolated in-memory database may
/// create in a table it may not select from, and the written records are not returned.
#[tokio::test]
async fn return_null_omits_records_the_session_may_not_select() {
    let db = Surreal::new::<Mem>(()).await.expect("in-memory engine");
    db.use_ns("test").use_db("test").await.expect("selected");
    let mut defined = db
        .query(
            "DEFINE ACCESS account ON DATABASE TYPE RECORD \
             SIGNUP (CREATE person SET name = $name) \
             SIGNIN (SELECT * FROM person WHERE name = $name); \
             DEFINE TABLE space_ship SCHEMALESS \
             PERMISSIONS FOR select NONE FOR create, update, delete FULL;",
        )
        .await
        .expect("defined");
    assert!(defined.take_errors().is_empty(), "definitions");
    db.signup(Record {
        namespace: "test".to_owned(),
        database: "test".to_owned(),
        access: "account".to_owned(),
        params: BTreeMap::from([("name".to_owned(), "Alice".to_owned())]),
    })
    .await
    .expect("signed up");

    let created = create::<SpaceShip>(SpaceShip::id("a"))
        .return_null()
        .execute(&db)
        .await
        .expect("created");
    assert!(created.is_empty(), "{created:?}");
    let one = create_only::<SpaceShip>(SpaceShip::id("b"))
        .return_null()
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(one, None);
}

/// `RETURN NULL` gives one `NULL` for each record a statement returns, recorded per case: a
/// `WHERE` that matches nothing and an `IGNORE`d duplicate return none. Select permissions are
/// covered by the test above.
#[tokio::test]
async fn return_null_gives_one_null_per_returned_record() {
    let db = db().await;
    let ship = |key: &str| SpaceShip {
        id: SpaceShip::id(key),
        name: "Apollo".to_owned(),
        ..Default::default()
    };
    let created = create::<SpaceShip>([SpaceShip::id("a").into(), SpaceShip::id("b").into()])
        .return_null()
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(created, [SurrealNull, SurrealNull]);

    let one = create_only::<SpaceShip>(SpaceShip::id("c"))
        .return_null()
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(one, Some(SurrealNull));

    let schema = SpaceShip::schema();
    let unmatched = update::<SpaceShip>(SpaceShip::table())
        .set(schema.name.equal_to("Gemini"))
        .where_(schema.name.equal("Mercury"))
        .return_null()
        .execute(&db)
        .await
        .expect("updated nothing");
    assert!(unmatched.is_empty());

    let missing = update_only::<SpaceShip>(SpaceShip::id("missing"))
        .return_null()
        .execute(&db)
        .await
        .expect("updated nothing");
    assert_eq!(missing, None);

    let ignored = insert(vec![ship("a"), ship("d")])
        .ignore()
        .return_null()
        .execute(&db)
        .await
        .expect("inserted");
    assert_eq!(ignored, [SurrealNull], "only the new record is returned");

    let deleted = delete::<SpaceShip>(SpaceShip::table())
        .return_null()
        .execute(&db)
        .await
        .expect("deleted");
    assert_eq!(deleted.len(), 4);
}

/// An `ONLY` result that is itself an array decodes whole, whatever its length.
#[tokio::test]
async fn only_keeps_an_array_result_whole() {
    let db = db().await;
    User {
        id: User::id("alice"),
        name: "Alice".to_owned(),
        ..Default::default()
    }
    .create(&db)
    .await
    .expect("alice");
    let schema = User::schema();
    for tags in [
        vec![],
        vec!["go".to_owned()],
        vec!["go".to_owned(), "sql".to_owned()],
    ] {
        let value = update_only::<User>(User::id("alice"))
            .set(schema.tags.equal_to(tags.clone()))
            .return_value(User::schema().tags)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(value, Some(tags.clone()));

        // A projection is an object per record, holding the array.
        let fields = update_only::<User>(User::id("alice"))
            .set(schema.tags.equal_to(tags.clone()))
            .return_fields::<BTreeMap<String, Vec<String>>>(Field::new("tags"))
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(fields, Some(BTreeMap::from([("tags".to_owned(), tags)])));
    }
}
