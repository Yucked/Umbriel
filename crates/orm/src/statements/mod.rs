use crate::{Render, Renderer};

/// A SurrealQL statement. Its body renders without the trailing `;`, so the same rendering
/// serves top-level statements and subqueries.
pub(crate) trait Statement {
    /// Writes the statement without its trailing `;`.
    fn render_body(&self, out: &mut Renderer);
}

pub(crate) mod any;
pub(crate) mod break_;
pub(crate) mod continue_;
pub(crate) mod create;
pub(crate) mod data;
pub(crate) mod define_access;
pub(crate) mod define_analyzer;
pub(crate) mod define_database;
pub(crate) mod define_event;
pub(crate) mod define_field;
pub(crate) mod define_function;
pub(crate) mod define_index;
pub(crate) mod define_namespace;
pub(crate) mod define_param;
pub(crate) mod define_table;
pub(crate) mod define_user;
pub(crate) mod delete;
pub(crate) mod for_loop;
pub(crate) mod for_permission;
pub(crate) mod ifelse;
pub(crate) mod info;
pub(crate) mod insert;
pub(crate) mod let_;
pub(crate) mod live_select;
pub(crate) mod relate;
pub(crate) mod remove_access;
pub(crate) mod remove_analyzer;
pub(crate) mod remove_database;
pub(crate) mod remove_event;
pub(crate) mod remove_field;
pub(crate) mod remove_function;
pub(crate) mod remove_index;
pub(crate) mod remove_namespace;
pub(crate) mod remove_param;
pub(crate) mod remove_table;
pub(crate) mod remove_user;
pub(crate) mod return_;
pub(crate) mod returning;
pub(crate) mod select;
pub(crate) mod show;
pub(crate) mod sleep;
pub(crate) mod subquery;
pub(crate) mod throw;
pub(crate) mod transaction;
pub(crate) mod update;
pub(crate) mod upsert;
pub(crate) mod use_;
pub(crate) mod utils_block;
pub(crate) mod utils_chain;

// Defined after the modules so they import these macros by path rather than inheriting them
// through textual scope.

/// Implements `Render` (the body followed by `;`) and `Queryable` for a [`Statement`].
// Separate arms: an optional `[..]` before a `ty` is ambiguous, since a type can start with `[`.
macro_rules! impl_statement {
    ([$($generics:tt)*] $statement:ty) => {
        impl<$($generics)*> $crate::Render for $statement {
            fn render(&self, out: &mut $crate::Renderer) {
                $crate::statements::Statement::render_body(self, out);
                out.push_str(";");
            }
        }

        impl<$($generics)*> $crate::Queryable for $statement {
            fn into_fragment(self) -> $crate::Fragment {
                $crate::Fragment::new(self)
            }
        }
    };
    ($statement:ty) => {
        impl $crate::Render for $statement {
            fn render(&self, out: &mut $crate::Renderer) {
                $crate::statements::Statement::render_body(self, out);
                out.push_str(";");
            }
        }

        impl $crate::Queryable for $statement {
            fn into_fragment(self) -> $crate::Fragment {
                $crate::Fragment::new(self)
            }
        }
    };
}
pub(crate) use impl_statement;

/// Implements `Render` and `Queryable` for a definition. SurrealDB stores a definition's
/// expressions verbatim and evaluates them later without the defining query's parameters, so
/// every value is written inline.
macro_rules! impl_definition {
    ($statement:ty) => {
        impl $crate::Render for $statement {
            fn render(&self, out: &mut $crate::Renderer) {
                out.push_inlined(&$crate::statements::StatementBody(self));
                out.push_str(";");
            }
        }

        impl $crate::Queryable for $statement {
            fn into_fragment(self) -> $crate::Fragment {
                $crate::Fragment::new(self)
            }
        }
    };
}
pub(crate) use impl_definition;

/// A statement's body as a node of its own.
pub(crate) struct StatementBody<'a, S: ?Sized>(pub(crate) &'a S);

impl<S: Statement + ?Sized> Render for StatementBody<'_, S> {
    fn render(&self, out: &mut Renderer) {
        self.0.render_body(out);
    }
}

pub use any::{
    AnyTarget, CreateAnyStatement, InsertAnyStatement, Models, create_any, create_any_only,
    insert_any,
};
pub use break_::{BreakStatement, break_};
pub use continue_::{ContinueStatement, continue_};
pub use create::{CreateStatement, CreateTarget, CreateTargets, create, create_only};
pub use data::Unsettables;
pub use define_access::{
    AccessExpression, DefineAccessStatement, JwtAccess, RecordAccess, define_access, jwt_access,
    record_access,
};
pub use define_analyzer::{
    AnalyzerFilter, DefineAnalyzerStatement, SnowballLanguage, Tokenizer, define_analyzer,
};
pub use define_database::{DefineDatabaseStatement, define_database};
pub use define_event::{DefineEventStatement, define_event};
pub use define_field::{DefineFieldStatement, define_field};
pub use define_function::{DefineFunctionStatement, FunctionArgument, define_function};
pub use define_index::{DefineIndexStatement, FullTextAnalyzer, define_index, fulltext_analyzer};
pub use define_namespace::{DefineNamespaceStatement, define_namespace};
pub use define_param::{DefineParamStatement, define_param};
pub use define_table::{DefineTableStatement, define_table};
pub use define_user::{DefineUserStatement, UserRole, define_user};
pub use delete::{DeleteStatement, delete, delete_edges, delete_only};
pub use for_loop::{ForLoopStatement, for_};
pub use for_permission::{ForPermission, Permissions, for_permission};
pub use ifelse::{IfElseStatement, if_};
pub use info::{InfoStatement, info_for};
pub use insert::{InsertStatement, Insertables, input_of, insert, insert_relation};
pub use let_::{LetStatement, let_};
pub use live_select::{LiveSelectStatement, live};
pub use relate::{RelateOrUpdateStatement, RelateStatement, relate, relate_only};
pub use remove_access::{RemoveAccessStatement, remove_access};
pub use remove_analyzer::{RemoveAnalyzerStatement, remove_analyzer};
pub use remove_database::{RemoveDatabaseStatement, remove_database};
pub use remove_event::{RemoveEventStatement, remove_event};
pub use remove_field::{RemoveFieldStatement, remove_field};
pub use remove_function::{RemoveFunctionStatement, remove_function};
pub use remove_index::{RemoveIndexStatement, remove_index};
pub use remove_namespace::{RemoveNamespaceStatement, remove_namespace};
pub use remove_param::{RemoveParamStatement, remove_param};
pub use remove_table::{RemoveTableStatement, remove_table};
pub use remove_user::{RemoveUserStatement, UserPermissionScope, remove_user};
pub use return_::{ReturnStatement, return_};
pub use returning::{
    Cardinality, DiffOp, Explained, Many, One, ReturnAfter, ReturnBefore, ReturnDefaultNone,
    ReturnDiff, ReturnFields, ReturnMode, ReturnNone, ReturnNull, ReturnValue, Returning,
};
pub use select::{
    Order, Orderables, SelectStatement, SelectStatementMini, Selectables, Splittables, order,
    select, select_value,
};
pub use show::{ChangesSince, ShowChangesStatement, show_changes_for_table};
pub use sleep::{SleepStatement, sleep};
pub use subquery::Subquery;
pub use throw::{ThrowStatement, throw};
pub use transaction::{QueryTransaction, TransactionCompletion, begin_transaction};
pub use update::{TargettablesForUpdate, UpdateStatement, update, update_only};
pub use upsert::{UpsertStatement, upsert, upsert_only};
pub use use_::{UseStatement, use_};

/// helpers for statements
pub mod utils {
    pub use super::utils_block::{Block, block, block_deprecated};
    pub use super::utils_chain::{Chainable, QueryChain, chain};
}

/// Where a definition applies.
pub enum NamespaceOrDatabase {
    /// `NAMESPACE`
    Namespace,
    /// `DATABASE`
    Database,
}

impl Render for NamespaceOrDatabase {
    fn render(&self, out: &mut Renderer) {
        out.push_str(match self {
            NamespaceOrDatabase::Namespace => "NAMESPACE",
            NamespaceOrDatabase::Database => "DATABASE",
        });
    }
}
