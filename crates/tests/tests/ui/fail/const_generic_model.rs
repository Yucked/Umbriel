use umbriel::{RecordId, model};

// Regression (UMB-029): a const parameter panicked the derive; it is a spanned error.
#[model(node, table = "grid")]
pub struct Grid<const N: usize> {
    pub id: RecordId<Self>,
    pub cells: Vec<u8>,
}

fn main() {}
