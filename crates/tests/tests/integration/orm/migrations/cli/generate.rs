//! `generate` (`orm/src/migrations/cli/generate.rs`): a migration from the difference between
//! the directory and the codebase, applied when asked; renames and deletions in both directions;
//! empty migrations only when the prompter allows them; and the renames generation refuses.

use pretty_assertions::assert_eq;
use umbriel::{DbResources, create_table_resources, migrations::*};
use umbriel_tests::migrations::{helpers::*, *};

const FLAGS: [MigrationFlag; 2] = [MigrationFlag::OneWay, MigrationFlag::TwoWay];

fn init(project: &Project, run: bool) -> Init {
    Init::new("migration init".into(), project.reversible(), run)
}

fn generate(name: &'static str, run: bool) -> Generate {
    Generate::new(name.into(), run)
}

#[tokio::test]
async fn generate_before_init_is_refused() {
    let project = Project::new(MigrationFlag::OneWay, Mode::Strict).await;
    let error = project
        .try_run(generate("migration gen 1", false), Resources, allow_empty())
        .await
        .expect_err("refused");
    assert!(
        matches!(error, MigrationError::MigrationDirectoryDoesNotExist(_)),
        "{error:?}"
    );
}

/// `generate` writes a migration either way, and applies the pending ones only with `--run`.
#[tokio::test]
async fn generation_applies_only_when_asked() {
    for flag in FLAGS {
        let project = Project::new(flag, Mode::Strict).await;
        project
            .run(init(&project, false), Resources, allow_empty())
            .await;
        project
            .run(generate("migration gen 1", false), Resources, allow_empty())
            .await;
        project
            .assert_state(&["migration_init", "migration_gen_1"], &[])
            .await;

        let project = Project::new(flag, Mode::Strict).await;
        project
            .run(init(&project, true), Resources, allow_empty())
            .await;
        project
            .run(generate("migration gen 1", false), Resources, allow_empty())
            .await;
        project
            .assert_state(&["migration_init", "migration_gen_1"], &["migration_init"])
            .await;
        project
            .run(generate("migration gen 2", true), Resources, allow_empty())
            .await;
        let all = ["migration_init", "migration_gen_1", "migration_gen_2"];
        project.assert_state(&all, &all).await;
    }
}

#[derive(Debug, Clone)]
struct Before;

impl DbResources for Before {
    create_table_resources!(Animal, Planet);
}

/// `AnimalV2` renames `attributes` to `characteristics` with `old_name` and drops the
/// timestamps; `PlanetV2` replaces `firstName` with `newName`, a single changed field the
/// prompter decides on.
#[derive(Debug, Clone)]
struct After;

impl DbResources for After {
    create_table_resources!(AnimalV2, PlanetV2);
}

/// An explicit rename (`old_name`) always moves the data, up and down; a single replaced field
/// is renamed or deleted as the prompter says. A one-way migration has the same up statements and
/// no down file.
#[tokio::test]
async fn renames_and_deletions_are_generated_in_both_directions() {
    let cases = [
        (MigrationFlag::TwoWay, RenameOrDelete::Rename),
        (MigrationFlag::TwoWay, RenameOrDelete::Delete),
        (MigrationFlag::OneWay, RenameOrDelete::Rename),
    ];
    for (flag, strategy) in cases {
        let prompter = MockPrompter::new(true, strategy);
        let project = Project::new(flag, Mode::Strict).await;
        project.run(init(&project, false), Before, prompter).await;
        project
            .run(generate("migration gen 1", false), After, prompter)
            .await;
        project
            .assert_state(&["migration_init", "migration_gen_1"], &[])
            .await;

        let newest = migration_files(&project.dir)
            .into_iter()
            .rfind(|name| !name.ends_with(".down.surql"))
            .expect("a migration");
        let up = std::fs::read_to_string(project.dir.join(&newest)).expect("readable");
        let case = format!("{flag:?} {strategy:?}");
        assert!(
            up.contains(
                "DEFINE FIELD OVERWRITE characteristics ON animal TYPE array<string> PERMISSIONS FULL;\
                 \nUPDATE animal SET characteristics = attributes;\
                 \nREMOVE FIELD attributes ON TABLE animal;"
            ),
            "{case}: the explicit rename:\n{up}"
        );
        assert!(
            up.contains("REMOVE FIELD createdAt ON TABLE animal;"),
            "{case}"
        );
        assert!(
            up.contains("REMOVE FIELD updatedAt ON TABLE animal;"),
            "{case}"
        );
        match strategy {
            RenameOrDelete::Rename => assert!(
                up.contains(
                    "DEFINE FIELD OVERWRITE newName ON planet TYPE string PERMISSIONS FULL;\
                     \nUPDATE planet SET newName = firstName;\
                     \nREMOVE FIELD firstName ON TABLE planet;"
                ),
                "{case}: the implicit rename:\n{up}"
            ),
            RenameOrDelete::Delete => {
                // Definitions come before removals.
                assert_order(
                    &up,
                    &[
                        "DEFINE FIELD OVERWRITE newName ON planet TYPE string PERMISSIONS FULL;",
                        "REMOVE FIELD firstName ON TABLE planet;",
                    ],
                );
                assert!(
                    !up.contains("UPDATE planet SET newName = firstName;"),
                    "{case}"
                );
            }
        }

        if !project.reversible() {
            continue;
        }
        let (_, down) = newest_migration(&project.dir);
        assert!(
            down.contains(
                "DEFINE FIELD OVERWRITE attributes ON animal TYPE array<string> PERMISSIONS FULL;\
                 \nUPDATE animal SET attributes = characteristics;\
                 \nREMOVE FIELD characteristics ON TABLE animal;"
            ),
            "{case}: the explicit rename reversed:\n{down}"
        );
        match strategy {
            RenameOrDelete::Rename => assert!(
                down.contains(
                    "DEFINE FIELD OVERWRITE firstName ON planet TYPE string PERMISSIONS FULL;\
                     \nUPDATE planet SET firstName = newName;\
                     \nREMOVE FIELD newName ON TABLE planet;"
                ),
                "{case}: the implicit rename reversed:\n{down}"
            ),
            RenameOrDelete::Delete => assert!(
                down.contains("REMOVE FIELD newName ON TABLE planet;"),
                "{case}: the deletion reversed:\n{down}"
            ),
        }
    }
}

/// Without changes, no migration is written unless the prompter allows an empty one.
#[tokio::test]
async fn an_empty_migration_is_written_only_when_allowed() {
    let refuse_empty = || MockPrompter::new(false, RenameOrDelete::Delete);
    for flag in FLAGS {
        let project = Project::new(flag, Mode::Strict).await;
        project
            .run(init(&project, true), Resources, refuse_empty())
            .await;
        for _ in 0..2 {
            project
                .run(generate("no change", true), Resources, refuse_empty())
                .await;
            project
                .assert_state(&["migration_init"], &["migration_init"])
                .await;
        }

        project
            .run(generate("allowed empty", true), Resources, allow_empty())
            .await;
        let all = ["migration_init", "allowed_empty"];
        project.assert_state(&all, &all).await;
    }
}

/// Once a rename is applied, the leftover `old_name` attributes are history: generating again
/// finds no change, and the empty migration the prompter allows applies.
#[tokio::test]
async fn regenerating_after_an_applied_rename_finds_no_change() {
    for flag in FLAGS {
        let project = Project::new(flag, Mode::Strict).await;
        project
            .run(init(&project, true), Resources, MockPrompter::default())
            .await;
        project
            .run(
                generate("migration 2 gen", true),
                ResourcesV2,
                MockPrompter::default(),
            )
            .await;
        project
            .run(
                generate("migration 3 gen", true),
                ResourcesV2,
                MockPrompter::default(),
            )
            .await;
        let all = ["migration_init", "migration_2_gen", "migration_3_gen"];
        project.assert_state(&all, &all).await;
        assert_eq!(project.schema().await, schema_of(&ResourcesV2, flag).await);
    }
}

/// Generates from `resources` after an applied first migration of `Resources`.
/// The error of generating from `resources` after an applied `init`.
async fn generate_after_init(resources: impl DbResources) -> MigrationError {
    let project = Project::new(MigrationFlag::OneWay, Mode::Strict).await;
    project
        .run(init(&project, true), Resources, MockPrompter::default())
        .await;
    project
        .try_run(
            generate("migration 2 gen", true),
            resources,
            MockPrompter::default(),
        )
        .await
        .expect_err("the generation is refused")
}

#[tokio::test]
async fn two_fields_renamed_from_the_same_old_field_are_refused() {
    let error =
        generate_after_init(invalid_cases::ResourcesVRenamingWithSameOldFieldNameDisallowed).await;
    assert!(
        matches!(
            error,
            MigrationError::RenamingToSameOldFieldDisallowed { .. }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn renaming_from_a_field_still_in_use_is_refused() {
    let error =
        generate_after_init(invalid_cases::ResourcesVRenamingFromCurrentlyUsedFieldNameDisallowed)
            .await;
    assert!(
        matches!(
            error,
            MigrationError::CannotRenameFromOldFieldInUSeInTable { .. }
        ),
        "{error:?}"
    );
}

/// `AnimalV2` renames `attributes`, which the empty migration directory never defined.
#[tokio::test]
async fn renaming_from_a_field_the_directory_never_defined_is_refused() {
    let project = Project::new(MigrationFlag::OneWay, Mode::Strict).await;
    let error = project
        .try_run(init(&project, true), ResourcesV2, MockPrompter::default())
        .await
        .expect_err("the generation is refused");
    assert!(
        matches!(error, MigrationError::InvalidOldFieldName { .. }),
        "{error:?}"
    );
}
