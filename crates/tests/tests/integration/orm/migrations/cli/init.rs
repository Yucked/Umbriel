//! `init` (`orm/src/migrations/cli/init.rs`): the first migration, one-way or two-way, written and
//! applied when asked, and a second `init` refused.

use pretty_assertions::assert_eq;
use umbriel::{DbResources, Raw, migrations::*};
use umbriel_tests::{
    migrations::{helpers::*, *},
    succeeds,
};

fn init(run: bool, reversible: bool) -> Init {
    Init::builder()
        .name("migration init".into())
        .reversible(reversible)
        .run(run)
        .build()
}

/// What the first migration of `Resources` defines, as the server renders it.
const UP: [&str; 37] = [
    "DELETE migration;",
    "DEFINE TABLE OVERWRITE animal TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
    "DEFINE FIELD OVERWRITE attributes ON animal TYPE array<string> PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE createdAt ON animal TYPE datetime PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE species ON animal TYPE string PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE updatedAt ON animal TYPE datetime PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE velocity ON animal TYPE int PERMISSIONS FULL;",
    "DEFINE TABLE OVERWRITE migration TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
    "DEFINE FIELD OVERWRITE checksum_up ON migration TYPE string PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE id ON migration TYPE string PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE name ON migration TYPE string PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE timestamp ON migration TYPE int PERMISSIONS FULL;",
    "DEFINE TABLE OVERWRITE planet TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
    "DEFINE FIELD OVERWRITE createdAt ON planet TYPE datetime PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE firstName ON planet TYPE string PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE labels ON planet TYPE array<string> PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE population ON planet TYPE int PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE updatedAt ON planet TYPE datetime PERMISSIONS FULL;",
    "DEFINE INDEX OVERWRITE species_speed_idx ON animal FIELDS species, velocity UNIQUE;",
    "DEFINE EVENT OVERWRITE event1 ON animal WHEN ",
    "DEFINE EVENT OVERWRITE event2 ON animal WHEN ",
    "DEFINE TABLE OVERWRITE crop TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
    "DEFINE FIELD OVERWRITE color ON crop TYPE string PERMISSIONS FULL;",
    "DEFINE TABLE OVERWRITE eats TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
    "DEFINE FIELD OVERWRITE createdAt ON eats TYPE datetime PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE in ON eats TYPE record",
    "DEFINE FIELD OVERWRITE out ON eats TYPE record",
    "DEFINE FIELD OVERWRITE place ON eats TYPE string PERMISSIONS FULL;",
    "DEFINE TABLE OVERWRITE student TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
    "DEFINE FIELD OVERWRITE age ON student TYPE int PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE createdAt ON student TYPE datetime PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE university ON student TYPE string PERMISSIONS FULL;",
    "DEFINE FIELD OVERWRITE updatedAt ON student TYPE datetime PERMISSIONS FULL;",
    "DEFINE FUNCTION OVERWRITE fn::get_animal_by_id(",
    "DEFINE ANALYZER OVERWRITE ascii ",
    "DEFINE PARAM OVERWRITE $__some_test_param1 ",
    "DEFINE ACCESS OVERWRITE partner_api ON DATABASE TYPE JWT",
];

const CHECKSUM_DOWN: &str =
    "DEFINE FIELD OVERWRITE checksum_down ON migration TYPE string PERMISSIONS FULL;";

/// The text of the project's only migration: its up file, and its down file when two-way.
fn first_migration(project: &Project) -> (String, Option<String>) {
    if project.reversible() {
        let (up, down) = newest_migration(&project.dir);
        return (up, Some(down));
    }
    let [file] = migration_files(&project.dir)
        .try_into()
        .unwrap_or_else(|files| panic!("one migration file: {files:?}"));
    let up = std::fs::read_to_string(project.dir.join(file)).expect("readable");
    (up, None)
}

/// `init` writes the codebase's whole schema, with the history table, and applies nothing until
/// `up`. A two-way first migration also records down checksums and removes every table on the
/// way down.
#[tokio::test]
async fn init_writes_the_first_migration_and_up_applies_it() {
    for flag in [MigrationFlag::OneWay, MigrationFlag::TwoWay] {
        let project = Project::new(flag, Mode::Strict).await;
        project
            .run(init(false, project.reversible()), Resources, allow_empty())
            .await;
        project.assert_state(&["migration_init"], &[]).await;

        let (up, down) = first_migration(&project);
        for statement in UP {
            assert!(up.contains(statement), "{flag:?}: missing `{statement}`");
        }
        // A model's record id is not a defined field.
        assert!(!up.contains("DEFINE FIELD OVERWRITE id ON animal"));
        assert!(!up.contains("DEFINE FIELD OVERWRITE id ON planet"));
        assert_eq!(up.contains(CHECKSUM_DOWN), project.reversible(), "{flag:?}");
        if let Some(down) = down {
            for table in ["animal", "crop", "eats", "migration", "planet", "student"] {
                assert!(down.contains(&format!("REMOVE TABLE {table};")), "{down}");
            }
        }

        project.run(Up::default(), Resources, allow_empty()).await;
        project
            .assert_state(&["migration_init"], &["migration_init"])
            .await;
        assert_eq!(project.schema().await, schema_of(&Resources, flag).await);
    }
}

/// `init --run` applies the first migration; an `up` after it changes nothing. Strict and lax
/// mode check the applied history differently.
#[tokio::test]
async fn init_with_run_applies_the_first_migration() {
    for flag in [MigrationFlag::OneWay, MigrationFlag::TwoWay] {
        for mode in [Mode::Strict, Mode::Lax] {
            let project = Project::new(flag, mode).await;
            project
                .run(init(true, project.reversible()), Resources, allow_empty())
                .await;
            project
                .assert_state(&["migration_init"], &["migration_init"])
                .await;

            project.run(Up::default(), Resources, allow_empty()).await;
            project
                .assert_state(&["migration_init"], &["migration_init"])
                .await;
        }
    }
}

#[tokio::test]
#[should_panic(
    expected = "Migrations already initialized. Run 'cargo run -- reset' to reset migration. \
                You can also specify the '-r' or '--reversible' argument to set as reversible. \
                Or delete the migrations directory and run 'cargo run -- init' again."
)]
async fn a_second_init_is_refused() {
    let project = Project::new(MigrationFlag::OneWay, Mode::Strict).await;
    project
        .run(init(false, false), Resources, allow_empty())
        .await;
    project
        .run(
            Init::builder()
                .name("another name".into())
                .reversible(false)
                .run(false)
                .build(),
            Resources,
            allow_empty(),
        )
        .await;
}

const BUCKET: &str = "DEFINE BUCKET avatars BACKEND 'memory';";

#[derive(Debug, Clone)]
struct Avatars;

impl DbResources for Avatars {
    fn buckets(&self) -> Vec<Raw> {
        vec![Raw::new(BUCKET)]
    }
}

/// A bucket, which needs the experimental `files` feature the main server allows, is defined
/// by the first migration.
#[tokio::test]
async fn init_applies_a_bucket() {
    let project = Project::new(MigrationFlag::OneWay, Mode::Strict).await;
    project
        .run(
            Init::builder()
                .name("avatars".into())
                .reversible(false)
                .run(true)
                .build(),
            Avatars,
            allow_empty(),
        )
        .await;
    project.assert_state(&["avatars"], &["avatars"]).await;
    // A plain `DEFINE` fails only where the bucket already exists.
    assert!(
        !succeeds(&project.db, BUCKET).await,
        "the migration defined the bucket"
    );
}
