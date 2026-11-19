//! The migration history (`orm/src/migrations/database/migration_model.rs`): reading it from a
//! database.

use umbriel::{Raw, Runnable, migrations::*};
use umbriel_tests::db;

    use surrealdb::types::{ConnectionError, NotAllowedError, NotFoundError};
    use umbriel::test_support::history_or_empty;

    #[tokio::test]
    async fn a_fresh_database_has_no_history() {
        let (db, _guard) = db().await;
        assert!(
            Migration::get_latest(&db)
                .await
                .expect("readable")
                .is_none()
        );
        assert!(
            Migration::get_all_desc(&db)
                .await
                .expect("readable")
                .is_empty()
        );
        let filename = MigrationFilename::try_from("20240101000000000_init.up.surql".to_owned())
            .expect("a migration file name");
        assert!(
            Migration::get_by_filename(&db, &filename)
                .await
                .expect("readable")
                .is_none()
        );
    }

    #[tokio::test]
    async fn an_undecodable_history_fails() {
        let (db, _guard) = db().await;
        Raw::new("DEFINE TABLE migration SCHEMALESS; CREATE migration:broken SET name = 1;")
            .run(&db)
            .await
            .expect("created");
        assert!(Migration::get_all_desc(&db).await.is_err());
    }

    /// A failed statement, classified as execution classifies it.
    fn statement_error(source: surrealdb::Error) -> umbriel::Result<Vec<Migration>> {
        Err(umbriel::test_support::classify(
            source,
            Some(umbriel::StatementContext {
                index: 0,
                related: vec![],
            }),
        ))
    }

    #[test]
    fn only_a_missing_history_table_reads_as_empty_history() {
        let missing = |details: NotFoundError| {
            statement_error(surrealdb::Error::not_found("missing".to_owned(), details))
        };

        let history = history_or_empty(missing(NotFoundError::Table {
            name: "migration".to_owned(),
        }))
        .expect("no history");
        assert!(history.is_empty());

        for error in [
            missing(NotFoundError::Table {
                name: "other".to_owned(),
            }),
            missing(NotFoundError::Database {
                name: "test".to_owned(),
            }),
            statement_error(surrealdb::Error::not_allowed(
                "permission denied".to_owned(),
                None::<NotAllowedError>,
            )),
            Err(umbriel::Error::Query(surrealdb::Error::connection(
                "connection lost".to_owned(),
                None::<ConnectionError>,
            ))),
            Err(umbriel::Error::Conversion(surrealdb::Error::internal(
                "undecodable".to_owned(),
            ))),
        ] {
            assert!(history_or_empty(error).is_err());
        }
    }
