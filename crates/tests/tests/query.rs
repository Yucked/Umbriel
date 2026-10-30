//! Query building blocks: rendering and binding, fields and paths, filters, operators,
//! parameters, clauses, values, arithmetic, casts and names.

use std::time::Duration;

use surrealdb::types::{self, SurrealValue, Value};
use umbriel::{
    AliasName, Aliasable, All, CrudType, Database, E, EdgeDirection, Event, Field, Fragment,
    GENERATED_PARAM_PREFIX, Ident, Interval, JwtAlgorithm, Last, Model, NONE, NULL, Namespace,
    Node, NumberLike, Operatable, Param, ParamNameError, Raw, Render, Runnable, SchemaGetter,
    Table, TableIndex, ValueLike, any_other_edges, arr, bracket, cast, chain, cond, index,
    statements::{let_, return_, select, select_value},
    this, value, where_,
};
use umbriel_tests::*;

mod rendering {
    use super::*;

    #[test]
    fn values_are_bound_in_order_and_inlined_for_debugging() {
        let condition = cond(Field::new("name").equal("Alice")).and(Field::new("age").gt(30));
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
        let shared = Fragment::new(Field::new("age").gt(18));
        let statement = chain(return_(ValueLike::new(shared.clone())))
            .chain(return_(ValueLike::new(shared.clone())));
        let query = statement.to_query().expect("valid");
        assert_eq!(query.sql(), "RETURN age > $_p0;\n\nRETURN age > $_p1;");
        assert_eq!(query.vars().len(), 2);
        assert_eq!(query.vars().get("_p0"), Some(&18i64.into_value()));
        assert_eq!(query.vars().get("_p1"), Some(&18i64.into_value()));
        assert_eq!(statement.to_query().expect("valid"), query, "stable");

        // So does one condition shared by two statements.
        let adult = Field::new("age").gt(18);
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
        assert_eq!(shared, Fragment::new(Field::new("age").gt(18)));
        assert_eq!(format!("{shared:?}"), "Fragment(\"age > 18\")");
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

        let db = db().await;
        let bound: Value = return_("$_p0")
            .run(db.clone())
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
                cond(Field::new("age").gt(18)).and(
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

        let db = db().await;
        Raw::new(
            "CREATE team:acme SET name = 'Acme Corp'; CREATE team:globex SET name = 'Globex'; \
             CREATE user:alice SET name = 'Alice', age = 30, team = team:acme; \
             CREATE user:bob SET name = 'Bob', age = 40, team = team:globex; \
             CREATE user:carol SET name = 'Carol', age = 12, team = team:acme;",
        )
        .run(db.clone())
        .await
        .expect("data");
        let names: Vec<String> = statement
            .run(db.clone())
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

mod fields {
    use super::*;

    #[test]
    fn names_and_paths() {
        assert_eq!(Field::new("name").to_debug_sql(), "name");
        assert_eq!(
            Field::new("address").with_field("city").to_debug_sql(),
            "address.city"
        );
        assert_eq!(Field::new("display-name").to_debug_sql(), "`display-name`");
        assert_eq!(Field::from("name").to_debug_sql(), "name");
        assert_eq!(Field::new("tags").all().to_debug_sql(), "tags.*");
        assert_eq!(
            Field::from(Param::new("user")).all().to_debug_sql(),
            "$user.*"
        );
        assert_eq!(
            Field::from_expr(ValueLike::from(User::id("alice"))).to_debug_sql(),
            "user:alice"
        );
        assert!(Field::new("").to_query().is_err());
    }

    /// T28: a name is always one escaped identifier, whatever it holds, and verbatim idiom text
    /// is the tracked `Field::raw`.
    #[test]
    fn names_are_escaped_and_raw_fields_are_tracked() {
        assert_eq!(Field::new("select").to_debug_sql(), "`select`");
        assert_eq!(Field::new("address.city").to_debug_sql(), "`address.city`");
        assert_eq!(Field::new("time::now()").to_debug_sql(), "`time::now()`");
        assert_parses(&format!(
            "SELECT {} FROM user;",
            Field::new("select").to_debug_sql()
        ));

        assert_eq!(Field::raw("tags[*]").to_debug_sql(), "tags[*]");
        let raw_with_value = umbriel::statements::select(Field::raw("tags[*]"))
            .from(Table::from("user"))
            .where_(Field::new("name").equal("Alice"));
        assert!(raw_with_value.to_query().is_err());
        assert!(
            umbriel::statements::select(Field::raw("tags[*]"))
                .from(Table::from("user"))
                .to_query()
                .is_ok()
        );
    }

    #[test]
    fn aliases() {
        assert_eq!(
            Field::new("name").__as__("moniker").to_debug_sql(),
            "name AS moniker"
        );
        assert_eq!(
            Field::new("age")
                .gt(18)
                .__as__(AliasName::new("adult"))
                .to_debug_sql(),
            "age > 18 AS adult"
        );
        assert_eq!(
            select(All)
                .from(Table::from("user"))
                .__as__("everyone")
                .to_debug_sql(),
            "(SELECT * FROM user) AS everyone"
        );
        assert_eq!(AliasName::new("total").as_str(), "total");
        let alias = Field::new("name").__as__("moniker");
        assert_eq!(alias.get_alias_name().to_string(), "moniker");
    }

    #[test]
    fn array_elements() {
        let alien = Alien::schema();
        assert_eq!(alien.tags(Last).to_debug_sql(), "tags[$]");
        assert_eq!(alien.tags(All).to_debug_sql(), "tags[*]");
        assert_eq!(alien.tags(index(0)).to_debug_sql(), "tags[0]");
        assert_eq!(
            alien.tags(index(1)).to_query().expect("valid").sql(),
            "tags[$_p0]"
        );
        assert_eq!(
            alien.tags(index(Field::new("position"))).to_debug_sql(),
            "tags[position]"
        );
        assert_eq!(
            alien.tags(index(Param::new("position"))).to_debug_sql(),
            "tags[$position]"
        );
        assert_eq!(
            alien.tags(index(1)).greater_than(1).to_debug_sql(),
            "tags[1] > 1"
        );
        assert_eq!(
            alien.tags(where_(this().less_than(18))).to_debug_sql(),
            "tags[WHERE $this < 18]"
        );
        assert_eq!(
            alien
                .tags(where_(this().less_than(18)))
                .all()
                .to_debug_sql(),
            "tags[WHERE $this < 18].*"
        );
    }
}

mod params {
    use super::*;

    #[test]
    fn names_are_checked() {
        assert_eq!(Param::check("name"), Ok(()));
        assert_eq!(Param::check("_name"), Ok(()));
        assert_eq!(Param::check(""), Err(ParamNameError::Empty));
        assert_eq!(Param::check("1st"), Err(ParamNameError::InvalidCharacter));
        assert_eq!(
            Param::check("first-name"),
            Err(ParamNameError::InvalidCharacter)
        );
        assert_eq!(Param::check("_p0"), Err(ParamNameError::Reserved));
        assert_eq!(Param::check("_p"), Err(ParamNameError::Reserved));
        assert!(Param::try_from("_p1").is_err());
        assert_eq!(
            Param::try_from("name").map(|param| param.name().to_owned()),
            Ok("name".to_owned())
        );
        assert!(ParamNameError::Reserved.to_string().contains("reserved"));
        assert_eq!(Param::new("name").to_debug_sql(), "$name");
        assert!(Param::new("first-name").to_query().is_err());
    }

    #[test]
    fn standard_params() {
        assert_eq!(this().to_debug_sql(), "$this");
        assert_eq!(value().to_debug_sql(), "$value");
        assert_eq!(umbriel::auth().to_debug_sql(), "$auth");
        assert_eq!(umbriel::token().to_debug_sql(), "$token");
        assert_eq!(umbriel::session().to_debug_sql(), "$session");
        assert_eq!(umbriel::before().to_debug_sql(), "$before");
        assert_eq!(umbriel::after().to_debug_sql(), "$after");
        assert_eq!(umbriel::input().to_debug_sql(), "$input");
        assert_eq!(umbriel::parent().to_debug_sql(), "$parent");
        assert_eq!(umbriel::event().to_debug_sql(), "$event");
    }

    #[test]
    fn paths_from_params() {
        assert_eq!(
            this()
                .with_path::<Student>(index(2))
                .bestFriend()
                .bestFriend()
                .course()
                .title
                .to_debug_sql(),
            "$this[2].bestFriend.bestFriend.course.title"
        );
        assert_eq!(
            value()
                .with_path::<Student>([2usize])
                .firstName
                .to_debug_sql(),
            "$value[2].firstName"
        );
        let age = Student::schema().age;
        assert_eq!(
            this()
                .with_path::<Student>(where_(age.greater_than(18)))
                .bestFriend()
                .semesterCourses([5usize])
                .title
                .to_debug_sql(),
            "$this[WHERE age > 18].bestFriend.semesterCourses[5].title"
        );
        assert_eq!(
            this()
                .with_path::<Student>(All)
                .bestFriend()
                .semesterCourses(index(5))
                .title
                .to_debug_sql(),
            "$this[*].bestFriend.semesterCourses[5].title"
        );
        assert_eq!(
            this().with_path::<Student>(E).lastName.to_debug_sql(),
            "$this.lastName"
        );
        assert_eq!(
            Param::new("list").index(index(3)).to_debug_sql(),
            "$list[3]"
        );
        let students = let_("students").equal_to(select(All).from(Student::table()));
        assert_eq!(
            students.with_path::<Student>(E).firstName.to_debug_sql(),
            "$students.firstName"
        );
    }
}

mod filters {
    use super::*;

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
}

mod operators {
    use super::*;

    #[test]
    fn comparison_and_logic() {
        let age = Field::new("age");
        let cases = [
            (age.equal(25), "age = 25"),
            (age.not_equal(25), "age != 25"),
            (age.neq(25), "age != 25"),
            (age.exactly_equal(25), "age == 25"),
            (age.any_equal(25), "age ?= 25"),
            (age.all_equal(25), "age *= 25"),
            (age.less_than(25), "age < 25"),
            (age.lt(25), "age < 25"),
            (age.less_than_or_equal(25), "age <= 25"),
            (age.lte(25), "age <= 25"),
            (age.greater_than(25), "age > 25"),
            (age.gt(25), "age > 25"),
            (age.greater_than_or_equal(25), "age >= 25"),
            (age.gte(25), "age >= 25"),
            (age.truthy_and(25), "age && 25"),
            (age.truthy_or(25), "age || 25"),
            (age.and(25), "age AND 25"),
            (age.or(25), "age OR 25"),
            (age.is(25), "age IS 25"),
            (age.is_not(NONE), "age IS NOT NONE"),
            (age.is(NULL), "age IS NULL"),
            (age.equal(Field::new("limit")), "age = limit"),
            (age.equal(Param::new("limit")), "age = $limit"),
        ];
        for (operation, expected) in cases {
            assert_eq!(operation.to_debug_sql(), expected);
            assert_parses(&operation.to_debug_sql());
        }
    }

    #[test]
    fn arithmetic_operators() {
        let age = Field::new("age");
        let cases = [
            (age.add(5), "age + 5"),
            (age.plus(5), "age + 5"),
            (age.subtract(5), "age - 5"),
            (age.minus(5), "age - 5"),
            (age.multiply(5), "age * 5"),
            (age.mul(5), "age * 5"),
            (age.divide(5), "age / 5"),
            (age.div(5), "age / 5"),
            (age.power(2), "age ** 2"),
            (age.pow(2), "age ** 2"),
        ];
        for (operation, expected) in cases {
            assert_eq!(operation.to_debug_sql(), expected);
        }
    }

    #[test]
    fn containment_and_geometry() {
        let tags = Field::new("tags");
        let cases = [
            (tags.contains("rust"), "tags CONTAINS 'rust'"),
            (tags.contains_not("go"), "tags CONTAINSNOT 'go'"),
            (
                tags.contains_all(vec!["rust", "sql"]),
                "tags CONTAINSALL ['rust', 'sql']",
            ),
            (
                tags.contains_any(vec!["rust", "sql"]),
                "tags CONTAINSANY ['rust', 'sql']",
            ),
            (tags.contains_none(vec!["go"]), "tags CONTAINSNONE ['go']"),
            (tags.inside(vec!["rust"]), "tags INSIDE ['rust']"),
            (tags.in_(vec!["rust"]), "tags INSIDE ['rust']"),
            (tags.not_inside(vec!["rust"]), "tags NOTINSIDE ['rust']"),
            (tags.all_inside([1, 2]), "tags ALLINSIDE [1, 2]"),
            (tags.any_inside([1, 2]), "tags ANYINSIDE [1, 2]"),
            (tags.none_inside(vec![1, 2]), "tags NONEINSIDE [1, 2]"),
        ];
        for (operation, expected) in cases {
            assert_eq!(operation.to_debug_sql(), expected);
            assert_parses(&operation.to_debug_sql());
        }

        let location = Field::new("location");
        let area = geo::Polygon::new(
            geo::LineString::from(vec![
                (0.0, 0.0),
                (2.0, 0.0),
                (2.0, 2.0),
                (0.0, 2.0),
                (0.0, 0.0),
            ]),
            vec![],
        );
        for operation in [location.outside(area.clone()), location.intersects(area)] {
            assert_parses(&operation.to_debug_sql());
            assert_eq!(operation.to_query().expect("valid").vars().len(), 1);
        }
        assert!(
            location
                .outside(geo::Point::new(1.0, 1.0))
                .to_debug_sql()
                .starts_with("location OUTSIDE ")
        );
    }

    #[test]
    fn ranges_and_full_text_matches() {
        let age = Field::new("age");
        assert_eq!(
            age.between(18, 30).to_debug_sql(),
            "(18 < age AND age < 30)"
        );
        assert_eq!(
            age.within(18, 30).to_debug_sql(),
            "(18 <= age AND age <= 30)"
        );
        assert_eq!(
            age.between(Field::new("min"), Field::new("max"))
                .to_debug_sql(),
            "(min < age AND age < max)"
        );
        assert_parses(&age.between(18, 30).to_debug_sql());
        let content = Field::new("content");
        assert_eq!(
            content.matches("hello world").to_debug_sql(),
            "content @@ 'hello world'"
        );
        let with_ref = content.matches_with_ref(1, "hello world");
        assert_eq!(with_ref.to_debug_sql(), "content @1@ 'hello world'");
        assert_eq!(
            with_ref.to_query().expect("valid").sql(),
            "content @1@ $_p0"
        );
    }

    #[tokio::test]
    async fn operators_evaluate_as_surrealdb_defines_them() {
        let db = db().await;
        let tags = Field::from(Param::new("tags"));
        let value = |value: &'static str| Field::from_expr(ValueLike::value(value));
        let checks: Vec<(umbriel::Operation, bool)> = vec![
            (tags.contains("rust"), true),
            (tags.contains_not("go"), true),
            (tags.contains_all(vec!["rust", "sql"]), true),
            (tags.contains_any(vec!["go", "sql"]), true),
            (tags.contains_none(vec!["go"]), true),
            (tags.any_equal("rust"), true),
            (tags.all_equal("rust"), false),
            (value("rust").inside(tags.clone()), true),
            (Field::from_expr(ValueLike::value(5)).between(1, 10), true),
            (Field::from_expr(ValueLike::value(10)).within(1, 10), true),
            (Field::from_expr(ValueLike::value(10)).between(1, 10), false),
            (
                Field::from_expr(ValueLike::from(umbriel::GeometryLike::from(
                    geo::Point::new(1.0, 1.0),
                )))
                .inside(geo::Polygon::new(
                    geo::LineString::from(vec![
                        (0.0, 0.0),
                        (2.0, 0.0),
                        (2.0, 2.0),
                        (0.0, 2.0),
                        (0.0, 0.0),
                    ]),
                    vec![],
                )),
                true,
            ),
        ];
        for (operation, expected) in checks {
            let sql = format!(
                "LET $tags = ['rust', 'sql']; RETURN {};",
                operation.to_debug_sql()
            );
            let mut response = db
                .query(sql.clone())
                .await
                .and_then(surrealdb::IndexedResults::check)
                .expect("ran");
            let result: Option<bool> = response.take(1).expect("bool");
            assert_eq!(result, Some(expected), "{sql}");
        }
    }
}

mod clauses {
    use super::*;

    #[test]
    fn link_steps() {
        let student = Student::schema();
        assert_eq!(
            student.semesterCourses(E).title.to_debug_sql(),
            "semesterCourses.title"
        );
        assert_eq!(
            student.semesterCourses(All).title.to_debug_sql(),
            "semesterCourses[*].title"
        );
        assert_eq!(
            student.semesterCourses(Last).title.to_debug_sql(),
            "semesterCourses[$].title"
        );
        assert_eq!(
            student.semesterCourses(index(2)).title.to_debug_sql(),
            "semesterCourses[2].title"
        );
    }

    #[test]
    fn graph_steps() {
        let likes = Table::new("likes");
        let reads = Table::new("reads");
        let path = Student::with(Student::id("alice"))
            .writes__(any_other_edges(&[likes, reads]).where_(Field::new("count").lte(50)))
            .book(E);
        let path = Field::from(path).to_debug_sql();
        assert_eq!(
            path,
            "student:alice->(writes, likes, reads WHERE count <= 50)->book"
        );
        assert_parses(&format!("SELECT {path} FROM student;"));
        let to_book = Student::with(Student::id("alice"))
            .writes__(E)
            .book(Book::id("rust"));
        assert_eq!(
            Field::from(to_book).to_debug_sql(),
            "student:alice->writes->book:rust"
        );
        // A typed id of another model does not compile; an untyped one is checked on rendering.
        let wrong = Student::with(Student::id("alice"))
            .writes__(E)
            .book(types::RecordId::from(Blog::id("rust")));
        assert!(Field::from(wrong).to_query().is_err());
        assert_eq!(EdgeDirection::Out.as_arrow_symbol().to_debug_sql(), "->");
        assert_eq!(EdgeDirection::In.as_arrow_symbol().to_debug_sql(), "<-");
    }
}

mod values {
    use super::*;

    #[test]
    fn heterogeneous_arrays_and_keywords() {
        let values = arr![
            1,
            "two",
            Field::new("three"),
            Param::new("four"),
            NONE,
            NULL,
            Duration::from_secs(60)
        ];
        assert_eq!(
            values.iter().map(Render::to_debug_sql).collect::<Vec<_>>(),
            ["1", "'two'", "three", "$four", "NONE", "NULL", "1m"]
        );
        assert_eq!(ValueLike::from(All).to_debug_sql(), "*");
        assert_eq!(ValueLike::from(E).to_debug_sql(), "");
        assert!(ValueLike::from(E).is_empty());
        assert_eq!(
            ValueLike::from(select(All).from(Table::from("user"))).to_debug_sql(),
            "(SELECT * FROM user)"
        );
        assert_eq!(
            ValueLike::from(let_("total").equal_to(1)).to_debug_sql(),
            "$total"
        );
        assert_eq!(
            umbriel::array![1i64, "two"],
            vec![1i64.into_value(), Value::String("two".to_owned())]
        );
        assert_eq!(umbriel::val(true), Value::Bool(true));
    }

    #[test]
    fn typed_expressions() {
        assert_eq!(NumberLike::from(5).to_debug_sql(), "5");
        assert_eq!(NumberLike::from(Field::new("age")).to_debug_sql(), "age");
        assert_eq!(umbriel::StrandLike::from("Alice").to_debug_sql(), "'Alice'");
        assert_eq!(umbriel::BoolLike::from(true).to_debug_sql(), "true");
        assert_eq!(
            umbriel::ArrayLike::from(vec![1, 2]).to_debug_sql(),
            "[1, 2]"
        );
        assert_eq!(
            umbriel::ArrayLike::from(vec![ValueLike::from(1), ValueLike::from(Field::new("age"))])
                .to_debug_sql(),
            "[1, age]"
        );
        assert_eq!(
            umbriel::DurationLike::from(Duration::from_secs(90)).to_debug_sql(),
            "1m30s"
        );
        assert_eq!(
            umbriel::RecordIdLike::from(User::id("alice")).to_debug_sql(),
            "user:alice"
        );
        assert_eq!(
            umbriel::StrandLike::from(Interval::Week).to_debug_sql(),
            "'week'"
        );
        assert_eq!(
            bracket(Field::new("age").add(1)).to_debug_sql(),
            "(age + 1)"
        );
    }

    #[test]
    fn casts() {
        let cases = [
            (cast::bool("true"), "<bool> 'true'"),
            (cast::int("5"), "<int> '5'"),
            (cast::float("1.5"), "<float> '1.5'"),
            (cast::decimal("1.5"), "<decimal> '1.5'"),
            (cast::number("5"), "<number> '5'"),
            (cast::string(5), "<string> 5"),
            (cast::duration("1h"), "<duration> '1h'"),
            (
                cast::datetime("2020-01-01T00:00:00Z"),
                "<datetime> '2020-01-01T00:00:00Z'",
            ),
        ];
        for (cast, expected) in cases {
            assert_eq!(cast.to_debug_sql(), expected);
            assert_parses(&format!("RETURN {expected};"));
        }
    }

    #[tokio::test]
    async fn casts_convert_values() {
        let db = db().await;
        let int: Option<i64> =
            umbriel::statements::return_(Field::from_expr(cast::int("5")).add(1))
                .run(db.clone())
                .await
                .expect("ran")
                .take(0)
                .expect("int");
        assert_eq!(int, Some(6));
        let string: Option<String> = umbriel::statements::return_(cast::string(5))
            .run(db.clone())
            .await
            .expect("ran")
            .take(0)
            .expect("string");
        assert_eq!(string.as_deref(), Some("5"));
    }

    #[tokio::test]
    async fn inline_values_read_back_as_the_bound_values() {
        let db = db().await;
        let datetime =
            chrono::DateTime::<chrono::Utc>::from_timestamp(1_700_000_000, 0).expect("valid");
        let values = vec![
            42i64.into_value(),
            1.5f64.into_value(),
            "text".into_value(),
            true.into_value(),
            Duration::from_secs(90).into_value(),
            datetime.into_value(),
            User::id("alice").into_value(),
            geo::Point::new(1.0, 2.0).into_value(),
            vec![1i64, 2, 3].into_value(),
        ];
        for value in values {
            // Debug SQL writes the literal SurrealDB parses back to the same value.
            let sql = format!("RETURN {};", ValueLike::value(value.clone()).to_debug_sql());
            let inline: Value = read(&db, &sql).await;
            assert_eq!(inline, value, "{sql}");

            // Taken as a `Value`: an `Option` would read an array result as several rows.
            let bound: Value = umbriel::statements::return_(ValueLike::value(value.clone()))
                .run(db.clone())
                .await
                .expect("ran")
                .take(0)
                .expect("value");
            assert_eq!(bound, value);
        }
    }
}

mod arithmetic {
    use super::*;

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
