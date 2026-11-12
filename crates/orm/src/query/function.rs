use std::{borrow::Cow, fmt, marker::PhantomData, str::FromStr};

use surrealdb::{Surreal, types::SurrealValue};

use crate::{
    Aliasable, Param, Render, Renderer, Result, Runnable, ValueLike,
    kinds::{self, KindOf},
    render::push_path,
    statements::return_,
};

/// A function call, e.g. `math::sum(age)`, typed by the kind `K` of its result.
///
/// The kind decides which typed expressions the result enters (see [`kinds`]): the result of
/// `string::len` is a `Function<kinds::Int>`, so it is a [`NumberLike`](crate::NumberLike) but not
/// a [`StrandLike`](crate::StrandLike). Calls whose result is not known statically are
/// `Function<kinds::Any>`, which enter every typed expression unchecked; [`Function::typed`] gives
/// such a call the kind its definition returns.
pub struct Function<K: KindOf = kinds::Any> {
    callee: Callee,
    args: Vec<ValueLike>,
    kind: PhantomData<fn() -> K>,
}

/// What a call names.
#[derive(Debug, Clone, PartialEq)]
enum Callee {
    /// A built-in or custom function written by name, e.g. `math::sum` or `fn::greet`.
    Name(Cow<'static, str>),
    /// `prefix::path`, each `::`-separated segment of `path` checked when rendering: `mod::a::b`.
    Path {
        prefix: &'static str,
        path: Cow<'static, str>,
    },
    /// `ml::path<version>`
    Model {
        path: Cow<'static, str>,
        version: MlVersion,
    },
    /// `receiver.method(args)`: the method form of a built-in function.
    Method {
        receiver: ValueLike,
        name: &'static str,
    },
    /// `$param(args)`: a closure held by a parameter.
    Param(Param),
    /// `function(args) body`: embedded JavaScript, written verbatim.
    Script(Cow<'static, str>),
}

impl<K: KindOf> Function<K> {
    /// A call to the function `name`, e.g. `math::sum`, with no arguments yet.
    pub(crate) fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self::with_callee(Callee::Name(name.into()))
    }

    /// The method `name` called on `receiver`, e.g. `name.len()`.
    pub(crate) fn method(receiver: ValueLike, name: &'static str) -> Self {
        Self::with_callee(Callee::Method { receiver, name })
    }

    /// `ml::path<version>`, with each segment of `path` checked when rendering.
    pub(crate) fn model(path: Cow<'static, str>, version: MlVersion) -> Self {
        Self::with_callee(Callee::Model { path, version })
    }

    /// `mod::path`, with each segment of `path` checked when rendering.
    pub(crate) fn module(path: Cow<'static, str>) -> Self {
        Self::with_callee(Callee::Path {
            prefix: "mod",
            path,
        })
    }

    fn with_callee(callee: Callee) -> Self {
        Self {
            callee,
            args: vec![],
            kind: PhantomData,
        }
    }

    /// Appends an argument.
    pub(crate) fn arg(mut self, arg: impl Into<ValueLike>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Appends each argument in order.
    pub(crate) fn args<A: Into<ValueLike>>(mut self, args: impl IntoIterator<Item = A>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// The same call with its result untyped, so it enters every typed expression unchecked; e.g.
    /// to keep calls of different kinds in one list.
    #[must_use]
    pub fn untyped(self) -> Function<kinds::Any> {
        Function {
            callee: self.callee,
            args: self.args,
            kind: PhantomData,
        }
    }

    /// Runs `RETURN <call>` with the call's values bound, through the same execution as
    /// statements, and decodes the result as `R`: `None` when it is `NONE`.
    ///
    /// # Errors
    ///
    /// Fails as [`Runnable::run`] does: when the call does not render, when SurrealDB refuses
    /// it, or when the result does not decode as `R`.
    pub fn run<R: SurrealValue>(
        &self,
        db: Surreal<impl surrealdb::Connection>,
    ) -> impl Future<Output = Result<Option<R>>> + Send {
        let statement = return_(self.clone());
        async move { statement.get_data(db).await }
    }
}

impl Function<kinds::Any> {
    /// Gives an untyped call the kind `T` its definition returns, e.g. a `fn::` function declared
    /// `-> int`. The kind is not checked: it is the caller's statement about the definition.
    #[must_use]
    pub fn typed<T: KindOf>(self) -> Function<T> {
        Function {
            callee: self.callee,
            args: self.args,
            kind: PhantomData,
        }
    }

    /// A call to the custom function `fn::<name>`; used by the helpers `define_function!`
    /// generates.
    #[doc(hidden)]
    pub fn __custom(name: &str, args: Vec<ValueLike>) -> Self {
        Self::new(format!("fn::{name}")).args(args)
    }

    /// `$param(args)`: calls the closure a parameter holds.
    pub(crate) fn param(param: Param) -> Self {
        Self::with_callee(Callee::Param(param))
    }

    /// `function(args) body`: embedded JavaScript.
    pub(crate) fn script(body: impl Into<Cow<'static, str>>) -> Self {
        Self::with_callee(Callee::Script(body.into()))
    }
}

impl<K: KindOf> Clone for Function<K> {
    fn clone(&self) -> Self {
        Self {
            callee: self.callee.clone(),
            args: self.args.clone(),
            kind: PhantomData,
        }
    }
}

impl<K: KindOf> fmt::Debug for Function<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Function")
            .field("callee", &self.callee)
            .field("args", &self.args)
            .field("kind", &K::kind())
            .finish()
    }
}

impl<K: KindOf> PartialEq for Function<K> {
    fn eq(&self, other: &Self) -> bool {
        self.callee == other.callee && self.args == other.args
    }
}

impl<K: KindOf> Render for Function<K> {
    fn render(&self, out: &mut Renderer) {
        match &self.callee {
            Callee::Name(name) => {
                out.push_str(name);
            }
            Callee::Path { prefix, path } => {
                if !push_path(out, prefix, path) {
                    return;
                }
            }
            Callee::Model { path, version } => {
                if !push_path(out, "ml", path) {
                    return;
                }
                out.push_str("<").push_display(version).push_str(">");
            }
            Callee::Method { receiver, name } => {
                receiver.render_receiver(out);
                out.push_str(".").push_str(name);
            }
            Callee::Param(param) => {
                out.push(param);
            }
            Callee::Script(_) => {
                out.push_str("function");
            }
        }
        out.push_str("(").push(&self.args).push_str(")");
        if let Callee::Script(body) = &self.callee {
            out.push_str(" ").push_str(body);
        }
    }
}

impl<K: KindOf> Aliasable for Function<K> {}

/// A SurrealML model version, `major.minor.patch`, as `ml::` calls take it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MlVersion {
    /// The major version.
    pub major: u32,
    /// The minor version.
    pub minor: u32,
    /// The patch version.
    pub patch: u32,
}

impl MlVersion {
    /// The version `major.minor.patch`.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for MlVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A version that is not exactly three `.`-separated unsigned integers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MlVersionError(String);

impl fmt::Display for MlVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid model version `{}`: expected `major.minor.patch`, each an unsigned integer",
            self.0
        )
    }
}

impl std::error::Error for MlVersionError {}

/// Parses exactly `N.N.N`, where each `N` is ASCII digits fitting a `u32`: SurrealDB's parser
/// takes nothing else (no sign, pre-release or build suffix, and no more or fewer parts).
impl FromStr for MlVersion {
    type Err = MlVersionError;

    fn from_str(version: &str) -> std::result::Result<Self, Self::Err> {
        let error = || MlVersionError(version.to_owned());
        let part = |text: Option<&str>| {
            text.filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|text| text.parse::<u32>().ok())
                .ok_or_else(error)
        };
        let mut parts = version.split('.');
        let parsed = Self::new(
            part(parts.next())?,
            part(parts.next())?,
            part(parts.next())?,
        );
        if parts.next().is_some() {
            return Err(error());
        }
        Ok(parsed)
    }
}
