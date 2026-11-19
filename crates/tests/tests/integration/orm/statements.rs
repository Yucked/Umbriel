//! The options every `DEFINE` takes (`orm/src/statements/mod.rs`): `OVERWRITE`, `IF NOT EXISTS`,
//! `COMMENT`, and names read from parameters, on every definition builder.

mod alter;
mod any;
mod create;
mod data;
mod define_api;
mod define_bucket;
mod define_config;
mod define_database;
mod define_event;
mod define_field;
mod define_function;
mod define_module;
mod define_param;
mod define_sequence;
mod define_table;
mod delete;
mod ifelse;
mod info;
mod insert;
mod let_;
mod relate;
mod remove;
mod return_;
mod returning;
mod select;
mod show;
mod subquery;
mod throw;
mod update;
mod upsert;
mod use_;
mod utils_chain;

use surrealdb::{Surreal, engine::any::Any, types::Kind};
use umbriel::{
    Field, JwtAlgorithm, Param, Runnable, Table, chain,
    statements::{
        FunctionArgument, define_access, define_analyzer, define_database, define_event,
        define_field, define_function, define_index, define_namespace, define_param, define_table,
        define_user, jwt_access, let_, return_, select,
    },
};
use umbriel_tests::{support::statements::*, *};

async fn run(db: &Surreal<Any>, statement: impl Runnable + Sync) {
    statement.run(db).await.expect("ran");
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
        assert_sql(
            &define_table("user").overwrite(),
            "DEFINE TABLE OVERWRITE user;",
        );
        assert_sql(
            &define_field(Field::new("email"))
                .on_table("user")
                .overwrite(),
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
            &define_param(&Param::new("greeting"))
                .value("Hello")
                .overwrite(),
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
                .jwt(
                    jwt_access()
                        .algorithm(JwtAlgorithm::RS256)
                        .key("example-key"),
                )
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
            &define_field(Field::new("email"))
                .on_table("user")
                .if_not_exists(),
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
            &define_access(&name).on_database().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::RS256)
                    .key("example-key"),
            ),
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
        run(&db, define_table("user")).await;
        assert!(define_table("user").run(&db).await.is_err());
    }

    #[tokio::test]
    async fn overwrite_replaces_a_definition() {
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
        run(&db, define_table("user").comment("Acme Corp users")).await;
        let table = stored_definition(&db, "INFO FOR DB;", "tables", "user").await;
        assert!(table.contains("COMMENT 'Acme Corp users'"), "{table}");
    }

    #[tokio::test]
    async fn a_table_name_is_read_from_a_parameter() {
        let (db, _guard) = db().await;
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

mod definitions {
    use super::*;

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

    #[test]
    fn index_definitions() {
        let user = Table::from("user");
        assert_sql(
            &define_index("user_email")
                .on_table(user.clone())
                .fields(Field::new("email"))
                .unique(),
            "DEFINE INDEX user_email ON TABLE user FIELDS email UNIQUE;",
        );
        assert_sql(
            &define_index("user_name")
                .on_table(user.clone())
                .columns(&[Field::new("first"), Field::new("last")]),
            "DEFINE INDEX user_name ON TABLE user COLUMNS first, last;",
        );
        assert_sql(
            &define_index("post_content")
                .on_table(Table::from("post"))
                .fields(Field::new("content"))
                .fulltext(fulltext_analyzer("ascii").bm25(1.2, 0.75).highlights()),
            "DEFINE INDEX post_content ON TABLE post FIELDS content \
             FULLTEXT ANALYZER ascii BM25(1.2, 0.75) HIGHLIGHTS;",
        );
        assert_sql(
            &define_index("post_title")
                .on_table(Table::from("post"))
                .fields(Field::new("title"))
                .fulltext(fulltext_analyzer("ascii")),
            "DEFINE INDEX post_title ON TABLE post FIELDS title FULLTEXT ANALYZER ascii;",
        );
    }

    #[test]
    fn define_analyzer_event_param_namespace_database_and_function() {
        assert_sql(
            &define_analyzer("ascii")
                .tokenizers([Tokenizer::Class, Tokenizer::Blank])
                .filters([
                    AnalyzerFilter::Lowercase,
                    AnalyzerFilter::Ascii,
                    AnalyzerFilter::Edgengram(2, 15),
                    AnalyzerFilter::Snowball(SnowballLanguage::English),
                ]),
            "DEFINE ANALYZER ascii TOKENIZERS Class,Blank \
             FILTERS lowercase,ascii,edgengram(2, 15),snowball(english);",
        );
        assert_sql(
            &define_event("email_changed")
                .on_table(Table::from("user"))
                .when(
                    Field::from(Param::new("before"))
                        .with_field("email")
                        .not_equal(Field::from(Param::new("after")).with_field("email")),
                )
                .then(
                    create::<SpaceShip>(SpaceShip::table())
                        .set(SpaceShip::schema().name.equal_to("Apollo")),
                ),
            "DEFINE EVENT email_changed ON TABLE user WHEN $before.email != $after.email \
             THEN (CREATE space_ship SET name = 'Apollo');",
        );
        umbriel::create_param_name_fn!(endpoint_base);
        assert_sql(
            &define_param(endpoint_base()).value("https://example.com"),
            "DEFINE PARAM $endpoint_base VALUE 'https://example.com';",
        );
        assert_sql(
            &define_namespace(Namespace::new("acme")),
            "DEFINE NAMESPACE acme;",
        );
        assert_sql(&define_database("acme"), "DEFINE DATABASE acme;");

        let total = Param::new("total");
        assert_sql(
            &define_function("add_one")
                .arguments(vec![umbriel::statements::FunctionArgument {
                    name: total.clone(),
                    type_: Kind::Int,
                }])
                .body(chain(return_(Field::from(&total).add(1)))),
            "DEFINE FUNCTION fn::add_one($total: int) {\nRETURN $total + 1;\n};",
        );
    }

    #[test]
    fn user_definitions() {
        assert_sql(
            &define_user("alice")
                .on_root()
                .password("example-password")
                .role(UserRole::Owner),
            "DEFINE USER alice ON ROOT PASSWORD 'example-password' ROLES OWNER;",
        );
        assert_sql(
            &define_user("bob")
                .on_namespace()
                .passhash("$argon2id$example")
                .role(UserRole::Editor),
            "DEFINE USER bob ON NAMESPACE PASSHASH '$argon2id$example' ROLES EDITOR;",
        );
        assert_sql(
            &define_user("carol")
                .on_database()
                .password("example")
                .role(UserRole::Viewer),
            "DEFINE USER carol ON DATABASE PASSWORD 'example' ROLES VIEWER;",
        );
    }

    #[test]
    fn access_definitions() {
        let signup =
            Raw::new("CREATE user SET email = $email, pass = crypto::argon2::generate($pass)");
        let signin = Raw::new(
            "SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)",
        );
        assert_sql(
            &define_access("account").on_database().record(
                record_access()
                    .signup(signup)
                    .signin(signin)
                    .with_jwt(JwtAlgorithm::HS512, "example-key")
                    .duration_for_token(Duration::from_secs(15 * 60))
                    .duration_for_session(Duration::from_secs(12 * 60 * 60)),
            ),
            "DEFINE ACCESS account ON DATABASE TYPE RECORD \
             SIGNUP (CREATE user SET email = $email, pass = crypto::argon2::generate($pass)) \
             SIGNIN (SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)) \
             WITH JWT ALGORITHM HS512 KEY 'example-key' \
             DURATION FOR TOKEN 15m, FOR SESSION 12h;",
        );
        assert_sql(
            &define_access("account").on_database().record(
                record_access()
                    .signin(select(All).from(Table::from("user")))
                    .duration_for_session(Duration::from_secs(60 * 60)),
            ),
            "DEFINE ACCESS account ON DATABASE TYPE RECORD SIGNIN (SELECT * FROM user) \
             DURATION FOR SESSION 1h;",
        );
        assert_sql(
            &define_access("api").on_namespace().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::RS256)
                    .key("example-public-key"),
            ),
            "DEFINE ACCESS api ON NAMESPACE TYPE JWT ALGORITHM RS256 KEY 'example-public-key';",
        );
        assert_sql(
            &define_access("issuer").on_root().jwt(
                jwt_access()
                    .url("https://example.com/jwks.json")
                    .duration_for_session(Duration::from_secs(60)),
            ),
            "DEFINE ACCESS issuer ON ROOT TYPE JWT URL 'https://example.com/jwks.json' \
             DURATION FOR SESSION 1m;",
        );
    }

    #[test]
    fn define_access_rejects_invalid_combinations() {
        assert_render_error(
            &define_access("account").on_root().record(record_access()),
            "only be defined on a database",
        );
        assert_render_error(
            &define_access("account")
                .on_namespace()
                .record(record_access()),
            "only be defined on a database",
        );
        assert_render_error(
            &define_access("api").jwt(jwt_access().url("https://example.com")),
            "needs a level",
        );
        assert_render_error(&define_access("api").on_database(), "needs a type");
        assert_render_error(
            &define_access("api").on_database().jwt(jwt_access()),
            "needs `algorithm` and `key`",
        );
        assert_render_error(
            &define_access("api").on_database().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::HS256)
                    .key("example")
                    .url("https://example.com"),
            ),
            "not both",
        );
    }

    #[test]
    fn for_permissions() {
        let age = Field::new("age");
        assert_eq!(
            for_permission(Select).where_(age.gte(18)).to_debug_sql(),
            "FOR select\n\tWHERE age >= 18"
        );
        assert_eq!(
            for_permission([Create, Update, Delete])
                .where_(age.lte(130))
                .to_debug_sql(),
            "FOR create, update, delete\n\tWHERE age <= 130"
        );
    }

    #[test]
    fn remove_statements() {
        assert_sql(&remove_table(Table::from("user")), "REMOVE TABLE user;");
        assert_sql(
            &remove_field(Field::new("email")).on_table(Table::from("user")),
            "REMOVE FIELD email ON TABLE user;",
        );
        assert_sql(
            &remove_index("user_email").on_table(Table::from("user")),
            "REMOVE INDEX user_email ON TABLE user;",
        );
        assert_sql(
            &remove_event("email_changed").on_table(Table::from("user")),
            "REMOVE EVENT email_changed ON TABLE user;",
        );
        assert_sql(&remove_analyzer("ascii"), "REMOVE ANALYZER ascii;");
        assert_sql(
            &remove_param("endpoint_base"),
            "REMOVE PARAM $endpoint_base;",
        );
        assert_sql(&remove_function("add_one"), "REMOVE FUNCTION fn::add_one;");
        assert_sql(&remove_namespace("acme"), "REMOVE NAMESPACE acme;");
        assert_sql(&remove_database("acme"), "REMOVE DATABASE acme;");
        assert_sql(
            &remove_user("alice").on_root(),
            "REMOVE USER alice ON ROOT;",
        );
        assert_sql(
            &remove_user("alice").on_namespace(),
            "REMOVE USER alice ON NAMESPACE;",
        );
        assert_sql(
            &remove_user("alice").on_database(),
            "REMOVE USER alice ON DATABASE;",
        );
        assert_sql(
            &remove_access("account").on_database(),
            "REMOVE ACCESS account ON DATABASE;",
        );
        assert_sql(
            &remove_access("api").on_namespace(),
            "REMOVE ACCESS api ON NAMESPACE;",
        );
        assert_sql(
            &remove_access("issuer").on_root(),
            "REMOVE ACCESS issuer ON ROOT;",
        );
        assert_render_error(&remove_access("account"), "needs a level");
        assert_render_error(&remove_param("_p0"), "reserved");
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

    #[tokio::test]
    async fn unique_indexes_reject_duplicates() {
        let (db, _guard) = db().await;
        run(
            &db,
            define_index("user_email")
                .on_table(Table::from("user"))
                .fields(Field::new("email"))
                .unique(),
        )
        .await;
        run(
            &db,
            Raw::new("CREATE user:alice SET email = 'alice@example.com';"),
        )
        .await;
        assert!(
            Raw::new("CREATE user:bob SET email = 'alice@example.com';")
                .run(&db)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn fulltext_indexes_answer_matches_with_scores() {
        let (db, _guard) = db().await;
        run(
            &db,
            define_analyzer("simple")
                .tokenizers([Tokenizer::Blank])
                .filters([AnalyzerFilter::Lowercase]),
        )
        .await;
        run(
            &db,
            define_index("post_content")
                .on_table(Table::from("post"))
                .fields(Field::new("content"))
                .fulltext(fulltext_analyzer("simple").bm25(1.2, 0.75).highlights()),
        )
        .await;
        run(
            &db,
            Raw::new(
                "CREATE post:one SET content = 'Hello world'; CREATE post:two SET content = 'Goodbye moon';",
            ),
        )
        .await;

        let content = Field::new("content");
        let found: Vec<String> = select_value(content.clone())
            .from(Table::from("post"))
            .where_(content.matches("hello"))
            .decode()
            .execute(&db)
            .await
            .expect("searched");
        assert_eq!(found, ["Hello world"]);

        let scored: Vec<BTreeMap<String, Value>> = select(arr![
            search::score!(1).__as__("score"),
            search::highlight!("<b>", "</b>", 1).__as__("marked")
        ])
        .from(Table::from("post"))
        .where_(content.matches_with_ref(1, "hello"))
        .decode()
        .execute(&db)
        .await
        .expect("scored");
        assert_eq!(scored.len(), 1);
        assert!(matches!(scored[0].get("score"), Some(Value::Number(_))));
        assert_eq!(
            scored[0].get("marked"),
            Some(&Value::String("<b>Hello</b> world".to_owned()))
        );
    }

    #[tokio::test]
    async fn events_run_on_changes() {
        let (db, _guard) = db().await;
        run(
            &db,
            define_event("ship_created")
                .on_table(Table::from("space_ship"))
                .when(Param::new("event").equal("CREATE"))
                .then(
                    create::<WeaponStats>(WeaponStats::table())
                        .set(WeaponStats::schema().averageStrength.equal_to(1.5)),
                ),
        )
        .await;
        space_ship("apollo", "Apollo")
            .create(&db)
            .await
            .expect("created");
        let stats: usize = read(&db, "RETURN count(SELECT * FROM weapon_stats);").await;
        assert_eq!(stats, 1);
    }

    #[tokio::test]
    async fn params_and_functions_are_usable_after_definition() {
        let (db, _guard) = db().await;
        umbriel::create_param_name_fn!(greeting);
        run(&db, define_param(greeting()).value("Hello")).await;
        let value: String = read(&db, "RETURN $greeting;").await;
        assert_eq!(value, "Hello");

        let total = Param::new("total");
        run(
            &db,
            define_function("add_one")
                .arguments(vec![umbriel::statements::FunctionArgument {
                    name: total.clone(),
                    type_: Kind::Int,
                }])
                .body(chain(return_(Field::from(&total).add(1)))),
        )
        .await;
        let result: i64 = read(&db, "RETURN fn::add_one(41);").await;
        assert_eq!(result, 42);

        run(&db, remove_function("add_one")).await;
        assert!(Raw::new("RETURN fn::add_one(1);").run(&db).await.is_err());
        run(&db, remove_param("greeting")).await;
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


mod info_show_and_utilities {
    use super::*;

    #[test]
    fn info() {
        assert_sql(&info_for().root(), "INFO FOR ROOT;");
        assert_sql(&info_for().namespace(), "INFO FOR NS;");
        assert_sql(&info_for().database(), "INFO FOR DB;");
        assert_sql(&info_for().table("user"), "INFO FOR TABLE user;");
    }

    #[test]
    fn show_changes() {
        assert_sql(
            &show_changes_for_table("reading", 5u64).limit(10),
            "SHOW CHANGES FOR TABLE reading SINCE 5 LIMIT 10;",
        );
        assert_sql(
            &show_changes_for_table("reading", DateTime::<Utc>::UNIX_EPOCH),
            "SHOW CHANGES FOR TABLE reading SINCE d'1970-01-01T00:00:00Z';",
        );
    }

    #[test]
    fn use_sleep_throw_live_and_loop_control() {
        assert_sql(&use_().namespace("test"), "USE NS test;");
        assert_sql(&use_().database("test"), "USE DB test;");
        assert_sql(
            &use_().database("test").namespace("acme"),
            "USE NS acme DB test;",
        );
        assert_render_error(&use_(), "needs a namespace");
        assert_sql(&sleep(Duration::from_secs(43)), "SLEEP 43s;");
        assert_sql(
            &throw("something went wrong"),
            "THROW 'something went wrong';",
        );
        assert_sql(
            &live(select(All).from(Table::from("person"))),
            "LIVE SELECT * FROM person;",
        );
        assert_eq!(break_().to_debug_sql(), "BREAK;");
        assert_eq!(continue_().to_debug_sql(), "CONTINUE;");
    }

    #[test]
    fn return_and_let() {
        assert_sql(
            &return_(User::id("alice")).fetch(&[Field::new("account"), Field::new("friends")]),
            "RETURN user:alice FETCH account, friends;",
        );
        assert_sql(&return_(Field::new("age").add(1)), "RETURN age + 1;");
        assert_sql(&let_("name").equal_to("Alice"), "LET $name = 'Alice';");
        assert_sql(
            &let_("users").equal_to(select(All).from(Table::from("user"))),
            "LET $users = (SELECT * FROM user);",
        );
        assert_render_error(&let_("_p1").equal_to(1), "reserved");
        assert_render_error(&let_("not valid").equal_to(1), "must start with a letter");
    }

    #[tokio::test]
    async fn throw_fails_and_sleep_and_return_succeed() {
        let (db, _guard) = db().await;
        match throw("something went wrong").run(&db).await {
            Err(error) => assert!(error.to_string().contains("something went wrong")),
            Ok(_) => panic!("THROW succeeded"),
        }
        run(&db, sleep(Duration::from_millis(1))).await;
        let value: Option<i64> = return_(41)
            .run(&db)
            .await
            .expect("ran")
            .take(0)
            .expect("value");
        assert_eq!(value, Some(41));
    }
}


mod control_flow {
    use super::*;

    #[test]
    fn if_else_branches() {
        let age = Field::new("age");
        let country = Field::new("country");
        assert_sql(
            &if_(age.gte(18)).then("adult").end(),
            "IF age >= 18 \n\t{ 'adult' };",
        );
        assert_sql(
            &if_(age.gte(18))
                .then("adult")
                .else_if(cond(country.is("CA")).or(country.is("NO")))
                .then("cold")
                .else_("other")
                .end(),
            "IF age >= 18 \n\t{ 'adult' }\nELSE IF (country IS 'CA') OR (country IS 'NO') \n\t{ 'cold' }\
             \nELSE\n\t{ 'other' };",
        );
        assert_sql(
            &if_(age.gte(18))
                .then(select(All).from(Table::from("user")))
                .end(),
            "IF age >= 18 \n\t{ (SELECT * FROM user) };",
        );
    }

    #[test]
    fn for_loops_iterate_one_parameter() {
        let name = Param::new("name");
        assert_sql(
            &for_(&name).in_(vec!["Alice", "Bob"]).block(chain(
                create::<SpaceShip>(SpaceShip::table())
                    .set(SpaceShip::schema().name.equal_to(&name)),
            )),
            "FOR $name IN ['Alice', 'Bob'] {\nCREATE space_ship SET name = $name;\n};",
        );
    }

    #[test]
    fn transactions_commit_or_cancel() {
        let create_ship = || {
            create::<SpaceShip>(SpaceShip::table()).set(SpaceShip::schema().name.equal_to("Apollo"))
        };
        let committed = begin_transaction()
            .query(create_ship())
            .query(select(All).from(SpaceShip::table()))
            .commit_transaction();
        assert_eq!(
            committed.to_debug_sql(),
            "BEGIN TRANSACTION;\n\nCREATE space_ship SET name = 'Apollo';\n\n\
             SELECT * FROM space_ship;\n\nCOMMIT TRANSACTION;\n\t"
        );
        assert_parses(&committed.to_debug_sql());
        let cancelled = begin_transaction()
            .query(create_ship())
            .cancel_transaction();
        assert!(
            cancelled
                .to_debug_sql()
                .ends_with("CANCEL TRANSACTION;\n\t")
        );
        assert_parses(cancelled.to_query().expect("valid").sql());
    }

    #[test]
    fn chains_and_blocks() {
        let total = let_("total").equal_to(41);
        let chained = chain(total.clone()).chain(return_(Field::from(&total.get_param()).add(1)));
        assert_eq!(
            chained.to_debug_sql(),
            "LET $total = 41;\n\nRETURN $total + 1;"
        );
        assert_parses(&chained.to_debug_sql());
        assert_eq!(
            chained.clone().parenthesized().to_debug_sql(),
            "(LET $total = 41;\n\nRETURN $total + 1;)"
        );
        let block = block(chained.clone());
        assert_eq!(
            block.to_debug_sql(),
            "{\nLET $total = 41;\n\nRETURN $total + 1;\n}"
        );
        assert_eq!(chained.as_block(), block);
    }

    #[tokio::test]
    async fn control_flow_runs_in_the_database() {
        let (db, _guard) = db().await;
        let name = Param::new("name");
        run(
            &db,
            for_(&name).in_(vec!["Apollo", "Gemini"]).block(chain(
                create::<SpaceShip>(SpaceShip::table())
                    .set(SpaceShip::schema().name.equal_to(&name)),
            )),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);

        let age = Param::new("age");
        let branch = chain(let_("age").equal_to(20))
            .chain(if_(age.gte(18)).then("adult").else_("minor").end());
        let result: Option<String> = branch.run(&db).await.expect("ran").take(1).expect("value");
        assert_eq!(result.as_deref(), Some("adult"));

        let block_value: Option<i64> = return_(block(
            chain(let_("total").equal_to(41))
                .chain(return_(Field::from(Param::new("total")).add(1))),
        ))
        .run(&db)
        .await
        .expect("ran")
        .take(0)
        .expect("value");
        assert_eq!(block_value, Some(42));
    }

    #[tokio::test]
    async fn committed_transactions_persist_and_cancelled_ones_do_not() {
        let (db, _guard) = db().await;
        run(
            &db,
            begin_transaction()
                .query(
                    create::<SpaceShip>(SpaceShip::table()).content(space_ship("apollo", "Apollo")),
                )
                .query(
                    create::<SpaceShip>(SpaceShip::table()).content(space_ship("gemini", "Gemini")),
                )
                .commit_transaction(),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);

        run(
            &db,
            begin_transaction()
                .query(
                    create::<SpaceShip>(SpaceShip::table())
                        .content(space_ship("mercury", "Mercury")),
                )
                .cancel_transaction(),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);
    }

    #[tokio::test]
    async fn a_transfer_between_accounts_is_atomic() {
        let (db, _guard) = db().await;
        let acc = Account::schema();
        let one = Account::id("one");
        let two = Account::id("two");
        run(
            &db,
            begin_transaction()
                .query(create::<Account>(Account::table()).content(Account {
                    id: one.clone(),
                    balance: 100.0,
                }))
                .query(create::<Account>(Account::table()).content(Account {
                    id: two.clone(),
                    balance: 50.0,
                }))
                .query(update::<Account>(&one).set(acc.balance.decrement_by(30.0)))
                .query(update::<Account>(&two).set(acc.balance.increment_by(30.0)))
                .commit_transaction(),
        )
        .await;
        let balances: Vec<f64> = read(&db, "SELECT VALUE balance FROM account ORDER BY id;").await;
        assert_eq!(balances, [70.0, 80.0]);

        let failed = begin_transaction()
            .query(update::<Account>(&one).set(acc.balance.decrement_by(30.0)))
            .query(throw("insufficient funds"))
            .commit_transaction()
            .run(&db)
            .await;
        assert!(failed.is_err());
        let balances: Vec<f64> = read(&db, "SELECT VALUE balance FROM account ORDER BY id;").await;
        assert_eq!(balances, [70.0, 80.0]);
    }

    #[tokio::test]
    async fn let_parameters_carry_results_between_statements() {
        let (db, _guard) = db().await;
        let balance = let_("balance").equal_to(
            create_only::<Balance>(CreateTarget::table())
                .set(Balance::schema().amount.equal_to(25.0)),
        );
        let created_balance = balance.with_path::<Balance>(E).amount;
        let acc = Account::schema();
        run(
            &db,
            chain(balance.clone())
                .chain(create::<Account>(Account::table()).content(Account {
                    id: Account::id("one"),
                    balance: 0.0,
                }))
                .chain(
                    update::<Account>(Account::id("one"))
                        .set(acc.balance.increment_by(created_balance)),
                ),
        )
        .await;
        let amount: f64 = read(&db, "RETURN account:one.balance;").await;
        assert_eq!(amount, 25.0);
    }
}

