use umbriel::{
    Field, StrandLike, cast,
    functions::{geo, string},
    kinds,
};

// A call's result enters only the typed expressions of its kind.
fn main() {
    let name = Field::new("name");
    // `string::len` is an `int`, not a geometry.
    let _ = geo::area!(string::len!(name.clone()));
    // An `int` cast is not a string.
    let _ = StrandLike::from(cast::<kinds::Int>(name));
}
