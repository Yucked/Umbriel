use surrealdb::types::Kind;

use crate::{Block, Param, Renderer, render::push_function_path};

use super::{DefineOptions, Statement, impl_define_options, impl_definition};

/// Represents a surrealdb define function statement argument
#[derive(Debug, Clone)]
pub struct FunctionArgument {
    /// The name of the argument
    pub name: Param,
    /// The type of the argument
    pub type_: Kind,
}

/// A function definition statement
#[derive(Debug, Clone)]
pub struct DefineFunctionStatement {
    name: String,
    args: Vec<FunctionArgument>,
    body: Option<Block>,
    options: DefineOptions,
}

impl DefineFunctionStatement {
    /// Create a new function definition statement
    ///
    /// # Arguments
    /// * `name` - The name of the function, without the `fn::` prefix
    pub fn new(name: String) -> Self {
        define_function(name)
    }

    /// Sets the arguments for the function
    pub fn arguments(mut self, args: Vec<FunctionArgument>) -> Self {
        self.args = args;
        self
    }

    /// Sets the body of the function.
    ///
    /// The body runs inside the calling statement's transaction. `BEGIN`, `COMMIT` and `CANCEL`
    /// are transaction statements only at the top level of a query: SurrealDB 3.3.4 accepts them
    /// in a body but reads them as plain values, so they neither start nor end a transaction.
    pub fn body(mut self, body: impl Into<Block>) -> Self {
        self.body = Some(body.into());
        self
    }
}

/// Creates a `DEFINE FUNCTION fn::name` statement. The name is the path after `fn::`; each
/// `::`-separated part must be an identifier (`[A-Za-z_][A-Za-z0-9_]*`), and a name still
/// starting with `fn::` fails, when the query is rendered.
///
/// ```rust
/// use umbriel::{*, statements::define_function};
///
/// assert!(
///     define_function("math::add_one")
///         .to_debug_sql()
///         .starts_with("DEFINE FUNCTION fn::math::add_one(")
/// );
/// assert!(define_function("fn::math::add_one").to_query().is_err());
/// ```
pub fn define_function(name: impl Into<String>) -> DefineFunctionStatement {
    DefineFunctionStatement {
        name: name.into(),
        args: vec![],
        body: None,
        options: DefineOptions::default(),
    }
}

/// Writes a function's arguments, `($name: type, …)`, as `DEFINE` and `ALTER FUNCTION` take them.
pub(crate) fn push_arguments(out: &mut Renderer, args: &[FunctionArgument]) {
    out.push_str("(");
    for (index, FunctionArgument { name, type_ }) in args.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push(name).push_str(": ").push(type_);
    }
    out.push_str(")");
}

impl Statement for DefineFunctionStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE FUNCTION");
        self.options.render_kind(out);
        out.push_str(" ");
        if !push_function_path(out, &self.name) {
            return;
        }
        push_arguments(out, &self.args);
        out.push_str(" ");
        if let Some(body) = &self.body {
            out.push(body);
        }
        self.options.render_comment(out);
    }
}

impl_define_options!(DefineFunctionStatement);
impl_definition!(DefineFunctionStatement);
