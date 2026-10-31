//! `DEFINE FIELD` (`orm/src/statements/define_field.rs`) and the field attributes of
//! `#[umbriel::model]`: `DEFAULT ALWAYS`, `READONLY`, `FLEXIBLE`, joined `ASSERT`s, and computed
//! fields, against SurrealDB 3.3.4.

use surrealdb::{
    Surreal,
    engine::local::Db,
    types::{Kind, Object, Value},
};
use umbriel::{
    Field, Merge, Model, Operatable, Patch, RecordId, Runnable, SchemaGetter, SurrealCrud,
    SurrealCrudNode, SurrealRead, model,
    statements::{Returning, define_field, define_table},
    value,
};
use umbriel_tests::*;

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("ran");
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
        let db = db().await;
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
        let db = db().await;
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
        let db = db().await;
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
        let db = db().await;
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
    async fn an_array_length_is_exact() {
        let db = db().await;
        run(
            &db,
            define_field(Field::new("pair"))
                .on_table("sale")
                .type_(Kind::Array(Box::new(Kind::Int), Some(2))),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1 SET pair = [1, 2];").await);
        assert!(!succeeds(&db, "CREATE sale:2 SET pair = [1];").await);
        assert!(!succeeds(&db, "CREATE sale:3 SET pair = [1, 2, 3];").await);
    }

    #[tokio::test]
    async fn a_flexible_computed_object_keeps_its_keys_on_a_schemafull_table() {
        let db = db().await;
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
        let db = db().await;
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
            .run(db.clone())
            .await;
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn computed_fields_read_other_computed_fields() {
        let db = db().await;
        run(
            &db,
            define_field(Field::new("doubled"))
                .on_table("sale")
                .computed(amount().multiply(2)),
        )
        .await;
        run(
            &db,
            define_field(Field::new("quadrupled"))
                .on_table("sale")
                .computed(Field::new("doubled").multiply(2)),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1 SET amount = 3;").await);
        let quadrupled: i64 = read(&db, "RETURN sale:1.quadrupled;").await;
        assert_eq!(quadrupled, 12);
    }

    #[tokio::test]
    async fn a_decimal_default_redefines_from_its_stored_form() {
        let db = db().await;
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
        let fresh = umbriel_tests::db().await;
        assert!(succeeds(&fresh, &format!("{sql};")).await, "{sql}");
        let price: Value = read(&fresh, "CREATE ONLY sale:1 RETURN VALUE price;").await;
        assert!(matches!(price, Value::Number(_)), "{price:?}");
    }
}

/// D5's baseline: what 3.3.4 does with a computed value written in a payload, and with a computed
/// expression that writes. The model API is built on these results.
mod computed_baseline {
    use super::*;

    async fn sale_with_doubled(db: &Surreal<Db>) {
        assert!(
            succeeds(
                db,
                "DEFINE FIELD amount ON sale TYPE int;
                 DEFINE FIELD doubled ON sale COMPUTED amount * 2;"
            )
            .await
        );
    }

    #[tokio::test]
    async fn a_written_computed_value_is_discarded() {
        let db = db().await;
        sale_with_doubled(&db).await;
        assert!(succeeds(&db, "CREATE sale:1 CONTENT { amount: 2, doubled: 100 };").await);
        assert!(succeeds(&db, "UPDATE sale:1 CONTENT { amount: 3, doubled: 100 };").await);
        let doubled: i64 = read(&db, "RETURN sale:1.doubled;").await;
        assert_eq!(doubled, 6);
        // Nothing was stored for it: without the definition, the field is absent.
        assert!(succeeds(&db, "REMOVE FIELD doubled ON sale;").await);
        let stored: Option<i64> = read(&db, "RETURN sale:1.doubled;").await;
        assert_eq!(stored, None);
    }

    #[tokio::test]
    async fn a_computed_expression_that_writes_is_refused_when_defined() {
        let db = db().await;
        let error = failure(
            &db,
            "DEFINE FIELD logged ON sale COMPUTED (CREATE log CONTENT { at: time::now() });",
        )
        .await;
        // `ExecError::ComputedWrite`, from the definition-time check.
        assert!(
            error.contains("`COMPUTED` bodies must be read-only"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn a_computed_expression_calling_a_writing_function_fails_when_read() {
        let db = db().await;
        assert!(
            succeeds(
                &db,
                "DEFINE FUNCTION fn::log() { CREATE log; RETURN 1; };
                 DEFINE FIELD logged ON sale COMPUTED fn::log();
                 CREATE sale:1 RETURN NONE;"
            )
            .await
        );
        let error = failure(&db, "SELECT logged FROM sale:1;").await;
        // `ExecError::ComputedFieldSideEffect`, from the read-time frame that refuses writes.
        assert!(
            error.contains("A COMPUTED clause cannot contain a statement that modifies data"),
            "{error}"
        );
    }
}

/// Whose rights a computed field reads with: the inventory's open question (reader capped by the
/// definer, or the definer's rights). It needs a record user, so it runs on an in-memory engine
/// with authentication on.
mod computed_privileges {
    use std::collections::BTreeMap;

    use surrealdb::{
        engine::local::Mem,
        opt::{
            Config,
            auth::{Record, Root},
        },
    };

    use super::*;

    fn root() -> Root {
        Root {
            username: "root".to_owned(),
            password: "example-password".to_owned(),
        }
    }

    #[tokio::test]
    async fn a_computed_field_reads_with_the_readers_rights() {
        let db = Surreal::new::<Mem>(Config::new().user(root()))
            .await
            .expect("in-memory engine");
        db.signin(root()).await.expect("signed in as root");
        db.use_ns("test").use_db("test").await.expect("selected");
        assert!(
            succeeds(
                &db,
                "DEFINE TABLE secret SCHEMALESS PERMISSIONS NONE;
                 CREATE secret:1; CREATE secret:2;
                 DEFINE TABLE sale SCHEMALESS PERMISSIONS FULL;
                 DEFINE FIELD hidden ON sale COMPUTED count(SELECT * FROM secret);
                 CREATE sale:1;
                 DEFINE TABLE user SCHEMALESS PERMISSIONS FULL;
                 DEFINE ACCESS account ON DATABASE TYPE RECORD
                     SIGNUP (CREATE user SET name = $name)
                     SIGNIN (SELECT * FROM user WHERE name = $name)
                     DURATION FOR SESSION 1h;"
            )
            .await
        );
        // The definer, root, sees both secrets through the field.
        let as_root: usize = read(&db, "RETURN sale:1.hidden;").await;
        assert_eq!(as_root, 2);

        db.signup(Record {
            namespace: "test".to_owned(),
            database: "test".to_owned(),
            access: "account".to_owned(),
            params: BTreeMap::from([("name".to_owned(), "alice".to_owned())]),
        })
        .await
        .expect("signed up as a record user");
        // A record user, who cannot select `secret`, gets the count its own rights allow: the
        // field runs with the reader's rights, never more than the definer's.
        let as_record_user: usize = read(&db, "RETURN sale:1.hidden;").await;
        assert_eq!(as_record_user, 0);
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

    async fn schema(db: &Surreal<Db>) {
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
        let db = db().await;
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
        let db = db().await;
        schema(&db).await;
        invoice(7).create(&db).await.expect("created");
        let doubled = Invoice::select_value(Invoice::schema().doubled)
            .execute(&db)
            .await
            .expect("selected");
        assert_eq!(doubled, vec![14]);
    }
}
