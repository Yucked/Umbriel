use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    All, Field, Operatable, RecordId, SchemaGetter, SetterAssignable, SurrealCrud, Table, TableOf, model,
    object,
    statements::{
        SelectStatement, create, create_any_only, delete, delete_only, insert, relate, select, update,
        update_only, upsert, upsert_only,
    },
};

pub fn sales() -> SelectStatement {
    select(All).from(Table::new("sale"))
}

#[model(view, table = "sale_copy")]
#[orm(as_ = sales)]
pub struct SaleCopy {
    pub id: RecordId<Self>,
    pub amount: i64,
}

#[derive(umbriel::Models)]
#[models(read_only)]
pub enum Reports {
    Copy(SaleCopy),
}

// A view's records come from its query: every typed write is refused at compile time.
fn writes(db: &Surreal<Db>, copy: SaleCopy, id: RecordId<SaleCopy>) {
    // `SurrealCrud`.
    let _ = copy.save(db);
    let _ = SaleCopy::delete_by_id(db, &id);
    let _ = SaleCopy::patch(db, &id, ());
    let _ = SaleCopy::merge(db, &id, ());
    let _ = SaleCopy::insert_many(db, vec![]);
    let _ = SaleCopy::delete_where(Field::new("amount").is(1));

    // Statement builders.
    let _ = update::<SaleCopy>(id.clone());
    let _ = update_only::<SaleCopy>(id.clone());
    let _ = upsert::<SaleCopy>(id.clone());
    let _ = upsert_only::<SaleCopy>(id.clone());
    let _ = delete::<SaleCopy>(id.clone());
    let _ = delete_only::<SaleCopy>(id.clone());
    let _ = create::<SaleCopy>(SaleCopy::table());
    let _ = insert::<SaleCopy>(vec![]);
    let _ = relate::<SaleCopy>(Field::new("a->b->c"));

    // `TableOf`.
    let _ = TableOf::<SaleCopy>::default().insert_many(db, vec![]);
    let _ = TableOf::<SaleCopy>::default().delete_where(Field::new("amount").is(1));

    // Mixed-model writes.
    let _ = create_any_only::<Reports>(TableOf::<SaleCopy>::default());
    let _ = create_any_only::<Reports>(id.clone());
    let _ = create_any_only::<Reports>(&id);
    let _ = umbriel::statements::insert_any(Vec::<Reports>::new());

    // A view's fields take no assignment.
    let _ = SaleCopy::schema().amount.equal_to(1);
    let _ = object!(SaleCopy { id: id.clone(), amount: 1 });
}

fn main() {}
