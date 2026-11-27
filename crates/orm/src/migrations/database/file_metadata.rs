use crate::migrations::*;
use chrono::Utc;

/// A two-way migration's up and down files.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MigrationFileTwoWayPair {
    /// The up file.
    pub up: FileMetadata,
    /// The down file.
    pub down: FileMetadata,
}

impl MigrationFileTwoWayPair {
    /// Pairs an up file with its down file.
    pub fn new(up: FileMetadata, down: FileMetadata) -> Self {
        Self { up, down }
    }
}

impl TryFrom<MigrationFileTwoWayPair> for Migration {
    type Error = MigrationError;

    fn try_from(migration: MigrationFileTwoWayPair) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Migration::create_id(&migration.up.name),
            name: migration.up.name.to_up().to_string(),
            timestamp: migration.up.name.timestamp(),
            checksum_up: migration.up.content.as_checksum()?,
            checksum_down: Some(migration.down.content.as_checksum()?),
        })
    }
}

/// A one-way migration's file.
#[derive(Clone, Debug)]
pub struct MigrationFileOneWay(FileMetadata);

impl MigrationFileOneWay {
    /// Wraps a one-way file.
    pub fn new(file: FileMetadata) -> Self {
        Self(file)
    }

    /// The file.
    pub fn file_meta(&self) -> &FileMetadata {
        &self.0
    }

    /// The file's name.
    pub fn name(&self) -> &MigrationFilename {
        &self.0.name
    }

    /// The file's content.
    pub fn content(&self) -> &FileContent {
        &self.0.content
    }
}

/// A migration file's name and content.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileMetadata {
    /// The file's name.
    pub name: MigrationFilename,
    /// The file's content.
    pub content: FileContent,
}

impl FileMetadata {
    /// A file with this name and content.
    pub fn new(name: MigrationFilename, content: FileContent) -> Self {
        Self { name, content }
    }
}

impl TryFrom<MigrationFileOneWay> for Migration {
    type Error = MigrationError;

    fn try_from(migration: MigrationFileOneWay) -> Result<Self, Self::Error> {
        let migration = migration.0;
        Ok(Self {
            id: Migration::create_id(&migration.name),
            name: migration.name.to_string(),
            timestamp: migration.name.timestamp(),
            checksum_up: Checksum::generate_from_content(&migration.content)?,
            checksum_down: None,
        })
    }
}

impl From<MigrationFileTwoWayPair> for MigrationFileOneWay {
    fn from(m: MigrationFileTwoWayPair) -> Self {
        Self::new(FileMetadata::new(m.up.name, m.up.content))
    }
}

/// A migration of either kind.
#[derive(Debug, Clone)]
pub enum MigrationFile {
    /// A one-way migration.
    OneWay(MigrationFileOneWay),
    /// A two-way migration.
    TwoWay(MigrationFileTwoWayPair),
}

impl MigrationFile {
    /// A new migration timestamped now; `down_queries` is used only for two-way migrations.
    pub fn new(
        migration_basename: &Basename,
        flag: &MigrationFlag,
        up_queries: &FileContent,
        down_queries: &FileContent,
    ) -> MigrationResult<Self> {
        let timestamp = Utc::now();
        let migration_file = match flag {
            MigrationFlag::TwoWay => {
                let file = MigrationFileTwoWayPair {
                    up: FileMetadata {
                        name: MigrationFilename::create_up(timestamp, migration_basename)?,
                        content: up_queries.clone(),
                    },
                    down: FileMetadata {
                        name: MigrationFilename::create_down(timestamp, migration_basename)?,
                        content: down_queries.clone(),
                    },
                };
                MigrationFile::TwoWay(file)
            }
            MigrationFlag::OneWay => {
                let file = MigrationFileOneWay::new(FileMetadata {
                    name: MigrationFilename::create_oneway(timestamp, migration_basename)?,
                    content: up_queries.clone(),
                });
                MigrationFile::OneWay(file)
            }
        };

        Ok(migration_file)
    }

    /// Writes the migration's files to the migration directory.
    pub fn create_file(&self, file_manager: &MigrationConfig) -> MigrationResult<()> {
        match self {
            Self::OneWay(m) => m.name().create_file(m.content(), file_manager)?,
            Self::TwoWay(m) => {
                m.down.name.create_file(&m.down.content, file_manager)?;
                m.up.name.create_file(&m.up.content, file_manager)?;
            }
        };

        Ok(())
    }
}

impl MigrationFile {
    /// The name of the file applied going forward: the up or only file.
    pub fn name_forward(&self) -> &MigrationFilename {
        match self {
            Self::OneWay(m) => m.name(),
            Self::TwoWay(m) => &m.up.name,
        }
    }

    /// The up or only file's content.
    pub fn up_content(&self) -> &FileContent {
        match self {
            Self::OneWay(m) => m.content(),
            Self::TwoWay(m) => &m.up.content,
        }
    }

    /// The down file's content; `None` for one-way migrations.
    pub fn down_content(&self) -> Option<&FileContent> {
        match self {
            Self::OneWay(_) => None,
            Self::TwoWay(m) => Some(&m.down.content),
        }
    }
}

impl From<MigrationFileOneWay> for MigrationFile {
    fn from(m: MigrationFileOneWay) -> Self {
        Self::OneWay(m)
    }
}

impl From<MigrationFileTwoWayPair> for MigrationFile {
    fn from(m: MigrationFileTwoWayPair) -> Self {
        Self::TwoWay(m)
    }
}
