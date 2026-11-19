// `query!` requires bindings only for the parameters a query takes from outside.
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

fn main() {}
