//! Surrealism module calls (`orm/src/functions/module.rs`): rendering and path checks. Running a
//! module function needs a module definition, which #22 covers.

use umbriel::{Field, Render, functions::module, kinds};
use umbriel_tests::*;

#[test]
fn calls_render() {
    let call = module::call::<kinds::Number, _>("geo::distance", [Field::new("a"), Field::new("b")]);
    assert_eq!(call.to_debug_sql(), "mod::geo::distance(a, b)");
    let bound = module::call::<kinds::Any, _>("tools", [1]);
    assert_eq!(bound.to_query().expect("renders").sql(), "mod::tools($_p0)");
}

#[test]
fn bad_paths_are_render_errors() {
    for path in ["", "a::", "::a", "a::1b", "a;b", "a b"] {
        let call = module::call::<kinds::Any, _>(path, [1]);
        assert_render_error(&call, "invalid function name");
    }
}
