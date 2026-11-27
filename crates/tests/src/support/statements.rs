//! What the statement test files share: the names they import and their fixtures. They import
//! it with `use umbriel_tests::support::statements::*;`, so each file uses only what it needs.

pub use std::{collections::BTreeMap, time::Duration};

use crate::*;
pub use chrono::{DateTime, Utc};
pub use surrealdb::{
    Surreal,
    engine::any::Any,
    opt::auth::Record,
    types::{Kind, SurrealValue, Value},
};
pub use umbriel::{
    AliasName, Aliasable, All,
    CrudType::*,
    E, Error, Field, JwtAlgorithm, Level, LinkOne, Merge, Model, NONE, Namespace, Node, OnDatabase,
    Operatable, Param, Patchable, Raw, Render, Runnable, SchemaGetter, SetterArray,
    SetterAssignable, SetterNumeric, StatementContext, SurrealCrud, SurrealCrudNode, Table,
    ValueLike, arr, block, chain, cond, count,
    functions::{math, search, string},
    statements::{
        AnalyzerFilter, CreateTarget, DiffOp, Expiry, Returning, SnowballLanguage, Tokenizer,
        UserRole, access, alter_user, bearer_access, begin_transaction, break_, continue_, create,
        create_only, define_access, define_analyzer, define_database, define_event, define_field,
        define_function, define_index, define_namespace, define_param, define_table, define_user,
        delete, delete_edges, delete_only, for_, for_permission, fulltext_analyzer, if_, info_for,
        input_of, insert, insert_relation, jwt_access, let_, live, order, record_access, relate,
        relate_only, remove_access, remove_analyzer, remove_database, remove_event, remove_field,
        remove_function, remove_index, remove_namespace, remove_param, remove_table, remove_user,
        return_, select, select_value, show_changes_for_table, sleep, throw, update, update_only,
        use_,
    },
    value,
};

pub fn space_ship(key: &str, name: &str) -> SpaceShip {
    SpaceShip {
        id: SpaceShip::id(key),
        name: name.to_owned(),
        created: DateTime::<Utc>::UNIX_EPOCH,
    }
}

pub fn organization(key: &str, name: &str, age: u8) -> Organization {
    Organization {
        id: Organization::id(key),
        name: name.to_owned(),
        age,
        ..Default::default()
    }
}

pub async fn run(db: &Surreal<Any>, statement: impl Runnable + Sync) {
    statement.run(db).await.expect("statement succeeds");
}

pub async fn ships(db: &Surreal<Any>) -> Vec<String> {
    read(db, "SELECT VALUE name FROM space_ship ORDER BY name;").await
}
