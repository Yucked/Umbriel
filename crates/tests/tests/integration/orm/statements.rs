//! The options every `DEFINE` takes (`orm/src/statements/mod.rs`): `OVERWRITE`, `IF NOT EXISTS`,
//! `COMMENT`, and names read from parameters, on every definition builder.

mod alter;
mod any;
mod create;
mod data;
mod define_access;
mod define_analyzer;
mod define_api;
mod define_bucket;
mod define_config;
mod define_database;
mod define_event;
mod define_field;
mod define_function;
mod define_index;
mod define_module;
mod define_namespace;
mod define_param;
mod define_sequence;
mod define_table;
mod define_user;
mod delete;
mod for_permission;
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
