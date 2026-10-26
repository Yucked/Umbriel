//! UPSERT (`orm/src/statements/upsert.rs`): every clause renders and parses, and the statement
//! inserts or updates as SurrealDB 3.3 documents.

use std::time::Duration;

use surrealdb::{Surreal, engine::local::Db, types::Value};
use umbriel::{
    Field, Model, Operatable, Patchable, Raw, Render, Runnable, SchemaGetter, SetterAssignable,
    SetterNumeric,
    statements::{Returning, upsert, upsert_only},
};
use umbriel_tests::*;

fn organization(key: &str, name: &str, age: u8) -> Organization {
    Organization {
        id: Organization::id(key),
        name: name.to_owned(),
        age,
        ..Default::default()
    }
}

async fn names(db: &Surreal<Db>) -> Vec<String> {
    read(db, "SELECT VALUE name FROM organization ORDER BY name;").await
}

#[test]
fn every_clause_renders_in_order() {
    let org = Organization::schema();
    assert_sql(
        &upsert_only::<Organization>(Organization::id("acme"))
            .set(org.age.increment_by(1u8))
            .where_(org.age.lt(30))
            .timeout(Duration::from_secs(1))
            .return_diff(),
        "UPSERT ONLY organization:acme SET age += 1 WHERE age < 30 RETURN DIFF TIMEOUT 1s;",
    );
    assert_sql(
        &upsert::<Organization>(Organization::table())
            .unset(Field::new("label"))
            .where_(org.name.equal("Acme Corp"))
            .explain(),
        "UPSERT organization UNSET label WHERE name = 'Acme Corp' EXPLAIN;",
    );
    assert_sql(
        &upsert::<Organization>(Organization::id("acme")).patch(org.name.patch_replace("Globex")),
        "UPSERT organization:acme PATCH [{ op: 'replace', path: '/name', value: 'Globex' }];",
    );
    let base = || upsert::<Organization>(Organization::id("acme")).set(org.age.equal_to(1u8));
    for (statement, clause) in [
        (base().return_none().to_debug_sql(), " RETURN NONE;"),
        (base().return_before().to_debug_sql(), " RETURN BEFORE;"),
        (base().return_after().to_debug_sql(), " RETURN AFTER;"),
        (
            base().return_value(Organization::schema().age).to_debug_sql(),
            " RETURN VALUE age;",
        ),
        (
            base().return_fields::<Value>(Field::new("age")).to_debug_sql(),
            " RETURN age;",
        ),
        (base().explain_full().to_debug_sql(), " EXPLAIN FULL;"),
    ] {
        assert!(statement.ends_with(clause), "{statement}");
        assert_parses(&statement);
    }
}

#[tokio::test]
async fn without_where_it_inserts_and_then_updates_a_record_id() {
    let db = db().await;
    let org = Organization::schema();
    let created = upsert::<Organization>(Organization::id("acme"))
        .content(organization("acme", "Acme Corp", 10))
        .execute(&db)
        .await
        .expect("inserted");
    assert_eq!(created[0].age, 10);
    let updated = upsert::<Organization>(Organization::id("acme"))
        .set(org.age.increment_by(1u8))
        .return_value(Organization::schema().age)
        .execute(&db)
        .await
        .expect("updated");
    assert_eq!(updated, [11]);
    let before = upsert_only::<Organization>(Organization::id("acme"))
        .set(org.age.increment_by(1u8))
        .return_before()
        .execute(&db)
        .await
        .expect("updated")
        .expect("returned");
    assert_eq!(before.age, 11);
}

#[tokio::test]
async fn on_a_table_without_where_it_always_inserts() {
    let db = db().await;
    let org = Organization::schema();
    for _ in 0..2 {
        upsert::<Organization>(Organization::table())
            .set(org.name.equal_to("Acme Corp"))
            .return_none()
            .execute(&db)
            .await
            .expect("inserted");
    }
    assert_eq!(names(&db).await, ["Acme Corp", "Acme Corp"]);
}

#[tokio::test]
async fn a_unique_index_match_updates_the_record_holding_the_value() {
    let db = db().await;
    Raw::new("DEFINE INDEX org_name ON organization FIELDS name UNIQUE;")
        .run(db.clone())
        .await
        .expect("defined");
    let org = Organization::schema();
    upsert::<Organization>(Organization::table())
        .set([org.name.equal_to("Acme Corp"), org.age.equal_to(10u8)])
        .return_none()
        .execute(&db)
        .await
        .expect("inserted");
    let ages = upsert::<Organization>(Organization::table())
        .set([org.name.equal_to("Acme Corp"), org.age.equal_to(20u8)])
        .return_value(Organization::schema().age)
        .execute(&db)
        .await
        .expect("updated through the index");
    assert_eq!(ages, [20]);
    assert_eq!(names(&db).await, ["Acme Corp"]);
}

#[tokio::test]
async fn where_gates_only_the_update_path() {
    let db = db().await;
    let org = Organization::schema();
    upsert::<Organization>(Organization::id("acme"))
        .content(organization("acme", "Acme Corp", 10))
        .return_none()
        .execute(&db)
        .await
        .expect("inserted");

    // An existing record that does not match is neither written nor returned.
    let unmatched = upsert::<Organization>(Organization::id("acme"))
        .set(org.age.equal_to(99u8))
        .where_(org.age.gt(50))
        .execute(&db)
        .await
        .expect("ran");
    assert!(unmatched.is_empty());
    let age: u8 = read(&db, "RETURN organization:acme.age;").await;
    assert_eq!(age, 10);

    // A table with no matching record creates one: the condition is not evaluated on the create
    // path.
    let created = upsert::<Organization>(Organization::table())
        .set([org.name.equal_to("Globex"), org.age.equal_to(1u8)])
        .where_(org.age.gt(50))
        .return_value(Organization::schema().name)
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(created, ["Globex"]);
}

#[tokio::test]
async fn explain_writes_nothing() {
    let db = db().await;
    let org = Organization::schema();
    let plan = upsert::<Organization>(Organization::table())
        .set(org.name.equal_to("Acme Corp"))
        .explain()
        .execute(&db)
        .await
        .expect("explained");
    assert!(!plan.is_empty());
    assert!(names(&db).await.is_empty());
}
