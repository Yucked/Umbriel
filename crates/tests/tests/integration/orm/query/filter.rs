//! Filters (`orm/src/query/filter.rs`): conditions grouped in the order they are chained, and
//! `cond!`.

use umbriel_tests::support::query::*;


    #[test]
    fn conditions_group_in_the_order_they_are_chained() {
        let age = Field::new("age");
        let name = Field::new("name");
        let title = Field::new("title");
        assert_eq!(cond(age.gt(18)).to_debug_sql(), "age > 18");
        assert_eq!(
            cond(age.gt(18))
                .and(name.contains("Alice"))
                .or(title.equal("Professor"))
                .to_debug_sql(),
            "((age > 18) AND (name CONTAINS 'Alice')) OR (title = 'Professor')"
        );
        let either = cond(age.gt(18))
            .or(title.equal("Professor"))
            .parenthesized();
        assert_eq!(
            either.to_debug_sql(),
            "((age > 18) OR (title = 'Professor'))"
        );
        assert_eq!(
            cond(either.clone()).and(name.is("Alice")).to_debug_sql(),
            "((age > 18) OR (title = 'Professor')) AND (name IS 'Alice')"
        );
        assert_eq!(umbriel::Filter::default().to_debug_sql(), "");
    }

    #[test]
    fn the_cond_macro_builds_the_same_filters() {
        let age = Field::new("age");
        let name = Field::new("name");
        let title = Field::new("title");
        assert_eq!(umbriel::cond!(age > 18).to_debug_sql(), "age > 18");
        assert_eq!(
            umbriel::cond!((age > 18) AND (name = "Alice") OR (title == "Professor"))
                .to_debug_sql(),
            cond(age.gt(18))
                .and(name.equal("Alice"))
                .or(title.exactly_equal("Professor"))
                .to_debug_sql()
        );
        assert_eq!(
            umbriel::cond!((age >= 18) AND (age <= 120)).to_debug_sql(),
            "(age >= 18) AND (age <= 120)"
        );
        assert_eq!(
            umbriel::cond!((name CONTAINS "Al") OR (age < 5)).to_debug_sql(),
            "(name CONTAINS 'Al') OR (age < 5)"
        );
    }
