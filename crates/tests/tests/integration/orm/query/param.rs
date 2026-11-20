//! Parameters (`orm/src/query/param.rs`): how long a `LET` parameter lives (Q36), and the session
//! values reserved parameters stand for (Q35).

use surrealdb::types::Value;
use umbriel::{
    Param, Runnable,
    statements::{let_, return_},
};
use umbriel_tests::{support::query::*, *};

/// A `LET` parameter lives for its own query: 3.3.4 binds a top-level `LET` into the
/// request-scoped context (`dbs/executor.rs`), and only the connection's `set` values are merged
/// into each query.
#[tokio::test]
async fn a_let_parameter_lives_for_its_query() {
    let (db, _guard) = db().await;
    let_("greeting")
        .equal_to("hello")
        .run(&db)
        .await
        .expect("set");
    let mut results = return_(Param::new("greeting")).run(&db).await.expect("ran");
    let greeting: Option<String> = results.take(0).expect("a value");
    assert_eq!(greeting, None);

    db.set("greeting", "hello")
        .await
        .expect("set on the connection");
    let mut results = return_(Param::new("greeting")).run(&db).await.expect("ran");
    let greeting: Option<String> = results.take(0).expect("a value");
    assert_eq!(greeting.as_deref(), Some("hello"));
}

/// A root session has no record access: the session fills `rd` only from a signed-in record
/// (3.3.4 `dbs/session.rs`).
#[tokio::test]
async fn a_root_session_has_no_record() {
    let (db, _guard) = db().await;
    let rd: Value = read(&db, "RETURN session::rd();").await;
    assert_eq!(rd, Value::None);
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
