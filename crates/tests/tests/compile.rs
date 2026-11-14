//! Compile-pass and compile-fail checks for the exported macros and their diagnostics.
//!
//! Each `tests/ui/fail` case has a `.stderr` file with the expected diagnostic. After changing a
//! diagnostic on purpose, regenerate them with `TRYBUILD=overwrite` and review the diff.

use std::process::Command;

#[test]
fn ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass/*.rs");
    cases.compile_fail("tests/ui/fail/*.rs");
}

/// Checks the standalone consumer crate `fixtures/<fixture>`. A fixture is a separate crate
/// because `trybuild` cases always see this crate's own dependencies.
fn check_fixture(fixture: &str) {
    let root = env!("CARGO_MANIFEST_DIR");
    let status = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet", "--manifest-path"])
        .arg(format!("{root}/fixtures/{fixture}/Cargo.toml"))
        .arg("--target-dir")
        .arg(format!("{root}/../../target/fixtures"))
        .status()
        .expect("failed to run cargo");
    assert!(status.success(), "{fixture} fixture failed to compile");
}

/// A consumer that depends on `umbriel` under another name must still expand.
#[test]
fn renamed_dependency() {
    check_fixture("renamed-dependency");
}

/// A consumer with `umbriel` as its only dependency connects, runs statements and model methods,
/// and uses an SDK transaction, all through Umbriel's re-exports (#29, K01).
#[test]
fn one_dependency() {
    check_fixture("one-dependency");
}
