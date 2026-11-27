use umbriel::{Models, RecordId, model};

#[model(node, table = "log", table_prefix = "log_")]
pub struct Log {
    pub id: RecordId<Self>,
}

#[model(node, table = "log_audit")]
#[orm(relax_table)]
pub struct Audit {
    pub id: RecordId<Self>,
}

// A fixed table inside another variant's family.
#[derive(Models)]
pub enum FixedInFamily {
    Log(Log),
    Audit(Audit),
}

fn main() {}
