use umbriel::{
    Field,
    functions::{array, math, search, string, vector},
};

// `search` and `vector` results enter only the typed expressions of their kinds.
fn main() {
    let embedding = Field::new("embedding");
    // `vector::add` is an array of numbers, not a number.
    let _ = math::abs(vector::add(embedding.clone(), embedding.clone()));
    // `vector::magnitude` is a number, not an array.
    let _ = array::len(vector::magnitude(embedding));
    // `search::highlight` may be `NONE`, a string or an array of strings.
    let _ = string::len(search::highlight!("<b>", "</b>", 1));
    // `search::score` may be `NONE`.
    let _ = math::abs(search::score!(1));
}
