use surrealdb::types::Duration;

use crate::{Database, Name, Renderer};

use super::{
    DefineOptions, Statement, define_table::push_changefeed, impl_define_options, impl_definition,
};

/// Define a new database statement.
///
/// # Arguments
///
/// * `database` - The name of the database to be defined.
///
/// # Example
/// ```rust
///  use umbriel::{*, statements::define_database};
///  assert_eq!(
///          define_database("acme").to_debug_sql(),
///          "DEFINE DATABASE acme;"
///      );
///
///  let acme = Database::new("acme");
///  assert_eq!(
///          define_database(acme).to_query().unwrap().sql(),
///          "DEFINE DATABASE acme;"
///      );
///
///  assert_eq!(
///          define_database("acme").overwrite().strict().to_debug_sql(),
///          "DEFINE DATABASE OVERWRITE acme STRICT;"
///      );
/// ```
pub fn define_database(database: impl Into<Name<Database>>) -> DefineDatabaseStatement {
    DefineDatabaseStatement {
        database: database.into(),
        options: DefineOptions::default(),
        strict: false,
        changefeed: None,
        include_original: false,
    }
}

/// A statement for defining a database.
pub struct DefineDatabaseStatement {
    database: Name<Database>,
    options: DefineOptions,
    strict: bool,
    changefeed: Option<Duration>,
    include_original: bool,
}

impl DefineDatabaseStatement {
    /// `CHANGEFEED @expiry`: records every change in the database, kept for `expiry`, for
    /// [`show_changes_for_database`](super::show_changes_for_database()). SurrealDB takes the
    /// duration only as a literal.
    ///
    /// No `ALTER` changes it: `ALTER DATABASE` takes only `COMPACT`. To change it, redefine the
    /// database with [`overwrite`](Self::overwrite). Migrations do not manage the database's
    /// own definition, so they neither write nor compare it.
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use umbriel::{*, statements::define_database};
    ///
    /// let statement = define_database("acme").changefeed(Duration::from_secs(3600));
    /// assert_eq!(statement.to_debug_sql(), "DEFINE DATABASE acme CHANGEFEED 1h;");
    /// ```
    pub fn changefeed(mut self, expiry: impl Into<Duration>) -> Self {
        self.changefeed = Some(expiry.into());
        self
    }

    /// `INCLUDE ORIGINAL`: each recorded update also holds the patch that reverts it, and each
    /// delete the deleted record. Without [`changefeed`](Self::changefeed), the query fails when
    /// it is rendered.
    pub fn include_original(mut self) -> Self {
        self.include_original = true;
        self
    }

    /// `STRICT`: a write to a table the database does not define fails, instead of defining the
    /// table on first use.
    ///
    /// No `ALTER` changes it. To change it, redefine the database with
    /// [`overwrite`](Self::overwrite).
    pub fn strict(mut self) -> Self {
        self.strict = true;
        self
    }
}

impl Statement for DefineDatabaseStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE DATABASE");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.database);
        if self.strict {
            out.push_str(" STRICT");
        }
        push_changefeed(out, self.changefeed.as_ref(), self.include_original);
        self.options.render_comment(out);
    }
}

impl_define_options!(DefineDatabaseStatement);
impl_definition!(DefineDatabaseStatement);
