use umbriel::{Models, RecordId, model};

#[model(node, table = "log", table_prefix = "log_")]
pub struct Log {
    pub id: RecordId<Self>,
}

#[model(node, table = "archive", table_prefix = "log_old_")]
pub struct Archive {
    pub id: RecordId<Self>,
}

// Two families where one prefix starts with the other.
#[derive(Models)]
pub enum NestedPrefixes {
    Log(Log),
    Archive(Archive),
}

fn main() {}
