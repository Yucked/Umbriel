use clap::Args;

use crate::migrations::{database::migration_model::HistoryReading, *};

/// Release the migration history's lock, which an interrupted prune leaves behind
#[derive(Args, Debug, Clone, Default)]
pub struct Unlock {
    /// Release without asking for confirmation
    #[arg(long)]
    pub(crate) yes: bool,
}

impl Unlock {
    /// Releases without asking for confirmation.
    pub fn yes(mut self) -> Self {
        self.yes = true;
        self
    }

    /// Releases the history's lock, once `prompter` confirms its holder (unless `--yes`). A
    /// prune holds the lock while it deletes files, so release it only when no prune is
    /// running. Without a lock, nothing is asked or changed.
    ///
    /// # Errors
    ///
    /// [`MigrationError::Declined`], and the failure of connecting or releasing the lock.
    pub async fn run(&self, cli: &mut Migrator, prompter: impl Prompter) -> MigrationResult<()> {
        cli.setup_db().await?;
        let db = cli.db()?;
        let Some(holder) = HistoryReading::read(db).await?.lock else {
            log::info!("The migration history is not locked");
            return Ok(());
        };
        confirmed(
            &prompter,
            self.yes,
            "Release the migration history's lock held by",
            std::slice::from_ref(&holder),
        )?;
        MigrationRunner::release_lock(db, &holder).await?;
        log::info!("Released the lock held by `{holder}`");
        Ok(())
    }
}
