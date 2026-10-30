use surrealdb::types::Kind;

use crate::{Block, Param, Renderer};

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

    /// Sets the body of the function
    pub fn body(mut self, body: impl Into<Block>) -> Self {
        self.body = Some(body.into());
        self
    }
}

/// Create a new function definition statement
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
        // The name is a `::`-separated path, written as given.
        out.push_str("DEFINE FUNCTION");
        self.options.render_kind(out);
        out.push_str(" fn::").push_str(&self.name);
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
