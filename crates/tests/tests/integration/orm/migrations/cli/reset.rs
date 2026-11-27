//! `reset` (`orm/src/migrations/cli/reset.rs`): replacing the migration directory with one first
//! migration of the codebase, applied when asked; and a failed reset leaving the directory alone.

use std::sync::Arc;

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
    let project = Arc::new(Project::new(MigrationFlag::TwoWay, Mode::Strict).await);
    project
        .run(
            Init::new("migration init".into(), true, false),
            Resources,
            MockPrompter::default(),
        )
        .await;
    let files = migration_files(&project.dir);

    let reset = tokio::spawn({
        let project = Arc::clone(&project);
        async move {
            project
                .run(
                    Reset::new("reset".into(), true, false),
                    UserResources,
                    MockPrompter::default(),
                )
                .await;
        }
    })
    .await;
    let panic = reset.expect_err("the reset panics").into_panic();
    let message = panic
        .downcast_ref::<String>()
        .expect("a formatted panic message");
    assert!(
        message.starts_with("Failed to generate migrations"),
        "{message}"
    );
    assert_eq!(migration_files(&project.dir), files);
}
