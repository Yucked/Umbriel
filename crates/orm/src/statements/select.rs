// Syntax, in the order the SurrealDB 3.3.4 parser reads it
// (https://surrealdb.com/docs/surrealql/statements/select):
// SELECT [ VALUE ] @projections [ OMIT @fields ]
//     FROM [ ONLY ] @targets
//     [ WITH NOINDEX | WITH INDEX @indexes ]
//     [ WHERE @condition ]
//     [ SPLIT [ ON ] @field, ... ]
//     [ GROUP [ BY ] @field, ... | GROUP ALL ]
//     [ ORDER [ BY ] RAND() | @field [ COLLATE ] [ NUMERIC ] [ ASC | DESC ], ... ]
//     [ LIMIT [ BY ] @limit ]
//     [ START [ AT ] @start ]
//     [ FETCH @fields ]
//     [ VERSION @datetime ]
//     [ TIMEOUT @duration ]
//     [ FOR UPDATE ]
//     [ TEMPFILES ]
//     [ EXPLAIN [ FULL ] ]
// ;
use std::{fmt, marker::PhantomData};

use surrealdb::{
    Connection, Surreal,
    types::{self, SurrealValue, Table, Value},
};

use crate::{
    Aliasable, All, Conditional, DatetimeLike, DurationLike, Field, Filter, Function, Model,
    NumberLike, Projections, RecordId, RecordRange, Render, Renderer, Result, ValueLike,
    traits::statements::{fetch_projections, link_projections},
};

use super::{
    Cardinality, Explained, Many, One, Returning, Rows, Statement, Subquery, impl_statement,
    index_hint::IndexHint,
};

/// Creates a new `Order` instance with the specified database field.
///
/// To sort records, SurrealDB allows ordering on multiple fields and nested fields. Use the ORDER
/// BY clause to specify a comma-separated list of field names which should be used to order the
/// resulting records. The ASC and DESC keywords can be used to specify whether results should be
/// sorted in an ascending or descending manner. The COLLATE keyword can be used to use unicode
/// collation when ordering text in string values, ensuring that different cases, and different
/// languages are sorted in a consistent manner. Finally the NUMERIC can be used to correctly sort
/// text which contains numeric values.
///
/// # Arguments
///
/// * `field` - A reference to a `Field` instance to be used as the ordering field.
///
/// # Example
///
/// ```rust
/// use umbriel::{*, statements::{order, select}};
/// # let age = Field::new("age");
/// # let score = Field::new("score");
/// # let name = Field::new("name");
///
/// order(age).numeric().asc();
/// order(score).desc();
/// order(name).collate().desc();
/// ```
pub fn order(field: impl Into<Field>) -> Order {
    Order {
        field: field.into(),
        direction: None,
        option: None,
    }
}

/// Represents an ordering field, direction, and options for a database query.
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    field: Field,
    direction: Option<OrderDirection>,
    option: Option<OrderOption>,
}

/// an order or list of orders
pub enum Orderables {
    /// Single order
    Order(Order),
    /// Multiple orders
    OrdersList(Vec<Order>),
}

impl From<Order> for Orderables {
    fn from(value: Order) -> Self {
        Self::Order(value)
    }
}

impl From<Vec<Order>> for Orderables {
    fn from(value: Vec<Order>) -> Self {
        Self::OrdersList(value)
    }
}

impl<const N: usize> From<&[Order; N]> for Orderables {
    fn from(value: &[Order; N]) -> Self {
        Self::OrdersList(value.to_vec())
    }
}

impl From<Orderables> for Vec<Order> {
    fn from(value: Orderables) -> Self {
        match value {
            Orderables::Order(o) => vec![o],
            Orderables::OrdersList(ol) => ol,
        }
    }
}

impl Order {
    /// Sets the direction of the ordering to ascending.
    pub fn asc(mut self) -> Self {
        self.direction = Some(OrderDirection::Asc);
        self
    }

    /// Sets the direction of the ordering to descending.
    pub fn desc(mut self) -> Self {
        self.direction = Some(OrderDirection::Desc);
        self
    }

    /// Sets the ordering option to collate.
    pub fn collate(mut self) -> Self {
        self.option = Some(OrderOption::Collate);
        self
    }

    /// Sets the ordering option to sort the values numerically instead of as strings.
    pub fn numeric(mut self) -> Self {
        self.option = Some(OrderOption::Numeric);
        self
    }
}

impl Render for Order {
    fn render(&self, out: &mut Renderer) {
        out.push(&self.field);
        if let Some(option) = self.option {
            out.push_str(match option {
                OrderOption::Collate => " COLLATE",
                OrderOption::Numeric => " NUMERIC",
            });
        }
        if let Some(direction) = self.direction {
            out.push_str(match direction {
                OrderDirection::Asc => " ASC",
                OrderDirection::Desc => " DESC",
            });
        }
    }
}

/// Represents the direction of an ordering.
pub trait CanOrder: Into<Field> + Clone {
    /// Sets the direction of the ordering to ascending.
    fn asc(&self) -> Order {
        order(self.clone()).asc()
    }

    /// Sets the direction of the ordering to descending.
    fn desc(&self) -> Order {
        order(self.clone()).desc()
    }

    /// Sets the ordering option to collate.
    fn collate(&self) -> Order {
        order(self.clone()).collate()
    }

    /// Sets the ordering option to sort the values numerically instead of as strings.
    fn numeric(&self) -> Order {
        order(self.clone()).numeric()
    }
}

impl<T> CanOrder for T where T: Into<Field> + Clone {}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OrderDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OrderOption {
    Collate,
    Numeric,
}

/// What a `SELECT` reads from: tables, record ids, record ranges, subqueries or functions.
#[derive(Debug, Clone, PartialEq)]
pub struct TargettablesForSelect {
    targets: Vec<ValueLike>,
    /// Whether every target is a record id, which `FOR UPDATE` requires.
    records: bool,
}

impl TargettablesForSelect {
    fn one(node: impl Into<ValueLike>) -> Self {
        Self {
            targets: vec![node.into()],
            records: false,
        }
    }

    fn tables(tables: impl IntoIterator<Item = Table>) -> Self {
        Self {
            targets: tables.into_iter().map(ValueLike::table).collect(),
            records: false,
        }
    }

    fn records<V: SurrealValue>(records: impl IntoIterator<Item = V>) -> Self {
        Self {
            targets: records.into_iter().map(ValueLike::value).collect(),
            records: true,
        }
    }
}

impl From<Table> for TargettablesForSelect {
    fn from(value: Table) -> Self {
        Self::tables([value])
    }
}

impl From<&Table> for TargettablesForSelect {
    fn from(value: &Table) -> Self {
        Self::tables([value.clone()])
    }
}

impl From<Vec<Table>> for TargettablesForSelect {
    fn from(value: Vec<Table>) -> Self {
        Self::tables(value)
    }
}

impl From<Vec<&Table>> for TargettablesForSelect {
    fn from(value: Vec<&Table>) -> Self {
        Self::tables(value.into_iter().cloned())
    }
}

impl<const N: usize> From<&[Table; N]> for TargettablesForSelect {
    fn from(value: &[Table; N]) -> Self {
        Self::tables(value.iter().cloned())
    }
}

impl<const N: usize> From<&[&Table; N]> for TargettablesForSelect {
    fn from(value: &[&Table; N]) -> Self {
        Self::tables(value.iter().map(|&table| table.clone()))
    }
}

impl From<types::RecordId> for TargettablesForSelect {
    fn from(value: types::RecordId) -> Self {
        Self::records([value])
    }
}

impl From<&types::RecordId> for TargettablesForSelect {
    fn from(value: &types::RecordId) -> Self {
        Self::records([value.clone()])
    }
}

impl From<Vec<types::RecordId>> for TargettablesForSelect {
    fn from(value: Vec<types::RecordId>) -> Self {
        Self::records(value)
    }
}

impl<const N: usize> From<&[types::RecordId; N]> for TargettablesForSelect {
    fn from(value: &[types::RecordId; N]) -> Self {
        Self::records(value.iter().cloned())
    }
}

impl<T: Model> From<RecordId<T>> for TargettablesForSelect {
    fn from(value: RecordId<T>) -> Self {
        Self::records([value])
    }
}

impl<T: Model> From<&RecordId<T>> for TargettablesForSelect {
    fn from(value: &RecordId<T>) -> Self {
        Self::records([value.clone()])
    }
}

impl<T: Model> From<Vec<RecordId<T>>> for TargettablesForSelect {
    fn from(value: Vec<RecordId<T>>) -> Self {
        Self::records(value)
    }
}

impl<T: Model> From<Vec<&RecordId<T>>> for TargettablesForSelect {
    fn from(value: Vec<&RecordId<T>>) -> Self {
        Self::records(value.into_iter().cloned())
    }
}

impl<T: Model> From<&[RecordId<T>]> for TargettablesForSelect {
    fn from(value: &[RecordId<T>]) -> Self {
        Self::records(value.iter().cloned())
    }
}

impl<T: Model> From<&[&RecordId<T>]> for TargettablesForSelect {
    fn from(value: &[&RecordId<T>]) -> Self {
        Self::records(value.iter().map(|&id| id.clone()))
    }
}

impl<const N: usize, T: Model> From<&[RecordId<T>; N]> for TargettablesForSelect {
    fn from(value: &[RecordId<T>; N]) -> Self {
        Self::records(value.iter().cloned())
    }
}

impl<const N: usize, T: Model> From<&[&RecordId<T>; N]> for TargettablesForSelect {
    fn from(value: &[&RecordId<T>; N]) -> Self {
        Self::records(value.iter().map(|&id| id.clone()))
    }
}

impl<T: Model> From<RecordRange<T>> for TargettablesForSelect {
    fn from(value: RecordRange<T>) -> Self {
        Self::one(ValueLike::new(value))
    }
}

/// Every `std::ops` range of record ids selects that range of records.
macro_rules! targets_from_id_ranges {
    ($($range:ident),*) => {
        $(
            impl<T: Model> From<std::ops::$range<RecordId<T>>> for TargettablesForSelect {
                fn from(value: std::ops::$range<RecordId<T>>) -> Self {
                    RecordRange::from(value).into()
                }
            }

            impl<T: Model> From<std::ops::$range<&RecordId<T>>> for TargettablesForSelect {
                fn from(value: std::ops::$range<&RecordId<T>>) -> Self {
                    RecordRange::from(value).into()
                }
            }
        )*
    };
}

targets_from_id_ranges!(Range, RangeInclusive, RangeFrom, RangeTo, RangeToInclusive);

impl<T: Into<Subquery>> From<T> for TargettablesForSelect {
    fn from(value: T) -> Self {
        Self::one(Into::<Subquery>::into(value))
    }
}

impl From<Function> for TargettablesForSelect {
    fn from(value: Function) -> Self {
        Self::one(value)
    }
}

/// Single field or multiple fields to split by
#[derive(Clone, Debug)]
pub enum Splittables {
    /// single field to split by
    Field(Field),
    /// Multiple fields to split by
    Fields(Vec<Field>),
}

impl From<Splittables> for Vec<Field> {
    fn from(value: Splittables) -> Self {
        match value {
            Splittables::Field(field) => vec![field],
            Splittables::Fields(fields) => fields,
        }
    }
}

impl<const N: usize> From<[Field; N]> for Splittables {
    fn from(value: [Field; N]) -> Self {
        Self::Fields(value.into())
    }
}

impl<const N: usize> From<[&Field; N]> for Splittables {
    fn from(value: [&Field; N]) -> Self {
        Self::Fields(value.map(Into::into).to_vec())
    }
}

impl<const N: usize> From<&[&Field; N]> for Splittables {
    fn from(value: &[&Field; N]) -> Self {
        Self::Fields(value.map(Into::into).to_vec())
    }
}

impl<const N: usize> From<&[Field; N]> for Splittables {
    fn from(value: &[Field; N]) -> Self {
        Self::Fields(value.to_vec())
    }
}

impl From<Vec<Field>> for Splittables {
    fn from(value: Vec<Field>) -> Self {
        Self::Fields(value)
    }
}

impl From<Vec<&Field>> for Splittables {
    fn from(value: Vec<&Field>) -> Self {
        Self::Fields(value.into_iter().map(Into::into).collect())
    }
}

impl From<Vec<ValueLike>> for Splittables {
    fn from(value: Vec<ValueLike>) -> Self {
        Self::Fields(value.into_iter().map(Field::from_expr).collect())
    }
}

type Groupables = Splittables;
pub(crate) type Fetchables = Groupables;
pub(crate) type Omittables = Groupables;
pub(crate) type IndexNames = Groupables;

impl<T: Into<Field>> From<T> for Splittables {
    fn from(value: T) -> Self {
        Self::Field(value.into())
    }
}

/// Items that can be selected
pub struct Selectables(Vec<ValueLike>);

impl Selectables {
    fn fields<'a>(fields: impl IntoIterator<Item = &'a Field>) -> Self {
        Self(fields.into_iter().map(ValueLike::from).collect())
    }
}

impl<T: Into<Field>> From<T> for Selectables {
    fn from(value: T) -> Self {
        Self(vec![ValueLike::from(Into::<Field>::into(value))])
    }
}

impl From<Vec<ValueLike>> for Selectables {
    fn from(value: Vec<ValueLike>) -> Self {
        Self(value)
    }
}

impl From<&All> for Selectables {
    fn from(_: &All) -> Self {
        Self(vec![All.into()])
    }
}

impl From<All> for Selectables {
    fn from(value: All) -> Self {
        Self(vec![value.into()])
    }
}

impl<const N: usize> From<&[Field; N]> for Selectables {
    fn from(value: &[Field; N]) -> Self {
        Self::fields(value)
    }
}

impl<const N: usize> From<&[&Field; N]> for Selectables {
    fn from(value: &[&Field; N]) -> Self {
        Self::fields(value.iter().copied())
    }
}

impl From<Vec<&Field>> for Selectables {
    fn from(value: Vec<&Field>) -> Self {
        Self::fields(value)
    }
}

impl From<Vec<Field>> for Selectables {
    fn from(value: Vec<Field>) -> Self {
        Self(value.into_iter().map(ValueLike::from).collect())
    }
}

impl From<Function> for Selectables {
    fn from(value: Function) -> Self {
        Self(vec![value.into()])
    }
}

impl From<&Function> for Selectables {
    fn from(value: &Function) -> Self {
        Self(vec![value.clone().into()])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SelectionType {
    Select,
    SelectValue,
}

/// Select statement initializer
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatementInit {
    selection_type: SelectionType,
    projections: Vec<ValueLike>,
    targets: Vec<ValueLike>,
    /// Whether every target is a record id.
    record_targets: bool,
    where_: Option<Filter>,
    split: Vec<Field>,
    group_by: Vec<Field>,
    group_all: bool,
    order_by: Vec<Order>,
    order_rand: bool,
    limit: Option<NumberLike>,
    start: Option<NumberLike>,
    fetch: Vec<Field>,
    omitted_fields: Vec<Field>,
    index_hint: Option<IndexHint>,
    version: Option<DatetimeLike>,
    timeout: Option<DurationLike>,
    for_update: bool,
    tempfiles: bool,
}

impl<O: 'static, C: Cardinality> Aliasable for SelectStatement<O, C> {}

impl<O: 'static, C: Cardinality> Conditional for SelectStatement<O, C> {}

/// Creates a SELECT statement.
///
/// The SELECT statement can be used for selecting and querying data in a database.
/// Each SELECT statement supports selecting from multiple targets, which can include
/// tables, records, edges, subqueries, paramaters, s, objects, and other values.
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::{order, select}, functions::{math}, types::RecordId};
/// # let name = Field::new("name");
/// # let age = Field::new("age");
/// # let country = Field::new("country");
/// # let city = Field::new("city");
/// # let ref fake_id = RecordId::new("user", "charlie");
/// # let fake_id2 = RecordId::new("user", "bennett");
///
///  select(All)
///     .from(fake_id)
///     .where_(cond(city.is("Prince Edward Island"))
///                 .and(city.is("NewFoundland"))
///                 .or(city.is("Toronto"))
///     )
///     .order_by(order(&age).numeric())
///     .limit(153)
///     .start(10);
///
///  // Using cond! macro
///  select(All)
///     .from(fake_id)
///     .where_(
///         cond!((city IS "Prince Edward Island") AND (city IS "NewFoundland") OR (city IS "Toronto"))
///     )
///     .order_by(order(&age).numeric())
///     .limit(153)
///     .start(10);
///
///  // You can even mix using cond! macro with the builder operation
///  select(All)
///     .from(fake_id)
///     .where_(
///         cond!((city.is("Prince Edward Island")) AND (city IS "NewFoundland") OR (city IS "Toronto"))
///     );
///
///  select(All)
///     .from(fake_id2)
///     .where_(country.is("INDONESIA"))
///     .order_by(order(&age).numeric())
///     .limit(20)
///     .start(5);
///
///  // Selecting heterogenous types e.g field, alias and all
///  # let user = Table::new("user");
///  # let country = Field::new("country");
///  # let gender = Field::new("gender");
///  # let total = AliasName::new("total");
///  select(arr![count!().__as__(total), math::sum!(age), &gender, &country])
///     .from(user)
///     .group_by(&[gender, country]);
///
///  // Select reference of reference
///  # let user = Table::new("user");
///  # let country = Field::new("country");
///  # let age = Field::new("age");
///  # let gender = Field::new("gender");
///  # let city = Field::new("city");
///  select(&[&gender, &country, &city])
///     .from(user)
///     .group_by(&[gender, country, city]);
///
///  // Select reference of owned types
///  # let user = Table::new("user");
///  # let country = Field::new("country");
///  # let gender = Field::new("gender");
///  # let city = Field::new("city");
///  select(&[gender, country, city])
///     .from(user);
///
///  // Select vector of homogenous fields
///  # let user = Table::new("user");
///  # let country = Field::new("country");
///  # let gender = Field::new("gender");
///  select(vec![gender, country])
///     .from(user);
/// ```
pub fn select(selectables: impl Into<Selectables>) -> SelectStatementInit {
    let selectables: Selectables = selectables.into();

    SelectStatementInit {
        selection_type: SelectionType::Select,
        projections: selectables.0,
        targets: vec![],
        record_targets: false,
        where_: None,
        split: vec![],
        group_by: vec![],
        group_all: false,
        order_by: vec![],
        order_rand: false,
        limit: None,
        start: None,
        fetch: vec![],
        omitted_fields: vec![],
        index_hint: None,
        version: None,
        timeout: None,
        for_update: false,
        tempfiles: false,
    }
}

/// `SELECT VALUE field`: one value per record instead of an object. The result is untyped until
/// [`decode`](SelectStatement::decode) names its type; a model's
/// [`select_value`](crate::SurrealCrud::select_value) types it by the field.
pub fn select_value(selectable_value: impl Into<Field>) -> SelectStatementInit {
    let mut select_statement = select(selectable_value);
    select_statement.selection_type = SelectionType::SelectValue;
    select_statement
}

impl SelectStatementInit {
    /// Specifies the table to select from.
    ///
    /// # Arguments
    ///
    /// * `targettables` - which can include tables, records, edges, subqueries, paramaters, s, objects, and other values.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::{*, statements::{order, select}, types::RecordId};
    /// # let user = Table::new("user");
    /// # let alien = Table::new("alien");
    /// # let user_id1 = RecordId::new("user", "charlie");
    ///  //  Can select from a table name
    ///  select(All).from(user);
    ///
    ///  //  Can also select from an id
    ///  select(All).from(user_id1);
    ///
    ///  //  or select fromj a subquery
    ///  select(All).from(select(All).from(alien));
    ///
    /// ```
    pub fn from(self, targettables: impl Into<TargettablesForSelect>) -> SelectStatement {
        self.with_targets(targettables)
    }

    /// `FROM ONLY`: one value instead of a list, so the result is an `Option`. A table target
    /// fails unless the statement has `LIMIT 1`, since it could return several records.
    #[expect(
        clippy::wrong_self_convention,
        reason = "a builder step named after SurrealQL's `FROM ONLY`, not a conversion"
    )]
    pub fn from_only(
        self,
        targettables: impl Into<TargettablesForSelect>,
    ) -> SelectStatement<Value, One> {
        self.with_targets(targettables)
    }

    fn with_targets<C: Cardinality>(
        mut self,
        targettables: impl Into<TargettablesForSelect>,
    ) -> SelectStatement<Value, C> {
        let targettables = targettables.into();
        self.record_targets = targettables.records;
        self.targets = targettables.targets;
        SelectStatement::new(self)
    }

    /// Omits fields from the result, including nested and destructured paths such as
    /// `opts.{ security, enabled }`. Omitting every field of a nested object leaves `{}`.
    pub fn omit(mut self, fetchables: impl Into<Omittables>) -> Self {
        self.omitted_fields
            .extend(Vec::<Field>::from(fetchables.into()));
        self
    }

    /// Writes the statement; `only` is its cardinality's `FROM ONLY`.
    fn render_select(&self, only: bool, out: &mut Renderer) {
        out.push_str(match self.selection_type {
            SelectionType::Select => "SELECT ",
            SelectionType::SelectValue => "SELECT VALUE ",
        });
        out.push(&self.projections).push_str(" ");

        if !self.omitted_fields.is_empty() {
            out.push_str("OMIT ")
                .push(&self.omitted_fields)
                .push_str(" ");
        }

        out.push_str("FROM ");
        if only {
            out.push_str("ONLY ");
        }
        out.push(&self.targets).push(&self.index_hint);

        if let Some(condition) = &self.where_ {
            out.push_str(" WHERE ").push(condition);
        }

        let grouped = !self.group_by.is_empty() || self.group_all;
        if !self.split.is_empty() {
            if grouped {
                out.push_error(
                    "SPLIT and GROUP cannot be combined in one SELECT; split or group in a subquery",
                );
            }
            out.push_str(" SPLIT ").push(&self.split);
        }

        if !self.group_by.is_empty() {
            out.push_str(" GROUP BY ").push(&self.group_by);
        } else if self.group_all {
            out.push_str(" GROUP ALL");
        }

        if self.order_rand {
            if !self.order_by.is_empty() {
                out.push_error("ORDER BY RAND() cannot be combined with ordering by fields");
            }
            out.push_str(" ORDER BY RAND()");
        } else if !self.order_by.is_empty() {
            out.push_str(" ORDER BY ").push(&self.order_by);
        }

        if let Some(limit) = &self.limit {
            out.push_str(" LIMIT ").push(limit);
        }

        if let Some(start) = &self.start {
            out.push_str(" START AT ").push(start);
        }

        if !self.fetch.is_empty() {
            out.push_str(" FETCH ").push(&self.fetch);
        }

        if let Some(version) = &self.version {
            out.push_str(" VERSION ").push(version);
        }

        if let Some(timeout) = &self.timeout {
            out.push_str(" TIMEOUT ").push_inlined(timeout);
        }

        if self.for_update {
            self.push_for_update_errors(grouped, out);
            out.push_str(" FOR UPDATE");
        }

        if self.tempfiles {
            out.push_str(" TEMPFILES");
        }
    }

    /// The combinations SurrealDB documents `FOR UPDATE` as refusing.
    fn push_for_update_errors(&self, grouped: bool, out: &mut Renderer) {
        if !self.record_targets {
            out.push_error("FOR UPDATE needs record id targets, not tables, ranges or subqueries");
        }
        if self.version.is_some() {
            out.push_error("FOR UPDATE cannot be combined with VERSION");
        }
        if grouped {
            out.push_error("FOR UPDATE cannot be combined with GROUP");
        }
        if !self.split.is_empty() {
            out.push_error("FOR UPDATE cannot be combined with SPLIT");
        }
        if self.limit.is_some() && self.targets.len() > 1 {
            out.push_error("FOR UPDATE cannot be combined with LIMIT over several targets");
        }
    }
}

/// A `SELECT` statement. `O` is the type each result decodes into and `C` whether it is written
/// with `ONLY`; both decide the type [`Returning::execute`] returns: `Vec<O>`, or `Option<O>`
/// with `ONLY`.
///
/// What `O` guarantees depends on how it was chosen. A model's
/// [`select`](crate::SurrealCrud::select) and [`select_value`](crate::SurrealCrud::select_value)
/// type it from the model and its field. [`decode`](Self::decode) names it, which types the
/// decoding but checks nothing about the selected shape: a projection, `FETCH`, `OMIT`, `GROUP`
/// or `SPLIT` that does not match fails with [`Error::Conversion`](crate::Error::Conversion)
/// when the result is decoded. Those clauses reset `O` to [`Value`] for that reason, so the
/// shape they produce is always named by the caller.
pub struct SelectStatement<O = Value, C = Many> {
    select: SelectStatementInit,
    output: PhantomData<fn() -> (O, C)>,
}

impl<O, C> Clone for SelectStatement<O, C> {
    fn clone(&self) -> Self {
        Self {
            select: self.select.clone(),
            output: PhantomData,
        }
    }
}

impl<O, C> fmt::Debug for SelectStatement<O, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SelectStatement")
            .field(&self.select)
            .finish()
    }
}

impl<O, C> PartialEq for SelectStatement<O, C> {
    fn eq(&self, other: &Self) -> bool {
        self.select == other.select
    }
}

impl<O, C: Cardinality> SelectStatement<O, C> {
    fn new(select: SelectStatementInit) -> Self {
        Self {
            select,
            output: PhantomData,
        }
    }

    fn retype<P>(self) -> SelectStatement<P, C> {
        SelectStatement::new(self.select)
    }

    /// Names the type each result decodes into. It types the decoding only: a result that does
    /// not match `P` fails with [`Error::Conversion`](crate::Error::Conversion) when it is
    /// executed.
    pub fn decode<P: SurrealValue>(self) -> SelectStatement<P, C> {
        self.retype()
    }
    /// Adds a condition to the `` clause of the SQL query.
    ///
    /// # Arguments
    ///
    /// * `condition` - A reference to a filter condition.
    ///
    /// # Example
    ///
    /// Examples
    /// ```rust
    /// # use umbriel::{*, statements::{order, select}, types::RecordId};
    /// # let name = Field::new("name");
    /// # let age = Field::new("age");
    /// # let country = Field::new("country");
    /// # let city = Field::new("city");
    /// # let fake_id = RecordId::new("user", "charlie");
    /// # let fake_id2 = RecordId::new("user", "bennett");
    /// // Supports simpler where clause without `cond` helper function
    /// # let select2 = select(All)
    /// #   .from(fake_id)
    ///     .where_(country.is("INDONESIA"));
    ///
    /// // Supports more complex where clause using `cond` helper function
    /// # let select1 = select(All).from(fake_id2)
    ///                     .where_(cond(city.is("Prince Edward Island"))
    ///                     .and(city.is("NewFoundland"))
    ///                     .or(city.is("Toronto"))
    ///                 );
    pub fn where_(mut self, condition: impl Conditional + Clone) -> Self {
        self.select.where_ = Some(Filter::new(condition));
        self
    }

    /// `WITH NOINDEX`: the planner reads through the table iterator and uses no index.
    pub fn with_no_index(mut self) -> Self {
        self.select.index_hint = Some(IndexHint::NoIndex);
        self
    }

    /// `WITH INDEX …`: the planner may use only the named indexes.
    pub fn with_index(mut self, indexes: impl Into<IndexNames>) -> Self {
        self.select.index_hint = Some(IndexHint::indexes(indexes));
        self
    }

    /// Adds a field or multiple fields to the `SPLIT BY` clause of the query.
    /// As SurrealDB supports s and nested fields within arrays,
    /// it is possible to split the result on a specific field name,
    /// returning each value in an  as a separate value, along with the record content itself.
    /// This is useful in data analysis contexts.
    ///
    /// # Arguments
    ///
    /// * `splittables` - The name of the field or  or vector of fields to add to the `SPLIT BY` clause.
    ///
    /// # Examples: For multiple fields
    ///
    /// ```rust
    /// # use umbriel::{*, statements::{order, select}};
    /// # let user = Table::new("user");
    /// # let country = Table::new("country");
    /// # let emails = Field::new("emails");
    /// # let cities = Field::new("cities");
    // // Split the results by each value in an
    ///  select(All)
    ///     .from(user)
    ///     .split(emails);
    ///
    /// ```
    /// ```rust, ignore
    /// // Split the results by each value in a nested
    ///  let locations = Country::schema();
    ///  select(All)
    ///     .from(country)
    ///     .split(locations.cities);
    /// ```
    ///
    /// Splitting changes each result's shape, so the result is untyped until
    /// [`decode`](Self::decode) names it.
    pub fn split(mut self, splittables: impl Into<Splittables>) -> SelectStatement<Value, C> {
        self.select
            .split
            .extend(Vec::<Field>::from(splittables.into()));
        self.retype()
    }

    /// Sets the GROUP BY clause for the query.
    /// SurrealDB supports data aggregation and grouping, with support for multiple fields, nested fields, and aggregate functions.
    /// In SurrealDB, every field which appears in the field projections of the select statement
    /// (and which is not an aggregate function), must also be present in the GROUP BY clause.
    ///
    /// # Arguments
    ///
    /// * `field(s)` - The name or names of the field to group by.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::{*, statements::{order, select}, functions::{count, math}};
    /// # let user = Table::new("user");
    /// # let country = Field::new("country");
    ///
    ///  // Group records by a single field
    ///  select(All)
    ///     .from(user)
    ///     .group_by(country);
    ///
    /// // Group results by list borrowed list of referenced fields
    /// # let user = Table::new("user");
    /// # let country = Field::new("country");
    /// # let age = Field::new("age");
    /// # let gender = Field::new("gender");
    /// # let city = Field::new("city");
    ///  select(All)
    ///     .from(user)
    ///     .group_by(&[&gender, &country, &city]);
    ///
    /// // Group results by list borrowed list of owned fields
    /// # let user = Table::new("user");
    ///  select(All)
    ///     .from(user)
    ///     .group_by(&[gender, country, city]);
    ///
    /// # let user = Table::new("user");
    /// # let country = Field::new("country");
    /// # let gender = Field::new("gender");
    /// # let total = AliasName::new("total");
    /// // Group results with aggregate functions
    ///  select(All)
    ///     .from(user)
    ///     .group_by(vec![gender, country]);
    ///
    /// // Group results by chaining
    /// # let user = Table::new("user");
    /// # let country = Field::new("country");
    /// # let gender = Field::new("gender");
    /// select(All)
    ///     .from(user)
    ///     .group_by(gender)
    ///     .group_by(country);
    /// ```
    /// ```rust, ignore
    /// // Group results by a nested field
    /// let settings = Article::schem();
    /// select(settings.published)
    ///     .from(article)
    ///     .group_by(settings.published);
    /// ```
    ///
    /// Grouping changes each result's shape, so the result is untyped until
    /// [`decode`](Self::decode) names it.
    pub fn group_by(mut self, groupables: impl Into<Groupables>) -> SelectStatement<Value, C> {
        self.select
            .group_by
            .extend(Vec::<Field>::from(groupables.into()));
        self.retype()
    }

    /// Groups all records in the query result into a single group. The result is untyped until
    /// [`decode`](Self::decode) names it; an empty table gives no group at all, not an empty one.
    pub fn group_all(mut self) -> SelectStatement<Value, C> {
        self.select.group_all = true;
        self.retype()
    }

    /// Sets the ORDER BY clause for the query. Multiple values can also be set within same call.
    /// Repeated calls are accumulated
    ///
    /// To sort records, SurrealDB allows ordering on multiple fields and nested fields.
    /// Use the ORDER BY clause to specify a comma-separated list of field names which
    /// should be used to order the resulting records. The ASC and DESC keywords can be
    /// used to specify whether results should be sorted in an ascending or descending manner.
    /// The COLLATE keyword can be used to use unicode collation when ordering text in string values,
    /// ensuring that different cases, and different languages are sorted in a consistent manner.
    /// Finally the NUMERIC can be used to correctly sort text which contains numeric values.
    ///
    /// # Arguments
    ///
    /// * `orderables` - The field(s) and direction to order by.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::{*, statements::{order, select}};
    /// # let user = Table::new("user");
    /// # let age = Field::new("age");
    /// # let country = Field::new("country");
    /// # let city = Field::new("city");
    /// # let state = Field::new("state");
    /// // Order by single field
    /// select(All)
    ///     .from(user)
    ///     .order_by(order(age).numeric().desc());
    ///
    /// # let user = Table::new("user");
    /// # let age = Field::new("age");
    /// // Order by multiple fields by using a list. Vector and `!` helper also work
    /// select(All)
    ///     .from(user)
    ///     .order_by(&[order(age).numeric().desc(), order(city).asc()]);
    ///
    /// # let user = Table::new("user");
    /// # let age = Field::new("age");
    /// // Order by multiple fields by chainging to accumulate
    /// select(All)
    ///     .from(user)
    ///     .order_by(order(age).numeric().desc())
    ///     .order_by(order(state).collate().asc());
    /// ```
    ///
    /// The fields need not be projected: since SurrealDB 3.3.0 the full record is sorted before
    /// the projection runs, and ties keep their order. Only field paths are accepted; to order by
    /// a computed value, project it with an alias, order by the alias and `omit` it.
    pub fn order_by(mut self, orderables: impl Into<Orderables>) -> Self {
        self.select
            .order_by
            .extend(Vec::<Order>::from(orderables.into()));
        self
    }

    /// Orders the results randomly: `ORDER BY RAND()`. Random order is the whole clause, so it
    /// cannot be combined with [`order_by`](Self::order_by).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::{*, statements::select};
    /// let query = select(All).from(Table::new("user")).order_rand().limit(3);
    /// assert_eq!(query.to_debug_sql(), "SELECT * FROM user ORDER BY RAND() LIMIT 3;");
    /// ```
    pub fn order_rand(mut self) -> Self {
        self.select.order_rand = true;
        self
    }

    /// Sets the LIMIT clause for the query.
    /// To limit the number of records returned, use the LIMIT clause.
    ///
    /// When using the LIMIT clause, it is possible to paginate results by using the START clause to start from a specific record from the result set.
    ///
    /// # Arguments
    ///
    /// * `limit` - The maximum number of rows to return.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::{*, statements::select};
    /// # let user = Table::new("user");
    /// select(All)
    ///     .from(user)
    ///     .limit(100);
    ///
    /// // When using the LIMIT clause, it is possible to paginate results by using the START clause to start from a specific record from the result set.
    /// # let user = Table::new("user");
    /// select(All)
    ///     .from(user)
    ///     .start(50)
    ///     .limit(50);
    /// ```
    pub fn limit(mut self, limit: impl Into<NumberLike>) -> Self {
        self.select.limit = Some(limit.into());
        self
    }

    /// Adds a start offset to the current query.
    ///
    /// # Arguments
    ///
    /// * `start` - An unsigned 64-bit integer representing the starting offset.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::{*, statements::select};
    /// # let user = Table::new("user");
    ///
    /// // When using the LIMIT clause, it is possible to paginate results by using the START clause to start from a specific record from the result set.
    /// select(All)
    ///     .from(user)
    ///     .start(50)
    ///     .limit(50);
    /// ```
    pub fn start(mut self, start: impl Into<NumberLike>) -> Self {
        self.select.start = Some(start.into());
        self
    }

    /// Adds a field or many fields to the list of fields to fetch in the current query.
    /// You can add as list in a single `fetch` call or chain to accumulate fields to fetch.
    ///
    /// One of the most powerful functions in SurrealDB is the related records and graph connections.
    /// Instead of pulling data from multiple tables and merging that data together,
    /// SurrealDB allows you to traverse related records efficiently without needing to use JOINs.
    /// To fetch and replace records with the remote record data, use the FETCH clause to specify the fields
    /// and nested fields which should be fetched in-place, and returned in the final statement response output.
    /// # Arguments
    ///
    /// * `fetchables` - A reference to a field/fields to be fetched in the query.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::{*, statements::select};
    /// # let user = Table::new("user");
    /// # let account = Field::new("account");
    /// # let friend = Field::new("friend");
    ///
    /// // Fetch single field
    /// select(All)
    ///     .from(user)
    ///     .fetch(account);
    ///
    /// // Fetch multiple field using a list
    /// # let user = Table::new("user");
    /// # let account = Field::new("account");
    /// select(All)
    ///     .from(user)
    ///     .fetch(&[account, friend]);
    ///
    /// // Fetch multiple field by chaining fetch method calls
    /// # let user = Table::new("user");
    /// # let account = Field::new("account");
    /// # let friend = Field::new("friend");
    /// select(All)
    ///     .from(user)
    ///     .fetch(account)
    ///     .fetch(friend);
    /// ```
    /// ```rust, ignore
    /// let account = Person::schema().account;
    /// select(All)
    ///     .from(user)
    ///     // Fetch nested field
    ///     .fetch(&[account, account.users]);
    /// ```
    ///
    /// Fetching replaces links with records, so the result is untyped until
    /// [`decode`](Self::decode) names it. A model's own links decode either way; for those,
    /// [`load_links`](Self::load_links) keeps the model type.
    pub fn fetch(mut self, fetchables: impl Into<Fetchables>) -> SelectStatement<Value, C> {
        self.select
            .fetch
            .extend(Vec::<Field>::from(fetchables.into()));
        self.retype()
    }

    /// `VERSION datetime`: reads the records as they were at that time.
    ///
    /// It needs a versioned store, such as SurrealKV opened with `versioned`; the in-memory engine
    /// refuses it since SurrealDB 3.3.0. A time before a record existed gives no record.
    pub fn version(mut self, datetime: impl Into<DatetimeLike>) -> Self {
        self.select.version = Some(datetime.into());
        self
    }

    /// Sets the timeout duration for the query.
    ///
    /// # Arguments
    ///
    /// * `duration` - a value that can represent a duration for the timeout: a duration, a
    ///   `Field` or a `Param`. It is written as a literal.
    pub fn timeout(mut self, duration: impl Into<DurationLike>) -> Self {
        self.select.timeout = Some(duration.into());
        self
    }

    /// `FOR UPDATE` (SurrealDB 3.3.0): registers the selected records with the transaction, so
    /// its `COMMIT` fails with a conflict if another transaction changed one of them first.
    ///
    /// It takes no lock, and no transaction waits on another; a conflict is retryable. It applies
    /// to record id targets only, and fails when rendered with a table, range or subquery target,
    /// with `VERSION`, `GROUP` or `SPLIT`, or with `LIMIT` over several targets. A read-only
    /// transaction rejects it.
    pub fn for_update(mut self) -> Self {
        self.select.for_update = true;
        self
    }

    /// `TEMPFILES`: processes the statement in temporary files instead of memory, trading speed
    /// for memory use.
    ///
    /// It takes effect only when the database has a temporary directory, such as the SDK's
    /// `Config::temporary_directory`; without one the statement runs in memory and nothing
    /// reports it.
    pub fn tempfiles(mut self) -> Self {
        self.select.tempfiles = true;
        self
    }
}

impl<O: 'static, C: Cardinality> SelectStatement<O, C> {
    /// `EXPLAIN`: returns the query plan instead of the records.
    pub fn explain(self) -> Explained {
        Explained::new(self, false)
    }

    /// `EXPLAIN FULL`: runs the query and returns the plan with each step's metrics, such as the
    /// number of records it processed.
    pub fn explain_full(self) -> Explained {
        Explained::new(self, true)
    }
}

impl<T: Model, C: Cardinality> SelectStatement<T, C> {
    /// Selects every field with the given record links fetched (`SELECT *, link.*`). A link to a
    /// missing record becomes `NULL`. The result still decodes as the model, whose links hold
    /// either an id or a record.
    ///
    /// # Errors
    ///
    /// [`Error::FieldsUnfetchableNotARecordLink`](crate::Error::FieldsUnfetchableNotARecordLink)
    /// when a field is not one of the model's links.
    pub fn load_links(
        mut self,
        linked_fields: impl IntoIterator<Item = impl Into<Field>>,
    ) -> Result<Self> {
        let Projections(projections) =
            fetch_projections::<T>(linked_fields.into_iter().map(Into::into).collect())?;
        self.select.projections = projections;
        Ok(self)
    }

    /// Selects every field with every record link fetched.
    pub fn load_all_links(mut self) -> Self {
        let Projections(projections) = link_projections(T::get_linked_fields());
        self.select.projections = projections;
        self
    }
}

impl<O, C: Cardinality> Statement for SelectStatement<O, C> {
    fn render_body(&self, out: &mut Renderer) {
        self.select.render_select(C::ONLY, out);
    }
}

impl_statement!([O: 'static, C: Cardinality] SelectStatement<O, C>);

impl<O: SurrealValue + 'static, C: Cardinality> Returning for SelectStatement<O, C> {
    type Mode = Rows<O>;
    type Cardinality = C;
}

/// Counts records: `SELECT VALUE count FROM (SELECT count(…) FROM table GROUP ALL)`.
#[derive(Debug, Clone)]
pub struct SelectCount(SelectStatement<u64>);

impl SelectCount {
    pub(crate) fn new(statement: SelectStatement) -> Self {
        Self(statement.decode())
    }

    /// Runs the count. A `GROUP ALL` over no records returns no group rather than a zero count,
    /// so that reads as 0.
    ///
    /// # Errors
    ///
    /// Fails as [`Returning::execute`] does.
    pub async fn execute(&self, db: &Surreal<impl Connection>) -> Result<u64> {
        Ok(self.0.execute(db).await?.first().copied().unwrap_or(0))
    }
}

impl Statement for SelectCount {
    fn render_body(&self, out: &mut Renderer) {
        self.0.render_body(out);
    }
}

impl_statement!(SelectCount);
