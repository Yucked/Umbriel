/// The database connection arguments.
pub mod config;
mod down;
mod generate;
mod init;
mod list;
mod prune;
mod reset;
mod unlock;
mod up;

use std::path::PathBuf;

pub use down::{Down, RollbackStrategyStruct};
pub use generate::Generate;
pub use init::Init;
pub use list::List;
pub use prune::Prune;
pub use reset::Reset;
pub use unlock::Unlock;

use surrealdb::{Surreal, engine::any::Any};
pub use up::{FastForwardDelta, Up};

use crate::DbResources;
use clap::{ArgAction, Parser};

pub use self::config::{
    COMPARISON_PASSWORD_VARIABLE, DatabaseConnection, Experimental, PASSWORD_VARIABLE,
    USER_VARIABLE,
};
use crate::migrations::{
    Comparison, MigrationConfig, MigrationResult, Mode, Prompter, RealPrompter,
};

/// Umbriel CLI
#[derive(Parser, Debug, Clone)]
#[clap(name = "umbriel", about = "Umbriel CLI")]
#[command(version)]
pub struct Migrator {
    /// Subcommand: generate, up, down, list
    #[command(subcommand)]
    subcmd: Option<SubCommand>,

    /// Optional custom migrations directory
    #[arg(global = true, short, long)]
    pub dir: Option<PathBuf>,

    /// Logs more with each `-v`: errors only by default, then warnings (-v), information (-vv),
    /// debugging (-vvv) and everything (-vvvv)
    #[arg(global = true, short, long, action = ArgAction::Count)]
    pub(crate) verbose: u8,

    /// Strict checks the migration files against the history (checksums and file names); lax
    /// doesn't
    #[arg(value_enum, global = true, long, default_value_t = Mode::Strict)]
    pub(crate) mode: Mode,

    #[command(flatten)]
    pub(crate) db_connection: DatabaseConnection,
}

impl Migrator {
    /// A command line over a connection made in code, for running commands without parsing
    /// arguments: no subcommand yet ([`set_cmd`](Self::set_cmd) sets one), migrations in `dir`,
    /// checked in `mode`, and no logging.
    pub fn connected(connection: DatabaseConnection, dir: PathBuf, mode: Mode) -> Self {
        Self {
            subcmd: None,
            dir: Some(dir),
            verbose: 0,
            mode,
            db_connection: connection,
        }
    }

    /// How closely migration files are checked against the history.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// How many `-v` were given: 0 logs errors only.
    pub fn verbosity(&self) -> u8 {
        self.verbose
    }

    /// The connected database.
    ///
    /// # Errors
    ///
    /// [`MigrationError::NotConnected`](crate::migrations::MigrationError::NotConnected) before
    /// [`setup_db`](Self::setup_db).
    pub fn db(&self) -> MigrationResult<&Surreal<Any>> {
        self.db_connection.db()
    }

    /// Sets the subcommand to run.
    pub fn set_cmd(&mut self, cmd: SubCommand) -> &mut Self {
        self.subcmd = Some(cmd);
        self
    }

    /// The migration configuration from the directory and mode arguments.
    pub fn file_manager(&self) -> MigrationConfig {
        let config = MigrationConfig::new().set_mode(self.mode);
        match &self.dir {
            Some(dir) => config.set_custom_path(dir.clone()),
            None => config,
        }
    }

    /// Run migration cli
    /// # Example
    /// ```rust,no_run
    /// use umbriel::{DbResources, migrations::Migrator};
    ///
    /// /// The codebase's schema: its tables, functions and other definitions.
    /// struct Resources;
    ///
    /// impl DbResources for Resources {}
    ///
    /// // A migrator binary runs this in its `main`, on the runtime it uses, and decides how a
    /// // failure ends the process.
    /// async fn migrate() -> umbriel::migrations::MigrationResult<()> {
    ///     Migrator::run(Resources).await
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// The subcommand's failure, as [`run_fn`](Self::run_fn).
    pub async fn run(codebase_resources: impl DbResources) -> MigrationResult<()> {
        let mut cli = Self::parse();
        cli.setup_logging();
        cli.run_fn(codebase_resources, RealPrompter).await
    }

    /// Runs the configured subcommand, `up` when none is set, answering prompts with `prompter`.
    ///
    /// # Errors
    ///
    /// The subcommand's failure: connecting, reading or writing the migration directory,
    /// generating or applying migrations.
    pub async fn run_fn(
        &mut self,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<()> {
        match self.subcmd.clone() {
            None => Up::default().run(self).await,
            Some(SubCommand::Init(init)) => init.run(self, codebase_resources, prompter).await,
            Some(SubCommand::Generate(generate)) => {
                generate.run(self, codebase_resources, prompter).await
            }
            Some(SubCommand::Up(up)) => up.run(self).await,
            Some(SubCommand::Down(down)) => down.run(self, prompter).await,
            Some(SubCommand::Prune(prune)) => prune.run(self, prompter).await,
            Some(SubCommand::List(list)) => list.run(self).await,
            Some(SubCommand::Reset(reset)) => reset.run(self, codebase_resources, prompter).await,
            Some(SubCommand::Unlock(unlock)) => unlock.run(self, prompter).await,
        }
    }

    pub(crate) fn setup_logging(&self) {
        let verbosity = self.verbose;
        let log_level = match verbosity {
            0 => "error",
            1 => "warn",
            2 => "info",
            3 => "debug",
            _ => "trace",
        };

        // The level is passed to the logger directly: setting `RUST_LOG` mutates the process
        // environment, which is `unsafe` since edition 2024. A logger already installed, by the
        // application or an earlier run, is kept.
        let _ = pretty_env_logger::formatted_builder()
            .parse_filters(log_level)
            .try_init();
    }

    /// Connects to the database unless already connected.
    ///
    /// # Errors
    ///
    /// As [`DatabaseConnection::setup`].
    pub async fn setup_db(&mut self) -> MigrationResult<()> {
        if self.db_connection.db_connection.is_none() {
            self.db_connection.setup().await?;
        }
        Ok(())
    }

    /// The comparison `init`, `generate` and `reset` generate migrations in (see
    /// [`DatabaseConnection::comparison`]).
    ///
    /// # Errors
    ///
    /// As [`DatabaseConnection::comparison`].
    pub async fn comparison(&mut self) -> MigrationResult<Comparison<Any>> {
        self.db_connection.comparison().await
    }
}

/// Subcommands
#[derive(Parser, Debug, Clone)]
pub enum SubCommand {
    /// Init migrations
    Init(Init),
    /// Generate migrations
    #[clap(alias = "gen")]
    Generate(Generate),
    /// Run migrations forward
    Up(Up),
    /// Rollback migrations
    Down(Down),
    /// Reset migrations. Deletes all migration files, migration table and reinitializes
    /// migrations
    Reset(Reset),
    /// List migrations
    #[clap(alias = "ls")]
    List(List),
    /// Delete Unapplied local migration files that have not been applied to the current database instance
    Prune(Prune),
    /// Release the migration history's lock, which an interrupted prune leaves behind
    Unlock(Unlock),
}

macro_rules! impl_from {
    ($cmd:ident) => {
        impl From<$cmd> for SubCommand {
            fn from(from: $cmd) -> Self {
                SubCommand::$cmd(from)
            }
        }
    };
}
impl_from!(Init);
impl_from!(Generate);
impl_from!(Up);
impl_from!(Down);
impl_from!(Reset);
impl_from!(List);
impl_from!(Prune);
impl_from!(Unlock);
