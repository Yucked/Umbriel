//! Crate-root items (`orm/src/lib.rs`): every name the root exports, and offline SurrealQL value
//! parsing behind the `parse` feature.

mod data_model;
mod errors;
mod execution;
mod functions;
mod kinds;
mod migrations;
mod query;
mod render;
mod statements;
mod traits;

use umbriel::types::Value;

/// #33 11.8: every name `.agent/33-exports.md` lists is reachable from the root by its own
/// path, so removing or renaming one is a compile error here.
#[expect(unused_imports, reason = "the imports are the test: each must resolve")]
mod compiles_root_exports {
    use umbriel::{
        Access, Alias, AliasName, Aliasable, All, AllGetter, AnyEdgeFilter, ArrayLike, Arrow,
        Block, BoolLike, Bracket, BytesLike, CanOrder, Cast, Chainable, Clause, ClauseType,
        Closure, ClosureLike, Conditional, Connection, Constant, CrudType, Database, DatetimeLike,
        DbResources, Destructure, DurationLike, E, Edge, EdgeClause, EdgeDirection, ElementField,
        Empty, Error, Event, Executor, Expression, Field, FieldChangeMeta, FieldMetadata,
        FieldTuple, FileLike, Filter, FlatMerge, FlatPatch, Fragment, Function,
        GENERATED_PARAM_PREFIX, GeometryLike, Graph, Ident, IdiomPart, Index, IndexName,
        IndexedResults, Interval, IntoNodeClause, JwtAlgorithm, Last, Level, LinkMany, LinkOne,
        LinkPath, LinkSelf, Links, Lookup, Match, Merge, MergeFields, MergeObject, MlVersion,
        MlVersionError, Model, ModelMerge, ModelPatch, Models, NONE, NULL, Name, Namespace, Node,
        NodeAliasClause, NodeClause, Nullable, NumberLike, Object, ObjectClause, ObjectLike,
        OnDatabase, OnLevel, OnNamespace, OnRoot, Operatable, Operation, Operator, Ordinal, Param,
        ParamAccess, ParamAction, ParamAfter, ParamAuth, ParamBefore, ParamEvent, ParamFile,
        ParamInput, ParamNameError, ParamParent, ParamReference, ParamRequest, ParamSession,
        ParamTarget, ParamThis, ParamToken, ParamValue, Patch, PatchOp, PatchOpConversionError,
        Patchable, Pickable, Projections, Query, QueryChain, Queryable, Raw,
        RecordAccessDefinition, RecordId, RecordIdLike, RecordKey, RecordRange, Recurse, Reference,
        Relate, Render, Renderer, Result, Retry, RowStream, Runnable, SchemaGetter, SetLike,
        Setter, SetterArg, SetterArray, SetterAssignable, SetterNumeric, ShortestTarget,
        StatementContext, Stats, StoredFields, StrandLike, StreamEvent, Surreal, SurrealCrud,
        SurrealCrudNode, SurrealRead, Table, TableIndex, TableLike, TableOf, TableResources,
        TableSpec, Tables, Transaction, User, ValueLike, WithStats, Writable, WritableModels,
        any_other_edges, block, bracket, cast, cast_dynamic, chain, closure, cond, count,
        define_function, engine, for_, functions, index, kinds, model, negate, not, opt, parse,
        pick, query, query_raw, statements, transaction, truthy, types, validators, where_,
    };
    use umbriel::{
        access, action, after, auth, before, event, file, input, parent, reference, request,
        session, target, this, token, value,
    };
    use umbriel::{
        arr, array, assert_fields, assert_not, check_unique_idents, create_param_name_fn,
        create_table_resources, function, object, object_partial, search_highlight, search_offsets,
        search_score,
    };
}

#[test]
fn offline_parse() {
    let Value::Object(object) = umbriel::parse::value(
        "{ name: 'Alice', tags: ['admin', 'staff'], owner: user:alice, joined: d'2024-01-01T00:00:00Z' }",
    )
    .expect("parses") else {
        panic!("an object literal parses as an object");
    };
    assert_eq!(object.get("name"), Some(&Value::String("Alice".to_owned())));
    assert!(matches!(object.get("tags"), Some(Value::Array(_))));
    assert!(matches!(object.get("owner"), Some(Value::RecordId(_))));
    assert!(matches!(object.get("joined"), Some(Value::Datetime(_))));

    assert!(
        umbriel::parse::value("{ name: ").is_err(),
        "incomplete text is refused"
    );
}
