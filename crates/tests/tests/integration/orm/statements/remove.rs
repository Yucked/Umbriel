//! `REMOVE` statements (`orm/src/statements/remove.rs`): `IF EXISTS` on every kind, and names read
//! from parameters.

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{
    Field, Param, Runnable,
    statements::{
        ModuleName, remove_access, remove_analyzer, remove_api, remove_bucket, remove_config_api,
        remove_database, remove_event, remove_field, remove_function, remove_index, remove_module,
        remove_namespace, remove_param, remove_sequence, remove_table, remove_user,
    },
};
use umbriel_tests::*;

#[test]
fn every_removal_takes_if_exists() {
    assert_sql(
        &remove_namespace("acme").if_exists(),
        "REMOVE NAMESPACE IF EXISTS acme;",
    );
    assert_sql(
        &remove_database("acme").if_exists(),
        "REMOVE DATABASE IF EXISTS acme;",
    );
    assert_sql(
        &remove_table("user").if_exists(),
        "REMOVE TABLE IF EXISTS user;",
    );
    assert_sql(
        &remove_field(Field::new("email"))
            .on_table("user")
            .if_exists(),
        "REMOVE FIELD IF EXISTS email ON TABLE user;",
    );
    assert_sql(
        &remove_index("user_email").on_table("user").if_exists(),
        "REMOVE INDEX IF EXISTS user_email ON TABLE user;",
    );
    assert_sql(
        &remove_event("audit").on_table("user").if_exists(),
        "REMOVE EVENT IF EXISTS audit ON TABLE user;",
    );
    assert_sql(
        &remove_analyzer("simple").if_exists(),
        "REMOVE ANALYZER IF EXISTS simple;",
    );
    assert_sql(
        &remove_param("greeting").if_exists(),
        "REMOVE PARAM IF EXISTS $greeting;",
    );
    assert_sql(
        &remove_function("greet").if_exists(),
        "REMOVE FUNCTION IF EXISTS fn::greet;",
    );
    assert_sql(
        &remove_user("alice").on_database().if_exists(),
        "REMOVE USER IF EXISTS alice ON DATABASE;",
    );
    assert_sql(
        &remove_access("api").on_database().if_exists(),
        "REMOVE ACCESS IF EXISTS api ON DATABASE;",
    );
    assert_sql(
        &remove_sequence("order_number").if_exists(),
        "REMOVE SEQUENCE IF EXISTS order_number;",
    );
    assert_sql(
        &remove_bucket("avatars").if_exists(),
        "REMOVE BUCKET IF EXISTS avatars;",
    );
    assert_sql(
        &remove_api("/status").if_exists(),
        "REMOVE API IF EXISTS '/status';",
    );
    assert_sql(
        &remove_module(ModuleName::Module("geo".to_owned())).if_exists(),
        "REMOVE MODULE IF EXISTS mod::geo;",
    );
    assert_sql(
        &remove_config_api().if_exists(),
        "REMOVE CONFIG IF EXISTS API;",
    );
}

#[test]
fn names_come_from_parameters() {
    let name = Param::new("name");
    let table = Param::new("table");
    assert_sql(&remove_namespace(&name), "REMOVE NAMESPACE $name;");
    assert_sql(&remove_database(&name), "REMOVE DATABASE $name;");
    assert_sql(&remove_table(&table), "REMOVE TABLE $table;");
    assert_sql(
        &remove_field(Field::new("email")).on_table(&table),
        "REMOVE FIELD email ON TABLE $table;",
    );
    assert_sql(
        &remove_index(&name).on_table(&table),
        "REMOVE INDEX $name ON TABLE $table;",
    );
    assert_sql(
        &remove_event(&name).on_table(&table),
        "REMOVE EVENT $name ON TABLE $table;",
    );
    assert_sql(&remove_analyzer(&name), "REMOVE ANALYZER $name;");
    assert_sql(
        &remove_user(&name).on_database(),
        "REMOVE USER $name ON DATABASE;",
    );
    assert_sql(
        &remove_access(&name).on_database(),
        "REMOVE ACCESS $name ON DATABASE;",
    );
}

/// Runs `statement` and reports whether it succeeded.
async fn runs(db: &Surreal<Any>, statement: impl Runnable + Sync) -> bool {
    statement.run(db).await.is_ok()
}

#[tokio::test]
async fn if_exists_removes_nothing_without_failing() {
    let (db, guard) = db().await;
    db.query("DEFINE TABLE user;")
        .await
        .and_then(surrealdb::IndexedResults::check)
        .expect("table defined");

    // Unique names: on a shared server a fixed one could name something real.
    let missing = format!("{}_missing", guard.database());
    assert!(runs(&db, remove_namespace(missing.as_str()).if_exists()).await);
    assert!(runs(&db, remove_database(missing.as_str()).if_exists()).await);
    assert!(runs(&db, remove_table("missing").if_exists()).await);
    assert!(
        runs(
            &db,
            remove_field(Field::new("email"))
                .on_table("user")
                .if_exists()
        )
        .await
    );
    assert!(runs(&db, remove_index("user_email").on_table("user").if_exists()).await);
    assert!(runs(&db, remove_event("audit").on_table("user").if_exists()).await);
    assert!(runs(&db, remove_analyzer("simple").if_exists()).await);
    assert!(runs(&db, remove_param("greeting").if_exists()).await);
    assert!(runs(&db, remove_function("greet").if_exists()).await);
    assert!(runs(&db, remove_user("alice").on_database().if_exists()).await);
    assert!(runs(&db, remove_access("api").on_database().if_exists()).await);
    assert!(runs(&db, remove_sequence("order_number").if_exists()).await);
    assert!(runs(&db, remove_api("/status").if_exists()).await);
    assert!(runs(&db, remove_config_api().if_exists()).await);
}

#[tokio::test]
async fn without_if_exists_a_missing_resource_fails() {
    let (db, _guard) = db().await;
    assert!(!runs(&db, remove_table("missing")).await);
    assert!(!runs(&db, remove_param("greeting")).await);
    assert!(!runs(&db, remove_analyzer("simple")).await);
}

/// Every kind of definition is accepted and removed again.
mod definitions {
    use umbriel_tests::{support::statements::*, *};

    #[test]
    fn users_and_accesses_are_removed_from_their_level() {
        for (statement, sql) in [
            (remove_user("alice").on_root(), "REMOVE USER alice ON ROOT;"),
            (
                remove_user("alice").on_namespace(),
                "REMOVE USER alice ON NAMESPACE;",
            ),
            (
                remove_user("alice").on_database(),
                "REMOVE USER alice ON DATABASE;",
            ),
        ] {
            assert_sql(&statement, sql);
        }
        for (statement, sql) in [
            (
                remove_access("issuer").on_root(),
                "REMOVE ACCESS issuer ON ROOT;",
            ),
            (
                remove_access("api").on_namespace(),
                "REMOVE ACCESS api ON NAMESPACE;",
            ),
            (
                remove_access("account").on_database(),
                "REMOVE ACCESS account ON DATABASE;",
            ),
        ] {
            assert_sql(&statement, sql);
        }
        assert_render_error(&remove_access("account"), "needs a level");
        assert_render_error(&remove_param("_p0"), "reserved");
    }

    #[tokio::test]
    async fn every_definition_is_accepted_and_removable() {
        let (db, guard) = db().await;
        // Unique names: a namespace is server-wide, and a fixed name could be another test's or
        // something real.
        let defined = format!("{}_defined", guard.database());
        let user = Table::from("user");
        run(&db, define_table(user.clone()).schemaless()).await;
        run(
            &db,
            define_field(Field::new("email"))
                .on_table(user.clone())
                .type_(Kind::String),
        )
        .await;
        run(
            &db,
            define_index("user_email")
                .on_table(user.clone())
                .fields(Field::new("email"))
                .unique(),
        )
        .await;
        run(
            &db,
            define_event("audit")
                .on_table(user.clone())
                .when(Param::new("event").equal("CREATE"))
                .then(select(All).from(user.clone())),
        )
        .await;
        run(
            &db,
            define_analyzer("simple").tokenizers([Tokenizer::Blank]),
        )
        .await;
        run(
            &db,
            define_access("account").on_database().record(
                record_access()
                    .signup(Raw::new("CREATE user SET email = $email"))
                    .signin(Raw::new("SELECT * FROM user WHERE email = $email"))
                    .with_jwt(JwtAlgorithm::HS512, "example-key")
                    .duration_for_session(Duration::from_secs(3600)),
            ),
        )
        .await;
        run(
            &db,
            define_access("api").on_database().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::HS256)
                    .key("example-key"),
            ),
        )
        .await;
        run(
            &db,
            define_user("alice")
                .on_database()
                .password("example-password")
                .role(UserRole::Viewer),
        )
        .await;
        run(&db, define_namespace(defined.as_str())).await;
        run(&db, define_database(defined.as_str())).await;

        let info: Option<BTreeMap<String, Value>> = info_for()
            .database()
            .run(&db)
            .await
            .expect("info")
            .take(0)
            .expect("object");
        let info = info.expect("database info");
        for key in ["accesses", "analyzers", "tables", "users"] {
            assert!(info.contains_key(key), "INFO FOR DB has `{key}`: {info:?}");
        }
        let table: Option<BTreeMap<String, Value>> = info_for()
            .table(user.clone())
            .run(&db)
            .await
            .expect("info")
            .take(0)
            .expect("object");
        assert!(table.expect("table info").contains_key("fields"));

        run(&db, remove_access("account").on_database()).await;
        run(&db, remove_access("api").on_database()).await;
        run(&db, remove_user("alice").on_database()).await;
        run(&db, remove_analyzer("simple")).await;
        run(&db, remove_event("audit").on_table(user.clone())).await;
        run(&db, remove_index("user_email").on_table(user.clone())).await;
        run(
            &db,
            remove_field(Field::new("email")).on_table(user.clone()),
        )
        .await;
        run(&db, remove_table(user)).await;
        run(&db, remove_database(defined.as_str())).await;
        run(&db, remove_namespace(defined.as_str())).await;
    }
}
