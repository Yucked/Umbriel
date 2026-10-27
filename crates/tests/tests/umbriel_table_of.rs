//! Table families (`orm/src/query/table_of.rs`): a model with a `table_prefix` is stored in tables
//! chosen at run time, and a record keeps its table through ids, statements, helpers, links and
//! graph paths. Tables outside the family fail before anything runs.

use surrealdb::types::{self, Kind, SurrealValue};
use umbriel::{
    E, Error, Field, LinkOne, Model, NONE, Node, Operatable, RecordId, Relate, Render,
    ReturnableStandard, SchemaGetter, SetterAssignable, SurrealCrud, SurrealCrudNode, TableOf,
    model,
    statements::{
        Returning, create, create_only, delete_only, insert, relate, update, upsert_only,
    },
};
use umbriel_tests::{assert_render_error, db, read};

#[model(node, table = "log", table_prefix = "log_")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Log {
    pub id: RecordId<Self>,
    pub message: String,
}

#[model(node, table = "app")]
#[derive(Debug, Clone, Default)]
pub struct App {
    pub id: RecordId<Self>,
    #[orm(link_one = "Log")]
    pub last: LinkOne<Log>,
    #[orm(relate(model = "AppWroteLog", connection = "->wrote->log"))]
    pub logs: Relate<Log>,
}

#[model(edge, table = "wrote")]
#[derive(Debug, Clone, Default)]
pub struct Wrote<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,
    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,
}

pub type AppWroteLog = Wrote<App, Log>;

fn october() -> TableOf<Log> {
    Log::table_of("log_2026_10").expect("a member")
}

fn entry(table: &TableOf<Log>, key: i64, message: &str) -> Log {
    Log {
        id: table.id(key),
        message: message.to_owned(),
    }
}

#[test]
fn members_are_the_default_table_and_longer_names_under_the_prefix() {
    assert_eq!(october().table().as_str(), "log_2026_10");
    assert_eq!(
        Log::table_of("log").expect("default").table().as_str(),
        "log"
    );
    // Names are escaped when rendered, so a member may hold any characters.
    for valid in ["log_a-b", "log_ä"] {
        assert!(Log::table_of(valid).is_ok(), "{valid:?}");
    }
    for invalid in ["log_", "audit", "logs", ""] {
        assert!(
            matches!(
                Log::table_of(invalid),
                Err(Error::TableNotInFamily { model: "log", .. })
            ),
            "{invalid:?}"
        );
    }
    assert!(
        App::table_of("app_x").is_err(),
        "a model without a prefix has one table"
    );
}

#[test]
fn ids_keep_their_member_and_decode_only_inside_the_family() {
    let id = october().id(1);
    assert_eq!(id.to_string(), "log_2026_10:1");
    assert_eq!(id.table_of(), october());

    let member = types::RecordId::new("log_b", 1);
    assert!(RecordId::<Log>::from_value(types::Value::RecordId(member)).is_ok());
    let outside = types::RecordId::new("audit", 1);
    assert!(RecordId::<Log>::from_value(types::Value::RecordId(outside)).is_err());

    // A prefix cannot be written as a SurrealQL kind.
    assert_eq!(RecordId::<Log>::kind_of(), Kind::Record(vec![]));
    assert_eq!(RecordId::<App>::kind_of(), Kind::Record(vec![App::table()]));
}

#[tokio::test]
async fn a_record_in_another_member_is_rejected_before_it_is_inserted() {
    let db = db().await;
    let november = Log::table_of("log_2026_11").expect("a member");
    let stray = entry(&november, 1, "Alice signed in");
    assert_render_error(
        &insert(stray.clone()).into_table(october()),
        "is not in the table",
    );
    assert!(matches!(
        october().insert_many(&db, vec![stray]).await,
        Err(Error::QueryBuilder(_))
    ));
}

/// `CONTENT` keeps the model's record id, so a record of another member, or another record, is
/// rejected when the statement is rendered rather than sent.
#[test]
fn content_in_another_member_is_rejected_before_it_runs() {
    let november = Log::table_of("log_2026_11").expect("a member");
    let stray = || entry(&november, 1, "Alice signed in");
    assert_render_error(
        &create::<Log>(october()).content(stray()),
        "is not in the target table `log_2026_10`",
    );
    assert_render_error(
        &create::<Log>(october().id(1)).content(stray()),
        "is not the target record",
    );
    assert_render_error(
        &update::<Log>(october()).content(stray()),
        "is not in the target table",
    );
    assert_render_error(
        &upsert_only::<Log>(october().id(1)).content(stray()),
        "is not the target record",
    );
    // The matching member renders.
    assert!(
        create::<Log>(november.clone())
            .content(stray())
            .to_query()
            .is_ok()
    );
}

#[tokio::test]
async fn a_member_supports_every_record_operation() {
    let db = db().await;
    let table = october();
    let schema = Log::schema();

    let created = create_only::<Log>(table.clone())
        .content(entry(&table, 1, "Alice signed in"))
        .execute(&db)
        .await
        .expect("created")
        .expect("returned");
    assert_eq!(created.id, table.id(1));

    table
        .insert_many(
            &db,
            vec![
                entry(&table, 2, "Bob signed in"),
                entry(&table, 3, "Bob left"),
            ],
        )
        .await
        .expect("inserted");
    insert(entry(&table, 4, "Alice left"))
        .into_table(table.clone())
        .return_none()
        .execute(&db)
        .await
        .expect("inserted");

    let read_back = Log::get(&db, &table.id(1))
        .await
        .expect("read")
        .expect("found");
    assert_eq!(read_back.id.table_of(), table);

    update::<Log>(table.id(1))
        .set(schema.message.equal_to("Alice signed out"))
        .return_none()
        .execute(&db)
        .await
        .expect("updated");
    upsert_only::<Log>(table.id(5))
        .content(entry(&table, 5, "Bob returned"))
        .execute(&db)
        .await
        .expect("upserted");

    let range = Log::get_range(&db, table.id(2)..table.id(4))
        .await
        .expect("range");
    assert_eq!(range.len(), 2);
    assert_eq!(table.count_all().get(db.clone()).await.expect("count"), 5);
    let found = table
        .find_where(schema.id.is_not(NONE))
        .return_many(db.clone())
        .await
        .expect("found");
    assert_eq!(found.len(), 5);
    assert!(found.iter().all(|log| log.id.table_of() == table));
    // Nothing was written to the default table, so it was never created.
    assert!(matches!(
        Log::count_all().get(db.clone()).await,
        Err(Error::TableNotFound { table, .. }) if table == "log"
    ));

    table
        .delete_where(schema.id.is_not(NONE))
        .execute(&db)
        .await
        .expect("deleted");
    assert_eq!(table.count_all().get(db.clone()).await.expect("count"), 0);
    let deleted = delete_only::<Log>(table.id(1))
        .return_before()
        .execute(&db)
        .await
        .expect("deleted nothing");
    assert!(deleted.is_none());
}

#[tokio::test]
async fn links_and_graph_paths_keep_the_member() {
    let db = db().await;
    let table = october();
    create_only::<Log>(table.id(1))
        .content(entry(&table, 1, "Alice signed in"))
        .return_none()
        .execute(&db)
        .await
        .expect("log");
    App {
        id: App::id("acme"),
        last: LinkOne::from(table.id(1)),
        ..Default::default()
    }
    .create(&db)
    .await
    .expect("app");
    let app = App::get(&db, &App::id("acme"))
        .await
        .expect("read")
        .expect("found");
    assert_eq!(app.last.key().map(RecordId::table_of), Some(table.clone()));

    let path: Field = App::with(App::id("acme"))
        .wrote__(E)
        .log(table.id(1))
        .into();
    assert_eq!(path.to_debug_sql(), "app:acme->wrote->log_2026_10:1");
    relate::<AppWroteLog>(path)
        .return_none()
        .execute(&db)
        .await
        .expect("related");
    let out: String = read(
        &db,
        "RETURN (SELECT VALUE record::tb(out) FROM ONLY wrote LIMIT 1);",
    )
    .await;
    assert_eq!(out, "log_2026_10");

    let outside: Field = App::with(App::id("acme"))
        .wrote__(E)
        .log(types::RecordId::new("audit", 1))
        .into();
    assert_render_error(
        &relate::<AppWroteLog>(outside),
        "does not belong to a table of `log`",
    );
}
