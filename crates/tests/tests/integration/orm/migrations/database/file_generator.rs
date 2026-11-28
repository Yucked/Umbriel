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
    MockPrompter::new(true, RenameOrDelete::Rename)
}

fn config(dir: &Path) -> MigrationConfig {
    MigrationConfig::new()
        .set_mode(Mode::Strict)
        .set_custom_path(dir)
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
    let (history, _history) = db().await;
    config(&dir)
        .one_way()
        .generate_migrations(comparison, &history, &"next".into(), Resources, prompter())
        .await
        .expect_err("the replay is refused")
}

#[tokio::test]
async fn generation_removes_its_comparison_databases() {
    let (comparison, guard) = comparison().await;
    let temp = tempfile::tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    let (history, _history) = db().await;
    config(&dir)
        .one_way()
        .generate_migrations(&comparison, &history, &"init".into(), Resources, prompter())
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
    let (history, _history) = db().await;
    config(&dir)
        .one_way()
        .generate_migrations(&comparison, &history, &"init".into(), Resources, prompter())
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
    let (history, _history) = db().await;
    let error = config(&dir)
        .one_way()
        .generate_migrations(
            &comparison,
            &history,
            &"init".into(),
            Unparsable,
            prompter(),
        )
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
    let (history, _history) = db().await;
    let error = config(&dir)
        .one_way()
        .generate_migrations(&comparison, &history, &"init".into(), Resources, prompter())
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
    let (history, _history) = db().await;
    let error = config(&dir)
        .one_way()
        .generate_migrations(&comparison, &history, &"init".into(), Resources, prompter())
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
        Err(MigrationError::InvalidTimestamp("synthetic".to_owned()))
    })
    .await
    .expect_err("the preparation fails");

    assert!(
        matches!(&error, MigrationError::InvalidTimestamp(message) if message == "synthetic"),
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
        Err::<(), _>(MigrationError::InvalidTimestamp("synthetic".to_owned())),
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
        matches!(generation.as_deref(), Some(MigrationError::InvalidTimestamp(message)) if message == "synthetic"),
        "{generation:?}"
    );
}

/// `init`, `generate` and `reset` compare schemas, so each refuses to run without
/// `--comparison-ns`.
mod the_cli_needs_a_comparison_namespace {
    use super::*;

    /// Runs `command` and asserts it is refused for the missing `--comparison-ns`.
    async fn refused(command: &[&str]) {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        let dir = dir.to_str().expect("a UTF-8 path");
        let mut cli = Migrator::try_parse_from(
            ["umbriel", "--dir", dir]
                .into_iter()
                .chain(command.iter().copied()),
        )
        .expect("the arguments parse");
        let error = cli
            .run_fn(Resources, prompter())
            .await
            .expect_err("the command is refused");
        assert!(
            matches!(
                error,
                MigrationError::MissingArgument {
                    argument: "--comparison-ns"
                }
            ),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn for_init() {
        refused(&["init", "--name", "init"]).await;
    }

    #[tokio::test]
    async fn for_generate() {
        refused(&["generate", "--name", "next"]).await;
    }

    #[tokio::test]
    async fn for_reset() {
        refused(&["reset", "--name", "init"]).await;
    }
}

/// Generation checks the history of the database `up` applies to before it creates anything:
/// the applied files must be the ones applied, and no pending file may sort before the newest
/// applied one, in either mode, since `up` refuses to apply it there.
mod history {
    use super::*;
    use umbriel::migrations::Basename;
    use umbriel_tests::migrations::helpers::{apply_up, migration_files, write_numbered};

    /// A two-way directory with migrations 1 and 3 applied to the returned database.
    async fn applied_one_and_three(dir: &Path) -> (Surreal<Any>, TestDb) {
        let (db, guard) = db().await;
        write_numbered(
            dir,
            1,
            "DEFINE TABLE person SCHEMAFULL;",
            "REMOVE TABLE person;",
        );
        write_numbered(
            dir,
            3,
            "DEFINE FIELD name ON person TYPE string;",
            "REMOVE FIELD name ON person;",
        );
        apply_up(dir, &db).await;
        (db, guard)
    }

    async fn generate_in(
        comparison: &Comparison<Any>,
        dir: &Path,
        mode: Mode,
        history: &Surreal<Any>,
    ) -> umbriel::migrations::MigrationResult<()> {
        config(dir)
            .set_mode(mode)
            .two_way()
            .generate_migrations(comparison, history, &"next".into(), Resources, prompter())
            .await
    }

    /// A namespace viewer may not create the comparison databases, so a history failure, rather
    /// than "not allowed", shows the history is checked first.
    #[tokio::test]
    async fn an_edited_applied_file_fails_strict_generation_before_any_comparison() {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        let (db, _guard) = applied_one_and_three(&dir).await;
        write_numbered(
            &dir,
            3,
            "DEFINE FIELD name ON person TYPE int;",
            "REMOVE FIELD name ON person;",
        );
        let files = migration_files(&dir);

        let (viewer, _root, _namespace) = comparison_as("VIEWER").await;
        let error = generate_in(&viewer, &dir, Mode::Strict, &db)
            .await
            .expect_err("the edited file is refused");
        assert!(
            matches!(error, MigrationError::ChecksumMismatch { .. }),
            "{error:?}"
        );
        assert_eq!(migration_files(&dir), files, "nothing is written");
    }

    #[tokio::test]
    async fn an_edited_applied_file_lets_lax_generation_proceed() {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        let (db, _guard) = applied_one_and_three(&dir).await;
        write_numbered(
            &dir,
            3,
            "DEFINE FIELD name ON person TYPE int;",
            "REMOVE FIELD name ON person;",
        );
        let before = migration_files(&dir).len();

        let (comparison, _namespace) = comparison().await;
        generate_in(&comparison, &dir, Mode::Lax, &db)
            .await
            .expect("lax generation proceeds");
        assert_eq!(
            migration_files(&dir).len(),
            before + 2,
            "a migration is written"
        );
    }

    #[tokio::test]
    async fn a_deleted_applied_file_fails_strict_generation() {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        let (db, _guard) = applied_one_and_three(&dir).await;
        let third = write_numbered(
            &dir,
            3,
            "DEFINE FIELD name ON person TYPE string;",
            "REMOVE FIELD name ON person;",
        );
        fs::remove_file(dir.join(&third)).expect("removed");
        fs::remove_file(dir.join(third.replace(".up.", ".down."))).expect("removed");
        let files = migration_files(&dir);

        let (viewer, _root, _namespace) = comparison_as("VIEWER").await;
        let error = generate_in(&viewer, &dir, Mode::Strict, &db)
            .await
            .expect_err("the missing file is refused");
        assert!(
            matches!(error, MigrationError::AppliedFileMissing { .. }),
            "{error:?}"
        );
        assert_eq!(migration_files(&dir), files, "nothing is written");
    }

    /// Migration 2 redefines the field migration 3 defines, so the order they replay in decides
    /// the schema; `up` refuses 2 after 3, and so does generation, in both modes.
    #[tokio::test]
    async fn an_out_of_order_file_fails_generation_in_both_modes() {
        for mode in [Mode::Strict, Mode::Lax] {
            let temp = tempfile::tempdir().expect("a temporary directory");
            let dir = temp.path().join("migrations");
            let (db, _guard) = applied_one_and_three(&dir).await;
            write_numbered(
                &dir,
                2,
                "DEFINE FIELD OVERWRITE name ON person TYPE int;",
                "REMOVE FIELD name ON person;",
            );
            let files = migration_files(&dir);

            let (viewer, _root, _namespace) = comparison_as("VIEWER").await;
            let error = generate_in(&viewer, &dir, mode, &db)
                .await
                .expect_err("the out-of-order file is refused");
            assert!(
                matches!(
                    &error,
                    MigrationError::OutOfOrder { name, .. }
                        if name.basename() == Basename::from("step_2")
                ),
                "{mode:?}: {error:?}"
            );
            assert_eq!(migration_files(&dir), files, "{mode:?}: nothing is written");
        }
    }
}

/// Audit item 6: a migration that removes tables or fields deletes their data, which no rollback
/// restores, so it lists them and asks before it is written.
mod data_removal {
    use super::*;
    use umbriel_tests::migrations::helpers::{generate, migration_files, newest_migration};

    /// A codebase where `gadget` has lost both its fields.
    #[derive(Debug, Clone)]
    struct Bare;

    impl DbResources for Bare {
        fn tables(&self) -> Vec<Raw> {
            vec![Raw::new("DEFINE TABLE gadget TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;")]
        }
    }

    fn with_two_fields(dir: &Path) {
        write_migration(
            dir,
            MigrationFlag::TwoWay,
            "DEFINE TABLE gadget TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;\n\
             DEFINE FIELD colour ON gadget TYPE string;\n\
             DEFINE FIELD weight ON gadget TYPE int;",
            "REMOVE TABLE gadget;",
        );
    }

    #[tokio::test]
    async fn the_removed_fields_are_listed_and_confirmed() {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        with_two_fields(&dir);
        let prompter = MockPrompter::default();
        generate(&dir, MigrationFlag::TwoWay, Bare, prompter.clone())
            .await
            .expect("generated");

        let removals = [
            "REMOVE FIELD colour ON gadget".to_owned(),
            "REMOVE FIELD weight ON gadget".to_owned(),
        ];
        assert_eq!(prompter.confirmations(), [removals.to_vec()]);
        let (up, _) = newest_migration(&dir);
        for removal in &removals {
            assert!(up.contains(&format!("Deletes data: {removal}")), "{up}");
        }
    }

    #[tokio::test]
    async fn a_declined_removal_writes_nothing() {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        with_two_fields(&dir);
        let files = migration_files(&dir);
        let error = generate(
            &dir,
            MigrationFlag::TwoWay,
            Bare,
            MockPrompter::default().confirming(false),
        )
        .await
        .expect_err("declined");
        assert!(matches!(error, MigrationError::Declined), "{error:?}");
        assert_eq!(migration_files(&dir), files);
    }
}
