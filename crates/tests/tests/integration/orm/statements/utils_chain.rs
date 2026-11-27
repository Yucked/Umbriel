//! Chains (`orm/src/statements/utils_chain.rs`): `chain!`, `block!`, `transaction!` and `for_!`
//! rendering statements in order, and running in the database.

use umbriel_tests::{support::macros::*, *};

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
        if age.greater_than_or_equal(18) {
            select(All).from(Table::from("adult"));
        } else if age.greater_than_or_equal(13) {
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
    let (db, _guard) = db().await;
    let name = SpaceShip::schema().name;
    let query = chain! {
        for ship in vec!["Apollo", "Gemini"] {
            create::<SpaceShip>(SpaceShip::table()).set(name.equal_to(&ship));
        };
        let count = 3;
        if count.greater_than(2) {
            create::<SpaceShip>(SpaceShip::table()).set(name.equal_to("Mercury"));
        } else {
            create::<SpaceShip>(SpaceShip::table()).set(name.equal_to("Vostok"));
        };
    };
    query.run(&db).await.expect("ran");
    assert_eq!(ships(&db).await, ["Apollo", "Gemini", "Mercury"]);
}

#[tokio::test]
async fn a_transaction_moves_money_between_accounts() {
    let (db, _guard) = db().await;
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
    transfer.run(&db).await.expect("committed");

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
    let (db, _guard) = db().await;
    let weapon = &Weapon::table();
    let WeaponSchema { ref strength, .. } = Weapon::schema();
    let weapons = (0..=14)
        .map(|i| Weapon {
            name: format!("Weapon {i}"),
            strength: f64::from(i),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    insert(weapons).run(&db).await.expect("inserted");

    let created = create::<WeaponStats>(WeaponStats::table()).set(object_partial!(WeaponStats {
        averageStrength: block! {
            let strengths = select_value(strength).from(weapon);
            let total = math::sum(strengths);
            let count = umbriel::functions::array::len(strengths);
            return math::ceil((((total / count) * (count * total)) / (total + 4)) * 100);
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
