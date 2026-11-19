//! `DEFINE FUNCTION` (`orm/src/statements/define_function.rs`) and the function name check it
//! shares with `REMOVE FUNCTION`: a name is the path after `fn::`, made of identifiers.

use surrealdb::types::Kind;
use umbriel::{
    Field, Operatable, Param, Render, Runnable, chain,
    statements::{
        DefineFunctionStatement, FunctionArgument, define_function, remove_function, return_,
    },
};
use umbriel_tests::*;

/// Names that would carry SurrealQL of their own, or are not paths of identifiers.
const HOSTILE: [&str; 10] = [
    "add_one; REMOVE TABLE user",
    "add_one() {}; DEFINE USER x",
    "add_one -- comment",
    "math::",
    "::add_one",
    "math::::add_one",
    "1st",
    "add-one",
    "`add_one`",
    "",
];

fn add_one(name: &str) -> DefineFunctionStatement {
    let x = Param::new("x");
    define_function(name)
        .arguments(vec![FunctionArgument {
            name: x.clone(),
            type_: Kind::Int,
        }])
        .body(chain(return_(Field::from(&x).add(1))))
}

#[test]
fn a_path_of_identifiers_renders_on_both_builders() {
    assert_sql(
        &add_one("math::add_one"),
        "DEFINE FUNCTION fn::math::add_one($x: int) {\nRETURN $x + 1;\n};",
    );
    assert_sql(
        &remove_function("math::add_one"),
        "REMOVE FUNCTION fn::math::add_one;",
    );
}

#[test]
fn hostile_names_fail_without_being_written() {
    for hostile in HOSTILE {
        let define = add_one(hostile);
        assert_render_error(&define, "invalid function name");
        let remove = remove_function(hostile);
        assert_render_error(&remove, "invalid function name");
        if !hostile.is_empty() {
            assert!(!define.to_debug_sql().contains(hostile), "{hostile}");
            assert!(!remove.to_debug_sql().contains(hostile), "{hostile}");
        }
    }
}

#[test]
fn a_name_given_with_fn_fails_with_a_hint() {
    for name in ["fn::greet", "fn::math::add_one", "fn"] {
        assert_render_error(&add_one(name), "pass the name without `fn::`");
        assert_render_error(&remove_function(name), "pass the name without `fn::`");
    }
}

#[tokio::test]
async fn a_defined_function_runs_and_is_removed() {
    let (db, _guard) = db().await;
    add_one("math::add_one")
        .run(&db)
        .await
        .expect("the function is defined");
    let result: i64 = read(&db, "RETURN fn::math::add_one(41);").await;
    assert_eq!(result, 42);

    remove_function("math::add_one")
        .run(&db)
        .await
        .expect("the function is removed");
    assert!(!succeeds(&db, "RETURN fn::math::add_one(1);").await);
}
