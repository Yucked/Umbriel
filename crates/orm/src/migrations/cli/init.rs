use super::up::Up;
use crate::DbResources;
use crate::migrations::*;
use clap::Args;

/// Init migrations
#[derive(Args, Debug, Clone)]
pub struct Init {
    /// Name of the first migration file(s)
    #[arg(long)]
    pub(crate) name: Basename,

    /// Whether to run the migrations after initialization
    #[arg(long)]
    pub(crate) run: bool,

    /// Write two-way migrations (an up and a down file each) instead of one-way ones
    #[arg(short, long)]
    pub(crate) reversible: bool,
}

impl Init {
    /// Initializes migrations with a first migration named `name`, two-way when `reversible`,
    /// applying it afterwards when `run`.
    pub fn new(name: Basename, reversible: bool, run: bool) -> Self {
        Self {
            name,
            run,
            reversible,
        }
    }

    /// Whether the migrations are two-way.
    pub fn reversible(&self) -> bool {
        self.reversible
    }

    /// Generates the first migration from the codebase's resources, and applies it if asked.
    ///
    /// # Errors
    ///
    /// [`MigrationError::AlreadyInitialized`] when the directory holds migrations, or the
    /// failure of reading the directory, generating or applying.
    pub async fn run(
        &self,
        cli: &mut Migrator,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<()> {
        let file_manager = cli.file_manager();
        if !file_manager.get_migrations_filenames(true)?.is_empty() {
            return Err(MigrationError::AlreadyInitialized);
        }

        let comparison = cli.comparison().await?;
        cli.setup_db().await?;
        let history = cli.db()?;
        let name = &self.name;
        if self.reversible {
            file_manager
                .two_way()
                .generate_migrations(&comparison, history, name, codebase_resources, prompter)
                .await?;
        } else {
            file_manager
                .one_way()
                .generate_migrations(&comparison, history, name, codebase_resources, prompter)
                .await?;
        }

        if self.run {
            log::info!("Running initial migrations");
            self.up().run(cli).await?;
        }

        log::info!("Successfully initialized and generated first migration(s)");
        Ok(())
    }

    /// The `up` that applies every pending migration.
    pub fn up(&self) -> Up {
        Up::new(UpdateStrategy::Latest)
    }
}
