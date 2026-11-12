use std::{borrow::Cow, fmt};

use crate::{
    Clause, Field, Function, GENERATED_PARAM_PREFIX, Index, Operatable, Render, Renderer,
    SchemaGetter, ValueLike,
};

/// Why a caller-supplied parameter name was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamNameError {
    /// The name is empty.
    Empty,
    /// The name is not `[A-Za-z_][A-Za-z0-9_]*`.
    InvalidCharacter,
    /// The name starts with the prefix reserved for generated parameters.
    Reserved,
}

impl ParamNameError {
    const fn message(self) -> &'static str {
        match self {
            Self::Empty => "parameter names cannot be empty",
            Self::InvalidCharacter => {
                "parameter names must start with a letter or `_` and contain only ASCII letters, digits and `_`"
            }
            Self::Reserved => "parameter names starting with `_p` are reserved for bound values",
        }
    }
}

impl fmt::Display for ParamNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for ParamNameError {}

/// A SurrealQL parameter, `$name`, named by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Param(Cow<'static, str>);

impl Param {
    /// Creates a parameter reference. A name rejected by [`Param::check`] fails when the query is
    /// rendered; use `Param::try_from` to reject it immediately.
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(name.into())
    }

    /// Validates a caller-supplied parameter name. This is the single rule for every
    /// caller-named parameter, at runtime and in macro-generated constants.
    pub const fn check(name: &str) -> Result<(), ParamNameError> {
        let bytes = name.as_bytes();
        if bytes.is_empty() {
            return Err(ParamNameError::Empty);
        }
        if !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
            return Err(ParamNameError::InvalidCharacter);
        }
        let mut i = 1;
        while i < bytes.len() {
            if !(bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                return Err(ParamNameError::InvalidCharacter);
            }
            i += 1;
        }
        let prefix = GENERATED_PARAM_PREFIX.as_bytes();
        if bytes.len() < prefix.len() {
            return Ok(());
        }
        let mut i = 0;
        while i < prefix.len() {
            if bytes[i] != prefix[i] {
                return Ok(());
            }
            i += 1;
        }
        Err(ParamNameError::Reserved)
    }

    /// Fails compilation when evaluated in a constant with an invalid name. Emitted by macros.
    #[doc(hidden)]
    pub const fn assert_valid(name: &str) {
        if let Err(error) = Self::check(name) {
            panic!("{}", error.message());
        }
    }

    /// The name without `$`.
    pub fn name(&self) -> &str {
        &self.0
    }

    /// Traverses from the parameter into a model's schema.
    pub fn with_path<T: SchemaGetter>(&self, clause: impl Into<Clause>) -> T::Schema {
        let mut path = Field::from(self);
        path.push_clause(clause.into());
        T::schema_prefixed(path)
    }

    /// `$param(args)`: calls the closure the parameter holds, e.g. `$double(3)` after
    /// `LET $double = |$n| $n * 2`. The result is untyped; [`Function::typed`] gives it the
    /// closure's result kind.
    pub fn call<A: Into<ValueLike>>(&self, args: impl IntoIterator<Item = A>) -> Function {
        Function::param(self.clone()).args(args)
    }

    /// Accesses an element of the list the parameter holds.
    pub fn index(&self, index: impl Into<Index>) -> Field {
        let mut path = Field::from(self);
        path.push_clause(index.into().into());
        path
    }
}

impl TryFrom<&str> for Param {
    type Error = ParamNameError;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Self::check(name)?;
        Ok(Self(Cow::Owned(name.to_owned())))
    }
}

impl From<&Param> for Param {
    fn from(param: &Param) -> Self {
        param.clone()
    }
}

impl Render for Param {
    fn render(&self, out: &mut Renderer) {
        if let Err(error) = Self::check(&self.0) {
            out.push_error(format!("invalid parameter `${}`: {error}", self.0));
        }
        out.push_str("$").push_str(&self.0);
    }
}

impl Operatable for Param {}
