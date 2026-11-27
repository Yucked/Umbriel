//! Rendering (`orm/src/render.rs`): values bound in order and inlined for debugging, fragments
//! numbered per query, errors collected, and identifiers and keywords escaped.

use umbriel_tests::{support::query::*, *};

mod rendering {
    use super::*;

    #[test]
    fn values_are_bound_in_order_and_inlined_for_debugging() {
        let condition =
            cond(Field::new("name").equal("Alice")).and(Field::new("age").greater_than(30));
        let query = condition.to_query().expect("valid");
        assert_eq!(query.sql(), "(name = $_p0) AND (age > $_p1)");
        assert_eq!(
            query.vars().get("_p0"),
            Some(&Value::String("Alice".to_owned()))
        );
        assert_eq!(query.vars().get("_p1"), Some(&30i64.into_value()));
        assert_eq!(condition.to_debug_sql(), "(name = 'Alice') AND (age > 30)");
        assert_eq!(GENERATED_PARAM_PREFIX, "_p");

        let (sql, vars) = query.into_parts();
        assert_eq!(sql, "(name = $_p0) AND (age > $_p1)");
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn reused_fragments_are_numbered_per_query() {
        // One fragment used twice binds its value once per use, under its own name.
        let shared = Fragment::new(Field::new("age").greater_than(18));
        let statement = chain(return_(ValueLike::new(shared.clone())))
            .chain(return_(ValueLike::new(shared.clone())));
        let query = statement.to_query().expect("valid");
        assert_eq!(query.sql(), "RETURN age > $_p0;\n\nRETURN age > $_p1;");
        assert_eq!(query.vars().len(), 2);
        assert_eq!(query.vars().get("_p0"), Some(&18i64.into_value()));
        assert_eq!(query.vars().get("_p1"), Some(&18i64.into_value()));
        assert_eq!(statement.to_query().expect("valid"), query, "stable");

        // So does one condition shared by two statements.
        let adult = Field::new("age").greater_than(18);
        let statement = chain(select(All).from(Table::from("user")).where_(adult.clone()))
            .chain(select(All).from(Table::from("user")).where_(adult));
        let query = statement.to_query().expect("valid");
        assert_eq!(
            query.sql(),
            "SELECT * FROM user WHERE age > $_p0;\n\nSELECT * FROM user WHERE age > $_p1;"
        );
        assert_eq!(query.vars().get("_p0"), Some(&18i64.into_value()));
        assert_eq!(query.vars().get("_p1"), Some(&18i64.into_value()));

        assert_eq!(shared.clone(), shared);
        assert_eq!(shared, Fragment::new(Field::new("age").greater_than(18)));
        assert_eq!(format!("{shared:?}"), "Fragment(\"age > 18\")");
    }

    /// #33 11.6: fragments that write the same text but differ in validity are not equal.
    #[test]
    fn same_text_with_a_validation_error_is_a_different_fragment() {
        let checked = Fragment::new(umbriel::statements::remove_param("_p0"));
        let raw = Fragment::new(Raw::new("REMOVE PARAM $_p0;"));
        assert_eq!(checked.to_debug_sql(), raw.to_debug_sql(), "the same text");
        assert!(
            checked.to_query().is_err(),
            "the builder refuses the reserved name"
        );
        assert_ne!(checked, raw);
        assert_eq!(
            checked,
            Fragment::new(umbriel::statements::remove_param("_p0")),
            "the same text and the same error"
        );
    }

    #[test]
    fn raw_text_cannot_share_a_query_with_bound_values() {
        let raw = Raw::new("RETURN $_p0;");
        assert!(raw.to_query().is_ok());
        let mixed = chain(raw).chain(select(All).from(Table::from("user")).limit(1));
        let error = mixed.to_query().expect_err("rejected");
        assert!(
            error
                .to_string()
                .contains("raw SurrealQL cannot share a query")
        );
        assert_eq!(Raw::new("RETURN 1;").as_str(), "RETURN 1;");
        assert!(Raw::new("").is_empty());
        assert_eq!(Raw::new("RETURN 1;").to_string(), "RETURN 1;");
    }

    #[tokio::test]
    async fn literal_text_resembling_a_parameter_stays_a_value() {
        // A value spelled like a generated parameter is bound and quoted, never substituted.
        let condition = cond(Field::new("note").equal("$_p1")).and(Field::new("rank").equal(2));
        let query = condition.to_query().expect("valid");
        assert_eq!(query.sql(), "(note = $_p0) AND (rank = $_p1)");
        assert_eq!(
            query.vars().get("_p0"),
            Some(&Value::String("$_p1".to_owned()))
        );
        assert_eq!(query.vars().get("_p1"), Some(&2i64.into_value()));
        assert_eq!(condition.to_debug_sql(), "(note = '$_p1') AND (rank = 2)");

        let (db, _guard) = db().await;
        let bound: Value = return_("$_p0")
            .run(&db)
            .await
            .expect("ran")
            .take(0)
            .expect("value");
        assert_eq!(bound, Value::String("$_p0".to_owned()));
        let literal = ValueLike::value("$_p0".to_owned()).to_debug_sql();
        assert_eq!(literal, "'$_p0'");
        let inline: String = read(&db, &format!("RETURN {literal};")).await;
        assert_eq!(inline, "$_p0");
    }

    #[tokio::test]
    async fn nested_queries_number_their_values_in_order() {
        // A subquery inside a condition inside a statement, with values at every level.
        let statement = select_value(Field::new("name"))
            .from(Table::from("user"))
            .where_(
                cond(Field::new("age").greater_than(18)).and(
                    Field::new("team").inside(
                        select_value(Field::new("id"))
                            .from(Table::from("team"))
                            .where_(Field::new("name").equal("Acme Corp")),
                    ),
                ),
            )
            .limit(5);
        let query = statement.to_query().expect("valid");
        let sql = query.sql();
        let positions: Vec<usize> = ["$_p0", "$_p1", "$_p2"]
            .iter()
            .map(|name| sql.find(name).unwrap_or_else(|| panic!("{name} in {sql}")))
            .collect();
        assert!(positions.is_sorted(), "parameters in written order: {sql}");
        assert!(!sql.contains("$_p3"), "{sql}");
        assert_eq!(query.vars().len(), 3);
        assert_eq!(query.vars().get("_p0"), Some(&18i64.into_value()));
        assert_eq!(
            query.vars().get("_p1"),
            Some(&Value::String("Acme Corp".to_owned()))
        );
        assert_eq!(query.vars().get("_p2"), Some(&5i64.into_value()));
        // The same instance renders identically again.
        assert_eq!(statement.to_query().expect("valid"), query, "stable");
        assert_parses(sql);

        let (db, _guard) = db().await;
        Raw::new(
            "CREATE team:acme SET name = 'Acme Corp'; CREATE team:globex SET name = 'Globex'; \
             CREATE user:alice SET name = 'Alice', age = 30, team = team:acme; \
             CREATE user:bob SET name = 'Bob', age = 40, team = team:globex; \
             CREATE user:carol SET name = 'Carol', age = 12, team = team:acme;",
        )
        .run(&db)
        .await
        .expect("data");
        let names: Vec<String> = statement
            .run(&db)
            .await
            .expect("ran")
            .take(0)
            .expect("names");
        assert_eq!(names, ["Alice"]);
    }

    #[test]
    fn errors_of_every_node_are_reported_together() {
        let statement = select(All)
            .from(Table::from(""))
            .where_(Param::new("_p9").equal(1));
        let error = statement.to_query().expect_err("invalid").to_string();
        assert!(error.contains("table names cannot be empty"), "{error}");
        assert!(error.contains("reserved for bound values"), "{error}");
    }
}

mod names_and_keywords {
    use super::*;

    #[test]
    fn identifiers_escape_when_needed() {
        assert_eq!(Ident::new("plain").to_debug_sql(), "plain");
        assert_eq!(Ident::new("needs space").to_debug_sql(), "`needs space`");
        assert_eq!(Ident::new("tick`").to_debug_sql(), "`tick\\``");
        assert!(Ident::new("").to_query().is_err());
        assert_eq!(Namespace::new("acme").to_debug_sql(), "acme");
        assert_eq!(Database::from("acme-db").to_debug_sql(), "`acme-db`");
        assert_eq!(Event::from("changed").as_str(), "changed");
        assert_eq!(TableIndex::new("user_email").to_string(), "user_email");
        assert_eq!(Table::from("user").to_debug_sql(), "user");
        assert!(Table::from("").to_query().is_err());
        assert!(
            Table::from("space ship")
                .to_debug_sql()
                .contains("space ship")
        );
    }

    #[test]
    fn keywords() {
        assert_eq!(CrudType::Create.to_string(), "create");
        assert_eq!(CrudType::Select.to_debug_sql(), "select");
        assert_eq!(CrudType::Update.as_str(), "update");
        assert_eq!(CrudType::Delete.as_str(), "delete");
        assert_eq!(JwtAlgorithm::HS512.as_str(), "HS512");
        assert_eq!(JwtAlgorithm::EDDSA.to_string(), "EDDSA");
        assert_eq!(Interval::Day.to_string(), "day");
        assert_eq!(umbriel::Operator::Matches.as_str(), "@@");
    }

    #[test]
    fn record_id_values() {
        let id = types::RecordId::new("user", "alice");
        assert_eq!(ValueLike::from(id).to_debug_sql(), "user:alice");
        let escaped = ValueLike::from(User::id("display-name")).to_debug_sql();
        assert!(escaped.starts_with("user:") && escaped.contains("display-name"));
        assert_parses(&format!("RETURN {escaped};"));
    }
}
