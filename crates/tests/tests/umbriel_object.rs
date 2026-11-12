//! `object::*` (`orm/src/functions/object.rs`): every function and method form rendered, and the
//! results SurrealDB computes.

use std::collections::BTreeMap;

use umbriel::{Field, ObjectLike, Param, Render, functions::object};
use umbriel_tests::*;

fn alice() -> BTreeMap<String, i64> {
    BTreeMap::from([("age".to_owned(), 30), ("score".to_owned(), 7)])
}

#[test]
fn rendering() {
    let settings = Field::new("settings");
    calls!(
        (
            object::entries!(settings.clone()),
            "object::entries(settings)"
        ),
        (
            object::extend!(settings.clone(), Field::new("defaults")),
            "object::extend(settings, defaults)"
        ),
        (
            object::from_entries!(Field::new("pairs")),
            "object::from_entries(pairs)"
        ),
        (
            object::is_empty!(settings.clone()),
            "object::is_empty(settings)"
        ),
        (object::keys!(settings.clone()), "object::keys(settings)"),
        (object::len!(settings.clone()), "object::len(settings)"),
        (
            object::remove!(settings.clone(), "age"),
            "object::remove(settings, 'age')"
        ),
        (object::values!(settings), "object::values(settings)"),
    );
    let settings = ObjectLike::from(Field::new("settings"));
    calls!(
        (settings.entries(), "settings.entries()"),
        (
            settings.extend(Field::new("defaults")),
            "settings.extend(defaults)"
        ),
        (settings.is_empty(), "settings.is_empty()"),
        (settings.keys(), "settings.keys()"),
        (settings.len(), "settings.len()"),
        (settings.remove("age"), "settings.remove('age')"),
        (settings.values(), "settings.values()"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(
        &db,
        [
            (object::entries!(alice()), "[['age', 30], ['score', 7]]"),
            (
                object::extend!(alice(), BTreeMap::from([("age".to_owned(), 31)])),
                "{ age: 31, score: 7 }"
            ),
            (
                object::from_entries!(object::entries!(alice())),
                "{ age: 30, score: 7 }"
            ),
            (object::is_empty!(BTreeMap::<String, i64>::new()), "true"),
            (object::keys!(alice()), "['age', 'score']"),
            (object::len!(alice()), "2"),
            (object::remove!(alice(), "age"), "{ score: 7 }"),
            (object::values!(alice()), "[30, 7]"),
            (ObjectLike::from(alice()).len(), "2"),
        ]
    );
}

#[test]
fn generated_bindings() {
    let nested = object::len!(object::extend!(Param::new("settings"), alice()));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "object::len(object::extend($settings, $_p0))");
    assert_eq!(vars.len(), 1);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = failure(&db, "RETURN object::keys([1]);").await;
    assert!(error.contains("object::keys"), "{error}");
}

/// Every mapped method runs on an object and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let db = db().await;
    let o = || ObjectLike::from(alice());
    let other = || BTreeMap::from([("age".to_owned(), 31)]);
    same_all!(&db, [
        (o().entries(), object::entries!(alice())),
        (o().extend(other()), object::extend!(alice(), other())),
        (o().is_empty(), object::is_empty!(alice())),
        (o().keys(), object::keys!(alice())),
        (o().len(), object::len!(alice())),
        (o().remove("age"), object::remove!(alice(), "age")),
        (o().values(), object::values!(alice())),
    ]);
}
