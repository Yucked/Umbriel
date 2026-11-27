use clap::ValueEnum;
use std::fmt::Display;

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
#[derive(ValueEnum, Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
