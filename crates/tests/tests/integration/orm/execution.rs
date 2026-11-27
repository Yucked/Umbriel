//! Execution (`orm/src/execution.rs`, #29): `Runnable::run_with_stats`, the session each handle
//! runs on, a shared client borrowed through `Executor` without cloning it, response checking
//! (`checking`), and SDK transaction handles (`sdk_transactions`).

use std::sync::Arc;

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{
    All, Error, Model, Raw, Runnable, SurrealCrud, SurrealCrudNode, SurrealRead,
    statements::{Returning, return_, select},
};
use umbriel_tests::*;

/// The namespace and database `db`'s session uses.
async fn selection(db: &Surreal<Any>) -> Vec<Option<String>> {
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
    /// the other. (A `USE` statement never changes a session; see `orm/statements/use_.rs`.) The
    /// selections are inside a namespace of the test's own, so nothing outside it is touched.
    #[tokio::test]
    async fn cloned_handles_keep_their_selection_independent() {
        let (db, scratch) = namespace().await;
        let ns = scratch.namespace();
        let other = db.clone();
        other.use_ns(ns).use_db("sales").await.expect("selected");
        db.use_ns(ns).use_db("support").await.expect("selected");
        assert_eq!(selection(&db).await, selected(ns, "support"));
        assert_eq!(selection(&other).await, selected(ns, "sales"));
    }

    /// A clone starts from its original's session.
    #[tokio::test]
    async fn a_clone_starts_from_the_original_selection() {
        let (db, scratch) = namespace().await;
        let ns = scratch.namespace();
        db.use_ns(ns).use_db("sales").await.expect("selected");
        let clone = db.clone();
        assert_eq!(selection(&clone).await, selected(ns, "sales"));
    }
}

/// A client shared as `Arc<Surreal<_>>`, borrowed with `as_ref()`.
///
/// That no execution API clones the SDK client is shown two ways. Statement execution and
/// `Function::run` are checked against the session id: each clone gets a session id of its own
/// (asserted below), so a query run on a clone would report a different `session::id()` than the
/// borrowed handle. Every execution API, model methods and
/// `TableOf` included, is generic over `Executor`, which has no `Clone` bound, so none can clone
/// the handle (`ui/fail/executor_is_not_clone.rs`).
mod shared_client {
    use super::*;
    use umbriel::{functions::session, types::Value};

    const SESSION_ID: &str = "RETURN session::id();";

    #[tokio::test]
    async fn statements_run_on_the_borrowed_session() {
        let (db, _guard) = db().await;
        let shared = Arc::new(db);
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
        let (db, _guard) = db().await;
        let shared = Arc::new(db);
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
        let (db, _guard) = db().await;
        let shared = Arc::new(db);
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

/// Statement-result checking: `select_failure` on constructed result sequences, `execute`
/// through `Runnable::run`, and how `Runnable::get_data` decodes a first result.
mod checking {
    use std::time::Duration;

    use surrealdb::{Error as SdkError, Surreal, engine::any::Any, types::QueryError};
    use umbriel::{Error, Model, Raw, Runnable, SurrealCrudNode, test_support::select_failure};
    use umbriel_tests::{SpaceShip, db, statement_error};

    fn thrown(message: &str) -> SdkError {
        SdkError::thrown(message.to_owned())
    }

    fn not_executed(message: &str) -> SdkError {
        SdkError::query(message.to_owned(), QueryError::NotExecuted)
    }

    fn cancelled(message: &str) -> SdkError {
        SdkError::query(message.to_owned(), QueryError::Cancelled)
    }

    /// A query error without a kind, as SurrealDB reports a commit whose outcome is unknown.
    fn unknown_outcome(message: &str) -> SdkError {
        SdkError::query(message.to_owned(), None)
    }

    /// A statement's index and error message.
    type Indexed = (usize, String);

    /// The selected index, the selected message and the related `(index, message)` pairs.
    fn selected(
        statement_count: usize,
        errors: Vec<(usize, SdkError)>,
    ) -> Option<(usize, String, Vec<Indexed>)> {
        select_failure(statement_count, errors).map(|error| match error {
            Error::Statement {
                index,
                source,
                related,
            } => (
                index,
                source.to_string(),
                related
                    .into_iter()
                    .map(|(index, error)| (index, error.to_string()))
                    .collect(),
            ),
            other => panic!("expected a statement error, got {other:?}"),
        })
    }

    mod select_failure {
        use super::*;

        #[test]
        fn no_errors_is_success() {
            assert_eq!(selected(3, vec![]), None);
        }

        #[test]
        fn a_single_failure_is_selected() {
            assert_eq!(
                selected(3, vec![(1, thrown("second"))]),
                Some((1, "second".to_owned(), vec![]))
            );
        }

        #[test]
        fn multiple_failures_select_the_lowest_index_and_keep_the_rest() {
            let errors = vec![(2, thrown("third")), (0, thrown("first"))];
            assert_eq!(
                selected(3, errors),
                Some((0, "first".to_owned(), vec![(2, "third".to_owned())]))
            );
        }

        #[test]
        fn a_failure_is_selected_over_the_not_executed_rows_it_caused() {
            // A failed transaction: the statements around the cause are marked not executed.
            let errors = vec![
                (0, not_executed("rolled back")),
                (1, thrown("cause")),
                (2, not_executed("rolled back")),
            ];
            let (index, message, related) = selected(3, errors).expect("a failure");
            assert_eq!((index, message.as_str()), (1, "cause"));
            assert_eq!(related.len(), 2);
        }

        #[test]
        fn a_commit_failure_selects_the_last_row_of_the_run() {
            // The rollback rows, then the closing COMMIT carrying the cause.
            let errors = vec![
                (0, not_executed("rolled back")),
                (1, not_executed("rolled back")),
                (2, not_executed("Cannot COMMIT: cause")),
            ];
            let (index, message, related) = selected(3, errors).expect("a failure");
            assert_eq!((index, message.as_str()), (2, "Cannot COMMIT: cause"));
            assert_eq!(
                related.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
                [0, 1]
            );
        }

        #[test]
        fn an_unknown_commit_outcome_keeps_the_detailed_cause() {
            // Generic "outcome unknown" rows without a kind, then the detailed commit error.
            let errors = vec![
                (0, unknown_outcome("outcome unknown")),
                (1, unknown_outcome("outcome unknown")),
                (2, unknown_outcome("Cannot COMMIT: detailed cause")),
            ];
            let (_, message, related) = selected(3, errors).expect("a failure");
            let mut messages = related
                .into_iter()
                .map(|(_, message)| message)
                .collect::<Vec<_>>();
            messages.push(message);
            assert!(
                messages
                    .iter()
                    .any(|m| m == "Cannot COMMIT: detailed cause")
            );
        }

        #[test]
        fn an_intentional_cancel_followed_by_success_is_success() {
            // Statements 0 and 1 were cancelled; statement 2 is CANCEL's `NONE`.
            let errors = vec![(0, cancelled("cancelled")), (1, cancelled("cancelled"))];
            assert_eq!(selected(3, errors), None);
        }

        #[test]
        fn a_failure_after_an_intentional_cancel_is_selected() {
            // Cancelled rows, CANCEL's success at 2, then a failing statement at 3.
            let errors = vec![
                (0, cancelled("cancelled")),
                (1, cancelled("cancelled")),
                (3, thrown("after")),
            ];
            let (index, message, related) = selected(4, errors).expect("a failure");
            assert_eq!((index, message.as_str()), (3, "after"));
            assert_eq!(related.len(), 2);
        }

        #[test]
        fn cancellation_without_a_successful_closing_result_is_a_failure() {
            // A timeout or context cancellation also fails the closing statement.
            let errors = vec![(0, cancelled("cancelled")), (1, cancelled("cancelled"))];
            assert_eq!(
                selected(2, errors).map(|(index, ..)| index),
                Some(0),
                "no result follows the cancelled run"
            );

            let errors = vec![
                (0, cancelled("cancelled")),
                (1, cancelled("cancelled")),
                (2, cancelled("Cannot COMMIT: the transaction was cancelled")),
            ];
            assert_eq!(selected(3, errors).map(|(index, ..)| index), Some(0));
        }

        #[test]
        fn not_executed_mixed_with_an_intentional_cancel() {
            // A cancelled transaction, then a failed one whose rows were not executed.
            let errors = vec![
                (0, cancelled("cancelled")),
                (2, not_executed("rolled back")),
                (3, thrown("cause")),
            ];
            let (index, message, _) = selected(4, errors).expect("a failure");
            assert_eq!((index, message.as_str()), (3, "cause"));

            // Without the cause, the not-executed run is reported.
            let errors = vec![
                (0, cancelled("cancelled")),
                (2, not_executed("rolled back")),
            ];
            assert_eq!(selected(3, errors).map(|(index, ..)| index), Some(2));
        }

        #[test]
        fn timeouts_conflicts_and_unknown_outcomes_are_failures() {
            let timed_out = SdkError::query(
                "timed out".to_owned(),
                QueryError::TimedOut {
                    duration: Duration::from_millis(10),
                },
            );
            for error in [timed_out, unknown_outcome("unknown")] {
                assert_eq!(
                    selected(1, vec![(0, error)]).map(|(index, ..)| index),
                    Some(0)
                );
            }
            // A conflict is selected too, and classified as its own variant (#29).
            let conflict = SdkError::query("conflict".to_owned(), QueryError::TransactionConflict);
            let error = select_failure(1, vec![(0, conflict)]).expect("a failure");
            assert!(
                matches!(
                    &error,
                    Error::Conflict {
                        statement: Some(umbriel::StatementContext { index: 0, .. }),
                        ..
                    }
                ),
                "{error:?}"
            );
        }

        #[test]
        fn a_response_with_an_unexcused_failure_is_never_success() {
            let cases: Vec<(usize, Vec<(usize, SdkError)>)> = vec![
                (2, vec![(1, thrown("x"))]),
                (3, vec![(0, cancelled("c")), (1, thrown("x"))]),
                (2, vec![(0, not_executed("n"))]),
                (1, vec![(0, cancelled("c"))]),
            ];
            for (count, errors) in cases {
                assert!(selected(count, errors).is_some());
            }
        }
    }

    mod execute {
        use super::*;

        async fn define_person(db: &Surreal<Any>) {
            Raw::new("DEFINE TABLE person SCHEMALESS;")
                .run(db)
                .await
                .expect("table");
        }

        async fn people(db: &Surreal<Any>) -> Vec<String> {
            db.query("SELECT VALUE record::id(id) FROM person ORDER BY id")
                .await
                .expect("query")
                .take::<Vec<String>>(0)
                .expect("ids")
        }

        #[tokio::test]
        async fn success_returns_the_results() {
            let (db, _guard) = db().await;
            let mut results = Raw::new("RETURN 1; RETURN 2;")
                .run(&db)
                .await
                .expect("success");
            assert_eq!(results.take::<Option<i64>>(1).expect("value"), Some(2));
        }

        #[tokio::test]
        async fn a_failing_statement_in_a_chain_is_reported_with_its_index() {
            let (db, _guard) = db().await;
            let query = Raw::new("RETURN 1; THROW 'second'; RETURN 3;");
            let (index, source, _) = statement_error(query.run(&db).await);
            assert_eq!(index, 1);
            assert!(source.to_string().contains("second"));
        }

        #[tokio::test]
        async fn multiple_failures_report_the_first_and_keep_the_others() {
            let (db, _guard) = db().await;
            let query = Raw::new("THROW 'first'; RETURN 1; THROW 'third';");
            let (index, source, related) = statement_error(query.run(&db).await);
            assert_eq!(index, 0);
            assert!(source.to_string().contains("first"));
            assert_eq!(related, 1);
        }

        #[tokio::test]
        async fn a_failed_transaction_reports_its_cause_and_persists_nothing() {
            let (db, _guard) = db().await;
            define_person(&db).await;

            let query = Raw::new(
                "BEGIN TRANSACTION; CREATE person:alice; CREATE person:bob; THROW 'cause'; \
             COMMIT TRANSACTION;",
            );
            let (_, source, _) = statement_error(query.run(&db).await);
            assert!(
                source.to_string().contains("cause"),
                "the cause, not a rollback row: {source}"
            );
            assert_ne!(source.query_details(), Some(&QueryError::NotExecuted));
            assert!(people(&db).await.is_empty());
        }

        #[tokio::test]
        async fn an_explicit_cancel_is_success_and_persists_nothing() {
            let (db, _guard) = db().await;
            define_person(&db).await;

            Raw::new("BEGIN TRANSACTION; CREATE person:alice; CANCEL TRANSACTION;")
                .run(&db)
                .await
                .expect("an intentional cancel succeeds");
            assert!(people(&db).await.is_empty());
        }

        #[tokio::test]
        async fn statements_after_a_cancel_run_and_are_checked() {
            let (db, _guard) = db().await;
            define_person(&db).await;

            let query = Raw::new(
                "BEGIN TRANSACTION; CREATE person:alice; CANCEL TRANSACTION; CREATE person:bob;",
            );
            query.run(&db).await.expect("success");
            assert_eq!(people(&db).await, ["bob"]);

            let query = Raw::new(
                "BEGIN TRANSACTION; CREATE person:carol; CANCEL TRANSACTION; THROW 'after';",
            );
            let (_, source, _) = statement_error(query.run(&db).await);
            assert!(source.to_string().contains("after"));
        }

        #[tokio::test]
        async fn a_statement_timeout_is_a_failure() {
            let (db, _guard) = db().await;
            let query = Raw::new("SELECT * FROM sleep(500ms) TIMEOUT 10ms;");
            let (_, source, _) = statement_error(query.run(&db).await);
            assert!(
                matches!(source.query_details(), Some(QueryError::TimedOut { .. })),
                "expected a timeout, got {source}"
            );

            let query = Raw::new(
                "BEGIN TRANSACTION; SELECT * FROM sleep(500ms) TIMEOUT 10ms; COMMIT TRANSACTION;",
            );
            let (_, source, _) = statement_error(query.run(&db).await);
            assert_ne!(source.query_details(), Some(&QueryError::NotExecuted));
        }
    }

    mod get_data {
        use super::*;

        async fn names(db: &Surreal<Any>, sql: &str) -> Option<Vec<String>> {
            Raw::new(sql).get_data(db).await.expect("decoded")
        }

        #[tokio::test]
        async fn decodes_the_first_result_as_it_is() {
            let (db, _guard) = db().await;
            assert_eq!(names(&db, "RETURN NONE;").await, None);
            assert_eq!(names(&db, "RETURN [];").await, Some(vec![]));
            assert_eq!(
                names(&db, "RETURN ['Alice'];").await,
                Some(vec!["Alice".to_owned()])
            );
            assert_eq!(
                names(&db, "RETURN ['Alice', 'Bob'];").await,
                Some(vec!["Alice".to_owned(), "Bob".to_owned()])
            );
        }

        /// The compatibility break: a statement without `ONLY` returns a list even for one record,
        /// so asking for a single model is a conversion error rather than the list's only element.
        #[tokio::test]
        async fn one_record_from_a_list_must_be_asked_for_as_a_list() {
            let (db, _guard) = db().await;
            SpaceShip {
                id: SpaceShip::id("apollo"),
                name: "Apollo".to_owned(),
                ..Default::default()
            }
            .create(&db)
            .await
            .expect("apollo");
            let query = Raw::new("SELECT * FROM space_ship;");

            let single = query.get_data::<SpaceShip>(&db).await;
            assert!(matches!(single, Err(Error::Conversion(_))), "{single:?}");

            let listed = query
                .get_data::<Vec<SpaceShip>>(&db)
                .await
                .expect("decoded")
                .expect("a result");
            assert_eq!(listed.len(), 1);
        }
    }
}

/// SDK transaction handles (#17): Umbriel statements and model methods run inside
/// `umbriel::Transaction`, an `Executor`, with its commit, cancellation, statement failures and
/// drop.
mod sdk_transactions {

    use surrealdb::{Surreal, engine::any::Any};
    use umbriel::{
        All, Model, Raw, Runnable, SurrealCrud, SurrealCrudNode, SurrealRead,
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
    async fn ships() -> (Surreal<Any>, TestDb) {
        let (db, guard) = db().await;
        Raw::new("DEFINE TABLE space_ship SCHEMALESS;")
            .run(&db)
            .await
            .expect("table defined");
        (db, guard)
    }

    /// The stored ships' names, read on the client rather than through a transaction.
    async fn names(db: &Surreal<Any>) -> Vec<String> {
        read(db, "SELECT VALUE name FROM space_ship ORDER BY name;").await
    }

    /// Statements and model methods write inside the transaction: the client sees nothing until it
    /// commits. `begin`, `commit` and `cancel` errors convert with `?`.
    async fn create_in_a_transaction(db: &Surreal<Any>) -> umbriel::Result<Vec<String>> {
        let tx = db.clone().begin().await?;
        ship("apollo", "Apollo").create(&tx).await?;
        // `created` too: the read below decodes every record as a `SpaceShip`.
        Raw::new(
            "CREATE space_ship:gemini SET name = 'Gemini', created = d'2026-01-01T00:00:00Z';",
        )
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
        let (db, _guard) = ships().await;
        let committed = create_in_a_transaction(&db).await.expect("committed");
        assert_eq!(committed, ["Apollo", "Gemini"]);
    }

    #[tokio::test]
    async fn insert_many_runs_inside_a_transaction() {
        let (db, _guard) = ships().await;
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
        let (db, _guard) = ships().await;
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
        let (db, _guard) = ships().await;
        let tx = db.clone().begin().await.expect("begins");
        ship("apollo", "Apollo").create(&tx).await.expect("created");
        SpaceShip::insert_many(&tx, vec![ship("gemini", "Gemini")])
            .await
            .expect("inserted");
        tx.cancel().await.expect("cancelled");
        assert!(names(&db).await.is_empty());
    }

    /// A failing statement is reported as the query's failure, and leaves the transaction to the
    /// caller, so a commit after it keeps the writes that succeeded.
    #[tokio::test]
    async fn a_failing_statement_is_reported_and_the_transaction_stays_open() {
        let (db, _guard) = ships().await;
        let tx = db.clone().begin().await.expect("begins");
        ship("apollo", "Apollo").create(&tx).await.expect("created");
        let (index, source, _) =
            statement_error(Raw::new("RETURN 1; THROW 'refused';").run(&tx).await);
        assert_eq!(index, 1);
        assert!(source.to_string().contains("refused"), "{source}");
        tx.commit().await.expect("the transaction still commits");
        assert_eq!(names(&db).await, ["Apollo"]);
    }

    /// A handle dropped without `commit` or `cancel` commits nothing, and reports nothing. This
    /// shows only that its writes stay invisible: whether the open transaction is then released
    /// cannot be observed through SDK 3.3.4's public API, so that cleanup is unverified.
    #[tokio::test]
    async fn a_dropped_handle_commits_nothing() {
        let (db, _guard) = ships().await;
        {
            let tx = db.clone().begin().await.expect("begins");
            ship("apollo", "Apollo").create(&tx).await.expect("created");
        }
        assert!(names(&db).await.is_empty());
    }
}

/// Result streams (#20, K10): rows and statement ends, the error contract, and what dropping or
/// cancelling does.
mod streams {
    use futures::StreamExt;
    use umbriel::{Field, Render, RowStream, StreamEvent, Table, statements::select_value};

    use super::*;

    /// Every item of `stream`, bounded by [`WAIT`].
    async fn collect<T>(stream: RowStream<T>) -> Vec<umbriel::Result<StreamEvent<T>>> {
        tokio::time::timeout(WAIT, stream.collect::<Vec<_>>())
            .await
            .expect("the stream ends")
    }

    /// The decoded rows of statement `index`.
    fn rows<T: Clone>(items: &[umbriel::Result<StreamEvent<T>>], index: usize) -> Vec<T> {
        items
            .iter()
            .filter_map(|item| match item {
                Ok(StreamEvent::Row {
                    statement,
                    value: Ok(value),
                }) if *statement == index => Some(value.clone()),
                _ => None,
            })
            .collect()
    }

    /// Whether statement `index` ended, and how.
    fn ended_ok<T>(items: &[umbriel::Result<StreamEvent<T>>], index: usize) -> Option<bool> {
        items.iter().find_map(|item| match item {
            Ok(StreamEvent::End {
                statement, result, ..
            }) if *statement == index => Some(result.is_ok()),
            _ => None,
        })
    }

    async fn count(db: &Surreal<Any>) -> i64 {
        read(db, "RETURN count(SELECT * FROM n);").await
    }

    /// L1: a typed statement's rows, then its end.
    #[tokio::test]
    async fn a_typed_statement_streams_rows_then_ends() {
        let (db, _guard) = db().await;
        assert!(succeeds(&db, "CREATE n:1 SET v = 1; CREATE n:2 SET v = 2;").await);
        let items = collect(
            select_value(Field::new("v"))
                .from(Table::from("n"))
                .decode::<i64>()
                .stream(&db)
                .expect("started"),
        )
        .await;
        let mut values = rows(&items, 0);
        values.sort_unstable();
        assert_eq!(values, [1, 2]);
        assert_eq!(ended_ok(&items, 0), Some(true));
        assert!(
            matches!(
                items.last(),
                Some(Ok(StreamEvent::End { statement: 0, .. }))
            ),
            "the end comes last"
        );
    }

    /// L2: a failed statement ends with its error, and the next statement still streams.
    #[tokio::test]
    async fn a_failed_statement_ends_with_its_error_and_the_next_runs() {
        let (db, _guard) = db().await;
        let items = collect(
            Raw::new("RETURN 1; THROW 'refused'; RETURN 3;")
                .to_query()
                .expect("renders")
                .stream(&db)
                .expect("started"),
        )
        .await;
        assert_eq!(ended_ok(&items, 1), Some(false));
        let refused = items.iter().find_map(|item| match item {
            Ok(StreamEvent::End {
                statement: 1,
                result: Err(error),
                ..
            }) => Some(error.to_string()),
            _ => None,
        });
        assert!(refused.is_some_and(|text| text.contains("refused")));
        assert_eq!(ended_ok(&items, 2), Some(true));
        assert_eq!(
            rows(&items, 2),
            [umbriel::types::SurrealValue::into_value(3i64)]
        );
    }

    /// L3: inside a transaction, the statements after a failure end with errors too, and nothing
    /// commits. The indexes SurrealDB gives `BEGIN` and `COMMIT` aren't assumed.
    #[tokio::test]
    async fn a_failure_in_a_transaction_cancels_the_rest() {
        let (db, _guard) = db().await;
        // Counting a table that doesn't exist is an error, and the rollback removes `n`.
        assert!(succeeds(&db, "DEFINE TABLE n;").await);
        let items = collect(
            Raw::new("BEGIN; CREATE n:1; THROW 'refused'; CREATE n:2; COMMIT;")
                .to_query()
                .expect("renders")
                .stream(&db)
                .expect("started"),
        )
        .await;
        let ends = items
            .iter()
            .filter_map(|item| match item {
                Ok(StreamEvent::End { result, .. }) => Some(result.is_ok()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(ends.iter().any(|ok| !ok), "the THROW fails: {ends:?}");
        assert_eq!(
            ends.last(),
            Some(&false),
            "the last statement doesn't succeed"
        );
        assert_eq!(count(&db).await, 0);
    }

    /// L4: a row that doesn't decode is an error of that row; the stream goes on.
    #[tokio::test]
    async fn an_undecodable_row_is_an_error_of_that_row() {
        let (db, _guard) = db().await;
        assert!(succeeds(&db, "CREATE n:1 SET v = 1; CREATE n:2 SET v = 'two';").await);
        let items = collect(
            select_value(Field::new("v"))
                .from(Table::from("n"))
                .decode::<i64>()
                .stream(&db)
                .expect("started"),
        )
        .await;
        assert!(items.iter().any(|item| matches!(
            item,
            Ok(StreamEvent::Row {
                value: Err(Error::Conversion(_)),
                ..
            })
        )));
        assert_eq!(rows(&items, 0), [1]);
        assert_eq!(ended_ok(&items, 0), Some(true));
    }

    /// L5: a query that can't start fails before the stream exists, classified.
    #[test]
    fn a_stream_on_an_unconnected_client_fails_to_start() {
        let client = Surreal::<Any>::init();
        let started = Raw::new("RETURN 1;")
            .to_query()
            .expect("renders")
            .stream(&client);
        assert!(
            matches!(started, Err(Error::Connection { .. })),
            "{:?}",
            started.err()
        );
    }

    /// L6a: undoing work is explicit: `CANCEL` in the script, or a transaction handle's
    /// `cancel`.
    #[tokio::test]
    async fn cancelling_commits_nothing() {
        let (db, _guard) = db().await;
        // Counting a table that doesn't exist is an error, and the rollback removes `n`.
        assert!(succeeds(&db, "DEFINE TABLE n;").await);
        let items = collect(
            Raw::new("BEGIN; CREATE n:1; SELECT * FROM n; CANCEL;")
                .to_query()
                .expect("renders")
                .stream(&db)
                .expect("started"),
        )
        .await;
        assert!(
            items.iter().all(Result::is_ok),
            "every item belongs to a statement"
        );
        assert_eq!(count(&db).await, 0);

        let tx = db.clone().begin().await.expect("begins");
        Raw::new("CREATE n:2;").run(&tx).await.expect("created");
        let items = collect(
            select_value(Field::new("id"))
                .from(Table::from("n"))
                .stream(&tx)
                .expect("started"),
        )
        .await;
        assert_eq!(
            rows(&items, 0).len(),
            1,
            "the transaction sees its own write"
        );
        tx.cancel().await.expect("cancelled");
        assert_eq!(count(&db).await, 0);
    }

    /// L6b: over WebSocket the result arrives once the query has finished, so dropping an
    /// autocommitted stream after its first row cancels nothing: the writes are committed, and
    /// the session runs the next query. This pins replay; it is not a rollback test.
    #[tokio::test]
    async fn dropping_a_replayed_stream_keeps_its_writes() {
        let (db, _guard) = db().await;
        let mut stream = Raw::new("CREATE n:1; CREATE n:2; SELECT * FROM n;")
            .to_query()
            .expect("renders")
            .stream(&db)
            .expect("started");
        let first = tokio::time::timeout(WAIT, stream.next())
            .await
            .expect("an item");
        assert!(first.is_some_and(|item| item.is_ok()));
        drop(stream);
        assert_eq!(count(&db).await, 2);
        assert!(
            succeeds(&db, "RETURN 1;").await,
            "the session runs the next query"
        );
    }
}
