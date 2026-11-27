use crate::migrations::*;
use clap::Args;

/// Delete Unapplied local migration files that have not been applied to the current database instance
/// cargo run -- prune
#[derive(Args, Debug, Clone)]
pub struct Prune;

impl Prune {
    /// Deletes the migration files not applied to the database.
    ///
    /// # Errors
    ///
    /// The failure of connecting, reading the history or deleting a file.
    pub async fn run(&self, cli: &mut Migrator) -> MigrationResult<()> {
        cli.setup_db().await?;
        let file_manager = cli.file_manager().set_mode(Mode::Lax);
        MigrationRunner::delete_unapplied_migration_files(cli.db()?, &file_manager).await?;
        log::info!("Prune successful");
        Ok(())
    }
}
