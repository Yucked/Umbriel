use umbriel::{All, Field, RecordId, Table, model, statements::{DefineFieldStatement, select}};

fn define_amount() -> DefineFieldStatement {
    unimplemented!()
}

// `as_` makes a table a read-only view, so a node cannot take it.
#[model(node, table = "sale")]
#[orm(as_ = select(All).from(Table::new("order")))]
pub struct Sale {
    pub id: RecordId<Self>,
}

// A view needs its query.
#[model(view, table = "sale_copy")]
pub struct SaleCopy {
    pub id: RecordId<Self>,
}

// `kind` is `any` or `normal`.
#[model(node, table = "customer")]
#[orm(kind = "relation")]
pub struct Customer {
    pub id: RecordId<Self>,
}

// `define` gives the whole definition, so each new clause conflicts with it.
#[model(node, table = "define_with_default_always")]
pub struct DefineWithDefaultAlways {
    pub id: RecordId<Self>,
    #[orm(ty = "int", define = define_amount(), default_always = 0)]
    pub amount: i64,
}

#[model(node, table = "define_with_value_expr")]
pub struct DefineWithValueExpr {
    pub id: RecordId<Self>,
    #[orm(ty = "int", define = define_amount(), value_expr = Field::new("amount"))]
    pub amount: i64,
}

#[model(node, table = "define_with_computed")]
pub struct DefineWithComputed {
    pub id: RecordId<Self>,
    #[orm(ty = "int", define = define_amount(), computed = Field::new("amount"))]
    pub amount: i64,
}

#[model(node, table = "define_with_readonly")]
pub struct DefineWithReadonly {
    pub id: RecordId<Self>,
    #[orm(ty = "int", define = define_amount(), readonly)]
    pub amount: i64,
}

#[model(node, table = "define_with_flexible")]
pub struct DefineWithFlexible {
    pub id: RecordId<Self>,
    #[orm(ty = "int", define = define_amount(), flexible)]
    pub amount: i64,
}

#[model(node, table = "define_with_comment")]
pub struct DefineWithComment {
    pub id: RecordId<Self>,
    #[orm(ty = "int", define = define_amount(), comment = "amount")]
    pub amount: i64,
}

// A computed field is evaluated on read, so it takes no write-side clause.
#[model(node, table = "computed_with_readonly")]
pub struct ComputedWithReadonly {
    pub id: RecordId<Self>,
    #[orm(computed = Field::new("amount"), readonly)]
    pub doubled: i64,
}

// One default per field.
#[model(node, table = "two_defaults")]
pub struct TwoDefaults {
    pub id: RecordId<Self>,
    #[orm(value = 0, default_always = 0)]
    pub amount: i64,
}

fn main() {}
