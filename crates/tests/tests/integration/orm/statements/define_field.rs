//! `DEFINE FIELD` (`orm/src/statements/define_field.rs`) and the field attributes of
//! `#[umbriel::model]`: `DEFAULT ALWAYS`, `READONLY`, `FLEXIBLE`, joined `ASSERT`s, and computed
//! fields, against SurrealDB 3.3.4.

use surrealdb::{
    Surreal,
    engine::any::Any,
    types::{Kind, Object, Value},
};
use umbriel::{
    Field, Merge, Model, Operatable, Patch, RecordId, Runnable, SchemaGetter, SurrealCrud,
    SurrealCrudNode, SurrealRead, model,
    statements::{Returning, define_field, define_table},
    value,
};
use umbriel_tests::*;

async fn run(db: &Surreal<Any>, statement: impl Runnable + Sync) {
    statement.run(db).await.expect("ran");
}

fn amount() -> Field {
    Field::new("amount")
}

/// `{ RETURN { total: amount } }`: a computed object whose key has no definition of its own.
fn summary() -> umbriel::statements::utils::Block {
    umbriel::chain(umbriel::Raw::new("RETURN { total: amount };")).as_block()
}

mod rendering {
    use super::*;

    #[test]
    fn the_new_clauses_render_where_surrealdb_reads_them() {
        assert_sql(
            &define_field(Field::new("refunds"))
                .on_table("sale")
                .type_(Kind::Int)
                .default_always(0),
            "DEFINE FIELD refunds ON TABLE sale TYPE int DEFAULT ALWAYS 0;",
        );
        assert_sql(
            &define_field(Field::new("created_by"))
                .on_table("sale")
                .type_(Kind::String)
                .readonly(),
            "DEFINE FIELD created_by ON TABLE sale TYPE string READONLY;",
        );
        assert_sql(
            &define_field(Field::new("meta"))
                .on_table("sale")
                .type_(Kind::Object)
                .flexible()
                .readonly(),
            "DEFINE FIELD meta ON TABLE sale TYPE object FLEXIBLE READONLY;",
        );
        assert_sql(
            &define_field(Field::new("tags"))
                .on_table("sale")
                .type_(Kind::Array(Box::new(Kind::Object), None))
                .flexible(),
            "DEFINE FIELD tags ON TABLE sale TYPE array<object> FLEXIBLE;",
        );
    }

    #[test]
    fn repeated_assertions_are_joined() {
        let statement = define_field(amount())
            .on_table("sale")
            .assert(value().gte(0))
            .assert(value().lte(100));
        assert_sql(
            &statement,
            "DEFINE FIELD amount ON TABLE sale ASSERT ($value >= 0) AND ($value <= 100);",
        );
    }

    #[test]
    fn a_later_default_replaces_an_earlier_one() {
        assert_sql(
            &define_field(amount())
                .on_table("sale")
                .default(1)
                .default_always(2),
            "DEFINE FIELD amount ON TABLE sale DEFAULT ALWAYS 2;",
        );
        assert_sql(
            &define_field(amount())
                .on_table("sale")
                .default_always(2)
                .default(1),
            "DEFINE FIELD amount ON TABLE sale DEFAULT 1;",
        );
    }

    #[test]
    fn clauses_surrealdb_refuses_fail_to_render() {
        assert_render_error(
            &define_field(Field::new("id"))
                .on_table("sale")
                .default_always(1),
            "DEFAULT ALWAYS",
        );
        assert_render_error(
            &define_field(Field::new("name"))
                .on_table("sale")
                .type_(Kind::String)
                .flexible(),
            "FLEXIBLE",
        );
        assert_render_error(
            &define_field(Field::new("meta")).on_table("sale").flexible(),
            "FLEXIBLE",
        );
        assert_render_error(
            &define_field(Field::new("doubled"))
                .on_table("sale")
                .computed(amount())
                .readonly(),
            "cannot also be READONLY",
        );
        assert_render_error(
            &define_field(Field::new("doubled"))
                .on_table("sale")
                .computed(amount())
                .default_always(0),
            "DEFAULT, VALUE or ASSERT",
        );
    }

    #[test]
    fn a_computed_field_may_be_flexible() {
        assert_sql(
            &define_field(Field::new("summary"))
                .on_table("sale")
                .computed(summary())
                .type_(Kind::Object)
                .flexible(),
            "DEFINE FIELD summary ON TABLE sale COMPUTED {\nRETURN { total: amount };\n} \
             TYPE object FLEXIBLE;",
        );
    }

    #[test]
    fn a_computed_field_named_by_a_parameter_is_checked_by_surrealdb() {
        let name = umbriel::Param::new("name");
        assert_sql(
            &define_field(Field::from(&name))
                .on_table("sale")
                .computed(amount().multiply(2)),
            "DEFINE FIELD $name ON TABLE sale COMPUTED amount * 2;",
        );
        // The checks that need no name still apply.
        assert_render_error(
            &define_field(Field::from(&name))
                .on_table("sale")
                .computed(amount())
                .readonly(),
            "cannot also be READONLY",
        );
    }

    #[test]
    fn options_and_a_parameter_table() {
        assert_sql(
            &define_field(amount())
                .on_table(umbriel::Param::new("table"))
                .type_(Kind::Int)
                .comment("in cents")
                .overwrite(),
            "DEFINE FIELD OVERWRITE amount ON TABLE $table TYPE int COMMENT 'in cents';",
        );
    }
}

mod behaviour {
    use super::*;

    #[tokio::test]
    async fn default_always_refills_a_field_left_none() {
        let (db, _guard) = db().await;
        run(
            &db,
            define_field(Field::new("refunds"))
                .on_table("sale")
                .type_(Kind::Int)
                .default_always(0),
        )
        .await;
        run(
            &db,
            define_field(Field::new("discount"))
                .on_table("sale")
                .type_(Kind::option(Kind::Int))
                .default(0),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1 SET refunds = 5, discount = 5;").await);
        assert!(succeeds(&db, "UPDATE sale:1 SET refunds = NONE, discount = NONE;").await);
        let refunds: i64 = read(&db, "RETURN sale:1.refunds;").await;
        assert_eq!(refunds, 0);
        let discount: Option<i64> = read(&db, "RETURN sale:1.discount;").await;
        assert_eq!(discount, None);
    }

    #[tokio::test]
    async fn readonly_refuses_a_change_and_accepts_the_same_value() {
        let (db, _guard) = db().await;
        run(
            &db,
            define_field(Field::new("created_by"))
                .on_table("sale")
                .type_(Kind::String)
                .readonly(),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1 SET created_by = 'Alice';").await);
        assert!(succeeds(&db, "UPDATE sale:1 SET created_by = 'Alice';").await);
        assert!(!succeeds(&db, "UPDATE sale:1 SET created_by = 'Bob';").await);
        let creator: String = read(&db, "RETURN sale:1.created_by;").await;
        assert_eq!(creator, "Alice");
    }

    #[tokio::test]
    async fn flexible_keeps_undefined_keys_on_a_schemafull_table() {
        let (db, _guard) = db().await;
        run(&db, define_table("sale").schemafull()).await;
        run(
            &db,
            define_field(Field::new("meta"))
                .on_table("sale")
                .type_(Kind::Object)
                .flexible(),
        )
        .await;
        run(
            &db,
            define_field(Field::new("plain"))
                .on_table("sale")
                .type_(Kind::Object),
        )
        .await;
        assert!(
            succeeds(
                &db,
                "CREATE sale:1 SET meta = { source: 'web' }, plain = {};"
            )
            .await
        );
        let meta: Object = read(&db, "RETURN sale:1.meta;").await;
        assert_eq!(meta.get("source"), Some(&Value::String("web".to_owned())));
        // Without `FLEXIBLE`, 3.3.4 refuses the undefined key instead of dropping it.
        assert!(
            !succeeds(
                &db,
                "CREATE sale:2 SET meta = {}, plain = { source: 'web' };"
            )
            .await
        );
    }

    #[tokio::test]
    async fn every_joined_assertion_applies() {
        let (db, _guard) = db().await;
        run(
            &db,
            define_field(amount())
                .on_table("sale")
                .type_(Kind::Int)
                .assert(value().gte(0))
                .assert(value().lte(100)),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1 SET amount = 50;").await);
        assert!(!succeeds(&db, "CREATE sale:2 SET amount = -1;").await);
        assert!(!succeeds(&db, "CREATE sale:3 SET amount = 101;").await);
    }

    #[tokio::test]
    async fn a_flexible_computed_object_keeps_its_keys_on_a_schemafull_table() {
        let (db, _guard) = db().await;
        run(&db, define_table("sale").schemafull()).await;
        run(
            &db,
            define_field(amount()).on_table("sale").type_(Kind::Int),
        )
        .await;
        run(
            &db,
            define_field(Field::new("summary"))
                .on_table("sale")
                .computed(summary())
                .type_(Kind::Object)
                .flexible(),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1 SET amount = 3;").await);
        let total: i64 = read(&db, "RETURN sale:1.summary.total;").await;
        assert_eq!(total, 3);
    }

    #[tokio::test]
    async fn a_computed_field_takes_its_name_from_a_parameter() {
        let (db, _guard) = db().await;
        let name = umbriel::Param::new("name");
        run(
            &db,
            umbriel::chain(umbriel::statements::let_("name").equal_to("doubled")).chain(
                define_field(Field::from(&name))
                    .on_table("sale")
                    .computed(amount().multiply(2)),
            ),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1 SET amount = 4;").await);
        let doubled: i64 = read(&db, "RETURN sale:1.doubled;").await;
        assert_eq!(doubled, 8);

        // The name is checked when SurrealDB resolves it: `id` is still refused.
        let refused = umbriel::chain(umbriel::statements::let_("name").equal_to("id"))
            .chain(
                define_field(Field::from(&name))
                    .on_table("sale")
                    .computed(amount()),
            )
            .run(&db)
            .await;
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn a_decimal_default_redefines_from_its_stored_form() {
        let (db, _guard) = db().await;
        run(
            &db,
            define_field(Field::new("price"))
                .on_table("sale")
                .type_(Kind::Decimal)
                // A float literal (`1.5f`), the inventory's case for decimal defaults.
                .default(1.5),
        )
        .await;
        let stored: Value = read(&db, "INFO FOR TABLE sale;").await;
        let Value::Object(info) = stored else {
            panic!("INFO returns an object");
        };
        let Some(Value::Object(fields)) = info.get("fields") else {
            panic!("fields are listed");
        };
        let Some(Value::String(sql)) = fields.get("price") else {
            panic!("price is defined");
        };
        // The stored statement defines the same field again.
        let (fresh, _fresh_guard) = umbriel_tests::db().await;
        assert!(succeeds(&fresh, &format!("{sql};")).await, "{sql}");
        let price: Value = read(&fresh, "CREATE ONLY sale:1 RETURN VALUE price;").await;
        assert!(matches!(price, Value::Number(_)), "{price:?}");
    }
}

/// The server behaviour the model API's computed fields are built on, pinned so an upgrade that
/// changes it fails here.
mod computed_contract {
    use super::*;

    /// Why a computed field has no patch, merge or setter and `object!` doesn't take it
    /// (`docs/model.md`): SurrealDB discards a value written to it.
    #[tokio::test]
    async fn a_written_computed_value_is_discarded() {
        let (db, _guard) = db().await;
        assert!(
            succeeds(
                &db,
                "DEFINE FIELD amount ON sale TYPE int;
                 DEFINE FIELD doubled ON sale COMPUTED amount * 2;"
            )
            .await
        );
        assert!(succeeds(&db, "CREATE sale:1 CONTENT { amount: 2, doubled: 100 };").await);
        assert!(succeeds(&db, "UPDATE sale:1 CONTENT { amount: 3, doubled: 100 };").await);
        let doubled: i64 = read(&db, "RETURN sale:1.doubled;").await;
        assert_eq!(doubled, 6);
        // Nothing was stored for it: without the definition, the field is absent.
        assert!(succeeds(&db, "REMOVE FIELD doubled ON sale;").await);
        let stored: Option<i64> = read(&db, "RETURN sale:1.doubled;").await;
        assert_eq!(stored, None);
    }
}

#[model(node, table = "invoice")]
#[derive(Debug, Clone, Default)]
pub struct Invoice {
    pub id: RecordId<Self>,
    pub amount: i64,
    #[orm(computed = Field::new("amount").multiply(2))]
    pub doubled: i64,
}

/// A model's computed field is read, and is in none of its write APIs.
mod computed_models {
    use super::*;

    async fn schema(db: &Surreal<Any>) {
        run(db, Invoice::define_table()).await;
        for field in Invoice::define_fields() {
            run(db, field).await;
        }
    }

    fn invoice(amount: i64) -> Invoice {
        Invoice {
            id: Invoice::id("acme"),
            amount,
            // Ignored: SurrealDB discards a written computed value.
            doubled: 1000,
        }
    }

    #[test]
    fn the_definition_is_computed() {
        let fields = Invoice::define_fields();
        let doubled = fields
            .iter()
            .map(|definition| definition.as_str().to_owned())
            .find(|sql| sql.contains("doubled"))
            .expect("doubled is defined");
        assert_eq!(
            doubled,
            "DEFINE FIELD doubled ON TABLE invoice COMPUTED amount * 2 TYPE int;"
        );
    }

    #[tokio::test]
    async fn every_write_reads_back_the_computed_value() {
        let (db, _guard) = db().await;
        schema(&db).await;

        let created = invoice(2).create(&db).await.expect("created");
        assert_eq!(created.doubled, 4);

        let saved = invoice(3).save(&db).await.expect("saved");
        assert_eq!(saved.doubled, 6);

        let patched = Invoice::patch(
            &db,
            &Invoice::id("acme"),
            InvoicePatch {
                amount: Patch::Set(4),
                ..Default::default()
            },
        )
        .await
        .expect("patched")
        .expect("found");
        assert_eq!(patched.doubled, 8);

        let merged = Invoice::merge(
            &db,
            &Invoice::id("acme"),
            InvoiceMerge {
                amount: Merge::Set(5),
                ..Default::default()
            },
        )
        .await
        .expect("merged")
        .expect("found");
        assert_eq!(merged.doubled, 10);

        let read = Invoice::get(&db, &Invoice::id("acme"))
            .await
            .expect("read")
            .expect("found");
        assert_eq!(read.doubled, 10);
        assert_eq!(read.amount, 5);
    }

    #[tokio::test]
    async fn a_computed_field_selects_by_its_type() {
        let (db, _guard) = db().await;
        schema(&db).await;
        invoice(7).create(&db).await.expect("created");
        let doubled = Invoice::select_value(Invoice::schema().doubled)
            .execute(&db)
            .await
            .expect("selected");
        assert_eq!(doubled, vec![14]);
    }
}

/// Definitions and their effect on records.
mod definitions {
    use umbriel_tests::{support::statements::*, *};

    #[test]
    fn field_definitions() {
        let name = Field::new("name");
        let age = Field::new("age");
        assert_sql(
            &define_field(Field::new("email"))
                .on_table(Table::from("user"))
                .type_(Kind::String)
                .default("alice@example.com")
                .assert(cond(value().is_not(NONE)).and(value().contains("@")))
                .permissions(for_permission(Select).where_(age.gte(18)))
                .permissions(for_permission([Create, Update]).where_(name.is("Alice"))),
            "DEFINE FIELD email ON TABLE user TYPE string DEFAULT 'alice@example.com' \
             ASSERT ($value IS NOT NONE) AND ($value CONTAINS '@')\n\
             PERMISSIONS\nFOR select\n\tWHERE age >= 18\nFOR create, update\n\tWHERE name IS 'Alice';",
        );
        assert_sql(
            &define_field(Field::new("email"))
                .on_table(Table::from("user"))
                .value(string::lowercase!(Param::new("value"))),
            "DEFINE FIELD email ON TABLE user VALUE string::lowercase($value);",
        );
        assert_sql(
            &define_field(Field::new("is_adult"))
                .on_table(Table::from("user"))
                .computed(age.gte(18))
                .type_(Kind::Bool)
                .permissions_none(),
            "DEFINE FIELD is_adult ON TABLE user COMPUTED age >= 18 TYPE bool PERMISSIONS NONE;",
        );
        // Definitions are stored, so their values are written inline even when bound.
        let query = define_field(Field::new("score"))
            .on_table(Table::from("user"))
            .default(0)
            .to_query()
            .expect("valid");
        assert_eq!(query.sql(), "DEFINE FIELD score ON TABLE user DEFAULT 0;");
        assert!(query.vars().is_empty());
    }

    #[test]
    fn computed_fields_reject_other_clauses_nested_paths_and_id() {
        let age = Field::new("age");
        let computed = |field: Field| {
            define_field(field)
                .on_table(Table::from("person"))
                .computed(age.gte(18))
        };
        assert!(computed(Field::new("is_adult")).to_query().is_ok());
        assert!(computed(Field::new("is-adult")).to_query().is_ok());
        assert_render_error(
            &computed(Field::new("is_adult")).default(false),
            "cannot also have",
        );
        assert_render_error(
            &computed(Field::new("is_adult")).value(false),
            "cannot also have",
        );
        assert_render_error(
            &computed(Field::new("is_adult")).assert(value().is_not(NONE)),
            "cannot also have",
        );
        assert_render_error(
            &computed(Field::new("flags").with_field("is_adult")),
            "top-level field",
        );
        assert_render_error(&computed(Field::raw("flags.is_adult")), "top-level field");
        assert_render_error(&computed(Field::new("id")), "`id` field");
    }

    async fn define_score_with_default(db: &Surreal<Any>) {
        run(db, define_table(Table::from("player")).schemaless()).await;
        run(
            db,
            define_field(Field::new("score"))
                .on_table(Table::from("player"))
                .type_(Kind::Int)
                .default(10),
        )
        .await;
    }

    #[tokio::test]
    async fn default_fills_a_missing_field_on_create() {
        let (db, _guard) = db().await;
        define_score_with_default(&db).await;

        run(&db, Raw::new("CREATE player:alice;")).await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:alice;").await;
        assert_eq!(score, Some(10));
    }

    #[tokio::test]
    async fn default_keeps_falsy_values() {
        let (db, _guard) = db().await;
        define_score_with_default(&db).await;
        run(
            &db,
            define_field(Field::new("active"))
                .on_table(Table::from("player"))
                .type_(Kind::Bool)
                .default(true),
        )
        .await;
        run(
            &db,
            define_field(Field::new("nickname"))
                .on_table(Table::from("player"))
                .type_(Kind::String)
                .default("Al"),
        )
        .await;

        run(
            &db,
            Raw::new("CREATE player:bob SET score = 0, active = false, nickname = '';"),
        )
        .await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:bob;").await;
        let active: Option<bool> = read(&db, "SELECT VALUE active FROM ONLY player:bob;").await;
        let nickname: Option<String> =
            read(&db, "SELECT VALUE nickname FROM ONLY player:bob;").await;
        assert_eq!(score, Some(0));
        assert_eq!(active, Some(false));
        assert_eq!(nickname.as_deref(), Some(""));
    }

    #[tokio::test]
    async fn default_is_not_reapplied_on_update() {
        let (db, _guard) = db().await;
        define_score_with_default(&db).await;

        run(&db, Raw::new("CREATE player:carol SET score = 5;")).await;
        run(&db, Raw::new("UPDATE player:carol SET level = 2;")).await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:carol;").await;
        assert_eq!(score, Some(5));

        // Without `ALWAYS`, removing the field does not bring the default back, so the `int`
        // field rejects the NONE left behind.
        let error = Raw::new("UPDATE player:carol UNSET score;")
            .run(&db)
            .await
            .expect_err("NONE is not an int");
        let error = format!("{error:?}");
        assert!(error.contains("Expected `int` but found `NONE`"), "{error}");
    }

    #[tokio::test]
    async fn value_transforms_on_create_and_update() {
        let (db, _guard) = db().await;
        run(&db, define_table(Table::from("player")).schemaless()).await;
        run(
            &db,
            define_field(Field::new("name"))
                .on_table(Table::from("player"))
                .type_(Kind::String)
                .value(string::lowercase!(Param::new("value"))),
        )
        .await;

        run(&db, Raw::new("CREATE player:dave SET name = 'ALICE';")).await;
        let name: Option<String> = read(&db, "SELECT VALUE name FROM ONLY player:dave;").await;
        assert_eq!(name.as_deref(), Some("alice"));

        run(&db, Raw::new("UPDATE player:dave SET name = 'BOB';")).await;
        let name: Option<String> = read(&db, "SELECT VALUE name FROM ONLY player:dave;").await;
        assert_eq!(name.as_deref(), Some("bob"));
    }

    #[tokio::test]
    async fn computed_fields_are_evaluated_on_read() {
        let (db, _guard) = db().await;
        run(&db, define_table(Table::from("person")).schemaless()).await;
        run(
            &db,
            define_field(Field::new("is_adult"))
                .on_table(Table::from("person"))
                .computed(Field::new("age").gte(18))
                .type_(Kind::Bool),
        )
        .await;

        run(&db, Raw::new("CREATE person:erin SET age = 20;")).await;
        let adult: Option<bool> = read(&db, "SELECT VALUE is_adult FROM ONLY person:erin;").await;
        assert_eq!(adult, Some(true));

        run(&db, Raw::new("UPDATE person:erin SET age = 10;")).await;
        let adult: Option<bool> = read(&db, "SELECT VALUE is_adult FROM ONLY person:erin;").await;
        assert_eq!(adult, Some(false));
    }
}
