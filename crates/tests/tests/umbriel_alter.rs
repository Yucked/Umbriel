//! `ALTER` statements (`orm/src/statements/alter.rs`): every builder's sets and drops, rendered
//! and checked with SurrealDB's parser, and their effect on stored definitions.

use surrealdb::{Surreal, engine::local::Db, types::Kind};
use umbriel::{
    All, Field, NONE, Operatable, Param, Runnable, Table, chain,
    statements::{
        AnalyzerFilter, FunctionArgument, Tokenizer, alter_analyzer, alter_event, alter_field,
        alter_function, alter_param, alter_table, define_analyzer, define_event, define_field,
        define_function, define_param, define_table, return_, select, table_relation,
    },
    value,
};
use umbriel_tests::*;

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("ran");
}

mod rendering {
    use super::*;

    #[test]
    fn alter_table_sets_and_drops() {
        assert_sql(
            &alter_table("user")
                .if_exists()
                .kind_relation(table_relation().in_(["user"]).out(["post"]))
                .schemaless()
                .comment("Acme Corp users")
                .permissions_full(),
            "ALTER TABLE IF EXISTS user TYPE RELATION IN user OUT post SCHEMALESS \
             COMMENT 'Acme Corp users' PERMISSIONS FULL;",
        );
        assert_sql(
            &alter_table(Param::new("table")).kind_any().drop_comment(),
            "ALTER TABLE $table TYPE ANY DROP COMMENT;",
        );
    }

    #[test]
    fn alter_field_sets_every_clause() {
        assert_sql(
            &alter_field(Field::new("meta"))
                .on_table("user")
                .if_exists()
                .type_(Kind::Object)
                .flexible()
                .readonly()
                .value(Field::new("meta"))
                .assert(value().is_not(NONE))
                .default_always(Field::new("fallback"))
                .comment("settings")
                .permissions_none(),
            "ALTER FIELD IF EXISTS meta ON TABLE user TYPE object FLEXIBLE READONLY VALUE meta \
             ASSERT $value IS NOT NONE DEFAULT ALWAYS fallback COMMENT 'settings' PERMISSIONS NONE;",
        );
    }

    #[test]
    fn alter_field_drops_every_clause() {
        assert_sql(
            &alter_field(Field::new("meta"))
                .on_table("user")
                .drop_type()
                .drop_flexible()
                .drop_readonly()
                .drop_value()
                .drop_assert()
                .drop_default()
                .drop_comment(),
            "ALTER FIELD meta ON TABLE user DROP TYPE DROP FLEXIBLE DROP READONLY DROP VALUE \
             DROP ASSERT DROP DEFAULT DROP COMMENT;",
        );
    }

    #[test]
    fn alter_event_sets_and_drops() {
        assert_sql(
            &alter_event("audit")
                .on_table("user")
                .when(Param::new("event").is("CREATE"))
                .then(select(All).from(Table::new("log")))
                .then(select(All).from(Table::new("archive")))
                .async_()
                .retry(2)
                .max_depth(1)
                .comment("audit trail"),
            "ALTER EVENT audit ON TABLE user WHEN $event IS 'CREATE' \
             THEN (SELECT * FROM log), (SELECT * FROM archive) ASYNC RETRY 2 MAXDEPTH 1 \
             COMMENT 'audit trail';",
        );
        assert_sql(
            &alter_event("audit")
                .on_table("user")
                .drop_when()
                .drop_then()
                .drop_async()
                .drop_comment(),
            "ALTER EVENT audit ON TABLE user DROP WHEN DROP THEN DROP ASYNC DROP COMMENT;",
        );
    }

    #[test]
    fn alter_param_sets_value_comment_and_permissions() {
        assert_sql(
            &alter_param(&Param::new("greeting"))
                .if_exists()
                .value("Hello")
                .comment("shown on sign-in")
                .permissions_where(Param::new("public").is(true)),
            "ALTER PARAM IF EXISTS $greeting VALUE 'Hello' COMMENT 'shown on sign-in' \
             PERMISSIONS WHERE $public IS true;",
        );
        assert_sql(
            &alter_param(&Param::new("greeting")).drop_comment(),
            "ALTER PARAM $greeting DROP COMMENT;",
        );
    }

    #[test]
    fn alter_function_replaces_the_signature() {
        let x = Param::new("x");
        assert_sql(
            &alter_function("add_one")
                .if_exists()
                .signature(
                    vec![FunctionArgument {
                        name: x.clone(),
                        type_: Kind::Int,
                    }],
                    chain(return_(Field::from(&x).add(1))),
                )
                .returns(Kind::Int)
                .comment("adds one")
                .permissions_full(),
            "ALTER FUNCTION IF EXISTS fn::add_one($x: int) -> int {\nRETURN $x + 1;\n} \
             COMMENT 'adds one' PERMISSIONS FULL;",
        );
        assert_sql(
            &alter_function("add_one").drop_comment(),
            "ALTER FUNCTION fn::add_one DROP COMMENT;",
        );
    }

    #[test]
    fn alter_analyzer_sets_and_drops() {
        assert_sql(
            &alter_analyzer("simple")
                .function("normalise")
                .tokenizers([Tokenizer::Blank, Tokenizer::Class])
                .filters([AnalyzerFilter::Lowercase, AnalyzerFilter::Ascii])
                .comment("plain text"),
            "ALTER ANALYZER simple FUNCTION fn::normalise TOKENIZERS Blank,Class \
             FILTERS lowercase,ascii COMMENT 'plain text';",
        );
        assert_sql(
            &alter_analyzer("simple")
                .drop_function()
                .drop_tokenizers()
                .drop_filters()
                .drop_comment(),
            "ALTER ANALYZER simple DROP FUNCTION DROP TOKENIZERS DROP FILTERS DROP COMMENT;",
        );
    }

    #[test]
    fn setting_and_dropping_one_clause_fails() {
        assert_render_error(
            &alter_table("user").comment("Acme Corp").drop_comment(),
            "both set and drop COMMENT",
        );
        assert_render_error(
            &alter_field(Field::new("email"))
                .on_table("user")
                .default(1)
                .drop_default(),
            "both set and drop DEFAULT",
        );
        assert_render_error(
            &alter_event("audit").on_table("user").async_().drop_async(),
            "both set and drop ASYNC",
        );
        assert_render_error(
            &alter_analyzer("simple")
                .filters([AnalyzerFilter::Lowercase])
                .drop_filters(),
            "both set and drop FILTERS",
        );
    }

    #[test]
    fn other_refused_combinations_fail() {
        assert_render_error(
            &alter_field(Field::new("name"))
                .on_table("user")
                .type_(Kind::String)
                .flexible(),
            "FLEXIBLE",
        );
        assert_render_error(&alter_event("audit").on_table("user").retry(2), "ASYNC");
        assert_render_error(
            &alter_function("add_one").returns(Kind::Int),
            "only with its signature",
        );
    }
}

mod behaviour {
    use super::*;

    #[tokio::test]
    async fn alter_table_changes_only_what_it_names() {
        let db = db().await;
        run(&db, define_table("user").schemaless().comment("first")).await;
        run(&db, alter_table("user").schemafull()).await;
        let table = stored_definition(&db, "INFO FOR DB;", "tables", "user").await;
        assert!(table.contains("SCHEMAFULL"), "{table}");
        assert!(table.contains("COMMENT 'first'"), "{table}");
        run(&db, alter_table("user").drop_comment()).await;
        let table = stored_definition(&db, "INFO FOR DB;", "tables", "user").await;
        assert!(!table.contains("COMMENT"), "{table}");
    }

    #[tokio::test]
    async fn alter_field_changes_its_clauses() {
        let db = db().await;
        run(
            &db,
            define_field(Field::new("score"))
                .on_table("user")
                .type_(Kind::Int)
                .default(1),
        )
        .await;
        run(
            &db,
            alter_field(Field::new("score"))
                .on_table("user")
                .readonly()
                .drop_default(),
        )
        .await;
        let field = stored_definition(&db, "INFO FOR TABLE user;", "fields", "score").await;
        assert!(field.contains("READONLY"), "{field}");
        assert!(!field.contains("DEFAULT"), "{field}");
        assert!(field.contains("TYPE int"), "{field}");
    }

    #[tokio::test]
    async fn a_none_default_leaves_new_records_without_the_field() {
        let db = db().await;
        run(
            &db,
            define_field(Field::new("score"))
                .on_table("user")
                .type_(Kind::option(Kind::Int))
                .default(1),
        )
        .await;
        run(
            &db,
            alter_field(Field::new("score")).on_table("user").default(NONE),
        )
        .await;
        assert!(succeeds(&db, "CREATE user:alice;").await);
        let score: Option<i64> = read(&db, "RETURN user:alice.score;").await;
        assert_eq!(score, None);
    }

    #[tokio::test]
    async fn alter_event_changes_how_it_runs() {
        let db = db().await;
        run(
            &db,
            define_event("audit")
                .on_table("user")
                .when(Param::new("event").is("CREATE"))
                .then(select(All).from(Table::new("log"))),
        )
        .await;
        run(
            &db,
            alter_event("audit")
                .on_table("user")
                .drop_when()
                .async_()
                .comment("audit trail"),
        )
        .await;
        let event = stored_definition(&db, "INFO FOR TABLE user;", "events", "audit").await;
        assert!(event.contains("ASYNC"), "{event}");
        assert!(event.contains("COMMENT 'audit trail'"), "{event}");
        assert!(!event.contains("CREATE"), "{event}");
    }

    #[tokio::test]
    async fn alter_param_replaces_the_value() {
        let db = db().await;
        let greeting = Param::new("greeting");
        run(&db, define_param(&greeting).value("Hello")).await;
        run(&db, alter_param(&greeting).value("Hi")).await;
        let value: String = read(&db, "RETURN $greeting;").await;
        assert_eq!(value, "Hi");
    }

    #[tokio::test]
    async fn alter_function_replaces_the_body() {
        let db = db().await;
        let x = Param::new("x");
        let arguments = || {
            vec![FunctionArgument {
                name: x.clone(),
                type_: Kind::Int,
            }]
        };
        run(
            &db,
            define_function("step")
                .arguments(arguments())
                .body(chain(return_(Field::from(&x).add(1)))),
        )
        .await;
        run(
            &db,
            alter_function("step").signature(arguments(), chain(return_(Field::from(&x).add(2)))),
        )
        .await;
        let result: i64 = read(&db, "RETURN fn::step(1);").await;
        assert_eq!(result, 3);
    }

    #[tokio::test]
    async fn alter_analyzer_replaces_the_filters() {
        let db = db().await;
        run(
            &db,
            define_analyzer("simple")
                .tokenizers([Tokenizer::Blank])
                .filters([AnalyzerFilter::Lowercase]),
        )
        .await;
        run(
            &db,
            alter_analyzer("simple").filters([AnalyzerFilter::Uppercase]),
        )
        .await;
        let analyzer =
            stored_definition(&db, "INFO FOR DB;", "analyzers", "simple").await.to_lowercase();
        assert!(analyzer.contains("uppercase"), "{analyzer}");
        assert!(!analyzer.contains("lowercase"), "{analyzer}");
    }

    #[tokio::test]
    async fn if_exists_alters_nothing_without_failing() {
        let db = db().await;
        assert!(alter_table("missing").if_exists().comment("x").run(db.clone()).await.is_ok());
        assert!(
            alter_field(Field::new("email"))
                .on_table("missing")
                .if_exists()
                .readonly()
                .run(db.clone())
                .await
                .is_ok()
        );
        assert!(
            alter_param(&Param::new("missing"))
                .if_exists()
                .value(1)
                .run(db.clone())
                .await
                .is_ok()
        );
        assert!(alter_table("missing").comment("x").run(db.clone()).await.is_err());
    }
}
