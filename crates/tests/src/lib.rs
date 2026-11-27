//! Shared test support: the fixture models, a database of its own per test on a developer-started
//! server (see `crates/tests/README.md`), and SurrealQL assertions.

mod models;

pub mod migrations;
pub mod support;

pub use models::*;

use std::{fmt::Display, time::Duration};

use surrealdb::{
    IndexedResults, Surreal,
    engine::any::{self, Any},
    opt::auth::Root,
    types::{SurrealValue, Value},
};
use tokio::sync::OnceCell;
use umbriel::{Function, Model, Raw, Render, Runnable, kinds::KindOf, migrations::Comparison};

/// Asserts a call's SurrealQL and that SurrealDB's parser accepts it.
#[track_caller]
pub fn call<K: KindOf>(function: &Function<K>, expected: &str) {
    pretty_assertions::assert_eq!(function.to_debug_sql(), expected);
    assert_parses(&format!("RETURN {expected};"));
}

/// Asserts that SurrealDB evaluates the call to a value equal to the SurrealQL `expected`.
pub async fn holds<K: KindOf>(db: &Surreal<Any>, function: &Function<K>, expected: &str) {
    let call = function.to_debug_sql();
    let equal: bool = read(db, &format!("RETURN {call} = {expected};")).await;
    if !equal {
        let actual: Value = read(db, &format!("RETURN {call};")).await;
        panic!("`{call}` returned {actual:?}, expected {expected}");
    }
}

/// Asserts that SurrealDB evaluates the call to a value of the given type, e.g. `float`.
pub async fn is_a<K: KindOf>(db: &Surreal<Any>, function: &Function<K>, kind: &str) {
    let call = function.to_debug_sql();
    let matches: bool = read(db, &format!("RETURN type::is_{kind}({call});")).await;
    assert!(matches, "`{call}` is not a {kind}");
}

/// Asserts that SurrealDB runs the method form and that it evaluates like the function form,
/// so the method is dispatched on that receiver to that function.
pub async fn same<K: KindOf, L: KindOf>(
    db: &Surreal<Any>,
    method: &Function<K>,
    function: &Function<L>,
) {
    let (method, function) = (method.to_debug_sql(), function.to_debug_sql());
    let equal: bool = read(db, &format!("RETURN {method} = {function};")).await;
    if !equal {
        let actual: Value = read(db, &format!("RETURN {method};")).await;
        let expected: Value = read(db, &format!("RETURN {function};")).await;
        panic!("`{method}` returned {actual:?}, `{function}` {expected:?}");
    }
}

/// `same` for each `(method, function)` pair, which may be calls of different kinds.
#[macro_export]
macro_rules! same_all {
    ($db:expr, [$(($method:expr, $function:expr $(,)?)),* $(,)?]) => {{
        $($crate::same($db, &$method, &$function).await;)*
    }};
}

/// The error SurrealDB gives for the call, as text. Panics when it succeeds.
pub async fn call_failure<K: KindOf>(db: &Surreal<Any>, function: &Function<K>) -> String {
    failure(db, &format!("RETURN {};", function.to_debug_sql())).await
}

/// Runs the call with its values bound, through [`Function::run`], and decodes the result.
pub async fn bound<T: SurrealValue, K: KindOf>(
    db: &Surreal<Any>,
    function: &Function<K>,
) -> Option<T> {
    function.run(db).await.expect("ran")
}

/// 2021-11-01T08:30:17Z, a Monday.
pub fn datetime() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_635_755_417, 0).expect("valid timestamp")
}

/// `holds` for each `(call, expected)` pair, which may be calls of different kinds.
#[macro_export]
macro_rules! holds_all {
    ($db:expr, [$(($function:expr, $expected:expr $(,)?)),* $(,)?]) => {{
        $($crate::holds($db, &$function, $expected).await;)*
    }};
}

/// `is_a` for each `(call, kind)` pair, which may be calls of different kinds.
#[macro_export]
macro_rules! is_each {
    ($db:expr, [$(($function:expr, $kind:expr $(,)?)),* $(,)?]) => {{
        $($crate::is_a($db, &$function, $kind).await;)*
    }};
}

/// `call` for each `(call, expected)` pair, which may be calls of different kinds.
#[macro_export]
macro_rules! calls {
    ($(($function:expr, $expected:expr $(,)?)),* $(,)?) => {{
        $($crate::call(&$function, $expected);)*
    }};
}

/// Definitions joined one per line.
pub fn joined(definitions: &[Raw]) -> String {
    definitions
        .iter()
        .map(Raw::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Defines `M`'s table and fields on `db`. Panics when SurrealDB refuses a definition.
pub async fn apply_definitions<M: Model>(db: &Surreal<Any>) {
    for definition in std::iter::once(M::define_table()).chain(M::define_fields()) {
        definition
            .run(db)
            .await
            .unwrap_or_else(|error| panic!("`{definition}` is accepted: {error}"));
    }
}

/// Asserts a statement's debug SQL, and that both it and its bound form parse as SurrealQL with
/// SurrealDB's own parser, so syntax SurrealDB no longer accepts fails even without a database.
#[track_caller]
pub fn assert_sql(statement: &impl Render, expected: &str) {
    let debug = statement.to_debug_sql();
    pretty_assertions::assert_eq!(debug, expected);
    assert_parses(&debug);
    let query = statement
        .to_query()
        .unwrap_or_else(|error| panic!("`{debug}` renders: {error}"));
    assert_parses(query.sql());
}

/// Asserts that `sql` parses as SurrealQL.
#[track_caller]
pub fn assert_parses(sql: &str) {
    if let Err(error) = surrealdb_syn::parse(sql) {
        panic!("`{sql}` is not valid SurrealQL: {error}");
    }
}

/// Asserts that rendering `statement` fails with an error containing `message`.
#[track_caller]
pub fn assert_render_error(statement: &impl Render, message: &str) {
    match statement.to_query() {
        Ok(query) => panic!("`{}` rendered, expected an error", query.sql()),
        Err(error) => assert!(
            error.to_string().contains(message),
            "`{error}` does not mention `{message}`"
        ),
    }
}

/// The failed statement's index, its error and the number of other failed statements of a run
/// expected to fail.
#[track_caller]
pub fn statement_error(
    result: umbriel::Result<surrealdb::IndexedResults>,
) -> (usize, surrealdb::Error, usize) {
    match result {
        Err(umbriel::Error::Statement {
            index,
            source,
            related,
        }) => (index, *source, related.len()),
        Err(other) => panic!("expected a statement error, got {other}"),
        Ok(_) => panic!("expected a statement error, got success"),
    }
}

/// The namespace every test database is created in.
pub const NAMESPACE: &str = "umbriel_test";

/// The prefix of every test database's name, so leftovers of an interrupted run can be found.
pub const DATABASE_PREFIX: &str = "umbriel_test_";

/// How long any one network wait may take, in setup, a test helper or cleanup.
pub const WAIT: Duration = Duration::from_secs(10);

/// A developer-started server the tests run against, each named by an environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    /// `UMBRIEL_TEST_URL`: WebSocket, with every capability the suite needs allowed.
    Main,
    /// `UMBRIEL_TEST_RESTRICTED_URL`: WebSocket, with the server's default capabilities, for the
    /// tests that check a capability is refused when it is off.
    Restricted,
    /// `UMBRIEL_TEST_HTTP_URL`: HTTP, for the transport scenarios in `server.rs`.
    Http,
}

impl Endpoint {
    /// The URL configured for this endpoint. Panics, naming the variable, when it is not set.
    pub fn url(self) -> String {
        config(match self {
            Endpoint::Main => "UMBRIEL_TEST_URL",
            Endpoint::Restricted => "UMBRIEL_TEST_RESTRICTED_URL",
            Endpoint::Http => "UMBRIEL_TEST_HTTP_URL",
        })
    }
}

/// A configuration variable. A missing one fails the test, so a misconfigured run can't pass.
fn config(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set; see crates/tests/README.md"))
}

/// The configured root user.
pub fn root() -> Root {
    Root {
        username: config("UMBRIEL_TEST_USER"),
        password: config("UMBRIEL_TEST_PASS"),
    }
}

/// Awaits `future` for at most [`WAIT`], panicking with `what` when it fails or times out.
pub async fn bounded<T, E: Display>(
    what: &str,
    future: impl IntoFuture<Output = Result<T, E>>,
) -> T {
    match tokio::time::timeout(WAIT, future).await {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => panic!("{what}: {error}"),
        Err(_) => panic!("{what}: no answer within {WAIT:?}"),
    }
}

/// A connection to `endpoint`, signed in as root, with nothing selected.
pub async fn connect(endpoint: Endpoint) -> Surreal<Any> {
    let url = endpoint.url();
    let db = bounded(&format!("connecting to {url}"), any::connect(url.as_str())).await;
    bounded("signing in as the configured root user", db.signin(root())).await;
    db
}

/// A connection to `endpoint` with no one signed in and nothing selected, for the sign-ins of
/// record users, bearer keys and system users other than root.
pub async fn guest(endpoint: Endpoint) -> Surreal<Any> {
    let url = endpoint.url();
    bounded(&format!("connecting to {url}"), any::connect(url.as_str())).await
}

/// The test namespace is defined once per process: concurrent definitions of one name can
/// conflict.
static NAMESPACE_DEFINED: OnceCell<()> = OnceCell::const_new();

/// Something a test created on a server, removed by one statement when this guard drops, whether
/// the test passed or panicked.
///
/// The removal runs on a thread of its own, with its own runtime and connection: `Drop` cannot
/// await, and the test's runtime cannot be blocked from inside. When the test is already
/// panicking, a failed removal is printed rather than raised, so the original failure is the one
/// reported (a second panic would abort the process). Otherwise a failed removal fails the test.
/// An interrupted run leaves what it created behind; see `crates/tests/README.md`.
#[derive(Debug)]
struct Removal {
    endpoint: Endpoint,
    /// The namespace the statement runs in, if any.
    namespace: Option<String>,
    /// The statement, removing one thing by name.
    statement: String,
}

impl Drop for Removal {
    fn drop(&mut self) {
        let endpoint = self.endpoint;
        let namespace = self.namespace.take();
        let statement = std::mem::take(&mut self.statement);
        let removal = std::thread::spawn({
            let statement = statement.clone();
            move || remove(endpoint, namespace.as_deref(), &statement)
        })
        .join()
        .unwrap_or_else(|_| Err("the cleanup thread panicked".to_owned()));
        let Err(error) = removal else {
            return;
        };
        let message = format!("`{statement}` failed: {error}");
        if std::thread::panicking() {
            eprintln!("{message}");
        } else {
            panic!("{message}");
        }
    }
}

/// How many times a removal runs before its failure is reported. Work a test leaves running on
/// the server, such as a concurrent index build, can make the removal's transaction conflict.
const REMOVAL_ATTEMPTS: u32 = 5;

/// The pause before a conflicting removal runs again.
const REMOVAL_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Runs the removal `statement` on `endpoint`, in `namespace` when given. A transaction conflict
/// ([`Retry::Safe`](umbriel::Retry::Safe)) is run again, up to [`REMOVAL_ATTEMPTS`] times, all
/// within [`WAIT`].
fn remove(endpoint: Endpoint, namespace: Option<&str>, statement: &str) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("no runtime: {error}"))?;
    runtime.block_on(async {
        let removed = tokio::time::timeout(WAIT, async {
            let db = any::connect(endpoint.url().as_str()).await?;
            db.signin(root()).await?;
            if let Some(namespace) = namespace {
                db.use_ns(namespace).await?;
            }
            let mut attempt = 1;
            loop {
                let error = match db
                    .query(statement.to_owned())
                    .await
                    .and_then(IndexedResults::check)
                {
                    Ok(_) => return Ok(()),
                    Err(error) => umbriel::Error::from(error),
                };
                if attempt == REMOVAL_ATTEMPTS || error.retry() != umbriel::Retry::Safe {
                    return Err(error);
                }
                attempt += 1;
                tokio::time::sleep(REMOVAL_RETRY_DELAY).await;
            }
        })
        .await;
        match removed {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err(format!("no answer within {WAIT:?}")),
        }
    })
}

/// A database a test created in [`NAMESPACE`], removed by name when this guard drops (see
/// [`Removal`]).
#[derive(Debug)]
pub struct TestDb {
    database: String,
    _removal: Removal,
}

impl TestDb {
    /// The namespace the database is in.
    pub fn namespace(&self) -> &str {
        NAMESPACE
    }

    /// The database's name.
    pub fn database(&self) -> &str {
        &self.database
    }
}

/// A namespace a test created, removed by name, with everything in it, when this guard drops
/// (see [`Removal`]).
#[derive(Debug)]
pub struct TestNamespace {
    namespace: String,
    _removal: Removal,
}

impl TestNamespace {
    /// The namespace's name.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
}

/// A new namespace of its own on the main server, `umbriel_test_ns_<uuid>`, and a connection
/// signed in as root with nothing selected.
pub async fn namespace() -> (Surreal<Any>, TestNamespace) {
    let db = connect(Endpoint::Main).await;
    let namespace = format!("{DATABASE_PREFIX}ns_{}", uuid::Uuid::new_v4().simple());
    // Without `IF NOT EXISTS` or `OVERWRITE`, `DEFINE NAMESPACE` refuses a name that exists.
    bounded("defining the test namespace", async {
        db.query(format!("DEFINE NAMESPACE {namespace};"))
            .await
            .and_then(IndexedResults::check)
    })
    .await;
    let guard = TestNamespace {
        _removal: Removal {
            endpoint: Endpoint::Main,
            namespace: None,
            statement: format!("REMOVE NAMESPACE {namespace};"),
        },
        namespace,
    };
    (db, guard)
}

/// A migration [`Comparison`] in a namespace of its own ([`namespace`]), on a root connection.
pub async fn comparison() -> (Comparison<Any>, TestNamespace) {
    let (db, guard) = namespace().await;
    (Comparison::new(db, guard.namespace()), guard)
}

/// The names of the databases in `namespace`, read as root on `db`.
pub async fn databases_in(db: &Surreal<Any>, namespace: &str) -> Vec<String> {
    let reader = db.clone();
    bounded("selecting the namespace", reader.use_ns(namespace)).await;
    let info: Value = read(&reader, "INFO FOR NS STRUCTURE;").await;
    let Value::Object(info) = info else {
        panic!("`INFO FOR NS` returns an object: {info:?}");
    };
    let Some(Value::Array(databases)) = info.get("databases") else {
        panic!("`INFO FOR NS STRUCTURE` lists databases: {info:?}");
    };
    databases
        .iter()
        .map(|database| match database {
            Value::Object(database) => match database.get("name") {
                Some(Value::String(name)) => name.clone(),
                other => panic!("a database has a name: {other:?}"),
            },
            other => panic!("a database is an object: {other:?}"),
        })
        .collect()
}

/// A new database on `endpoint`, defined with `options` (e.g. ` STRICT`), selected on a
/// connection signed in as root.
async fn open(endpoint: Endpoint, options: &str) -> (Surreal<Any>, TestDb) {
    let db = connect(endpoint).await;
    NAMESPACE_DEFINED
        .get_or_init(|| async {
            bounded("defining the test namespace", async {
                db.query(format!("DEFINE NAMESPACE IF NOT EXISTS {NAMESPACE};"))
                    .await
                    .and_then(IndexedResults::check)
            })
            .await;
        })
        .await;
    bounded("selecting the test namespace", db.use_ns(NAMESPACE)).await;
    let database = format!("{DATABASE_PREFIX}{}", uuid::Uuid::new_v4().simple());
    // Without `IF NOT EXISTS` or `OVERWRITE`, `DEFINE DATABASE` refuses a name that exists, so a
    // test never takes over a database it did not create.
    bounded("defining the test database", async {
        db.query(format!("DEFINE DATABASE {database}{options};"))
            .await
            .and_then(IndexedResults::check)
    })
    .await;
    let guard = TestDb {
        _removal: Removal {
            endpoint,
            namespace: Some(NAMESPACE.to_owned()),
            statement: format!("REMOVE DATABASE {database};"),
        },
        database,
    };
    bounded("selecting the test database", db.use_db(guard.database())).await;
    (db, guard)
}

/// A new database of its own on the main server ([`Endpoint::Main`]), selected. Keep the guard
/// alive for the whole test: `let (db, _guard) = db().await;`.
pub async fn db() -> (Surreal<Any>, TestDb) {
    open(Endpoint::Main, "").await
}

/// [`db`] on the server with default capabilities ([`Endpoint::Restricted`]).
pub async fn restricted_db() -> (Surreal<Any>, TestDb) {
    open(Endpoint::Restricted, "").await
}

/// [`db`] over HTTP ([`Endpoint::Http`]).
pub async fn http_db() -> (Surreal<Any>, TestDb) {
    open(Endpoint::Http, "").await
}

/// [`db`], defined `STRICT`, so tables must be defined before use.
pub async fn strict_db() -> (Surreal<Any>, TestDb) {
    open(Endpoint::Main, " STRICT").await
}

/// Runs `sql` and returns SurrealDB's answer, every statement checked. Panics, naming `sql`, when
/// there is no answer within [`WAIT`].
async fn answer(
    db: &Surreal<impl surrealdb::Connection>,
    sql: &str,
) -> surrealdb::Result<IndexedResults> {
    tokio::time::timeout(WAIT, db.query(sql.to_owned()))
        .await
        .unwrap_or_else(|_| panic!("`{sql}`: no answer within {WAIT:?}"))
        .and_then(IndexedResults::check)
}

/// Runs `sql` and decodes the result of its first statement. Used to check what a test stored
/// independently of the API under test.
pub async fn read<T: SurrealValue>(db: &Surreal<impl surrealdb::Connection>, sql: &str) -> T {
    let value: Value = answer(db, sql)
        .await
        .unwrap_or_else(|error| panic!("`{sql}` fails: {error}"))
        .take(0)
        .unwrap_or_else(|error| panic!("`{sql}` has no result: {error}"));
    T::from_value(value).unwrap_or_else(|error| panic!("`{sql}` doesn't decode: {error}"))
}

/// The `DEFINE` statement that `info` (e.g. `INFO FOR DB;`) lists for `name` under `kind`
/// (e.g. `tables`). Panics when the shape differs or the definition is missing.
pub async fn stored_definition(
    db: &Surreal<impl surrealdb::Connection>,
    info: &str,
    kind: &str,
    name: &str,
) -> String {
    let Value::Object(info) = read::<Value>(db, info).await else {
        panic!("`{info}` returns an object");
    };
    let Some(Value::Object(definitions)) = info.get(kind) else {
        panic!("`{kind}` lists definitions");
    };
    match definitions.get(name) {
        Some(Value::String(sql)) => sql.clone(),
        other => panic!("`{name}` is not defined: {other:?}"),
    }
}

/// Runs `sql` and reports whether every statement in it succeeded. Used where a test checks only
/// that SurrealDB accepts or refuses something.
pub async fn succeeds(db: &Surreal<impl surrealdb::Connection>, sql: &str) -> bool {
    answer(db, sql).await.is_ok()
}

/// The error SurrealDB gives for `sql`, as text. Panics when every statement succeeds, so a test
/// can assert why a statement is refused rather than only that it is.
pub async fn failure(db: &Surreal<impl surrealdb::Connection>, sql: &str) -> String {
    match answer(db, sql).await {
        Ok(_) => panic!("`{sql}` succeeded, expected an error"),
        Err(error) => error.to_string(),
    }
}
