use umbriel::{
    Field, Merge, Patch, RecordId, SchemaGetter, SetterAssignable, model, object, object_partial,
    statements::update,
};

#[model(node, table = "sale")]
pub struct Sale {
    pub id: RecordId<Self>,
    pub amount: i64,
    #[orm(computed = Field::new("amount"))]
    pub doubled: i64,
}

// SurrealDB discards a value written to a computed field, so the generated write API has none.
fn writes(id: RecordId<Sale>) {
    let _ = SalePatch {
        doubled: Patch::Set(2),
        ..Default::default()
    };
    let _ = SaleMerge {
        doubled: Merge::Set(2),
        ..Default::default()
    };
    let _ = update::<Sale>(id.clone()).set(Sale::schema().doubled.equal_to(2));
    let _ = object!(Sale { id: id.clone(), amount: 1, doubled: 2 });
    let _ = object_partial!(Sale { doubled: 2 });
}

fn main() {}
