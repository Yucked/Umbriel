//! The migration CLI (`orm/src/migrations/cli/mod.rs`, `config.rs`): commands parsed from
//! arguments, as a migrator binary receives them, connecting to a server as a database user.

mod down;
mod generate;
mod init;
mod list;
mod prune;
mod reset;
mod up;

use clap::Parser;
use pretty_assertions::assert_eq;
use umbriel::migrations::{Init, MigrationFlag, Migrator, Mode};
use umbriel_tests::{
    Endpoint,
    migrations::{Resources, helpers::*},
    succeeds,
};

fn migrator(args: &[&str]) -> Migrator {
    Migrator::try_parse_from(std::iter::once("migrator").chain(args.iter().copied()))
        .expect("the arguments parse")
}

/// `up` connects with `--url`, `--ns`, `--db`, `--user` and `--pass` as a database user, and
/// applies what `init` wrote.
#[tokio::test]
async fn up_connects_as_a_database_user_and_applies_the_migrations() {
    let project = Project::new(MigrationFlag::TwoWay, Mode::Strict).await;
    project
        .run(
            Init::new("test migration 1".into(), true, false),
            Resources,
            allow_empty(),
        )
        .await;
    assert!(
        succeeds(
            &project.db,
            "DEFINE USER migrator ON DATABASE PASSWORD 'example-password' ROLES OWNER;"
        )
        .await,
        "the database user is defined"
    );
    let database: String = umbriel_tests::read(&project.db, "RETURN session::db();").await;

    let url = Endpoint::Main.url();
    let dir = project.dir.to_str().expect("a UTF-8 path");
    let mut up = migrator(&[
        "up",
        "--dir",
        dir,
        "--url",
        &url,
        "--ns",
        umbriel_tests::NAMESPACE,
        "--db",
        &database,
        "--user",
        "migrator",
        "--pass",
        "example-password",
    ]);
    up.run_test(None::<Resources>, allow_empty()).await;
    assert_eq!(applied(up.db()).await.len(), 1);
    project
        .assert_state(&["test_migration_1"], &["test_migration_1"])
        .await;
}

#[test]
fn an_unknown_experimental_feature_is_refused() {
    let parsed = Migrator::try_parse_from(["migrator", "up", "--allow-experimental", "graphql"]);
    assert!(parsed.is_err(), "{parsed:?}");
}
