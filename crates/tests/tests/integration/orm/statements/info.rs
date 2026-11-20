//! `INFO` (`orm/src/statements/info.rs`, `info_result.rs`): every form's rendering, and the typed
//! results decoded from SurrealDB 3.3.4.

use chrono::{TimeZone, Utc};
use surrealdb::{
    Surreal,
    engine::any::Any,
    types::{Object, Value},
};
use umbriel::{
    Field, Param, Render, Runnable, Table, chain,
    statements::{
        DatabaseInfo, FunctionArgument, IndexBuildStatus, InfoOutput, Statements, Structures,
        TableInfo, define_analyzer, define_event, define_field, define_function, define_index,
        define_param, define_table, define_user, info_for, return_, select,
    },
};
use umbriel_tests::*;

async fn run(db: &Surreal<Any>, statement: impl Runnable + Sync) {
    statement.run(db).await.expect("ran");
}

mod rendering {
    use super::*;

    #[test]
    fn every_level_renders_in_both_forms() {
        assert_sql(&info_for().root(), "INFO FOR ROOT;");
        assert_sql(&info_for().root().structure(), "INFO FOR ROOT STRUCTURE;");
        assert_sql(&info_for().namespace(), "INFO FOR NS;");
        assert_sql(
            &info_for().namespace().structure(),
            "INFO FOR NS STRUCTURE;",
        );
        assert_sql(&info_for().database(), "INFO FOR DB;");
        assert_sql(&info_for().database().structure(), "INFO FOR DB STRUCTURE;");
        assert_sql(&info_for().table("user"), "INFO FOR TABLE user;");
        assert_sql(
            &info_for().table(Param::new("table")).structure(),
            "INFO FOR TABLE $table STRUCTURE;",
        );
    }

    #[test]
    fn users_and_indexes_render() {
        assert_sql(&info_for().user("alice"), "INFO FOR USER alice;");
        assert_sql(
            &info_for().user("alice").on_root(),
            "INFO FOR USER alice ON ROOT;",
        );
        assert_sql(
            &info_for().user("alice").on_namespace().structure(),
            "INFO FOR USER alice ON NAMESPACE STRUCTURE;",
        );
        assert_sql(
            &info_for().index("user_email", "user"),
            "INFO FOR INDEX user_email ON TABLE user;",
        );
    }

    #[test]
    fn version_precedes_structure_and_is_bound() {
        let at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let statement = info_for().database().version(at).structure();
        let debug = statement.to_debug_sql();
        assert_parses(&debug);
        assert!(debug.starts_with("INFO FOR DB VERSION "), "{debug}");
        assert!(debug.ends_with(" STRUCTURE;"), "{debug}");
        let query = statement.to_query().expect("renders");
        assert_eq!(query.sql(), "INFO FOR DB VERSION $_p0 STRUCTURE;");
    }
}

/// One of each kind `INFO FOR DB` and `INFO FOR TABLE` list.
async fn schema(db: &Surreal<Any>) {
    run(db, define_table("user").schemafull()).await;
    run(
        db,
        define_field(Field::new("email"))
            .on_table("user")
            .type_(surrealdb::types::Kind::String),
    )
    .await;
    run(
        db,
        define_index("user_email")
            .on_table("user")
            .fields(Field::new("email"))
            .unique(),
    )
    .await;
    run(
        db,
        define_event("audit")
            .on_table("user")
            .then(select(umbriel::All).from(Table::new("log"))),
    )
    .await;
    run(db, define_param(&Param::new("greeting")).value("Hello")).await;
    let x = Param::new("x");
    run(
        db,
        define_function("greet")
            .arguments(vec![FunctionArgument {
                name: x.clone(),
                type_: surrealdb::types::Kind::String,
            }])
            .body(chain(return_(Field::from(&x)))),
    )
    .await;
    run(db, define_analyzer("simple")).await;
    run(
        db,
        define_user("alice")
            .on_database()
            .password("example-password"),
    )
    .await;
}

mod results {
    use super::*;

    #[tokio::test]
    async fn database_info_decodes_in_both_forms() {
        let (db, _guard) = db().await;
        schema(&db).await;

        let plain: DatabaseInfo<Statements> =
            info_for().database().execute(&db).await.expect("decoded");
        assert!(plain.tables["user"].starts_with("DEFINE TABLE user"));
        assert!(plain.params.contains_key("greeting"));
        assert!(plain.analyzers.contains_key("simple"));
        assert!(plain.users.contains_key("alice"));
        // Function keys have no `fn::`.
        assert!(
            plain.functions.contains_key("greet"),
            "{:?}",
            plain.functions
        );
        assert!(plain.other.is_empty(), "{:?}", plain.other);

        let structured: DatabaseInfo<Structures> = info_for()
            .database()
            .structure()
            .execute(&db)
            .await
            .expect("decoded");
        assert_eq!(structured.tables.len(), 1);
        assert_eq!(
            structured.tables[0].get("name"),
            Some(&Value::String("user".to_owned()))
        );
        assert!(structured.other.is_empty(), "{:?}", structured.other);
    }

    #[tokio::test]
    async fn table_info_decodes_in_both_forms() {
        let (db, _guard) = db().await;
        schema(&db).await;

        let plain: TableInfo<Statements> = info_for()
            .table("user")
            .execute(&db)
            .await
            .expect("decoded");
        assert!(plain.fields.contains_key("email"));
        assert!(plain.indexes.contains_key("user_email"));
        assert!(plain.events.contains_key("audit"));
        assert!(plain.lives.is_empty());
        assert!(plain.tables.is_empty());

        let structured: TableInfo<Structures> = info_for()
            .table("user")
            .structure()
            .execute(&db)
            .await
            .expect("decoded");
        assert_eq!(structured.fields.len(), 1);
        assert_eq!(structured.indexes.len(), 1);
        assert_eq!(structured.events.len(), 1);
    }

    #[tokio::test]
    async fn root_and_namespace_info_decode() {
        let (db, guard) = db().await;
        let root = info_for().root().execute(&db).await.expect("decoded");
        assert!(
            root.namespaces.contains_key(guard.namespace()),
            "{:?}",
            root.namespaces
        );
        assert!(!root.system.is_empty());
        let root = info_for()
            .root()
            .structure()
            .execute(&db)
            .await
            .expect("decoded");
        assert!(!root.namespaces.is_empty());

        let namespace = info_for().namespace().execute(&db).await.expect("decoded");
        assert!(namespace.databases.contains_key(guard.database()));
        let namespace = info_for()
            .namespace()
            .structure()
            .execute(&db)
            .await
            .expect("decoded");
        assert!(!namespace.databases.is_empty());
    }

    #[tokio::test]
    async fn user_info_is_one_statement_or_one_object() {
        let (db, _guard) = db().await;
        schema(&db).await;
        let sql = info_for()
            .user("alice")
            .on_database()
            .execute(&db)
            .await
            .expect("decoded");
        assert!(sql.starts_with("DEFINE USER alice ON DATABASE"), "{sql}");
        let structure: Object = info_for()
            .user("alice")
            .on_database()
            .structure()
            .execute(&db)
            .await
            .expect("decoded");
        assert_eq!(
            structure.get("name"),
            Some(&Value::String("alice".to_owned()))
        );
    }

    #[tokio::test]
    async fn index_info_is_its_build_status() {
        let (db, _guard) = db().await;
        schema(&db).await;
        let status = info_for()
            .index("user_email", "user")
            .execute(&db)
            .await
            .expect("decoded");
        assert_eq!(status.status, IndexBuildStatus::Ready);
        assert_eq!(status.error, None);
        assert!(status.initial.is_some() && status.pending.is_some() && status.updated.is_some());
        // The key set 3.3.4 gives an index built by `DEFINE INDEX`: the build counters too.
        let raw: Value = read(&db, "INFO FOR INDEX user_email ON TABLE user;").await;
        let Value::Object(raw) = raw else {
            panic!("an object");
        };
        let Some(Value::Object(building)) = raw.get("building") else {
            panic!("`building` is an object");
        };
        let mut keys: Vec<&str> = building.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["compacting", "initial", "pending", "status", "updated"]
        );
    }

    #[test]
    fn keys_the_types_do_not_name_are_kept() {
        let mut info = Object::new();
        for key in ["events", "fields", "indexes", "lives", "tables"] {
            info.insert(key.to_owned(), Value::Array(Default::default()));
        }
        info.insert("graphs".to_owned(), Value::Array(Default::default()));
        let table = TableInfo::<Structures>::from_info(Value::Object(info)).expect("decoded");
        assert!(table.other.contains_key("graphs"));
    }

    #[test]
    fn a_missing_key_is_an_error() {
        let mut info = Object::new();
        info.insert("events".to_owned(), Value::Array(Default::default()));
        assert!(TableInfo::<Structures>::from_info(Value::Object(info)).is_err());
    }

    /// `BEGIN`, `COMMIT` and `CANCEL` in a function body are not transaction statements: 3.3.4
    /// parses a body's statements as expressions, so the keywords read as plain values, the
    /// definition is accepted, and the body runs in the caller's transaction.
    #[tokio::test]
    async fn begin_inside_a_function_body_is_no_transaction() {
        let sql = "DEFINE FUNCTION fn::transfer() { BEGIN; CREATE log:1; CANCEL; RETURN 1; };";
        assert!(surrealdb_syn::parse(sql).is_ok());
        let (db, _guard) = db().await;
        assert!(succeeds(&db, sql).await);
        let result: i64 = read(&db, "RETURN fn::transfer();").await;
        assert_eq!(result, 1);
        // `CANCEL` cancelled nothing: the write inside the body is kept.
        let kept: usize = read(&db, "RETURN count(SELECT * FROM log);").await;
        assert_eq!(kept, 1);
    }
}

/// `IndexStatus` for indexes built in the background (`CONCURRENTLY`).
mod index_builds {
    use super::*;
    use std::time::Duration;
    use umbriel::{
        Raw,
        statements::{IndexStatus, hnsw},
    };

    /// The index's status once its build has stopped running. The whole polling, every request
    /// included, is bounded by [`WAIT`].
    async fn settled(db: &Surreal<Any>, index: &str, table: &str) -> IndexStatus {
        let polling = async {
            loop {
                let status = info_for()
                    .index(index, table)
                    .execute(db)
                    .await
                    .expect("decoded");
                if matches!(
                    status.status,
                    IndexBuildStatus::Ready | IndexBuildStatus::Error | IndexBuildStatus::Aborted
                ) {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        tokio::time::timeout(WAIT, polling)
            .await
            .unwrap_or_else(|_| panic!("`{index}` is still building after {WAIT:?}"))
    }

    #[tokio::test]
    async fn a_concurrent_build_reports_its_progress_until_ready() {
        let (db, _guard) = db().await;
        run(
            &db,
            Raw::new("FOR $n IN 1..=500 { CREATE user SET email = <string> $n + '@example.com' };"),
        )
        .await;
        run(
            &db,
            define_index("user_email")
                .on_table(Table::from("user"))
                .fields(Field::new("email"))
                .concurrently(),
        )
        .await;
        let status = settled(&db, "user_email", "user").await;
        assert_eq!(status.status, IndexBuildStatus::Ready, "{status:?}");
        assert_eq!(status.error, None);
        assert!(status.initial.is_some(), "{status:?}");
        let indexed: i64 = read(
            &db,
            "RETURN array::len(SELECT id FROM user WITH INDEX user_email WHERE email != NONE);",
        )
        .await;
        assert_eq!(indexed, 500);
    }

    /// The failure trigger: records whose embeddings do not have the index's dimension, which a
    /// background build meets only after the statement has returned.
    #[tokio::test]
    async fn a_failed_build_reports_error_and_its_message() {
        let (db, _guard) = db().await;
        run(&db, Raw::new("CREATE doc:wrong SET embedding = [1, 2, 3];")).await;
        run(
            &db,
            define_index("doc_embedding")
                .on_table(Table::from("doc"))
                .fields(Field::new("embedding"))
                .hnsw(hnsw(2))
                .concurrently(),
        )
        .await;
        let status = settled(&db, "doc_embedding", "doc").await;
        assert_eq!(status.status, IndexBuildStatus::Error, "{status:?}");
        assert!(
            status
                .error
                .as_deref()
                .is_some_and(|error| !error.is_empty()),
            "{status:?}"
        );
    }
}
