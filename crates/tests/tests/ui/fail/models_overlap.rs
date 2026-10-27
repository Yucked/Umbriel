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

#[model(node, table = "log", table_prefix = "log_")]
pub struct Log {
    pub id: RecordId<Self>,
}

#[model(node, table = "log_audit")]
#[orm(relax_table)]
pub struct Audit {
    pub id: RecordId<Self>,
}

#[model(node, table = "archive", table_prefix = "log_old_")]
pub struct Archive {
    pub id: RecordId<Self>,
}

// Two equal fixed tables.
#[derive(Models)]
pub enum SameTable {
    Book(Book),
    Novel(Novel),
}

// A fixed table inside another variant's family.
#[derive(Models)]
pub enum FixedInFamily {
    Log(Log),
    Audit(Audit),
}

// Two families where one prefix starts with the other.
#[derive(Models)]
pub enum NestedPrefixes {
    Log(Log),
    Archive(Archive),
}

fn main() {}
