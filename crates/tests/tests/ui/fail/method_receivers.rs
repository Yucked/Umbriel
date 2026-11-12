use umbriel::{Field, NumberLike, StrandLike};

// Method forms exist only where SurrealDB dispatches them on that receiver.
fn main() {
    let score = NumberLike::from(Field::new("score"));
    // SurrealDB 3.3.4 has no `fixed` method on numbers, only `math::fixed`.
    let _ = score.fixed(2);
    // `uppercase` is a string method.
    let _ = score.uppercase();
    // `abs` is a number method.
    let _ = StrandLike::from(Field::new("name")).abs();
}
