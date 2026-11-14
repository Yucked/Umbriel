//! Transactions (#17): Umbriel statements and model methods run inside an SDK transaction handle
//! (`umbriel::Transaction`, an `Executor`), with its commit, cancellation, statement failures,
//! drop and abandonment on the embedded engine. Query transactions (`begin_transaction`) are
//! tested in `statements.rs` (`control_flow`) and `execution.rs`; their conflicts in
//! `umbriel_errors.rs`; handles over WebSocket and HTTP in `server.rs`.

use std::time::Duration;

use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    All, Error, Model, Raw, Retry, Runnable, SurrealCrud, SurrealCrudNode, SurrealRead,
    statements::{Returning, select},
};
use umbriel_tests::*;

fn ship(key: &str, name: &str) -> SpaceShip {
    SpaceShip {
        id: SpaceShip::id(key),
        name: name.to_owned(),
        ..Default::default()
    }
}

/// A database with the ship table defined: SurrealDB 3.3.4 refuses to read a table that was
/// never defined, so a read before the first commit would otherwise fail.
async fn ships() -> Surreal<Db> {
    let db = db().await;
    Raw::new("DEFINE TABLE space_ship SCHEMALESS;")
        .run(&db)
        .await
        .expect("table defined");
    db
}

/// The stored ships' names, read on the client rather than through a transaction.
async fn names(db: &Surreal<Db>) -> Vec<String> {
    read(db, "SELECT VALUE name FROM space_ship ORDER BY name;").await
}

/// Statements and model methods write inside the transaction: the client sees nothing until it
/// commits. `begin`, `commit` and `cancel` errors convert with `?`.
async fn create_in_a_transaction(db: &Surreal<Db>) -> umbriel::Result<Vec<String>> {
    let tx = db.clone().begin().await?;
    ship("apollo", "Apollo").create(&tx).await?;
    // `created` too: the read below decodes every record as a `SpaceShip`.
    Raw::new("CREATE space_ship:gemini SET name = 'Gemini', created = d'2026-01-01T00:00:00Z';")
        .run(&tx)
        .await?;
    let inside = select(All)
        .from(SpaceShip::table())
        .decode::<SpaceShip>()
        .execute(&tx)
        .await?;
    assert_eq!(inside.len(), 2, "the transaction reads its own writes");
    assert!(
        names(db).await.is_empty(),
        "nothing is visible before commit"
    );
    tx.commit().await?;
    Ok(names(db).await)
}

#[tokio::test]
async fn statements_and_model_methods_commit_together() {
    let db = ships().await;
    let committed = create_in_a_transaction(&db).await.expect("committed");
    assert_eq!(committed, ["Apollo", "Gemini"]);
}

#[tokio::test]
async fn insert_many_runs_inside_a_transaction() {
    let db = ships().await;
    let tx = db.clone().begin().await.expect("begins");
    let inserted = SpaceShip::insert_many(
        &tx,
        vec![ship("apollo", "Apollo"), ship("gemini", "Gemini")],
    )
    .await
    .expect("inserted");
    assert_eq!(inserted.len(), 2);
    assert!(names(&db).await.is_empty());
    tx.commit().await.expect("committed");
    assert_eq!(names(&db).await, ["Apollo", "Gemini"]);
}

#[tokio::test]
async fn reads_and_writes_by_id_run_inside_a_transaction() {
    let db = ships().await;
    ship("apollo", "Apollo").create(&db).await.expect("created");
    let tx = db.clone().begin().await.expect("begins");
    let mut apollo = SpaceShip::get(&tx, &SpaceShip::id("apollo"))
        .await
        .expect("read")
        .expect("apollo exists");
    apollo.name = "Apollo 11".to_owned();
    apollo.save(&tx).await.expect("saved");
    SpaceShip::delete_by_id(&tx, &SpaceShip::id("apollo"))
        .await
        .expect("deleted");
    assert_eq!(
        names(&db).await,
        ["Apollo"],
        "the client still sees the original"
    );
    tx.commit().await.expect("committed");
    assert!(names(&db).await.is_empty());
}

#[tokio::test]
async fn a_cancelled_transaction_persists_nothing() {
    let db = ships().await;
    let tx = db.clone().begin().await.expect("begins");
    ship("apollo", "Apollo").create(&tx).await.expect("created");
    SpaceShip::insert_many(&tx, vec![ship("gemini", "Gemini")])
        .await
        .expect("inserted");
    tx.cancel().await.expect("cancelled");
    assert!(names(&db).await.is_empty());
}

/// Recorded behaviour of the embedded engine: a failing statement is reported as the query's
/// failure, and leaves the transaction to the caller (`surrealdb-engine-local` `engine.rs:361`),
/// so a commit after it keeps the writes that succeeded.
#[tokio::test]
async fn a_failing_statement_is_reported_and_the_transaction_stays_open() {
    let db = ships().await;
    let tx = db.clone().begin().await.expect("begins");
    ship("apollo", "Apollo").create(&tx).await.expect("created");
    let (index, source, _) = statement_error(Raw::new("RETURN 1; THROW 'refused';").run(&tx).await);
    assert_eq!(index, 1);
    assert!(source.to_string().contains("refused"), "{source}");
    tx.commit().await.expect("the transaction still commits");
    assert_eq!(names(&db).await, ["Apollo"]);
}

/// A handle dropped without `commit` or `cancel` commits nothing, and reports nothing. This
/// shows only that its writes stay invisible: whether the open transaction is then released
/// cannot be observed through SDK 3.3.4's public API (the embedded engine keeps it in private
/// session state, `surrealdb-engine-local` `session.rs:21`), so that cleanup is unverified.
#[tokio::test]
async fn a_dropped_handle_commits_nothing() {
    let db = ships().await;
    {
        let tx = db.clone().begin().await.expect("begins");
        ship("apollo", "Apollo").create(&tx).await.expect("created");
    }
    assert!(names(&db).await.is_empty());
}

/// A query future dropped while it runs marks its transaction abandoned: the next `commit` rolls
/// it back and fails (`surrealdb-engine-local` `engine.rs:661`).
///
/// The query writes and then sleeps for an hour, so it is still running when it is dropped. The
/// query and a read loop through the same transaction run concurrently, each progressing, until
/// the read sees the query's write; only then is the query dropped, so the drop happens after
/// the write, inside the transaction. A deadline fails the test clearly if the write is never
/// seen. The commit error has no structured kind, so its message is the only way to tell the
/// abandonment apart from another internal failure.
#[tokio::test]
async fn an_abandoned_query_makes_the_commit_fail() {
    let db = ships().await;
    let tx = db.clone().begin().await.expect("begins");
    // `created` too: the read loop decodes the record as a `SpaceShip`.
    let query = Raw::new(
        "CREATE space_ship:apollo SET name = 'Apollo', created = d'2026-01-01T00:00:00Z'; \
         SLEEP 1h;",
    );
    let written = async {
        while SpaceShip::get(&tx, &SpaceShip::id("apollo"))
            .await
            .expect("read inside the transaction")
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    };
    // `select!` polls both futures each time it is woken, and drops the query once the read has
    // seen its write.
    tokio::time::timeout(Duration::from_secs(10), async {
        tokio::select! {
            () = written => {}
            finished = query.run(&tx) => {
                panic!("the query finished instead of sleeping: {finished:?}")
            }
        }
    })
    .await
    .expect("the read sees the query's write within the deadline");
    let error = Error::from(tx.commit().await.expect_err("the commit fails"));
    assert!(
        matches!(
            &error,
            Error::Query(source)
                if source.is_internal() && source.to_string().contains("abandoned")
        ),
        "{error:?}"
    );
    assert_eq!(error.retry(), Retry::Uncertain);
    assert!(
        names(&db).await.is_empty(),
        "the write made inside the abandoned transaction is rolled back"
    );
}
