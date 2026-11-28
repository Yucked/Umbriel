use std::fmt::Display;

use crate::migrations::MigrationFilename;

/// Which migrations to list. The CLI parses it by its `ValueEnum` names.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Status {
    /// Recorded in the history.
    Applied,
    /// Not yet applied.
    Pending,
    /// Every migration file.
    All,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Status::Applied => "applied",
            Status::Pending => "pending",
            Status::All => "all",
        })
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

/// How far to roll back.
pub enum RollbackStrategy {
    /// The latest applied migration (`down`, `down --previous`); the default.
    Previous,
    /// This many applied migrations (`down --number 3`).
    Number(u32),
    /// Every migration applied after this one, and it (`down --till <file name>`).
    Till(MigrationFilename),
}

/// Which way a migration is applied.
#[derive(Debug, Clone)]
pub enum Direction {
    /// From the previous schema to the new one.
    Up,
    /// Back from the new schema to the previous one.
    Down,
}

impl Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Up => "up",
            Self::Down => "down",
        })
    }
}

/// Whether migrations can be rolled back.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationFlag {
    /// Each migration is an `.up.surql` and `.down.surql` pair.
    #[default]
    TwoWay,
    /// Each migration is a single `.surql` file.
    OneWay,
}

impl Display for MigrationFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TwoWay => "two_way",
            Self::OneWay => "one_way",
        })
    }
}

/// How closely migration files are checked against the history. The CLI parses it by its
/// `ValueEnum` names, `strict` and `lax`.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Mode {
    /// File names and checksums must match the history.
    #[default]
    Strict,
    /// Files are not checked against the history.
    Lax,
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Strict => "strict",
            Self::Lax => "lax",
        })
    }
}
