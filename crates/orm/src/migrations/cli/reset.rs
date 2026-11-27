use clap::Args;
use std::fs;
use typed_builder::TypedBuilder;

use crate::DbResources;
use crate::migrations::*;

/// Resets migrations. Deletes all migration files, migration table and reinitializes
/// migrations.
#[derive(Args, Debug, TypedBuilder, Clone)]
pub struct Reset {
    /// Name of the first migration file(s) to reinitialize to
    #[arg(long)]
    pub(crate) name: Basename,

    /// Whether or not to run the migrations after reinitialization. Reinitalization
    /// is done by deleting all migration files, and regenerating
    /// the first migration file(s) which include queries to delete all old
    /// migration metadata in the database before creating the new ones.
    #[arg(long)]
    pub(crate) run: bool,

    /// Reinitialize as two-way migrations (an up and a down file each) instead of one-way ones
    #[arg(short, long)]
    pub(crate) reversible: bool,
}

impl Reset {
    /// Whether the reinitialized migrations are two-way.
    pub fn reversible(&self) -> bool {
        self.reversible
    }

    /// Replaces the migration directory's files with a first migration generated like `init`'s,
    /// and applies it if asked. The migration is generated before anything is deleted, so one
    /// that cannot be generated, e.g. for a redacted secret, leaves the existing files in place.
    ///
    /// The first migration is generated from an empty schema and applied to the existing
    /// database, so it defines every resource again. A sequence is defined without `OVERWRITE`,
    /// which would reset the values it has issued, so applying the migration to a database that
    /// already holds the sequence fails. The migration runs in one transaction, so the database
    /// keeps its definitions and its old history; but the directory has already been replaced,
    /// and rolling back the database does not restore it. A failed `reset --run` leaves the
    /// regenerated files beside the unchanged history.
    ///
    /// # Panics
    ///
    /// If generation fails or the directory cannot be recreated.
    pub async fn run(
        &self,
        cli: &mut Migrator,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) {
        let file_manager = cli.file_manager().set_flag(if self.reversible {
            MigrationFlag::TwoWay
        } else {
            MigrationFlag::OneWay
        });
        let comparison = cli.comparison().await;
        let migration = MigratorDatabase::first_migration(
            &comparison,
            &self.name,
            &file_manager,
            codebase_resources,
            prompter,
        )
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to generate migrations: {e}");
            panic!("Failed to generate migrations: {e}");
        });

        let replaced = file_manager
            .get_migration_dir_create_if_none()
            .and_then(|dir| {
                fs::remove_dir_all(&dir)
                    .and_then(|()| fs::create_dir(&dir))
                    .map_err(|e| MigrationError::IoError(e.to_string()))
            })
            .and_then(|()| migration.create_file(&file_manager));
        if let Err(e) = replaced {
            log::error!("Failed to replace the migrations: {e}");
            panic!("Failed to replace the migrations: {e}");
        }
        log::info!("Migration directory recreated.");

        if self.run {
            cli.setup_db().await;
            log::info!("Running initial migrations");
            Up::default().run(cli).await;
        }

        log::info!("Reset successful");
    }
}
