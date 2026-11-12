//! Values (`orm/src/query/value_like.rs`): SDK values are bound, not written into the query, and
//! the serde bridge through `serde_json::Value`.

use umbriel::{
    Model, RecordId, Render, SurrealCrudNode, SurrealRead, model,
    statements::let_,
    types::{self, SurrealValue, Value},
};
use umbriel_tests::*;

/// A node holding a JSON document.
#[model(node, table = "document")]
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub id: RecordId<Self>,
    #[orm(ty = "object")]
    pub body: serde_json::Value,
}

#[tokio::test]
async fn a_table_value_binds_as_a_parameter() {
    let db = db().await;
    assert!(succeeds(&db, "CREATE user:alice SET name = 'Alice';").await);

    let source = let_("source").equal_to(types::Table::new("user"));
    let (sql, vars) = source.to_query().expect("renders").into_parts();
    assert!(
        !sql.contains("user"),
        "the table is bound, not written: {sql}"
    );

    let ids: Vec<types::RecordId> = db
        .query(format!("{sql} SELECT VALUE id FROM $source;"))
        .bind(vars)
        .await
        .expect("runs")
        .take(1)
        .expect("ids");
    assert_eq!(ids, [types::RecordId::new("user", "alice")]);
}

#[tokio::test]
async fn values_cross_serde() {
    let db = db().await;
    apply_definitions::<Document>(&db).await;
    let body = serde_json::json!({
        "name": "Acme Corp",
        "employees": 12,
        "rating": 4.5,
        "public": true,
        "parent": null,
        "tags": ["tools", "hardware"],
        "address": { "city": "Springfield" },
    });
    let document = Document {
        id: Document::id("acme"),
        body,
    };
    document.clone().create(&db).await.expect("created");
    assert_eq!(
        Document::get(&db, &document.id).await.expect("read"),
        Some(document)
    );

    // JSON has no set, decimal, table, duration or NONE: each becomes the nearest JSON value, and
    // converting back does not restore the SurrealQL kind.
    let set = Value::Set(types::Set::from(vec![
        1_i64.into_value(),
        2_i64.into_value(),
    ]));
    let json = serde_json::Value::from_value(set).expect("converts");
    assert_eq!(json, serde_json::json!([1, 2]));
    assert!(matches!(json.into_value(), Value::Array(_)));

    let lossy = [
        (
            Value::Number(types::Number::Decimal(types::Decimal::new(125, 2))),
            serde_json::json!("1.25"),
        ),
        (
            Value::Table(types::Table::new("user")),
            serde_json::json!("user"),
        ),
        (
            std::time::Duration::from_secs(90).into_value(),
            serde_json::json!("1m30s"),
        ),
        (Value::None, serde_json::Value::Null),
    ];
    for (value, expected) in lossy {
        assert_eq!(
            serde_json::Value::from_value(value).expect("converts"),
            expected
        );
    }
}
