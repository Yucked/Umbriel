//! `FOR` permissions (`orm/src/statements/for_permission.rs`): one or several operations under a condition.

use umbriel_tests::support::statements::*;

#[test]
fn for_permissions() {
    let age = Field::new("age");
    assert_eq!(
        for_permission(Select)
            .where_(age.greater_than_or_equal(18))
            .to_debug_sql(),
        "FOR select\n\tWHERE age >= 18"
    );
    assert_eq!(
        for_permission([Create, Update, Delete])
            .where_(age.less_than_or_equal(130))
            .to_debug_sql(),
        "FOR create, update, delete\n\tWHERE age <= 130"
    );
}
