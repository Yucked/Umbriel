use umbriel::{
    All, Field, Models, Operatable, RecordId, SchemaGetter, SurrealRead, Table, TableOf, model,
    statements::{SelectStatement, insert_any, select},
};

// Every field clause the derive offers.
#[model(node, table = "sale")]
#[orm(kind = "normal", comment = "Acme Corp sales")]
pub struct Sale {
    pub id: RecordId<Self>,
    pub amount: i64,
    #[orm(computed = Field::new("amount"))]
    pub doubled: i64,
    #[orm(default_always = 0)]
    pub refunds: i64,
    #[orm(value_expr = Field::new("amount"))]
    pub recorded: i64,
    #[orm(readonly, comment = "set once")]
    pub created_by: String,
    #[orm(ty = "object", flexible)]
    pub meta: umbriel::types::Object,
}

#[model(node, table = "customer")]
pub struct Customer {
    pub id: RecordId<Self>,
    pub name: String,
}

pub fn sales() -> SelectStatement {
    select(All).from(Table::new("sale"))
}

// A view: read through every read method, written through none.
#[model(view, table = "sale_copy")]
#[orm(as_ = sales)]
pub struct SaleCopy {
    pub id: RecordId<Self>,
    pub amount: i64,
}

// A union holding a view decodes reads.
#[derive(Models)]
#[models(read_only)]
pub enum Reports {
    Sale(Sale),
    Copy(SaleCopy),
}

// A union of writable models is inserted as before.
#[derive(Models)]
pub enum Records {
    Sale(Sale),
    Customer(Customer),
}

fn insert(records: Vec<Records>) {
    let _ = insert_any(records);
}

fn main() {
    let _ = SaleCopy::select();
    let _ = SaleCopy::find_where(Field::new("amount").gte(1));
    let _ = SaleCopy::count_all();
    let _ = SaleCopy::select_value(SaleCopy::schema().amount);
    let _ = TableOf::<SaleCopy>::default().select();
    let _ = Sale::select_value(Sale::schema().doubled);
    let _: SelectStatement<Reports> = select(All).from(Table::new("sale")).decode();
    insert(vec![]);
}
