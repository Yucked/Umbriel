use std::{
    fmt::{Display, Write},
    fs,
    path::PathBuf,
};

use sha2::{Digest, Sha256};
use surrealdb::types::SurrealValue;

use crate::migrations::*;

/// The SHA-256 digest of a migration file, in lowercase hex. It is computed from a file
/// ([`generate_from_content`](Self::generate_from_content)) or read from the history; nothing
/// builds one from arbitrary text.
#[derive(SurrealValue, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Checksum(String);

impl Display for Checksum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Checksum {
    /// The checksum of a file's content.
    pub fn generate_from_content(content: &FileContent) -> MigrationResult<Self> {
        Ok(Self::of(content.as_bytes()))
    }

    /// The checksum of the file at `file_path`.
    pub fn generate_from_path(file_path: impl Into<PathBuf>) -> MigrationResult<Self> {
        let file_path: PathBuf = file_path.into();
        let content = fs::read(&file_path).map_err(|source| MigrationError::Io {
            action: "reading the migration file",
            path: file_path.clone(),
            source,
        })?;
        Ok(Self::of(&content))
    }

    /// The SHA-256 digest of `bytes` in lowercase hex.
    fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest.iter() {
            // invariant: writing to a `String` cannot fail.
            let _ = write!(hex, "{byte:02x}");
        }
        Self(hex)
    }

    /// Checks that `content` still has this checksum, i.e. the applied file was not edited.
    pub fn verify(
        &self,
        migration_filename: &MigrationFilename,
        content: &FileContent,
    ) -> MigrationResult<()> {
        let checksum = Checksum::generate_from_content(content)?;
        if checksum != *self {
            return Err(MigrationError::ChecksumMismatch {
                migration_name: migration_filename.to_string(),
                expected_checksum: self.to_string(),
                actual_checksum: checksum.to_string(),
            });
        }
        Ok(())
    }
}
