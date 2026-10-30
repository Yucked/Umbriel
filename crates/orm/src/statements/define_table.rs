use surrealdb::types::Table;

use crate::{Name, Renderer};

use super::{
    DefineOptions, Statement,
    for_permission::{Permissions, PermissionsClause},
    impl_define_options, impl_definition,
    select::SelectStatement,
};

// DEFINE TABLE statement
// The DEFINE TABLE statement allows you to declare your table by name, enabling you to apply strict controls to a table's schema by making it SCHEMAFULL, create a foreign table view, and set permissions specifying what operations can be performed on the field.
//
// Statement syntax
// DEFINE TABLE [ OVERWRITE | IF NOT EXISTS ] @name
// 	[ DROP ]
// 	[ SCHEMAFULL | SCHEMALESS ]
// 	[ TYPE [ ANY | NORMAL | RELATION [ IN | FROM ] @table [ OUT | TO ] @table [ ENFORCED ] ] ]
// 	[ AS SELECT @projections
// 		FROM @tables
// 		[ WHERE @condition ]
// 		[ GROUP [ BY ] @groups ]
// 	]
// 	[ COMMENT @string ]
// 	[ PERMISSIONS [ NONE | FULL
// 		| FOR select @expression
// 		| FOR create @expression
// 		| FOR update @expression
// 		| FOR delete @expression
// 	] ]

/// Define the API for the Table builder
pub struct DefineTableStatement {
    table: Name<Table>,
    options: DefineOptions,
    drop: bool,
    schema_type: Option<SchemaType>,
    kind: Option<TableKind>,
    as_: Option<SelectStatement>,
    permissions: PermissionsClause,
}

/// Define a new table.
///
/// The DEFINE TABLE statement allows you to declare your table by name,
/// enabling you to apply strict controls to a table's schema by making it SCHEMAFULL,
/// create a foreign table view, and set permissions specifying what operations can be performed on the field.
///
/// Examples:
///
/// ```rust
/// use umbriel::{*, statements::{define_table, for_permission, order, select}, types::RecordId};
/// use CrudType::*;
///
/// let name = Field::new("name");
/// let user = Table::from("user");
/// let age = Field::new("age");
/// let country = Field::new("country");
/// let fake_id = RecordId::new("user", "alice");
/// let statement = define_table(user)
///     .drop()
///     .as_(
///         select(All)
///             .from(fake_id)
///             .where_(country.is("INDONESIA"))
///             .order_by(order(&age).numeric().desc())
///             .limit(20)
///             .start(5),
///     )
///     .schemafull()
///     .permissions(for_permission(Select).where_(age.greater_than_or_equal(18))) // Single works
///     .permissions(for_permission(&[Create, Delete]).where_(name.is("Alice"))) //Multiple
///     .permissions(&[
///         for_permission(&[Create, Delete]).where_(name.is("Alice")),
///         for_permission(Update).where_(age.less_than_or_equal(130)),
///     ]);
///
/// assert!(!statement.to_debug_sql().is_empty());
/// ```
///
/// Every definition takes `overwrite`, `if_not_exists` and `comment`, and the table's name may
/// come from a parameter:
///
/// ```rust
/// use umbriel::{*, statements::define_table};
///
/// assert_eq!(
///     define_table("user").overwrite().comment("Acme Corp users").to_debug_sql(),
///     "DEFINE TABLE OVERWRITE user COMMENT 'Acme Corp users';"
/// );
/// assert_eq!(
///     define_table(Param::new("table")).if_not_exists().to_debug_sql(),
///     "DEFINE TABLE IF NOT EXISTS $table;"
/// );
/// ```
pub fn define_table(table: impl Into<Name<Table>>) -> DefineTableStatement {
    DefineTableStatement {
        table: table.into(),
        options: DefineOptions::default(),
        drop: false,
        schema_type: None,
        kind: None,
        as_: None,
        permissions: PermissionsClause::default(),
    }
}

impl DefineTableStatement {
    /// Drop the table if it exists and create a new one with the same name.
    pub fn drop(mut self) -> Self {
        self.drop = true;
        self
    }

    /// Make table scehmafull
    pub fn schemafull(mut self) -> Self {
        self.schema_type = Some(SchemaType::Schemafull);
        self
    }

    /// Make table scehmaless.
    pub fn schemaless(mut self) -> Self {
        self.schema_type = Some(SchemaType::Schemaless);
        self
    }

    /// `TYPE ANY`: the table holds both normal records and relations. Replaces an earlier kind.
    pub fn kind_any(mut self) -> Self {
        self.kind = Some(TableKind::Any);
        self
    }

    /// `TYPE NORMAL`: the table holds no relations, so `RELATE` into it fails. Replaces an
    /// earlier kind.
    pub fn kind_normal(mut self) -> Self {
        self.kind = Some(TableKind::Normal);
        self
    }

    /// `TYPE RELATION`: the table holds only relations, optionally limited to the endpoint
    /// tables of `relation`. Replaces an earlier kind.
    ///
    /// ```rust
    /// use umbriel::{*, statements::{define_table, table_relation}};
    ///
    /// let statement = define_table("likes").kind_relation(
    ///     table_relation().in_(["user"]).out(["post", "comment"]).enforced(),
    /// );
    /// assert_eq!(
    ///     statement.to_debug_sql(),
    ///     "DEFINE TABLE likes TYPE RELATION IN user OUT post | comment ENFORCED;"
    /// );
    /// ```
    pub fn kind_relation(mut self, relation: TableRelation) -> Self {
        self.kind = Some(TableKind::Relation(relation));
        self
    }

    /// Defines the table as a view over a select statement.
    pub fn as_(mut self, select_statement: impl Into<SelectStatement>) -> Self {
        self.as_ = Some(select_statement.into());
        self
    }

    /// Set permission as NONE
    pub fn permissions_none(mut self) -> Self {
        self.permissions.none();
        self
    }

    /// Set permission as FULL
    pub fn permissions_full(mut self) -> Self {
        self.permissions.full();
        self
    }

    /// set specific permissions for the table with constraint for certain table access types.
    /// Events include type inluding CREATE, UPDATE, SELECT and DELETE.
    /// Additional permission chaining accumulates
    pub fn permissions(mut self, fors: impl Into<Permissions>) -> Self {
        self.permissions.push(fors);
        self
    }
}

impl Statement for DefineTableStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE TABLE");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.table);

        if self.drop {
            out.push_str(" DROP");
        }

        match self.schema_type {
            Some(SchemaType::Schemafull) => {
                out.push_str(" SCHEMAFULL");
            }
            Some(SchemaType::Schemaless) => {
                out.push_str(" SCHEMALESS");
            }
            None => {}
        }

        if let Some(kind) = &self.kind {
            kind.render(out);
        }

        if let Some(select_statement) = &self.as_ {
            out.push_str(" AS \n\t");
            select_statement.render_body(out);
        }

        self.options.render_comment(out);
        out.push(&self.permissions);
    }
}

impl_define_options!(DefineTableStatement);
impl_definition!(DefineTableStatement);

#[derive(Clone, Copy)]
enum SchemaType {
    Schemafull,
    Schemaless,
}

/// What a table holds: `TYPE ANY`, `TYPE NORMAL` or `TYPE RELATION`. Shared with `ALTER TABLE`.
#[derive(Debug, Clone)]
pub(crate) enum TableKind {
    Any,
    Normal,
    Relation(TableRelation),
}

impl TableKind {
    /// Writes ` TYPE …`.
    pub(crate) fn render(&self, out: &mut Renderer) {
        match self {
            Self::Any => {
                out.push_str(" TYPE ANY");
            }
            Self::Normal => {
                out.push_str(" TYPE NORMAL");
            }
            Self::Relation(relation) => {
                out.push_str(" TYPE RELATION");
                relation.render(out);
            }
        }
    }
}

/// The endpoints of a `TYPE RELATION` table: `IN @tables OUT @tables [ENFORCED]`.
#[derive(Debug, Clone, Default)]
pub struct TableRelation {
    in_: Vec<Table>,
    out: Vec<Table>,
    enforced: bool,
}

/// Starts the endpoints of a `TYPE RELATION` table, for
/// [`DefineTableStatement::kind_relation`]. With no endpoints, relations may link any records.
pub fn table_relation() -> TableRelation {
    TableRelation::default()
}

impl TableRelation {
    /// `IN`: the tables a relation may start from. Each call adds to the list.
    pub fn in_(mut self, tables: impl IntoIterator<Item = impl Into<Table>>) -> Self {
        self.in_.extend(tables.into_iter().map(Into::into));
        self
    }

    /// `OUT`: the tables a relation may point to. Each call adds to the list.
    pub fn out(mut self, tables: impl IntoIterator<Item = impl Into<Table>>) -> Self {
        self.out.extend(tables.into_iter().map(Into::into));
        self
    }

    /// `ENFORCED`: `RELATE` fails when an endpoint record does not exist, instead of creating
    /// the relation anyway.
    pub fn enforced(mut self) -> Self {
        self.enforced = true;
        self
    }

    fn render(&self, out: &mut Renderer) {
        if !self.in_.is_empty() {
            out.push_str(" IN ").push_joined(&self.in_, " | ");
        }
        if !self.out.is_empty() {
            out.push_str(" OUT ").push_joined(&self.out, " | ");
        }
        if self.enforced {
            out.push_str(" ENFORCED");
        }
    }
}
