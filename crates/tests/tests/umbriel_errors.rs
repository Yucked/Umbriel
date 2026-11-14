//! Errors (`orm/src/errors.rs`, classified in `orm/src/execution.rs`): a duplicate record, an
//! undefined table, a transaction conflict, a refused capability and an unusable connection are
//! distinct errors on both execution paths, statements and SDK methods, each keeping the database
//! error as its source; and `Error::retry` gives each its advice (#29).

use std::error::Error as _;

use surrealdb::{
    Error as SdkError, Surreal,
    engine::{
        any::Any,
        local::{Db, Mem},
    },
    types::{AlreadyExistsError, ConnectionError, NotFoundError, QueryError},
};
use umbriel::{
    All, Error, Model, Raw, Retry, Runnable, SchemaGetter, SetterAssignable, StatementContext,
    SurrealCrudNode, SurrealRead,
    statements::{CreateTarget, Returning, create, create_only, delete, select, update, use_},
};
use umbriel_tests::*;

fn apollo() -> SpaceShip {
    SpaceShip {
        id: SpaceShip::id("apollo"),
        name: "Apollo".to_owned(),
        ..Default::default()
    }
}

/// An in-memory database defined `STRICT`, where tables must be defined before use.
async fn strict_db() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.expect("in-memory engine");
    let mut defined = db
        .query("DEFINE NAMESPACE test; USE NS test; DEFINE DATABASE test STRICT;")
        .await
        .expect("defined");
    assert!(defined.take_errors().is_empty(), "a strict database");
    db.use_ns("test").use_db("test").await.expect("selected");
    db
}

mod record_exists {
    use super::*;

    /// The feasibility check for the SDK path: the engine reports the duplicate with structured
    /// details, so it is classified without reading the message.
    #[tokio::test]
    async fn a_second_sdk_create_is_record_exists_with_its_cause() {
        let db = db().await;
        apollo().create(&db).await.expect("created");
        let error = apollo().create(&db).await.expect_err("a duplicate");
        let Error::RecordExists {
            id,
            source,
            statement,
        } = &error
        else {
            panic!("expected RecordExists, got {error:?}");
        };
        assert_eq!(id, "space_ship:apollo");
        assert!(statement.is_none(), "an SDK call has no statement context");
        assert!(matches!(
            source.already_exists_details(),
            Some(AlreadyExistsError::Record { .. })
        ));
        assert!(error.source().is_some());
    }

    /// The feasibility check for the statement path.
    #[tokio::test]
    async fn a_second_create_statement_is_record_exists_with_its_context() {
        let db = db().await;
        let statement = || create_only::<SpaceShip>(CreateTarget::table()).content(apollo());
        statement().execute(&db).await.expect("created");

        let error = statement().execute(&db).await.expect_err("a duplicate");
        assert!(
            matches!(
                &error,
                Error::RecordExists {
                    statement: Some(StatementContext { index: 0, related }),
                    ..
                } if related.is_empty()
            ),
            "{error:?}"
        );
        assert!(error.to_string().contains("space_ship:apollo"), "{error}");
    }

    #[tokio::test]
    async fn the_context_keeps_the_index_and_related_errors() {
        let db = db().await;
        apollo().create(&db).await.expect("created");
        let error =
            Raw::new("RETURN 1; CREATE space_ship:apollo SET name = 'Apollo'; THROW 'later';")
                .run(&db)
                .await
                .expect_err("a duplicate");
        assert!(
            matches!(
                &error,
                Error::RecordExists {
                    statement: Some(StatementContext { index: 1, related }),
                    ..
                } if related.len() == 1
            ),
            "{error:?}"
        );
    }

    /// `IndexExists` has no structured details in `surrealdb-core`, so a unique index conflict is
    /// not classified and stays a statement error.
    #[tokio::test]
    async fn a_unique_index_conflict_stays_a_statement_error() {
        let db = db().await;
        Raw::new("DEFINE INDEX ship_name ON space_ship FIELDS name UNIQUE;")
            .run(&db)
            .await
            .expect("defined");
        apollo().create(&db).await.expect("created");
        let clash = create::<SpaceShip>(SpaceShip::id("gemini"))
            .set(SpaceShip::schema().name.equal_to("Apollo"))
            .execute(&db)
            .await;
        assert!(matches!(clash, Err(Error::Statement { .. })), "{clash:?}");
    }
}

mod table_not_found {
    use super::*;

    fn is_space_ship_missing(error: &Error, from_statement: bool) -> bool {
        matches!(
            error,
            Error::TableNotFound { table, source, statement }
                if table == "space_ship"
                    && statement.is_some() == from_statement
                    && matches!(source.not_found_details(), Some(NotFoundError::Table { .. }))
        )
    }

    #[tokio::test]
    async fn reading_updating_and_deleting_an_undefined_table_fail_distinctly() {
        let db = strict_db().await;
        let read = select(All)
            .from(SpaceShip::table())
            .decode::<SpaceShip>()
            .execute(&db)
            .await
            .expect_err("undefined");
        assert!(is_space_ship_missing(&read, true), "{read:?}");

        let updated = update::<SpaceShip>(SpaceShip::table())
            .set(SpaceShip::schema().name.equal_to("Apollo"))
            .execute(&db)
            .await
            .expect_err("undefined");
        assert!(is_space_ship_missing(&updated, true), "{updated:?}");

        let deleted = delete::<SpaceShip>(SpaceShip::table())
            .execute(&db)
            .await
            .expect_err("undefined");
        assert!(is_space_ship_missing(&deleted, true), "{deleted:?}");
    }

    #[tokio::test]
    async fn an_sdk_read_of_an_undefined_table_is_classified_too() {
        let db = strict_db().await;
        let error = SpaceShip::get(&db, &SpaceShip::id("apollo"))
            .await
            .expect_err("undefined");
        assert!(is_space_ship_missing(&error, false), "{error:?}");
    }
}

/// Write conflicts. The ordering of the two committers is fixed by the test, never by timing: a
/// query transaction commits inside one request, so it cannot be made to lose to another
/// committer on the embedded engine without racing it. The query path is therefore checked with
/// the responses SurrealDB gives (its `COMMIT` result keeps the structured `TransactionConflict`
/// kind, `surrealdb-core` `dbs/executor.rs:2027`), and the end-to-end conflict with SDK handles,
/// ordered by their commits: here on the embedded engine, and over WebSocket in
/// `tests/server.rs` (`sdk_transactions`), where the kind survives.
mod conflict {
    use super::*;
    use umbriel::test_support::select_failure;

    /// A query transaction whose `COMMIT` conflicted, as SurrealDB 3.3.4 reports it: `BEGIN`
    /// succeeds, the statements inside are not executed, and the `COMMIT` carries the conflict.
    #[test]
    fn a_conflicting_commit_of_a_query_transaction_is_conflict() {
        let not_executed =
            |message: &str| SdkError::query(message.to_owned(), QueryError::NotExecuted);
        let error = select_failure(
            4,
            [
                (1, not_executed("the update was not executed")),
                (2, not_executed("the select was not executed")),
                (
                    3,
                    SdkError::query(
                        "Cannot COMMIT: transaction conflict".to_owned(),
                        QueryError::TransactionConflict,
                    ),
                ),
            ],
        )
        .expect("a failure");
        assert!(
            matches!(
                &error,
                Error::Conflict {
                    source,
                    statement: Some(StatementContext { index: 3, related }),
                } if source.query_details() == Some(&QueryError::TransactionConflict)
                    && related.len() == 2
            ),
            "{error:?}"
        );
        assert_eq!(error.retry(), Retry::Safe);
    }

    /// The conversion classifies as statement execution does.
    #[test]
    fn an_sdk_conflict_converts_to_conflict() {
        let error = Error::from(SdkError::query(
            "conflict".to_owned(),
            QueryError::TransactionConflict,
        ));
        assert!(
            matches!(
                &error,
                Error::Conflict {
                    statement: None,
                    ..
                }
            ),
            "{error:?}"
        );
        assert_eq!(error.retry(), Retry::Safe);
    }

    /// Advice covers the failed transaction only: a write the same batch committed before it
    /// stays applied, so replaying the whole batch would write it twice.
    #[tokio::test]
    async fn an_earlier_write_of_the_batch_stays_applied() {
        let db = db().await;
        apollo().create(&db).await.expect("created");
        let error = Raw::new(
            "CREATE space_ship:gemini SET name = 'Gemini'; \
             BEGIN TRANSACTION; UPDATE space_ship:apollo SET name = 'Vostok'; THROW 'refused'; \
             COMMIT TRANSACTION;",
        )
        .run(&db)
        .await
        .expect_err("the transaction fails");
        assert!(
            matches!(&error, Error::Statement { source, .. } if source.is_thrown()),
            "{error:?}"
        );
        let apollo: String = read(&db, "RETURN space_ship:apollo.name;").await;
        assert_eq!(apollo, "Apollo", "the failed transaction committed nothing");
        let earlier: Option<String> = read(&db, "RETURN space_ship:gemini.name;").await;
        assert_eq!(
            earlier.as_deref(),
            Some("Gemini"),
            "the earlier write stays"
        );
    }

    /// Q13b on the embedded engine, ordered by the handles' commits: `FOR UPDATE` takes no lock,
    /// so a write to the selected record committed first fails the reader's commit. The SDK
    /// handle's `commit` error converts through `From<surrealdb::Error>`, but on the embedded
    /// engine it carries no structured kind: `surrealdb-engine-local` maps the commit failure
    /// with `Error::internal(text)` (`lib.rs:113`), so it is not classified as a conflict and its
    /// advice is `Uncertain`. Over WebSocket it is a `Conflict` (`tests/server.rs`).
    #[tokio::test]
    async fn an_embedded_handle_commit_conflict_converts_without_a_kind() {
        let db = db().await;
        apollo().create(&db).await.expect("created");
        let reader = db.clone().begin().await.expect("reader begins");
        select(All)
            .from(SpaceShip::id("apollo"))
            .for_update()
            .run(&reader)
            .await
            .expect("selected for update");
        let writer = db.clone().begin().await.expect("writer begins");
        update::<SpaceShip>(SpaceShip::id("apollo"))
            .set(SpaceShip::schema().name.equal_to("Gemini"))
            .run(&writer)
            .await
            .expect("updated");
        writer.commit().await.expect("the writer commits first");

        let error = Error::from(
            reader
                .commit()
                .await
                .expect_err("the reader's commit fails"),
        );
        assert!(
            matches!(&error, Error::Query(source) if source.is_internal()),
            "{error:?}"
        );
        assert_eq!(error.retry(), Retry::Uncertain);
    }
}

mod not_allowed {
    use super::*;

    /// Scripting is a capability the default embedded engine does not grant.
    #[tokio::test]
    async fn a_refused_capability_is_not_allowed() {
        let db = db().await;
        let error = Raw::new("RETURN 1; RETURN function() { return 1; };")
            .run(&db)
            .await
            .expect_err("scripting is refused");
        assert!(
            matches!(
                &error,
                Error::NotAllowed {
                    source,
                    statement: Some(StatementContext { index: 1, .. }),
                } if source.is_not_allowed()
            ),
            "{error:?}"
        );
        assert_eq!(error.retry(), Retry::Never);
    }
}

mod connection {
    use super::*;

    /// A pre-dispatch failure: a client that was never connected fails before anything is
    /// sent, with `ConnectionError::Uninitialised` (`surrealdb` `lib.rs:387`).
    #[tokio::test]
    async fn an_uninitialised_client_fails_before_dispatch() {
        let client = Surreal::<Any>::init();
        let error = Raw::new("CREATE space_ship:apollo;")
            .run(&client)
            .await
            .expect_err("not connected");
        assert!(
            matches!(
                &error,
                Error::Connection { source, statement: None }
                    if source.connection_details() == Some(&ConnectionError::Uninitialised)
            ),
            "{error:?}"
        );
        assert_eq!(error.retry(), Retry::Never);
    }

    #[test]
    fn retry_advice_follows_the_connection_error_kind() {
        let advice = |details: Option<ConnectionError>| {
            Error::from(SdkError::connection("connection".to_owned(), details)).retry()
        };
        assert_eq!(advice(Some(ConnectionError::Uninitialised)), Retry::Never);
        assert_eq!(
            advice(Some(ConnectionError::AlreadyConnected)),
            Retry::Never
        );
        assert_eq!(
            advice(Some(ConnectionError::ConnectionFailed)),
            Retry::Uncertain
        );
        assert_eq!(advice(None), Retry::Uncertain);
    }
}

mod retry {
    use super::*;
    use umbriel::migrations::MigrationError;

    /// SurrealDB 3.3.4 reports a commit whose outcome is unknown as a query error without a kind
    /// (`dbs/executor.rs:2010`); the write may have applied.
    #[test]
    fn an_unknown_commit_outcome_is_uncertain() {
        let error = umbriel::test_support::select_failure(
            3,
            [(
                2,
                SdkError::query(
                    "The transaction's commit outcome is unknown".to_owned(),
                    None,
                ),
            )],
        )
        .expect("a failure");
        assert!(
            matches!(&error, Error::Statement { index: 2, .. }),
            "{error:?}"
        );
        assert_eq!(error.retry(), Retry::Uncertain);
    }

    #[test]
    fn an_error_without_a_kind_is_uncertain() {
        let internal = Error::from(SdkError::internal("rolled back".to_owned()));
        assert_eq!(internal.retry(), Retry::Uncertain);
        let cancelled = Error::from(SdkError::query(
            "cancelled".to_owned(),
            QueryError::Cancelled,
        ));
        assert_eq!(cancelled.retry(), Retry::Uncertain);
    }

    #[test]
    fn refusals_and_unexecuted_statements_are_never() {
        let advice = |source: SdkError| Error::from(source).retry();
        assert_eq!(advice(SdkError::thrown("refused".to_owned())), Retry::Never);
        assert_eq!(
            advice(SdkError::query(
                "not executed".to_owned(),
                QueryError::NotExecuted
            )),
            Retry::Never
        );
        assert_eq!(
            advice(SdkError::not_found(
                "missing".to_owned(),
                NotFoundError::Database {
                    name: "test".to_owned()
                }
            )),
            Retry::Never
        );
        assert_eq!(
            advice(SdkError::already_exists(
                "exists".to_owned(),
                AlreadyExistsError::Table {
                    name: "space_ship".to_owned()
                }
            )),
            Retry::Never
        );
    }

    /// A migration failure takes the advice of the database error it wraps.
    #[test]
    fn a_migration_failure_takes_its_database_errors_advice() {
        let wrapped = |error: MigrationError| Error::from(error).retry();
        let conflict = Error::from(SdkError::query(
            "conflict".to_owned(),
            QueryError::TransactionConflict,
        ));
        assert_eq!(
            wrapped(MigrationError::ProblemWithQuery(conflict)),
            Retry::Safe
        );
        assert_eq!(
            wrapped(MigrationError::DbError(SdkError::internal(
                "commit failed".to_owned()
            ))),
            Retry::Uncertain
        );
        assert_eq!(
            wrapped(MigrationError::NoMigrationsRegisteredYetInDb),
            Retry::Never
        );
    }

    /// A server-reported timeout is `Uncertain`. Here the timed-out transaction committed
    /// nothing, which this case shows but which is not claimed for every timeout path.
    #[tokio::test]
    async fn a_timed_out_write_transaction_is_uncertain() {
        let db = db().await;
        let error = Raw::new(
            "BEGIN TRANSACTION; CREATE space_ship:apollo SET name = 'Apollo'; \
             SELECT * FROM sleep(500ms) TIMEOUT 10ms; COMMIT TRANSACTION;",
        )
        .run(&db)
        .await
        .expect_err("timed out");
        assert!(
            matches!(
                &error,
                Error::Statement { source, .. }
                    if matches!(source.query_details(), Some(QueryError::TimedOut { .. }))
            ),
            "{error:?}"
        );
        assert_eq!(error.retry(), Retry::Uncertain);
        let stored: Option<String> = read(&db, "RETURN space_ship:apollo.name;").await;
        assert_eq!(stored, None);
    }

    #[tokio::test]
    async fn a_query_that_does_not_build_is_never_retried() {
        let db = db().await;
        let error = use_().run(&db).await.expect_err("USE needs a target");
        assert!(matches!(&error, Error::QueryBuilder(_)), "{error:?}");
        assert_eq!(error.retry(), Retry::Never);
    }

    #[tokio::test]
    async fn a_duplicate_record_is_never_retried() {
        let db = db().await;
        apollo().create(&db).await.expect("created");
        let error = apollo().create(&db).await.expect_err("a duplicate");
        assert_eq!(error.retry(), Retry::Never);
    }
}
