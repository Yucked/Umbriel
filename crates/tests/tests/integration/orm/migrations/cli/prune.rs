//! `prune` (`orm/src/migrations/cli/prune.rs`): deleting the files of migrations not applied,
//! and only those.

use umbriel::migrations::*;
use umbriel_tests::migrations::{helpers::*, *};

#[tokio::test]
async fn prune_deletes_only_unapplied_migrations() {
    for flag in [MigrationFlag::OneWay, MigrationFlag::TwoWay] {
        let project = Project::new(flag, Mode::Strict).await;
        generate_twelve(&project).await;
        project.run(Prune, Resources, allow_empty()).await;
        project.assert_state(&[], &[]).await;

        let project = Project::new(flag, Mode::Strict).await;
        generate_twelve(&project).await;
        project
            .run(
                Up::new(UpdateStrategy::Number(5)),
                Resources,
                allow_empty(),
            )
            .await;
        project.run(Prune, Resources, allow_empty()).await;
        project.assert_state(first(5), first(5)).await;
    }
}
