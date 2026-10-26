//! Statement-result checking: `select_failure` on constructed result sequences, and `execute`
//! (through `Runnable::run`) against the in-memory engine, and how `Runnable::get_data` decodes a
//! first result.

use std::time::Duration;

use surrealdb::{Error as SdkError, Surreal, engine::local::Db, types::QueryError};
use umbriel::{Error, Raw, Runnable, test_support::select_failure};
use umbriel_tests::{db, statement_error};

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
        let conflict = SdkError::query("conflict".to_owned(), QueryError::TransactionConflict);
        for error in [timed_out, conflict, unknown_outcome("unknown")] {
            assert_eq!(
                selected(1, vec![(0, error)]).map(|(index, ..)| index),
                Some(0)
            );
        }
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

    async fn define_person(db: &Surreal<Db>) {
        Raw::new("DEFINE TABLE person SCHEMALESS;")
            .run(db.clone())
            .await
            .expect("table");
    }

    async fn people(db: &Surreal<Db>) -> Vec<String> {
        db.query("SELECT VALUE record::id(id) FROM person ORDER BY id")
            .await
            .expect("query")
            .take::<Vec<String>>(0)
            .expect("ids")
    }

    #[tokio::test]
    async fn success_returns_the_results() {
        let db = db().await;
        let mut results = Raw::new("RETURN 1; RETURN 2;")
            .run(db.clone())
            .await
            .expect("success");
        assert_eq!(results.take::<Option<i64>>(1).expect("value"), Some(2));
    }

    #[tokio::test]
    async fn a_failing_statement_in_a_chain_is_reported_with_its_index() {
        let db = db().await;
        let query = Raw::new("RETURN 1; THROW 'second'; RETURN 3;");
        let (index, source, _) = statement_error(query.run(db).await);
        assert_eq!(index, 1);
        assert!(source.to_string().contains("second"));
    }

    #[tokio::test]
    async fn multiple_failures_report_the_first_and_keep_the_others() {
        let db = db().await;
        let query = Raw::new("THROW 'first'; RETURN 1; THROW 'third';");
        let (index, source, related) = statement_error(query.run(db).await);
        assert_eq!(index, 0);
        assert!(source.to_string().contains("first"));
        assert_eq!(related, 1);
    }

    #[tokio::test]
    async fn a_failed_transaction_reports_its_cause_and_persists_nothing() {
        let db = db().await;
        define_person(&db).await;

        let query = Raw::new(
            "BEGIN TRANSACTION; CREATE person:alice; CREATE person:bob; THROW 'cause'; \
             COMMIT TRANSACTION;",
        );
        let (_, source, _) = statement_error(query.run(db.clone()).await);
        assert!(
            source.to_string().contains("cause"),
            "the cause, not a rollback row: {source}"
        );
        assert_ne!(source.query_details(), Some(&QueryError::NotExecuted));
        assert!(people(&db).await.is_empty());
    }

    #[tokio::test]
    async fn an_explicit_cancel_is_success_and_persists_nothing() {
        let db = db().await;
        define_person(&db).await;

        Raw::new("BEGIN TRANSACTION; CREATE person:alice; CANCEL TRANSACTION;")
            .run(db.clone())
            .await
            .expect("an intentional cancel succeeds");
        assert!(people(&db).await.is_empty());
    }

    #[tokio::test]
    async fn statements_after_a_cancel_run_and_are_checked() {
        let db = db().await;
        define_person(&db).await;

        let query = Raw::new(
            "BEGIN TRANSACTION; CREATE person:alice; CANCEL TRANSACTION; CREATE person:bob;",
        );
        query.run(db.clone()).await.expect("success");
        assert_eq!(people(&db).await, ["bob"]);

        let query =
            Raw::new("BEGIN TRANSACTION; CREATE person:carol; CANCEL TRANSACTION; THROW 'after';");
        let (_, source, _) = statement_error(query.run(db.clone()).await);
        assert!(source.to_string().contains("after"));
    }

    #[tokio::test]
    async fn a_statement_timeout_is_a_failure() {
        let db = db().await;
        let query = Raw::new("SELECT * FROM sleep(500ms) TIMEOUT 10ms;");
        let (_, source, _) = statement_error(query.run(db.clone()).await);
        assert!(
            matches!(source.query_details(), Some(QueryError::TimedOut { .. })),
            "expected a timeout, got {source}"
        );

        let query = Raw::new(
            "BEGIN TRANSACTION; SELECT * FROM sleep(500ms) TIMEOUT 10ms; COMMIT TRANSACTION;",
        );
        let (_, source, _) = statement_error(query.run(db).await);
        assert_ne!(source.query_details(), Some(&QueryError::NotExecuted));
    }
}

mod get_data {
    use super::*;

    async fn names(db: &Surreal<Db>, sql: &str) -> Option<Vec<String>> {
        Raw::new(sql).get_data(db.clone()).await.expect("decoded")
    }

    #[tokio::test]
    async fn decodes_the_first_result_as_it_is() {
        let db = db().await;
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
}
