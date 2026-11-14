use crate::migrations::*;
use clap::Args;

/// Delete Unapplied local migration files that have not been applied to the current database instance
/// cargo run -- prune
#[derive(Args, Debug, Clone)]
pub struct Prune;

impl Prune {
    /// Deletes the migration files not applied to the database.
    ///
    /// # Panics
    ///
    /// If deleting fails.
    pub async fn run(&self, cli: &mut Migrator) {
        cli.setup_db().await;
        let file_manager = cli.file_manager();
        let db = cli.db();

        let res =
            MigrationRunner::delete_unapplied_migration_files(db, &file_manager.relax()).await;

        if let Err(ref e) = res {
            log::error!("Failed to prune migrations: {e}");
            panic!();
        }

        log::info!("Prune successful");
    }
}
