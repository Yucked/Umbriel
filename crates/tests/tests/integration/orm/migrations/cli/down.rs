//! `down` (`orm/src/migrations/cli/down.rs`): rolling two-way migrations back to the previous one,
//! by number or till a migration, with `--prune` deleting the files of what is no longer
//! applied. Each step is checked against the schema of the codebase version it returns to.

use pretty_assertions::assert_eq;
use umbriel::migrations::*;
use umbriel_tests::migrations::{helpers::*, *};

fn down(strategy: RollbackStrategyStruct, prune: bool) -> Down {
    Down::builder().strategy(strategy).prune(prune).build()
}

fn previous(prune: bool) -> Down {
    down(
        RollbackStrategyStruct::builder().previous(true).build(),
        prune,
    )
}

fn by_default(prune: bool) -> Down {
    down(RollbackStrategyStruct::default(), prune)
}

fn by_number(number: u32, prune: bool) -> Down {
    down(
        RollbackStrategyStruct::builder().number(number).build(),
        prune,
    )
}

fn till(filename: MigrationFilename, prune: bool) -> Down {
    down(
        RollbackStrategyStruct::builder().till(filename).build(),
        prune,
    )
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

/// A two-way project with all twelve migrations applied.
async fn applied(mode: Mode) -> Project {
    let project = Project::new(MigrationFlag::TwoWay, mode).await;
    generate_twelve(&project).await;
    project.run(Up::default(), Resources, allow_empty()).await;
    assert_at(&project, 12, 12).await;
    project
}

/// The previous migration, named explicitly or by default, rolls back one at a time, in strict
/// and lax mode, down to an empty history; `--prune` deletes the files of every migration no
/// longer applied.
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
        project
            .run(by_default(true), Resources, allow_empty())
            .await;
        assert_at(&project, 8, 8).await;
        for applied in (3..8).rev() {
            project
                .run(by_default(false), Resources, allow_empty())
                .await;
            assert_at(&project, 8, applied).await;
        }
        project
            .run(by_default(true), Resources, allow_empty())
            .await;
        assert_at(&project, 2, 2).await;
        project
            .run(by_default(true), Resources, allow_empty())
            .await;
        assert_at(&project, 1, 1).await;
        project
            .run(by_default(false), Resources, allow_empty())
            .await;
        assert_at(&project, 1, 0).await;
        project
            .run(by_default(true), Resources, allow_empty())
            .await;
        assert_at(&project, 0, 0).await;
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
        .run(by_number(5, false), Resources, allow_empty())
        .await;
    assert_at(&project, 12, 7).await;
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
#[should_panic(expected = "Rollback Failed")]
async fn rolling_back_till_a_rolled_back_migration_fails() {
    let project = applied(Mode::Strict).await;
    let newest = project.filename_at(12);
    project
        .run(till(newest.clone(), false), Resources, allow_empty())
        .await;
    assert_at(&project, 12, 11).await;
    project
        .run(till(newest, false), Resources, allow_empty())
        .await;
}

#[tokio::test]
#[should_panic]
async fn rolling_back_till_a_migration_that_does_not_exist_fails() {
    let project = applied(Mode::Strict).await;
    let missing = MigrationFilename::try_from(
        "20231220050955_this_migration_does_not_exist.up.surql".to_owned(),
    )
    .expect("a migration file name");
    project
        .run(till(missing, false), Resources, allow_empty())
        .await;
}
