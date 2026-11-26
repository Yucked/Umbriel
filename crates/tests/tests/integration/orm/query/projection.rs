//! Projections (`orm/src/query/projection.rs`): a tuple of a model's schema fields selects them as
//! one typed array value per record.

use umbriel::{
    TableOf,
    statements::{SelectStatement, order},
};
use umbriel_tests::{support::models::*, *};

#[test]
fn a_field_tuple_renders_an_array_value() {
    assert_sql(
        &Member::select_fields((Member::schema().name,)),
        "SELECT VALUE [name] FROM member;",
    );
    assert_sql(
        &Member::select_fields((Member::schema().name, Member::schema().worksFor)),
        "SELECT VALUE [name, worksFor] FROM member;",
    );
    assert_sql(
        &TableOf::<Member>::default().select_fields((Member::schema().id, Member::schema().name)),
        "SELECT VALUE [id, name] FROM member;",
    );
}

/// The result type follows from the fields, in order, at every arity up to ten.
mod compiles_select_fields {
    use super::*;

    /// The largest tuple: ten `name` fields.
    type TenNames = (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    );

    #[test]
    fn the_tuple_holds_each_fields_type_in_order() {
        let one: SelectStatement<(String,)> = Member::select_fields((Member::schema().name,));
        let two: SelectStatement<(LinkOne<Bank>, String)> =
            Member::select_fields((Member::schema().worksFor, Member::schema().name));
        let name = || Member::schema().name;
        let ten: SelectStatement<TenNames> = Member::select_fields((
            name(),
            name(),
            name(),
            name(),
            name(),
            name(),
            name(),
            name(),
            name(),
            name(),
        ));
        assert_sql(
            &ten,
            "SELECT VALUE [name, name, name, name, name, name, name, name, name, name] FROM member;",
        );
        let _ = (one, two);
    }

    #[test]
    fn clauses_keep_the_tuple_type() {
        let statement: SelectStatement<(String, LinkOne<Wallet>)> =
            Member::select_fields((Member::schema().name, Member::schema().wallet))
                .where_(Member::schema().name.equal("Alice"))
                .order_by(order(Member::schema().name).desc())
                .limit(1);
        assert_sql(
            &statement,
            "SELECT VALUE [name, wallet] FROM member WHERE name = 'Alice' ORDER BY name DESC LIMIT \
             1;",
        );
    }
}
