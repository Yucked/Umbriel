//! `query!` and `query_raw!` (`umbriel-macros` `query`): queries checked at compile time, and
//! the parameters they bind.

use umbriel_tests::{support::macros::*, *};


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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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

/// `query_raw!` accepts a valid query.
mod compiles_query_raw {
#[test]
fn compiles() {
    let _query = umbriel::query_raw!("SELECT name, email FROM user WHERE age > 18");
}
}

/// `query!` requires bindings only for the parameters a query takes from outside: these calls
/// compile, which is the test, so `run` is never called.
#[expect(dead_code, reason = "compile-only: the checks happen at compile time")]
mod compiles_query_params {
async fn run(db: surrealdb::Surreal<surrealdb::engine::any::Any>) {
    // Text that looks like a parameter: in strings and comments.
    let _ = umbriel::query!(db, "RETURN \"$price\" + '$tax'; /* $note */ # $other").await;
    // Parameters the query declares.
    let _ = umbriel::query!(
        db,
        "LET $limit = 10;
         FOR $item IN [1, 2] { CREATE item SET rank = $item; };
         DEFINE PARAM $threshold VALUE 5;
         RETURN $limit + $threshold;"
    )
    .await;
    // Parameters SurrealDB sets.
    let _ = umbriel::query!(
        db,
        "SELECT *, (SELECT VALUE name FROM user WHERE manager = $parent.id) AS reports
         FROM user WHERE $this.age > 18 AND $auth = NONE"
    )
    .await;
    // A closure's parameter shadows a bound one only inside the closure.
    let _ = umbriel::query!(db, "RETURN [1].map(|$y| $y * 2); RETURN $y;", { y: 3 }).await;
    // A use before the `LET` takes the bound value.
    let _ = umbriel::query!(db, "RETURN $total; LET $total = 1; RETURN $total;", {
        total: 0
    })
    .await;
    // A declared parameter in one query of several.
    let _ = umbriel::query!(db, ["LET $name = 'Alice'", "RETURN $name + $suffix"], {
        suffix: "!"
    })
    .await;
}
}
