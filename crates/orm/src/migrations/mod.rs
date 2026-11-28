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
//! for two-way migrations, of the down file. The runner also keeps a revision record in the
//! `migration_revision` table, which serializes concurrent runners and holds a prune's lock.
//!
//! A migration is pending when the history doesn't record its file name. Migrations apply in file
//! name order, and only in it: a pending file that sorts before the newest applied one, as one
//! merged from another branch may, is [`MigrationError::OutOfOrder`] in both modes, and applies
//! once renamed with a newer timestamp. The history's file order is therefore the order it was
//! applied in, which rollback and generation rely on.
//!
//! In [`Mode::Strict`], the default, `up` and `generate` first check that every applied
//! migration's file exists and still has its checksums, and a rollback (`down`) checks the files
//! of every migration it rolls back before changing anything; listing the applied migrations
//! fails when the history records migrations the directory lacks. [`Mode::Lax`] logs these
//! instead, and a lax rollback stops at the first migration without a down file. Each migration
//! commits in one transaction with its record, so the history agrees with the database's
//! committed changes; see [`MigrationRunner`] for failures, interrupted runs and concurrent
//! runners. A migration that changes other history records, as a reset's first migration clears
//! the history, applies only through `reset --run` or `up --replace-history` (see `cli::Reset`).
//! Connecting never applies anything: only `up`, `down`, `reset --run`, `init --run` and
//! `generate --run` (or their library forms) write schema.
//!
//! # Renames and removals
//!
//! A field renamed with `#[orm(old_name = "…")]`, or a single field replaced by another that the
//! [`Prompter`] calls a rename, is written as three statements: the new field's `DEFINE FIELD`,
//! an `UPDATE` of every record of the table copying the old field's values into the new one, and
//! the old field's `REMOVE FIELD`. The `UPDATE` checks each record against the table's field
//! definitions, so a table's renames are written before its other field changes: a new required
//! field defined first would fail it. A rename keeps its values in both directions. Any other
//! removed table or field loses its data, which a rollback does not bring back: the migration
//! lists them in its header, and generation asks the [`Prompter`] to confirm them first.
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
//! `Migrator::run`, behind the `cli` feature, is the command line, run from a binary of the
//! consuming crate that names its resources. Its subcommands are `init`, `generate`, `up` (the default), `down`, `list`, `prune`,
//! `reset` and `unlock`. Every subcommand takes the connection (`--url`, `--ns`, `--db`, `--user`,
//! `--pass`), `--dir` and `--mode`; `init` and `generate` check the directory against that
//! database's history. `prune`, `down --prune`, `reset` and `unlock` list what they delete,
//! replace or release and ask before going ahead, unless given `--yes`. `init`, `generate` and
//! `reset` also need
//! `--comparison-ns`, `--comparison-user` and the password in `UMBRIEL_COMPARISON_PASS`, for the
//! comparison databases they create. An embedded engine allows experimental features only when
//! asked, with `--allow-experimental files,surrealism`, which bucket and module migrations need.
//!
//! # Compatibility with earlier migrations
//!
//! Umbriel reads the directories and history `surreal_orm` wrote: the file names, timestamps,
//! checksums and the `migration` table's fields are unchanged, so applied migrations stay applied
//! and a strict rollback still verifies them. Three differences remain:
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
//!   from the codebase (see `cli::Reset`), or rewrite the old files in SurrealDB 3 syntax. A
//!   rewritten file is not run again where it is already applied, but it no longer matches its
//!   checksum, so strict `up`, `generate` and rollback refuse it there.
//! - `surreal_orm` chose pending migrations by timestamp, so an older file merged later was
//!   never applied. Here it is pending, and refused as [`MigrationError::OutOfOrder`] until it
//!   is renamed.
//!
//! Upgrading this crate never changes stored data or applied files by itself.

/// The migration command line, run from a binary that names the codebase's resources. Behind
/// the `cli` feature, which adds `clap`, `inquire` and `pretty_env_logger`; the rest of this
/// module needs only `migrations`.
#[cfg(feature = "cli")]
pub mod cli;
pub(crate) mod database;
mod error;
pub(crate) mod files;
pub(crate) mod resources;
pub(crate) mod schema;

#[cfg(feature = "cli")]
pub use cli::*;
pub use database::*;
pub use error::*;
pub use files::*;
pub use resources::*;
pub use schema::{Definition, ResourceKey, ResourceKind, Schema};

pub use umbriel_macros::embed_migrations;
