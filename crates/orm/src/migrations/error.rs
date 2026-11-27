use std::path::PathBuf;

use crate::{Field, Render, Table};
use thiserror::Error;

use crate::migrations::{MigrationFilename, ResourceKey};

/// Errors of generating, applying, listing and rolling back migrations.
#[derive(Error, Debug)]
pub enum MigrationError {
    /// A migration file's content no longer matches the checksum recorded when it was applied.
    #[error(
        "Checksum mismatch in {migration_name}. Expected checksum: {expected_checksum}. Actual checksum: {actual_checksum}"
    )]
    ChecksumMismatch {
        /// The migration's file name.
        migration_name: String,
        /// The checksum recorded in the history.
        expected_checksum: String,
        /// The checksum of the file now.
        actual_checksum: String,
    },

    /// A two-way migration's history record has no down checksum.
    #[error("Checksum not found in database for migration: {migration_name}")]
    NoChecksumInDb {
        /// The migration's file name.
        migration_name: String,
    },

    /// `init` found migrations already in the directory.
    #[error(
        "Migrations already initialized. Run 'cargo run -- reset' to reset migration. You can also \
        specify the '-r' or '--reversible' argument to set as reversible. Or delete the migrations \
        directory and run 'cargo run -- init' again."
    )]
    AlreadyInitialized,

    /// A command needs an argument, or an environment variable, that was not given.
    #[error("{argument} is required for this command")]
    MissingArgument {
        /// The argument or variable, e.g. `--comparison-ns`.
        argument: &'static str,
    },

    /// `down` was asked to roll back one-way migrations, which have no down files.
    #[error(
        "Cannot rollback one way migrations. Create a new migration to reverse the changes or run \
        cargo run -- reset -r to use two way migrations"
    )]
    OneWayRollback,

    /// The command line's database was used before it was connected.
    #[error("the database is not connected")]
    NotConnected,

    /// The history holds no migrations to roll back.
    #[error("No migrations have yet been registered in the database")]
    NoMigrationsRegisteredYetInDb,

    /// The history holds no migration with this file name.
    #[error("Migration - {filename} does not exist")]
    MigrationDoesNotExist {
        /// The requested migration.
        filename: MigrationFilename,
    },

    /// The requested migration is not pending: it was applied already or does not exist.
    #[error(
        "Migration with name: {0} not found in pending migrations. It has either been applied or does not exist."
    )]
    MigrationNotFoundFromPendingMigrations(MigrationFilename),

    /// A migration file is missing, such as the up or down half of a two-way pair.
    #[error("Migration file - {0} - does not exist")]
    MigrationFileDoesNotExist(String),

    /// A rollback could not be planned.
    #[error("Rollback failed. Error: {0}")]
    RollbackFailed(String),

    /// The migration file and its history record name different migrations.
    #[error(
        "Migration file name and database name mismatch. Migration file name: {migration_file_name}. Migration database name: {migration_db_name}"
    )]
    MigrationFileVsDbNamesMismatch {
        /// The migration file's name.
        migration_file_name: String,
        /// The name in the history record.
        migration_db_name: String,
    },

    /// A file name does not follow `<timestamp>_<name>.[up.|down.]surql`.
    #[error(
        "Invalid migration name. {0}. Make sure it's in the format - <timestamp>_<migration_name>.<up|down|>.surql if two way or <timestamp>_<migration_name.surql if one way"
    )]
    InvalidMigrationName(String),

    /// A migration file could not be read.
    #[error("Migration file not found in path: {path} with error: {error}")]
    MigrationFilePathDoesNotExist {
        /// The file's path.
        path: String,
        /// Why it could not be read.
        error: String,
    },

    /// Whether migrations are one-way or two-way was not set.
    #[error("Migration flag not set")]
    MigrationFlagNotSet,

    /// A migration timestamp could not be produced.
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),

    /// An embedded file's name does not match the migrations' direction.
    #[error("Invalid migration file name for mode: {0}")]
    InvalidMigrationFileNameForMode(String),

    /// The migration directory does not exist.
    #[error("The migration directory ({0}) provided does not exist. You may need to explicitly provide a migration directory path argument. \
        If you are using the default migration directory, make sure you are running the command from the root of your project.
    Check --help for more information")]
    MigrationDirectoryDoesNotExist(String),

    /// The project directory (`CARGO_MANIFEST_DIR`) is not set.
    #[error("Invalid path")]
    PathDoesNotExist,

    /// A field's `old_name` names no field of the migrations' schema, which holds neither the old
    /// nor the new field. (Once the rename is applied, the new field exists and `old_name` is
    /// ignored.)
    #[error(
        "The field - {new_name} - on table - {table} - has an invalid old name - '{old_name}'. \
        It never existed or is wrongly spelt. \
         Also, make sure you are using the correct case for the field name. It should be one of these: {renamables}",
        new_name = .new_name.to_debug_sql(),
        old_name = .old_name.to_debug_sql()
    )]
    InvalidOldFieldName {
        /// The renamed field.
        new_name: Box<Field>,
        /// Its table.
        table: Table,
        /// The `old_name` given.
        old_name: Box<Field>,
        /// The fields that can be renamed.
        renamables: String,
    },

    /// A field is both renamed and the old name of another field.
    #[error(
        "You are trying to rename the field - {field} - on table - {table} - to the same old field name - {field}. \
        This is likely not intentional. Use a different name for the new field",
        field = .field.to_debug_sql()
    )]
    RenamingToSameOldFieldDisallowed {
        /// The field.
        field: Field,
        /// Its table.
        table: Table,
    },

    /// A field is renamed from a field the codebase still defines.
    #[error(
        "You are trying to rename the field-'{new_field}' - on table-'{table}' from a field-'{old_field}' \
        currently still in use in the table. \
        This is likely not intentional. Use a different name for the new field. \
        You can rename only from one of {renamables}"
    )]
    CannotRenameFromOldFieldInUSeInTable {
        /// The renamed field.
        new_field: String,
        /// The `old_name` given.
        old_field: String,
        /// Its table.
        table: Table,
        /// The fields that can be renamed.
        renamables: String,
    },

    /// A database defines a kind of resource migrations do not manage.
    #[error(
        "The {level} defines {kind} ({names}), which migrations do not manage",
        names = .names.join(", ")
    )]
    UnsupportedSchemaResource {
        /// Where the definitions were found, e.g. `namespace` or ``table `person` ``.
        level: String,
        /// The `INFO` key listing them, e.g. `sequences`.
        kind: String,
        /// Their names.
        names: Vec<String>,
    },

    /// A definition whose secrets the database redacts would have to be written to a migration.
    #[error(
        "Cannot write {key} to a migration: the database hides its secrets, so its definition \
        cannot be reproduced from the schema. Manage it outside generated migrations"
    )]
    RedactedDefinition {
        /// The definition.
        key: ResourceKey,
    },

    /// A bucket's backend URL would carry credentials into a migration file. SurrealDB does not
    /// redact it, so the definition is refused instead of being written.
    #[error(
        "Cannot write {key} to a migration: its backend URL carries {reason}, which may be a \
        credential. Give the server its storage credentials through its environment, or manage \
        the bucket outside generated migrations"
    )]
    CredentialInBucketBackend {
        /// The bucket.
        key: ResourceKey,
        /// What in the URL was refused, such as `query parameter secret_key`. Never the value.
        reason: String,
    },

    /// A sequence's batch or start changed. Redefining a sequence resets the values it has
    /// issued, so the change is not written.
    #[error(
        "Cannot update {key} in a migration: changing its batch or start means redefining it, \
        which resets the values it has issued, so they may be issued again. Only its timeout can \
        change in a generated migration; reset it deliberately in a hand-written migration"
    )]
    UnsafeSequenceUpdate {
        /// The sequence.
        key: ResourceKey,
    },

    /// Another runner changed the migration history after this run read it, so this run
    /// applied nothing more. Running again applies what is still pending.
    #[error(
        "Did not apply `{migration}`: the migration history changed after this run read it, \
        so another runner has applied migrations. Run again to apply what is still pending"
    )]
    HistoryChanged {
        /// The migration this run was about to apply.
        migration: MigrationFilename,
    },

    /// A bucket's backend changed. Redefining a bucket connects a new store, which empties a
    /// `memory` bucket, and `ALTER BUCKET` leaves the open store in place until a restart, so the
    /// change is not written. Comment, `READONLY` and permission changes are written as
    /// `ALTER BUCKET`.
    #[error(
        "Cannot change the backend of {key} in a migration: redefining a bucket connects a new \
        store, which loses the files of a `memory` bucket. Change it deliberately in a \
        hand-written migration"
    )]
    UnsafeBucketUpdate {
        /// The bucket.
        key: ResourceKey,
    },

    /// An index change would lose its `PREPARE REMOVE` mark: an index marked for removal can
    /// only change its comment, and marking one for removal cannot be combined with another
    /// change, since redefining it (`OVERWRITE`) clears the mark.
    #[error(
        "Cannot change {key} in a migration: it is or becomes marked for removal (`ALTER INDEX \
        … PREPARE REMOVE`), and redefining it would clear the mark. Change only its comment, or \
        make the change in a hand-written migration"
    )]
    UnsafeIndexUpdate {
        /// The index.
        key: ResourceKey,
    },

    /// A field's `DEFINE FIELD` statement could not be told apart in `INFO` output.
    #[error(
        "Field `{field}` of table `{table}` matches {matches} DEFINE FIELD statements; expected exactly one"
    )]
    FieldDefinitionMismatch {
        /// The field's table.
        table: String,
        /// The field's canonical name.
        field: String,
        /// How many statements matched.
        matches: usize,
    },

    /// A migration statement for a definition could not be rendered, such as the `REMOVE` of a
    /// function whose name is not a path of identifiers.
    #[error("Cannot write {key} to a migration: {}", .errors.join("; "))]
    InvalidStatement {
        /// The definition.
        key: ResourceKey,
        /// Why the statement could not be rendered.
        errors: Vec<String>,
    },

    /// A definition's `DEFINE` statement is not in the server's canonical form.
    #[error("The DEFINE statement of {key} does not start with `DEFINE {}`", .key.kind.keyword())]
    UnexpectedDefinitionText {
        /// The definition.
        key: ResourceKey,
    },

    /// `INFO` output did not have the shape SurrealDB documents.
    #[error("Unexpected INFO output: {0}")]
    UnexpectedInfoShape(String),

    /// The history and the migration directory disagree in strict mode.
    #[error(
        "Invalid migration state. The number of registered migrations in the database ({db_migration_count}) does not match the number of migration files in the migration directory ({local_dir_migration_count}). \
        This could be because you have deleted some migration files from the migration directory or you have deleted some migration records from the database. \
        If you have deleted some migration files from the migration directory, you can run the command 'cargo run -- prune' to delete all unapplied migrations from the database. \
        If you have deleted some migration records from the database, you can run the command 'cargo run -- reset' to delete all migration records from the database. \
        If you have deleted some migration files from the migration directory and some migration records from the database, you can run the command 'cargo run -- reset --prune' to delete all migration records from the database and all unapplied migrations from the database."
    )]
    InvalidMigrationState {
        /// The applied migrations in the history.
        db_migration_count: usize,
        /// Those of them found in the migration directory.
        local_dir_migration_count: usize,
    },

    /// The migration directory holds both one-way and two-way files, or neither.
    #[error("Ambiguous migration direction. Both or neither one-way and two-way file types found in migration directory. Use one or the other.
Up only should not be sufficed with 'up.surql' or 'down.surql' but up and down migrations can be.
There are {one_way_filecount} up only files and {two_way_filecount} up and down files")]
    AmbiguousMigrationDirection {
        /// One-way files found.
        one_way_filecount: usize,
        /// Two-way files found.
        two_way_filecount: usize,
    },

    /// A query failed.
    #[error(transparent)]
    ProblemWithQuery(#[from] crate::Error),

    /// A file system operation failed.
    #[error("{action} `{}` failed", .path.display())]
    Io {
        /// What was being done, e.g. `reading the migration file`.
        action: &'static str,
        /// The file or directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },

    /// A prompt failed.
    #[error(transparent)]
    PromptError(#[from] inquire::error::InquireError),

    /// The database reported an error.
    #[error(transparent)]
    DbError(#[from] surrealdb::Error),

    /// The generator could not remove comparison databases it created (see
    /// [`Comparison`](crate::migrations::Comparison)). Remove them by name.
    #[error(
        "The comparison databases {databases:?} in namespace `{namespace}` were not removed: {cleanup}"
    )]
    ComparisonNotRemoved {
        /// The comparison namespace.
        namespace: String,
        /// The databases left behind.
        databases: Vec<String>,
        /// Why the removal failed.
        cleanup: Box<MigrationError>,
        /// The generation's own failure, when it failed too.
        #[source]
        generation: Option<Box<MigrationError>>,
    },

    /// The server did not confirm a comparison database's removal within
    /// [`COMPARISON_REMOVAL_TIMEOUT`](crate::migrations::COMPARISON_REMOVAL_TIMEOUT). It may still
    /// exist.
    #[error("The comparison database `{database}` was not removed in time")]
    ComparisonTimedOut {
        /// The database.
        database: String,
    },
}

/// The result type of migrations.
pub type MigrationResult<T> = Result<T, MigrationError>;
