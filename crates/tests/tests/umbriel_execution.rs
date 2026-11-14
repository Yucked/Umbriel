//! Execution (`orm/src/execution.rs`, #29): `Runnable::run_with_stats`, the session each handle
//! runs on, and a shared client borrowed through `Executor` without cloning it. Response checking
//! itself is tested in `execution.rs`.

use std::sync::Arc;

use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    All, Error, Model, Raw, Runnable, SurrealCrud, SurrealCrudNode, SurrealRead,
    statements::{Returning, return_, select},
};
use umbriel_tests::*;

/// The namespace and database `db`'s session uses.
async fn selection(db: &Surreal<Db>) -> Vec<Option<String>> {
    Raw::new("RETURN [session::ns(), session::db()];")
        .get_data(db)
        .await
        .expect("ran")
        .expect("a selection")
}

fn selected(namespace: &str, database: &str) -> Vec<Option<String>> {
    vec![Some(namespace.to_owned()), Some(database.to_owned())]
}

mod stats {
    use super::*;

    #[tokio::test]
    async fn each_statement_has_its_statistics() {
        let db = db().await;
        let mut response = Raw::new("RETURN 1; RETURN 2;")
            .run_with_stats(&db)
            .await
            .expect("ran");
        for index in 0..2 {
            let (stats, value) = response.take::<Option<i64>>(index).expect("a statement");
            assert_eq!(value.expect("decoded"), Some(index as i64 + 1));
            assert!(stats.execution_time.is_some(), "statement {index}");
        }
    }

    #[tokio::test]
    async fn the_response_is_checked_as_run_checks_it() {
        let db = db().await;
        let error = Raw::new("RETURN 1; THROW 'second';")
            .run_with_stats(&db)
            .await
            .expect_err("a failed statement");
        assert!(
            matches!(&error, Error::Statement { index: 1, .. }),
            "{error:?}"
        );
    }
}

mod sessions {
    use super::*;

    /// A clone's session is its own: changing the selection of one through the SDK does not move
    /// the other. (A `USE` statement never changes a session; see `umbriel_use.rs`.)
    #[tokio::test]
    async fn cloned_handles_keep_their_selection_independent() {
        let db = db().await;
        let other = db.clone();
        other
            .use_ns("acme")
            .use_db("sales")
            .await
            .expect("selected");
        db.use_db("support").await.expect("selected");
        assert_eq!(selection(&db).await, selected("test", "support"));
        assert_eq!(selection(&other).await, selected("acme", "sales"));
    }

    /// A clone starts from its original's session: the embedded engine copies it
    /// (`surrealdb-engine-local` `session.rs:56`).
    #[tokio::test]
    async fn a_clone_starts_from_the_original_selection() {
        let db = db().await;
        db.use_ns("acme").use_db("sales").await.expect("selected");
        let clone = db.clone();
        assert_eq!(selection(&clone).await, selected("acme", "sales"));
    }
}

/// A client shared as `Arc<Surreal<_>>`, borrowed with `as_ref()`.
///
/// That no execution API clones the SDK client is shown two ways. Statement execution and
/// `Function::run` are checked against the session id: each clone gets a session id of its own
/// (`surrealdb-engine-local` `session.rs:58`), so a query run on a clone would report a
/// different `session::id()` than the borrowed handle. Every execution API, model methods and
/// `TableOf` included, is generic over `Executor`, which has no `Clone` bound, so none can clone
/// the handle (`ui/fail/executor_is_not_clone.rs`).
mod shared_client {
    use super::*;
    use umbriel::{functions::session, types::Value};

    const SESSION_ID: &str = "RETURN session::id();";

    #[tokio::test]
    async fn statements_run_on_the_borrowed_session() {
        let shared = Arc::new(db().await);
        let db = shared.as_ref();
        let own: Value = read(db, SESSION_ID).await;

        // The check can tell a clone apart: a clone reports a session id of its own.
        let clone = db.clone();
        let cloned: Value = read(&clone, SESSION_ID).await;
        assert_ne!(cloned, own, "a clone has its own session id");

        let via_run: Value = Raw::new(SESSION_ID)
            .run(db)
            .await
            .expect("ran")
            .take(0)
            .expect("an id");
        assert_eq!(via_run, own, "run");

        let via_get_data: Option<Value> = Raw::new(SESSION_ID).get_data(db).await.expect("ran");
        assert_eq!(via_get_data.unwrap_or(Value::None), own, "get_data");

        let (_, via_stats) = Raw::new(SESSION_ID)
            .run_with_stats(db)
            .await
            .expect("ran")
            .take::<Value>(0)
            .expect("a statement");
        assert_eq!(via_stats.expect("an id"), own, "run_with_stats");

        let via_function: Option<Value> = session::id!().run(db).await.expect("ran");
        assert_eq!(via_function.unwrap_or(Value::None), own, "Function::run");
    }

    /// Compatibility: every execution API accepts the borrowed client.
    #[tokio::test]
    async fn statements_and_model_methods_run_on_a_borrowed_arc() {
        let shared = Arc::new(db().await);
        let db = shared.as_ref();

        let apollo = SpaceShip {
            id: SpaceShip::id("apollo"),
            name: "Apollo".to_owned(),
            ..Default::default()
        };
        apollo.create(db).await.expect("created");
        SpaceShip::insert_many(
            db,
            vec![SpaceShip {
                id: SpaceShip::id("gemini"),
                name: "Gemini".to_owned(),
                ..Default::default()
            }],
        )
        .await
        .expect("inserted");
        let stored = select(All)
            .from(SpaceShip::table())
            .decode::<SpaceShip>()
            .execute(db)
            .await
            .expect("read");
        assert_eq!(stored.len(), 2);
        assert!(
            SpaceShip::get(db, &SpaceShip::id("apollo"))
                .await
                .expect("read")
                .is_some()
        );
        let two: Option<i64> = return_(2).get_data(db).await.expect("ran");
        assert_eq!(two, Some(2));
    }

    /// The execution futures are `Send`, so a borrowed shared client's work can run on a
    /// multi-threaded runtime's tasks.
    #[tokio::test]
    async fn execution_futures_are_send() {
        fn assert_send<F: Future + Send>(future: F) -> F {
            future
        }
        let shared = Arc::new(db().await);
        let task = {
            let shared = Arc::clone(&shared);
            tokio::spawn(async move {
                let db = shared.as_ref();
                // The table is defined first: SurrealDB 3.3.4 refuses to read an undefined one.
                assert_send(Raw::new("DEFINE TABLE space_ship SCHEMALESS;").run(db))
                    .await
                    .expect("ran");
                assert_send(SpaceShip::get(db, &SpaceShip::id("apollo")))
                    .await
                    .expect("read")
            })
        };
        assert!(task.await.expect("the task finished").is_none());
    }
}
