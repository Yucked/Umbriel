//! `list` (`orm/src/migrations/cli/list.rs`): the migrations by status, and listing changing
//! nothing.

use pretty_assertions::assert_eq;
use umbriel::migrations::*;
use umbriel_tests::migrations::{helpers::*, *};

/// Twelve migrations, the first five applied.
async fn five_of_twelve(flag: MigrationFlag, mode: Mode) -> Project {
    let project = Project::new(flag, mode).await;
    generate_twelve(&project).await;
    project
        .run(Up::new(UpdateStrategy::Number(5)), Resources, allow_empty())
        .await;
    project.assert_state(&TWELVE, first(5)).await;
    project
}

/// The migrations listed with `status`, as `list` reads them.
async fn listed(project: &Project, status: Status) -> Vec<MigrationFilename> {
    let config = config(&project.dir);
    match project.flag {
        MigrationFlag::TwoWay => {
            config
                .two_way()
                .list_migrations(&project.db, status, project.mode)
                .await
        }
        MigrationFlag::OneWay => {
            config
                .one_way()
                .list_migrations(&project.db, status, project.mode)
                .await
        }
    }
    .expect("the migrations are listed")
}

#[tokio::test]
async fn migrations_are_listed_by_status() {
    for flag in [MigrationFlag::OneWay, MigrationFlag::TwoWay] {
        for mode in [Mode::Strict, Mode::Lax] {
            let project = five_of_twelve(flag, mode).await;
            assert_eq!(listed(&project, Status::All).await.len(), 12);
            assert_eq!(listed(&project, Status::Pending).await.len(), 7);
            assert_eq!(listed(&project, Status::Applied).await.len(), 5);
        }
    }
}

#[tokio::test]
async fn listing_changes_nothing() {
    for flag in [MigrationFlag::OneWay, MigrationFlag::TwoWay] {
        let project = five_of_twelve(flag, Mode::Strict).await;
        for status in [Status::All, Status::Pending, Status::Applied] {
            project
                .run(List::new(status), Resources, allow_empty())
                .await;
        }
        project.assert_state(&TWELVE, first(5)).await;
    }
}
