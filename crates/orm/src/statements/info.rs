use std::marker::PhantomData;

use surrealdb::{
    Connection, Surreal,
    types::{Object, Table, Value},
};

use crate::{
    DatetimeLike, Error, Name, Render, Renderer, Result, TableIndex, User, execution::execute,
};

use super::{
    Statement, impl_statement,
    info_result::{
        DatabaseInfo, IndexStatus, InfoOutput, NamespaceInfo, RootInfo, Statements, Structures,
        TableInfo,
    },
    remove::UserPermissionScope,
};

/// Creates statement for INFO for ROOT, NAMESPACE, DATABASE, TABLE, USER or INDEX.
///
/// Each statement executes to a typed result (see [`RootInfo`], [`DatabaseInfo`], …): the plain
/// form lists each definition's `DEFINE` statement, and [`structure`](InfoStatement::structure)
/// lists their properties instead.
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::info_for};
/// assert_eq!(info_for().root().to_debug_sql(), "INFO FOR ROOT;");
/// assert_eq!(
///     info_for().user("alice").on_database().to_debug_sql(),
///     "INFO FOR USER alice ON DATABASE;"
/// );
/// assert_eq!(
///     info_for().index("user_email", "user").to_debug_sql(),
///     "INFO FOR INDEX user_email ON TABLE user;"
/// );
/// ```
pub fn info_for() -> InfoStatementInit {
    InfoStatementInit
}

/// What an `INFO` statement reports on.
enum InfoTarget {
    Root,
    Namespace,
    Database,
    Table(Name<Table>),
    User {
        name: Name<User>,
        level: Option<UserPermissionScope>,
    },
    Index {
        index: Name<TableIndex>,
        table: Name<Table>,
    },
}

/// Information statement initialization builder
pub struct InfoStatementInit;

impl InfoStatementInit {
    /// Creates statement for INFO for ROOT.
    /// The top-level ROOT command returns information regarding the namespaces, root users and
    /// root access methods of the SurrealDB system.
    pub fn root(self) -> InfoStatement<RootInfo> {
        InfoStatement::new(InfoTarget::Root)
    }

    /// Creates statement for INFO for NAMESPACE.
    /// The NS or NAMESPACE command returns information regarding the users, access methods and databases under a specific Namespace.
    pub fn namespace(self) -> InfoStatement<NamespaceInfo> {
        InfoStatement::new(InfoTarget::Namespace)
    }

    /// Creates statement for INFO for DATABASE.
    /// The DB or DATABASE command returns information regarding the users, access methods and tables under a specific Database.
    pub fn database(self) -> InfoStatement<DatabaseInfo> {
        InfoStatement::new(InfoTarget::Database)
    }

    /// Creates statement for INFO for TABLE.
    /// The TABLE command returns information regarding the events, fields, indexes, and foreign table configurations on a specific Table.
    ///
    /// ```rust
    /// use umbriel::{*, statements::info_for};
    /// assert_eq!(info_for().table("test_table").to_debug_sql(), "INFO FOR TABLE test_table;");
    /// ```
    pub fn table(self, table: impl Into<Name<Table>>) -> InfoStatement<TableInfo> {
        InfoStatement::new(InfoTarget::Table(table.into()))
    }

    /// Creates statement for INFO for USER: the user's `DEFINE USER` statement, with its secrets
    /// redacted. Without a level, SurrealDB looks the user up on the current database.
    pub fn user(self, name: impl Into<Name<User>>) -> InfoStatement<String> {
        InfoStatement::new(InfoTarget::User {
            name: name.into(),
            level: None,
        })
    }

    /// Creates statement for INFO for INDEX: the build status of the index `index` on `table`.
    pub fn index(
        self,
        index: impl Into<Name<TableIndex>>,
        table: impl Into<Name<Table>>,
    ) -> InfoStatement<IndexStatus> {
        InfoStatement::new(InfoTarget::Index {
            index: index.into(),
            table: table.into(),
        })
    }
}

/// An `INFO` statement, which executes to `O`.
pub struct InfoStatement<O> {
    target: InfoTarget,
    structure: bool,
    version: Option<DatetimeLike>,
    output: PhantomData<fn() -> O>,
}

impl<O> InfoStatement<O> {
    fn new(target: InfoTarget) -> Self {
        Self {
            target,
            structure: false,
            version: None,
            output: PhantomData,
        }
    }

    /// The same statement executing to another form.
    fn into_form<P>(self, structure: bool) -> InfoStatement<P> {
        InfoStatement {
            target: self.target,
            structure,
            version: self.version,
            output: PhantomData,
        }
    }
}

impl<O: InfoOutput + 'static> InfoStatement<O> {
    /// Runs the statement and decodes its result.
    ///
    /// # Errors
    ///
    /// The database error, or [`Error::Conversion`] when the result does not have the shape
    /// SurrealDB 3.3.4 gives it.
    pub async fn execute(&self, db: &Surreal<impl Connection>) -> Result<O> {
        let mut results = execute(db, self.to_query()?).await?;
        O::from_info(results.take::<Value>(0).map_err(Error::Conversion)?)
    }
}

/// `structure` and `version` for the levels that list definitions.
macro_rules! listing_levels {
    ($($info:ident),*) => {
        $(
            impl InfoStatement<$info<Statements>> {
                /// Returns each definition as an object of its properties (`STRUCTURE`) instead
                /// of its `DEFINE` statement.
                ///
                /// ```rust
                /// use umbriel::{*, statements::info_for};
                /// assert_eq!(info_for().database().structure().to_debug_sql(), "INFO FOR DB STRUCTURE;");
                /// assert_eq!(
                ///     info_for().table("person").structure().to_debug_sql(),
                ///     "INFO FOR TABLE person STRUCTURE;"
                /// );
                /// ```
                pub fn structure(self) -> InfoStatement<$info<Structures>> {
                    self.into_form(true)
                }
            }

            impl<F: super::info_result::InfoForm> InfoStatement<$info<F>> {
                /// `VERSION datetime`: the definitions as they were at that time. It needs a
                /// versioned store, such as SurrealKV opened with `versioned`.
                pub fn version(mut self, datetime: impl Into<DatetimeLike>) -> Self {
                    self.version = Some(datetime.into());
                    self
                }
            }
        )*
    };
}

listing_levels!(RootInfo, NamespaceInfo, DatabaseInfo, TableInfo);

impl InfoStatement<String> {
    /// `ON ROOT`: the root user with this name.
    pub fn on_root(self) -> Self {
        self.on(UserPermissionScope::Root)
    }

    /// `ON NAMESPACE`: the user with this name on the current namespace.
    pub fn on_namespace(self) -> Self {
        self.on(UserPermissionScope::Namespace)
    }

    /// `ON DATABASE`: the user with this name on the current database.
    pub fn on_database(self) -> Self {
        self.on(UserPermissionScope::Database)
    }

    fn on(mut self, scope: UserPermissionScope) -> Self {
        if let InfoTarget::User { level, .. } = &mut self.target {
            *level = Some(scope);
        }
        self
    }

    /// Returns the user's properties (`STRUCTURE`) instead of its `DEFINE USER` statement.
    pub fn structure(self) -> InfoStatement<Object> {
        self.into_form(true)
    }
}

impl<O> Statement for InfoStatement<O> {
    fn render_body(&self, out: &mut Renderer) {
        match &self.target {
            InfoTarget::Root => {
                out.push_str("INFO FOR ROOT");
            }
            InfoTarget::Namespace => {
                out.push_str("INFO FOR NS");
            }
            InfoTarget::Database => {
                out.push_str("INFO FOR DB");
            }
            InfoTarget::Table(table) => {
                out.push_str("INFO FOR TABLE ").push(table);
            }
            InfoTarget::User { name, level } => {
                out.push_str("INFO FOR USER ").push(name);
                if let Some(level) = level {
                    out.push_str(" ON ").push_display(level);
                }
            }
            InfoTarget::Index { index, table } => {
                out.push_str("INFO FOR INDEX ")
                    .push(index)
                    .push_str(" ON TABLE ")
                    .push(table);
            }
        }
        if let Some(version) = &self.version {
            out.push_str(" VERSION ").push(version);
        }
        if self.structure {
            out.push_str(" STRUCTURE");
        }
    }
}

impl_statement!([O: 'static] InfoStatement<O>);
