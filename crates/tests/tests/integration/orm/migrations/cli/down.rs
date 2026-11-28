//! `down` (`orm/src/migrations/cli/down.rs`): rolling two-way migrations back to the previous one,
//! by number or till a migration, with `--prune` deleting the files of what is no longer
//! applied. Each step is checked against the schema of the codebase version it returns to.

use pretty_assertions::assert_eq;
use umbriel::migrations::*;
use umbriel_tests::migrations::{helpers::*, *};

fn previous(prune: bool) -> Down {
    Down::new(RollbackStrategy::Previous, prune)
}

/// The strategy `down` takes when no argument is given.
fn by_default(prune: bool) -> Down {
    Down::new(
        RollbackStrategy::from(&RollbackStrategyStruct::default()),
        prune,
    )
}

fn by_number(number: u32, prune: bool) -> Down {
    Down::new(RollbackStrategy::Number(number), prune)
}

fn till(filename: MigrationFilename, prune: bool) -> Down {
    Down::new(RollbackStrategy::Till(filename), prune)
}

/// Asserts the first `files` of the twelve migrations are on disk and the first `applied` are
/// applied, with their schema.
async fn assert_at(project: &Project, files: usize, applied: usize) {
    project.assert_state(first(files), first(applied)).await;
    if applied > 0 {
        assert_eq!(
            project.schema().await,
            schema_at(applied, project.flag).await,
            "the schema after rolling back to {applied}"
        );
    }
}

/// Asserts the migrations at `kept` (positions from 1) are on disk and the first `applied` are
/// applied, with their schema: a prune deletes only what it rolled back, so the files left need
/// not be the first ones.
async fn assert_kept(project: &Project, kept: &[usize], applied: usize) {
    let files = kept
        .iter()
        .map(|&position| TWELVE[position - 1])
        .collect::<Vec<_>>();
    project.assert_state(&files, first(applied)).await;
    if applied > 0 {
        assert_eq!(
            project.schema().await,
            schema_at(applied, project.flag).await,
            "the schema after rolling back to {applied}"
        );
    }
}

/// A two-way project with all twelve migrations applied.
async fn applied(mode: Mode) -> Project {
    let project = Project::new(MigrationFlag::TwoWay, mode).await;
    generate_twelve(&project).await;
    project.run(Up::default(), Resources, allow_empty()).await;
    assert_at(&project, 12, 12).await;
    project
}

/// The previous migration, named explicitly or by default, rolls back one at a time, in strict
/// and lax mode, down to an empty history; `--prune` deletes the files of the migrations it
/// rolled back, and keeps those of migrations pending before it ran.
#[tokio::test]
async fn rolling_back_the_previous_migration() {
    for mode in [Mode::Strict, Mode::Lax] {
        let project = applied(mode).await;
        project.run(previous(false), Resources, allow_empty()).await;
        assert_at(&project, 12, 11).await;
        project
            .run(by_default(false), Resources, allow_empty())
            .await;
        assert_at(&project, 12, 10).await;
        project.run(previous(false), Resources, allow_empty()).await;
        assert_at(&project, 12, 9).await;

        project.run(Up::default(), Resources, allow_empty()).await;
        assert_at(&project, 12, 12).await;
        project.run(previous(true), Resources, allow_empty()).await;
        assert_at(&project, 11, 11).await;
        project
            .run(by_default(false), Resources, allow_empty())
            .await;
        assert_at(&project, 11, 10).await;
        project.run(previous(false), Resources, allow_empty()).await;
        assert_at(&project, 11, 9).await;
        // 10 and 11 were pending before this prune, so their files stay.
        project
            .run(by_default(true), Resources, allow_empty())
            .await;
        assert_kept(&project, &[1, 2, 3, 4, 5, 6, 7, 8, 10, 11], 8).await;
        for applied in (3..8).rev() {
            project
                .run(by_default(false), Resources, allow_empty())
                .await;
            assert_kept(&project, &[1, 2, 3, 4, 5, 6, 7, 8, 10, 11], applied).await;
        }
        project
            .run(by_default(true), Resources, allow_empty())
            .await;
        assert_kept(&project, &[1, 2, 4, 5, 6, 7, 8, 10, 11], 2).await;
        project
            .run(by_default(true), Resources, allow_empty())
            .await;
        assert_kept(&project, &[1, 4, 5, 6, 7, 8, 10, 11], 1).await;
        project
            .run(by_default(false), Resources, allow_empty())
            .await;
        assert_kept(&project, &[1, 4, 5, 6, 7, 8, 10, 11], 0).await;
        // Nothing is applied, so nothing is rolled back or deleted.
        project
            .run(by_default(true), Resources, allow_empty())
            .await;
        assert_kept(&project, &[1, 4, 5, 6, 7, 8, 10, 11], 0).await;
    }
}

/// `--number` rolls back that many; more than are applied rolls back all of them.
#[tokio::test]
async fn rolling_back_by_number() {
    let project = applied(Mode::Strict).await;
    project
        .run(by_number(1, false), Resources, allow_empty())
        .await;
    assert_at(&project, 12, 11).await;
    project
        .run(by_default(false), Resources, allow_empty())
        .await;
    assert_at(&project, 12, 10).await;
    project
        .run(by_number(1, false), Resources, allow_empty())
        .await;
    assert_at(&project, 12, 9).await;

    project.run(Up::default(), Resources, allow_empty()).await;
    assert_at(&project, 12, 12).await;
    project
        .run(by_number(5, true), Resources, allow_empty())
        .await;
    assert_at(&project, 7, 7).await;
    project
        .run(by_number(3, true), Resources, allow_empty())
        .await;
    assert_at(&project, 4, 4).await;
    project
        .run(by_number(4, false), Resources, allow_empty())
        .await;
    assert_at(&project, 4, 0).await;

    project.run(Up::default(), Resources, allow_empty()).await;
    assert_at(&project, 4, 4).await;
    project
        .run(by_number(400, true), Resources, allow_empty())
        .await;
    assert_at(&project, 0, 0).await;
}

/// `--till` rolls back the named migration and every one after it.
#[tokio::test]
async fn rolling_back_till_a_migration() {
    let project = applied(Mode::Strict).await;
    project
        .run(
            till(project.filename_at(12), false),
            Resources,
            allow_empty(),
        )
        .await;
    assert_at(&project, 12, 11).await;
    project
        .run(
            till(project.filename_at(11), false),
            Resources,
            allow_empty(),
        )
        .await;
    assert_at(&project, 12, 10).await;
    project
        .run(
            till(project.filename_at(6), false),
            Resources,
            allow_empty(),
        )
        .await;
    assert_at(&project, 12, 5).await;

    project.run(Up::default(), Resources, allow_empty()).await;
    assert_at(&project, 12, 12).await;
    project
        .run(
            till(project.filename_at(12), true),
            Resources,
            allow_empty(),
        )
        .await;
    assert_at(&project, 11, 11).await;
    project
        .run(till(project.filename_at(4), true), Resources, allow_empty())
        .await;
    assert_at(&project, 3, 3).await;
    project
        .run(till(project.filename_at(1), true), Resources, allow_empty())
        .await;
    assert_at(&project, 0, 0).await;
}

/// A migration rolled back already is no longer applied, so rolling back till it again fails.
#[tokio::test]
async fn rolling_back_till_a_rolled_back_migration_fails() {
    let project = applied(Mode::Strict).await;
    let newest = project.filename_at(12);
    project
        .run(till(newest.clone(), false), Resources, allow_empty())
        .await;
    assert_at(&project, 12, 11).await;
    let error = project
        .try_run(till(newest, false), Resources, allow_empty())
        .await
        .expect_err("refused");
    assert!(
        matches!(error, MigrationError::MigrationDoesNotExist { .. }),
        "{error:?}"
    );
    assert_at(&project, 12, 11).await;
}

#[tokio::test]
async fn rolling_back_till_a_migration_that_does_not_exist_fails() {
    let project = applied(Mode::Strict).await;
    let missing = MigrationFilename::try_from(
        "20231220050955_this_migration_does_not_exist.up.surql".to_owned(),
    )
    .expect("a migration file name");
    let error = project
        .try_run(till(missing, false), Resources, allow_empty())
        .await
        .expect_err("refused");
    assert!(
        matches!(error, MigrationError::MigrationDoesNotExist { .. }),
        "{error:?}"
    );
}

mod prune {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Twelve applied, then the newest rolled back without pruning, so 12 is pending.
    async fn twelve_pending() -> Project {
        let project = applied(Mode::Strict).await;
        project.run(previous(false), Resources, allow_empty()).await;
        assert_at(&project, 12, 11).await;
        project
    }

    /// The confirmation lists exactly the files of the migrations to roll back, before any is
    /// rolled back; the pending migration 12 is neither listed nor deleted.
    #[tokio::test]
    async fn the_confirmation_lists_exactly_the_rolled_back_files() {
        let project = twelve_pending().await;
        // Named before the prune deletes them.
        let mut expected = [10, 11]
            .into_iter()
            .flat_map(|position| {
                let down = project.filename_at(position);
                [down.to_up().to_string(), down.to_string()]
            })
            .collect::<Vec<_>>();
        expected.sort();
        let prompter = allow_empty();
        project
            .run(by_number(2, true), Resources, prompter.clone())
            .await;
        assert_kept(&project, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 12], 9).await;

        let confirmations = prompter.confirmations();
        assert_eq!(confirmations.len(), 1, "{confirmations:?}");
        let mut listed = confirmations[0].clone();
        listed.sort();
        assert_eq!(listed, expected);
    }

    #[tokio::test]
    async fn a_declined_prune_rolls_back_nothing() {
        let project = twelve_pending().await;
        let error = project
            .try_run(
                by_number(2, true),
                Resources,
                allow_empty().confirming(false),
            )
            .await
            .expect_err("declined");
        assert!(matches!(error, MigrationError::Declined), "{error:?}");
        assert_at(&project, 12, 11).await;
    }

    /// `--yes` rolls back and prunes without asking; without `--prune` nothing is asked either.
    #[tokio::test]
    async fn yes_and_no_prune_do_not_ask() {
        let project = twelve_pending().await;
        let declining = allow_empty().confirming(false);
        project
            .run(by_number(1, false), Resources, declining.clone())
            .await;
        assert_at(&project, 12, 10).await;
        project
            .run(by_number(1, true).yes(), Resources, declining.clone())
            .await;
        assert_kept(&project, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12], 9).await;
        assert!(declining.confirmations().is_empty(), "nothing is asked");
    }

    /// Each migration's files are deleted only after its reversal commits: when the second
    /// reversal fails, the first one's files are gone and the failing one's stay.
    #[tokio::test]
    async fn a_failed_reversal_keeps_its_files() {
        // Lax, since the edited down file no longer matches its applied checksum.
        let project = applied(Mode::Lax).await;
        let down = project.dir.join(project.filename_at(11).to_string());
        let content = std::fs::read_to_string(&down).expect("readable");
        std::fs::write(&down, format!("{content}\nTHROW 'the rollback fails';")).expect("written");

        let error = project
            .try_run(by_number(2, true), Resources, allow_empty())
            .await
            .expect_err("the second reversal fails");
        assert!(
            !matches!(error, MigrationError::Declined),
            "the reversal's own failure: {error:?}"
        );
        project.assert_state(first(11), first(11)).await;
    }
}
