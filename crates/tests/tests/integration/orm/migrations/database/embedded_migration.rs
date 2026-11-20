//! Embedded migrations (`orm/src/migrations/database/embedded_migration.rs`): the files
//! `embed_migrations!` compiles in, and applying them.

use umbriel::migrations::*;
use umbriel_tests::db;

use pretty_assertions::assert_eq;
use umbriel::migrations::{self, EmbeddedMigrationsTwoWay, FileMetadataStatic, embed_migrations};

const MIGRATIONS_ONE_WAY: migrations::EmbeddedMigrationsOneWay =
    embed_migrations!("tests/integration/fixtures/migrations_one_way");

const MIGRATIONS_TWO_WAY: migrations::EmbeddedMigrationsTwoWay =
    embed_migrations!("tests/integration/fixtures/migrations_two_way");

const MIGRATOR_ONE_WAY: migrations::EmbeddedMigrationsOneWay =
    embed_migrations!("tests/integration/fixtures/runner_one_way");

#[test]
fn test_embedded() {
    assert_eq!(MIGRATIONS_ONE_WAY.files().len(), 2);
    // One up/down pair.
    assert_eq!(MIGRATIONS_TWO_WAY.files().len(), 2);
    assert_eq!(MIGRATOR_ONE_WAY.files().len(), 2);

    let migs = MIGRATIONS_ONE_WAY.to_migrations_one_way().unwrap();
    assert_eq!(migs.len(), 2);
    // check the meta data
    assert_eq!(
        migs[0].name().to_string(),
        "20231029202315_create_new_stuff.surql"
    );

    assert_eq!(
        migs[0].name().basename(),
        "create_new_stuff".to_string().into()
    );
    assert_eq!(
        migs[0].content().to_string(),
        include_str!("../../../fixtures/migrations_one_way/20231029202315_create_new_stuff.surql")
    );
    assert_eq!(
        migs[1].name().to_string(),
        "20231029224601_create_another.surql"
    );
    assert_eq!(migs[1].name().basename(), "create_another".into());
    assert_eq!(migs.len(), 2);
    assert_eq!(
        migs[1].content().to_string(),
        include_str!("../../../fixtures/migrations_one_way/20231029224601_create_another.surql")
    );

    let migs = MIGRATIONS_TWO_WAY.to_migrations_two_way().unwrap();
    assert_eq!(migs.len(), 1);

    // check the meta data
    assert_eq!(
        migs[0].up.name.to_string(),
        "20231030025711_migration_name_example.up.surql"
    );
    assert_eq!(migs[0].up.name.basename(), "migration_name_example".into());
    assert_eq!(
        migs[0].up.content.to_string(),
        include_str!(
            "../../../fixtures/migrations_two_way/20231030025711_migration_name_example.up.surql"
        )
    );
    assert_eq!(
        migs[0].down.content.to_string(),
        include_str!(
            "../../../fixtures/migrations_two_way/20231030025711_migration_name_example.down.surql"
        )
    );
}

/// The recorded migrations, newest first: their names, and whether each has a down
/// checksum.
async fn history(db: &surrealdb::Surreal<surrealdb::engine::any::Any>) -> Vec<(String, bool)> {
    Migration::get_all_desc(db)
        .await
        .expect("the history is read")
        .into_iter()
        .map(|migration| (migration.name, migration.checksum_down.is_some()))
        .collect()
}

#[tokio::test]
async fn test_embedded_run_one_way() {
    let (db, _guard) = db().await;

    MIGRATIONS_ONE_WAY
        .run(&db, UpdateStrategy::Latest, Mode::Strict)
        .await
        .unwrap();

    assert_eq!(
        history(&db).await,
        [
            ("20231029224601_create_another.surql".to_owned(), false),
            ("20231029202315_create_new_stuff.surql".to_owned(), false),
        ]
    );
}

#[tokio::test]
async fn test_embedded_run_two_way() {
    let (db, _guard) = db().await;

    MIGRATIONS_TWO_WAY
        .run(&db, UpdateStrategy::Latest, Mode::Strict)
        .await
        .unwrap();

    assert_eq!(
        history(&db).await,
        [(
            "20231030025711_migration_name_example.up.surql".to_owned(),
            true
        )]
    );
}

const PAIR_UP: FileMetadataStatic = FileMetadataStatic {
    name: "20231030025711_create_user.up.surql",
    content: "DEFINE TABLE user;",
};
const PAIR_DOWN: FileMetadataStatic = FileMetadataStatic {
    name: "20231030025711_create_user.down.surql",
    content: "REMOVE TABLE user;",
};
const ORPHAN_DOWN: FileMetadataStatic = FileMetadataStatic {
    name: "20231031000000_create_company.down.surql",
    content: "REMOVE TABLE company;",
};

fn assert_missing_up(migrations: EmbeddedMigrationsTwoWay) {
    match migrations.to_migrations_two_way() {
        Err(MigrationError::MigrationFileDoesNotExist(name)) => {
            assert_eq!(name, "20231031000000_create_company.up.surql");
        }
        other => panic!("expected MigrationFileDoesNotExist, got {other:?}"),
    }
}

#[test]
fn test_two_way_orphan_down_beside_pair() {
    assert_missing_up(EmbeddedMigrationsTwoWay::new(&[
        PAIR_DOWN,
        PAIR_UP,
        ORPHAN_DOWN,
    ]));
}

#[test]
fn test_two_way_only_orphan_down() {
    assert_missing_up(EmbeddedMigrationsTwoWay::new(&[ORPHAN_DOWN]));
}
