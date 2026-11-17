//! `SHOW CHANGES` (`orm/src/statements/show.rs`): rendering, and the typed change sets a
//! changefeed records, including the revert patches of `INCLUDE ORIGINAL`, whose direction is
//! current to previous.

use std::time::Duration;

use surrealdb::{
    Surreal,
    engine::local::Db,
    types::{Object, RecordId, SurrealValue, Value},
};
use umbriel::{
    Runnable,
    statements::{
        ChangeSet, DiffOp, TableChange, define_database, define_table, show_changes_for_database,
        show_changes_for_table,
    },
};
use umbriel_tests::*;

/// A record of the `reading` table.
#[derive(Debug, Clone, PartialEq, SurrealValue)]
#[surreal(crate = "umbriel::types")]
struct Reading {
    id: RecordId,
    name: String,
}

/// A type no `reading` record decodes into.
#[derive(Debug, Clone, PartialEq, SurrealValue)]
#[surreal(crate = "umbriel::types")]
struct Counter {
    count: i64,
}

const HOUR: Duration = Duration::from_secs(3600);

#[test]
fn either_target_renders() {
    assert_sql(
        &show_changes_for_table("reading", 3u64).limit(2),
        "SHOW CHANGES FOR TABLE reading SINCE 3 LIMIT 2;",
    );
    assert_sql(
        &show_changes_for_database(datetime()),
        "SHOW CHANGES FOR DATABASE SINCE d'2021-11-01T08:30:17Z';",
    );
}

/// Runs each statement as a query, and so a transaction, of its own.
async fn each(db: &Surreal<Db>, statements: &[&str]) {
    for statement in statements {
        assert!(succeeds(db, statement).await, "{statement}");
    }
}

/// The record changes of `sets`, without table definitions.
fn record_changes<R>(sets: Vec<ChangeSet<R>>) -> Vec<TableChange<R>> {
    sets.into_iter()
        .flat_map(|set| set.changes)
        .filter(|change| !matches!(change, TableChange::DefineTable(_)))
        .collect()
}

fn string(text: &str) -> Value {
    Value::String(text.to_owned())
}

#[tokio::test]
async fn include_original_records_patches_from_current_to_previous() {
    let db = db().await;
    define_table("reading")
        .changefeed(HOUR)
        .include_original()
        .run(&db)
        .await
        .expect("defined");
    each(
        &db,
        &[
            "CREATE reading:alice SET name = 'Alice';",
            "UPDATE reading:alice SET name = 'Alicia';",
            "UPDATE reading:alice SET note = 'Acme Corp';",
            "UPDATE reading:alice UNSET note;",
            "DELETE reading:alice;",
        ],
    )
    .await;

    let sets = show_changes_for_table("reading", 0u64)
        .execute::<Object>(&db)
        .await
        .expect("the changes decode");
    let changes = record_changes(sets);
    let [create, renamed, noted, unnoted, deleted] = changes.as_slice() else {
        panic!("five record changes: {changes:#?}");
    };

    // A create has nothing to revert to.
    assert!(matches!(create, TableChange::Update(_)), "{create:?}");
    let revert = |change: &TableChange<Object>| match change {
        TableChange::Updated { revert, .. } => revert.clone(),
        other => panic!("an update with its revert patch: {other:?}"),
    };
    // Each patch turns the record after the change back into the one before it. 3.3.4 reverts a
    // changed string with a text patch, in the `diff-match-patch` format.
    assert_eq!(
        revert(renamed),
        [DiffOp::Change {
            path: "/name".to_owned(),
            value: "@@ -1,6 +1,5 @@\n Alic\n-ia\n+e\n".to_owned(),
        }]
    );
    assert_eq!(
        revert(noted),
        [DiffOp::Remove {
            path: "/note".to_owned(),
        }]
    );
    assert_eq!(
        revert(unnoted),
        [DiffOp::Add {
            path: "/note".to_owned(),
            value: string("Acme Corp"),
        }]
    );
    let TableChange::Updated { current, .. } = renamed else {
        unreachable!("checked above");
    };
    assert_eq!(current.get("name"), Some(&string("Alicia")));

    match deleted {
        TableChange::Delete { id, original } => {
            assert_eq!(id, &RecordId::new("reading", "alice"));
            let original = original
                .as_ref()
                .expect("INCLUDE ORIGINAL keeps the record");
            assert_eq!(original.get("name"), Some(&string("Alicia")));
        }
        other => panic!("a delete: {other:?}"),
    }
}

#[tokio::test]
async fn without_include_original_updates_carry_the_record_only() {
    let db = db().await;
    define_table("reading")
        .changefeed(HOUR)
        .run(&db)
        .await
        .expect("defined");
    each(
        &db,
        &[
            "CREATE reading:alice SET name = 'Alice';",
            "UPDATE reading:alice SET name = 'Alicia';",
            "DELETE reading:alice;",
        ],
    )
    .await;

    let changes = record_changes(
        show_changes_for_table("reading", 0u64)
            .execute::<Reading>(&db)
            .await
            .expect("the records decode as `Reading`"),
    );
    let alicia = Reading {
        id: RecordId::new("reading", "alice"),
        name: "Alicia".to_owned(),
    };
    assert!(
        matches!(changes.as_slice(), [
            TableChange::Update(_),
            TableChange::Update(updated),
            TableChange::Delete { original: None, .. },
        ] if updated == &alicia),
        "{changes:#?}"
    );
}

#[tokio::test]
async fn since_and_limit_select_the_change_sets() {
    let db = db().await;
    define_table("reading")
        .changefeed(HOUR)
        .run(&db)
        .await
        .expect("defined");
    each(
        &db,
        &[
            "CREATE reading:alice SET name = 'Alice';",
            "CREATE reading:bob SET name = 'Bob';",
        ],
    )
    .await;

    let all = show_changes_for_table("reading", 0u64)
        .execute::<Object>(&db)
        .await
        .expect("decodes");
    let last = all.last().expect("change sets").versionstamp;
    let since = show_changes_for_table("reading", last)
        .execute::<Object>(&db)
        .await
        .expect("decodes");
    assert_eq!(
        since.iter().map(|set| set.versionstamp).collect::<Vec<_>>(),
        [last]
    );
    let limited = show_changes_for_table("reading", 0u64)
        .limit(1)
        .execute::<Object>(&db)
        .await
        .expect("decodes");
    assert_eq!(limited.len(), 1);
    // Pins an upstream limitation of the in-memory engine: it turns a `SINCE` time into
    // nanoseconds since the epoch, which lie past every versionstamp it has issued, so even a
    // time before every change selects nothing. Selection by time is not verified here.
    let by_time = show_changes_for_table("reading", datetime())
        .execute::<Object>(&db)
        .await
        .expect("decodes");
    assert!(by_time.is_empty(), "{by_time:#?}");
}

#[tokio::test]
async fn a_database_changefeed_spans_its_tables() {
    let db = db().await;
    define_database("test")
        .overwrite()
        .changefeed(HOUR)
        .run(&db)
        .await
        .expect("defined");
    each(
        &db,
        &[
            "CREATE reading:alice SET name = 'Alice';",
            "CREATE company:acme SET name = 'Acme Corp';",
        ],
    )
    .await;

    let changes = record_changes(
        show_changes_for_database(0u64)
            .execute::<Object>(&db)
            .await
            .expect("decodes"),
    );
    let tables = changes
        .iter()
        .map(|change| match change {
            TableChange::Update(record) => match record.get("id") {
                Some(Value::RecordId(id)) => id.table.to_string(),
                other => panic!("a record id: {other:?}"),
            },
            other => panic!("a create: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(tables, ["reading", "company"]);
}

#[tokio::test]
async fn a_record_that_does_not_decode_fails() {
    let db = db().await;
    define_table("reading")
        .changefeed(HOUR)
        .run(&db)
        .await
        .expect("defined");
    each(&db, &["CREATE reading:alice SET name = 'Alice';"]).await;

    let error = show_changes_for_table("reading", 0u64)
        .execute::<Counter>(&db)
        .await
        .expect_err("a reading is not a counter");
    assert!(matches!(error, umbriel::Error::Conversion(_)), "{error:?}");
}
