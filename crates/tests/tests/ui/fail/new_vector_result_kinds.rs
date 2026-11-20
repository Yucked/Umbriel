use umbriel::{
    Field,
    functions::{array, vector},
};

// `vector::sum` may be `NONE`, so its result is not an array.
fn main() {
    let embeddings = Field::new("embeddings");
    let _ = array::len!(vector::sum!(embeddings));
}
