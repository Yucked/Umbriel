//! Reserved parameters (`orm/src/query/params_standard.rs`): each renders as its `$name`, and the
//! protected ones cannot be set.

use umbriel::{Render, Runnable, statements::let_};
use umbriel_tests::*;

#[test]
fn every_reserved_parameter_renders_its_name() {
    let rendered = [
        (umbriel::access().to_debug_sql(), "$access"),
        (umbriel::reference().to_debug_sql(), "$reference"),
        (umbriel::request().to_debug_sql(), "$request"),
        (umbriel::action().to_debug_sql(), "$action"),
        (umbriel::file().to_debug_sql(), "$file"),
        (umbriel::target().to_debug_sql(), "$target"),
        (umbriel::auth().to_debug_sql(), "$auth"),
        (umbriel::token().to_debug_sql(), "$token"),
        (umbriel::session().to_debug_sql(), "$session"),
    ];
    for (sql, expected) in rendered {
        assert_eq!(sql, expected);
    }
}

#[tokio::test]
async fn protected_parameters_cannot_be_set() {
    let db = db().await;
    for name in ["access", "auth", "token", "session"] {
        let refused = let_(name).equal_to(1).run(db.clone()).await;
        assert!(refused.is_err(), "${name}: {refused:?}");
    }
}
