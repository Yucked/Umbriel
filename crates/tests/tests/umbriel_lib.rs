//! Crate-root items (`orm/src/lib.rs`): offline SurrealQL value parsing behind the `parse`
//! feature.

use umbriel::types::Value;

#[test]
fn offline_parse() {
    let Value::Object(object) = umbriel::parse::value(
        "{ name: 'Alice', tags: ['admin', 'staff'], owner: user:alice, joined: d'2024-01-01T00:00:00Z' }",
    )
    .expect("parses") else {
        panic!("an object literal parses as an object");
    };
    assert_eq!(object.get("name"), Some(&Value::String("Alice".to_owned())));
    assert!(matches!(object.get("tags"), Some(Value::Array(_))));
    assert!(matches!(object.get("owner"), Some(Value::RecordId(_))));
    assert!(matches!(object.get("joined"), Some(Value::Datetime(_))));

    assert!(
        umbriel::parse::value("{ name: ").is_err(),
        "incomplete text is refused"
    );
}
