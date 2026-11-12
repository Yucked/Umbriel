use std::{borrow::Cow, collections::HashSet};

use surrealdb::types::Kind;

use crate::{Param, Render, Renderer, ValueLike};

/// A SurrealQL closure, `|$n| $n * 2`, built by [`closure`].
///
/// A closure can be passed to the functions that take one (`array::map`, `array::fold`,
/// `set::filter`, …, through [`ClosureLike`](crate::ClosureLike)) or assigned to a parameter with
/// `LET` and called through it with [`Param::call`]. SurrealDB refuses to store one in a record.
#[derive(Debug, Clone, PartialEq)]
pub struct Closure {
    params: Vec<(Param, Option<Kind>)>,
    returns: Option<Kind>,
    body: ValueLike,
    /// Names given to [`Closure::typed`] that are not parameters of the closure.
    unknown: Vec<String>,
}

/// `|$a, $b| body`: a closure over the parameters `names`. `body` receives each parameter, in
/// order, and returns the closure's body.
///
/// Every name must be a parameter name (`[A-Za-z_][A-Za-z0-9_]*`, not starting with `_p`) and
/// appear once; otherwise the query fails to render and nothing of the closure is written.
///
/// ```rust
/// use umbriel::{*, functions::array};
/// let doubled = array::map!(vec![1, 2], closure(["n"], |[n]| n.multiply(2)));
/// assert_eq!(doubled.to_debug_sql(), "array::map([1, 2], |$n| $n * 2)");
/// ```
pub fn closure<const N: usize, B: Into<ValueLike>>(
    names: [&str; N],
    body: impl FnOnce([Param; N]) -> B,
) -> Closure {
    let params = names.map(|name| Param::new(Cow::Owned(name.to_owned())));
    Closure {
        params: params.iter().map(|param| (param.clone(), None)).collect(),
        returns: None,
        body: body(params).into(),
        unknown: vec![],
    }
}

impl Closure {
    /// Gives parameters a kind, `|$n: int| …`. Each name must be one of the closure's
    /// parameters; another fails the query when rendered.
    #[must_use]
    pub fn typed<'a>(mut self, kinds: impl IntoIterator<Item = (&'a str, Kind)>) -> Self {
        for (name, kind) in kinds {
            match self
                .params
                .iter_mut()
                .find(|(param, _)| param.name() == name)
            {
                Some((_, slot)) => *slot = Some(kind),
                None => self.unknown.push(name.to_owned()),
            }
        }
        self
    }

    /// The closure's result kind, `|$n| -> int { … }`; the body is written as a block, as
    /// SurrealDB requires with a result kind.
    #[must_use]
    pub fn returns(mut self, kind: Kind) -> Self {
        self.returns = Some(kind);
        self
    }

    /// The first reason the closure cannot be written, if any.
    fn error(&self) -> Option<String> {
        if let Some(name) = self.unknown.first() {
            return Some(format!(
                "closure kind given for `{name}`, which is not one of its parameters"
            ));
        }
        let mut seen = HashSet::with_capacity(self.params.len());
        for (param, _) in &self.params {
            if let Err(error) = Param::check(param.name()) {
                return Some(format!(
                    "invalid closure parameter `${}`: {error}",
                    param.name()
                ));
            }
            if !seen.insert(param.name()) {
                return Some(format!("closure parameter `${}` is repeated", param.name()));
            }
        }
        None
    }
}

/// Writes a kind as SurrealDB parses it after a closure parameter's `:`: a union inside `<…>`.
fn push_param_kind(out: &mut Renderer, kind: &Kind) {
    if matches!(kind, Kind::Either(_)) {
        out.push_str("<").push_syntax(kind).push_str(">");
    } else {
        out.push_syntax(kind);
    }
}

impl Render for Closure {
    fn render(&self, out: &mut Renderer) {
        if let Some(error) = self.error() {
            out.push_error(error);
            return;
        }
        out.push_str("|");
        for (index, (param, kind)) in self.params.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push(param);
            if let Some(kind) = kind {
                out.push_str(": ");
                push_param_kind(out, kind);
            }
        }
        out.push_str("| ");
        match &self.returns {
            Some(kind) => {
                out.push_str("-> ")
                    .push_syntax(kind)
                    .push_str(" { ")
                    .push(&self.body)
                    .push_str(" }");
            }
            None => {
                out.push(&self.body);
            }
        }
    }
}

/// A closure is a value: it can be assigned with `LET` or passed to a function.
impl From<Closure> for ValueLike {
    fn from(closure: Closure) -> Self {
        ValueLike::new(closure)
    }
}
