//! The migration generator (`orm/src/migrations/database/file_generator.rs`, #32): it compares
//! schemas in two databases it creates in the caller's comparison namespace, as a user confined to
//! each, removes them whether generation succeeded or failed, and creates nothing a user without
//! the right may not.

use std::{cell::Cell, fs, future::Future, path::Path, pin::Pin};

use clap::Parser;
use surrealdb::{
    Surreal,
    engine::any::{self, Any},
    opt::auth::Namespace,
    types::Value,
};
use umbriel::{
    DbResources, Raw,
    migrations::{
        COMPARISON_PREFIX, Comparison, ComparisonDatabase, MigrationConfig, MigrationError,
        MigrationFlag, Migrator, MockPrompter, Mode, RenameOrDelete,
    },
    test_support::{init_comparison_with, remove_comparison},
};
use umbriel_tests::{
    migrations::{Resources, helpers::write_migration},
    *,
};

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

/// A new namespace, a root connection, and a comparison in that namespace signed in as a
/// namespace user with `role`.
async fn comparison_as(role: &str) -> (Comparison<Any>, Surreal<Any>, TestNamespace) {
    let (root, guard) = namespace().await;
    let admin = root.clone();
    bounded("selecting the namespace", admin.use_ns(guard.namespace())).await;
    assert!(
        succeeds(
            &admin,
            &format!(
                "DEFINE USER generator ON NAMESPACE PASSWORD 'example-password' ROLES {role};"
            )
        )
        .await,
        "the {role} is defined"
    );
    let db = bounded(
        "connecting as the namespace user",
        any::connect(Endpoint::Main.url().as_str()),
    )
    .await;
    bounded(
        "signing in as the namespace user",
        db.signin(Namespace {
            namespace: guard.namespace().to_owned(),
            username: "generator".to_owned(),
            password: "example-password".to_owned(),
        }),
    )
    .await;
    (Comparison::new(db, guard.namespace()), root, guard)
}

/// The users defined on `namespace`, as `INFO FOR NS` lists them, read as root.
async fn namespace_users(root: &Surreal<Any>, namespace: &str) -> Value {
    let reader = root.clone();
    bounded("selecting the namespace", reader.use_ns(namespace)).await;
    // A `RETURN` needs a database selected; the `INFO` statement itself does not.
    let Value::Object(info) = read::<Value>(&reader, "INFO FOR NS;").await else {
        panic!("`INFO FOR NS` returns an object");
    };
    info.get("users")
        .cloned()
        .unwrap_or_else(|| panic!("`INFO FOR NS` lists users: {info:?}"))
}

/// Generates from a one-way migration directory holding `up`, and returns the failure.
async fn generation_failure(comparison: &Comparison<Any>, up: &str) -> MigrationError {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    write_migration(&dir, MigrationFlag::OneWay, up, "");
    config(&dir)
        .one_way()
        .generate_migrations(comparison, &"next".into(), Resources, prompter())
        .await
        .expect_err("the replay is refused")
}

#[tokio::test]
async fn generation_removes_its_comparison_databases() {
    let (comparison, guard) = comparison().await;
    let temp = tempfile::tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    config(&dir)
        .one_way()
        .generate_migrations(&comparison, &"init".into(), Resources, prompter())
        .await
        .expect("the migration is generated");

    let files = fs::read_dir(&dir).expect("the directory exists").count();
    assert!(files > 0, "a migration file is written");
    let root = connect(Endpoint::Main).await;
    assert_eq!(
        databases_in(&root, guard.namespace()).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn the_generators_users_never_enter_a_migration() {
    let (comparison, _guard) = comparison().await;
    let temp = tempfile::tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    config(&dir)
        .one_way()
        .generate_migrations(&comparison, &"init".into(), Resources, prompter())
        .await
        .expect("the migration is generated");

    for entry in fs::read_dir(&dir).expect("the directory exists") {
        let path = entry.expect("a directory entry").path();
        let content = fs::read_to_string(&path).expect("the migration is readable");
        assert!(
            !content.contains(COMPARISON_PREFIX),
            "{}: {content}",
            path.display()
        );
    }
}

#[tokio::test]
async fn a_failed_generation_removes_its_comparison_databases_and_keeps_its_cause() {
    let (comparison, guard) = comparison().await;
    let temp = tempfile::tempdir().expect("a temporary directory");
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
    assert_eq!(
        databases_in(&root, guard.namespace()).await,
        Vec::<String>::new()
    );
}

/// The replay runs as a user confined to its comparison database, so SurrealDB refuses a
/// namespace-level definition rather than the generator finding it afterwards.
#[tokio::test]
async fn a_migration_cannot_add_a_namespace_user() {
    let (comparison, guard) = comparison().await;
    let root = connect(Endpoint::Main).await;
    let before = namespace_users(&root, guard.namespace()).await;

    let error = generation_failure(
        &comparison,
        "DEFINE USER intruder ON NAMESPACE PASSWORD 'example-password' ROLES OWNER;",
    )
    .await;

    assert!(
        !matches!(error, MigrationError::ComparisonNotRemoved { .. }),
        "{error:?}"
    );
    assert_eq!(namespace_users(&root, guard.namespace()).await, before);
    assert_eq!(
        databases_in(&root, guard.namespace()).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_migration_cannot_remove_or_redefine_a_namespace_user() {
    let (comparison, guard) = comparison().await;
    let root = connect(Endpoint::Main).await;
    let admin = root.clone();
    bounded("selecting the namespace", admin.use_ns(guard.namespace())).await;
    assert!(
        succeeds(
            &admin,
            "DEFINE USER alice ON NAMESPACE PASSWORD 'example-password' ROLES VIEWER;"
        )
        .await,
        "alice is defined"
    );
    let before = namespace_users(&root, guard.namespace()).await;

    for up in [
        "REMOVE USER alice ON NAMESPACE;",
        "DEFINE USER OVERWRITE alice ON NAMESPACE PASSWORD 'example-password' ROLES OWNER;",
    ] {
        let error = generation_failure(&comparison, up).await;
        assert!(
            !matches!(error, MigrationError::ComparisonNotRemoved { .. }),
            "{up}: {error:?}"
        );
        assert_eq!(
            namespace_users(&root, guard.namespace()).await,
            before,
            "{up}"
        );
    }
    assert_eq!(
        databases_in(&root, guard.namespace()).await,
        Vec::<String>::new()
    );
}

/// A namespace viewer may not define databases: the generator fails before it has created any.
#[tokio::test]
async fn a_user_who_may_not_define_databases_creates_nothing() {
    let (comparison, root, guard) = comparison_as("VIEWER").await;
    let temp = tempfile::tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    let error = config(&dir)
        .one_way()
        .generate_migrations(&comparison, &"init".into(), Resources, prompter())
        .await
        .expect_err("the viewer may not define a database");

    assert!(error.to_string().contains("not allowed"), "{error}");
    let databases = databases_in(&root, guard.namespace()).await;
    assert!(
        databases
            .iter()
            .all(|name| !name.starts_with(COMPARISON_PREFIX)),
        "{databases:?}"
    );
}

/// A namespace editor may define databases but not the generator's users, so it cannot generate,
/// and the databases it created are removed.
#[tokio::test]
async fn a_namespace_editor_cannot_generate() {
    let (comparison, root, guard) = comparison_as("EDITOR").await;
    let temp = tempfile::tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    let error = config(&dir)
        .one_way()
        .generate_migrations(&comparison, &"init".into(), Resources, prompter())
        .await
        .expect_err("the editor may not define the generator's users");

    assert!(error.to_string().contains("not allowed"), "{error}");
    let databases = databases_in(&root, guard.namespace()).await;
    assert!(
        databases
            .iter()
            .all(|name| !name.starts_with(COMPARISON_PREFIX)),
        "{databases:?}"
    );
}

/// The failure goes through the same creation and error handling as `ComparisonDatabase::init`;
/// only the preparation step is the test's.
#[tokio::test]
async fn a_failure_after_both_creations_removes_both() {
    let (comparison, guard) = comparison().await;
    let root = connect(Endpoint::Main).await;
    let error = init_comparison_with(&comparison, async |_: &mut ComparisonDatabase<Any>| {
        let databases = databases_in(&root, guard.namespace()).await;
        assert_eq!(
            databases
                .iter()
                .filter(|name| name.starts_with(COMPARISON_PREFIX))
                .count(),
            2,
            "{databases:?}"
        );
        Err(MigrationError::IoError("synthetic".to_owned()))
    })
    .await
    .expect_err("the preparation fails");

    assert!(
        matches!(&error, MigrationError::IoError(message) if message == "synthetic"),
        "{error:?}"
    );
    assert_eq!(
        databases_in(&root, guard.namespace()).await,
        Vec::<String>::new()
    );
}

/// One comparison database's removal, as `remove_comparison` takes it.
type Removal<'a> = Pin<Box<dyn Future<Output = Result<(), MigrationError>> + 'a>>;

/// Paused time runs the removal bound out at once on a removal that never answers.
#[tokio::test(start_paused = true)]
async fn a_removal_that_times_out_still_removes_the_rest() {
    let answered = Cell::new(false);
    let removals: [(String, Removal<'_>); 2] = [
        ("stalled".to_owned(), Box::pin(std::future::pending())),
        (
            "answered".to_owned(),
            Box::pin(async {
                answered.set(true);
                Ok(())
            }),
        ),
    ];
    let error = remove_comparison(
        "comparison",
        removals,
        Err::<(), _>(MigrationError::IoError("synthetic".to_owned())),
    )
    .await
    .expect_err("a removal timed out");

    assert!(answered.get(), "the second removal runs");
    let MigrationError::ComparisonNotRemoved {
        databases,
        cleanup,
        generation,
        ..
    } = error
    else {
        panic!("expected ComparisonNotRemoved, got {error:?}");
    };
    assert_eq!(databases, ["stalled"]);
    assert!(
        matches!(&*cleanup, MigrationError::ComparisonTimedOut { database } if database == "stalled"),
        "{cleanup:?}"
    );
    assert!(
        matches!(generation.as_deref(), Some(MigrationError::IoError(message)) if message == "synthetic"),
        "{generation:?}"
    );
}

/// `init`, `generate` and `reset` compare schemas, so each refuses to run without
/// `--comparison-ns`.
mod the_cli_needs_a_comparison_namespace {
    use super::*;

    async fn run(command: &[&str]) {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        let dir = dir.to_str().expect("a UTF-8 path");
        let mut cli = Migrator::try_parse_from(
            ["umbriel", "--dir", dir]
                .into_iter()
                .chain(command.iter().copied()),
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
