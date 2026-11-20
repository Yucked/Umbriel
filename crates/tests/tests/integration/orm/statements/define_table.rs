//! `DEFINE TABLE` (`orm/src/statements/define_table.rs`): table kinds, `TYPE RELATION`
//! endpoints, and view models (`#[umbriel::model(view)]`).

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{
    All, Field, Model, Operatable, RecordId, Runnable, SurrealRead, Table, model,
    statements::{Returning, SelectStatement, define_table, select, table_relation},
};
use umbriel_tests::*;

async fn run(db: &Surreal<Any>, statement: impl Runnable + Sync) {
    statement.run(db).await.expect("ran");
}

mod rendering {
    use super::*;

    #[test]
    fn every_kind_renders() {
        assert_sql(
            &define_table("user").kind_any(),
            "DEFINE TABLE user TYPE ANY;",
        );
        assert_sql(
            &define_table("user").kind_normal().schemafull(),
            "DEFINE TABLE user SCHEMAFULL TYPE NORMAL;",
        );
        assert_sql(
            &define_table("likes").kind_relation(table_relation()),
            "DEFINE TABLE likes TYPE RELATION;",
        );
        assert_sql(
            &define_table("likes").kind_relation(
                table_relation()
                    .in_(["user", "team"])
                    .out(["post"])
                    .enforced(),
            ),
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
            &define_table("user")
                .comment("Acme Corp users")
                .permissions_none(),
            "DEFINE TABLE user COMMENT 'Acme Corp users' PERMISSIONS NONE;",
        );
    }

    #[test]
    fn a_changefeed_renders_with_its_expiry() {
        assert_sql(
            &define_table("reading").changefeed(std::time::Duration::from_secs(3600)),
            "DEFINE TABLE reading CHANGEFEED 1h;",
        );
        assert_sql(
            &define_table("reading")
                .schemafull()
                .changefeed(std::time::Duration::from_secs(7 * 86_400))
                .include_original()
                .comment("Acme Corp readings"),
            "DEFINE TABLE reading SCHEMAFULL CHANGEFEED 1w INCLUDE ORIGINAL \
             COMMENT 'Acme Corp readings';",
        );
        assert_render_error(
            &define_table("reading").include_original(),
            "INCLUDE ORIGINAL needs a CHANGEFEED",
        );
    }
}

mod behaviour {
    use super::*;

    #[tokio::test]
    async fn a_normal_table_refuses_relations() {
        let (db, _guard) = db().await;
        run(&db, define_table("likes").kind_normal()).await;
        assert!(!succeeds(&db, "RELATE user:alice->likes->post:hello;").await);
    }

    #[tokio::test]
    async fn an_enforced_relation_needs_both_endpoints() {
        let (db, _guard) = db().await;
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
    select(All)
        .from(Table::new("sale"))
        .where_(Field::new("amount").gte(10))
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

    async fn sales(db: &Surreal<Any>) {
        // A view's source table must exist before the view is defined (3.3.4).
        run(db, Sale::define_table()).await;
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
        let (db, _guard) = db().await;
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

    /// Why view models offer no writes (`docs/model.md`, `TableOf::create`): SurrealDB refuses
    /// them.
    #[tokio::test]
    async fn surrealdb_refuses_writes_to_a_view() {
        let (db, _guard) = db().await;
        sales(&db).await;
        assert!(!succeeds(&db, "CREATE large_sale:x SET amount = 50;").await);
    }
}

/// Definitions and their effect on records.
mod definitions {
    use umbriel_tests::{support::statements::*, *};

    #[test]
    fn table_definitions() {
        let age = Field::new("age");
        let name = Field::new("name");
        assert_sql(&define_table(Table::from("user")), "DEFINE TABLE user;");
        assert_sql(
            &define_table(Table::from("user"))
                .schemaless()
                .permissions_none(),
            "DEFINE TABLE user SCHEMALESS PERMISSIONS NONE;",
        );
        assert_sql(
            &define_table(Table::from("user"))
                .schemafull()
                .permissions_full(),
            "DEFINE TABLE user SCHEMAFULL PERMISSIONS FULL;",
        );
        assert_sql(
            &define_table(Table::from("adult"))
                .drop()
                .as_(select(All).from(Table::from("user")).where_(age.gte(18)))
                .permissions(for_permission(Select).where_(age.gte(18)))
                .permissions([
                    for_permission([Create, Delete]).where_(name.is("Alice")),
                    for_permission(Update).where_(age.lte(130)),
                ]),
            "DEFINE TABLE adult DROP AS \n\tSELECT * FROM user WHERE age >= 18\n\
             PERMISSIONS\nFOR select\n\tWHERE age >= 18\n\
             FOR create, delete\n\tWHERE name IS 'Alice'\nFOR update\n\tWHERE age <= 130;",
        );
    }

    #[tokio::test]
    async fn schemafull_tables_reject_undefined_fields() {
        let (db, _guard) = db().await;
        run(&db, define_table(Table::from("person")).schemafull()).await;
        run(
            &db,
            define_field(Field::new("name"))
                .on_table(Table::from("person"))
                .type_(Kind::String),
        )
        .await;

        run(&db, Raw::new("CREATE person:alice SET name = 'Alice';")).await;
        assert!(
            Raw::new("CREATE person:carol SET name = 'Carol', extra = 1;")
                .run(&db)
                .await
                .is_err(),
            "an undefined field is rejected"
        );
        assert!(
            Raw::new("CREATE person:bob SET name = 1;")
                .run(&db)
                .await
                .is_err(),
            "a field of the wrong type is rejected"
        );
    }
}
