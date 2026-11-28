//! `unlock` (`orm/src/migrations/cli/unlock.rs`): releasing the history's lock, which a prune
//! holds while it deletes files and leaves behind when it is interrupted.

use pretty_assertions::assert_eq;
use umbriel::{migrations::*, test_support::lock_history};
use umbriel_tests::migrations::{helpers::*, *};

/// A two-way project with an applied init and its history locked by an interrupted prune.
async fn locked() -> Project {
    let project = Project::new(MigrationFlag::TwoWay, Mode::Strict).await;
    project
        .run(
            Init::new("migration init".into(), true, true),
            Resources,
            allow_empty(),
        )
        .await;
    lock_history(&project.db, "prune-interrupted")
        .await
        .expect("the history is locked");
    project
}

async fn assert_locked(project: &Project) {
    let error = project
        .try_run(Up::default(), Resources, allow_empty())
        .await
        .expect_err("the history is locked");
    assert!(
        matches!(&error, MigrationError::HistoryLocked { holder } if holder == "prune-interrupted"),
        "{error:?}"
    );
}

#[tokio::test]
async fn unlock_names_the_holder_and_releases_the_lock() {
    let project = locked().await;
    assert_locked(&project).await;

    let prompter = allow_empty();
    project
        .run(Unlock::default(), Resources, prompter.clone())
        .await;
    assert_eq!(
        prompter.confirmations(),
        [vec!["prune-interrupted".to_owned()]]
    );
    project.run(Up::default(), Resources, allow_empty()).await;
}

#[tokio::test]
async fn a_declined_unlock_keeps_the_lock() {
    let project = locked().await;
    let error = project
        .try_run(
            Unlock::default(),
            Resources,
            allow_empty().confirming(false),
        )
        .await
        .expect_err("declined");
    assert!(matches!(error, MigrationError::Declined), "{error:?}");
    assert_locked(&project).await;
}

/// `--yes` releases without asking; with no lock held, nothing is asked or changed.
#[tokio::test]
async fn unlock_with_yes_or_without_a_lock_does_not_ask() {
    let project = locked().await;
    let declining = allow_empty().confirming(false);
    project
        .run(Unlock::default().yes(), Resources, declining.clone())
        .await;
    project
        .run(Unlock::default(), Resources, declining.clone())
        .await;
    assert!(declining.confirmations().is_empty(), "nothing is asked");
    project.run(Up::default(), Resources, allow_empty()).await;
}
