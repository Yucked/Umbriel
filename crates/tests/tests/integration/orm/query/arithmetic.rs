//! Arithmetic (`orm/src/query/arithmetic.rs`): fields and operations combined with numbers.

use umbriel_tests::{support::query::*, *};


    #[test]
    fn fields_and_operations_combine_with_numbers() {
        let strength = Field::new("strength");
        assert_eq!((strength.clone() + 5).to_debug_sql(), "strength + 5");
        assert_eq!((strength.clone() - 5).to_debug_sql(), "strength - 5");
        assert_eq!((strength.clone() * 5).to_debug_sql(), "strength * 5");
        assert_eq!((strength.clone() / 5).to_debug_sql(), "strength / 5");
        assert_eq!((&strength + 5).to_debug_sql(), "strength + 5");
        assert_eq!(
            (strength.clone() + Field::new("bonus")).to_debug_sql(),
            "strength + bonus"
        );
        assert_eq!(
            ((strength.clone() + 5) * 2).to_debug_sql(),
            "(strength + 5) * 2"
        );
        assert_eq!(
            (strength.clone() * (Field::new("bonus") - 1)).to_debug_sql(),
            "strength * (bonus - 1)"
        );
        let total = let_("total").equal_to(10);
        assert_eq!((total.clone() + 1).to_debug_sql(), "$total + 1");
        assert_eq!((&total - 1).to_debug_sql(), "$total - 1");
        assert_eq!((total / 2).to_debug_sql(), "$total / 2");
    }

    #[test]
    fn schema_fields_take_part_in_arithmetic() {
        let rocket = Weapon::schema().rocket().strength;
        assert_eq!((rocket.clone() + 1).to_debug_sql(), "rocket.strength + 1");
        assert_eq!((&rocket * 2).to_debug_sql(), "rocket.strength * 2");
    }
