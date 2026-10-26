//! Return modes (`orm/src/statements/returning.rs`): `DiffOp` decodes every JSON Patch operation
//! by its `op` tag, and `ONLY` turns a list result into a single optional one.

use surrealdb::types::{Object, SurrealValue, Value};
use umbriel::{
    Model,
    statements::{CreateTarget, DiffOp, Returning, create, create_only, delete_only, update_only},
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
            op(&[("op", string("add")), ("path", string("/a")), ("value", 1_i64.into_value())]),
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
