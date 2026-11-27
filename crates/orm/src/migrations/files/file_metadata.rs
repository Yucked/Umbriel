use crate::migrations::*;
use chrono::Utc;

/// A two-way migration's up and down files, which name the same migration.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MigrationFileTwoWayPair {
    up: FileMetadata,
    down: FileMetadata,
}

impl MigrationFileTwoWayPair {
    /// Pairs an up file with its down file.
    ///
    /// # Errors
    ///
    /// [`MigrationError::InvalidMigrationName`] unless `up` is an up file and `down` the down
    /// file of the same timestamp and name.
    pub fn new(up: FileMetadata, down: FileMetadata) -> MigrationResult<Self> {
        if !up.name.is_up() || down.name != up.name.to_down() {
            return Err(MigrationError::InvalidMigrationName(format!(
                "`{}` and `{}` are not the up and down files of one migration",
                up.name, down.name
            )));
        }
        Ok(Self { up, down })
    }

    /// The up file.
    pub fn up(&self) -> &FileMetadata {
        &self.up
    }

    /// The down file.
    pub fn down(&self) -> &FileMetadata {
        &self.down
    }
}

/// A one-way migration's file. A type of its own, so a one-way file and either half of a
/// two-way pair aren't mixed up.
#[derive(Clone, Debug)]
pub struct MigrationFileOneWay(FileMetadata);

impl MigrationFileOneWay {
    /// Wraps a one-way file.
    ///
    /// # Errors
    ///
    /// [`MigrationError::InvalidMigrationName`] when the file is the up or down half of a two-way
    /// migration.
    pub fn new(file: FileMetadata) -> MigrationResult<Self> {
        if !file.name.is_unidirectional() {
            return Err(MigrationError::InvalidMigrationName(format!(
                "`{}` is half of a two-way migration, not a one-way one",
                file.name
            )));
        }
        Ok(Self(file))
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
                })?;
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
