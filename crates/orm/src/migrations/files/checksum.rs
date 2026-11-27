use std::fmt::{Display, Write};

use sha2::{Digest, Sha256};
use surrealdb::types::{Kind, SerializationError, SurrealValue, Value};

use crate::migrations::*;

/// The SHA-256 digest of a migration file, in lowercase hex. It is computed from a file's
/// content ([`generate_from_content`](Self::generate_from_content)) or decoded from the history,
/// which accepts only 64 lowercase hex digits; nothing builds one from arbitrary text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Checksum(String);

impl Display for Checksum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Checksum {
    /// The checksum of a file's content.
    pub fn generate_from_content(content: &FileContent) -> Self {
        let digest = Sha256::digest(content.as_bytes());
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest.iter() {
            // invariant: writing to a `String` cannot fail.
            let _ = write!(hex, "{byte:02x}");
        }
        Self(hex)
    }

    /// Whether `text` is a SHA-256 digest in lowercase hex, as the history stores it.
    fn is_digest(text: &str) -> bool {
        text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    /// Checks that `content` still has this checksum, i.e. the applied file was not edited.
    ///
    /// # Errors
    ///
    /// [`MigrationError::ChecksumMismatch`] when it doesn't.
    pub fn verify(
        &self,
        migration_filename: &MigrationFilename,
        content: &FileContent,
    ) -> MigrationResult<()> {
        let checksum = Checksum::generate_from_content(content);
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

/// Stored as its hex string. Decoding refuses a string that isn't a SHA-256 digest in lowercase
/// hex, so a corrupt history record fails when it is read rather than at a later comparison.
impl SurrealValue for Checksum {
    fn kind_of() -> Kind {
        Kind::String
    }

    fn into_value(self) -> Value {
        Value::String(self.0)
    }

    fn from_value(value: Value) -> Result<Self, surrealdb::Error> {
        let text = String::from_value(value)?;
        if !Self::is_digest(&text) {
            return Err(surrealdb::Error::serialization(
                format!("`{text}` is not a SHA-256 checksum in lowercase hex"),
                SerializationError::Deserialization,
            ));
        }
        Ok(Self(text))
    }
}
