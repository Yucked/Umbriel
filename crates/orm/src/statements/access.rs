//! The `ACCESS` statement, which manages the grants of a bearer access method:
//! `ACCESS @name [ON ROOT | NAMESPACE | DATABASE] GRANT | SHOW | REVOKE | PURGE …`.
//! Syntax: <https://surrealdb.com/docs/surrealql/statements/access>, checked against
//! `parse_access` in `surrealdb-syn` 3.3.4.

use std::marker::PhantomData;

use surrealdb::types::{self, Duration, SurrealValue, Value};

use crate::{
    Access, Conditional, Error, Executor, Filter, Level, Render, Renderer, Result, User, ValueLike,
    execution::execute,
};

use super::{Statement, impl_statement};

/// A grant of an access method, as `ACCESS … GRANT`, `SHOW` and `REVOKE` return it
/// (`access_object_from_grant` in `surrealdb-core` 3.3.4).
#[derive(SurrealValue, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AccessGrant {
    /// The grant's id, which `show_grant` and `revoke_grant` take.
    pub id: String,
    /// The access method's name.
    pub ac: String,
    /// The grant's kind: `bearer` for the grants this statement makes.
    #[surreal(rename = "type")]
    pub kind: String,
    /// When the grant was made.
    pub creation: types::Datetime,
    /// When the grant expires, from the method's `DURATION FOR GRANT`, or `None` for never.
    pub expiration: Option<types::Datetime>,
    /// When the grant was revoked, or `None` while it's in force.
    pub revocation: Option<types::Datetime>,
    /// Who the grant is for.
    pub subject: GrantSubject,
    /// The bearer key.
    pub grant: BearerKey,
}

/// Who a grant is for: a system user or a record user.
#[derive(SurrealValue, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GrantSubject {
    /// The system user's name, for a grant to a user.
    pub user: Option<String>,
    /// The record, for a grant to a record user.
    pub record: Option<types::RecordId>,
}

/// A bearer key. SurrealDB returns the `key` in full once, from `GRANT`; `SHOW` and `REVOKE`
/// return it redacted.
#[derive(SurrealValue, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BearerKey {
    /// The key's id.
    pub id: String,
    /// The key a bearer signs in with, as the `key` variable of record-style credentials.
    pub key: String,
}

/// Starts an `ACCESS` statement for the bearer access method `name`.
///
/// ```rust
/// use umbriel::{*, statements::access};
///
/// assert_eq!(
///     access("api").on(Level::Database).grant_for_user("alice").to_debug_sql(),
///     "ACCESS api ON DATABASE GRANT FOR USER alice;"
/// );
/// assert_eq!(access("api").show_all().to_debug_sql(), "ACCESS api SHOW ALL;");
/// assert_eq!(
///     access("api").purge_revoked().purge_expired().for_(std::time::Duration::from_secs(86_400).into())
///         .to_debug_sql(),
///     "ACCESS api PURGE EXPIRED, REVOKED FOR 1d;"
/// );
/// ```
pub fn access(name: impl Into<Access>) -> AccessInit {
    AccessInit {
        name: name.into(),
        level: None,
    }
}

/// An `ACCESS` statement awaiting its operation.
#[must_use]
pub struct AccessInit {
    name: Access,
    level: Option<Level>,
}

impl AccessInit {
    /// `ON @level`: the level the access method is defined on. Without it, SurrealDB uses the
    /// session's database, namespace or root, the most specific one selected.
    pub fn on(mut self, level: Level) -> Self {
        self.level = Some(level);
        self
    }

    fn operation<O>(self, operation: AccessOperation) -> AccessStatement<O> {
        AccessStatement {
            name: self.name,
            level: self.level,
            operation,
            output: PhantomData,
        }
    }

    /// `GRANT FOR USER @name`: a new bearer key for a system user.
    pub fn grant_for_user(self, user: impl Into<User>) -> AccessStatement<AccessGrant> {
        self.operation(AccessOperation::GrantUser(user.into()))
    }

    /// `GRANT FOR RECORD @id`: a new bearer key for a record user.
    pub fn grant_for_record(
        self,
        record: impl Into<types::RecordId>,
    ) -> AccessStatement<AccessGrant> {
        self.operation(AccessOperation::GrantRecord(record.into()))
    }

    /// `SHOW ALL`: every grant of the method.
    pub fn show_all(self) -> AccessStatement<Vec<AccessGrant>> {
        self.operation(AccessOperation::Show(Selection::All))
    }

    /// `SHOW GRANT @id`: one grant.
    pub fn show_grant(self, id: impl Into<String>) -> AccessStatement<AccessGrant> {
        self.operation(AccessOperation::Show(Selection::Grant(id.into())))
    }

    /// `SHOW WHERE @condition`: the grants the condition holds for.
    pub fn show_where(self, condition: impl Conditional) -> AccessStatement<Vec<AccessGrant>> {
        self.operation(AccessOperation::Show(Selection::Where(Filter::new(
            condition,
        ))))
    }

    /// `REVOKE ALL`: revokes every grant; a revoked key no longer signs in.
    pub fn revoke_all(self) -> AccessStatement<Vec<AccessGrant>> {
        self.operation(AccessOperation::Revoke(Selection::All))
    }

    /// `REVOKE GRANT @id`: revokes one grant, and gives the revoked grants as a list, like the
    /// other revocations. `SHOW GRANT` gives the grant itself.
    pub fn revoke_grant(self, id: impl Into<String>) -> AccessStatement<Vec<AccessGrant>> {
        self.operation(AccessOperation::Revoke(Selection::Grant(id.into())))
    }

    /// `REVOKE WHERE @condition`: revokes the grants the condition holds for.
    pub fn revoke_where(self, condition: impl Conditional) -> AccessStatement<Vec<AccessGrant>> {
        self.operation(AccessOperation::Revoke(Selection::Where(Filter::new(
            condition,
        ))))
    }

    /// `PURGE EXPIRED`: deletes expired grants. Combine with
    /// [`purge_revoked`](AccessStatement::purge_revoked).
    pub fn purge_expired(self) -> AccessStatement<Vec<AccessGrant>> {
        self.operation(AccessOperation::Purge(Purge {
            expired: true,
            revoked: false,
            grace: None,
        }))
    }

    /// `PURGE REVOKED`: deletes revoked grants. Combine with
    /// [`purge_expired`](AccessStatement::purge_expired).
    pub fn purge_revoked(self) -> AccessStatement<Vec<AccessGrant>> {
        self.operation(AccessOperation::Purge(Purge {
            expired: false,
            revoked: true,
            grace: None,
        }))
    }
}

enum AccessOperation {
    GrantUser(User),
    GrantRecord(types::RecordId),
    Show(Selection),
    Revoke(Selection),
    Purge(Purge),
}

enum Selection {
    All,
    Grant(String),
    Where(Filter),
}

struct Purge {
    expired: bool,
    revoked: bool,
    grace: Option<Duration>,
}

/// An `ACCESS` statement whose result decodes into `O`.
#[must_use]
pub struct AccessStatement<O> {
    name: Access,
    level: Option<Level>,
    operation: AccessOperation,
    output: PhantomData<fn() -> O>,
}

impl AccessStatement<Vec<AccessGrant>> {
    /// Adds `EXPIRED` to a `PURGE`.
    pub fn purge_expired(mut self) -> Self {
        if let AccessOperation::Purge(purge) = &mut self.operation {
            purge.expired = true;
        }
        self
    }

    /// Adds `REVOKED` to a `PURGE`.
    pub fn purge_revoked(mut self) -> Self {
        if let AccessOperation::Purge(purge) = &mut self.operation {
            purge.revoked = true;
        }
        self
    }

    /// `FOR @duration` on a `PURGE`: keeps grants that expired or were revoked within the
    /// duration. SurrealDB takes a duration literal only.
    pub fn for_(mut self, grace: Duration) -> Self {
        if let AccessOperation::Purge(purge) = &mut self.operation {
            purge.grace = Some(grace);
        }
        self
    }
}

impl<O: SurrealValue + 'static> AccessStatement<O> {
    /// Runs the statement and decodes its result.
    ///
    /// # Errors
    ///
    /// The database error, such as [`Error::NotAllowed`] for a session that may not manage the
    /// method's grants, or [`Error::Conversion`] when the result does not have the shape
    /// SurrealDB 3.3.4 gives it.
    pub async fn execute(&self, db: &impl Executor) -> Result<O> {
        let mut results = execute(db, self.to_query()?).await?;
        let value = results.take::<Value>(0).map_err(Error::Conversion)?;
        let value = match &self.operation {
            AccessOperation::Revoke(_) => flatten_revocations(value),
            _ => value,
        };
        O::from_value(value).map_err(Error::Conversion)
    }
}

/// SurrealDB 3.3.4 answers `REVOKE GRANT` and `REVOKE WHERE` with a list of lists of grants
/// (`[[{…}]]`, as the statement's documentation shows), so the inner lists are joined into one.
/// A grant outside an inner list is kept, so a flat answer decodes too.
fn flatten_revocations(value: Value) -> Value {
    let Value::Array(outer) = value else {
        return value;
    };
    let mut grants = Vec::new();
    for item in outer {
        match item {
            Value::Array(inner) => grants.extend(inner),
            grant => grants.push(grant),
        }
    }
    grants.into_value()
}

fn render_selection(selection: &Selection, out: &mut Renderer) {
    match selection {
        Selection::All => {
            out.push_str(" ALL");
        }
        Selection::Grant(id) => {
            out.push_str(" GRANT ").push_ident(id);
        }
        Selection::Where(condition) => {
            out.push_str(" WHERE ").push(condition);
        }
    }
}

impl<O> Statement for AccessStatement<O> {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("ACCESS ").push(&self.name);
        if let Some(level) = &self.level {
            out.push_str(" ON ").push(level);
        }
        match &self.operation {
            AccessOperation::GrantUser(user) => {
                out.push_str(" GRANT FOR USER ").push(user);
            }
            AccessOperation::GrantRecord(record) => {
                // The parser takes a record id literal here, not a parameter.
                out.push_str(" GRANT FOR RECORD ")
                    .push_inlined(&ValueLike::value(record.clone()));
            }
            AccessOperation::Show(selection) => {
                out.push_str(" SHOW");
                render_selection(selection, out);
            }
            AccessOperation::Revoke(selection) => {
                out.push_str(" REVOKE");
                render_selection(selection, out);
            }
            AccessOperation::Purge(purge) => {
                match (purge.expired, purge.revoked) {
                    (true, true) => {
                        out.push_str(" PURGE EXPIRED, REVOKED");
                    }
                    (true, false) => {
                        out.push_str(" PURGE EXPIRED");
                    }
                    (false, true) => {
                        out.push_str(" PURGE REVOKED");
                    }
                    // invariant: `purge_expired` and `purge_revoked` each set one of the two.
                    (false, false) => {
                        out.push_error("ACCESS PURGE needs EXPIRED, REVOKED or both");
                    }
                }
                if let Some(grace) = &purge.grace {
                    out.push_str(" FOR ").push_display(grace);
                }
            }
        }
    }
}

impl_statement!([O: 'static] AccessStatement<O>);
