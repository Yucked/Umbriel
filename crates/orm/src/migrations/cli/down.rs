use clap::Args;

use crate::migrations::{database::db_runner::delete_file, *};

/// Rollback migrations
#[derive(Args, Debug, Clone)]
pub struct Down {
    #[command(flatten)]
    strategy: RollbackStrategyStruct,

    /// Delete the rolled-back migration files afterwards
    #[arg(global = true, long, default_value_t = false)]
    pub(crate) prune: bool,

    /// With `--prune`, delete without asking for confirmation
    #[arg(long)]
    pub(crate) yes: bool,
}

impl Down {
    /// A rollback as `strategy` says, deleting the rolled-back files afterwards when `prune`.
    pub fn new(strategy: RollbackStrategy, prune: bool) -> Self {
        let mut arguments = RollbackStrategyStruct {
            previous: false,
            number: None,
            till: None,
        };
        match strategy {
            RollbackStrategy::Previous => arguments.previous = true,
            RollbackStrategy::Number(number) => arguments.number = Some(number),
            RollbackStrategy::Till(till) => arguments.till = Some(till),
        }
        Self {
            strategy: arguments,
            prune,
            yes: false,
        }
    }

    /// Prunes without asking for confirmation.
    pub fn yes(mut self) -> Self {
        self.yes = true;
        self
    }

    /// How far to roll back.
    pub fn rollback_strategy(&self) -> RollbackStrategy {
        RollbackStrategy::from(&self.strategy)
    }
}

/// The rollback arguments; at most one is given, and none means the previous migration.
#[derive(Args, Debug, Clone)]
#[group(required = false, multiple = false)]
pub struct RollbackStrategyStruct {
    /// Roll back the last applied migration
    #[arg(long)]
    pub(crate) previous: bool,

    /// Roll back this many applied migrations
    #[arg(short, long)]
    pub(crate) number: Option<u32>,

    /// Roll back till a specific migration, named by its timestamp and name
    #[arg(short, long)]
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
    /// Rolls back two-way migrations, each in its own transaction (see [`MigrationRunner`]).
    /// When one fails, those rolled back before it stay rolled back.
    ///
    /// With `--prune`, the files of the migrations to roll back are listed and confirmed with
    /// `prompter` (unless `--yes`) before anything is rolled back. The rollback then runs under
    /// the history's lock, and each migration's files are deleted once its reversal commits; a
    /// reversal that fails keeps its files. No other file is touched.
    ///
    /// # Errors
    ///
    /// [`MigrationError::OneWayRollback`] for one-way migrations, [`MigrationError::Declined`],
    /// and the failure of connecting, detecting the migration type, rolling back
    /// (see [`MigrationRunner::rollback_migrations`]) or deleting a file.
    pub async fn run(&self, cli: &mut Migrator, prompter: impl Prompter) -> MigrationResult<()> {
        cli.setup_db().await?;
        let file_manager = cli.file_manager();
        if file_manager.detect_migration_type()? == MigrationFlag::OneWay {
            return Err(MigrationError::OneWayRollback);
        }
        let db = cli.db()?;
        let options = RollbackOptions {
            rollback_strategy: self.rollback_strategy(),
            mode: cli.mode,
        };
        let plan = &MigrationRunner::plan_rollback(db, &file_manager, options).await?;
        let files = plan.files();
        if !self.prune || files.is_empty() {
            let revision = plan.reading().revision;
            MigrationRunner::execute_rollback(db, plan, None, revision, |_| Ok(())).await?;
            log::info!("Rollback successful");
            return Ok(());
        }

        let listed = files.iter().map(ToString::to_string).collect::<Vec<_>>();
        confirmed(
            &prompter,
            self.yes,
            "Roll back these migrations and delete their files",
            &listed,
        )?;
        let dir = &file_manager.get_migration_dir()?;
        MigrationRunner::with_lock(db, plan.reading(), |holder| async move {
            // Taking the lock wrote the revision after the plan's.
            let revision = plan.reading().revision + 1;
            MigrationRunner::execute_rollback(db, plan, Some(&holder), revision, |name| {
                for file in [name.clone(), name.to_down()] {
                    delete_file(&dir.join(file.to_string()))?;
                }
                Ok(())
            })
            .await
        })
        .await?;
        log::info!("Rollback successful");
        Ok(())
    }
}
