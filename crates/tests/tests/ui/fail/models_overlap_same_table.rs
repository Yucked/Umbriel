use umbriel::{Models, RecordId, model};

#[model(node, table = "book")]
pub struct Book {
    pub id: RecordId<Self>,
}

#[model(node, table = "book")]
#[orm(relax_table)]
pub struct Novel {
    pub id: RecordId<Self>,
}

// Two equal fixed tables.
#[derive(Models)]
pub enum SameTable {
    Book(Book),
    Novel(Novel),
}

fn main() {}
