//! `REBUILD INDEX` (`orm/src/statements/rebuild.rs`): its rendering, and what SurrealDB does with
//! it.

use umbriel::statements::{alter_index, rebuild_index};
use umbriel_tests::{support::statements::*, *};

#[test]
fn rebuild_renders_every_option() {
    assert_sql(
        &rebuild_index("note_body").on_table(Table::from("note")),
        "REBUILD INDEX note_body ON TABLE note;",
    );
    assert_sql(
        &rebuild_index("note_body")
            .on_table(Table::from("note"))
            .if_exists()
            .concurrently(),
        "REBUILD INDEX IF EXISTS note_body ON TABLE note CONCURRENTLY;",
    );
    assert!(
        rebuild_index("note_body").to_query().is_err(),
        "the table is required"
    );
}

/// A note under a full-text index over an analyzer that keeps case.
async fn notes(db: &Surreal<Any>) {
    run(
        db,
        Raw::new(
            "DEFINE ANALYZER words TOKENIZERS blank; \
             DEFINE INDEX note_body ON note FIELDS body FULLTEXT ANALYZER words BM25; \
             CREATE note:one SET body = 'Hello';",
        ),
    )
    .await;
}

#[tokio::test]
async fn rebuilding_reindexes_existing_records() {
    let (db, _guard) = db().await;
    notes(&db).await;
    let lowercase_match = "RETURN array::len(SELECT id FROM note WHERE body @@ 'hello');";
    // The index holds `Hello`; the redefined analyzer lowercases the query to `hello`.
    run(
        &db,
        Raw::new("DEFINE ANALYZER OVERWRITE words TOKENIZERS blank FILTERS lowercase;"),
    )
    .await;
    let before: i64 = read(&db, lowercase_match).await;
    assert_eq!(
        before, 0,
        "redefining the analyzer leaves the index as it was"
    );
    run(
        &db,
        rebuild_index("note_body").on_table(Table::from("note")),
    )
    .await;
    let after: i64 = read(&db, lowercase_match).await;
    assert_eq!(after, 1, "the rebuilt index holds `hello`");
}

#[tokio::test]
async fn rebuilding_a_missing_index_with_if_exists_is_a_no_op() {
    let (db, _guard) = db().await;
    run(&db, Raw::new("DEFINE TABLE note;")).await;
    run(
        &db,
        rebuild_index("missing")
            .on_table(Table::from("note"))
            .if_exists(),
    )
    .await;
}

#[tokio::test]
async fn without_if_exists_fails() {
    let (db, _guard) = db().await;
    run(&db, Raw::new("DEFINE TABLE note;")).await;
    assert!(
        rebuild_index("missing")
            .on_table(Table::from("note"))
            .run(&db)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rebuilding_an_index_marked_for_removal_fails() {
    let (db, _guard) = db().await;
    notes(&db).await;
    run(
        &db,
        alter_index("note_body")
            .on_table(Table::from("note"))
            .prepare_remove(),
    )
    .await;
    assert!(
        rebuild_index("note_body")
            .on_table(Table::from("note"))
            .run(&db)
            .await
            .is_err()
    );
}
