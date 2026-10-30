//! `REMOVE` statements. Each takes `IF EXISTS`, which makes removing a missing resource a no-op
//! instead of an error.

use std::{
    borrow::Cow,
    fmt::{self, Display},
};

use surrealdb::types::Table;

use crate::{
    Access, AccessLevel, Database, Event, Field, Ident, Name, Namespace, Param, Renderer,
    TableIndex, User,
};

use super::{Statement, impl_definition};

/// Adds `if_exists` to a `REMOVE` statement holding an `if_exists` flag.
macro_rules! impl_if_exists {
    ($statement:ty) => {
        impl $statement {
            /// `IF EXISTS`: removing a resource that does not exist succeeds instead of failing.
            pub fn if_exists(mut self) -> Self {
                self.if_exists = true;
                self
            }
        }
    };
}

/// Writes `REMOVE <KIND> [IF EXISTS] `, ready for the name.
fn push_head(out: &mut Renderer, kind: &str, if_exists: bool) {
    out.push_str("REMOVE ").push_str(kind);
    if if_exists {
        out.push_str(" IF EXISTS");
    }
    out.push_str(" ");
}

/// Writes ` ON TABLE @table` when a table is set.
fn push_table(out: &mut Renderer, table: Option<&Name<Table>>) {
    if let Some(table) = table {
        out.push_str(" ON TABLE ").push(table);
    }
}

/// Removes an access method: `REMOVE ACCESS [IF EXISTS] @name ON [ ROOT | NAMESPACE | DATABASE ]`.
///
/// # Example
/// ```rust
/// use umbriel::{*, statements::remove_access};
///
/// let statement = remove_access("account").on_database();
/// assert_eq!(statement.to_debug_sql(), "REMOVE ACCESS account ON DATABASE;");
///
/// let statement = remove_access("account").on_database().if_exists();
/// assert_eq!(statement.to_debug_sql(), "REMOVE ACCESS IF EXISTS account ON DATABASE;");
///
/// // The level is required.
/// assert!(remove_access("account").to_query().is_err());
/// ```
pub fn remove_access(name: impl Into<Name<Access>>) -> RemoveAccessStatement {
    RemoveAccessStatement {
        name: name.into(),
        level: None,
        if_exists: false,
    }
}

/// A `REMOVE ACCESS` statement.
pub struct RemoveAccessStatement {
    name: Name<Access>,
    level: Option<AccessLevel>,
    if_exists: bool,
}

impl RemoveAccessStatement {
    /// Removes the access method defined on the root.
    pub fn on_root(mut self) -> Self {
        self.level = Some(AccessLevel::Root);
        self
    }

    /// Removes the access method defined on the current namespace.
    pub fn on_namespace(mut self) -> Self {
        self.level = Some(AccessLevel::Namespace);
        self
    }

    /// Removes the access method defined on the current database.
    pub fn on_database(mut self) -> Self {
        self.level = Some(AccessLevel::Database);
        self
    }
}

impl Statement for RemoveAccessStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "ACCESS", self.if_exists);
        out.push(&self.name);
        match self.level {
            Some(level) => {
                out.push_str(" ON ").push(&level);
            }
            None => {
                out.push_error(
                    "REMOVE ACCESS needs a level: `on_root`, `on_namespace` or `on_database`",
                );
            }
        }
    }
}

impl_if_exists!(RemoveAccessStatement);
impl_definition!(RemoveAccessStatement);

/// Creates a REMOVE ANALYZER statement builder.
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::remove_analyzer};
/// assert_eq!(remove_analyzer("standard").to_debug_sql(), "REMOVE ANALYZER standard;");
/// assert_eq!(
///     remove_analyzer("standard").if_exists().to_debug_sql(),
///     "REMOVE ANALYZER IF EXISTS standard;"
/// );
/// ```
pub fn remove_analyzer(name: impl Into<Name<Ident>>) -> RemoveAnalyzerStatement {
    RemoveAnalyzerStatement {
        name: name.into(),
        if_exists: false,
    }
}

/// Represents the REMOVE ANALYZER statement.
pub struct RemoveAnalyzerStatement {
    name: Name<Ident>,
    if_exists: bool,
}

impl Statement for RemoveAnalyzerStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "ANALYZER", self.if_exists);
        out.push(&self.name);
    }
}

impl_if_exists!(RemoveAnalyzerStatement);
impl_definition!(RemoveAnalyzerStatement);

/// Remove database statement.
///
/// # Arguments
///
/// * `database` - The name of the database to be removed.
///
/// # Example
/// ```rust
///  use umbriel::{*, statements::remove_database};
///  assert_eq!(
///          remove_database("acme").to_debug_sql(),
///          "REMOVE DATABASE acme;"
///      );
///
///  let acme = Database::new("acme");
///  assert_eq!(
///          remove_database(acme).if_exists().to_query().unwrap().sql(),
///          "REMOVE DATABASE IF EXISTS acme;"
///      );
/// ```
pub fn remove_database(database: impl Into<Name<Database>>) -> RemoveDatabaseStatement {
    RemoveDatabaseStatement {
        database: database.into(),
        if_exists: false,
    }
}

/// A statement for removing a database.
pub struct RemoveDatabaseStatement {
    database: Name<Database>,
    if_exists: bool,
}

impl Statement for RemoveDatabaseStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "DATABASE", self.if_exists);
        out.push(&self.database);
    }
}

impl_if_exists!(RemoveDatabaseStatement);
impl_definition!(RemoveDatabaseStatement);

/// Remove event statement
///
/// # Arguments
///
/// * `event` - The name of the event to be removed. Can be a string or an Event type.
///
/// # Example
/// ```rust
/// use umbriel::{*, statements::remove_event};
/// # let user = Table::new("user");
/// # let party = Event::new("party");
///
/// let statement = remove_event(party).on_table(user);
/// assert_eq!(statement.to_debug_sql(), "REMOVE EVENT party ON TABLE user;");
/// ```
pub fn remove_event(event: impl Into<Name<Event>>) -> RemoveEventStatement {
    RemoveEventStatement {
        event: event.into(),
        table: None,
        if_exists: false,
    }
}

/// Remove event statement
pub struct RemoveEventStatement {
    event: Name<Event>,
    table: Option<Name<Table>>,
    if_exists: bool,
}

impl RemoveEventStatement {
    /// Set the table to remove the event from.
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.table = Some(table.into());
        self
    }
}

impl Statement for RemoveEventStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "EVENT", self.if_exists);
        out.push(&self.event);
        push_table(out, self.table.as_ref());
    }
}

impl_if_exists!(RemoveEventStatement);
impl_definition!(RemoveEventStatement);

/// Remove field statement
///
/// # Arguments
/// * `field` - The name of the field to be removed. Can be a string or a Field type.
///
/// # Example
/// ```rust
/// use umbriel::{*, statements::remove_field};
/// # let user = Table::new("user");
/// # let name = Field::new("name");
///
/// let statement = remove_field(name).on_table(user);
/// assert_eq!(statement.to_debug_sql(), "REMOVE FIELD name ON TABLE user;");
/// ```
pub fn remove_field(field: impl Into<Field>) -> RemoveFieldStatement {
    RemoveFieldStatement {
        field: field.into(),
        table: None,
        if_exists: false,
    }
}

/// Remove field statement
pub struct RemoveFieldStatement {
    field: Field,
    table: Option<Name<Table>>,
    if_exists: bool,
}

impl RemoveFieldStatement {
    /// Set the table to remove the field from
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.table = Some(table.into());
        self
    }
}

impl Statement for RemoveFieldStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "FIELD", self.if_exists);
        out.push(&self.field);
        push_table(out, self.table.as_ref());
    }
}

impl_if_exists!(RemoveFieldStatement);
impl_definition!(RemoveFieldStatement);

/// A function's full name, e.g. `fn::update_author`, written verbatim.
pub struct FunctionName(Cow<'static, str>);

impl From<&str> for FunctionName {
    fn from(name: &str) -> Self {
        Self(Cow::Owned(name.to_owned()))
    }
}

impl From<String> for FunctionName {
    fn from(name: String) -> Self {
        Self(Cow::Owned(name))
    }
}

/// Creates a REMOVE FUNCTION statement builder. The name is a literal: SurrealDB does not read
/// function names from parameters.
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::remove_function};
/// assert_eq!(
///     remove_function("fn::update_author").to_debug_sql(),
///     "REMOVE FUNCTION fn::update_author;"
/// );
/// assert_eq!(
///     remove_function("fn::update_author").if_exists().to_debug_sql(),
///     "REMOVE FUNCTION IF EXISTS fn::update_author;"
/// );
/// ```
pub fn remove_function(name: impl Into<FunctionName>) -> RemoveFunctionStatement {
    RemoveFunctionStatement {
        name: name.into(),
        if_exists: false,
    }
}

/// Represents the REMOVE FUNCTION statement.
pub struct RemoveFunctionStatement {
    name: FunctionName,
    if_exists: bool,
}

impl Statement for RemoveFunctionStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "FUNCTION", self.if_exists);
        out.push_str(&self.name.0);
    }
}

impl_if_exists!(RemoveFunctionStatement);
impl_definition!(RemoveFunctionStatement);

/// Remove index statement
///
/// # Arguments
/// * `index` - The name of the index to be removed. Can be a string or a TableIndex type.
///
/// # Example
/// ```rust
/// use umbriel::{*, statements::remove_index};
/// # let user = Table::new("user");
/// # let party = TableIndex::new("party");
///
/// let statement = remove_index(party).on_table(user);
/// assert_eq!(statement.to_debug_sql(), "REMOVE INDEX party ON TABLE user;");
/// ```
pub fn remove_index(index: impl Into<Name<TableIndex>>) -> RemoveIndexStatement {
    RemoveIndexStatement {
        index: index.into(),
        table: None,
        if_exists: false,
    }
}

/// Remove index statement
pub struct RemoveIndexStatement {
    index: Name<TableIndex>,
    table: Option<Name<Table>>,
    if_exists: bool,
}

impl RemoveIndexStatement {
    /// Set the table to remove the index from
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.table = Some(table.into());
        self
    }
}

impl Statement for RemoveIndexStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "INDEX", self.if_exists);
        out.push(&self.index);
        push_table(out, self.table.as_ref());
    }
}

impl_if_exists!(RemoveIndexStatement);
impl_definition!(RemoveIndexStatement);

/// Remove namespace statement
///
/// # Arguments
///
/// * `namespace` - The name of the namespace to be removed. Can be a string or a Namespace type.
///
/// # Example
/// ```rust
/// use umbriel::{*, statements::remove_namespace};
///
/// # let namespace = Namespace::new("acme");
/// let statement = remove_namespace(namespace);
/// assert_eq!(statement.to_debug_sql(), "REMOVE NAMESPACE acme;");
/// ```
pub fn remove_namespace(namespace: impl Into<Name<Namespace>>) -> RemoveNamespaceStatement {
    RemoveNamespaceStatement {
        namespace: namespace.into(),
        if_exists: false,
    }
}

/// Remove namespace statement
pub struct RemoveNamespaceStatement {
    namespace: Name<Namespace>,
    if_exists: bool,
}

impl Statement for RemoveNamespaceStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "NAMESPACE", self.if_exists);
        out.push(&self.namespace);
    }
}

impl_if_exists!(RemoveNamespaceStatement);
impl_definition!(RemoveNamespaceStatement);

/// Creates a REMOVE PARAM statement builder. An invalid name (see [`Param::check`]) fails when
/// the query is rendered. The name is a literal: SurrealDB does not read param names from
/// parameters.
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::remove_param};
/// assert_eq!(remove_param("website_name").to_debug_sql(), "REMOVE PARAM $website_name;");
/// assert_eq!(
///     remove_param("website_name").if_exists().to_debug_sql(),
///     "REMOVE PARAM IF EXISTS $website_name;"
/// );
/// ```
pub fn remove_param(name: impl Into<Cow<'static, str>>) -> RemoveParamStatement {
    RemoveParamStatement {
        name: Param::new(name),
        if_exists: false,
    }
}

/// Represents the REMOVE PARAM statement.
pub struct RemoveParamStatement {
    name: Param,
    if_exists: bool,
}

impl Statement for RemoveParamStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "PARAM", self.if_exists);
        out.push(&self.name);
    }
}

impl_if_exists!(RemoveParamStatement);
impl_definition!(RemoveParamStatement);

/// Remove table statement
///
/// # Arguments
///
/// * `table` - The name of the table to be removed. Can be a string, a Table type or a
///   parameter holding the name.
///
/// # Example
/// ```rust
/// use umbriel::{*, statements::remove_table};
/// # let table = Table::new("user");
/// let statement = remove_table(table);
/// assert_eq!(statement.to_debug_sql(), "REMOVE TABLE user;");
///
/// let statement = remove_table(Param::new("table")).if_exists();
/// assert_eq!(statement.to_debug_sql(), "REMOVE TABLE IF EXISTS $table;");
/// ```
pub fn remove_table(table: impl Into<Name<Table>>) -> RemoveTableStatement {
    RemoveTableStatement {
        table: table.into(),
        if_exists: false,
    }
}

/// Remove table statement
pub struct RemoveTableStatement {
    table: Name<Table>,
    if_exists: bool,
}

impl Statement for RemoveTableStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "TABLE", self.if_exists);
        out.push(&self.table);
    }
}

impl_if_exists!(RemoveTableStatement);
impl_definition!(RemoveTableStatement);

/// Remove user statement
///
/// # Arguments
///
/// * `user` - User to be removed. Can be a string or a User type.
///
/// # Example
/// ```rust
/// use umbriel::{*, statements::remove_user};
/// let statement = remove_user("alice").on_namespace();
/// assert_eq!(statement.to_debug_sql(), "REMOVE USER alice ON NAMESPACE;");
///
/// let statement = remove_user("alice").on_database();
/// assert_eq!(statement.to_query().unwrap().sql(), "REMOVE USER alice ON DATABASE;");
///
/// let statement = remove_user("alice").on_database().if_exists();
/// assert_eq!(statement.to_debug_sql(), "REMOVE USER IF EXISTS alice ON DATABASE;");
/// ```
pub fn remove_user(user: impl Into<Name<User>>) -> RemoveUserStatementInit {
    RemoveUserStatementInit { user: user.into() }
}

/// The level a user is removed from.
#[derive(Clone, Copy)]
pub enum UserPermissionScope {
    /// The current namespace
    Namespace,
    /// The current database
    Database,
    /// The root level
    Root,
}

impl Display for UserPermissionScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UserPermissionScope::Namespace => "NAMESPACE",
            UserPermissionScope::Database => "DATABASE",
            UserPermissionScope::Root => "ROOT",
        })
    }
}

/// A REMOVE USER statement awaiting its level.
pub struct RemoveUserStatementInit {
    user: Name<User>,
}

impl RemoveUserStatementInit {
    /// Specify to remove the User from root
    pub fn on_root(self) -> RemoveUserStatement {
        self.on(UserPermissionScope::Root)
    }

    /// Specify to remove the User from namespace
    pub fn on_namespace(self) -> RemoveUserStatement {
        self.on(UserPermissionScope::Namespace)
    }

    /// Specify to remove the User from database
    pub fn on_database(self) -> RemoveUserStatement {
        self.on(UserPermissionScope::Database)
    }

    fn on(self, on: UserPermissionScope) -> RemoveUserStatement {
        RemoveUserStatement {
            user: self.user,
            on,
            if_exists: false,
        }
    }
}

/// Remove User statement
pub struct RemoveUserStatement {
    user: Name<User>,
    on: UserPermissionScope,
    if_exists: bool,
}

impl Statement for RemoveUserStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "USER", self.if_exists);
        out.push(&self.user)
            .push_str(" ON ")
            .push_display(&self.on);
    }
}

impl_if_exists!(RemoveUserStatement);
impl_definition!(RemoveUserStatement);
