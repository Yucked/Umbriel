use std::{fmt::Display, fs, path::PathBuf};

use crate::migrations::{Checksum, MigrationError, MigrationResult};

/// A migration file's SurrealQL. The default is no content.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileContent(String);

impl Display for FileContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl FileContent {
    /// The content's checksum.
    pub fn as_checksum(&self) -> MigrationResult<Checksum> {
        Checksum::generate_from_content(self)
    }

    /// Reads the file at `file_path`.
    pub fn from_file(file_path: impl Into<PathBuf>) -> MigrationResult<Self> {
        let file_path = file_path.into();
        let content = fs::read_to_string(&file_path).map_err(|source| MigrationError::Io {
            action: "reading the migration file",
            path: file_path,
            source,
        })?;
        Ok(Self(content))
    }

    /// The content as bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// The content as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether there is no content.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for FileContent {
    fn from(content: String) -> Self {
        Self(content)
    }
}
