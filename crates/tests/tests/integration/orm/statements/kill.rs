//! `KILL` (`orm/src/statements/kill.rs`): a parameter or a live query's id (L15).

use umbriel::{Param, Render, statements::kill};
use umbriel_tests::*;

#[test]
fn a_parameter_renders_and_parses() {
    let id = Param::new("id");
    assert_eq!(kill(&id).to_debug_sql(), "KILL $id;");
    assert_eq!(kill(id).to_debug_sql(), "KILL $id;");
    assert_parses("KILL $id;");
}
