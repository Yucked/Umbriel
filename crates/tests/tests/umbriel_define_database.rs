//! `DEFINE DATABASE` (`orm/src/statements/define_database.rs`): `STRICT`, and how `OVERWRITE`
//! changes it, since no `ALTER` does.

use surrealdb::{Surreal, engine::local::Db};
use umbriel::{Runnable, statements::define_database};
use umbriel_tests::*;

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("ran");
}

/// Whether `CREATE person` succeeds in the database `name`.
async fn creates_undefined_table(db: &Surreal<Db>, name: &str) -> bool {
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

#[tokio::test]
async fn a_strict_database_refuses_undefined_tables() {
    let db = db().await;
    run(&db, define_database("acme").strict()).await;
    assert!(!creates_undefined_table(&db, "acme").await);
}

#[tokio::test]
async fn overwrite_without_strict_lifts_it() {
    let db = db().await;
    run(&db, define_database("acme").strict()).await;
    run(&db, define_database("acme").overwrite()).await;
    assert!(creates_undefined_table(&db, "acme").await);
}
