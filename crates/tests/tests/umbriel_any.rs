//! Mixed-model writes (`orm/src/statements/any.rs` and `#[derive(Models)]`): a `CREATE` or a
//! targetless `INSERT` over several models, each returned record decoded into its variant by the
//! table of its id. Targets and records outside the union fail.

use surrealdb::types::{self, SurrealNull, SurrealValue, Value};
use umbriel::{
    Model, Models, RecordId, Render, TableOf, model,
    statements::{AnyTarget, Returning, create_any, create_any_only, insert_any},
};
use umbriel_tests::{assert_parses, assert_render_error, db, read};

#[model(node, table = "book")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Book {
    pub id: RecordId<Self>,
    pub title: Option<String>,
}

#[model(node, table = "note", table_prefix = "note_")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Note {
    pub id: RecordId<Self>,
    pub text: Option<String>,
}

#[model(node, table = "tag")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tag {
    pub id: RecordId<Self>,
}

#[derive(Debug, Clone, PartialEq, Models)]
pub enum Item {
    Book(Book),
    Note(Note),
}

fn drafts() -> TableOf<Note> {
    Note::table_of("note_drafts").expect("a member")
}

#[test]
fn the_union_lists_its_members_and_targets_outside_it_fail() {
    assert_eq!(<Item as Models>::MEMBERS.len(), 2);
    assert!(Item::accepts_table("note_drafts"));
    assert!(!Item::accepts_table("tag"));

    let statement = create_any::<Item>([Book::id("rust").into(), drafts().into()]);
    assert_eq!(statement.to_debug_sql(), "CREATE book:rust, note_drafts;");
    assert_parses(&statement.to_debug_sql());

    let target: AnyTarget = Tag::id("t").into();
    assert_render_error(
        &create_any::<Item>([target]),
        "is not in a table of the union's models",
    );
}

#[test]
fn a_record_outside_the_union_does_not_decode() {
    let tag = Tag { id: Tag::id("t") }.into_value();
    assert!(Item::from_value(tag).is_err());
    let book = Book {
        id: Book::id("rust"),
        title: Some("Rust".to_owned()),
    };
    assert_eq!(
        Item::from_value(book.clone().into_value()).expect("decodes"),
        Item::Book(book)
    );
}

#[tokio::test]
async fn a_mixed_create_decodes_each_record_into_its_variant() {
    let db = db().await;
    let created = create_any::<Item>([Book::id("rust").into(), drafts().id("n1").into()])
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(created.len(), 2);
    assert!(created.contains(&Item::Book(Book {
        id: Book::id("rust"),
        title: None,
    })));
    assert!(created.contains(&Item::Note(Note {
        id: drafts().id("n1"),
        text: None,
    })));

    let only = create_any_only::<Item>(Note::id("n2"))
        .execute(&db)
        .await
        .expect("created");
    assert!(matches!(only, Some(Item::Note(note)) if note.id == Note::id("n2")));

    let nulls = create_any::<Item>([Book::id("go").into()])
        .return_null()
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(nulls, [SurrealNull]);
    let () = create_any::<Item>([Book::id("zig").into()])
        .return_none()
        .execute(&db)
        .await
        .expect("created");
    let diffs = create_any::<Item>([Book::id("c").into()])
        .return_diff()
        .execute(&db)
        .await
        .expect("created");
    assert_eq!(diffs.len(), 1);
}

#[tokio::test]
async fn a_mixed_targetless_insert_stores_each_record_in_its_table() {
    let db = db().await;
    let records = vec![
        Item::Book(Book {
            id: Book::id("rust"),
            title: Some("Rust".to_owned()),
        }),
        Item::Note(Note {
            id: drafts().id("n1"),
            text: Some("Alice's draft".to_owned()),
        }),
    ];
    let statement = insert_any(records.clone());
    assert_parses(&statement.to_debug_sql());
    let inserted = statement.execute(&db).await.expect("inserted");
    assert_eq!(inserted.len(), 2);
    for record in &records {
        assert!(inserted.contains(record), "{record:?}");
    }
    let tables: Vec<String> = read(
        &db,
        "RETURN [record::tb((SELECT VALUE id FROM ONLY book LIMIT 1)), \
         record::tb((SELECT VALUE id FROM ONLY note_drafts LIMIT 1))];",
    )
    .await;
    assert_eq!(tables, ["book", "note_drafts"]);

    // A duplicate is skipped under `IGNORE`.
    let ignored = insert_any(records)
        .ignore()
        .return_null()
        .execute(&db)
        .await
        .expect("ignored");
    assert!(ignored.is_empty());
}

/// The union's `SurrealValue` is an object holding a record.
#[test]
fn the_union_is_an_object_value() {
    assert_eq!(<Item as SurrealValue>::kind_of(), types::Kind::Object);
    assert!(!Item::is_value(&Value::None));
}
