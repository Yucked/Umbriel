mod arg_parser;
/// The database connection arguments.
pub mod config;
mod down;
mod generate;
mod init;
mod list;
mod prune;
mod reset;
mod up;

use std::path::PathBuf;

pub use arg_parser::*;
pub use down::{Down, RollbackStrategy, RollbackStrategyStruct};
pub use generate::Generate;
pub use init::Init;
pub use list::{List, Status};
pub use prune::Prune;
pub use reset::Reset;

use surrealdb::{Surreal, engine::any::Any};
use typed_builder::TypedBuilder;
pub use up::{FastForwardDelta, Up, UpdateStrategy};

use crate::DbResources;
use clap::{ArgAction, Parser};

pub use self::config::DatabaseConnection;
use crate::migrations::{
    MigrationConfig, MockPrompter, Mode, Prompter, RealPrompter, RenameOrDelete,
};

/// Umbriel CLI
#[derive(Parser, Debug, Clone, TypedBuilder)]
#[clap(name = "SurrealOrm", about = "Umbriel CLI")]
#[command(version)]
pub struct Migrator {
    /// Subcommand: generate, up, down, list
    #[command(subcommand)]
    #[builder(default, setter(strip_option))]
    subcmd: Option<SubCommand>,

    /// Optional custom migrations dir
    #[arg(
        global = true,
        short,
        long,
        help = "Optional custom migrations directory"
    )]
    #[builder(default, setter(strip_option))]
    pub dir: Option<PathBuf>,

    /// Sets the level of verbosity e.g -v, -vv, -vvv, -vvvv
    #[arg(global = true, short, long, action = ArgAction::Count, default_value_t=3)]
    pub(crate) verbose: u8,

    #[arg(
        value_enum,
        global = true,
        long,
        help = "If to be strict or lax. Strictness validates the migration files against the database e.g doing checksum checks to make sure.\
            that file contents and valid and also checking filenames. Lax does not.",
        default_value_t = Mode::Strict,
    )]
    pub(crate) mode: Mode,

    #[command(flatten)]
    pub(crate) db_connection: DatabaseConnection,
}

impl Migrator {
    /// How closely migration files are checked against the history.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The connected database.
    ///
    /// # Panics
    ///
    /// If the connection has not been set up.
    pub fn db(&self) -> &Surreal<Any> {
        self.db_connection.db().expect("Failed to get db")
    }

    /// Sets the subcommand to run.
    pub fn set_cmd(&mut self, cmd: SubCommand) -> &mut Self {
        self.subcmd = Some(cmd);
        self
    }

    /// The migration configuration from the directory and mode arguments.
    pub fn file_manager(&self) -> MigrationConfig {
        let fm_init = MigrationConfig::builder()
            .custom_path(self.dir.clone())
            .mode(self.mode);

        fm_init.build()
    }

    /// Run migration cli
    /// # Example
    /// ```rust, ignore
    /// use umbriel_tests::migrations::Resources;
    /// use umbriel::migrations::Migrator;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     Migrator::run(Resources).await;
    /// }
    /// ```
    pub async fn run(codebase_resources: impl DbResources) {
        let mut cli = Self::parse();
        cli.setup_logging();
        cli.run_fn(codebase_resources, RealPrompter).await;
    }

    /// Runs the command line from the process arguments, answering prompts with a
    /// [`MockPrompter`] that allows empty migrations and renames replaced fields.
    pub async fn run_test_main(codebase_resources: impl DbResources) {
        let mut cli = Self::parse();
        cli.setup_logging();
        cli.run_fn(
            codebase_resources,
            MockPrompter::builder()
                .allow_empty_migrations_gen(true)
                .rename_or_delete_single_field_change(RenameOrDelete::Rename)
                .build(),
        )
        .await;
    }

    /// Runs the configured subcommand, `up` when none is set, answering prompts with `prompter`.
    ///
    /// # Panics
    ///
    /// If `init`, `generate` or `reset` runs without `codebase_resources`, or the subcommand
    /// fails.
    pub async fn run_test(
        &mut self,
        codebase_resources: Option<impl DbResources>,
        prompter: MockPrompter,
    ) {
        match self.subcmd.clone() {
            None => {
                Up::default().run(self).await;
            }
            Some(subcmd) => match subcmd {
                SubCommand::Init(init) => {
                    init.run(
                        self,
                        codebase_resources.expect("resources must be provided for init command"),
                        prompter,
                    )
                    .await
                }
                SubCommand::Generate(generate) => {
                    generate
                        .run(
                            self,
                            codebase_resources
                                .expect("resources must be provided for generate command"),
                            prompter,
                        )
                        .await
                }
                SubCommand::Up(up) => up.run(self).await,
                SubCommand::Down(down) => down.run(self).await,
                SubCommand::Prune(prune) => prune.run(self).await,
                SubCommand::List(prune) => prune.run(self).await,
                SubCommand::Reset(reset) => {
                    reset
                        .run(
                            self,
                            codebase_resources
                                .expect("resources must be provided for reset command"),
                            prompter,
                        )
                        .await
                }
            },
        };
    }

    /// Runs the configured subcommand, `up` when none is set.
    ///
    /// # Panics
    ///
    /// If the subcommand fails.
    pub async fn run_fn(&mut self, codebase_resources: impl DbResources, prompter: impl Prompter) {
        match self.subcmd.clone() {
            None => {
                Up::default().run(self).await;
            }
            Some(subcmd) => match subcmd {
                SubCommand::Init(init) => init.run(self, codebase_resources, prompter).await,
                SubCommand::Generate(generate) => {
                    generate.run(self, codebase_resources, prompter).await
                }
                SubCommand::Up(up) => up.run(self).await,
                SubCommand::Down(down) => down.run(self).await,
                SubCommand::Prune(prune) => prune.run(self).await,
                SubCommand::List(prune) => prune.run(self).await,
                SubCommand::Reset(reset) => reset.run(self, codebase_resources, prompter).await,
            },
        };
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
    pub async fn setup_db(&mut self) {
        if self.db_connection.db_connection.is_none() {
            self.db_connection.setup().await;
        }
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
