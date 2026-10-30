use crate::{Database, Name, Renderer};

use super::{DefineOptions, Statement, impl_define_options, impl_definition};

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
    }
}

/// A statement for defining a database.
pub struct DefineDatabaseStatement {
    database: Name<Database>,
    options: DefineOptions,
    strict: bool,
}

impl DefineDatabaseStatement {
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
        self.options.render_comment(out);
    }
}

impl_define_options!(DefineDatabaseStatement);
impl_definition!(DefineDatabaseStatement);
