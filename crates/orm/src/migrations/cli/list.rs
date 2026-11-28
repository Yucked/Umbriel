use clap::Args;

use crate::migrations::*;

/// List migrations according to status
#[derive(Args, Debug, Clone)]
pub struct List {
    /// Status of migrations to list: applied, pending or all
    #[arg(value_enum, long, default_value_t = Status::Applied)]
    pub(crate) status: Status,
}

impl List {
    /// Lists the migrations with `status`.
    pub fn new(status: Status) -> Self {
        Self { status }
    }

    /// Logs the migrations with the requested status.
    ///
    /// # Errors
    ///
    /// The failure of connecting, detecting the migration type or reading the history, such as
    /// [`MigrationError::InvalidMigrationState`] in strict mode.
    pub async fn run(&self, cli: &mut Migrator) -> MigrationResult<()> {
        cli.setup_db().await?;
        let db = cli.db()?;
        let file_manager = cli.file_manager();

        let migrations = match file_manager.detect_migration_type()? {
            MigrationFlag::TwoWay => {
                file_manager
                    .two_way()
                    .list_migrations(db, self.status, cli.mode)
                    .await?
            }
            MigrationFlag::OneWay => {
                file_manager
                    .one_way()
                    .list_migrations(db, self.status, cli.mode)
                    .await?
            }
        };
        log::info!("Listing {} {} migrations", migrations.len(), self.status);
        for migration in migrations {
            log::info!("{migration}");
        }
        Ok(())
    }
}
