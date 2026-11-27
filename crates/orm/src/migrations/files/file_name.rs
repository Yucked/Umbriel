use std::{
    collections::BTreeSet,
    fmt::Display,
    fs::File,
    io::Write,
    ops::Deref,
    path::Path,
};

use chrono::{DateTime, Utc};

use crate::migrations::*;

/// Which file of a migration a name is for. Ordered as file names with the same timestamp and
/// basename sort: up, then down, then one-way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Kind {
    Up,
    Down,
    OneWay,
}

impl Kind {
    /// The file extension, after the basename.
    const fn extension(self) -> &'static str {
        match self {
            Kind::Up => "up.surql",
            Kind::Down => "down.surql",
            Kind::OneWay => "surql",
        }
    }
}

/// A parsed migration file name: `<timestamp>_<basename>.<extension>`, where the extension is
/// `up.surql` or `down.surql` for a two-way migration's files and `surql` for a one-way one.
///
/// Equal, hashed and ordered by timestamp, then basename, then kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MigrationFilename {
    timestamp: u64,
    basename: Basename,
    kind: Kind,
}

/// The file names in a migration directory.
#[derive(Debug, Clone)]
pub struct MigrationFilenames(Vec<MigrationFilename>);

impl Deref for MigrationFilenames {
    type Target = Vec<MigrationFilename>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<MigrationFilename>> for MigrationFilenames {
    fn from(value: Vec<MigrationFilename>) -> Self {
        Self(value)
    }
}

/// Reads a migration file; a failure is [`MigrationError::Io`] with the path and its cause.
fn read(migration_dir: &Path, name: &MigrationFilename) -> MigrationResult<FileContent> {
    FileContent::from_file(migration_dir.join(name.to_string()))
}

impl MigrationFilenames {
    /// Every file name.
    pub fn into_vec(self) -> Vec<MigrationFilename> {
        self.0
    }

    /// How many file names are one-way, and how many are halves of two-way migrations.
    pub fn count_by_direction(&self) -> (usize, usize) {
        let one_way = self.0.iter().filter(|m| m.kind == Kind::OneWay).count();
        (one_way, self.0.len() - one_way)
    }

    /// The two-way migrations read from `migration_dir`, newest first. Each migration is read
    /// once, whichever of its files is listed: a missing up or down file is an error.
    pub fn bidirectional_pair_meta_sorted_desc_checked(
        &self,
        migration_dir: &Path,
    ) -> MigrationResult<Vec<MigrationFileTwoWayPair>> {
        // One entry per migration, by identity, newest first.
        let migrations = self
            .0
            .iter()
            .filter(|m| matches!(m.kind, Kind::Up | Kind::Down))
            .map(MigrationFilename::to_up)
            .collect::<BTreeSet<_>>();

        let mut pairs = Vec::with_capacity(migrations.len());
        for up in migrations.into_iter().rev() {
            let down = up.to_down();
            pairs.push(MigrationFileTwoWayPair::new(
                FileMetadata {
                    content: read(migration_dir, &up)?,
                    name: up,
                },
                FileMetadata {
                    content: read(migration_dir, &down)?,
                    name: down,
                },
            )?);
        }
        Ok(pairs)
    }

    /// The one-way migrations read from `migration_dir`, oldest first.
    pub fn unidirectional_pair_meta(
        &self,
        migration_dir: &Path,
    ) -> MigrationResult<Vec<MigrationFileOneWay>> {
        let migrations = self
            .0
            .iter()
            .filter(|m| m.kind == Kind::OneWay)
            .collect::<BTreeSet<_>>();

        let mut files = Vec::with_capacity(migrations.len());
        for migration in migrations {
            files.push(MigrationFileOneWay::new(FileMetadata {
                content: read(migration_dir, migration)?,
                name: migration.clone(),
            })?);
        }
        Ok(files)
    }
}

/// Name without extension nor timestamp
/// e.g for 20210912121212_create_users.up.surql, the basename is create_users
#[derive(Clone, Hash, Debug, PartialEq, Ord, PartialOrd, Eq)]
pub struct Basename(String);

impl Basename {
    /// The name with every character other than `_` and alphanumerics replaced by `_`.
    pub fn normalize_ensure(&self) -> Basename {
        Basename(
            self.0
                .chars()
                .map(|c| {
                    if c == '_' || c.is_alphanumeric() {
                        c
                    } else {
                        '_'
                    }
                })
                .collect(),
        )
    }
}

impl From<&str> for Basename {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for Basename {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Display for Basename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl MigrationFilename {
    fn new(timestamp: u64, basename: Basename, kind: Kind) -> Self {
        Self {
            timestamp,
            basename,
            kind,
        }
    }

    /// Whether this is an up file.
    pub fn is_up(&self) -> bool {
        self.kind == Kind::Up
    }

    /// Whether this is a down file.
    pub fn is_down(&self) -> bool {
        self.kind == Kind::Down
    }

    /// Whether this is a one-way file.
    pub fn is_unidirectional(&self) -> bool {
        self.kind == Kind::OneWay
    }

    /// The timestamp the name starts with.
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp.into()
    }

    /// The extension for the file's direction: `up.surql`, `down.surql` or `surql`.
    pub fn extension(&self) -> &'static str {
        self.kind.extension()
    }

    /// The name without timestamp or extension.
    pub fn basename(&self) -> Basename {
        self.basename.clone()
    }

    fn with_kind(&self, kind: Kind) -> MigrationFilename {
        Self::new(self.timestamp, self.basename.clone(), kind)
    }

    /// The up file name with the same timestamp and basename.
    pub fn to_up(&self) -> MigrationFilename {
        self.with_kind(Kind::Up)
    }

    /// The down file name with the same timestamp and basename.
    pub fn to_down(&self) -> MigrationFilename {
        self.with_kind(Kind::Down)
    }

    /// The one-way file name with the same timestamp and basename.
    pub fn to_unidirectional(&self) -> MigrationFilename {
        self.with_kind(Kind::OneWay)
    }

    /// Writes `query` to this file in the migration directory, creating the directory if needed.
    ///
    /// # Errors
    ///
    /// [`MigrationError::Io`] when the file already exists (it is never overwritten) or cannot be
    /// written.
    pub fn create_file(&self, query: &FileContent, config: &MigrationConfig) -> MigrationResult<()> {
        let file_path = config
            .resolve_migration_directory(true)?
            .join(self.to_string());

        let mut file = File::create_new(&file_path).map_err(|source| MigrationError::Io {
            action: "creating the migration file",
            path: file_path.clone(),
            source,
        })?;

        file.write_all(query.as_bytes())
            .map_err(|source| MigrationError::Io {
                action: "writing the migration file",
                path: file_path.clone(),
                source,
            })?;

        Ok(())
    }

    /// An up file name for `timestamp`.
    pub fn create_up(timestamp: DateTime<Utc>, basename: &Basename) -> MigrationResult<Self> {
        Ok(Self::new(
            Self::format_timestamp(timestamp)?,
            basename.clone(),
            Kind::Up,
        ))
    }

    /// A down file name for `timestamp`.
    pub fn create_down(timestamp: DateTime<Utc>, basename: &Basename) -> MigrationResult<Self> {
        Ok(Self::new(
            Self::format_timestamp(timestamp)?,
            basename.clone(),
            Kind::Down,
        ))
    }

    /// A one-way file name for `timestamp`.
    pub fn create_oneway(timestamp: DateTime<Utc>, basename: &Basename) -> MigrationResult<Self> {
        Ok(Self::new(
            Self::format_timestamp(timestamp)?,
            basename.clone(),
            Kind::OneWay,
        ))
    }

    /// `YYYYMMDDHHMMSSmmm` as a number. A year beyond 9999 formats to more digits, and one large
    /// enough overflows `u64`, which is an error.
    fn format_timestamp(timestamp: DateTime<Utc>) -> MigrationResult<u64> {
        timestamp
            .format("%Y%m%d%H%M%S%3f")
            .to_string()
            .parse::<u64>()
            .map_err(|e| MigrationError::InvalidTimestamp(e.to_string()))
    }

    /// Parses `<14–20 ASCII digits>_<basename>.<up.surql | down.surql | surql>`: the basename is
    /// one or more alphanumerics (Unicode), `_` or `-`; the timestamp must fit a `u64`; nothing
    /// may follow the extension.
    fn parse(name: &str) -> Option<Self> {
        let digits = name.bytes().take_while(u8::is_ascii_digit).count();
        if !(14..=20).contains(&digits) {
            return None;
        }
        let (timestamp, rest) = name.split_at(digits);
        let timestamp = timestamp.parse::<u64>().ok()?;
        let rest = rest.strip_prefix('_')?;
        let end = rest
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        let (basename, extension) = rest.split_at(end);
        let kind = match extension {
            ".up.surql" => Kind::Up,
            ".down.surql" => Kind::Down,
            ".surql" => Kind::OneWay,
            _ => return None,
        };
        Some(Self::new(timestamp, basename.into(), kind))
    }
}

impl TryFrom<String> for MigrationFilename {
    type Error = MigrationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).ok_or(MigrationError::InvalidMigrationName(value))
    }
}

impl std::str::FromStr for MigrationFilename {
    type Err = MigrationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or_else(|| MigrationError::InvalidMigrationName(value.to_owned()))
    }
}

impl Display for MigrationFilename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}_{}.{}",
            self.timestamp,
            self.basename,
            self.kind.extension()
        )
    }
}
