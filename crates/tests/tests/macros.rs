//! The exported macros: `query!`/`query_raw!`, `chain!`/`block!`/`transaction!`, `for_!`,
//! `define_function!`, `object!`/`object_partial!`, `let_!`, `pick!`/`Pickable`, and the
//! deprecated `block_deprecated!`.

use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    All, E, Field, Model, NONE, Operatable, Param, Pickable, Render, Runnable,
    SchemaGetter, SetterAssignable, SetterNumeric, Table, block, chain, define_function, for_,
    functions::math,
    object, object_partial, pick, query, query_raw,
    statements::{
        CreateTarget, Returning, create, create_only, insert, return_, select, select_value,
        update,
    },
    transaction,
};
use umbriel_tests::*;

async fn ships(db: &Surreal<Db>) -> Vec<String> {
    read(db, "SELECT VALUE name FROM space_ship ORDER BY name;").await
}

mod query_macros {
    use super::*;

    #[test]
    fn query_raw_checks_the_query_at_compile_time() {
        let query = query_raw!("SELECT name, age, * FROM user WHERE name = $name");
        assert_eq!(query, "SELECT name, age, * FROM user WHERE name = $name");
        let graph = query_raw!(
            "SELECT ->purchased->product<-purchased<-person->purchased->product FROM person:alice"
        );
        assert_parses(graph);
    }

    #[tokio::test]
    async fn query_binds_values_to_placeholders() {
        let db = db().await;
        let name = "Alice";
        let mut response = query!(db, "RETURN $name", { name: name })
            .await
            .expect("ran");
        let returned: Option<String> = response.take(0).expect("string");
        assert_eq!(returned.as_deref(), Some("Alice"));

        let mut response = query!(db, "RETURN 1").await.expect("ran");
        let one: Option<i64> = response.take(0).expect("int");
        assert_eq!(one, Some(1));

        let mut response = query!(db, "RETURN 2", {}).await.expect("ran");
        let two: Option<i64> = response.take(0).expect("int");
        assert_eq!(two, Some(2));
    }

    #[tokio::test]
    async fn query_runs_several_queries_with_shared_bindings() {
        let db = db().await;
        let mut response = query!(
            db,
            [
                "CREATE user:alice SET name = $name, skills = $skills",
                "SELECT VALUE skills FROM ONLY user:alice"
            ],
            {
                name: "Alice",
                skills: vec!["Rust", "SurrealQL"]
            }
        )
        .await
        .expect("ran");
        // The SDK reads an array result as rows, so the array is taken as a `Vec`.
        let skills: Vec<String> = response.take(1).expect("skills");
        assert_eq!(skills, ["Rust", "SurrealQL"]);
    }

    #[tokio::test]
    async fn query_binds_only_the_parameters_it_does_not_declare() {
        let db = db().await;
        // `$y` is the only parameter the query takes: the others are text, commented out or
        // declared by the query itself.
        let mut response = query!(
            db,
            [
                "RETURN '$price'; -- RETURN $commented;
                 LET $x = 1;
                 RETURN $x + $y;
                 LET $double = |$n: int| $n * 2;
                 RETURN $double(3);",
                "DEFINE FUNCTION fn::twice($v: int) { RETURN $v * 2; };
                 RETURN fn::twice($x);
                 RETURN [1, 2, 3].filter(|$item| $item > $x);"
            ],
            { y: 2 }
        )
        .await
        .expect("ran");
        let text: Option<String> = response.take(0).expect("string");
        assert_eq!(text.as_deref(), Some("$price"));
        let sum: Option<i64> = response.take(2).expect("int");
        assert_eq!(sum, Some(3));
        let doubled: Option<i64> = response.take(4).expect("int");
        assert_eq!(doubled, Some(6));
        let twice: Option<i64> = response.take(6).expect("int");
        assert_eq!(twice, Some(2));
        let filtered: Vec<i64> = response.take(7).expect("ints");
        assert_eq!(filtered, [2, 3]);
    }

    #[tokio::test]
    async fn query_binds_parameters_used_outside_their_declarations() {
        let db = db().await;
        // SurrealDB agrees with the macro: the closure's `$y` shadows the bound `$y` only inside
        // the closure, and `$total` is the bound value until its `LET`.
        let mut response = query!(
            db,
            "RETURN [1, 2].map(|$y| $y * 10);
             RETURN $y;
             RETURN $total;
             LET $total = 1;
             RETURN $total;",
            { y: 3, total: 0 }
        )
        .await
        .expect("ran");
        let mapped: Vec<i64> = response.take(0).expect("ints");
        assert_eq!(mapped, [10, 20]);
        let bound: Option<i64> = response.take(1).expect("int");
        assert_eq!(bound, Some(3));
        let before: Option<i64> = response.take(2).expect("int");
        assert_eq!(before, Some(0));
        let after: Option<i64> = response.take(4).expect("int");
        assert_eq!(after, Some(1));
    }
}

mod chain_macros {
    use super::*;

    #[test]
    fn chain_renders_lets_and_statements_in_order() {
        let user = &Table::from("user");
        let query = chain! {
            let name = "Alice";
            let name = select(All).from(user);
            select(All).from(user);
        };
        assert_eq!(
            query.to_debug_sql(),
            "LET $name = 'Alice';\n\nLET $name = (SELECT * FROM user);\n\nSELECT * FROM user;"
        );
        assert_eq!(
            query.to_query().expect("valid").sql(),
            "LET $name = $_p0;\n\nLET $name = (SELECT * FROM user);\n\nSELECT * FROM user;"
        );
        assert_parses(&query.to_debug_sql());
    }

    #[test]
    fn chain_with_begin_and_commit_is_a_transaction() {
        let user = &Table::from("user");
        let query = chain! {
            begin transaction;
            let name = "Alice";
            select(All).from(user);
            commit transaction;
        };
        assert_eq!(
            query.to_debug_sql(),
            "BEGIN TRANSACTION;\n\nLET $name = 'Alice';\n\nSELECT * FROM user;\n\nCOMMIT TRANSACTION;\n\t"
        );
        assert_parses(&query.to_debug_sql());

        let cancelled = transaction! {
            BEGIN TRANSACTION;
            select(All).from(user);
            CANCEL TRANSACTION;
        };
        assert!(
            cancelled
                .to_debug_sql()
                .ends_with("CANCEL TRANSACTION;\n\t")
        );
    }

    #[test]
    fn chain_with_return_is_a_block() {
        let user = &Table::from("user");
        let query = chain! {
            let name = "Alice";
            select(All).from(user);
            return name;
        };
        assert_eq!(
            query.to_debug_sql(),
            "{\nLET $name = 'Alice';\n\nSELECT * FROM user;\n\nRETURN $name;\n}"
        );
        let explicit = block! {
            let name = "Alice";
            select(All).from(user);
            return name;
        };
        assert_eq!(explicit, query);
    }

    #[test]
    fn if_else_and_for_render_inside_chains() {
        let age = Field::new("age");
        let query = chain! {
            if age.gte(18) {
                select(All).from(Table::from("adult"));
            } else if age.gte(13) {
                select(All).from(Table::from("teen"));
            } else {
                select(All).from(Table::from("child"));
            };
        };
        assert_eq!(
            query.to_debug_sql(),
            "IF age >= 18 \n\t{ SELECT * FROM adult; }\nELSE IF age >= 13 \n\t{ SELECT * FROM teen; }\
             \nELSE\n\t{ SELECT * FROM child; };"
        );
        assert_parses(&query.to_debug_sql());

        let query = chain! {
            for name in vec!["Alice", "Bob"] {
                select(All).from(Table::from("user")).where_(Field::new("name").equal(&name));
            };
        };
        assert_eq!(
            query.to_debug_sql(),
            "FOR $name IN ['Alice', 'Bob'] {\nSELECT * FROM user WHERE name = $name;\n};"
        );

        let standalone = for_!((name in vec!["Alice", "Bob"]) {
            select(All).from(Table::from("user")).where_(Field::new("name").equal(&name));
        };);
        assert_eq!(standalone.to_debug_sql(), query.to_debug_sql());
    }

    #[tokio::test]
    async fn loops_and_branches_run_in_the_database() {
        let db = db().await;
        let name = SpaceShip::schema().name;
        let query = chain! {
            for ship in vec!["Apollo", "Gemini"] {
                create::<SpaceShip>(SpaceShip::table()).set(name.equal_to(&ship));
            };
            let count = 3;
            if count.gt(2) {
                create::<SpaceShip>(SpaceShip::table()).set(name.equal_to("Mercury"));
            } else {
                create::<SpaceShip>(SpaceShip::table()).set(name.equal_to("Vostok"));
            };
        };
        query.run(db.clone()).await.expect("ran");
        assert_eq!(ships(&db).await, ["Apollo", "Gemini", "Mercury"]);
    }

    #[tokio::test]
    async fn a_transaction_moves_money_between_accounts() {
        let db = db().await;
        let one = &Account::id("one");
        let two = &Account::id("two");
        let amount = 300.0;
        let acc = Account::schema();

        let transfer = transaction! {
            BEGIN TRANSACTION;

            let balance = create_only(CreateTarget::table()).content(Balance {
                id: Balance::id("transfer"),
                amount,
            });

            create_only(CreateTarget::table()).content(Account { id: one.clone(), balance: 1_000.0 });
            create_only(CreateTarget::table()).content(Account { id: two.clone(), balance: 500.0 });

            update::<Account>(one).set(acc.balance.increment_by(balance.with_path::<Balance>(E).amount));
            update::<Account>(two).set(acc.balance.decrement_by(amount));

            COMMIT TRANSACTION;
        };
        transfer.run(db.clone()).await.expect("committed");

        let accounts: Vec<Account> = select(All)
            .from(one..=two)
            .decode()
            .execute(&db)
            .await
            .expect("accounts");
        assert_eq!(
            accounts
                .iter()
                .map(|account| account.balance)
                .collect::<Vec<_>>(),
            [1_300.0, 200.0]
        );
    }

    #[tokio::test]
    async fn a_block_computes_a_value_from_other_records() {
        let db = db().await;
        let weapon = &Weapon::table();
        let WeaponSchema { ref strength, .. } = Weapon::schema();
        let weapons = (0..=14)
            .map(|i| Weapon {
                name: format!("Weapon {i}"),
                strength: f64::from(i),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        insert(weapons).run(db.clone()).await.expect("inserted");

        let created = create::<WeaponStats>(WeaponStats::table()).set(object_partial!(WeaponStats {
            averageStrength: block! {
                let strengths = select_value(strength).from(weapon);
                let total = math::sum!(strengths);
                let count = umbriel::functions::array::len!(strengths);
                return math::ceil!((((total / count) * (count * total)) / (total + 4)) * 100);
            }
        }));
        assert_eq!(
            created.to_debug_sql(),
            "CREATE weapon_stats SET averageStrength = {\n\
             LET $strengths = (SELECT VALUE strength FROM weapon);\n\n\
             LET $total = math::sum($strengths);\n\n\
             LET $count = array::len($strengths);\n\n\
             RETURN math::ceil(((($total / $count) * ($count * $total)) / ($total + 4)) * 100);\n\
             };"
        );
        let stats = created.execute(&db).await.expect("created");
        assert_eq!(stats[0].average_strength, 10_115.0);
    }
}

mod definitions {
    use super::*;

    define_function!(double(amount: int) {
        return amount.multiply(2);
    });

    define_function!(ship_named(key: string, fallback: option<string>) {
        let ships = select(All).from(SpaceShip::table()).where_(SpaceShip::schema().name.equal(key));
        if ships.with_path::<SpaceShip>([0usize]).id.is_not(NONE) {
            return ships.with_path::<SpaceShip>([0usize]).name;
        } else {
            return fallback;
        };
    });

    #[test]
    fn functions_render_their_definition_and_calls() {
        assert_eq!(
            double_statement().to_debug_sql(),
            "DEFINE FUNCTION fn::double($amount: int) {\nRETURN $amount * 2;\n};"
        );
        assert_parses(&double_statement().to_debug_sql());
        assert_eq!(double!(21).to_debug_sql(), "fn::double(21)");
        assert_eq!(
            double!(21).to_query().expect("valid").sql(),
            "fn::double($_p0)"
        );
        assert_eq!(double!(Field::new("age")).to_debug_sql(), "fn::double(age)");
        assert_eq!(
            double_fn(Param::new("age")).to_debug_sql(),
            "fn::double($age)"
        );

        let definition = ship_named_statement().to_debug_sql();
        assert!(
            definition.starts_with(
                "DEFINE FUNCTION fn::ship_named($key: string, $fallback: none | string) {"
            ),
            "{definition}"
        );
        assert_parses(&definition);
        assert_eq!(
            ship_named!("Apollo", None::<umbriel::ValueLike>).to_debug_sql(),
            "fn::ship_named('Apollo', NONE)"
        );
    }

    #[tokio::test]
    async fn defined_functions_run() {
        let db = db().await;
        double_statement().run(db.clone()).await.expect("defined");
        let doubled: Option<i64> = return_(double!(21))
            .run(db.clone())
            .await
            .expect("ran")
            .take(0)
            .expect("int");
        assert_eq!(doubled, Some(42));
    }

    #[test]
    fn object_macros_assign_model_fields() {
        let all = object!(SpaceShip {
            id: SpaceShip::id("apollo"),
            name: "Apollo",
            created: Param::new("now"),
        });
        assert_sql(
            &create::<SpaceShip>(SpaceShip::table()).set(all),
            "CREATE space_ship SET id = space_ship:apollo, name = 'Apollo', created = $now;",
        );
        let some = object_partial!(SpaceShip { name: "Gemini" });
        assert_sql(
            &update::<SpaceShip>(SpaceShip::id("apollo")).set(some),
            "UPDATE space_ship:apollo SET name = 'Gemini';",
        );
    }

    #[test]
    fn let_macro_declares_a_parameter() {
        umbriel::statements::let_!(total = 5);
        assert_eq!(total.to_debug_sql(), "LET $total = 5;");
        assert_eq!(total.get_param().to_debug_sql(), "$total");
    }

    #[allow(dead_code)]
    #[derive(Pickable, Debug)]
    struct Person<'a, T: 'a, U: 'a> {
        name: String,
        age: u8,
        favourite: &'a T,
        other: &'a U,
    }

    pick!(PersonSummary, Person<'a,_,_> as PersonPickable, [name, age]);
    pick!(PersonWithGenerics, Person<'a,T,U> as PersonPickable, [name, age]);
    pick! {
        #[derive(Debug, PartialEq)]
        PersonName, Person<'a,_,_> as PersonPickable,
        [
            #[allow(dead_code)]
            name,
        ]
    }

    #[test]
    fn pick_copies_fields_with_their_types() {
        let summary = PersonSummary {
            name: "Alice".to_owned(),
            age: 30,
        };
        assert_eq!((summary.name.as_str(), summary.age), ("Alice", 30));
        let generic = PersonWithGenerics::<'_, u32, &str> {
            name: "Bob".to_owned(),
            age: 25,
        };
        assert_eq!((generic.name.as_str(), generic.age), ("Bob", 25));
        assert_eq!(
            PersonName {
                name: "Alice".to_owned()
            },
            PersonName {
                name: "Alice".to_owned()
            }
        );
    }
}

mod deprecated_macros {
    use super::*;

    #[test]
    fn block_deprecated_still_renders() {
        let user = &Table::from("user");
        let code = umbriel::statements::utils::block_deprecated! {
            let total = select_value(Field::new("age")).from(user);
            return total;
        };
        assert_eq!(
            code.to_debug_sql(),
            "{\nLET $total = (SELECT VALUE age FROM user);\n\nRETURN $total;\n}"
        );
    }
}
