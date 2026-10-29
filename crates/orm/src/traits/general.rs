use crate::{Field, Fragment, Render, ValueLike};

/// Denoted by `.*`. Used for accessing all nested fields and arrays and links
pub trait AllGetter {
    /// Appends `.*` to the current path. Get all nested fields and arrays and links
    fn all(&self) -> Field;
}

impl<B: Render + Clone + Send + Sync + 'static> AllGetter for B {
    fn all(&self) -> Field {
        Field::from_expr(self.clone()).all()
    }
}

/// Used for statements
pub trait Queryable: Render {
    /// The statement as a node another query can hold: a chain, block or transaction.
    #[doc(hidden)]
    fn into_fragment(self) -> Fragment
    where
        Self: Sized;
}

impl<T: Queryable + Clone> Queryable for &T {
    fn into_fragment(self) -> Fragment {
        self.clone().into_fragment()
    }
}

/// Used for filters. A condition is the node's expression form, so a `LET` statement stands for
/// its parameter and a statement for its subquery.
///
/// SurrealDB tests a condition for truthiness, not for being `true`: `NONE`, `NULL`, `false`,
/// `0`, an empty string, array or object are false, and every other value is true. So
/// `where_(field)` keeps the records whose field is set and non-empty. Comparisons across types
/// follow SurrealDB's ordering of value kinds rather than failing.
pub trait Conditional: Render + Into<ValueLike> {
    /// Whether the condition is already a single parenthesized group.
    fn is_grouped(&self) -> bool {
        false
    }
}

/// Used for marking a struct used in UPDATE MERGE statement.
pub trait DataUpdater {}
