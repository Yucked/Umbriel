//! Against a running SurrealDB server, over each supported remote transport (HTTP and
//! WebSocket): connecting, model definitions, CRUD, relations, and statement and transaction
//! error handling. The other test files use the in-memory engine; these check the same behaviour
//! survives the network round trip.
//!
//! Ignored by default. Run them against a server you started with
//! `cargo test -p umbriel-tests --test server -- --ignored`, configured by:
//!
//! - `UMBRIEL_TEST_HTTP_URL`, e.g. `http://127.0.0.1:8000`
//! - `UMBRIEL_TEST_WS_URL`, e.g. `ws://127.0.0.1:8000`
//! - `UMBRIEL_TEST_USER`, `UMBRIEL_TEST_PASS`: a root user
//! - `UMBRIEL_TEST_NS`, `UMBRIEL_TEST_DB`: the namespace and database to use
//!
//! The tests write only to the tables `server_person`, `server_company` and `server_works_at`,
//! which they remove before and after each test, and run one at a time. `cloned_sessions` also
//! defines one database user named `server_reader_<random>`, refusing a name that exists, and
//! removes only that user once it has created it.
//!
//! The `versioned_*` tests also need the server's store to keep versions (SurrealKV started with
//! `versioned=true`), since the memory engine refuses `SELECT … VERSION`. The `eval_*` tests
//! need a server started with `--allow-eval-query`, which `--allow-all` does not include. SDK
//! transaction handles run over WebSocket only; over HTTP the test checks `begin` is refused (as
//! a method not found).

use std::collections::BTreeMap;

use chrono::TimeZone;
use surrealdb::{
    IndexedResults, Surreal,
    engine::any::{self, Any},
    opt::auth::{Database, Root},
    types::{self, NotFoundError, SurrealValue, Value},
};
use umbriel::{
    All, E, Error, LinkOne, Model, Node, Operatable, Patch, Raw, RecordId, Relate, Retry, Runnable,
    SchemaGetter, SetterAssignable, SurrealCrud, SurrealCrudNode, SurrealRead,
    functions::eval,
    model,
    statements::{Returning, info_for, relate, return_, select, update},
};
use umbriel_tests::{read, statement_error};

#[model(node, table = "server_person")]
#[derive(Debug, Clone, Default)]
pub struct ServerPerson {
    pub id: RecordId<Self>,
    pub name: String,
    pub age: u8,
    pub nickname: Option<String>,
    #[orm(relate(
        model = "PersonWorksAtCompany",
        connection = "->server_works_at->server_company"
    ))]
    pub employers: Relate<ServerCompany>,
}

#[model(node, table = "server_company")]
#[derive(Debug, Clone, Default)]
pub struct ServerCompany {
    pub id: RecordId<Self>,
    pub name: String,
}

#[model(edge, table = "server_works_at")]
#[derive(Debug, Clone)]
pub struct ServerWorksAt<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,
    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,
    pub role: String,
}

pub type PersonWorksAtCompany = ServerWorksAt<ServerPerson, ServerCompany>;

/// A remote transport the SDK supports.
#[derive(Clone, Copy)]
enum Transport {
    Http,
    Ws,
}

/// A configuration variable; a missing one fails the test rather than passing it unchecked.
fn config(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} is not set; see the module documentation of tests/server.rs")
    })
}

/// Serializes the tests: they share the server's tables.
static SERVER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A connection over `transport`, signed in and using the configured namespace and database.
async fn connect(transport: Transport) -> Surreal<Any> {
    let (variable, schemes) = match transport {
        Transport::Http => ("UMBRIEL_TEST_HTTP_URL", ["http://", "https://"]),
        Transport::Ws => ("UMBRIEL_TEST_WS_URL", ["ws://", "wss://"]),
    };
    let url = config(variable);
    // `any::connect` picks the transport from the scheme, so the scheme decides what is tested.
    assert!(
        schemes.iter().any(|scheme| url.starts_with(scheme)),
        "{variable} must start with {} or {}, got {url}",
        schemes[0],
        schemes[1]
    );
    let db = any::connect(&url)
        .await
        .unwrap_or_else(|error| panic!("cannot connect to {url}: {error}"));
    db.signin(Root {
        username: config("UMBRIEL_TEST_USER"),
        password: config("UMBRIEL_TEST_PASS"),
    })
    .await
    .expect("signed in as the configured root user");
    db.use_ns(config("UMBRIEL_TEST_NS"))
        .use_db(config("UMBRIEL_TEST_DB"))
        .await
        .expect("namespace and database selected");
    db
}

/// Removes the tables the tests create, and only those.
async fn remove_tables(db: &Surreal<Any>) -> umbriel::Result<IndexedResults> {
    Raw::new(
        "REMOVE TABLE IF EXISTS server_works_at; REMOVE TABLE IF EXISTS server_person; \
         REMOVE TABLE IF EXISTS server_company;",
    )
    .run(db)
    .await
}

async fn define_models(db: &Surreal<Any>) {
    let definitions = [
        ServerPerson::define_table(),
        ServerCompany::define_table(),
        PersonWorksAtCompany::define_table(),
    ]
    .into_iter()
    .chain(ServerPerson::define_fields())
    .chain(ServerCompany::define_fields())
    .chain(PersonWorksAtCompany::define_fields());
    for definition in definitions {
        definition
            .run(db)
            .await
            .unwrap_or_else(|error| panic!("`{definition}` is accepted: {error}"));
    }
}

fn person(key: &str, name: &str, age: u8) -> ServerPerson {
    ServerPerson {
        id: ServerPerson::id(key),
        name: name.to_owned(),
        age,
        ..Default::default()
    }
}

async fn people(db: &Surreal<Any>) -> Vec<String> {
    read(
        db,
        "SELECT VALUE record::id(id) FROM server_person ORDER BY id;",
    )
    .await
}

/// The scenarios, each run over every transport.
mod scenarios {
    use super::*;

    pub async fn connection(db: &Surreal<Any>) {
        let mut results = Raw::new("RETURN 1; RETURN session::ns(); RETURN session::db();")
            .run(db)
            .await
            .expect("ran");
        assert_eq!(results.take::<Option<i64>>(0).expect("int"), Some(1));
        assert_eq!(
            results.take::<Option<String>>(1).expect("string"),
            Some(config("UMBRIEL_TEST_NS"))
        );
        assert_eq!(
            results.take::<Option<String>>(2).expect("string"),
            Some(config("UMBRIEL_TEST_DB"))
        );

        // Bound values travel as query variables.
        let mut results = return_("Alice").run(db).await.expect("ran");
        assert_eq!(
            results
                .take::<Option<String>>(0)
                .expect("string")
                .as_deref(),
            Some("Alice")
        );
    }

    pub async fn definitions(db: &Surreal<Any>) {
        define_models(db).await;

        let info: Option<BTreeMap<String, Value>> = db
            .query("INFO FOR TABLE server_person;")
            .await
            .and_then(IndexedResults::check)
            .expect("info")
            .take(0)
            .expect("an object");
        let fields = info
            .and_then(|mut info| info.remove("fields"))
            .map(|fields| BTreeMap::<String, Value>::from_value(fields).expect("fields"))
            .expect("the table has fields");
        assert_eq!(
            fields.keys().map(String::as_str).collect::<Vec<_>>(),
            ["age", "name", "nickname"]
        );

        let wrong_type = Raw::new("CREATE server_person:typo SET name = 1, age = 30;")
            .run(db)
            .await;
        assert!(wrong_type.is_err(), "a field of the wrong type is rejected");
    }

    pub async fn crud(db: &Surreal<Any>) {
        define_models(db).await;
        let alice = person("alice", "Alice", 30);

        let created = alice.clone().create(db).await.expect("created");
        assert_eq!((created.name.as_str(), created.age), ("Alice", 30));
        assert!(
            alice.clone().create(db).await.is_err(),
            "creating an existing id fails"
        );

        let fetched = ServerPerson::get(db, &alice.id)
            .await
            .expect("read")
            .expect("alice exists");
        assert_eq!((fetched.name.as_str(), fetched.age), ("Alice", 30));

        ServerPerson {
            nickname: Some("Al".to_owned()),
            age: 31,
            ..alice.clone()
        }
        .save(db)
        .await
        .expect("saved");
        let saved = ServerPerson::get(db, &alice.id)
            .await
            .expect("read")
            .expect("alice exists");
        assert_eq!((saved.age, saved.nickname.as_deref()), (31, Some("Al")));

        // Saving replaces the whole record, so an omitted optional field is removed.
        ServerPerson {
            nickname: None,
            ..saved
        }
        .save(db)
        .await
        .expect("saved");
        let removed: bool = read(db, "RETURN type::is_none(server_person:alice.nickname);").await;
        assert!(removed, "the omitted nickname is removed");

        // Saving a record that does not exist inserts it.
        let dave = person("dave", "Dave", 25);
        dave.clone().save(db).await.expect("inserted");
        let inserted = ServerPerson::get(db, &dave.id)
            .await
            .expect("read")
            .expect("dave was inserted");
        assert_eq!((inserted.name.as_str(), inserted.age), ("Dave", 25));

        let patched = ServerPerson::patch(
            db,
            &alice.id,
            ServerPersonPatch {
                name: Patch::Set("Alicia".to_owned()),
                nickname: Patch::Set(Some("Ally".to_owned())),
                ..Default::default()
            },
        )
        .await
        .expect("patched")
        .expect("alice exists");
        assert_eq!(
            (
                patched.name.as_str(),
                patched.age,
                patched.nickname.as_deref()
            ),
            ("Alicia", 31, Some("Ally"))
        );

        person("bob", "Bob", 40).create(db).await.expect("created");
        let ServerPersonSchema { age, .. } = ServerPerson::schema();
        let older = ServerPerson::find_where(age.gt(35))
            .execute(db)
            .await
            .expect("read");
        assert_eq!(
            older.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Bob"]
        );
        assert_eq!(
            ServerPerson::count_all().execute(db).await.expect("count"),
            3
        );

        let deleted = ServerPerson::delete_by_id(db, &ServerPerson::id("bob"))
            .await
            .expect("deleted");
        assert!(deleted.is_some());
        assert_eq!(people(db).await, ["alice", "dave"]);
    }

    pub async fn relations(db: &Surreal<Any>) {
        define_models(db).await;
        let alice = person("alice", "Alice", 30);
        let acme = ServerCompany {
            id: ServerCompany::id("acme"),
            name: "Acme Corp".to_owned(),
        };
        alice.clone().create(db).await.expect("alice");
        acme.clone().create(db).await.expect("acme");

        let path = ServerPerson::with(alice.id.clone())
            .server_works_at__(E)
            .server_company(acme.id.clone());
        let edge = relate::<PersonWorksAtCompany>(path)
            .content(ServerWorksAt {
                id: PersonWorksAtCompany::id("alice_acme"),
                r#in: LinkOne::from(alice.id.clone()),
                out: LinkOne::from(acme.id.clone()),
                role: "Engineer".to_owned(),
            })
            .execute(db)
            .await
            .expect("related")
            .pop()
            .expect("the edge is returned");
        assert_eq!(edge.r#in.key(), Some(&alice.id));
        assert_eq!(edge.out.key(), Some(&acme.id));

        let employers: Vec<String> = read(
            db,
            "SELECT VALUE ->server_works_at->server_company.name FROM ONLY server_person:alice;",
        )
        .await;
        assert_eq!(employers, ["Acme Corp"]);
        let roles: Vec<String> = read(
            db,
            "SELECT VALUE role FROM server_works_at WHERE in = server_person:alice;",
        )
        .await;
        assert_eq!(roles, ["Engineer"]);
    }

    pub async fn statement_errors(db: &Surreal<Any>) {
        let (index, source, _) = statement_error(
            Raw::new("RETURN 1; THROW 'second'; RETURN 3;")
                .run(db)
                .await,
        );
        assert_eq!(index, 1);
        assert!(source.to_string().contains("second"), "{source}");

        let (index, source, related) = statement_error(
            Raw::new("THROW 'first'; RETURN 1; THROW 'third';")
                .run(db)
                .await,
        );
        assert_eq!(index, 0);
        assert!(source.to_string().contains("first"), "{source}");
        assert_eq!(related, 1);
    }

    pub async fn transactions(db: &Surreal<Any>) {
        define_models(db).await;

        let failed = Raw::new(
            "BEGIN TRANSACTION; \
             CREATE server_person:alice SET name = 'Alice', age = 30; \
             THROW 'cause'; \
             COMMIT TRANSACTION;",
        )
        .run(db)
        .await;
        let (_, source, _) = statement_error(failed);
        assert!(
            source.to_string().contains("cause"),
            "the cause, not a rollback row: {source}"
        );
        assert!(
            people(db).await.is_empty(),
            "a failed transaction persists nothing"
        );

        Raw::new(
            "BEGIN TRANSACTION; \
             CREATE server_person:bob SET name = 'Bob', age = 40; \
             CANCEL TRANSACTION;",
        )
        .run(db)
        .await
        .expect("an intentional cancel succeeds");
        assert!(
            people(db).await.is_empty(),
            "a cancelled transaction persists nothing"
        );

        Raw::new(
            "BEGIN TRANSACTION; \
             CREATE server_person:carol SET name = 'Carol', age = 50; \
             COMMIT TRANSACTION;",
        )
        .run(db)
        .await
        .expect("committed");
        assert_eq!(people(db).await, ["carol"]);
    }

    /// `SELECT … VERSION` reads a record as it was at a time, and finds nothing before the record
    /// existed. The memory engine refuses `VERSION`, so this needs a versioned server store.
    pub async fn versioned_reads(db: &Surreal<Any>) {
        define_models(db).await;
        person("alice", "Alice", 30)
            .create(db)
            .await
            .expect("created");
        let before_update: types::Datetime = read(db, "RETURN time::now();").await;
        Raw::new("UPDATE server_person:alice SET age = 31;")
            .run(db)
            .await
            .expect("updated");

        let then = select(All)
            .from_only(ServerPerson::id("alice"))
            .version(before_update)
            .decode::<ServerPerson>()
            .execute(db)
            .await
            .expect("read")
            .expect("alice existed then");
        assert_eq!(then.age, 30);

        let long_ago = chrono::Utc
            .with_ymd_and_hms(2000, 1, 1, 0, 0, 0)
            .single()
            .expect("a time");
        let none = select(All)
            .from(ServerPerson::table())
            .version(long_ago)
            .decode::<ServerPerson>()
            .execute(db)
            .await
            .expect("read");
        assert!(none.is_empty());
    }

    /// `INFO … VERSION` lists a table's definitions as they were at a time: a field defined
    /// later is missing. The memory engine refuses `VERSION`, so this needs a versioned store.
    ///
    /// The added field's name is new on every run: a versioned store keeps the history of a field
    /// that was defined before and then removed with its table, and `INFO … VERSION` lists such a
    /// field again, so a fixed name would be found by later runs and by the other transport.
    pub async fn versioned_info(db: &Surreal<Any>) {
        define_models(db).await;
        let suffix: String = read(
            db,
            "RETURN string::replace(<string> rand::uuid(), '-', '_');",
        )
        .await;
        let added = format!("added_{suffix}");
        let before: types::Datetime = read(db, "RETURN time::now();").await;
        Raw::new(format!(
            "DEFINE FIELD {added} ON server_person TYPE option<string>;"
        ))
        .run(db)
        .await
        .expect("defined");

        let now = info_for()
            .table("server_person")
            .execute(db)
            .await
            .expect("info");
        assert!(now.fields.contains_key(&added));

        let then = info_for()
            .table("server_person")
            .version(before)
            .execute(db)
            .await
            .expect("versioned info");
        assert!(!then.fields.contains_key(&added), "{:?}", then.fields);
        assert!(then.fields.contains_key("name"), "{:?}", then.fields);
    }

    /// `eval::surql` runs a query and returns its result, with bindings. The embedded engine
    /// cannot allow it in 3.3.4, so this needs a server started with `--allow-eval-query`.
    pub async fn eval_surql_runs(db: &Surreal<Any>) {
        let sum: Option<i64> = eval::surql!("RETURN 1 + 2")
            .run(db)
            .await
            .expect("eval::surql runs");
        assert_eq!(sum, Some(3));
        let bound: Option<i64> =
            eval::surql!("RETURN $n * 2", BTreeMap::from([("n".to_owned(), 21)]))
                .run(db)
                .await
                .expect("eval::surql runs with bindings");
        assert_eq!(bound, Some(42));
    }

    /// K17: a clone signed in as another user keeps its own authentication; the original stays
    /// the root user.
    ///
    /// The other user is this run's own: its name is new on every run, and `DEFINE USER` without
    /// `IF NOT EXISTS` or `OVERWRITE` refuses a name that exists, so the test never takes over a
    /// user it did not create. Only a user it created is removed, after the checks, whether they
    /// pass or panic.
    pub async fn cloned_sessions(db: &Surreal<Any>) {
        define_models(db).await;
        let suffix: String = read(
            db,
            "RETURN string::replace(<string> rand::uuid(), '-', '_');",
        )
        .await;
        let username = format!("server_reader_{suffix}");
        Raw::new(format!(
            "DEFINE USER {username} ON DATABASE PASSWORD 'example-password' ROLES VIEWER;"
        ))
        .run(db)
        .await
        .expect("this run's user is created, not found existing");

        let checks = tokio::spawn({
            let db = db.clone();
            let username = username.clone();
            async move { reader_checks(&db, username).await }
        })
        .await;
        Raw::new(format!("REMOVE USER {username} ON DATABASE;"))
            .run(db)
            .await
            .expect("this run's user is removed");
        if let Err(error) = checks {
            match error.try_into_panic() {
                Ok(panic) => std::panic::resume_unwind(panic),
                Err(error) => panic!("the checks did not finish: {error}"),
            }
        }
    }

    async fn reader_checks(db: &Surreal<Any>, username: String) {
        let reader = db.clone();
        reader
            .signin(Database {
                namespace: config("UMBRIEL_TEST_NS"),
                database: config("UMBRIEL_TEST_DB"),
                username,
                password: "example-password".to_owned(),
            })
            .await
            .expect("the clone signs in as the reader");

        // A viewer's write falls back to the table's permissions, and a table defined without
        // `PERMISSIONS` allows no `CREATE`: the server skips the record rather than refusing the
        // statement (`surrealdb-core` `ctx/context.rs:606`, `doc/check.rs:259`), so `create`
        // finds no record. `seen` below shows nothing was written.
        let skipped = person("eve", "Eve", 20)
            .create(&reader)
            .await
            .expect_err("a viewer's write is skipped");
        assert!(
            matches!(&skipped, Error::RecordNotFound(id) if id == "server_person:eve"),
            "{skipped:?}"
        );
        person("alice", "Alice", 30)
            .create(db)
            .await
            .expect("the original is still root");
        let seen: Vec<String> = read(
            &reader,
            "SELECT VALUE record::id(id) FROM server_person ORDER BY id;",
        )
        .await;
        assert_eq!(seen, ["alice"], "the reader can still read");
    }

    /// #17: SDK transaction handles over WebSocket: commit, cancel, drop, and a commit conflict
    /// ordered by the handles' commits, which the RPC layer reports with its structured kind
    /// (`surrealdb-core` `rpc/error.rs`). The drop shows only that nothing is committed: the
    /// server's release of the open transaction is not observable through the SDK.
    pub async fn sdk_transactions(db: &Surreal<Any>) {
        define_models(db).await;

        let tx = db.clone().begin().await.expect("begins");
        person("alice", "Alice", 30)
            .create(&tx)
            .await
            .expect("created");
        ServerPerson::insert_many(&tx, vec![person("bob", "Bob", 40)])
            .await
            .expect("inserted");
        assert!(people(db).await.is_empty(), "nothing visible before commit");
        tx.commit().await.expect("committed");
        assert_eq!(people(db).await, ["alice", "bob"]);

        let tx = db.clone().begin().await.expect("begins");
        person("carol", "Carol", 50)
            .create(&tx)
            .await
            .expect("created");
        tx.cancel().await.expect("cancelled");
        {
            let tx = db.clone().begin().await.expect("begins");
            person("dave", "Dave", 25)
                .create(&tx)
                .await
                .expect("created");
        }
        assert_eq!(
            people(db).await,
            ["alice", "bob"],
            "cancelled and dropped transactions persist nothing"
        );

        let reader = db.clone().begin().await.expect("reader begins");
        select(All)
            .from(ServerPerson::id("alice"))
            .for_update()
            .run(&reader)
            .await
            .expect("selected for update");
        let writer = db.clone().begin().await.expect("writer begins");
        update::<ServerPerson>(ServerPerson::id("alice"))
            .set(ServerPerson::schema().age.equal_to(31_u8))
            .run(&writer)
            .await
            .expect("updated");
        writer.commit().await.expect("the writer commits first");
        let error = Error::from(reader.commit().await.expect_err("the reader conflicts"));
        assert!(matches!(&error, Error::Conflict { .. }), "{error:?}");
        assert_eq!(error.retry(), Retry::Safe);
    }

    /// #17: the server refuses `begin` over HTTP, where requests share no connection. Recorded
    /// behaviour of SurrealDB 3.3.4: the HTTP endpoint reports the method as not found, not as
    /// not allowed, so the error has no structured kind of its own in Umbriel.
    pub async fn begin_is_refused(db: &Surreal<Any>) {
        let error = Error::from(db.clone().begin().await.expect_err("HTTP refuses begin"));
        assert!(
            matches!(
                &error,
                Error::Query(source) if matches!(
                    source.not_found_details(),
                    Some(NotFoundError::Method { name }) if name == "begin"
                )
            ),
            "{error:?}"
        );
        assert_eq!(error.retry(), Retry::Never);
    }
}

/// One ignored test per scenario and transport, holding the server and starting and ending with
/// the test tables removed. `$http` and `$ws` name the modules, and `$reason` says what server
/// the scenarios need.
macro_rules! over_each_transport {
    ($reason:literal, $http:ident, $ws:ident; $($scenario:ident),* $(,)?) => {
        over_each_transport!(@transport $http, Http, $reason; $($scenario),*);
        over_each_transport!(@transport $ws, Ws, $reason; $($scenario),*);
    };
    (@transport $module:ident, $transport:ident, $reason:literal; $($scenario:ident),*) => {
        mod $module {
            use super::*;

            $(
                #[tokio::test]
                #[ignore = $reason]
                async fn $scenario() {
                    let _server = SERVER.lock().await;
                    let db = connect(Transport::$transport).await;
                    remove_tables(&db).await.expect("test tables removed");
                    // A task of its own, so the tables are removed even when the scenario panics.
                    let outcome = tokio::spawn({
                        let db = db.clone();
                        async move { scenarios::$scenario(&db).await }
                    })
                    .await;
                    let cleanup = remove_tables(&db).await;
                    if let Err(error) = outcome {
                        match error.try_into_panic() {
                            Ok(panic) => std::panic::resume_unwind(panic),
                            Err(error) => panic!("the scenario did not finish: {error}"),
                        }
                    }
                    cleanup.expect("test tables removed");
                }
            )*
        }
    };
}

over_each_transport!(
    "needs a running SurrealDB server; see tests/server.rs",
    http,
    ws;
    connection,
    definitions,
    crud,
    relations,
    statement_errors,
    transactions,
    cloned_sessions,
);

over_each_transport!(
    @transport sdk_transactions_ws,
    Ws,
    "needs a running SurrealDB server; see tests/server.rs";
    sdk_transactions
);

over_each_transport!(
    @transport sdk_transactions_http,
    Http,
    "needs a running SurrealDB server; see tests/server.rs";
    begin_is_refused
);

over_each_transport!(
    "needs a SurrealDB server with a versioned store (SurrealKV, `versioned=true`); see tests/server.rs",
    versioned_http,
    versioned_ws;
    versioned_reads,
    versioned_info,
);

over_each_transport!(
    "needs a SurrealDB server started with `--allow-eval-query`; see tests/server.rs",
    eval_http,
    eval_ws;
    eval_surql_runs,
);
