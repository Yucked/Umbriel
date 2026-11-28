//! `prune` (`orm/src/migrations/cli/prune.rs`): deleting the files of migrations not applied,
//! and only those, once the user confirms the list.

use pretty_assertions::assert_eq;
use umbriel::migrations::*;
use umbriel_tests::migrations::{helpers::*, *};

/// The base names a confirmation listed, each once, in order.
fn listed(files: &[String]) -> Vec<Basename> {
    let mut names = files
        .iter()
        .map(|name| {
            MigrationFilename::try_from(name.clone())
                .expect("a migration file name")
                .basename()
        })
        .collect::<Vec<_>>();
    names.dedup();
    names
}

/// A project of `flag` with all twelve migrations written and the first five applied.
async fn five_applied(flag: MigrationFlag) -> Project {
    let project = Project::new(flag, Mode::Strict).await;
    generate_twelve(&project).await;
    project
        .run(Up::new(UpdateStrategy::Number(5)), Resources, allow_empty())
        .await;
    project
}

#[tokio::test]
async fn prune_deletes_only_unapplied_migrations_once_confirmed() {
    for flag in [MigrationFlag::OneWay, MigrationFlag::TwoWay] {
        let project = five_applied(flag).await;
        let prompter = allow_empty();
        project
            .run(Prune::default(), Resources, prompter.clone())
            .await;
        project.assert_state(first(5), first(5)).await;

        let confirmations = prompter.confirmations();
        assert_eq!(confirmations.len(), 1, "{confirmations:?}");
        let expected = TWELVE[5..]
            .iter()
            .map(|&name| Basename::from(name.to_owned()))
            .collect::<Vec<_>>();
        assert_eq!(listed(&confirmations[0]), expected);
        let per_migration = if project.reversible() { 2 } else { 1 };
        assert_eq!(confirmations[0].len(), expected.len() * per_migration);
    }
}

/// Regression (UMB-010): a missing history table read as an empty history, so prune against
/// the wrong database deleted every migration file.
#[tokio::test]
async fn prune_without_a_history_table_deletes_nothing() {
    for flag in [MigrationFlag::OneWay, MigrationFlag::TwoWay] {
        let project = Project::new(flag, Mode::Strict).await;
        generate_twelve(&project).await;
        let prompter = allow_empty();
        let error = project
            .try_run(Prune::default(), Resources, prompter.clone())
            .await
            .expect_err("refused");
        assert!(matches!(error, MigrationError::NoHistory), "{error:?}");
        project.assert_state(&TWELVE, &[]).await;
        assert!(prompter.confirmations().is_empty(), "nothing is asked");
    }
}

#[tokio::test]
async fn a_declined_prune_deletes_nothing() {
    let project = five_applied(MigrationFlag::TwoWay).await;
    let error = project
        .try_run(Prune::default(), Resources, allow_empty().confirming(false))
        .await
        .expect_err("declined");
    assert!(matches!(error, MigrationError::Declined), "{error:?}");
    project.assert_state(&TWELVE, first(5)).await;
}

/// `--yes` deletes without asking.
#[tokio::test]
async fn prune_with_yes_does_not_ask() {
    let project = five_applied(MigrationFlag::TwoWay).await;
    let prompter = allow_empty().confirming(false);
    project
        .run(Prune::default().yes(), Resources, prompter.clone())
        .await;
    project.assert_state(first(5), first(5)).await;
    assert!(prompter.confirmations().is_empty(), "nothing is asked");
}
