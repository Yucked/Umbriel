//! Errors (`orm/src/errors.rs`, classified in `orm/src/execution.rs`): a duplicate record and an
//! undefined table are distinct errors on both execution paths, statements and SDK methods, and
//! each keeps the database error as its source.

use std::error::Error as _;

use surrealdb::{
    Surreal,
    engine::local::{Db, Mem},
    types::{AlreadyExistsError, NotFoundError},
};
use umbriel::{
    All, Error, Model, Raw, Runnable, SchemaGetter, SetterAssignable, StatementContext,
    SurrealCrud, SurrealCrudNode,
    statements::{CreateTarget, Returning, create, create_only, delete, select, update},
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
                .run(db.clone())
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
            .run(db.clone())
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
