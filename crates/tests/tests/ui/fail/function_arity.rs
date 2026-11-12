use umbriel::{
    Field,
    functions::{math, string},
};

// A fixed-arity function takes exactly its arguments, and a variadic one at least one.
fn main() {
    let name = Field::new("name");
    let _ = string::len!();
    let _ = string::len!(name.clone(), name);
    let _ = math::fixed!(1);
    let _ = string::join!();
}
