async fn run(db: surrealdb::Surreal<surrealdb::engine::any::Any>) {
    // A placeholder without a binding.
    let _ = umbriel::query!(db, "RETURN $name").await;
    // A binding without a placeholder.
    let _ = umbriel::query!(db, "RETURN 1", { name: "Alice" }).await;
    // Generated parameter names are reserved.
    let _ = umbriel::query!(db, "RETURN $_p0", { _p0: 1 }).await;
}

fn main() {}
