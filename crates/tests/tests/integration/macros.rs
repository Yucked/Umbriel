//! The remaining exported macros (`umbriel-macros` `lib.rs`): `define_function!`,
//! `object!`/`object_partial!`, `let_!` and `pick!`.

mod models;
mod query;

use umbriel_tests::{support::macros::*, *};

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
        let (db, _guard) = db().await;
        double_statement().run(&db).await.expect("defined");
        let doubled: Option<i64> = return_(double!(21))
            .run(&db)
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
