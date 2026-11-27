//! `DEFINE CONFIG`: the API, GraphQL and default configurations.
//!
//! SurrealDB 3.3.4 stores no comment for a config, and its `ALTER CONFIG` drops the comment it
//! is given, writes `DEFAULT` to a database key the root default never reads, and creates a
//! config that does not exist (`surrealdb-core` `alter/config.rs`). So there is no
//! `alter_config_*`: a config is changed by redefining it with `overwrite`.

use crate::{Database, Name, Namespace, Renderer, render::push_function_path};

use super::{DefineOptions, Statement, define_api::ApiConfig, impl_define_kind, impl_definition};

/// Defines the database's API configuration: `DEFINE CONFIG [OVERWRITE | IF NOT EXISTS] API
/// [PERMISSIONS …] [MIDDLEWARE …]`, which applies to every API of the database.
///
/// ```rust
/// use umbriel::{*, statements::{api_config, define_config_api}};
///
/// let statement = define_config_api(api_config().permissions_none()).overwrite();
/// assert_eq!(statement.to_debug_sql(), "DEFINE CONFIG OVERWRITE API PERMISSIONS NONE;");
/// ```
pub fn define_config_api(config: ApiConfig) -> DefineConfigApiStatement {
    DefineConfigApiStatement {
        options: DefineOptions::default(),
        config,
    }
}

/// A `DEFINE CONFIG API` statement.
pub struct DefineConfigApiStatement {
    options: DefineOptions,
    config: ApiConfig,
}

impl Statement for DefineConfigApiStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE CONFIG");
        self.options.render_kind(out);
        out.push_str(" API").push(&self.config);
    }
}

impl_define_kind!(DefineConfigApiStatement);
impl_definition!(DefineConfigApiStatement);

/// Which tables or functions GraphQL exposes.
#[derive(Debug, Clone)]
enum Exposed {
    Auto,
    None,
    Include(Vec<String>),
    Exclude(Vec<String>),
}

impl Exposed {
    /// Writes ` <keyword> AUTO | NONE | INCLUDE … | EXCLUDE …`, each name through `push_name`.
    fn render(&self, out: &mut Renderer, keyword: &str, push_name: fn(&mut Renderer, &str)) {
        out.push_str(" ").push_str(keyword).push_str(" ");
        let (word, names) = match self {
            Self::Auto => {
                out.push_str("AUTO");
                return;
            }
            Self::None => {
                out.push_str("NONE");
                return;
            }
            Self::Include(names) => ("INCLUDE ", names),
            Self::Exclude(names) => ("EXCLUDE ", names),
        };
        if names.is_empty() {
            out.push_error(format!(
                "{keyword} INCLUDE and EXCLUDE need at least one name"
            ));
            return;
        }
        out.push_str(word)
            .push_joined_with(names, ", ", |out, name| {
                push_name(out, name);
            });
    }
}

/// Defines the database's GraphQL configuration: `DEFINE CONFIG [OVERWRITE | IF NOT EXISTS]
/// GRAPHQL [TABLES …] [FUNCTIONS …] [DEPTH n] [COMPLEXITY n] [INTROSPECTION AUTO | NONE]`.
/// SurrealDB exposes no tables and no functions unless told to.
///
/// ```rust
/// use umbriel::{*, statements::define_config_graphql};
///
/// let statement = define_config_graphql()
///     .tables_include(["user", "post"])
///     .functions_auto()
///     .depth(5);
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE CONFIG GRAPHQL TABLES INCLUDE user, post FUNCTIONS AUTO DEPTH 5;"
/// );
/// ```
pub fn define_config_graphql() -> DefineConfigGraphqlStatement {
    DefineConfigGraphqlStatement {
        options: DefineOptions::default(),
        tables: None,
        functions: None,
        depth: None,
        complexity: None,
        introspection: None,
    }
}

/// A `DEFINE CONFIG GRAPHQL` statement.
pub struct DefineConfigGraphqlStatement {
    options: DefineOptions,
    tables: Option<Exposed>,
    functions: Option<Exposed>,
    depth: Option<u32>,
    complexity: Option<u32>,
    introspection: Option<bool>,
}

impl DefineConfigGraphqlStatement {
    /// `TABLES AUTO`: every table.
    pub fn tables_auto(mut self) -> Self {
        self.tables = Some(Exposed::Auto);
        self
    }

    /// `TABLES NONE`.
    pub fn tables_none(mut self) -> Self {
        self.tables = Some(Exposed::None);
        self
    }

    /// `TABLES INCLUDE …`: only these tables.
    pub fn tables_include(mut self, tables: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tables = Some(Exposed::Include(
            tables.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// `TABLES EXCLUDE …`: every table but these.
    pub fn tables_exclude(mut self, tables: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tables = Some(Exposed::Exclude(
            tables.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// `FUNCTIONS AUTO`: every function.
    pub fn functions_auto(mut self) -> Self {
        self.functions = Some(Exposed::Auto);
        self
    }

    /// `FUNCTIONS NONE`.
    pub fn functions_none(mut self) -> Self {
        self.functions = Some(Exposed::None);
        self
    }

    /// `FUNCTIONS INCLUDE fn::…`: only these functions, each named by the path after `fn::`.
    pub fn functions_include(
        mut self,
        functions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.functions = Some(Exposed::Include(
            functions.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// `FUNCTIONS EXCLUDE fn::…`: every function but these, each named by the path after `fn::`.
    pub fn functions_exclude(
        mut self,
        functions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.functions = Some(Exposed::Exclude(
            functions.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// `DEPTH n`: the deepest query nesting accepted.
    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = Some(depth);
        self
    }

    /// `COMPLEXITY n`: the most complex query accepted.
    pub fn complexity(mut self, complexity: u32) -> Self {
        self.complexity = Some(complexity);
        self
    }

    /// `INTROSPECTION AUTO`, or `INTROSPECTION NONE` when `enabled` is false.
    pub fn introspection(mut self, enabled: bool) -> Self {
        self.introspection = Some(enabled);
        self
    }
}

impl Statement for DefineConfigGraphqlStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE CONFIG");
        self.options.render_kind(out);
        out.push_str(" GRAPHQL");
        if let Some(tables) = &self.tables {
            tables.render(out, "TABLES", |out, table| {
                out.push_ident(table);
            });
        }
        if let Some(functions) = &self.functions {
            functions.render(out, "FUNCTIONS", |out, function| {
                push_function_path(out, function);
            });
        }
        if let Some(depth) = self.depth {
            out.push_str(" DEPTH ").push_display(&depth);
        }
        if let Some(complexity) = self.complexity {
            out.push_str(" COMPLEXITY ").push_display(&complexity);
        }
        match self.introspection {
            Some(true) => {
                out.push_str(" INTROSPECTION AUTO");
            }
            Some(false) => {
                out.push_str(" INTROSPECTION NONE");
            }
            None => {}
        }
    }
}

impl_define_kind!(DefineConfigGraphqlStatement);
impl_definition!(DefineConfigGraphqlStatement);

/// Defines the root's default namespace and database, used by a connection that selects none:
/// `DEFINE CONFIG [OVERWRITE | IF NOT EXISTS] DEFAULT [NAMESPACE @ns] [DATABASE @db]`. It is
/// stored on the root, so `INFO FOR ROOT` shows it as `defaults`. At least one is required.
///
/// ```rust
/// use umbriel::{*, statements::define_config_default};
///
/// let statement = define_config_default().namespace("acme").database("app");
/// assert_eq!(statement.to_debug_sql(), "DEFINE CONFIG DEFAULT NAMESPACE acme DATABASE app;");
/// assert!(define_config_default().to_query().is_err());
/// ```
pub fn define_config_default() -> DefineConfigDefaultStatement {
    DefineConfigDefaultStatement {
        options: DefineOptions::default(),
        namespace: None,
        database: None,
    }
}

/// A `DEFINE CONFIG DEFAULT` statement.
pub struct DefineConfigDefaultStatement {
    options: DefineOptions,
    namespace: Option<Name<Namespace>>,
    database: Option<Name<Database>>,
}

impl DefineConfigDefaultStatement {
    /// `NAMESPACE @ns`.
    pub fn namespace(mut self, namespace: impl Into<Name<Namespace>>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// `DATABASE @db`.
    pub fn database(mut self, database: impl Into<Name<Database>>) -> Self {
        self.database = Some(database.into());
        self
    }
}

impl Statement for DefineConfigDefaultStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE CONFIG");
        self.options.render_kind(out);
        out.push_str(" DEFAULT");
        if self.namespace.is_none() && self.database.is_none() {
            out.push_error("DEFINE CONFIG DEFAULT needs a namespace, a database or both");
        }
        if let Some(namespace) = &self.namespace {
            out.push_str(" NAMESPACE ").push(namespace);
        }
        if let Some(database) = &self.database {
            out.push_str(" DATABASE ").push(database);
        }
    }
}

impl_define_kind!(DefineConfigDefaultStatement);
impl_definition!(DefineConfigDefaultStatement);
