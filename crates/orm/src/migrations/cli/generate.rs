use super::up::Up;

use crate::DbResources;
use crate::migrations::*;
use clap::Args;

/// Generate migrations
#[derive(Args, Debug, Clone)]
pub struct Generate {
    /// Name of the migration
    #[arg(long)]
    pub(crate) name: Basename,

    /// Whether to run the migrations after generation
    #[arg(long)]
    pub(crate) run: bool,
}

impl Generate {
    /// Generates a migration named `name`, applying it afterwards when `run`.
    pub fn new(name: Basename, run: bool) -> Self {
        Self { name, run }
    }

    /// Generates a migration from the difference between the migration directory and the
    /// codebase's resources, after checking the directory against the history of the database
    /// `up` applies to, and applies it if asked.
    ///
    /// # Errors
    ///
    /// The failure of connecting, detecting the migration type (such as
    /// [`MigrationError::MigrationDirectoryDoesNotExist`] before `init`), checking the history,
    /// generating or applying.
    pub async fn run(
        &self,
        cli: &mut Migrator,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<()> {
        let file_manager = cli.file_manager();
        let comparison = cli.comparison().await?;
        cli.setup_db().await?;
        let history = cli.db()?;
        let name = &self.name;
        match file_manager.detect_migration_type()? {
            MigrationFlag::TwoWay => {
                file_manager
                    .two_way()
                    .generate_migrations(&comparison, history, name, codebase_resources, prompter)
                    .await?;
            }
            MigrationFlag::OneWay => {
                file_manager
                    .one_way()
                    .generate_migrations(&comparison, history, name, codebase_resources, prompter)
                    .await?;
            }
        }

        if self.run {
            log::info!("Running generated migrations");
            self.up().run(cli).await?;
        }

        log::info!("Migration generation done.");
        Ok(())
    }

    fn up(&self) -> Up {
        Up::new(UpdateStrategy::Latest)
    }
}
