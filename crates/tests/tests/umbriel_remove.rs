//! `REMOVE` statements (`orm/src/statements/remove.rs`): `IF EXISTS` on every kind, and names read
//! from parameters.

use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    Field, Param, Runnable,
    statements::{
        remove_access, remove_analyzer, remove_database, remove_event, remove_field,
        remove_function, remove_index, remove_namespace, remove_param, remove_table, remove_user,
    },
};
use umbriel_tests::*;

#[test]
fn every_removal_takes_if_exists() {
    assert_sql(
        &remove_namespace("acme").if_exists(),
        "REMOVE NAMESPACE IF EXISTS acme;",
    );
    assert_sql(
        &remove_database("acme").if_exists(),
        "REMOVE DATABASE IF EXISTS acme;",
    );
    assert_sql(&remove_table("user").if_exists(), "REMOVE TABLE IF EXISTS user;");
    assert_sql(
        &remove_field(Field::new("email")).on_table("user").if_exists(),
        "REMOVE FIELD IF EXISTS email ON TABLE user;",
    );
    assert_sql(
        &remove_index("user_email").on_table("user").if_exists(),
        "REMOVE INDEX IF EXISTS user_email ON TABLE user;",
    );
    assert_sql(
        &remove_event("audit").on_table("user").if_exists(),
        "REMOVE EVENT IF EXISTS audit ON TABLE user;",
    );
    assert_sql(
        &remove_analyzer("simple").if_exists(),
        "REMOVE ANALYZER IF EXISTS simple;",
    );
    assert_sql(
        &remove_param("greeting").if_exists(),
        "REMOVE PARAM IF EXISTS $greeting;",
    );
    assert_sql(
        &remove_function("fn::greet").if_exists(),
        "REMOVE FUNCTION IF EXISTS fn::greet;",
    );
    assert_sql(
        &remove_user("alice").on_database().if_exists(),
        "REMOVE USER IF EXISTS alice ON DATABASE;",
    );
    assert_sql(
        &remove_access("api").on_database().if_exists(),
        "REMOVE ACCESS IF EXISTS api ON DATABASE;",
    );
}

#[test]
fn names_come_from_parameters() {
    let name = Param::new("name");
    let table = Param::new("table");
    assert_sql(&remove_namespace(&name), "REMOVE NAMESPACE $name;");
    assert_sql(&remove_database(&name), "REMOVE DATABASE $name;");
    assert_sql(&remove_table(&table), "REMOVE TABLE $table;");
    assert_sql(
        &remove_field(Field::new("email")).on_table(&table),
        "REMOVE FIELD email ON TABLE $table;",
    );
    assert_sql(
        &remove_index(&name).on_table(&table),
        "REMOVE INDEX $name ON TABLE $table;",
    );
    assert_sql(
        &remove_event(&name).on_table(&table),
        "REMOVE EVENT $name ON TABLE $table;",
    );
    assert_sql(&remove_analyzer(&name), "REMOVE ANALYZER $name;");
    assert_sql(
        &remove_user(&name).on_database(),
        "REMOVE USER $name ON DATABASE;",
    );
    assert_sql(
        &remove_access(&name).on_database(),
        "REMOVE ACCESS $name ON DATABASE;",
    );
}

/// Runs `statement` and reports whether it succeeded.
async fn runs(db: &Surreal<Db>, statement: impl Runnable + Sync) -> bool {
    statement.run(db.clone()).await.is_ok()
}

#[tokio::test]
async fn if_exists_removes_nothing_without_failing() {
    let db = db().await;
    db.query("DEFINE TABLE user;")
        .await
        .and_then(surrealdb::IndexedResults::check)
        .expect("table defined");

    assert!(runs(&db, remove_namespace("acme").if_exists()).await);
    assert!(runs(&db, remove_database("acme").if_exists()).await);
    assert!(runs(&db, remove_table("missing").if_exists()).await);
    assert!(
        runs(
            &db,
            remove_field(Field::new("email")).on_table("user").if_exists()
        )
        .await
    );
    assert!(runs(&db, remove_index("user_email").on_table("user").if_exists()).await);
    assert!(runs(&db, remove_event("audit").on_table("user").if_exists()).await);
    assert!(runs(&db, remove_analyzer("simple").if_exists()).await);
    assert!(runs(&db, remove_param("greeting").if_exists()).await);
    assert!(runs(&db, remove_function("fn::greet").if_exists()).await);
    assert!(runs(&db, remove_user("alice").on_database().if_exists()).await);
    assert!(runs(&db, remove_access("api").on_database().if_exists()).await);
}

#[tokio::test]
async fn without_if_exists_a_missing_resource_fails() {
    let db = db().await;
    assert!(!runs(&db, remove_table("missing")).await);
    assert!(!runs(&db, remove_param("greeting")).await);
    assert!(!runs(&db, remove_analyzer("simple")).await);
}
