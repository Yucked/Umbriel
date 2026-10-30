use std::{borrow::Cow, fmt};

use surrealdb::types::{Kind, Table};

use crate::{Param, Render, Renderer, TableOf};

/// A SurrealQL identifier, rendered bare when it is a plain identifier and escaped otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ident(Cow<'static, str>);

impl Ident {
    /// Creates an identifier. An empty name fails when the query is rendered.
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(name.into())
    }

    /// The unescaped name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Render for Ident {
    fn render(&self, out: &mut Renderer) {
        out.push_ident(&self.0);
    }
}

impl From<&str> for Ident {
    fn from(name: &str) -> Self {
        Self(Cow::Owned(name.to_owned()))
    }
}

impl From<String> for Ident {
    fn from(name: String) -> Self {
        Self(Cow::Owned(name))
    }
}

impl From<&Ident> for Ident {
    fn from(name: &Ident) -> Self {
        name.clone()
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Render for Table {
    fn render(&self, out: &mut Renderer) {
        if self.as_str().is_empty() {
            out.push_error("table names cannot be empty");
        }
        out.push_syntax(self);
    }
}

/// A type in SurrealDB's canonical syntax, e.g. `none | string`, `record<user>`.
impl Render for Kind {
    fn render(&self, out: &mut Renderer) {
        out.push_syntax(self);
    }
}

macro_rules! define_names {
    ($($(#[$attr:meta])* $name:ident),* $(,)?) => {
        $(
            $(#[$attr])*
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct $name(Ident);

            impl $name {
                /// Creates the name.
                pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
                    Self(Ident::new(name))
                }

                /// The unescaped name.
                pub fn as_str(&self) -> &str {
                    self.0.as_str()
                }
            }

            impl Render for $name {
                fn render(&self, out: &mut Renderer) {
                    self.0.render(out);
                }
            }

            impl From<&str> for $name {
                fn from(name: &str) -> Self {
                    Self(name.into())
                }
            }

            impl From<String> for $name {
                fn from(name: String) -> Self {
                    Self(name.into())
                }
            }

            impl From<Ident> for $name {
                fn from(name: Ident) -> Self {
                    Self(name)
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.0.fmt(f)
                }
            }
        )*
    };
}

define_names!(
    /// A namespace name.
    Namespace,
    /// A database name.
    Database,
    /// An access method name.
    Access,
    /// A user name.
    User,
    /// An event name.
    Event,
    /// An index name.
    TableIndex,
);

/// The name a definition, alteration or removal applies to: written out, or held by a parameter
/// that SurrealDB reads when the statement runs.
///
/// SurrealDB 3 reads the names of tables, fields, events, indexes, namespaces, databases,
/// analyzers, users and accesses as expressions, so `LET $table = 'user'; DEFINE TABLE $table`
/// defines `user`. Function and param names are literals and take no parameter.
///
/// ```rust
/// use umbriel::{*, statements::define_table};
///
/// assert_eq!(define_table("user").to_debug_sql(), "DEFINE TABLE user;");
/// assert_eq!(define_table(Param::new("table")).to_debug_sql(), "DEFINE TABLE $table;");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Name<T> {
    /// The name itself.
    Named(T),
    /// A parameter holding the name.
    Param(Param),
}

impl<T: Render> Render for Name<T> {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::Named(name) => name.render(out),
            Self::Param(param) => param.render(out),
        }
    }
}

impl<T> From<Param> for Name<T> {
    fn from(param: Param) -> Self {
        Self::Param(param)
    }
}

impl<T> From<&Param> for Name<T> {
    fn from(param: &Param) -> Self {
        Self::Param(param.clone())
    }
}

/// `From` for a [`Name`] of each listed type, from the type itself and from what converts into
/// it. A blanket `impl<U: Into<T>>` would overlap with `From<Param>`.
macro_rules! name_from {
    ($target:ty: $($source:ty),*) => {
        impl From<$target> for Name<$target> {
            fn from(name: $target) -> Self {
                Self::Named(name)
            }
        }

        $(
            impl From<$source> for Name<$target> {
                fn from(name: $source) -> Self {
                    Self::Named(name.into())
                }
            }
        )*
    };
}

name_from!(Table: &str, String);
name_from!(Ident: &str, String, &Ident);
name_from!(Namespace: &str, String, Ident);
name_from!(Database: &str, String, Ident);
name_from!(Access: &str, String, Ident);
name_from!(User: &str, String, Ident);
name_from!(Event: &str, String, Ident);
name_from!(TableIndex: &str, String, Ident);

impl<T> From<TableOf<T>> for Name<Table> {
    fn from(table: TableOf<T>) -> Self {
        Self::Named(table.into())
    }
}

/// A list of tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tables(Vec<Table>);

impl From<Vec<Table>> for Tables {
    fn from(value: Vec<Table>) -> Self {
        Self(value)
    }
}

impl<const N: usize> From<[Table; N]> for Tables {
    fn from(value: [Table; N]) -> Self {
        Self(value.into())
    }
}

impl<const N: usize> From<&[Table; N]> for Tables {
    fn from(value: &[Table; N]) -> Self {
        Self(value.to_vec())
    }
}

impl<const N: usize> From<&[&Table; N]> for Tables {
    fn from(value: &[&Table; N]) -> Self {
        Self(value.iter().map(|&table| table.clone()).collect())
    }
}

impl From<Tables> for Vec<Table> {
    fn from(value: Tables) -> Self {
        value.0
    }
}

impl Render for Tables {
    fn render(&self, out: &mut Renderer) {
        out.push_joined(&self.0, ", ");
    }
}
