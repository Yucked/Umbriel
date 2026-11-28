use crate::migrations::{database::replacement, *};

use clap::Args;

/// Applies pending migrations: all of them (`up`, `up --latest`), a number of them
/// (`up --number 2`), or those up to and including one (`up --till
/// 20240101120000_create_users`), named by its timestamp and name.
#[derive(Args, Debug, Clone, Default)]
pub struct Up {
    #[command(flatten)]
    pub(crate) fast_forward: FastForwardDelta,

    /// First apply a reset's migration, which replaces the history the reset read
    #[arg(long)]
    pub(crate) replace_history: bool,
}

impl Up {
    /// Applies pending migrations as `strategy` says.
    pub fn new(strategy: UpdateStrategy) -> Self {
        let mut fast_forward = FastForwardDelta {
            latest: false,
            number: None,
            till: None,
        };
        match strategy {
            UpdateStrategy::Latest => fast_forward.latest = true,
            UpdateStrategy::Number(number) => fast_forward.number = Some(number),
            UpdateStrategy::Till(till) => fast_forward.till = Some(till),
        }
        Self {
            fast_forward,
            replace_history: false,
        }
    }

    /// How far to apply pending migrations.
    pub fn update_strategy(&self) -> UpdateStrategy {
        UpdateStrategy::from(&self.fast_forward)
    }
}

/// The arguments for applying pending migrations; at most one is given, and none means all.
#[derive(Args, Debug, Clone)]
#[group(required = false, multiple = false)]
pub struct FastForwardDelta {
    /// Apply every pending migration
    #[arg(long)]
    pub(crate) latest: bool,

    /// Apply this many pending migrations
    #[arg(short, long)]
    pub(crate) number: Option<u32>,

    /// Apply pending migrations up to and including this one
    #[arg(short, long)]
    pub(crate) till: Option<MigrationFilename>,
}

impl Default for FastForwardDelta {
    fn default() -> Self {
        Self {
            latest: true,
            number: None,
            till: None,
        }
    }
}

impl From<&FastForwardDelta> for UpdateStrategy {
    fn from(fast_forward: &FastForwardDelta) -> Self {
        if fast_forward.latest {
            UpdateStrategy::Latest
        } else if let Some(by_count) = fast_forward.number {
            UpdateStrategy::Number(by_count)
        } else if let Some(till) = fast_forward.till.clone() {
            UpdateStrategy::Till(till)
        } else {
            UpdateStrategy::Latest
        }
    }
}

impl Up {
    /// First applies a reset's migration, which replaces the history the reset read (see
    /// [`Reset`]), then the pending migrations.
    pub fn replace_history(mut self) -> Self {
        self.replace_history = true;
        self
    }

    /// Applies pending migrations, each in its own transaction: when one fails, those before it
    /// stay applied (see [`MigrationRunner`]). With `--replace-history`, a reset's migration is
    /// applied first, only onto the history the reset read, and the reset's backup is removed.
    ///
    /// # Errors
    ///
    /// The failure of connecting, detecting the migration type (such as
    /// [`MigrationError::MigrationDirectoryDoesNotExist`] before `init`), replacing the history
    /// ([`MigrationError::NoPendingReset`], [`MigrationError::ResetIncomplete`]), or applying
    /// (such as [`MigrationError::MigrationNotFoundFromPendingMigrations`] for a `--till`
    /// target that is not pending).
    pub async fn run(&self, cli: &mut Migrator) -> MigrationResult<()> {
        cli.setup_db().await?;
        let file_manager = cli.file_manager();
        let update_strategy = self.update_strategy();
        let db = cli.db()?;
        if self.replace_history {
            replacement::replace_history(db, &file_manager).await?;
        }

        match file_manager.detect_migration_type()? {
            MigrationFlag::TwoWay => {
                log::info!("Running two way migrations");
                file_manager
                    .two_way()
                    .run_up_pending_migrations(db, update_strategy)
                    .await?;
            }
            MigrationFlag::OneWay => {
                log::info!("Running one way migrations");
                file_manager
                    .one_way()
                    .run_pending_migrations(db, update_strategy)
                    .await?;
            }
        }

        log::info!("Successfully ran migrations");
        Ok(())
    }
}
