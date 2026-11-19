//! `DEFINE DATABASE` (`orm/src/statements/define_database.rs`): `STRICT`, and how `OVERWRITE`
//! changes it, since no `ALTER` does.

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{Runnable, statements::define_database};
use umbriel_tests::*;

async fn run(db: &Surreal<Any>, statement: impl Runnable + Sync) {
    statement.run(db).await.expect("ran");
}

/// Whether `CREATE person` succeeds in the database `name`.
async fn creates_undefined_table(db: &Surreal<Any>, name: &str) -> bool {
    db.use_db(name).await.expect("database selected");
    db.query("CREATE person:alice;")
        .await
        .and_then(surrealdb::IndexedResults::check)
        .is_ok()
}

#[test]
fn strict_renders_after_the_name() {
    assert_sql(
        &define_database("acme").strict(),
        "DEFINE DATABASE acme STRICT;",
    );
    assert_sql(
        &define_database("acme")
            .if_not_exists()
            .strict()
            .comment("Acme Corp"),
        "DEFINE DATABASE IF NOT EXISTS acme STRICT COMMENT 'Acme Corp';",
    );
}

#[test]
fn a_changefeed_renders_with_its_expiry() {
    assert_sql(
        &define_database("acme")
            .changefeed(std::time::Duration::from_secs(86_400))
            .include_original(),
        "DEFINE DATABASE acme CHANGEFEED 1d INCLUDE ORIGINAL;",
    );
    assert_render_error(
        &define_database("acme").include_original(),
        "INCLUDE ORIGINAL needs a CHANGEFEED",
    );
}

#[tokio::test]
async fn the_database_stores_its_changefeed() {
    let (db, _guard) = db().await;
    run(
        &db,
        define_database("acme").changefeed(std::time::Duration::from_secs(3600)),
    )
    .await;
    let info: surrealdb::types::Object = read(&db, "INFO FOR NS;").await;
    let Some(surrealdb::types::Value::Object(databases)) = info.get("databases") else {
        panic!("`databases` is listed: {info:?}");
    };
    match databases.get("acme") {
        Some(surrealdb::types::Value::String(sql)) => {
            assert!(sql.contains("CHANGEFEED 1h"), "{sql}");
        }
        other => panic!("`acme` is defined: {other:?}"),
    }
}

#[tokio::test]
async fn a_strict_database_refuses_undefined_tables() {
    let (db, _guard) = db().await;
    run(&db, define_database("acme").strict()).await;
    assert!(!creates_undefined_table(&db, "acme").await);
}

#[tokio::test]
async fn overwrite_without_strict_lifts_it() {
    let (db, _guard) = db().await;
    run(&db, define_database("acme").strict()).await;
    run(&db, define_database("acme").overwrite()).await;
    assert!(creates_undefined_table(&db, "acme").await);
}
