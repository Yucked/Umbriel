//! IF (`orm/src/statements/ifelse.rs`) inside a transaction: a branch reads a field the same
//! transaction wrote.

use umbriel::{
    Field, Model, Operatable, Runnable, SchemaGetter, SetterAssignable, ValueLike,
    statements::{begin_transaction, create, if_, update},
};
use umbriel_tests::*;

#[tokio::test]
async fn a_branch_reads_a_field_written_earlier_in_the_transaction() {
    let db = db().await;
    let org = Organization::schema();
    let acme = Organization::id("acme");
    let age = Field::from_expr(ValueLike::from(acme.clone())).with_field("age");
    begin_transaction()
        .query(create::<Organization>(acme.clone()).set([
            org.name.equal_to("Acme Corp"),
            org.age.equal_to(30u8),
        ]))
        .query(
            if_(age.gt(18))
                .then(update::<Organization>(acme.clone()).set(org.name.equal_to("Adult Corp")))
                .else_(update::<Organization>(acme).set(org.name.equal_to("Young Corp")))
                .end(),
        )
        .commit_transaction()
        .run(db.clone())
        .await
        .expect("committed");
    let name: String = read(&db, "RETURN organization:acme.name;").await;
    assert_eq!(name, "Adult Corp");
}
