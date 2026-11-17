//! Buckets (`orm/src/statements/define_bucket.rs`, and `alter_bucket`/`remove_bucket`):
//! rendering, checked with SurrealDB's parser, and their effect on a database that allows the
//! experimental `files` feature.

use umbriel::{
    Field, Operatable, Param, Runnable,
    statements::{alter_bucket, define_bucket, remove_bucket},
};
use umbriel_tests::*;

#[test]
fn a_definition_writes_its_clauses() {
    assert_sql(&define_bucket("avatars"), "DEFINE BUCKET avatars;");
    assert_sql(
        &define_bucket("avatars")
            .overwrite()
            .backend("memory")
            .permissions_where(Field::new("owner").is(Param::new("auth")))
            .readonly()
            .comment("Acme Corp avatars"),
        "DEFINE BUCKET OVERWRITE avatars BACKEND 'memory' PERMISSIONS WHERE owner IS $auth \
         READONLY COMMENT 'Acme Corp avatars';",
    );
    assert_sql(
        &define_bucket(Param::new("name"))
            .if_not_exists()
            .permissions_none(),
        "DEFINE BUCKET IF NOT EXISTS $name PERMISSIONS NONE;",
    );
}

#[test]
fn alter_sets_and_drops() {
    assert_sql(
        &alter_bucket("avatars")
            .if_exists()
            .readonly()
            .drop_backend()
            .permissions_full()
            .comment("read only"),
        "ALTER BUCKET IF EXISTS avatars READONLY DROP BACKEND PERMISSIONS FULL COMMENT 'read only';",
    );
    assert_sql(
        &alter_bucket("avatars")
            .drop_readonly()
            .backend("memory")
            .drop_comment(),
        "ALTER BUCKET avatars DROP READONLY BACKEND 'memory' DROP COMMENT;",
    );
    assert_render_error(
        &alter_bucket("avatars").readonly().drop_readonly(),
        "both set and drop READONLY",
    );
}

#[test]
fn remove_takes_if_exists() {
    assert_sql(
        &remove_bucket("avatars").if_exists(),
        "REMOVE BUCKET IF EXISTS avatars;",
    );
}

#[tokio::test]
async fn a_bucket_is_defined_altered_and_removed() {
    let db = experimental_db().await;
    define_bucket("avatars")
        .backend("memory")
        .comment("Acme Corp avatars")
        .run(&db)
        .await
        .expect("defined");
    let bucket = stored_definition(&db, "INFO FOR DB;", "buckets", "avatars").await;
    assert!(bucket.contains("COMMENT 'Acme Corp avatars'"), "{bucket}");

    alter_bucket("avatars")
        .readonly()
        .drop_comment()
        .run(&db)
        .await
        .expect("altered");
    let bucket = stored_definition(&db, "INFO FOR DB;", "buckets", "avatars").await;
    assert!(bucket.contains("READONLY"), "{bucket}");
    assert!(!bucket.contains("COMMENT"), "{bucket}");

    remove_bucket("avatars").run(&db).await.expect("removed");
    assert!(remove_bucket("avatars").run(&db).await.is_err());
}

#[tokio::test]
async fn a_bucket_needs_the_files_feature() {
    let db = db().await;
    let error = define_bucket("avatars")
        .backend("memory")
        .run(&db)
        .await
        .expect_err("files are experimental")
        .to_string();
    assert!(error.contains("files"), "{error}");
}
