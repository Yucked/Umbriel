use umbriel::Executor;

// Code generic over `Executor`, as every model method, `TableOf` method and `Function::run` is,
// cannot clone the SDK handle it borrows: `Executor` has no `Clone` bound, so it cannot open a
// new session.
fn clone_the_handle<E: Executor>(db: &E) -> E {
    db.clone()
}

fn main() {}
