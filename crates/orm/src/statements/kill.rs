//! `KILL @id`: ends a live query. SurrealDB 3.3.4 takes a UUID or a parameter only
//! (`parse_kill_stmt`); a string is a parse error.

use surrealdb::types::{Uuid, Value};

use crate::{Param, Renderer};

use super::{Statement, impl_statement};

/// The live query a `KILL` ends: its id, or a parameter holding it.
pub enum KillId {
    /// The live query's id.
    Uuid(Uuid),
    /// A parameter holding the id.
    Param(Param),
}

impl From<Uuid> for KillId {
    fn from(id: Uuid) -> Self {
        Self::Uuid(id)
    }
}

impl From<Param> for KillId {
    fn from(param: Param) -> Self {
        Self::Param(param)
    }
}

impl From<&Param> for KillId {
    fn from(param: &Param) -> Self {
        Self::Param(param.clone())
    }
}

/// Ends a live query. Only its owner, or a root user, may.
///
/// ```rust
/// use umbriel::{*, statements::kill};
///
/// assert_eq!(kill(&Param::new("id")).to_debug_sql(), "KILL $id;");
/// ```
///
/// A live query's id is [`LiveStream::query_id`](super::LiveStream::query_id); its
/// [`kill`](super::LiveStream::kill) runs this statement.
pub fn kill(id: impl Into<KillId>) -> KillStatement {
    KillStatement(id.into())
}

/// A `KILL` statement.
#[must_use]
pub struct KillStatement(KillId);

impl Statement for KillStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("KILL ");
        match &self.0 {
            KillId::Uuid(id) => {
                out.push_value(&Value::Uuid(*id));
            }
            KillId::Param(param) => {
                out.push(param);
            }
        }
    }
}

impl_statement!(KillStatement);
