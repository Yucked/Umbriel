use umbriel::{
    Model, RecordId, SchemaGetter, SetterNumeric, model,
    statements::{input_of, insert},
};

#[model(node, table = "page")]
pub struct Page {
    pub id: RecordId<Self>,
    pub title: String,
    pub visits: u32,
}

// `$input.title` is a `String`, which cannot increment a `u32`.
fn main() {
    let page = Page {
        id: Page::id("home"),
        title: "Home".to_owned(),
        visits: 1,
    };
    let PageSchema { visits, .. } = Page::schema();
    let _ = insert(page).on_duplicate_key_update(visits.increment_by(input_of::<Page>().title));
}
