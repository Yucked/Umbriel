//! Data clauses (`orm/src/statements/data.rs`): repeated calls of one clause extend it, a
//! different clause replaces it, and a statement without one writes none.

use umbriel::{
    Field, Model, Node, Patchable, Render, SchemaGetter, SetterAssignable, SetterNumeric,
    statements::{CreateTarget, create, relate, update, upsert},
};
use umbriel_tests::*;

#[test]
fn repeated_clauses_extend_and_different_ones_replace() {
    let org = Organization::schema();
    let acme = Organization::id("acme");
    assert_sql(
        &update::<Organization>(&acme)
            .set(org.age.increment_by(1u8))
            .set(org.name.equal_to("Acme Corp")),
        "UPDATE organization:acme SET age += 1, name = 'Acme Corp';",
    );
    assert_sql(
        &update::<Organization>(&acme)
            .patch(org.name.patch_remove())
            .patch(org.age.patch_replace(2u8)),
        "UPDATE organization:acme PATCH [{ op: 'remove', path: '/name' }, \
         { op: 'replace', path: '/age', value: 2 }];",
    );
    // `SET` and `UNSET` are alternatives: the later clause is the statement's.
    assert_sql(
        &update::<Organization>(&acme)
            .set(org.age.increment_by(1u8))
            .unset(Field::new("label")),
        "UPDATE organization:acme UNSET label;",
    );
    assert_sql(
        &upsert::<Organization>(&acme)
            .unset(Field::new("label"))
            .content(Organization {
                id: acme.clone(),
                ..Default::default()
            })
            .return_none(),
        &format!(
            "UPSERT organization:acme CONTENT {} RETURN NONE;",
            umbriel::ValueLike::value(Organization {
                id: acme.clone(),
                ..Default::default()
            })
            .to_debug_sql()
        ),
    );
}

#[test]
fn statements_without_a_data_clause_write_none() {
    assert_sql(
        &update::<Organization>(Organization::table()),
        "UPDATE organization;",
    );
    assert_sql(&create::<SpaceShip>(CreateTarget::count(2)), "CREATE |space_ship:2|;");
    assert_sql(
        &relate::<CompanyLikeUser>(
            Company::with(Company::id("acme"))
                .like__(umbriel::E)
                .user(User::id("alice")),
        ),
        "RELATE company:acme->like->user:alice;",
    );
}
