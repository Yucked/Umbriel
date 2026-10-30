//! `DEFINE TABLE` (`orm/src/statements/define_table.rs`): table kinds, `TYPE RELATION`
//! endpoints, the stored default kind, and view models (`#[umbriel::model(view)]`).

use surrealdb::{Surreal, engine::local::Db, types::Value};
use umbriel::{
    All, Field, Model, Operatable, RecordId, Runnable, SurrealRead, Table, model,
    statements::{SelectStatement, define_table, select, table_relation},
};
use umbriel_tests::*;

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("ran");
}

/// Runs `sql` and reports whether every statement succeeded.
async fn succeeds(db: &Surreal<Db>, sql: &str) -> bool {
    db.query(sql.to_owned())
        .await
        .and_then(surrealdb::IndexedResults::check)
        .is_ok()
}

/// The `DEFINE TABLE` statement `INFO FOR DB` lists for `table`.
async fn stored(db: &Surreal<Db>, table: &str) -> String {
    let Value::Object(info) = read::<Value>(db, "INFO FOR DB;").await else {
        panic!("INFO returns an object");
    };
    let Some(Value::Object(tables)) = info.get("tables") else {
        panic!("tables are listed");
    };
    match tables.get(table) {
        Some(Value::String(sql)) => sql.clone(),
        other => panic!("`{table}` is not defined: {other:?}"),
    }
}

mod rendering {
    use super::*;

    #[test]
    fn every_kind_renders() {
        assert_sql(&define_table("user").kind_any(), "DEFINE TABLE user TYPE ANY;");
        assert_sql(
            &define_table("user").kind_normal().schemafull(),
            "DEFINE TABLE user SCHEMAFULL TYPE NORMAL;",
        );
        assert_sql(
            &define_table("likes").kind_relation(table_relation()),
            "DEFINE TABLE likes TYPE RELATION;",
        );
        assert_sql(
            &define_table("likes")
                .kind_relation(table_relation().in_(["user", "team"]).out(["post"]).enforced()),
            "DEFINE TABLE likes TYPE RELATION IN user | team OUT post ENFORCED;",
        );
    }

    #[test]
    fn the_last_kind_wins() {
        assert_sql(
            &define_table("user").kind_any().kind_normal(),
            "DEFINE TABLE user TYPE NORMAL;",
        );
    }

    #[test]
    fn the_comment_precedes_permissions() {
        assert_sql(
            &define_table("user").comment("Acme Corp users").permissions_none(),
            "DEFINE TABLE user COMMENT 'Acme Corp users' PERMISSIONS NONE;",
        );
    }
}

mod behaviour {
    use super::*;

    #[tokio::test]
    async fn the_stored_default_kind_is_any_and_schemafull_is_normal() {
        let db = db().await;
        run(&db, define_table("user")).await;
        run(&db, define_table("account").schemafull()).await;
        assert!(stored(&db, "user").await.contains("TYPE ANY"));
        assert!(stored(&db, "account").await.contains("TYPE NORMAL"));
    }

    #[tokio::test]
    async fn a_normal_table_refuses_relations() {
        let db = db().await;
        run(&db, define_table("likes").kind_normal()).await;
        assert!(!succeeds(&db, "RELATE user:alice->likes->post:hello;").await);
    }

    #[tokio::test]
    async fn an_enforced_relation_needs_both_endpoints() {
        let db = db().await;
        run(
            &db,
            define_table("likes")
                .kind_relation(table_relation().in_(["user"]).out(["post"]).enforced()),
        )
        .await;
        assert!(!succeeds(&db, "RELATE user:alice->likes->post:hello;").await);
        assert!(succeeds(&db, "CREATE user:alice; CREATE post:hello;").await);
        assert!(succeeds(&db, "RELATE user:alice->likes->post:hello;").await);
        // The endpoints are limited to the listed tables.
        assert!(succeeds(&db, "CREATE team:acme;").await);
        assert!(!succeeds(&db, "RELATE team:acme->likes->post:hello;").await);
    }
}

#[model(node, table = "sale")]
#[derive(Debug, Clone)]
pub struct Sale {
    pub id: RecordId<Self>,
    pub region: String,
    pub amount: i64,
}

pub fn totals() -> SelectStatement {
    select(All).from(Table::new("sale")).where_(Field::new("amount").gte(10))
}

/// Sales of at least 10, kept by SurrealDB from `sale`.
#[model(view, table = "large_sale")]
#[orm(as_ = totals)]
#[derive(Debug, Clone)]
pub struct LargeSale {
    pub id: RecordId<Self>,
    pub region: String,
    pub amount: i64,
}

mod views {
    use super::*;
    use umbriel::SurrealCrud;

    async fn sales(db: &Surreal<Db>) {
        run(db, LargeSale::define_table()).await;
        for (key, region, amount) in [("a", "north", 5), ("b", "north", 20), ("c", "south", 30)] {
            Sale {
                id: Sale::id(key),
                region: region.to_owned(),
                amount,
            }
            .save(db)
            .await
            .expect("saved");
        }
    }

    #[test]
    fn a_view_defines_its_query_and_no_fields() {
        assert_eq!(
            LargeSale::define_table().as_str(),
            "DEFINE TABLE large_sale AS \n\tSELECT * FROM sale WHERE amount >= 10;"
        );
        assert!(LargeSale::define_fields().is_empty());
    }

    #[tokio::test]
    async fn a_view_model_reads_the_rows_its_query_keeps() {
        let db = db().await;
        sales(&db).await;
        let mut large = LargeSale::select().execute(&db).await.expect("selected");
        large.sort_by_key(|sale| sale.amount);
        let amounts: Vec<i64> = large.iter().map(|sale| sale.amount).collect();
        assert_eq!(amounts, vec![20, 30]);
        let count = LargeSale::count_all().execute(&db).await.expect("counted");
        assert_eq!(count, 2);
        let north = LargeSale::find_where(Field::new("region").is("north"))
            .execute(&db)
            .await
            .expect("found");
        assert_eq!(north.len(), 1);
    }

    #[tokio::test]
    async fn surrealdb_refuses_writes_to_a_view() {
        let db = db().await;
        sales(&db).await;
        assert!(!succeeds(&db, "CREATE large_sale:x SET amount = 50;").await);
    }

    #[tokio::test]
    async fn a_view_source_cannot_be_removed() {
        let db = db().await;
        sales(&db).await;
        assert!(!succeeds(&db, "REMOVE TABLE sale;").await);
    }
}
