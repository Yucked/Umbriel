//! Migration file names (`orm/src/migrations/files/file_name.rs`): the name rules at their
//! boundaries, the two-way loader's pairing and its error sources, and `create_file` never
//! overwriting a file.

use std::{error::Error as _, fs, io::ErrorKind};

use umbriel::migrations::*;
use umbriel_tests::migrations::helpers::{config, migration_dir};

fn parses(name: &str) -> bool {
    name.parse::<MigrationFilename>().is_ok()
}

#[test]
fn the_timestamp_has_14_to_20_ascii_digits_that_fit_a_u64() {
    assert!(!parses("2023103002571_note.surql"), "13 digits");
    assert!(parses("20231030025711_note.surql"), "14 digits");
    assert!(
        parses("20231030025711123_note.surql"),
        "17 digits, as generated"
    );
    assert!(
        parses("12345678901234567890_note.surql"),
        "20 digits within u64"
    );
    assert!(
        !parses("99999999999999999999_note.surql"),
        "20 digits over u64::MAX"
    );
    assert!(!parses("123456789012345678901_note.surql"), "21 digits");
    assert!(
        !parses("٢٠٢٣١٠٣٠٠٢٥٧١١_note.surql"),
        "non-ASCII digits are not a timestamp"
    );
}

#[test]
fn the_rest_of_the_name_is_strict() {
    assert!(!parses("20231030025711note.surql"), "no separator");
    assert!(!parses("20231030025711_.surql"), "empty basename");
    assert!(!parses("20231030025711_a b.surql"), "a space");
    assert!(!parses("20231030025711_note.sql"), "another extension");
    assert!(!parses("20231030025711_note.surql.bak"), "something after");
    assert!(
        !parses("20231030025711_note.up.down.surql"),
        "two directions"
    );
    assert!(parses("20231030025711_add-notes_v2.surql"), "`_` and `-`");
}

#[test]
fn a_unicode_basename_is_kept() {
    let name = "20231030025711_café_ñandú.up.surql";
    let parsed: MigrationFilename = name.parse().expect("parses");
    assert_eq!(parsed.to_string(), name);
    assert_eq!(parsed.basename(), "café_ñandú".into());
    assert!(parsed.is_up());
}

#[test]
fn the_loader_pairs_each_migration_once() {
    let (_temp, dir) = migration_dir();
    fs::create_dir(&dir).expect("created");
    for (name, content) in [
        ("20231030025711_note.up.surql", "DEFINE TABLE note;"),
        ("20231030025711_note.down.surql", "REMOVE TABLE note;"),
        ("20231031000000_tag.up.surql", "DEFINE TABLE tag;"),
        ("20231031000000_tag.down.surql", "REMOVE TABLE tag;"),
    ] {
        fs::write(dir.join(name), content).expect("written");
    }
    let pairs = config(&dir)
        .get_two_way_migrations_sorted_asc(false)
        .expect("loaded");
    let loaded = pairs
        .iter()
        .map(|pair| (pair.up().name.to_string(), pair.down().content.to_string()))
        .collect::<Vec<_>>();
    assert_eq!(
        loaded,
        [
            (
                "20231030025711_note.up.surql".to_owned(),
                "REMOVE TABLE note;".to_owned()
            ),
            (
                "20231031000000_tag.up.surql".to_owned(),
                "REMOVE TABLE tag;".to_owned()
            ),
        ]
    );
}

#[test]
fn a_missing_half_is_an_io_error_with_its_source() {
    let (_temp, dir) = migration_dir();
    fs::create_dir(&dir).expect("created");
    fs::write(
        dir.join("20231030025711_note.up.surql"),
        "DEFINE TABLE note;",
    )
    .expect("written");
    let error = config(&dir)
        .get_two_way_migrations_sorted_asc(false)
        .expect_err("the down file is missing");
    let MigrationError::Io { path, source, .. } = &error else {
        panic!("expected Io, got {error:?}");
    };
    assert!(path.ends_with("20231030025711_note.down.surql"), "{path:?}");
    assert_eq!(source.kind(), ErrorKind::NotFound);
    assert!(error.source().is_some(), "the io::Error is the source");
}

#[test]
fn create_file_never_overwrites() {
    let (_temp, dir) = migration_dir();
    let name: MigrationFilename = "20231030025711_note.surql".parse().expect("parses");
    let config = config(&dir);
    name.create_file(&"DEFINE TABLE note;".to_owned().into(), &config)
        .expect("created");
    let error = name
        .create_file(&"REMOVE TABLE note;".to_owned().into(), &config)
        .expect_err("the file exists");
    let MigrationError::Io { source, .. } = &error else {
        panic!("expected Io, got {error:?}");
    };
    assert_eq!(source.kind(), ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(dir.join(name.to_string())).expect("readable"),
        "DEFINE TABLE note;"
    );
}
