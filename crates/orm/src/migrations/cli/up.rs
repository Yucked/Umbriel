use crate::migrations::*;

use clap::Args;
use typed_builder::TypedBuilder;

/// Run migrations
/// cargo run -- up
/// cargo run -- up -l
/// cargo run -- up -n 2
/// cargo run -- up -t 2021-09-09-xxxxx
#[derive(Args, Debug, TypedBuilder, Clone, Default)]
pub struct Up {
    #[command(flatten)]
    pub(crate) fast_forward: FastForwardDelta,
}

impl Up {
    /// How far to apply pending migrations.
    pub fn update_strategy(&self) -> UpdateStrategy {
        UpdateStrategy::from(&self.fast_forward)
    }
}

/// The arguments for applying pending migrations; at most one is given, and none means all.
#[derive(Args, Debug, Clone, TypedBuilder)]
#[group(required = false, multiple = false)]
pub struct FastForwardDelta {
    /// Run forward to the latest migration
    #[arg(long, help = "Run forward to the next migration")]
    #[builder(default)]
    pub(crate) latest: bool,

    /// Run forward by count/number
    #[arg(short, long, help = "Run forward by the number specified")]
    #[builder(default, setter(strip_option))]
    pub(crate) number: Option<u32>,

    /// Run forward till a specific migration ID
    #[arg(
        short,
        long,
        value_parser = mig_name_parser,
        help = "Run forward till a specific migration ID"
    )]
    #[builder(default, setter(strip_option))]
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

/// How far to apply pending migrations.
pub enum UpdateStrategy {
    /// Every pending migration (`up`, `up --latest`); the default.
    Latest,
    /// This many pending migrations (`up --number 3`).
    Number(u32),
    /// Pending migrations up to and including this one (`up --till <file name>`).
    Till(MigrationFilename),
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
    /// Applies pending migrations, each in its own transaction: when one fails, those before it
    /// stay applied (see [`MigrationRunner`]).
    ///
    /// # Panics
    ///
    /// If the migration type cannot be detected or applying fails.
    pub async fn run(&self, cli: &mut Migrator) {
        cli.setup_db().await;
        let file_manager = cli.file_manager();
        let update_strategy = self.update_strategy();
        let db = cli.db();

        match file_manager.detect_migration_type() {
            Ok(MigrationFlag::TwoWay) => {
                log::info!("Running two way migrations");
                let run = file_manager
                    .two_way()
                    .run_up_pending_migrations(db, update_strategy)
                    .await;
                if let Err(e) = run {
                    log::error!("Failed to run migrations: {e}");
                    panic!("Failed to run migrations. Migration already run or not found");
                }
            }
            Ok(MigrationFlag::OneWay) => {
                log::info!("Running one way migrations");
                let run = file_manager
                    .one_way()
                    .run_pending_migrations(db, update_strategy)
                    .await;
                if let Err(e) = run {
                    log::error!("Failed to run migrations: {e}");
                    panic!("Failed to run migrations. Migration already run or not found");
                }
            }
            Err(e) => {
                log::error!(
                    "Failed to detect migration type. Make sure the migration  \
                is first initialized or reset by running cargo run -- init -n '<migration name>'. Error: {e}"
                );
                panic!("Failed to detect migration type.");
            }
        };

        log::info!("Successfully ran migrations");
        match Schema::inspect(db).await {
            Ok(schema) => log::info!("Database: {schema:?}"),
            Err(e) => log::error!("Failed to inspect the database schema: {e}"),
        }
    }
}
