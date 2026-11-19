// A declaration holds only in its scope, and only after it.
async fn run(db: surrealdb::Surreal<surrealdb::engine::any::Any>) {
    // A closure's parameter, outside the closure.
    let _ = umbriel::query!(db, "RETURN [1].map(|$n| $n * 2); RETURN $n;").await;
    // A function's argument, outside its body.
    let _ = umbriel::query!(
        db,
        "DEFINE FUNCTION fn::twice($v: int) { RETURN $v * 2; }; RETURN $v;"
    )
    .await;
    // A loop's parameter, after the loop.
    let _ = umbriel::query!(db, "FOR $i IN [1, 2] { CREATE item SET rank = $i; }; RETURN $i;").await;
    // A parameter used before its `LET`.
    let _ = umbriel::query!(db, "RETURN $total; LET $total = 1;").await;
    // A binding that only a closure's own parameter would use.
    let _ = umbriel::query!(db, "RETURN [1].map(|$y| $y);", { y: 2 }).await;
}

fn main() {}
