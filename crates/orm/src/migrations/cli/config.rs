use clap::{Args, ValueEnum};
use std::fmt::Display;
use std::str::FromStr;
use surrealdb::opt::{
    Config, WaitFor,
    capabilities::{Capabilities, ExperimentalFeature},
};
use typed_builder::TypedBuilder;

use surrealdb::engine::any::{Any, connect};

use surrealdb::Surreal;
use surrealdb::opt::auth::{Database, Root};

/// Where the database is: the in-memory engine or a URL.
#[derive(Clone, Debug)]
pub enum UrlDb {
    /// The in-memory engine, written `memory`.
    Memory,
    /// Any other endpoint, such as `ws://localhost:8000`.
    Others(String),
}

impl Display for UrlDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UrlDb::Memory => write!(f, "mem://"),
            UrlDb::Others(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for UrlDb {
    type Err = String;

    // Can be one of memory, file:<path>, tikv:<addr>, file://<path>, tikv://<addr>
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        if s == "memory" {
            Ok(UrlDb::Memory)
        } else {
            Ok(UrlDb::Others(s))
        }
    }
}

/// An experimental SurrealDB feature an embedded engine can be started with.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Experimental {
    /// `files`: buckets and file pointers.
    Files,
    /// `surrealism`: modules.
    Surrealism,
}

impl From<Experimental> for ExperimentalFeature {
    fn from(feature: Experimental) -> Self {
        match feature {
            Experimental::Files => ExperimentalFeature::Files,
            Experimental::Surrealism => ExperimentalFeature::Surrealism,
        }
    }
}

/// The database the command line connects to and its credentials.
#[derive(Args, Debug, Clone, TypedBuilder)]
pub struct DatabaseConnection {
    /// URL or path to connect to a database instance. Supports various backends.
    /// Examples:
    /// - Local WebSocket: `ws://localhost:8000`
    /// - Remote WebSocket: `wss://cloud.example.com`
    /// - HTTP: `http://localhost:8000`
    /// - HTTPS: `https://cloud.example.com`
    /// - In-Memory: `mem://`
    /// - File-Backend: `file://temp.db`
    /// - IndxDB-Backend: `indxdb://MyDatabase`
    /// - TiKV-Backend: `tikv://localhost:2379`
    /// - FoundationDB-Backend: `fdb://fdb.cluster`
    #[arg(
            global = true,
            long,
            // value_name = "URL",
            default_value = "ws://localhost:8000",
            // default_value = "memory",
            help = "Example:\n\
                    - ws://localhost:8000\n\
                    - wss://cloud.example.com\n\
                    - http://localhost:8000\n\
                    - https://cloud.example.com\n\
                    - mem://\n\
                    - file://temp.db\n\
                    - indxdb://MyDatabase\n\
                    - tikv://localhost:2379\n\
                    - fdb://fdb.cluster"
        )]
    pub(crate) url: UrlDb,

    #[arg(global = true, long, default_value_t = String::from("test"), help = "Database name")]
    #[builder(default = "test".into())]
    pub(crate) db: String,

    #[arg(global = true, long, default_value_t = String::from("test"), help = "Namespace name")]
    #[builder(default = "test".into())]
    pub(crate) ns: String,

    /// users scope
    #[arg(global = true, long, help = "Scope")]
    #[builder(default, setter(strip_option))]
    pub(crate) sc: Option<String>,

    #[arg(
        global = true,
        short,
        long,
        default_value_t = String::from("root"),
        help = "User name"
    )]
    #[builder(default = "root".into())]
    pub(crate) user: String,

    #[arg(global = true, short, long, default_value_t = String::from("root"), help = "Password")]
    #[builder(default = "root".into())]
    pub(crate) pass: String,

    /// Experimental features an embedded engine allows, such as `files` for buckets and
    /// `surrealism` for modules. None by default. A server keeps its own capabilities, so this
    /// has no effect on a remote URL.
    #[arg(
        global = true,
        long,
        value_enum,
        value_delimiter = ',',
        help = "Experimental features an embedded engine allows (files, surrealism); none by default"
    )]
    #[builder(default)]
    pub(crate) allow_experimental: Vec<Experimental>,

    #[arg(skip)]
    #[builder(default, setter(strip_option))]
    pub(crate) db_connection: Option<Surreal<Any>>,
}

impl DatabaseConnection {
    /// Connects, signs in, and selects the namespace and database. The in-memory engine is
    /// started with the credentials as its root user; a server is signed in to as a database
    /// user.
    ///
    /// # Panics
    ///
    /// If connecting, signing in or selecting the namespace and database fails.
    pub async fn setup(&mut self) -> &mut Self {
        let root = Root {
            username: self.user.clone(),
            password: self.pass.clone(),
        };
        let capabilities = self
            .allow_experimental
            .iter()
            .fold(Capabilities::all(), |capabilities, &feature| {
                capabilities.with_experimental_feature_allowed(feature.into())
            });
        let mut config = Config::new().capabilities(capabilities);
        if let UrlDb::Memory = self.url {
            config = config.user(root.clone());
        }

        let db_instance = connect((self.url.to_string(), config))
            .await
            .expect("Failed to connect");

        match self.url {
            UrlDb::Memory => db_instance.signin(root).await,
            UrlDb::Others(_) => {
                db_instance
                    .signin(Database {
                        namespace: self.ns.clone(),
                        database: self.db.clone(),
                        username: self.user.clone(),
                        password: self.pass.clone(),
                    })
                    .await
            }
        }
        .expect("Failed to signin");
        db_instance
            .use_ns(self.ns.as_str())
            .use_db(self.db.as_str())
            .await
            .expect("Failed to use the namespace and database");

        db_instance.wait_for(WaitFor::Connection).await;
        self.db_connection = Some(db_instance);

        self
    }

    /// The connection, once [`setup`](Self::setup) has run.
    pub fn db(&self) -> Option<&Surreal<Any>> {
        self.db_connection.as_ref()
    }
}

impl Default for DatabaseConnection {
    fn default() -> Self {
        DatabaseConnection::builder()
            .db("test".into())
            .ns("test".into())
            .user("root".into())
            .pass("root".into())
            .url(UrlDb::Memory)
            .build()
    }
}
