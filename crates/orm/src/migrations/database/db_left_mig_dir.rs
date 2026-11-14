use std::ops::Deref;

use surrealdb::{Connection, Surreal};

use super::db_runner::run_transaction;
use crate::migrations::*;

/// The scratch database the migration directory is replayed into, the left side of a diff.
#[derive(Debug, Clone)]
pub struct LeftDatabase(
    /// The scratch database.
    pub MigratorDatabase,
);

impl Deref for LeftDatabase {
    type Target = MigratorDatabase;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl LeftDatabase {
    /// Applies every two-way migration's up file in order, each in its own transaction.
    pub async fn run_twoway_up_migrations(
        &self,
        file_manager: &MigrationConfig,
        create_migration_table: bool,
    ) -> MigrationResult<()> {
        for migration in file_manager.get_two_way_migrations_sorted_asc(create_migration_table)? {
            run_transaction(self.db(), migration.up.content.to_string()).await?;
        }
        Ok(())
    }

    /// Applies `migrations` to `db` in order, each in its own transaction.
    pub async fn run_local_dir_oneway_content_migrations<C: Connection>(
        db: &Surreal<C>,
        migrations: Vec<MigrationFileOneWay>,
    ) -> MigrationResult<()> {
        for migration in migrations {
            log::info!("Running migration: {}", migration.name());
            run_transaction(db, migration.content().to_string()).await?;
        }
        Ok(())
    }

    /// Applies every one-way migration in order, each in its own transaction.
    pub async fn run_oneway_migrations(
        &self,
        fm: &MigrationConfig,
        create_migration_table: bool,
    ) -> MigrationResult<&Self> {
        for migration in fm.get_oneway_migrations_sorted_asc(create_migration_table)? {
            run_transaction(self.db(), migration.content().to_string()).await?;
        }
        Ok(self)
    }
}
