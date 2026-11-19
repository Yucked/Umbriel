//! Values (`orm/src/query/value_like.rs`): SDK values are bound, not written into the query, and
//! the serde bridge through `serde_json::Value`.

use umbriel::{
    Model, RecordId, Render, SurrealCrudNode, SurrealRead, model,
    statements::let_,
    types::{self, SurrealValue, Value},
};
use umbriel_tests::{support::query::*, *};

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
    let (db, _guard) = db().await;
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
    let (db, _guard) = db().await;
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

mod values {
    use super::*;

    #[test]
    fn heterogeneous_arrays_and_keywords() {
        let values = arr![
            1,
            "two",
            Field::new("three"),
            Param::new("four"),
            NONE,
            NULL,
            Duration::from_secs(60)
        ];
        assert_eq!(
            values.iter().map(Render::to_debug_sql).collect::<Vec<_>>(),
            ["1", "'two'", "three", "$four", "NONE", "NULL", "1m"]
        );
        assert_eq!(ValueLike::from(All).to_debug_sql(), "*");
        assert_eq!(ValueLike::from(E).to_debug_sql(), "");
        assert!(ValueLike::from(E).is_empty());
        assert_eq!(
            ValueLike::from(select(All).from(Table::from("user"))).to_debug_sql(),
            "(SELECT * FROM user)"
        );
        assert_eq!(
            ValueLike::from(let_("total").equal_to(1)).to_debug_sql(),
            "$total"
        );
        assert_eq!(
            umbriel::array![1i64, "two"],
            vec![1i64.into_value(), Value::String("two".to_owned())]
        );
        assert_eq!(umbriel::val(true), Value::Bool(true));
    }

    #[test]
    fn typed_expressions() {
        assert_eq!(NumberLike::from(5).to_debug_sql(), "5");
        assert_eq!(NumberLike::from(Field::new("age")).to_debug_sql(), "age");
        assert_eq!(umbriel::StrandLike::from("Alice").to_debug_sql(), "'Alice'");
        assert_eq!(umbriel::BoolLike::from(true).to_debug_sql(), "true");
        assert_eq!(
            umbriel::ArrayLike::from(vec![1, 2]).to_debug_sql(),
            "[1, 2]"
        );
        assert_eq!(
            umbriel::ArrayLike::from(vec![ValueLike::from(1), ValueLike::from(Field::new("age"))])
                .to_debug_sql(),
            "[1, age]"
        );
        assert_eq!(
            umbriel::DurationLike::from(Duration::from_secs(90)).to_debug_sql(),
            "1m30s"
        );
        assert_eq!(
            umbriel::RecordIdLike::from(User::id("alice")).to_debug_sql(),
            "user:alice"
        );
        assert_eq!(
            umbriel::StrandLike::from(Interval::Week).to_debug_sql(),
            "'week'"
        );
        assert_eq!(
            bracket(Field::new("age").add(1)).to_debug_sql(),
            "(age + 1)"
        );
    }

    #[tokio::test]
    async fn inline_values_read_back_as_the_bound_values() {
        let (db, _guard) = db().await;
        let datetime =
            chrono::DateTime::<chrono::Utc>::from_timestamp(1_700_000_000, 0).expect("valid");
        let values = vec![
            42i64.into_value(),
            1.5f64.into_value(),
            "text".into_value(),
            true.into_value(),
            Duration::from_secs(90).into_value(),
            datetime.into_value(),
            User::id("alice").into_value(),
            geo::Point::new(1.0, 2.0).into_value(),
            vec![1i64, 2, 3].into_value(),
        ];
        for value in values {
            // Debug SQL writes the literal SurrealDB parses back to the same value.
            let sql = format!("RETURN {};", ValueLike::value(value.clone()).to_debug_sql());
            let inline: Value = read(&db, &sql).await;
            assert_eq!(inline, value, "{sql}");

            // Taken as a `Value`: an `Option` would read an array result as several rows.
            let bound: Value = umbriel::statements::return_(ValueLike::value(value.clone()))
                .run(&db)
                .await
                .expect("ran")
                .take(0)
                .expect("value");
            assert_eq!(bound, value);
        }
    }
}

