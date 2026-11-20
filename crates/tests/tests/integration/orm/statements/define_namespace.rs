//! `DEFINE NAMESPACE` (`orm/src/statements/define_namespace.rs`).

use umbriel_tests::{support::statements::*, *};

#[test]
fn a_namespace_renders_by_name() {
    assert_sql(
        &define_namespace(Namespace::new("acme")),
        "DEFINE NAMESPACE acme;",
    );
}
