//! The options every `DEFINE` takes (`orm/src/statements/mod.rs`): `OVERWRITE`, `IF NOT EXISTS`,
//! `COMMENT`, and names read from parameters, on every definition builder.

use surrealdb::{Surreal, engine::local::Db, types::Kind};
use umbriel::{
    Field, JwtAlgorithm, Param, Runnable, Table, chain,
    statements::{
        FunctionArgument, define_access, define_analyzer, define_database, define_event,
        define_field, define_function, define_index, define_namespace, define_param,
        define_table, define_user, jwt_access, let_, return_, select,
    },
};
use umbriel_tests::*;

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("ran");
}

mod rendering {
    use super::*;

    #[test]
    fn every_definition_takes_overwrite() {
        assert_sql(
            &define_namespace("acme").overwrite(),
            "DEFINE NAMESPACE OVERWRITE acme;",
        );
        assert_sql(
            &define_database("acme").overwrite(),
            "DEFINE DATABASE OVERWRITE acme;",
        );
        assert_sql(&define_table("user").overwrite(), "DEFINE TABLE OVERWRITE user;");
        assert_sql(
            &define_field(Field::new("email")).on_table("user").overwrite(),
            "DEFINE FIELD OVERWRITE email ON TABLE user;",
        );
        assert_sql(
            &define_index("user_email")
                .on_table("user")
                .fields(Field::new("email"))
                .overwrite(),
            "DEFINE INDEX OVERWRITE user_email ON TABLE user FIELDS email;",
        );
        assert_sql(
            &define_event("audit")
                .on_table("user")
                .then(select(umbriel::All).from(Table::new("log")))
                .overwrite(),
            "DEFINE EVENT OVERWRITE audit ON TABLE user THEN (SELECT * FROM log);",
        );
        assert_sql(
            &define_param(&Param::new("greeting")).value("Hello").overwrite(),
            "DEFINE PARAM OVERWRITE $greeting VALUE 'Hello';",
        );
        assert_sql(
            &define_function("greet")
                .body(chain(return_("Hello")))
                .overwrite(),
            "DEFINE FUNCTION OVERWRITE fn::greet() {\nRETURN 'Hello';\n};",
        );
        assert_sql(
            &define_analyzer("simple").overwrite(),
            "DEFINE ANALYZER OVERWRITE simple;",
        );
        assert_sql(
            &define_user("alice")
                .on_database()
                .password("example-password")
                .overwrite(),
            "DEFINE USER OVERWRITE alice ON DATABASE PASSWORD 'example-password';",
        );
        assert_sql(
            &define_access("api")
                .on_database()
                .jwt(jwt_access().algorithm(JwtAlgorithm::RS256).key("example-key"))
                .overwrite(),
            "DEFINE ACCESS OVERWRITE api ON DATABASE TYPE JWT ALGORITHM RS256 KEY 'example-key';",
        );
    }

    #[test]
    fn every_definition_takes_if_not_exists() {
        assert_sql(
            &define_namespace("acme").if_not_exists(),
            "DEFINE NAMESPACE IF NOT EXISTS acme;",
        );
        assert_sql(
            &define_database("acme").if_not_exists(),
            "DEFINE DATABASE IF NOT EXISTS acme;",
        );
        assert_sql(
            &define_table("user").if_not_exists(),
            "DEFINE TABLE IF NOT EXISTS user;",
        );
        assert_sql(
            &define_field(Field::new("email")).on_table("user").if_not_exists(),
            "DEFINE FIELD IF NOT EXISTS email ON TABLE user;",
        );
        assert_sql(
            &define_param(&Param::new("greeting"))
                .value("Hello")
                .if_not_exists(),
            "DEFINE PARAM IF NOT EXISTS $greeting VALUE 'Hello';",
        );
        assert_sql(
            &define_analyzer("simple").if_not_exists(),
            "DEFINE ANALYZER IF NOT EXISTS simple;",
        );
    }

    #[test]
    fn the_last_mode_wins() {
        assert_sql(
            &define_table("user").overwrite().if_not_exists(),
            "DEFINE TABLE IF NOT EXISTS user;",
        );
        assert_sql(
            &define_table("user").if_not_exists().overwrite(),
            "DEFINE TABLE OVERWRITE user;",
        );
    }

    #[test]
    fn every_definition_takes_a_comment() {
        assert_sql(
            &define_namespace("acme").comment("Acme Corp"),
            "DEFINE NAMESPACE acme COMMENT 'Acme Corp';",
        );
        assert_sql(
            &define_database("acme").comment("Acme Corp"),
            "DEFINE DATABASE acme COMMENT 'Acme Corp';",
        );
        assert_sql(
            &define_table("user").comment("people"),
            "DEFINE TABLE user COMMENT 'people';",
        );
        assert_sql(
            &define_field(Field::new("email"))
                .on_table("user")
                .type_(Kind::String)
                .comment("login"),
            "DEFINE FIELD email ON TABLE user TYPE string COMMENT 'login';",
        );
        assert_sql(
            &define_index("user_email")
                .on_table("user")
                .fields(Field::new("email"))
                .unique()
                .comment("one account per address"),
            "DEFINE INDEX user_email ON TABLE user FIELDS email UNIQUE \
             COMMENT 'one account per address';",
        );
        assert_sql(
            &define_event("audit")
                .on_table("user")
                .then(select(umbriel::All).from(Table::new("log")))
                .comment("audit trail"),
            "DEFINE EVENT audit ON TABLE user THEN (SELECT * FROM log) COMMENT 'audit trail';",
        );
        assert_sql(
            &define_param(&Param::new("greeting"))
                .value("Hello")
                .comment("shown on sign-in"),
            "DEFINE PARAM $greeting VALUE 'Hello' COMMENT 'shown on sign-in';",
        );
        assert_sql(
            &define_function("greet")
                .body(chain(return_("Hello")))
                .comment("says hello"),
            "DEFINE FUNCTION fn::greet() {\nRETURN 'Hello';\n} COMMENT 'says hello';",
        );
        assert_sql(
            &define_analyzer("simple").comment("plain text"),
            "DEFINE ANALYZER simple COMMENT 'plain text';",
        );
        assert_sql(
            &define_user("alice")
                .on_database()
                .password("example-password")
                .comment("Alice"),
            "DEFINE USER alice ON DATABASE PASSWORD 'example-password' COMMENT 'Alice';",
        );
    }

    #[test]
    fn a_comment_with_quotes_parses() {
        let statement = define_table("user").comment("Alice's \"team\"");
        let debug = umbriel::Render::to_debug_sql(&statement);
        assert_parses(&debug);
        assert!(debug.contains("COMMENT "), "{debug}");
    }

    #[test]
    fn names_come_from_parameters() {
        let name = Param::new("name");
        let table = Param::new("table");
        assert_sql(&define_namespace(&name), "DEFINE NAMESPACE $name;");
        assert_sql(&define_database(&name), "DEFINE DATABASE $name;");
        assert_sql(&define_table(&table), "DEFINE TABLE $table;");
        assert_sql(
            &define_field(Field::new("email")).on_table(&table),
            "DEFINE FIELD email ON TABLE $table;",
        );
        assert_sql(
            &define_index(&name)
                .on_table(&table)
                .fields(Field::new("email")),
            "DEFINE INDEX $name ON TABLE $table FIELDS email;",
        );
        assert_sql(
            &define_event(&name)
                .on_table(&table)
                .then(select(umbriel::All).from(Table::new("log"))),
            "DEFINE EVENT $name ON TABLE $table THEN (SELECT * FROM log);",
        );
        assert_sql(&define_analyzer(&name), "DEFINE ANALYZER $name;");
        assert_sql(
            &define_user(&name)
                .on_database()
                .password("example-password"),
            "DEFINE USER $name ON DATABASE PASSWORD 'example-password';",
        );
        assert_sql(
            &define_access(&name)
                .on_database()
                .jwt(jwt_access().algorithm(JwtAlgorithm::RS256).key("example-key")),
            "DEFINE ACCESS $name ON DATABASE TYPE JWT ALGORITHM RS256 KEY 'example-key';",
        );
    }

    #[test]
    fn function_names_stay_literal() {
        let x = Param::new("x");
        assert_sql(
            &define_function("math::add_one")
                .arguments(vec![FunctionArgument {
                    name: x.clone(),
                    type_: Kind::Int,
                }])
                .body(chain(return_(Field::from(&x)))),
            "DEFINE FUNCTION fn::math::add_one($x: int) {\nRETURN $x;\n};",
        );
    }
}

mod execution {
    use super::*;

    #[tokio::test]
    async fn if_not_exists_keeps_an_existing_definition() {
        let db = db().await;
        run(&db, define_table("user").comment("first")).await;
        run(&db, define_table("user").if_not_exists().comment("second")).await;
        let table = stored_definition(&db, "INFO FOR DB;", "tables", "user").await;
        assert!(table.contains("COMMENT 'first'"), "{table}");

        run(
            &db,
            define_field(Field::new("email"))
                .on_table("user")
                .type_(Kind::String),
        )
        .await;
        run(
            &db,
            define_field(Field::new("email"))
                .on_table("user")
                .type_(Kind::Int)
                .if_not_exists(),
        )
        .await;
        let field = stored_definition(&db, "INFO FOR TABLE user;", "fields", "email").await;
        assert!(field.contains("TYPE string"), "{field}");

        run(&db, define_param(&Param::new("greeting")).value("Hello")).await;
        run(
            &db,
            define_param(&Param::new("greeting"))
                .value("Goodbye")
                .if_not_exists(),
        )
        .await;
        let greeting: String = read(&db, "RETURN $greeting;").await;
        assert_eq!(greeting, "Hello");
    }

    #[tokio::test]
    async fn a_plain_definition_of_an_existing_resource_fails() {
        let db = db().await;
        run(&db, define_table("user")).await;
        assert!(define_table("user").run(db.clone()).await.is_err());
    }

    #[tokio::test]
    async fn overwrite_replaces_a_definition() {
        let db = db().await;
        run(
            &db,
            define_field(Field::new("email"))
                .on_table("user")
                .type_(Kind::String),
        )
        .await;
        run(
            &db,
            define_field(Field::new("email"))
                .on_table("user")
                .type_(Kind::Int)
                .overwrite(),
        )
        .await;
        let field = stored_definition(&db, "INFO FOR TABLE user;", "fields", "email").await;
        assert!(field.contains("TYPE int"), "{field}");
    }

    #[tokio::test]
    async fn the_comment_is_stored() {
        let db = db().await;
        run(&db, define_table("user").comment("Acme Corp users")).await;
        let table = stored_definition(&db, "INFO FOR DB;", "tables", "user").await;
        assert!(table.contains("COMMENT 'Acme Corp users'"), "{table}");
    }

    #[tokio::test]
    async fn a_table_name_is_read_from_a_parameter() {
        let db = db().await;
        let table = let_("table").equal_to("user");
        run(
            &db,
            chain(table).chain(define_table(Param::new("table")).comment("from a parameter")),
        )
        .await;
        let user = stored_definition(&db, "INFO FOR DB;", "tables", "user").await;
        assert!(user.contains("COMMENT 'from a parameter'"), "{user}");
    }
}
