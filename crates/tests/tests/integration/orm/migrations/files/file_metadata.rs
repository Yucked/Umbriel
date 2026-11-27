//! Migration files (`orm/src/migrations/files/file_metadata.rs`): a two-way pair is the up and
//! down of one migration, and a one-way file is neither half of a pair.

use umbriel::migrations::*;

fn file(name: &str, content: &str) -> FileMetadata {
    FileMetadata::new(
        name.parse().expect("a migration file name"),
        content.to_owned().into(),
    )
}

#[test]
fn a_pair_is_the_up_and_down_of_one_migration() {
    let up = || file("20231030025711_note.up.surql", "DEFINE TABLE note;");
    let down = || file("20231030025711_note.down.surql", "REMOVE TABLE note;");
    let pair = MigrationFileTwoWayPair::new(up(), down()).expect("a pair");
    assert_eq!(pair.up(), &up());
    assert_eq!(pair.down(), &down());

    for (up, down) in [
        (down(), up()),
        (up(), file("20231030025711_other.down.surql", "")),
        (up(), file("20231030025712_note.down.surql", "")),
        (up(), file("20231030025711_note.surql", "")),
    ] {
        let error = MigrationFileTwoWayPair::new(up, down).expect_err("not a pair");
        assert!(
            matches!(error, MigrationError::InvalidMigrationName(_)),
            "{error:?}"
        );
    }
}

#[test]
fn a_one_way_file_is_not_half_of_a_pair() {
    assert!(MigrationFileOneWay::new(file("20231030025711_note.surql", "")).is_ok());
    for half in [
        "20231030025711_note.up.surql",
        "20231030025711_note.down.surql",
    ] {
        let error = MigrationFileOneWay::new(file(half, "")).expect_err("a two-way half");
        assert!(
            matches!(error, MigrationError::InvalidMigrationName(_)),
            "{error:?}"
        );
    }
}
