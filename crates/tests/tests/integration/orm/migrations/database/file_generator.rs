//! The migration generator (`orm/src/migrations/database/file_generator.rs`, #32): it compares
//! schemas in two databases it creates in the caller's comparison namespace, removes them whether
//! generation succeeded or failed, and creates nothing a user without the right may not.

use std::path::Path;

use clap::Parser;
use surrealdb::{engine::any, opt::auth::Namespace};
use tempfile::tempdir;
use umbriel::{
    DbResources, Raw,
    migrations::{
        COMPARISON_PREFIX, Comparison, MigrationConfig, MigrationError, Migrator, MockPrompter,
        Mode, RenameOrDelete,
    },
};
use umbriel_tests::{migrations::Resources, *};

fn prompter() -> MockPrompter {
    MockPrompter::builder()
        .allow_empty_migrations_gen(true)
        .rename_or_delete_single_field_change(RenameOrDelete::Rename)
        .build()
}

fn config(dir: &Path) -> MigrationConfig {
    MigrationConfig::builder()
        .mode(Mode::Strict)
        .custom_path(Some(dir.to_path_buf()))
        .build()
}

/// A codebase whose only table SurrealDB refuses to define.
struct Unparsable;

impl DbResources for Unparsable {
    fn tables(&self) -> Vec<Raw> {
        vec![Raw::new("DEFINE TABLE;")]
    }
}

#[tokio::test]
async fn generation_removes_its_comparison_databases() {
    let (comparison, guard) = comparison().await;
    let temp = tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    config(&dir)
        .one_way()
        .generate_migrations(&comparison, &"init".into(), Resources, prompter())
        .await
        .expect("the migration is generated");

    let files = std::fs::read_dir(&dir).expect("the directory exists").count();
    assert!(files > 0, "a migration file is written");
    let root = connect(Endpoint::Main).await;
    assert_eq!(databases_in(&root, guard.namespace()).await, Vec::<String>::new());
}

#[tokio::test]
async fn a_failed_generation_removes_its_comparison_databases_and_keeps_its_cause() {
    let (comparison, guard) = comparison().await;
    let temp = tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    let error = config(&dir)
        .one_way()
        .generate_migrations(&comparison, &"init".into(), Unparsable, prompter())
        .await
        .expect_err("SurrealDB refuses the table");

    assert!(
        !matches!(error, MigrationError::ComparisonNotRemoved { .. }),
        "the generation's own failure: {error:?}"
    );
    let root = connect(Endpoint::Main).await;
    assert_eq!(databases_in(&root, guard.namespace()).await, Vec::<String>::new());
}

/// A namespace viewer may not define databases: the generator fails before it has created any.
#[tokio::test]
async fn a_user_who_may_not_define_databases_creates_nothing() {
    let (root, guard) = namespace().await;
    let viewer = root.clone();
    bounded("selecting the namespace", viewer.use_ns(guard.namespace())).await;
    assert!(
        succeeds(
            &viewer,
            "DEFINE USER viewer ON NAMESPACE PASSWORD 'example-password' ROLES VIEWER;"
        )
        .await,
        "the viewer is defined"
    );
    let db = bounded(
        "connecting as the viewer",
        any::connect(Endpoint::Main.url().as_str()),
    )
    .await;
    bounded(
        "signing in as the viewer",
        db.signin(Namespace {
            namespace: guard.namespace().to_owned(),
            username: "viewer".to_owned(),
            password: "example-password".to_owned(),
        }),
    )
    .await;

    let temp = tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    let error = config(&dir)
        .one_way()
        .generate_migrations(
            &Comparison::new(db, guard.namespace()),
            &"init".into(),
            Resources,
            prompter(),
        )
        .await
        .expect_err("the viewer may not define a database");

    assert!(
        error.to_string().to_lowercase().contains("permission"),
        "{error}"
    );
    let databases = databases_in(&root, guard.namespace()).await;
    assert!(
        databases.iter().all(|name| !name.starts_with(COMPARISON_PREFIX)),
        "{databases:?}"
    );
}

/// `init`, `generate` and `reset` compare schemas, so each refuses to run without
/// `--comparison-ns`.
mod the_cli_needs_a_comparison_namespace {
    use super::*;

    async fn run(command: &[&str]) {
        let temp = tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        let dir = dir.to_str().expect("a UTF-8 path");
        let mut cli = Migrator::try_parse_from(
            ["umbriel", "--dir", dir].into_iter().chain(command.iter().copied()),
        )
        .expect("the arguments parse");
        cli.run_test(Some(Resources), prompter()).await;
    }

    #[tokio::test]
    #[should_panic(expected = "--comparison-ns")]
    async fn for_init() {
        run(&["init", "--name", "init"]).await;
    }

    #[tokio::test]
    #[should_panic(expected = "--comparison-ns")]
    async fn for_generate() {
        run(&["generate", "--name", "next"]).await;
    }

    #[tokio::test]
    #[should_panic(expected = "--comparison-ns")]
    async fn for_reset() {
        run(&["reset", "--name", "init"]).await;
    }
}
