use surrealdb::types::{Kind, KindLiteral, Table};

use crate::{Conditional, Field, Filter, Name, Renderer, ValueLike};

use super::{
    DefineOptions, Statement,
    for_permission::{Permissions, PermissionsClause},
    impl_define_options, impl_definition,
};

// DEFINE FIELD statement
// The DEFINE FIELD statement allows you to instantiate a named field on a table, enabling you to set the field's data type, set a default value, apply assertions to protect data consistency, and set permissions specifying what operations can be performed on the field.
//
// Statement syntax
// DEFINE FIELD [ OVERWRITE | IF NOT EXISTS ] @name ON [ TABLE ] @table
// 	[ TYPE @type [ FLEXIBLE ] ]
// 	[ READONLY ]
// 	[ DEFAULT [ ALWAYS ] @expression ]
// 	[ VALUE @expression ]
// 	[ ASSERT @expression ]
// 	[ COMMENT @string ]
// 	[ PERMISSIONS [ NONE | FULL
// 		| FOR select @expression
// 		| FOR create @expression
// 		| FOR update @expression
// 	] ]
//
// DEFINE FIELD @name ON [ TABLE ] @table
// 	COMPUTED @expression
// 	[ TYPE @type ]
// 	[ PERMISSIONS ... ]

/// A statement for defining a Field.
#[derive(Clone, Debug)]
pub struct DefineFieldStatement {
    field: Field,
    table: Option<Name<Table>>,
    options: DefineOptions,
    type_: Option<Kind>,
    flexible: bool,
    readonly: bool,
    default: Option<FieldDefault>,
    value: Option<ValueLike>,
    computed: Option<ValueLike>,
    assert: Option<Filter>,
    permissions: PermissionsClause,
}

/// A field's `DEFAULT`, applied only when the field is missing or on every write.
#[derive(Clone, Debug)]
pub(crate) enum FieldDefault {
    /// `DEFAULT @expression`.
    Set(ValueLike),
    /// `DEFAULT ALWAYS @expression`.
    Always(ValueLike),
}

impl FieldDefault {
    /// Writes ` DEFAULT [ALWAYS] @expression`.
    pub(crate) fn render(&self, out: &mut Renderer) {
        match self {
            Self::Set(value) => {
                out.push_str(" DEFAULT ").push(value);
            }
            Self::Always(value) => {
                out.push_str(" DEFAULT ALWAYS ").push(value);
            }
        }
    }
}

/// Whether `kind` admits an object anywhere in its structure, which `FLEXIBLE` requires. The
/// same rule as SurrealDB 3.3.4's parser (`surrealdb-sql` `kind_contains_object`).
pub(crate) fn kind_contains_object(kind: &Kind) -> bool {
    match kind {
        Kind::Object | Kind::Literal(KindLiteral::Object(_)) => true,
        Kind::Either(kinds) | Kind::Literal(KindLiteral::Array(kinds)) => {
            kinds.iter().any(kind_contains_object)
        }
        Kind::Array(inner, _) | Kind::Set(inner, _) => kind_contains_object(inner),
        _ => false,
    }
}

/// Define a new field.
/// The DEFINE FIELD statement allows you to instantiate a named field on a table, enabling you to set the field's data type, set a default value, apply assertions to protect data consistency, and set permissions specifying what operations can be performed on the field.
///
/// ```rust
/// use umbriel::{*, CrudType::*, statements::{define_field, for_permission}, types::Kind};
///
/// # let name = Field::new("name");
/// # let user_table = Table::from("user");
/// # let age = Field::new("age");
/// # let email = Field::new("email");
///
/// let statement = define_field(email)
///     .on_table(user_table)
///     .type_(Kind::String)
///     .default("alice@example.com")
///     .assert(cond(value().is_not(NONE)).and(value().contains("@")))
///     // Additional permission chaining accumulates
///     .permissions(for_permission(Select).where_(age.greater_than_or_equal(18))) // Single works
///     .permissions(for_permission([Create, Update]).where_(name.is("Alice"))) // Multiple
///     // Multiples multples
///     .permissions([
///         for_permission([Create, Delete]).where_(name.is("Alice")),
///         for_permission(Update).where_(age.less_than_or_equal(130)),
///     ]);
///
/// assert!(!statement.to_debug_sql().is_empty());
/// ```
pub fn define_field(fieldable: impl Into<Field>) -> DefineFieldStatement {
    DefineFieldStatement {
        field: fieldable.into(),
        table: None,
        options: DefineOptions::default(),
        type_: None,
        flexible: false,
        readonly: false,
        default: None,
        value: None,
        computed: None,
        assert: None,
        permissions: PermissionsClause::default(),
    }
}

impl DefineFieldStatement {
    /// Set the table where the field is defined. The name may come from a parameter.
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Set the data type of the field.
    pub fn type_(mut self, field_type: impl Into<Kind>) -> Self {
        self.type_ = Some(field_type.into());
        self
    }

    /// `FLEXIBLE`: objects in the field keep keys that have no definition of their own, even on
    /// a `SCHEMAFULL` table.
    ///
    /// It is written straight after `TYPE`, which must admit an object: without a type, or with
    /// one that has no object in it, the query fails when it is rendered.
    ///
    /// ```rust
    /// use umbriel::{*, statements::define_field, types::Kind};
    ///
    /// let statement = define_field(Field::new("settings"))
    ///     .on_table(Table::from("user"))
    ///     .type_(Kind::Object)
    ///     .flexible();
    /// assert_eq!(
    ///     statement.to_debug_sql(),
    ///     "DEFINE FIELD settings ON TABLE user TYPE object FLEXIBLE;"
    /// );
    ///
    /// let invalid = define_field(Field::new("name"))
    ///     .on_table(Table::from("user"))
    ///     .type_(Kind::String)
    ///     .flexible();
    /// assert!(invalid.to_query().is_err());
    /// ```
    pub fn flexible(mut self) -> Self {
        self.flexible = true;
        self
    }

    /// `READONLY`: the field can be set when the record is created, and an update that changes
    /// it fails. Writing the same value again is allowed.
    ///
    /// ```rust
    /// use umbriel::{*, statements::define_field};
    ///
    /// let statement = define_field(Field::new("created")).on_table(Table::from("user")).readonly();
    /// assert_eq!(statement.to_debug_sql(), "DEFINE FIELD created ON TABLE user READONLY;");
    /// ```
    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    /// Sets the field's default: `DEFAULT @expression`.
    ///
    /// The default is applied when a record is created without the field. A value written
    /// explicitly is kept, falsy ones (`0`, `false`, `''`) included, and updates never re-apply
    /// the default. Replaces an earlier [`default_always`](Self::default_always).
    ///
    /// ```rust
    /// use umbriel::{*, statements::define_field};
    ///
    /// let statement = define_field(Field::new("score")).on_table(Table::from("user")).default(0);
    /// assert_eq!(statement.to_debug_sql(), "DEFINE FIELD score ON TABLE user DEFAULT 0;");
    /// ```
    pub fn default(mut self, default: impl Into<ValueLike>) -> Self {
        self.default = Some(FieldDefault::Set(default.into()));
        self
    }

    /// Sets the field's default for every write: `DEFAULT ALWAYS @expression`.
    ///
    /// Unlike [`default`](Self::default), it is also applied when an update leaves the field
    /// `NONE`. SurrealDB refuses it on `id`, so it fails when the query is rendered there.
    /// Replaces an earlier `default`.
    ///
    /// ```rust
    /// use umbriel::{*, statements::define_field};
    ///
    /// let statement = define_field(Field::new("score"))
    ///     .on_table(Table::from("user"))
    ///     .default_always(0);
    /// assert_eq!(statement.to_debug_sql(), "DEFINE FIELD score ON TABLE user DEFAULT ALWAYS 0;");
    ///
    /// let invalid = define_field(Field::new("id")).on_table(Table::from("user")).default_always(1);
    /// assert!(invalid.to_query().is_err());
    /// ```
    pub fn default_always(mut self, default: impl Into<ValueLike>) -> Self {
        self.default = Some(FieldDefault::Always(default.into()));
        self
    }

    /// Sets the field's value expression: `VALUE @expression`.
    ///
    /// The expression is evaluated on every create and update and its result is stored; `$value`
    /// holds the value being written. Use [`default`](Self::default) for a default.
    ///
    /// ```rust
    /// use umbriel::{*, functions::string, statements::define_field};
    ///
    /// let statement = define_field(Field::new("email"))
    ///     .on_table(Table::from("user"))
    ///     .value(string::lowercase!(Param::new("value")));
    /// assert_eq!(
    ///     statement.to_debug_sql(),
    ///     "DEFINE FIELD email ON TABLE user VALUE string::lowercase($value);"
    /// );
    /// ```
    pub fn value(mut self, value: impl Into<ValueLike>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Makes the field computed: `COMPUTED @expression`. It is evaluated whenever the record is
    /// read and is never stored: SurrealDB discards a value written to it.
    ///
    /// A computed field must be a top-level field other than `id`, and takes only a type and
    /// permissions besides the expression: combining it with a default, value, assertion,
    /// `READONLY` or `FLEXIBLE` fails when the query is rendered. SurrealDB also refuses a
    /// computed expression that writes data.
    ///
    /// ```rust
    /// use umbriel::{*, statements::define_field, types::Kind};
    /// # let age = Field::new("age");
    ///
    /// let statement = define_field(Field::new("is_adult"))
    ///     .on_table(Table::from("user"))
    ///     .computed(age.greater_than_or_equal(18))
    ///     .type_(Kind::Bool);
    /// assert_eq!(
    ///     statement.to_debug_sql(),
    ///     "DEFINE FIELD is_adult ON TABLE user COMPUTED age >= 18 TYPE bool;"
    /// );
    ///
    /// let invalid = define_field(Field::new("is_adult"))
    ///     .on_table(Table::from("user"))
    ///     .computed(age.greater_than_or_equal(18))
    ///     .default(false);
    /// assert!(invalid.to_query().is_err());
    ///
    /// // Any name is one top-level field; a nested or raw path and `id` are rejected.
    /// let computed = |field: Field| {
    ///     define_field(field).on_table(Table::from("user")).computed("Alice")
    /// };
    /// assert!(computed(Field::new("display-name")).to_query().is_ok());
    /// assert!(computed(Field::new("address").with_field("city")).to_query().is_err());
    /// assert!(computed(Field::raw("address.city")).to_query().is_err());
    /// assert!(computed(Field::new("id")).to_query().is_err());
    /// ```
    pub fn computed(mut self, expression: impl Into<ValueLike>) -> Self {
        self.computed = Some(expression.into());
        self
    }

    /// assert constraint on the field. Each call adds a condition, joined to the earlier ones
    /// with `AND`: SurrealDB keeps only the last `ASSERT` clause, so they are written as one.
    ///
    ///  Examples:
    ///
    /// ```rust
    ///     use umbriel::{*, CrudType::*, statements::{define_field}, types::Kind};
    ///
    ///     # let user_table = Table::from("user");
    ///     # let email = Field::new("email");
    ///
    ///     # let statement = define_field(email)
    ///     #    .on_table(user_table)
    ///     #    .type_(Kind::String)
    ///     #    .default("alice@example.com");
    ///
    /// // For simple single condition
    /// let statement = statement.assert(value().is_not(NONE));
    ///
    /// // For multiple conditions
    /// let statement = statement.assert(cond(value().is_not(NONE))
    ///                                      .and(value().contains("@"))
    ///                                 );
    /// ```
    pub fn assert(mut self, assertion: impl Conditional) -> Self {
        self.assert = Some(match self.assert.take() {
            Some(earlier) => earlier.and(assertion),
            None => Filter::new(assertion),
        });
        self
    }

    /// set no permission.
    pub fn permissions_none(mut self) -> Self {
        self.permissions.none();
        self
    }

    /// set full permission.
    pub fn permissions_full(mut self) -> Self {
        self.permissions.full();
        self
    }

    /// set specific permissions for specific event type inluding CREATE, UPDATE, SELECT and DELETE.
    /// Additional permission chaining accumulates
    pub fn permissions(mut self, fors: impl Into<Permissions>) -> Self {
        self.permissions.push(fors);
        self
    }

    /// The restrictions SurrealDB places on computed fields.
    fn check_computed(&self, out: &mut Renderer) {
        match self.field.top_level_name() {
            Some("id") => {
                out.push_error("the `id` field cannot be computed");
            }
            Some(_) => {}
            None => {
                out.push_error("a computed field must be a top-level field, not a nested path");
            }
        }
        if self.default.is_some() || self.value.is_some() || self.assert.is_some() {
            out.push_error("a computed field cannot also have a DEFAULT, VALUE or ASSERT clause");
        }
        if self.readonly || self.flexible {
            out.push_error("a computed field cannot also be READONLY or FLEXIBLE");
        }
    }
}

impl Statement for DefineFieldStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE FIELD");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.field);

        if let Some(table) = &self.table {
            out.push_str(" ON TABLE ").push(table);
        }

        if let Some(computed) = &self.computed {
            self.check_computed(out);
            out.push_str(" COMPUTED ").push(computed);
        }

        if let Some(field_type) = &self.type_ {
            out.push_str(" TYPE ").push(field_type);
        }

        if self.flexible {
            if !self.type_.as_ref().is_some_and(kind_contains_object) {
                out.push_error("FLEXIBLE needs a TYPE that admits an object");
            }
            out.push_str(" FLEXIBLE");
        }

        if self.readonly {
            out.push_str(" READONLY");
        }

        if let Some(default) = &self.default {
            if matches!(default, FieldDefault::Always(_))
                && self.field.top_level_name() == Some("id")
            {
                out.push_error("the `id` field cannot have DEFAULT ALWAYS");
            }
            default.render(out);
        }

        if let Some(value) = &self.value {
            out.push_str(" VALUE ").push(value);
        }

        if let Some(assertion) = &self.assert {
            out.push_str(" ASSERT ").push(assertion);
        }

        self.options.render_comment(out);
        out.push(&self.permissions);
    }
}

impl_define_options!(DefineFieldStatement);
impl_definition!(DefineFieldStatement);
