use clap::Args;
use std::path::{Path, PathBuf};

use crate::DbResources;
use crate::migrations::{
    database::{migration_model::HistoryReading, replacement},
    *,
};

/// Resets migrations. Deletes all migration files, migration table and reinitializes
/// migrations.
#[derive(Args, Debug, Clone)]
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

    /// Replace the migration files without asking for confirmation
    #[arg(long)]
    pub(crate) yes: bool,
}

impl Reset {
    /// Resets to a first migration named `name`, two-way when `reversible`, applying it
    /// afterwards when `run`.
    pub fn new(name: Basename, reversible: bool, run: bool) -> Self {
        Self {
            name,
            run,
            reversible,
            yes: false,
        }
    }

    /// Replaces the migration files without asking for confirmation.
    pub fn yes(mut self) -> Self {
        self.yes = true;
        self
    }

    /// The directory a reset of the migration directory `dir` keeps the replaced migration
    /// files in, beside `dir`, until its migration is applied.
    pub fn backup_dir(dir: &Path) -> PathBuf {
        replacement::reset_backup_dir(dir)
    }

    /// Whether the reinitialized migrations are two-way.
    pub fn reversible(&self) -> bool {
        self.reversible
    }

    /// Replaces the migration directory's migration files with a first migration generated like
    /// `init`'s, and applies it if asked. Other files in the directory stay.
    ///
    /// The files to replace are confirmed with `prompter` (unless `--yes`), and the migration is
    /// generated before anything changes, so one that cannot be generated, e.g. for a redacted
    /// secret, leaves the existing files in place. The new migration is written to a staging
    /// directory, and the replaced files are moved to a backup directory beside the
    /// migration directory ([`backup_dir`](Self::backup_dir)), with a record of the history the
    /// reset read and of its migration. The backup stays until the migration is applied, by
    /// `--run` or a later `up --replace-history`, which apply it only onto that history.
    ///
    /// The first migration is generated from an empty schema and applied to the existing
    /// database, so it defines every resource again. A sequence is defined without `OVERWRITE`,
    /// which would reset the values it has issued, so applying the migration to a database that
    /// already holds the sequence fails. The migration runs in one transaction, so the database
    /// keeps its definitions and its old history, and a failed `--run` then restores the
    /// replaced files from the backup.
    ///
    /// # Errors
    ///
    /// [`MigrationError::Declined`]; [`MigrationError::ResetIncomplete`] when an earlier reset is
    /// unfinished or this one cannot settle its files; the failure of connecting, generating,
    /// writing or moving the files, or applying the migration.
    pub async fn run(
        &self,
        cli: &mut Migrator,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<()> {
        let file_manager = cli.file_manager().set_flag(if self.reversible {
            MigrationFlag::TwoWay
        } else {
            MigrationFlag::OneWay
        });
        let dir = file_manager.get_migration_dir_create_if_none()?;
        let replaced = migration_filenames_in(&dir)?
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        confirmed(
            &prompter,
            self.yes,
            "Replace these migration files",
            &replaced,
        )?;
        let comparison = cli.comparison().await?;
        let migration = MigratorDatabase::first_migration(
            &comparison,
            &self.name,
            &file_manager,
            codebase_resources,
            prompter,
        )
        .await?;
        cli.setup_db().await?;
        let db = cli.db()?;
        let reading = HistoryReading::read(db).await?;
        reading.check_unlocked()?;
        replacement::stage_reset(&dir, &file_manager, &reading, &migration)?;
        log::info!("Migration files replaced.");

        if self.run {
            log::info!("Running the reset's migration");
            if let Err(error) = replacement::replace_history(db, &file_manager).await {
                return replacement::settle_failed_reset(db, &dir, error).await;
            }
        }

        log::info!("Reset successful");
        Ok(())
    }
}
