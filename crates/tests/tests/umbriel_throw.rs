//! THROW (`orm/src/statements/throw.rs`): any value can be thrown, and it fails the query.

use umbriel::{
    All, Error, Param, Runnable, Table,
    statements::{select, throw},
    types::{Object, Value},
};
use umbriel_tests::*;

#[test]
fn any_value_can_be_thrown() {
    assert_sql(&throw("insufficient funds"), "THROW 'insufficient funds';");
    assert_sql(&throw(Param::new("reason")), "THROW $reason;");
    assert_sql(&throw(42), "THROW 42;");
    assert_sql(
        &throw(select(All).from(Table::from("event"))),
        "THROW (SELECT * FROM event);",
    );
}

#[tokio::test]
async fn a_thrown_object_fails_the_query_with_its_text() {
    let db = db().await;
    let mut error = Object::new();
    error.insert("code".to_owned(), Value::String("E42".to_owned()));
    let failed = throw(Value::Object(error)).run(db.clone()).await;
    match failed {
        Err(Error::Statement { source, .. }) => {
            assert!(source.to_string().contains("E42"), "{source}");
        }
        other => panic!("expected a statement error, got {:?}", other.err()),
    }
}
