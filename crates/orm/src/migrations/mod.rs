//! Migrations generated from the codebase's schema, applied from a directory, from files embedded
//! in the binary, or from the command line.
//!
//! # Files
//!
//! A migration directory (`migrations` next to `Cargo.toml` unless configured) holds either
//! one-way migrations, `<timestamp>_<name>.surql`, or two-way ones, a `<timestamp>_<name>.up.surql`
//! and `.down.surql` pair; a directory holds one kind. The timestamp is UTC to the millisecond
//! (`%Y%m%d%H%M%S%3f`) and orders the migrations. Generation compares the codebase's
//! [`DbResources`](crate::DbResources) with the schema the directory's migrations produce, in a
//! [`Comparison`] database on a server, and writes the difference.
//!
//! # History
//!
//! Applied migrations are recorded in the `migration` table, one [`Migration`] record per file,
//! keyed by its file name: the timestamp, and the SHA-256 checksum of the up (or only) file and,
//! for two-way migrations, of the down file. A migration is pending when its timestamp is newer
//! than the latest record. Applying (`up`) checks no checksum: it runs only the pending files. In
//! [`Mode::Strict`], the default, a rollback (`down`) verifies the checksums and names of the
//! migrations it rolls back against their records, and listing the applied migrations fails when
//! the history records migrations the directory lacks; [`Mode::Lax`] skips both checks. Each migration commits in one
//! transaction with its record, so the history agrees with the database's committed changes; see
//! [`MigrationRunner`] for failures, interrupted runs and concurrent runners. Connecting never
//! applies anything: only `up`, `down`, `reset --run`, `init --run` and `generate --run` (or their
//! library forms) write schema.
//!
//! # Library
//!
//! [`MigrationConfig`] selects the directory and [`Mode`]; [`one_way`](MigrationConfig::one_way)
//! and [`two_way`](MigrationConfig::two_way) give the operations for each kind of directory:
//!
//! ```rust,no_run
//! use umbriel::{Surreal, engine::any::Any};
//! use umbriel::migrations::{MigrationConfig, MigrationResult, UpdateStrategy};
//!
//! async fn migrate(db: &Surreal<Any>) -> MigrationResult<()> {
//!     MigrationConfig::new()
//!         .two_way()
//!         .run_up_pending_migrations(db, UpdateStrategy::Latest)
//!         .await
//! }
//! ```
//!
//! [`embed_migrations!`] compiles a directory into the binary, as an [`EmbeddedMigrationsOneWay`]
//! or [`EmbeddedMigrationsTwoWay`] constant whose `run` applies the pending ones, so a deployed
//! binary needs no migration files on disk. Rolling back is [`FileManagerBi::run_down_migrations`]
//! with a [`RollbackStrategy`]; it needs the down files, so it works on two-way directories only.
//!
//! # Command line
//!
//! [`Migrator::run`] is the command line, run from a binary of the consuming crate that names its
//! resources. Its subcommands are `init`, `generate`, `up` (the default), `down`, `list`, `prune`
//! and `reset`. Every subcommand takes the connection (`--url`, `--ns`, `--db`, `--user`,
//! `--pass`), `--dir` and `--mode`. `init`, `generate` and `reset` also need
//! `--comparison-ns`, `--comparison-user` and the password in `UMBRIEL_COMPARISON_PASS`, for the
//! comparison databases they create. An embedded engine allows experimental features only when
//! asked, with `--allow-experimental files,surrealism`, which bucket and module migrations need.
//!
//! # Compatibility with earlier migrations
//!
//! Umbriel reads the directories and history `surreal_orm` wrote: the file names, timestamps,
//! checksums and the `migration` table's fields are unchanged, so applied migrations stay applied
//! and a strict rollback still verifies them. Two differences remain:
//!
//! - The history table's `id` field is defined as `string`, the record key's type in SurrealDB 3,
//!   where `surreal_orm` defined it as `record<migration>`. The history table is compared like
//!   any other, so the first migration generated over an older directory redefines the field.
//! - A migration file is applied as written. Files generated for SurrealDB 1.x can use
//!   statements SurrealDB 3 no longer parses or that behave differently, such as `DEFINE SCOPE`
//!   and `DEFINE TOKEN` (replaced by `DEFINE ACCESS` in 2.0) or a plain `DEFINE` of an existing
//!   resource, which fails since 2.0. Lax mode does not avoid them: every file in the directory is
//!   still run, by `generate`, which replays the whole directory in a comparison database to
//!   learn its schema, and by `up` on any database whose history lacks them, such as a new
//!   deployment. `init` and `reset` start from an empty schema and replay nothing. Upgrade the database itself as SurrealDB's upgrade guide
//!   describes, then start a new history with `reset`, which regenerates a single first migration
//!   from the codebase (see [`Reset`]), or rewrite the old files in SurrealDB 3 syntax. A
//!   rewritten file is not run again where it is already applied, but a strict rollback of it
//!   fails its checksum.
//!
//! Upgrading this crate never changes stored data or applied files by itself.

/// The migration command line, run from a binary that names the codebase's resources.
pub mod cli;
pub(crate) mod database;
mod error;
pub(crate) mod files;
pub(crate) mod resources;
pub(crate) mod schema;

pub use cli::*;
pub use database::*;
pub use error::*;
pub use files::*;
pub use resources::*;
pub use schema::{Definition, ResourceKey, ResourceKind, Schema};

pub use umbriel_macros::embed_migrations;
