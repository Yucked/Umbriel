//! `reset` (`orm/src/migrations/cli/reset.rs`): replacing the migration directory with one first
//! migration of the codebase, applied when asked; and a failed reset leaving the directory alone.

use pretty_assertions::assert_eq;
use umbriel::migrations::*;
use umbriel_tests::migrations::{helpers::*, *};

/// Each project kind with `--run` and without it, in the mode each is checked in.
const CASES: [(MigrationFlag, Mode, bool); 4] = [
    (MigrationFlag::TwoWay, Mode::Strict, true),
    (MigrationFlag::TwoWay, Mode::Lax, false),
    (MigrationFlag::OneWay, Mode::Strict, true),
    (MigrationFlag::OneWay, Mode::Lax, false),
];

fn reset(project: &Project, run: bool) -> Reset {
    Reset::new("migration reset".into(), project.reversible(), run)
}

/// Resets `project` from `ResourcesV3` and asserts the one migration left: applied with `--run`,
/// replacing the history; otherwise the history is `kept`.
async fn reset_and_assert(project: &Project, run: bool, kept: &[&str]) {
    project
        .run(reset(project, run), ResourcesV3, MockPrompter::default())
        .await;
    assert_eq!(project.files(), [Basename::from("migration_reset")]);
    if run {
        project
            .assert_state(&["migration_reset"], &["migration_reset"])
            .await;
    } else {
        let kept = kept
            .iter()
            .map(|&name| Basename::from(name.to_owned()))
            .collect::<Vec<_>>();
        assert_eq!(project.history().await, kept);
    }
}

#[tokio::test]
async fn a_reset_before_init_writes_the_first_migration() {
    for (flag, mode, run) in CASES {
        let project = Project::new(flag, mode).await;
        project.assert_state(&[], &[]).await;
        reset_and_assert(&project, run, &[]).await;
        if run {
            assert_eq!(project.schema().await, schema_of(&ResourcesV3, flag).await);
        }
    }
}

#[tokio::test]
async fn a_reset_after_an_applied_init_replaces_it() {
    for (flag, mode, run) in CASES {
        let project = Project::new(flag, mode).await;
        project
            .run(
                Init::new("migration init".into(), project.reversible(), true),
                Resources,
                MockPrompter::default(),
            )
            .await;
        project
            .assert_state(&["migration_init"], &["migration_init"])
            .await;
        reset_and_assert(&project, run, &["migration_init"]).await;
    }
}

#[tokio::test]
async fn a_reset_replaces_unapplied_migrations() {
    for (flag, mode, run) in CASES {
        let project = Project::new(flag, mode).await;
        generate_twelve(&project).await;
        reset_and_assert(&project, run, &[]).await;
    }
}

#[tokio::test]
async fn a_reset_replaces_applied_migrations() {
    for (flag, mode, run) in CASES {
        let project = Project::new(flag, mode).await;
        generate_twelve(&project).await;
        project.run(Up::default(), Resources, allow_empty()).await;
        reset_and_assert(&project, run, &TWELVE).await;
    }
}

#[tokio::test]
async fn a_reset_replaces_partly_applied_migrations() {
    for (flag, mode, run) in CASES {
        let project = Project::new(flag, mode).await;
        generate_twelve(&project).await;
        project
            .run(Up::new(UpdateStrategy::Number(7)), Resources, allow_empty())
            .await;
        reset_and_assert(&project, run, first(7)).await;
    }
}

/// The reset's migration is generated before anything is deleted: one that fails, here for a
/// redacted secret, leaves the existing files in place.
#[tokio::test]
async fn a_failed_reset_keeps_the_existing_migrations() {
    let project = Project::new(MigrationFlag::TwoWay, Mode::Strict).await;
    project
        .run(
            Init::new("migration init".into(), true, false),
            Resources,
            MockPrompter::default(),
        )
        .await;
    let files = migration_files(&project.dir);

    let error = project
        .try_run(
            Reset::new("reset".into(), true, false),
            UserResources,
            MockPrompter::default(),
        )
        .await
        .expect_err("the reset fails");
    assert!(
        matches!(error, MigrationError::RedactedDefinition { .. }),
        "{error:?}"
    );
    assert_eq!(migration_files(&project.dir), files);
}

/// A two-way project whose init is applied.
async fn applied_init(mode: Mode) -> Project {
    let project = Project::new(MigrationFlag::TwoWay, mode).await;
    project
        .run(
            Init::new("migration init".into(), true, true),
            Resources,
            MockPrompter::default(),
        )
        .await;
    project
}

/// Regression (UMB-011): reset removed the whole directory before writing the new files.
/// Only migration files are replaced.
#[tokio::test]
async fn a_reset_keeps_other_files() {
    for run in [true, false] {
        let project = applied_init(Mode::Strict).await;
        let readme = project.dir.join("README.md");
        std::fs::write(&readme, "Migrations of the example app.").expect("written");
        project
            .run(reset(&project, run), ResourcesV3, MockPrompter::default())
            .await;
        assert_eq!(
            std::fs::read_to_string(&readme).expect("kept"),
            "Migrations of the example app.",
            "run: {run}"
        );
        // `files` reads every file as a migration.
        std::fs::remove_file(&readme).expect("removed");
        assert_eq!(project.files(), [Basename::from("migration_reset")]);
    }
}

/// Without `--run`, the replaced files stay in the backup until `up --replace-history` applies
/// the reset's migration; then the backup goes.
#[tokio::test]
async fn a_reset_without_run_keeps_its_backup_until_the_replacement() {
    let project = applied_init(Mode::Strict).await;
    let replaced = migration_files(&project.dir);
    project
        .run(reset(&project, false), ResourcesV3, MockPrompter::default())
        .await;
    let backup = Reset::backup_dir(&project.dir);
    let mut kept = replaced.clone();
    kept.push("reset.record".to_owned());
    assert_eq!(
        migration_files(&backup),
        kept,
        "the backup holds the old files and the reset's record"
    );

    project
        .run(
            Up::default().replace_history(),
            ResourcesV3,
            MockPrompter::default(),
        )
        .await;
    project
        .assert_state(&["migration_reset"], &["migration_reset"])
        .await;
    assert!(!backup.exists(), "the backup is removed");
}

#[tokio::test]
async fn a_declined_reset_changes_nothing() {
    let project = applied_init(Mode::Strict).await;
    let files = migration_files(&project.dir);
    let error = project
        .try_run(
            reset(&project, true),
            ResourcesV3,
            MockPrompter::default().confirming(false),
        )
        .await
        .expect_err("declined");
    assert!(matches!(error, MigrationError::Declined), "{error:?}");
    assert_eq!(migration_files(&project.dir), files);
    assert_eq!(project.history().await, [Basename::from("migration_init")]);
}

/// The confirmation lists the files the reset replaces; `--yes` replaces them without asking.
#[tokio::test]
async fn a_reset_confirms_the_files_it_replaces() {
    let project = applied_init(Mode::Strict).await;
    let files = migration_files(&project.dir);
    let prompter = MockPrompter::default();
    project
        .run(reset(&project, true), ResourcesV3, prompter.clone())
        .await;
    let mut listed = prompter.confirmations().concat();
    listed.sort();
    assert_eq!(listed, files);

    let declining = MockPrompter::default().confirming(false);
    project
        .run(reset(&project, true).yes(), ResourcesV3, declining.clone())
        .await;
    assert!(declining.confirmations().is_empty(), "nothing is asked");
}

/// A codebase with a sequence. A reset's migration defines it again without `OVERWRITE`, which
/// would reset the values it issued, so applying it where the sequence exists fails.
#[derive(Debug, Clone)]
struct Sequenced;

impl umbriel::DbResources for Sequenced {
    fn sequences(&self) -> Vec<umbriel::Raw> {
        vec![umbriel::Raw::new("DEFINE SEQUENCE orders;")]
    }
}

/// A failed `reset --run` leaves the history unchanged, so the replaced files come back from the
/// backup.
#[tokio::test]
async fn a_failed_reset_run_restores_the_replaced_files() {
    let project = Project::new(MigrationFlag::TwoWay, Mode::Strict).await;
    project
        .run(
            Init::new("migration init".into(), true, true),
            Sequenced,
            MockPrompter::default(),
        )
        .await;
    let files = migration_files(&project.dir);

    let error = project
        .try_run(reset(&project, true), Sequenced, MockPrompter::default())
        .await
        .expect_err("the sequence exists");
    assert!(
        !matches!(error, MigrationError::ResetIncomplete { .. }),
        "the migration's own failure: {error:?}"
    );
    assert_eq!(
        migration_files(&project.dir),
        files,
        "the files are restored"
    );
    assert_eq!(project.history().await, [Basename::from("migration_init")]);
    assert!(
        !Reset::backup_dir(&project.dir).exists(),
        "nothing is left over"
    );
}

/// The swap moves the migration files aside and the new ones in; a failed move is undone, and
/// an undo that fails too leaves every file where the error says.
mod swap {
    use std::{
        fs, io,
        path::{Path, PathBuf},
    };

    use pretty_assertions::assert_eq;
    use umbriel::{migrations::MigrationError, test_support::swap_migration_files};
    use umbriel_tests::migrations::helpers::migration_files;

    struct Dirs {
        _temp: tempfile::TempDir,
        dir: PathBuf,
        staging: PathBuf,
        backup: PathBuf,
    }

    /// Two old migrations and a README in `dir`, one new migration in `staging`.
    fn dirs() -> Dirs {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let dir = temp.path().join("migrations");
        let staging = temp.path().join("staging");
        let backup = temp.path().join("backup");
        fs::create_dir_all(&dir).expect("created");
        fs::create_dir_all(&staging).expect("created");
        for name in [
            "20240101000000001_old_1.up.surql",
            "20240101000000001_old_1.down.surql",
            "20240101000000002_old_2.up.surql",
            "20240101000000002_old_2.down.surql",
        ] {
            fs::write(dir.join(name), name).expect("written");
        }
        fs::write(dir.join("README.md"), "kept").expect("written");
        for name in [
            "20240201000000000_reset.up.surql",
            "20240201000000000_reset.down.surql",
        ] {
            fs::write(staging.join(name), name).expect("written");
        }
        Dirs {
            _temp: temp,
            dir,
            staging,
            backup,
        }
    }

    /// A rename that fails on the calls `fails` accepts and renames otherwise.
    fn renaming(fails: impl Fn(usize) -> bool) -> impl FnMut(&Path, &Path) -> io::Result<()> {
        let mut call = 0;
        move |from, to| {
            call += 1;
            if fails(call) {
                return Err(io::Error::other(format!("rename {call} fails")));
            }
            fs::rename(from, to)
        }
    }

    #[test]
    fn a_failed_move_is_undone() {
        let dirs = dirs();
        let before = migration_files(&dirs.dir);
        let result = swap_migration_files(
            &dirs.dir,
            &dirs.staging,
            &dirs.backup,
            renaming(|call| call == 3),
        );
        assert!(
            matches!(&result, Err(MigrationError::Io { .. })),
            "{result:?}"
        );
        assert_eq!(migration_files(&dirs.dir), before);
    }

    #[test]
    fn a_failed_undo_keeps_every_file() {
        let dirs = dirs();
        let total = migration_files(&dirs.dir).len() + migration_files(&dirs.staging).len();
        let result = swap_migration_files(
            &dirs.dir,
            &dirs.staging,
            &dirs.backup,
            renaming(|call| call >= 3),
        );
        let Err(MigrationError::ResetIncomplete { backup, .. }) = &result else {
            panic!("{result:?}");
        };
        assert_eq!(backup, &dirs.backup);
        let kept = [&dirs.dir, &dirs.staging, &dirs.backup]
            .iter()
            .map(|dir| migration_files(dir).len())
            .sum::<usize>();
        assert_eq!(kept, total, "no file is lost");
        assert!(dirs.dir.join("README.md").exists());
    }

    #[test]
    fn a_swap_replaces_only_migration_files() {
        let dirs = dirs();
        swap_migration_files(&dirs.dir, &dirs.staging, &dirs.backup, |from, to| {
            fs::rename(from, to)
        })
        .expect("swapped");
        assert_eq!(
            migration_files(&dirs.dir),
            [
                "20240201000000000_reset.down.surql",
                "20240201000000000_reset.up.surql",
                "README.md",
            ]
        );
        assert_eq!(migration_files(&dirs.backup).len(), 4);
    }
}
