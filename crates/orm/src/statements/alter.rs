//! `ALTER` statements: changes to part of an existing definition, without redefining it.
//!
//! Each builder sets or drops the clauses SurrealDB 3.3.4 lets that kind change, and takes
//! `IF EXISTS`, which makes altering a missing resource a no-op. Setting and dropping the same
//! clause fails when the query is rendered. Like definitions, alterations are rendered with their
//! values inline, since SurrealDB stores the expressions they set.
//!
//! `ALTER NAMESPACE` and `ALTER DATABASE` take only `COMPACT`, which is server maintenance, so
//! they have no builder.

use std::ops::Deref;

use surrealdb::types::{Duration, Kind, Table, Value};

use crate::{
    Block, Conditional, Event, Field, Filter, Ident, Name, Param, Renderer, TableIndex, ValueLike,
    render::push_function_path,
};

use super::{
    Statement, Subquery,
    define_analyzer::{AnalyzerFilter, Tokenizer, push_list},
    define_api::{ApiAction, ApiConfig, ApiMethod, push_api_path, push_methods},
    define_event::EventMode,
    define_field::{FieldDefault, kind_contains_object},
    define_function::{FunctionArgument, push_arguments},
    define_module::ModuleName,
    define_table::{TableKind, TableRelation, push_changefeed},
    for_permission::{PermissionValue, PermissionsClause, impl_permissions},
    impl_definition,
};

/// A clause an `ALTER` sets, drops, or leaves as it is.
#[derive(Debug, Clone)]
struct Change<T> {
    set: Option<T>,
    drop: bool,
}

impl<T> Default for Change<T> {
    fn default() -> Self {
        Self {
            set: None,
            drop: false,
        }
    }
}

impl<T> Change<T> {
    /// Writes ` DROP <keyword>`, or the set value through `render_set`. A clause both set and
    /// dropped is an error.
    fn render(
        &self,
        out: &mut Renderer,
        keyword: &str,
        render_set: impl FnOnce(&T, &mut Renderer),
    ) {
        match (&self.set, self.drop) {
            (Some(_), true) => {
                out.push_error(format!("ALTER cannot both set and drop {keyword}"));
            }
            (Some(value), false) => render_set(value, out),
            (None, true) => {
                out.push_str(" DROP ").push_str(keyword);
            }
            (None, false) => {}
        }
    }
}

/// Writes ` COMMENT '…'` or ` DROP COMMENT`. `ALTER` takes the comment as a string literal.
fn render_comment(comment: &Change<String>, out: &mut Renderer) {
    comment.render(out, "COMMENT", |comment, out| {
        out.push_str(" COMMENT ")
            .push_value(&Value::String(comment.clone()));
    });
}

/// Writes `ALTER <KIND> [IF EXISTS] `, ready for the name.
fn push_head(out: &mut Renderer, kind: &str, if_exists: bool) {
    out.push_str("ALTER ").push_str(kind);
    if if_exists {
        out.push_str(" IF EXISTS");
    }
    out.push_str(" ");
}

/// Writes ` ON TABLE @table` when a table is set.
fn push_table(out: &mut Renderer, table: Option<&Name<Table>>) {
    if let Some(table) = table {
        out.push_str(" ON TABLE ").push(table);
    }
}

/// Adds `if_exists`, `comment` and `drop_comment`, which every `ALTER` takes.
macro_rules! impl_alter_common {
    ($statement:ty) => {
        impl $statement {
            /// `IF EXISTS`: altering a resource that does not exist succeeds instead of failing.
            pub fn if_exists(mut self) -> Self {
                self.if_exists = true;
                self
            }

            /// `COMMENT`: replaces the definition's comment.
            pub fn comment(mut self, comment: impl Into<String>) -> Self {
                self.comment.set = Some(comment.into());
                self
            }

            /// `DROP COMMENT`: removes the definition's comment.
            pub fn drop_comment(mut self) -> Self {
                self.comment.drop = true;
                self
            }
        }
    };
}

/// Alters a table: `ALTER TABLE [IF EXISTS] @name …`.
///
/// It changes the comment, the kind, `SCHEMAFULL`/`SCHEMALESS`, the changefeed, the inline
/// caches and the permissions; the rest of the definition is kept.
///
/// ```rust
/// use umbriel::{*, statements::alter_table};
///
/// let statement = alter_table("user").schemafull().comment("Acme Corp users");
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER TABLE user SCHEMAFULL COMMENT 'Acme Corp users';"
/// );
///
/// let statement = alter_table("user").if_exists().kind_normal().drop_comment();
/// assert_eq!(statement.to_debug_sql(), "ALTER TABLE IF EXISTS user TYPE NORMAL DROP COMMENT;");
/// ```
pub fn alter_table(table: impl Into<Name<Table>>) -> AlterTableStatement {
    AlterTableStatement {
        table: table.into(),
        if_exists: false,
        kind: None,
        schemafull: None,
        changefeed: Change::default(),
        include_original: false,
        comment: Change::default(),
        inline_edges: Change::default(),
        inline_references: Change::default(),
        permissions: PermissionsClause::default(),
    }
}

/// An `ALTER TABLE` statement.
pub struct AlterTableStatement {
    table: Name<Table>,
    if_exists: bool,
    kind: Option<TableKind>,
    schemafull: Option<bool>,
    changefeed: Change<Duration>,
    include_original: bool,
    comment: Change<String>,
    inline_edges: Change<u32>,
    inline_references: Change<u32>,
    permissions: PermissionsClause,
}

impl AlterTableStatement {
    /// `INLINE EDGES @cap`: each record caches up to `cap` of its edges. SurrealDB rejects a cap
    /// above its maximum.
    pub fn inline_edges(mut self, cap: u32) -> Self {
        self.inline_edges.set = Some(cap);
        self
    }

    /// `DROP INLINE EDGES`.
    pub fn drop_inline_edges(mut self) -> Self {
        self.inline_edges.drop = true;
        self
    }

    /// `INLINE REFERENCES @cap`: each record caches up to `cap` of the references to it.
    /// SurrealDB rejects a cap above its maximum.
    pub fn inline_references(mut self, cap: u32) -> Self {
        self.inline_references.set = Some(cap);
        self
    }

    /// `DROP INLINE REFERENCES`.
    pub fn drop_inline_references(mut self) -> Self {
        self.inline_references.drop = true;
        self
    }

    /// `CHANGEFEED @expiry`: replaces the table's changefeed. SurrealDB takes the duration only
    /// as a literal.
    pub fn changefeed(mut self, expiry: impl Into<Duration>) -> Self {
        self.changefeed.set = Some(expiry.into());
        self
    }

    /// `INCLUDE ORIGINAL`, written with [`changefeed`](Self::changefeed): updates also record the
    /// patch that reverts them, and deletes the deleted record. Without a changefeed, the query
    /// fails when it is rendered.
    pub fn include_original(mut self) -> Self {
        self.include_original = true;
        self
    }

    /// `DROP CHANGEFEED`: the table stops recording its changes.
    pub fn drop_changefeed(mut self) -> Self {
        self.changefeed.drop = true;
        self
    }

    /// `TYPE ANY`.
    pub fn kind_any(mut self) -> Self {
        self.kind = Some(TableKind::Any);
        self
    }

    /// `TYPE NORMAL`.
    pub fn kind_normal(mut self) -> Self {
        self.kind = Some(TableKind::Normal);
        self
    }

    /// `TYPE RELATION`, with the endpoints of `relation`.
    pub fn kind_relation(mut self, relation: TableRelation) -> Self {
        self.kind = Some(TableKind::Relation(relation));
        self
    }

    /// `SCHEMAFULL`.
    pub fn schemafull(mut self) -> Self {
        self.schemafull = Some(true);
        self
    }

    /// `SCHEMALESS`.
    pub fn schemaless(mut self) -> Self {
        self.schemafull = Some(false);
        self
    }
}

impl_permissions!(per_for: AlterTableStatement, AlterFieldStatement);
impl_permissions!(
    single: AlterParamStatement,
    AlterFunctionStatement,
    AlterBucketStatement,
    AlterModuleStatement,
);

impl Statement for AlterTableStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "TABLE", self.if_exists);
        out.push(&self.table);
        if let Some(kind) = &self.kind {
            kind.render(out);
        }
        match self.schemafull {
            Some(true) => {
                out.push_str(" SCHEMAFULL");
            }
            Some(false) => {
                out.push_str(" SCHEMALESS");
            }
            None => {}
        }
        if self.include_original && self.changefeed.set.is_none() {
            out.push_error("INCLUDE ORIGINAL needs a CHANGEFEED");
        }
        self.changefeed.render(out, "CHANGEFEED", |expiry, out| {
            push_changefeed(out, Some(expiry), self.include_original);
        });
        render_comment(&self.comment, out);
        self.inline_edges.render(out, "INLINE EDGES", |cap, out| {
            out.push_str(" INLINE EDGES ").push_display(cap);
        });
        self.inline_references
            .render(out, "INLINE REFERENCES", |cap, out| {
                out.push_str(" INLINE REFERENCES ").push_display(cap);
            });
        out.push(&self.permissions);
    }
}

impl_alter_common!(AlterTableStatement);
impl_definition!(AlterTableStatement);

/// Alters a field: `ALTER FIELD [IF EXISTS] @name ON [TABLE] @table …`.
///
/// Each clause can be set or dropped; the others are kept. `READONLY` and `FLEXIBLE` are set
/// with [`readonly`](AlterFieldStatement::readonly) and
/// [`flexible`](AlterFieldStatement::flexible) and removed with their `drop_` methods.
///
/// ```rust
/// use umbriel::{*, statements::alter_field, types::Kind};
///
/// let statement = alter_field(Field::new("email"))
///     .on_table("user")
///     .type_(Kind::String)
///     .readonly()
///     .drop_default();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER FIELD email ON TABLE user TYPE string READONLY DROP DEFAULT;"
/// );
///
/// // A clause cannot be both set and dropped.
/// let invalid = alter_field(Field::new("email")).on_table("user").readonly().drop_readonly();
/// assert!(invalid.to_query().is_err());
/// ```
pub fn alter_field(field: impl Into<Field>) -> AlterFieldStatement {
    AlterFieldStatement {
        field: field.into(),
        table: None,
        if_exists: false,
        type_: Change::default(),
        flexible: Change::default(),
        readonly: Change::default(),
        value: Change::default(),
        assert: Change::default(),
        default: Change::default(),
        comment: Change::default(),
        inline: Change::default(),
        permissions: PermissionsClause::default(),
    }
}

/// An `ALTER FIELD` statement.
pub struct AlterFieldStatement {
    field: Field,
    table: Option<Name<Table>>,
    if_exists: bool,
    type_: Change<Kind>,
    flexible: Change<()>,
    readonly: Change<()>,
    inline: Change<()>,
    value: Change<ValueLike>,
    assert: Change<Filter>,
    default: Change<FieldDefault>,
    comment: Change<String>,
    permissions: PermissionsClause,
}

impl AlterFieldStatement {
    /// The table the field belongs to. The name may come from a parameter.
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// `TYPE @type`.
    pub fn type_(mut self, field_type: impl Into<Kind>) -> Self {
        self.type_.set = Some(field_type.into());
        self
    }

    /// `DROP TYPE`: the field takes any type.
    pub fn drop_type(mut self) -> Self {
        self.type_.drop = true;
        self
    }

    /// `FLEXIBLE`. With [`type_`](Self::type_), the type must admit an object.
    pub fn flexible(mut self) -> Self {
        self.flexible.set = Some(());
        self
    }

    /// `DROP FLEXIBLE`.
    pub fn drop_flexible(mut self) -> Self {
        self.flexible.drop = true;
        self
    }

    /// `READONLY`.
    pub fn readonly(mut self) -> Self {
        self.readonly.set = Some(());
        self
    }

    /// `DROP READONLY`.
    pub fn drop_readonly(mut self) -> Self {
        self.readonly.drop = true;
        self
    }

    /// `INLINE`: the field's value is stored with the edge's adjacency, so traversals read it
    /// without loading the edge. Only on top-level fields of `TYPE RELATION` tables, other than
    /// `id`, `in`, `out` and computed fields.
    pub fn inline(mut self) -> Self {
        self.inline.set = Some(());
        self
    }

    /// `DROP INLINE`.
    pub fn drop_inline(mut self) -> Self {
        self.inline.drop = true;
        self
    }

    /// `VALUE @expression`.
    pub fn value(mut self, value: impl Into<ValueLike>) -> Self {
        self.value.set = Some(value.into());
        self
    }

    /// `DROP VALUE`.
    pub fn drop_value(mut self) -> Self {
        self.value.drop = true;
        self
    }

    /// `ASSERT @condition`. Each call adds a condition, joined to the earlier ones with `AND`.
    pub fn assert(mut self, assertion: impl Conditional) -> Self {
        self.assert.set = Some(match self.assert.set.take() {
            Some(earlier) => earlier.and(assertion),
            None => Filter::new(assertion),
        });
        self
    }

    /// `DROP ASSERT`.
    pub fn drop_assert(mut self) -> Self {
        self.assert.drop = true;
        self
    }

    /// `DEFAULT @expression`. Replaces an earlier [`default_always`](Self::default_always).
    pub fn default(mut self, default: impl Into<ValueLike>) -> Self {
        self.default.set = Some(FieldDefault::Set(default.into()));
        self
    }

    /// `DEFAULT ALWAYS @expression`. Replaces an earlier [`default`](Self::default).
    pub fn default_always(mut self, default: impl Into<ValueLike>) -> Self {
        self.default.set = Some(FieldDefault::Always(default.into()));
        self
    }

    /// `DROP DEFAULT`.
    pub fn drop_default(mut self) -> Self {
        self.default.drop = true;
        self
    }
}

impl Statement for AlterFieldStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "FIELD", self.if_exists);
        out.push(&self.field);
        push_table(out, self.table.as_ref());
        self.type_.render(out, "TYPE", |kind, out| {
            out.push_str(" TYPE ").push(kind);
        });
        if self.flexible.set.is_some()
            && let Some(kind) = &self.type_.set
            && !kind_contains_object(kind)
        {
            out.push_error("FLEXIBLE needs a TYPE that admits an object");
        }
        self.flexible.render(out, "FLEXIBLE", |(), out| {
            out.push_str(" FLEXIBLE");
        });
        self.readonly.render(out, "READONLY", |(), out| {
            out.push_str(" READONLY");
        });
        self.inline.render(out, "INLINE", |(), out| {
            out.push_str(" INLINE");
        });
        self.value.render(out, "VALUE", |value, out| {
            out.push_str(" VALUE ").push(value);
        });
        self.assert.render(out, "ASSERT", |assertion, out| {
            out.push_str(" ASSERT ").push(assertion);
        });
        self.default.render(out, "DEFAULT", FieldDefault::render);
        render_comment(&self.comment, out);
        self.permissions.render_for_field(out);
    }
}

impl_alter_common!(AlterFieldStatement);
impl_definition!(AlterFieldStatement);

/// Alters an index: `ALTER INDEX [IF EXISTS] @name ON TABLE @table [PREPARE REMOVE]
/// [COMMENT …]`.
///
/// Unlike `DEFINE INDEX OVERWRITE`, which rebuilds the index, it keeps the index's data. It
/// changes only the comment and [`prepare_remove`](AlterIndexStatement::prepare_remove); the
/// table is required, and a statement without one fails to render.
///
/// ```rust
/// use umbriel::{*, statements::alter_index};
///
/// let statement = alter_index("post_content").on_table("post").comment("Search");
/// assert_eq!(statement.to_debug_sql(), "ALTER INDEX post_content ON TABLE post COMMENT 'Search';");
///
/// let statement = alter_index("post_content").on_table("post").if_exists().prepare_remove();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER INDEX IF EXISTS post_content ON TABLE post PREPARE REMOVE;"
/// );
/// ```
pub fn alter_index(index: impl Into<Name<TableIndex>>) -> AlterIndexStatement {
    AlterIndexStatement {
        index: index.into(),
        table: None,
        if_exists: false,
        prepare_remove: false,
        comment: Change::default(),
    }
}

/// An `ALTER INDEX` statement.
pub struct AlterIndexStatement {
    index: Name<TableIndex>,
    table: Option<Name<Table>>,
    if_exists: bool,
    prepare_remove: bool,
    comment: Change<String>,
}

impl AlterIndexStatement {
    /// The table the index is on. The name may come from a parameter.
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// `PREPARE REMOVE`: takes the index out of service ahead of its removal. Writes stop
    /// maintaining it, queries stop using it, and a build in progress is cancelled. Nothing but
    /// a redefinition (`DEFINE INDEX OVERWRITE`, which rebuilds it) or `REMOVE INDEX` undoes it,
    /// and `REBUILD INDEX` fails on it.
    pub fn prepare_remove(mut self) -> Self {
        self.prepare_remove = true;
        self
    }
}

impl Statement for AlterIndexStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "INDEX", self.if_exists);
        out.push(&self.index);
        if self.table.is_none() {
            out.push_error("ALTER INDEX needs the index's table: call `on_table`");
        }
        push_table(out, self.table.as_ref());
        if self.prepare_remove {
            out.push_str(" PREPARE REMOVE");
        }
        render_comment(&self.comment, out);
    }
}

impl_alter_common!(AlterIndexStatement);
impl_definition!(AlterIndexStatement);

/// Alters an event: `ALTER EVENT [IF EXISTS] @name ON [TABLE] @table …`.
///
/// `WHEN` and `THEN` are replaced; [`async_`](AlterEventStatement::async_) with `retry` and
/// `max_depth` makes it asynchronous, and [`drop_async`](AlterEventStatement::drop_async) makes
/// it synchronous again.
///
/// There is no `drop_when` or `drop_then`: SurrealDB 3.3.4 parses `DROP WHEN` and `DROP THEN` but
/// leaves the event unchanged (`alter/event.rs`). To run on every change, set a condition every
/// change meets, such as `$event IS NOT NONE`.
///
/// ```rust
/// use umbriel::{*, statements::alter_event};
///
/// let statement = alter_event("audit")
///     .on_table("user")
///     .async_()
///     .retry(3)
///     .when(Param::new("event").is_not(NONE));
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER EVENT audit ON TABLE user WHEN $event IS NOT NONE ASYNC RETRY 3;"
/// );
/// ```
pub fn alter_event(event: impl Into<Name<Event>>) -> AlterEventStatement {
    AlterEventStatement {
        event: event.into(),
        table: None,
        if_exists: false,
        when: None,
        then: Vec::new(),
        mode: Change::default(),
        comment: Change::default(),
    }
}

/// An `ALTER EVENT` statement.
pub struct AlterEventStatement {
    event: Name<Event>,
    table: Option<Name<Table>>,
    if_exists: bool,
    when: Option<Filter>,
    then: Vec<Subquery>,
    mode: Change<EventMode>,
    comment: Change<String>,
}

impl AlterEventStatement {
    /// The table the event belongs to. The name may come from a parameter.
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// `WHEN @condition`: replaces when the event runs.
    pub fn when(mut self, condition: impl Conditional) -> Self {
        self.when = Some(Filter::new(condition));
        self
    }

    /// `THEN …`: replaces what the event runs. Each call adds a statement after the earlier ones.
    pub fn then(mut self, query: impl Into<Subquery>) -> Self {
        self.then.push(query.into());
        self
    }

    /// `ASYNC`: the event runs after the triggering transaction commits.
    pub fn async_(mut self) -> Self {
        self.mode.set.get_or_insert_default().asynchronous = true;
        self
    }

    /// `RETRY n` for an `ASYNC` event. Without [`async_`](Self::async_) the query fails when it
    /// is rendered.
    pub fn retry(mut self, retries: u16) -> Self {
        self.mode.set.get_or_insert_default().retry = Some(retries);
        self
    }

    /// `MAXDEPTH n` for an `ASYNC` event. Without [`async_`](Self::async_) the query fails when
    /// it is rendered.
    pub fn max_depth(mut self, depth: u16) -> Self {
        self.mode.set.get_or_insert_default().max_depth = Some(depth);
        self
    }

    /// `DROP ASYNC`: the event runs inside the triggering transaction again.
    pub fn drop_async(mut self) -> Self {
        self.mode.drop = true;
        self
    }
}

impl Statement for AlterEventStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "EVENT", self.if_exists);
        out.push(&self.event);
        push_table(out, self.table.as_ref());
        if let Some(when) = &self.when {
            out.push_str(" WHEN ").push(when);
        }
        if !self.then.is_empty() {
            out.push_str(" THEN ").push_joined(&self.then, ", ");
        }
        self.mode.render(out, "ASYNC", |mode, out| mode.render(out));
        render_comment(&self.comment, out);
    }
}

impl_alter_common!(AlterEventStatement);
impl_definition!(AlterEventStatement);

/// Alters a param: `ALTER PARAM [IF EXISTS] $name …`. The name is a literal.
///
/// SurrealDB has no `DROP VALUE` for params; a param without a value is removed instead.
///
/// ```rust
/// use umbriel::{*, statements::alter_param};
///
/// let statement = alter_param(&Param::new("greeting")).value("Hello").permissions_none();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER PARAM $greeting VALUE 'Hello' PERMISSIONS NONE;"
/// );
/// ```
pub fn alter_param(param: impl Deref<Target = Param>) -> AlterParamStatement {
    AlterParamStatement {
        name: param.deref().clone(),
        if_exists: false,
        value: None,
        comment: Change::default(),
        permissions: None,
    }
}

/// An `ALTER PARAM` statement.
pub struct AlterParamStatement {
    name: Param,
    if_exists: bool,
    value: Option<ValueLike>,
    comment: Change<String>,
    permissions: Option<PermissionValue>,
}

impl AlterParamStatement {
    /// `VALUE @value`.
    pub fn value(mut self, value: impl Into<ValueLike>) -> Self {
        self.value = Some(value.into());
        self
    }
}

impl Statement for AlterParamStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "PARAM", self.if_exists);
        out.push(&self.name);
        if let Some(value) = &self.value {
            out.push_str(" VALUE ").push(value);
        }
        render_comment(&self.comment, out);
        if let Some(permissions) = &self.permissions {
            out.push(permissions);
        }
    }
}

impl_alter_common!(AlterParamStatement);
impl_definition!(AlterParamStatement);

/// Alters a function: `ALTER FUNCTION [IF EXISTS] fn::name …`. The name is written after `fn::`;
/// each `::`-separated part must be an identifier (`[A-Za-z_][A-Za-z0-9_]*`), or the query fails
/// when it is rendered.
///
/// [`signature`](AlterFunctionStatement::signature) replaces the arguments and body together, as
/// SurrealDB takes them, optionally with a return type.
///
/// ```rust
/// use umbriel::{*, statements::{FunctionArgument, alter_function, return_}, types::Kind};
///
/// let x = Param::new("x");
/// let statement = alter_function("add_one")
///     .signature(
///         vec![FunctionArgument { name: x.clone(), type_: Kind::Int }],
///         chain(return_(Field::from(&x).add(1))),
///     )
///     .returns(Kind::Int);
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER FUNCTION fn::add_one($x: int) -> int {\nRETURN $x + 1;\n};"
/// );
/// ```
pub fn alter_function(name: impl Into<String>) -> AlterFunctionStatement {
    AlterFunctionStatement {
        name: name.into(),
        if_exists: false,
        signature: None,
        returns: None,
        comment: Change::default(),
        permissions: None,
    }
}

/// An `ALTER FUNCTION` statement.
pub struct AlterFunctionStatement {
    name: String,
    if_exists: bool,
    signature: Option<(Vec<FunctionArgument>, Block)>,
    returns: Option<Kind>,
    comment: Change<String>,
    permissions: Option<PermissionValue>,
}

impl AlterFunctionStatement {
    /// Replaces the function's arguments and body.
    pub fn signature(mut self, arguments: Vec<FunctionArgument>, body: impl Into<Block>) -> Self {
        self.signature = Some((arguments, body.into()));
        self
    }

    /// `-> @type`: the return type, written with the [`signature`](Self::signature). Without a
    /// signature the query fails when it is rendered.
    pub fn returns(mut self, kind: impl Into<Kind>) -> Self {
        self.returns = Some(kind.into());
        self
    }
}

impl Statement for AlterFunctionStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "FUNCTION", self.if_exists);
        push_function_path(out, &self.name);
        match (&self.signature, &self.returns) {
            (Some((arguments, body)), returns) => {
                push_arguments(out, arguments);
                if let Some(kind) = returns {
                    out.push_str(" -> ").push(kind);
                }
                out.push_str(" ").push(body);
            }
            (None, Some(_)) => {
                out.push_error("ALTER FUNCTION writes a return type only with its signature");
            }
            (None, None) => {}
        }
        render_comment(&self.comment, out);
        if let Some(permissions) = &self.permissions {
            out.push(permissions);
        }
    }
}

impl_alter_common!(AlterFunctionStatement);
impl_definition!(AlterFunctionStatement);

/// Alters an analyzer: `ALTER ANALYZER [IF EXISTS] @name …`.
///
/// ```rust
/// use umbriel::{*, statements::{AnalyzerFilter, Tokenizer, alter_analyzer}};
///
/// let statement = alter_analyzer("simple")
///     .tokenizers([Tokenizer::Blank])
///     .filters([AnalyzerFilter::Lowercase])
///     .drop_function();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER ANALYZER simple DROP FUNCTION TOKENIZERS Blank FILTERS lowercase;"
/// );
/// ```
pub fn alter_analyzer(name: impl Into<Name<Ident>>) -> AlterAnalyzerStatement {
    AlterAnalyzerStatement {
        name: name.into(),
        if_exists: false,
        function: Change::default(),
        tokenizers: Change::default(),
        filters: Change::default(),
        comment: Change::default(),
    }
}

/// An `ALTER ANALYZER` statement.
pub struct AlterAnalyzerStatement {
    name: Name<Ident>,
    if_exists: bool,
    function: Change<String>,
    tokenizers: Change<Vec<Tokenizer>>,
    filters: Change<Vec<AnalyzerFilter>>,
    comment: Change<String>,
}

impl AlterAnalyzerStatement {
    /// `FUNCTION fn::name`: a function the analyzer passes text through first. The name is
    /// written after `fn::`.
    pub fn function(mut self, name: impl Into<String>) -> Self {
        self.function.set = Some(name.into());
        self
    }

    /// `DROP FUNCTION`.
    pub fn drop_function(mut self) -> Self {
        self.function.drop = true;
        self
    }

    /// `TOKENIZERS …`: replaces the tokenizers.
    pub fn tokenizers(mut self, tokenizers: impl IntoIterator<Item = Tokenizer>) -> Self {
        self.tokenizers.set = Some(tokenizers.into_iter().collect());
        self
    }

    /// `DROP TOKENIZERS`.
    pub fn drop_tokenizers(mut self) -> Self {
        self.tokenizers.drop = true;
        self
    }

    /// `FILTERS …`: replaces the filters.
    pub fn filters(mut self, filters: impl IntoIterator<Item = AnalyzerFilter>) -> Self {
        self.filters.set = Some(filters.into_iter().collect());
        self
    }

    /// `DROP FILTERS`.
    pub fn drop_filters(mut self) -> Self {
        self.filters.drop = true;
        self
    }
}

impl Statement for AlterAnalyzerStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "ANALYZER", self.if_exists);
        out.push(&self.name);
        self.function.render(out, "FUNCTION", |name, out| {
            out.push_str(" FUNCTION ");
            push_function_path(out, name);
        });
        self.tokenizers
            .render(out, "TOKENIZERS", |tokenizers, out| {
                out.push_str(" TOKENIZERS ");
                push_list(out, tokenizers);
            });
        self.filters.render(out, "FILTERS", |filters, out| {
            out.push_str(" FILTERS ");
            push_list(out, filters);
        });
        render_comment(&self.comment, out);
    }
}

impl_alter_common!(AlterAnalyzerStatement);
impl_definition!(AlterAnalyzerStatement);

/// Alters a sequence's timeout: `ALTER SEQUENCE [IF EXISTS] @name TIMEOUT @duration | NONE`.
///
/// It is the only change SurrealDB 3.3.4 makes to a sequence without redefining it, and it keeps
/// the values the sequence has issued. A sequence has no comment.
///
/// ```rust
/// use std::time::Duration;
/// use umbriel::{*, statements::alter_sequence};
///
/// assert_eq!(
///     alter_sequence("order_number").timeout(Duration::from_secs(5)).to_debug_sql(),
///     "ALTER SEQUENCE order_number TIMEOUT 5s;"
/// );
/// assert_eq!(
///     alter_sequence("order_number").if_exists().drop_timeout().to_debug_sql(),
///     "ALTER SEQUENCE IF EXISTS order_number TIMEOUT NONE;"
/// );
/// ```
pub fn alter_sequence(name: impl Into<Name<Ident>>) -> AlterSequenceStatement {
    AlterSequenceStatement {
        name: name.into(),
        if_exists: false,
        timeout: Change::default(),
    }
}

/// An `ALTER SEQUENCE` statement.
pub struct AlterSequenceStatement {
    name: Name<Ident>,
    if_exists: bool,
    timeout: Change<crate::DurationLike>,
}

impl AlterSequenceStatement {
    /// `IF EXISTS`: altering a sequence that does not exist succeeds instead of failing.
    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self
    }

    /// `TIMEOUT @duration`: how long `sequence::nextval` waits to reserve a batch. SurrealDB
    /// 3.3.4 parses it as an expression, like every other `TIMEOUT`.
    pub fn timeout(mut self, timeout: impl Into<crate::DurationLike>) -> Self {
        self.timeout.set = Some(timeout.into());
        self
    }

    /// `TIMEOUT NONE`: removes the timeout. SurrealDB has no `DROP TIMEOUT`.
    pub fn drop_timeout(mut self) -> Self {
        self.timeout.drop = true;
        self
    }
}

impl Statement for AlterSequenceStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "SEQUENCE", self.if_exists);
        out.push(&self.name);
        match (&self.timeout.set, self.timeout.drop) {
            (Some(_), true) => {
                out.push_error("ALTER cannot both set and drop TIMEOUT");
            }
            (Some(timeout), false) => {
                out.push_str(" TIMEOUT ").push(timeout);
            }
            (None, true) => {
                out.push_str(" TIMEOUT NONE");
            }
            (None, false) => {}
        }
    }
}

impl_definition!(AlterSequenceStatement);

/// Alters a bucket: `ALTER BUCKET [IF EXISTS] @name …`. Buckets need the experimental `files`
/// feature.
///
/// ```rust
/// use umbriel::{*, statements::alter_bucket};
///
/// let statement = alter_bucket("avatars").drop_readonly().backend("memory").permissions_none();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER BUCKET avatars DROP READONLY BACKEND 'memory' PERMISSIONS NONE;"
/// );
/// ```
pub fn alter_bucket(name: impl Into<Name<Ident>>) -> AlterBucketStatement {
    AlterBucketStatement {
        name: name.into(),
        if_exists: false,
        readonly: Change::default(),
        backend: Change::default(),
        permissions: None,
        comment: Change::default(),
    }
}

/// An `ALTER BUCKET` statement.
pub struct AlterBucketStatement {
    name: Name<Ident>,
    if_exists: bool,
    readonly: Change<()>,
    backend: Change<String>,
    permissions: Option<PermissionValue>,
    comment: Change<String>,
}

impl AlterBucketStatement {
    /// `READONLY`.
    pub fn readonly(mut self) -> Self {
        self.readonly.set = Some(());
        self
    }

    /// `DROP READONLY`.
    pub fn drop_readonly(mut self) -> Self {
        self.readonly.drop = true;
        self
    }

    /// `BACKEND @url`, a string literal.
    pub fn backend(mut self, url: impl Into<String>) -> Self {
        self.backend.set = Some(url.into());
        self
    }

    /// `DROP BACKEND`: the bucket uses the server's global bucket.
    pub fn drop_backend(mut self) -> Self {
        self.backend.drop = true;
        self
    }
}

impl Statement for AlterBucketStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "BUCKET", self.if_exists);
        out.push(&self.name);
        self.readonly.render(out, "READONLY", |(), out| {
            out.push_str(" READONLY");
        });
        self.backend.render(out, "BACKEND", |url, out| {
            out.push_str(" BACKEND ")
                .push_value(&Value::String(url.clone()));
        });
        if let Some(permissions) = &self.permissions {
            out.push(permissions);
        }
        render_comment(&self.comment, out);
    }
}

impl_alter_common!(AlterBucketStatement);
impl_definition!(AlterBucketStatement);

/// Alters an API: `ALTER API [IF EXISTS] @path …`.
///
/// [`for_any`](AlterApiStatement::for_any) replaces the middleware and permissions of every
/// request, [`fallback`](AlterApiStatement::fallback) and
/// [`drop_fallback`](AlterApiStatement::drop_fallback) replace or remove the answer to methods no
/// action names, [`action`](AlterApiStatement::action) adds or replaces the action of its methods,
/// and [`drop_action`](AlterApiStatement::drop_action) removes one. A method both set and dropped
/// is an error.
///
/// ```rust
/// use umbriel::{*, statements::{ApiMethod, alter_api, api_action}};
///
/// let statement = alter_api("/status")
///     .action(api_action([ApiMethod::Post], "ok"))
///     .drop_action([ApiMethod::Get])
///     .drop_fallback();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER API '/status' FOR ANY DROP THEN FOR POST THEN 'ok' FOR GET DROP THEN;"
/// );
/// ```
pub fn alter_api(path: impl Into<String>) -> AlterApiStatement {
    AlterApiStatement {
        path: path.into(),
        if_exists: false,
        any: None,
        fallback: Change::default(),
        actions: Vec::new(),
        dropped: Vec::new(),
        comment: Change::default(),
    }
}

/// An `ALTER API` statement.
pub struct AlterApiStatement {
    path: String,
    if_exists: bool,
    any: Option<ApiConfig>,
    fallback: Change<ValueLike>,
    actions: Vec<ApiAction>,
    dropped: Vec<Vec<ApiMethod>>,
    comment: Change<String>,
}

impl AlterApiStatement {
    /// `FOR ANY @config`: replaces the middleware and permissions of every request.
    pub fn for_any(mut self, config: ApiConfig) -> Self {
        self.any = Some(config);
        self
    }

    /// `FOR ANY THEN @fallback`: replaces the answer to methods no action names.
    pub fn fallback(mut self, fallback: impl Into<ValueLike>) -> Self {
        self.fallback.set = Some(fallback.into());
        self
    }

    /// `FOR ANY DROP THEN`: removes the fallback.
    pub fn drop_fallback(mut self) -> Self {
        self.fallback.drop = true;
        self
    }

    /// `FOR @methods … THEN @action`: sets the action of its methods.
    pub fn action(mut self, action: ApiAction) -> Self {
        self.actions.push(action);
        self
    }

    /// `FOR @methods DROP THEN`: removes the action of these methods.
    pub fn drop_action(mut self, methods: impl IntoIterator<Item = ApiMethod>) -> Self {
        self.dropped.push(methods.into_iter().collect());
        self
    }
}

impl Statement for AlterApiStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "API", self.if_exists);
        push_api_path(out, &self.path);
        let fallback_changes = self.fallback.set.is_some() || self.fallback.drop;
        if self.any.is_some() || fallback_changes {
            out.push_str(" FOR ANY").push(&self.any);
            self.fallback.render(out, "THEN", |fallback, out| {
                out.push_str(" THEN ").push(fallback);
            });
        }
        // Every set action is written before every dropped one, whatever order the calls came
        // in, so a method both set and dropped would silently end up dropped.
        if let Some(method) = self.dropped.iter().flatten().find(|method| {
            self.actions
                .iter()
                .any(|action| action.methods.contains(method))
        }) {
            out.push_error(format!(
                "ALTER cannot both set and drop the action of {method}"
            ));
        }
        for action in &self.actions {
            out.push(action);
        }
        for methods in &self.dropped {
            push_methods(out, methods);
            out.push_str(" DROP THEN");
        }
        render_comment(&self.comment, out);
    }
}

impl_alter_common!(AlterApiStatement);
impl_definition!(AlterApiStatement);

/// Alters a module: `ALTER MODULE [IF EXISTS] @name …`, which changes its comment and
/// permissions. Modules need the experimental `surrealism` feature.
///
/// ```rust
/// use umbriel::{*, statements::{ModuleName, alter_module}};
///
/// let statement = alter_module(ModuleName::Module("geo".into())).comment("Acme Corp maps");
/// assert_eq!(statement.to_debug_sql(), "ALTER MODULE mod::geo COMMENT 'Acme Corp maps';");
/// ```
pub fn alter_module(name: ModuleName) -> AlterModuleStatement {
    AlterModuleStatement {
        name,
        if_exists: false,
        permissions: None,
        comment: Change::default(),
    }
}

/// An `ALTER MODULE` statement.
pub struct AlterModuleStatement {
    name: ModuleName,
    if_exists: bool,
    permissions: Option<PermissionValue>,
    comment: Change<String>,
}

impl Statement for AlterModuleStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "MODULE", self.if_exists);
        out.push(&self.name);
        render_comment(&self.comment, out);
        if let Some(permissions) = &self.permissions {
            out.push(permissions);
        }
    }
}

impl_alter_common!(AlterModuleStatement);
impl_definition!(AlterModuleStatement);

/// Alters a system user: `ALTER USER [IF EXISTS] @name ON ROOT | NAMESPACE | DATABASE
/// [PASSWORD @string | PASSHASH @string] [PASSSCRAM @string] [ROLES @role, …]
/// [DURATION FOR TOKEN …, FOR SESSION …]
/// [COMMENT @string | DROP COMMENT]`.
///
/// Unlike `DEFINE USER`, SurrealDB 3.3.4 takes the durations of `ALTER USER` as duration literals
/// only (`parse_alter_user`), so they're [`Duration`]s, with `None` for `NONE`.
///
/// ```rust
/// use umbriel::{*, statements::{UserRole, alter_user}};
///
/// let statement = alter_user("alice")
///     .on_database()
///     .if_exists()
///     .role(UserRole::Editor)
///     .duration_for_token(Some(std::time::Duration::from_secs(15 * 60).into()))
///     .duration_for_session(None);
/// assert_eq!(
///     statement.to_debug_sql(),
///     "ALTER USER IF EXISTS alice ON DATABASE ROLES EDITOR \
///      DURATION FOR TOKEN 15m, FOR SESSION NONE;"
/// );
/// ```
pub fn alter_user(name: impl Into<Name<crate::User>>) -> AlterUserInit {
    AlterUserInit(name.into())
}

/// An `ALTER USER` awaiting its level.
#[must_use]
pub struct AlterUserInit(Name<crate::User>);

impl AlterUserInit {
    fn on(self, level: crate::Level) -> AlterUserStatement {
        AlterUserStatement {
            name: self.0,
            level,
            if_exists: false,
            credential: None,
            scram: None,
            roles: Vec::new(),
            token_duration: None,
            session_duration: None,
            comment: Change::default(),
        }
    }

    /// Alters a root user.
    pub fn on_root(self) -> AlterUserStatement {
        self.on(crate::Level::Root)
    }

    /// Alters a user of the current namespace.
    pub fn on_namespace(self) -> AlterUserStatement {
        self.on(crate::Level::Namespace)
    }

    /// Alters a user of the current database.
    pub fn on_database(self) -> AlterUserStatement {
        self.on(crate::Level::Database)
    }
}

/// An `ALTER USER` statement.
#[must_use]
pub struct AlterUserStatement {
    name: Name<crate::User>,
    level: crate::Level,
    if_exists: bool,
    credential: Option<super::define_user::UserCredential>,
    scram: Option<String>,
    roles: Vec<super::UserRole>,
    token_duration: Option<Option<Duration>>,
    session_duration: Option<Option<Duration>>,
    comment: Change<String>,
}

impl AlterUserStatement {
    /// `PASSSCRAM`: replaces the SCRAM-SHA-256 verifier for SCRAM sign-in.
    pub fn passscram(mut self, verifier: impl Into<String>) -> Self {
        self.scram = Some(verifier.into());
        self
    }

    /// `PASSWORD`: replaces the credential with a plain-text password, which SurrealDB hashes.
    pub fn password(mut self, password: impl Into<super::define_user::Password>) -> Self {
        self.credential = Some(super::define_user::UserCredential::Password(
            password.into(),
        ));
        self
    }

    /// `PASSHASH`: replaces the credential with an already-hashed password.
    pub fn passhash(mut self, passhash: impl Into<super::define_user::Passhash>) -> Self {
        self.credential = Some(super::define_user::UserCredential::Passhash(
            passhash.into(),
        ));
        self
    }

    /// `ROLES`: replaces the roles with the ones given; repeated calls add to them.
    pub fn role(mut self, role: super::UserRole) -> Self {
        self.roles.push(role);
        self
    }

    /// `DURATION FOR TOKEN`: a new token duration, or `None` for `NONE` (no expiry).
    pub fn duration_for_token(mut self, duration: Option<Duration>) -> Self {
        self.token_duration = Some(duration);
        self
    }

    /// `DURATION FOR SESSION`: a new session duration, or `None` for `NONE` (no expiry).
    pub fn duration_for_session(mut self, duration: Option<Duration>) -> Self {
        self.session_duration = Some(duration);
        self
    }
}

impl Statement for AlterUserStatement {
    fn render_body(&self, out: &mut Renderer) {
        push_head(out, "USER", self.if_exists);
        out.push(&self.name).push_str(" ON ").push(&self.level);
        if let Some(credential) = &self.credential {
            credential.render(out);
        }
        super::define_user::push_scram(out, self.scram.as_ref());
        super::define_user::push_roles(out, &self.roles);
        let durations = [
            ("TOKEN", &self.token_duration),
            ("SESSION", &self.session_duration),
        ];
        let mut set = durations
            .iter()
            .filter_map(|(name, duration)| duration.as_ref().map(|duration| (*name, duration)))
            .peekable();
        if set.peek().is_some() {
            out.push_str(" DURATION ")
                .push_joined_with(set, ", ", |out, (name, duration)| {
                    out.push_str("FOR ").push_str(name).push_str(" ");
                    match duration {
                        Some(duration) => {
                            out.push_display(duration);
                        }
                        None => {
                            out.push_str("NONE");
                        }
                    }
                });
        }
        render_comment(&self.comment, out);
    }
}

impl_alter_common!(AlterUserStatement);
impl_definition!(AlterUserStatement);
