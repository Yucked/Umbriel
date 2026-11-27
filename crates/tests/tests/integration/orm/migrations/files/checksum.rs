//! Checksums (`orm/src/migrations/files/checksum.rs`): computed from content, and decoded from
//! the history only when they are a SHA-256 digest in lowercase hex.

use surrealdb::types::{SurrealValue, Value};
use umbriel::migrations::*;

#[test]
fn a_checksum_is_the_content_digest_and_round_trips() {
    let content: FileContent = "DEFINE TABLE note;".to_owned().into();
    let checksum = Checksum::generate_from_content(&content);
    assert_eq!(checksum.to_string().len(), 64);
    assert_eq!(
        Checksum::from_value(checksum.clone().into_value()).expect("decodes"),
        checksum
    );
}

#[test]
fn decoding_refuses_anything_but_a_lowercase_digest() {
    let digest = "a".repeat(64);
    assert!(Checksum::from_value(Value::String(digest.clone())).is_ok());
    for corrupt in [
        String::new(),
        "a".repeat(63),
        "a".repeat(65),
        digest.to_uppercase(),
        format!("{}g", "a".repeat(63)),
    ] {
        assert!(
            Checksum::from_value(Value::String(corrupt.clone())).is_err(),
            "`{corrupt}` decodes"
        );
    }
    assert!(Checksum::from_value(1i64.into_value()).is_err());
}
