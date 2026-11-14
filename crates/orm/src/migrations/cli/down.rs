use clap::Args;
use typed_builder::TypedBuilder;

use crate::migrations::*;

/// Rollback migrations
#[derive(Args, Debug, TypedBuilder, Clone)]
pub struct Down {
    #[command(flatten)]
    strategy: RollbackStrategyStruct,

    #[arg(
        global = true,
        long,
        help = "If to prune migration files after rollback",
        default_value_t = false
    )]
    pub(crate) prune: bool,
}

impl Down {
    /// How far to roll back.
    pub fn rollback_strategy(&self) -> RollbackStrategy {
        RollbackStrategy::from(&self.strategy)
    }
}

/// The rollback arguments; at most one is given, and none means the previous migration.
#[derive(Args, Debug, Clone, TypedBuilder)]
#[group(required = false, multiple = false)]
pub struct RollbackStrategyStruct {
    /// Rollback to the previous migration
    #[arg(long, help = "Rollback to the previous migration")]
    #[builder(default)]
    pub(crate) previous: bool,

    /// Rollback by count/number
    #[arg(short, long, help = "Rollback by the number specified")]
    #[builder(default, setter(strip_option))]
    pub(crate) number: Option<u32>,

    /// Rollback till a specific migration ID
    #[arg(
        short,
        long,
        value_parser = mig_name_parser,
        help = "Rollback till a specific migration ID"
    )]
    #[builder(default, setter(strip_option))]
    pub(crate) till: Option<MigrationFilename>,
}

impl Default for RollbackStrategyStruct {
    fn default() -> Self {
        RollbackStrategyStruct {
            previous: true,
            number: None,
            till: None,
        }
    }
}

/// How far to roll back.
pub enum RollbackStrategy {
    /// The latest applied migration (`down`, `down --previous`); the default.
    Previous,
    /// This many applied migrations (`down --number 3`).
    Number(u32),
    /// Every migration applied after this one (`down --till <file name>`).
    Till(MigrationFilename),
}

impl From<&RollbackStrategyStruct> for RollbackStrategy {
    fn from(strategy: &RollbackStrategyStruct) -> Self {
        if strategy.previous {
            RollbackStrategy::Previous
        } else if let Some(by_count) = strategy.number {
            RollbackStrategy::Number(by_count)
        } else if let Some(by_name) = strategy.till.clone() {
            RollbackStrategy::Till(by_name)
        } else {
            RollbackStrategy::Previous
        }
    }
}

impl Down {
    /// Rolls back two-way migrations, each in its own transaction, then prunes their files if
    /// asked. When one fails, those rolled back before it stay rolled back (see
    /// [`MigrationRunner`]).
    ///
    /// # Panics
    ///
    /// If the migrations are one-way or the rollback fails.
    pub async fn run(&self, cli: &mut Migrator) {
        cli.setup_db().await;
        let file_manager = cli.file_manager();
        let db = cli.db();

        if let Ok(MigrationFlag::OneWay) = file_manager.detect_migration_type() {
            let err = "Cannot rollback one way migrations. \
            Create a new migration to reverse the changes or run cargo run -- reset -r \
            to use two way migrations";

            log::error!("{err}");
            panic!("{err}");
        }

        let rollback_strategy = self.rollback_strategy();

        let rollback = file_manager
            .two_way()
            .run_down_migrations(
                db,
                RollbackOptions {
                    rollback_strategy,
                    mode: cli.mode,
                },
            )
            .await;

        if let Err(ref e) = rollback {
            log::error!("Rollback Failed: {e}");
            panic!("Rollback Failed: {e}");
        } else {
            log::info!("Rollback successful");

            if self.prune {
                log::info!("Pruning all pending migration files");
                self.prune().run(cli).await;
                log::info!("Pruning successful");
            }

            log::info!("Rollback Ddne");
        }
    }

    fn prune(&self) -> Prune {
        Prune
    }
}
