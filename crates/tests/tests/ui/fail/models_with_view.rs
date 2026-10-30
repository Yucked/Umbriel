use umbriel::{All, Models, RecordId, Table, model, statements::{SelectStatement, select}};

pub fn sales() -> SelectStatement {
    select(All).from(Table::new("sale"))
}

#[model(node, table = "sale")]
pub struct Sale {
    pub id: RecordId<Self>,
}

#[model(view, table = "sale_copy")]
#[orm(as_ = sales)]
pub struct SaleCopy {
    pub id: RecordId<Self>,
}

// `insert_any` writes every variant, so a union holding a view must be `read_only`.
#[derive(Models)]
pub enum Records {
    Sale(Sale),
    Copy(SaleCopy),
}

// Any other `models` setting is refused.
#[derive(Models)]
#[models(writable)]
pub enum Unknown {
    Sale(Sale),
}

fn main() {}
