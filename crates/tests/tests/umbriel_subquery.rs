//! Subqueries (`orm/src/statements/subquery.rs`) against the database: nested statements see
//! `$this` and `$parent` as SurrealDB 3.3 defines them.

use std::collections::BTreeMap;

use surrealdb::{Surreal, engine::local::Db, types::Value};
use umbriel::{
    Aliasable, Field, Operatable, Raw, Runnable, Table, arr,
    statements::{Returning, order, select, select_value},
};
use umbriel_tests::*;

async fn people() -> Surreal<Db> {
    let db = db().await;
    Raw::new(
        "CREATE person:alice SET name = 'Alice', age = 30; \
         CREATE person:bob SET name = 'Bob', age = 20; \
         RELATE person:bob->knows->person:alice; RELATE person:alice->knows->person:bob;",
    )
    .run(db.clone())
    .await
    .expect("seeded");
    db
}

#[tokio::test]
async fn a_subquery_reads_its_parent_record() {
    let db = people().await;
    let person = Table::from("person");
    let parent_age = Field::from(&*umbriel::parent()).with_field("age");
    let older = select_value(Field::new("name"))
        .from(person.clone())
        .where_(Field::new("age").gt(parent_age));
    let rows: Vec<BTreeMap<String, Value>> = select(arr![Field::new("name"), older.__as__("older")])
        .from(person)
        .order_by(order(Field::new("name")))
        .decode()
        .execute(&db)
        .await
        .expect("read");
    let none: Value = read(&db, "RETURN [];").await;
    let alice: Value = read(&db, "RETURN ['Alice'];").await;
    assert_eq!(rows[0].get("older"), Some(&none));
    assert_eq!(rows[1].get("older"), Some(&alice));
}

#[tokio::test]
async fn this_is_the_current_record() {
    let db = people().await;
    let names: Vec<String> = select_value(Field::from(&*umbriel::this()).with_field("name"))
        .from(Table::from("person"))
        .order_by(order(Field::new("name")))
        .decode()
        .execute(&db)
        .await
        .expect("read");
    assert_eq!(names, ["Alice", "Bob"]);
}

/// Since 3.0.5/3.1.0, `$parent` inside a graph traversal's `WHERE` is the outer record, not
/// `NONE`. The graph filter's builder belongs to #14, so the query is written out.
#[tokio::test]
async fn parent_inside_a_graph_filter_is_the_outer_record() {
    let db = people().await;
    let older: Vec<Vec<String>> = read(
        &db,
        "SELECT VALUE ->knows->(person WHERE age > $parent.age).name FROM person ORDER BY name;",
    )
    .await;
    assert_eq!(older, [vec![], vec!["Alice".to_owned()]]);
}

/// Since 3.3.0-beta.4, an inline `SELECT` keeps its array shape whatever its row count.
#[tokio::test]
async fn an_inline_select_stays_an_array() {
    let db = people().await;
    let one = select_value(Field::new("name"))
        .from(Table::from("person"))
        .where_(Field::new("age").gt(25));
    let mut results = umbriel::statements::return_(one)
        .run(db.clone())
        .await
        .expect("ran");
    let names: Vec<String> = results.take(0).expect("an array");
    assert_eq!(names, ["Alice"]);
}
