//! `up` (`orm/src/migrations/cli/up.rs`): applying pending migrations to the latest, by number or
//! till a migration, and the targets it refuses. Each step is checked against the schema of the
//! codebase version it reaches.

use pretty_assertions::assert_eq;
use umbriel::migrations::*;
use umbriel_tests::migrations::{helpers::*, *};

const FLAGS: [MigrationFlag; 2] = [MigrationFlag::OneWay, MigrationFlag::TwoWay];

fn by_number(number: u32) -> Up {
    Up::new(UpdateStrategy::Number(number))
}

fn till(filename: MigrationFilename) -> Up {
    Up::new(UpdateStrategy::Till(filename))
}

/// Asserts the project has all twelve files and the first `count` applied, with their schema.
async fn assert_applied(project: &Project, count: usize) {
    project.assert_state(&TWELVE, first(count)).await;
    if count > 0 {
        assert_eq!(
            project.schema().await,
            schema_at(count, project.flag).await,
            "the schema after {count} migrations"
        );
    }
}

#[tokio::test]
async fn up_before_init_is_refused() {
    let project = Project::new(MigrationFlag::OneWay, Mode::Strict).await;
    let error = project
        .try_run(Up::default(), Resources, allow_empty())
        .await
        .expect_err("refused");
    assert!(
        matches!(error, MigrationError::MigrationDirectoryDoesNotExist(_)),
        "{error:?}"
    );
}

/// `up` without a target applies every pending migration, in strict and lax mode alike, and an
/// `up` after it applies nothing.
#[tokio::test]
async fn up_applies_every_pending_migration() {
    for flag in FLAGS {
        for mode in [Mode::Strict, Mode::Lax] {
            let project = Project::new(flag, mode).await;
            generate_twelve(&project).await;
            assert_applied(&project, 0).await;

            project.run(Up::default(), Resources, allow_empty()).await;
            assert_applied(&project, 12).await;
            project
                .run(Up::new(UpdateStrategy::Latest), Resources, allow_empty())
                .await;
            assert_applied(&project, 12).await;
        }
    }
}

/// `--number` applies that many pending migrations; zero applies none, and more than are pending
/// applies the rest.
#[tokio::test]
async fn up_by_number_applies_that_many() {
    for flag in FLAGS {
        let project = Project::new(flag, Mode::Strict).await;
        generate_twelve(&project).await;
        for (number, applied) in [(1, 1), (5, 6), (0, 6), (1, 7), (5, 12), (1000, 12)] {
            project
                .run(by_number(number), Resources, allow_empty())
                .await;
            assert_applied(&project, applied).await;
        }
    }
}

/// `--till` applies the pending migrations up to and including the one named.
#[tokio::test]
async fn up_till_applies_through_the_named_migration() {
    for flag in FLAGS {
        let project = Project::new(flag, Mode::Strict).await;
        generate_twelve(&project).await;
        for position in [5, 9, 12] {
            project
                .run(
                    till(project.filename_at(position)),
                    Resources,
                    allow_empty(),
                )
                .await;
            assert_applied(&project, position).await;
        }
        project.run(Up::default(), Resources, allow_empty()).await;
        assert_applied(&project, 12).await;
    }
}

/// Targets combined: till, then by number, then till again, then the latest.
#[tokio::test]
async fn up_targets_combine() {
    for flag in FLAGS {
        let project = Project::new(flag, Mode::Strict).await;
        generate_twelve(&project).await;
        project
            .run(till(project.filename_at(3)), Resources, allow_empty())
            .await;
        assert_applied(&project, 3).await;
        project.run(by_number(4), Resources, allow_empty()).await;
        assert_applied(&project, 7).await;
        project
            .run(till(project.filename_at(9)), Resources, allow_empty())
            .await;
        assert_applied(&project, 9).await;
        project
            .run(Up::new(UpdateStrategy::Latest), Resources, allow_empty())
            .await;
        assert_applied(&project, 12).await;
    }
}

/// `--till` names a pending migration: one already applied, one before it, or one that doesn't
/// exist is refused.
mod till_refuses {
    use super::*;

    async fn applied_through(position: usize) -> Project {
        let project = Project::new(MigrationFlag::OneWay, Mode::Strict).await;
        generate_twelve(&project).await;
        project
            .run(
                till(project.filename_at(position)),
                Resources,
                allow_empty(),
            )
            .await;
        project
    }

    /// Asserts `up --till target` is refused because `target` is not pending.
    async fn refused(project: &Project, target: MigrationFilename) {
        let error = project
            .try_run(till(target), Resources, allow_empty())
            .await
            .expect_err("refused");
        assert!(
            matches!(
                error,
                MigrationError::MigrationNotFoundFromPendingMigrations(_)
            ),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn an_applied_migration() {
        let project = applied_through(12).await;
        refused(&project, project.filename_at(12)).await;
    }

    #[tokio::test]
    async fn an_older_migration() {
        let project = applied_through(12).await;
        refused(&project, project.filename_at(5)).await;
    }

    #[tokio::test]
    async fn a_migration_that_does_not_exist() {
        let project = Project::new(MigrationFlag::OneWay, Mode::Strict).await;
        generate_twelve(&project).await;
        let missing = MigrationFilename::try_from(
            "20231220050955_this_migration_does_not_exist.surql".to_owned(),
        )
        .expect("a migration file name");
        refused(&project, missing).await;
    }
}

/// A reset's migration clears the old history. `up` applies it only when asked, and only onto
/// the history the reset replaced.
mod replace_history {
    use super::*;
    use pretty_assertions::assert_eq;

    /// A two-way project in `mode` with an applied init, reset without `--run`.
    async fn reset_without_run(mode: Mode) -> Project {
        let project = Project::new(MigrationFlag::TwoWay, mode).await;
        project
            .run(
                Init::new("migration init".into(), true, true),
                Resources,
                allow_empty(),
            )
            .await;
        project
            .run(
                Reset::new("migration reset".into(), true, false),
                ResourcesV3,
                allow_empty(),
            )
            .await;
        project
    }

    /// Without `--replace-history`, neither mode lets the reset's migration clear the history.
    #[tokio::test]
    async fn up_without_the_flag_refuses_the_reset_migration() {
        for mode in [Mode::Strict, Mode::Lax] {
            let project = reset_without_run(mode).await;
            let error = project
                .try_run(Up::default(), ResourcesV3, allow_empty())
                .await
                .expect_err("refused");
            match mode {
                Mode::Strict => assert!(
                    matches!(error, MigrationError::AppliedFileMissing { .. }),
                    "{error:?}"
                ),
                Mode::Lax => assert!(
                    matches!(error, MigrationError::HistoryRewritten { .. }),
                    "{error:?}"
                ),
            }
            assert_eq!(project.files(), [Basename::from("migration_reset")]);
            assert_eq!(project.history().await, [Basename::from("migration_init")]);
        }
    }

    /// A history that is neither the one the reset replaced nor its replacement, here cleared by
    /// hand, as on another database: nothing is applied and the backup is kept.
    #[tokio::test]
    async fn a_history_the_reset_did_not_read_is_refused() {
        let project = reset_without_run(Mode::Strict).await;
        assert!(
            umbriel_tests::succeeds(&project.db, "DELETE migration;").await,
            "the history is cleared"
        );
        let error = project
            .try_run(Up::default().replace_history(), ResourcesV3, allow_empty())
            .await
            .expect_err("refused");
        assert!(
            matches!(error, MigrationError::ResetIncomplete { .. }),
            "{error:?}"
        );
        assert!(project.history().await.is_empty(), "nothing is applied");
        assert!(
            Reset::backup_dir(&project.dir).exists(),
            "the backup is kept"
        );
    }

    #[tokio::test]
    async fn the_flag_without_a_reset_is_refused() {
        let project = Project::new(MigrationFlag::TwoWay, Mode::Strict).await;
        generate_twelve(&project).await;
        let error = project
            .try_run(Up::default().replace_history(), Resources, allow_empty())
            .await
            .expect_err("refused");
        assert!(matches!(error, MigrationError::NoPendingReset), "{error:?}");
        project.assert_state(&TWELVE, &[]).await;
    }
}
