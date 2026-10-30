//! `DEFINE PARAM` (`orm/src/statements/define_param.rs`): its single-form `PERMISSIONS`.
//!
//! Only rendering is tested: checking that `PERMISSIONS NONE` hides the param needs a record
//! user, and the record-access setup belongs to #15.

use umbriel::{Param, Render, statements::define_param};
use umbriel_tests::*;

#[test]
fn permissions_take_none_full_or_a_condition() {
    let greeting = Param::new("greeting");
    assert_sql(
        &define_param(&greeting).value("Hello").permissions_none(),
        "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS NONE;",
    );
    assert_sql(
        &define_param(&greeting).value("Hello").permissions_full(),
        "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS FULL;",
    );
    assert_sql(
        &define_param(&greeting)
            .value("Hello")
            .permissions_where(Param::new("public").is(true)),
        "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS WHERE $public IS true;",
    );
}

#[test]
fn the_last_permission_wins() {
    let statement = define_param(&Param::new("greeting"))
        .value("Hello")
        .permissions_none()
        .permissions_full();
    assert_eq!(
        statement.to_debug_sql(),
        "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS FULL;"
    );
}

#[test]
fn permissions_follow_the_comment() {
    assert_sql(
        &define_param(&Param::new("greeting"))
            .value("Hello")
            .comment("shown on sign-in")
            .permissions_none(),
        "DEFINE PARAM $greeting VALUE 'Hello' COMMENT 'shown on sign-in' PERMISSIONS NONE;",
    );
}
