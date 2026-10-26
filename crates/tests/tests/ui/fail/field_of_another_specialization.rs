use umbriel::{
    Model, RecordId, SchemaGetter, SetterAssignable, model, statements::update,
    types::SurrealValue, validators,
};

#[model(node, table = "planet")]
pub struct Planet<T: SurrealValue + validators::Int + 'static> {
    pub id: RecordId<Self>,
    #[orm(ty = "int")]
    pub data: T,
}

// Each specialization of a generic model has its own field types: `Planet<u32>`'s `data` is
// neither a field of `Planet<u64>` nor a value for its `data`.
fn main() {
    let _ = update::<Planet<u64>>(Planet::<u64>::id("earth"))
        .return_value(Planet::<u32>::schema().data);
    let _ = Planet::<u64>::schema()
        .data
        .equal_to(Planet::<u32>::schema().data);
}
