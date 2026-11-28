use crate::migrations::{
    database::{db_runner::prune_confirmed, migration_model::HistoryReading},
    *,
};
use clap::Args;

/// Delete Unapplied local migration files that have not been applied to the current database instance
/// cargo run -- prune
#[derive(Args, Debug, Clone, Default)]
pub struct Prune {
    /// Delete without asking for confirmation
    #[arg(long)]
    pub(crate) yes: bool,
}

impl Prune {
    /// Deletes without asking for confirmation.
    pub fn yes(mut self) -> Self {
        self.yes = true;
        self
    }

    /// Deletes the migration files not applied to the database, once `prompter` confirms the
    /// list (unless `--yes`). They are deleted under the history's lock, taken only if the
    /// history is still the one the list was made from.
    ///
    /// # Errors
    ///
    /// [`MigrationError::NoHistory`] when the database has no history table,
    /// [`MigrationError::Declined`], [`MigrationError::HistoryChanged`] when the history changed
    /// after the list was made, and the failure of connecting or deleting a file.
    pub async fn run(&self, cli: &mut Migrator, prompter: impl Prompter) -> MigrationResult<()> {
        cli.setup_db().await?;
        let file_manager = cli.file_manager();
        let db = cli.db()?;
        let reading = HistoryReading::read(db).await?;
        reading.check_unlocked()?;
        let selection = MigrationRunner::pending_filenames(&reading, &file_manager)?.into_vec();
        if selection.is_empty() {
            log::info!("No unapplied migration files to prune");
            return Ok(());
        }
        let listed = selection
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        confirmed(
            &prompter,
            self.yes,
            "Delete these unapplied migration files",
            &listed,
        )?;
        prune_confirmed(db, &file_manager, &reading, &selection).await?;
        log::info!("Prune successful");
        Ok(())
    }
}
