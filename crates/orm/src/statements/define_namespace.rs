use crate::{Name, Namespace, Renderer};

use super::{DefineOptions, Statement, impl_define_options, impl_definition};

/// Define a new namespace .
/// SurrealDB has a multi-tenancy model which allows you to scope databases to a namespace.
/// There is no limit to the number of databases that can be in a namespace,
/// nor is there a limit to the number of namespaces allowed. Only users root users are
/// authorized to create namespaces.
///
/// Let's say that you're using SurrealDB to create a multi-tenant SaaS application. You can guarantee that the data of each tenant will be kept separate from other tenants if you put each tenant's databases into separate namespaces. In other words, this will ensure that information will remain siloed so user will only have access the information in the namespace they are a member of.
///
/// Requirements
/// You must be authenticated as a root user to use the DEFINE NAMESPACE statement.
///
/// Examples:
///
/// ```rust
/// use umbriel::{*, statements::{define_namespace}};
/// let acme = Namespace::new("acme");
///
/// let statement = define_namespace(acme);
///
/// assert_eq!(statement.to_debug_sql(), "DEFINE NAMESPACE acme;");
///
/// let statement = define_namespace("acme").if_not_exists().comment("Acme Corp");
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE NAMESPACE IF NOT EXISTS acme COMMENT 'Acme Corp';"
/// );
/// ```
pub fn define_namespace(namespace: impl Into<Name<Namespace>>) -> DefineNamespaceStatement {
    DefineNamespaceStatement {
        namespace: namespace.into(),
        options: DefineOptions::default(),
    }
}

/// Define namespace
pub struct DefineNamespaceStatement {
    namespace: Name<Namespace>,
    options: DefineOptions,
}

impl Statement for DefineNamespaceStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE NAMESPACE");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.namespace);
        self.options.render_comment(out);
    }
}

impl_define_options!(DefineNamespaceStatement);
impl_definition!(DefineNamespaceStatement);
