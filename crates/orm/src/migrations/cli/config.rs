use clap::{Args, ValueEnum};
use std::fmt::Display;
use std::str::FromStr;
use surrealdb::opt::{
    Config, WaitFor,
    capabilities::{Capabilities, ExperimentalFeature},
};

use surrealdb::engine::any::{Any, connect};

use surrealdb::Surreal;
use surrealdb::opt::auth::{Database, Namespace, Root};

use crate::migrations::{Comparison, MigrationError, MigrationResult};

/// The environment variable holding the comparison user's password, so it stays out of the
/// process list and shell history.
pub const COMPARISON_PASSWORD_VARIABLE: &str = "UMBRIEL_COMPARISON_PASS";

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
#[derive(Args, Debug, Clone)]
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
    #[arg(global = true, long, default_value = "ws://localhost:8000")]
    pub(crate) url: UrlDb,

    /// Database name
    #[arg(global = true, long, default_value_t = String::from("test"))]
    pub(crate) db: String,

    /// Namespace name
    #[arg(global = true, long, default_value_t = String::from("test"))]
    pub(crate) ns: String,

    /// User name
    #[arg(global = true, short, long, default_value_t = String::from("root"))]
    pub(crate) user: String,

    /// Password
    #[arg(global = true, short, long, default_value_t = String::from("root"))]
    pub(crate) pass: String,

    /// Experimental features an embedded engine allows, such as `files` for buckets and
    /// `surrealism` for modules. None by default. A server keeps its own capabilities, so this
    /// has no effect on a remote URL.
    #[arg(global = true, long, value_enum, value_delimiter = ',')]
    pub(crate) allow_experimental: Vec<Experimental>,

    #[arg(skip)]
    pub(crate) db_connection: Option<Surreal<Any>>,

    /// The namespace `init`, `generate` and `reset` create their comparison databases in (see
    /// [`Comparison`]). Required by those commands; no default.
    #[arg(global = true, long)]
    pub(crate) comparison_ns: Option<String>,

    /// The namespace-level user the generator signs in as, on the same URL: an `OWNER`, since the
    /// generator defines a user of its own in each comparison database (see [`Comparison`]). Its
    /// password is read from `UMBRIEL_COMPARISON_PASS`.
    #[arg(global = true, long)]
    pub(crate) comparison_user: Option<String>,

    /// A comparison already connected, used instead of connecting with the arguments above.
    #[arg(skip)]
    pub(crate) comparison: Option<Comparison<Any>>,
}

impl DatabaseConnection {
    /// A connection made in code: `db`, already signed in with its namespace and database
    /// selected, and `comparison` for the commands that compare schemas. `url` is the server's
    /// address, as the command line would name it.
    pub fn connected(url: UrlDb, db: Surreal<Any>, comparison: Comparison<Any>) -> Self {
        Self {
            url,
            db: "test".to_owned(),
            ns: "test".to_owned(),
            user: "root".to_owned(),
            pass: "root".to_owned(),
            allow_experimental: Vec::new(),
            db_connection: Some(db),
            comparison_ns: None,
            comparison_user: None,
            comparison: Some(comparison),
        }
    }

    /// Connects, signs in, and selects the namespace and database. The in-memory engine is
    /// started with the credentials as its root user; a server is signed in to as a database
    /// user.
    ///
    /// # Errors
    ///
    /// [`MigrationError::DbError`] when connecting, signing in or selecting the namespace and
    /// database fails.
    pub async fn setup(&mut self) -> MigrationResult<()> {
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

        let db_instance = connect((self.url.to_string(), config)).await?;

        // The session keeps the sign-in; the token itself isn't needed.
        let _token = match self.url {
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
        }?;
        db_instance
            .use_ns(self.ns.as_str())
            .use_db(self.db.as_str())
            .await?;

        db_instance.wait_for(WaitFor::Connection).await;
        self.db_connection = Some(db_instance);
        Ok(())
    }

    /// The connection, once [`setup`](Self::setup) has run.
    ///
    /// # Errors
    ///
    /// [`MigrationError::NotConnected`] before it has.
    pub fn db(&self) -> MigrationResult<&Surreal<Any>> {
        self.db_connection
            .as_ref()
            .ok_or(MigrationError::NotConnected)
    }

    /// The comparison the generator uses: the one given to [`connected`](Self::connected), or a connection to the
    /// URL signed in as `--comparison-user` (password from `UMBRIEL_COMPARISON_PASS`) in
    /// `--comparison-ns`. The connection is made once and reused.
    ///
    /// # Errors
    ///
    /// [`MigrationError::MissingArgument`] when `--comparison-ns`, `--comparison-user` or
    /// `UMBRIEL_COMPARISON_PASS` is missing; [`MigrationError::DbError`] when connecting or
    /// signing in fails.
    pub async fn comparison(&mut self) -> MigrationResult<Comparison<Any>> {
        if let Some(comparison) = &self.comparison {
            return Ok(comparison.clone());
        }
        let namespace = self
            .comparison_ns
            .clone()
            .ok_or(MigrationError::MissingArgument {
                argument: "--comparison-ns",
            })?;
        let username = self
            .comparison_user
            .clone()
            .ok_or(MigrationError::MissingArgument {
                argument: "--comparison-user",
            })?;
        let password = std::env::var(COMPARISON_PASSWORD_VARIABLE).map_err(|_| {
            MigrationError::MissingArgument {
                argument: COMPARISON_PASSWORD_VARIABLE,
            }
        })?;
        let db = connect(self.url.to_string()).await?;
        db.signin(Namespace {
            namespace: namespace.clone(),
            username,
            password,
        })
        .await?;
        let comparison = Comparison::new(db, namespace);
        self.comparison = Some(comparison.clone());
        Ok(comparison)
    }
}
