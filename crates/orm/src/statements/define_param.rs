use std::ops::Deref;

use crate::{Param, Renderer, ValueLike};

use super::{
    DefineOptions, Statement,
    for_permission::{PermissionValue, impl_permissions},
    impl_define_options, impl_definition,
};

/// Define a new parameter. DEFINE PARAM statement allows you to define global (database-wide) parameters that are available to every client.
///
/// Requirements
/// You must be authenticated as a root, namespace, or database user before you can use the DEFINE PARAM statement.
/// You must select your namespace and database before you can use the DEFINE PARAM statement.
///
/// Arguments
/// name: The name of the parameter.
/// value: The value of the parameter.
///
/// Example
///
/// ```rust
/// use umbriel::{*, statements::{define_param}};
///   // First create the param name as rust function i.e endpoint_base()
///   create_param_name_fn!(endpoint_base);
///   // Can also we wwriten as below if you want to add a doc comment.
///   // create_param_name_fn!(
///   //  /// endpoint of example.com
///   //  =>
///   //  endpoint_base
///   // );
/// let statement = define_param(endpoint_base()).value("https://example.com");
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE PARAM $endpoint_base VALUE 'https://example.com';"
/// );
/// ```
pub fn define_param(param_name: impl Deref<Target = Param>) -> DefineParamStatementBuilder {
    DefineParamStatementBuilder(param_name.deref().clone())
}

/// A DEFINE PARAM statement awaiting its value.
pub struct DefineParamStatementBuilder(Param);

/// Define param statement
pub struct DefineParamStatement {
    name: Param,
    value: ValueLike,
    options: DefineOptions,
    permissions: Option<PermissionValue>,
}

impl DefineParamStatementBuilder {
    /// Set the value of the parameter.
    pub fn value(self, value: impl Into<ValueLike>) -> DefineParamStatement {
        DefineParamStatement {
            name: self.0,
            value: value.into(),
            options: DefineOptions::default(),
            permissions: None,
        }
    }
}

impl_permissions!(single: DefineParamStatement);

impl Statement for DefineParamStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE PARAM");
        self.options.render_kind(out);
        out.push_str(" ")
            .push(&self.name)
            .push_str(" VALUE ")
            .push(&self.value);
        self.options.render_comment(out);
        if let Some(permissions) = &self.permissions {
            out.push(permissions);
        }
    }
}

impl_define_options!(DefineParamStatement);
impl_definition!(DefineParamStatement);
