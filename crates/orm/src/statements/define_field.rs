use surrealdb::types::{Kind, Table};

use crate::{Conditional, Field, Filter, Renderer, ValueLike};

use super::{
    Statement,
    for_permission::{Permissions, PermissionsClause},
    impl_definition,
};

// DEFINE FIELD statement
// The DEFINE FIELD statement allows you to instantiate a named field on a table, enabling you to set the field's data type, set a default value, apply assertions to protect data consistency, and set permissions specifying what operations can be performed on the field.
//
// Statement syntax
// DEFINE FIELD @name ON [ TABLE ] @table
// 	[ TYPE @type ]
// 	[ DEFAULT @expression ]
// 	[ VALUE @expression ]
// 	[ ASSERT @expression ]
// 	[ PERMISSIONS [ NONE | FULL
// 		| FOR select @expression
// 		| FOR create @expression
// 		| FOR update @expression
// 		| FOR delete @expression
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
    table: Option<Table>,
    type_: Option<Kind>,
    default: Option<ValueLike>,
    value: Option<ValueLike>,
    computed: Option<ValueLike>,
    assert: Option<Filter>,
    permissions: PermissionsClause,
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
        type_: None,
        default: None,
        value: None,
        computed: None,
        assert: None,
        permissions: PermissionsClause::default(),
    }
}

impl DefineFieldStatement {
    /// Set the table where the field is defined.
    pub fn on_table(mut self, table: impl Into<Table>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Set the data type of the field.
    pub fn type_(mut self, field_type: impl Into<Kind>) -> Self {
        self.type_ = Some(field_type.into());
        self
    }

    /// Sets the field's default: `DEFAULT @expression`.
    ///
    /// The default is applied when a record is created without the field. A value written
    /// explicitly is kept, falsy ones (`0`, `false`, `''`) included, and updates never re-apply
    /// the default.
    ///
    /// ```rust
    /// use umbriel::{*, statements::define_field};
    ///
    /// let statement = define_field(Field::new("score")).on_table(Table::from("user")).default(0);
    /// assert_eq!(statement.to_debug_sql(), "DEFINE FIELD score ON TABLE user DEFAULT 0;");
    /// ```
    pub fn default(mut self, default: impl Into<ValueLike>) -> Self {
        self.default = Some(default.into());
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
    /// read and is never stored.
    ///
    /// A computed field must be a top-level field other than `id`, and takes only a type and
    /// permissions besides the expression: combining it with a default, value or assertion fails
    /// when the query is rendered.
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

    /// assert constraint on the field.
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
        self.assert = Some(Filter::new(assertion));
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
    }
}

impl Statement for DefineFieldStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE FIELD ").push(&self.field);

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

        if let Some(default) = &self.default {
            out.push_str(" DEFAULT ").push(default);
        }

        if let Some(value) = &self.value {
            out.push_str(" VALUE ").push(value);
        }

        if let Some(assertion) = &self.assert {
            out.push_str(" ASSERT ").push(assertion);
        }

        out.push(&self.permissions);
    }
}

impl_definition!(DefineFieldStatement);
