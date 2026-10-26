/*
 * Syntax from specs:https://surrealdb.com/docs/surrealql/statements/select
 * SELECT @projections
    FROM @targets
    [  @condition ]
    [ SPLIT [ AT ] @field ... ]
    [ GROUP [ BY ] @field ... ]
    [ ORDER [ BY ]
        RAND()
        | @field [ COLLATE ] [ NUMERIC ] [ ASC | DESC ] ...
    ]
    [ LIMIT [ BY ] @limit ]
    [ START [ AT ] @start ]
    [ FETCH @field ... ]
    [ TIMEOUT @duration ]
; */
use std::marker::PhantomData;

use surrealdb::{
    Connection, Surreal,
    types::{self, SurrealValue, Table},
};

use crate::{
    Aliasable, All, Conditional, DurationLike, Field, Filter, Function, IndexName, Model,
    NumberLike, Projections, RecordId, RecordRange, Render, Renderer, Result, ReturnType,
    ReturnableSelect, ReturnableStandard, ValueLike,
};

use super::{Statement, Subquery, impl_statement};

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
pub struct TargettablesForSelect(Vec<ValueLike>);

impl TargettablesForSelect {
    fn one(node: impl Into<ValueLike>) -> Self {
        Self(vec![node.into()])
    }

    fn tables<'a>(tables: impl IntoIterator<Item = &'a Table>) -> Self {
        Self(tables.into_iter().cloned().map(ValueLike::table).collect())
    }

    fn values<V: SurrealValue>(values: impl IntoIterator<Item = V>) -> Self {
        Self(values.into_iter().map(ValueLike::value).collect())
    }
}

impl From<Table> for TargettablesForSelect {
    fn from(value: Table) -> Self {
        Self::one(ValueLike::table(value))
    }
}

impl From<&Table> for TargettablesForSelect {
    fn from(value: &Table) -> Self {
        Self::tables([value])
    }
}

impl From<Vec<Table>> for TargettablesForSelect {
    fn from(value: Vec<Table>) -> Self {
        Self(value.into_iter().map(ValueLike::table).collect())
    }
}

impl From<Vec<&Table>> for TargettablesForSelect {
    fn from(value: Vec<&Table>) -> Self {
        Self::tables(value)
    }
}

impl<const N: usize> From<&[Table; N]> for TargettablesForSelect {
    fn from(value: &[Table; N]) -> Self {
        Self::tables(value)
    }
}

impl<const N: usize> From<&[&Table; N]> for TargettablesForSelect {
    fn from(value: &[&Table; N]) -> Self {
        Self::tables(value.iter().copied())
    }
}

impl From<types::RecordId> for TargettablesForSelect {
    fn from(value: types::RecordId) -> Self {
        Self::values([value])
    }
}

impl From<&types::RecordId> for TargettablesForSelect {
    fn from(value: &types::RecordId) -> Self {
        Self::values([value.clone()])
    }
}

impl From<Vec<types::RecordId>> for TargettablesForSelect {
    fn from(value: Vec<types::RecordId>) -> Self {
        Self::values(value)
    }
}

impl<const N: usize> From<&[types::RecordId; N]> for TargettablesForSelect {
    fn from(value: &[types::RecordId; N]) -> Self {
        Self::values(value.iter().cloned())
    }
}

impl<T: Model> From<RecordId<T>> for TargettablesForSelect {
    fn from(value: RecordId<T>) -> Self {
        Self::values([value])
    }
}

impl<T: Model> From<&RecordId<T>> for TargettablesForSelect {
    fn from(value: &RecordId<T>) -> Self {
        Self::values([value.clone()])
    }
}

impl<T: Model> From<Vec<RecordId<T>>> for TargettablesForSelect {
    fn from(value: Vec<RecordId<T>>) -> Self {
        Self::values(value)
    }
}

impl<T: Model> From<Vec<&RecordId<T>>> for TargettablesForSelect {
    fn from(value: Vec<&RecordId<T>>) -> Self {
        Self::values(value.into_iter().cloned())
    }
}

impl<T: Model> From<&[RecordId<T>]> for TargettablesForSelect {
    fn from(value: &[RecordId<T>]) -> Self {
        Self::values(value.iter().cloned())
    }
}

impl<T: Model> From<&[&RecordId<T>]> for TargettablesForSelect {
    fn from(value: &[&RecordId<T>]) -> Self {
        Self::values(value.iter().map(|&id| id.clone()))
    }
}

impl<const N: usize, T: Model> From<&[RecordId<T>; N]> for TargettablesForSelect {
    fn from(value: &[RecordId<T>; N]) -> Self {
        Self::values(value.iter().cloned())
    }
}

impl<const N: usize, T: Model> From<&[&RecordId<T>; N]> for TargettablesForSelect {
    fn from(value: &[&RecordId<T>; N]) -> Self {
        Self::values(value.iter().map(|&id| id.clone()))
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

#[derive(Debug, Clone, PartialEq)]
enum WithIndexType {
    NoIndex,
    Indexes(Vec<IndexName>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ExplainMode {
    Simple,
    Full,
}

/// Select statement initializer
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatementInit {
    selection_type: SelectionType,
    projections: Vec<ValueLike>,
    targets: Vec<ValueLike>,
    only: bool,
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
    with_index_type: Option<WithIndexType>,
    timeout: Option<DurationLike>,
    explain_mode: Option<ExplainMode>,
}

impl Aliasable for SelectStatement {}

impl ReturnableSelect for SelectStatement {}

impl Conditional for SelectStatement {}

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
        only: false,
        where_: None,
        split: vec![],
        group_by: vec![],
        group_all: false,
        order_by: vec![],
        order_rand: false,
        limit: None,
        with_index_type: None,
        explain_mode: None,
        start: None,
        fetch: vec![],
        omitted_fields: vec![],
        timeout: None,
    }
}

/// Just like normal select statement but useful for selecting a single value out of the returned object.
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

    /// Same as normal from but only selects from a single table
    #[expect(
        clippy::wrong_self_convention,
        reason = "a builder step named after SurrealQL's `FROM ONLY`, not a conversion"
    )]
    pub fn from_only(mut self, targettables: impl Into<TargettablesForSelect>) -> SelectStatement {
        self.only = true;
        self.with_targets(targettables)
    }

    fn with_targets(mut self, targettables: impl Into<TargettablesForSelect>) -> SelectStatement {
        self.targets.extend(targettables.into().0);
        SelectStatement(self)
    }

    /// Omits a field or multiple fields from the query return values.
    pub fn omit(mut self, fetchables: impl Into<Omittables>) -> Self {
        self.omitted_fields
            .extend(Vec::<Field>::from(fetchables.into()));
        self
    }
}

/// The query builder struct used to construct complex database queries.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement(SelectStatementInit);

impl From<SelectStatementInit> for SelectStatement {
    fn from(value: SelectStatementInit) -> Self {
        Self(value)
    }
}

impl SelectStatement {
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
        self.0.where_ = Some(Filter::new(condition));
        self
    }

    /// Makes sure no index is used for the query.
    pub fn with_no_index(mut self) -> Self {
        self.0.with_index_type = Some(WithIndexType::NoIndex);
        self
    }

    /// Adds an index or multiple indexes to the `WITH INDEX` clause of the query.
    pub fn with_index(mut self, indexes: impl Into<IndexNames>) -> Self {
        self.0.with_index_type = Some(WithIndexType::Indexes(indexes.into().into()));
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
    pub fn split(mut self, splittables: impl Into<Splittables>) -> Self {
        self.0.split.extend(Vec::<Field>::from(splittables.into()));
        self
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
    pub fn group_by(mut self, groupables: impl Into<Groupables>) -> Self {
        self.0
            .group_by
            .extend(Vec::<Field>::from(groupables.into()));
        self
    }

    /// Groups all records in the query result into a single group.
    pub fn group_all(mut self) -> Self {
        self.0.group_all = true;
        self
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
    pub fn order_by(mut self, orderables: impl Into<Orderables>) -> Self {
        self.0
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
        self.0.order_rand = true;
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
        self.0.limit = Some(limit.into());
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
        self.0.start = Some(start.into());
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
    pub fn fetch(mut self, fetchables: impl Into<Fetchables>) -> Self {
        self.0.fetch.extend(Vec::<Field>::from(fetchables.into()));
        self
    }

    /// Sets the timeout duration for the query.
    ///
    /// # Arguments
    ///
    /// * `duration` - a value that can represent a duration for the timeout: a duration, a
    ///   `Field` or a `Param`. It is written as a literal.
    pub fn timeout(mut self, duration: impl Into<DurationLike>) -> Self {
        self.0.timeout = Some(duration.into());
        self
    }

    /// Indicates that the query should be explained.
    pub fn explain(mut self) -> Self {
        self.0.explain_mode = Some(ExplainMode::Simple);
        self
    }

    /// Indicates that the query should be explained in full.
    pub fn explain_full(mut self) -> Self {
        self.0.explain_mode = Some(ExplainMode::Full);
        self
    }
}

impl Statement for SelectStatement {
    fn render_body(&self, out: &mut Renderer) {
        let statement = &self.0;
        out.push_str(match statement.selection_type {
            SelectionType::Select => "SELECT ",
            SelectionType::SelectValue => "SELECT VALUE ",
        });
        out.push(&statement.projections).push_str(" ");

        if !statement.omitted_fields.is_empty() {
            out.push_str("OMIT ")
                .push(&statement.omitted_fields)
                .push_str(" ");
        }

        out.push_str("FROM ");
        if statement.only {
            out.push_str("ONLY ");
        }
        out.push(&statement.targets);

        match &statement.with_index_type {
            Some(WithIndexType::NoIndex) => {
                out.push_str(" WITH NOINDEX");
            }
            Some(WithIndexType::Indexes(indexes)) => {
                out.push_str(" WITH INDEX ").push(indexes);
            }
            None => {}
        }

        if let Some(condition) = &statement.where_ {
            out.push_str(" WHERE ").push(condition);
        }

        let grouped = !statement.group_by.is_empty() || statement.group_all;
        if !statement.split.is_empty() {
            if grouped {
                out.push_error(
                    "SPLIT and GROUP cannot be combined in one SELECT; split or group in a subquery",
                );
            }
            out.push_str(" SPLIT ").push(&statement.split);
        }

        if !statement.group_by.is_empty() {
            out.push_str(" GROUP BY ").push(&statement.group_by);
        } else if statement.group_all {
            out.push_str(" GROUP ALL");
        }

        if statement.order_rand {
            if !statement.order_by.is_empty() {
                out.push_error("ORDER BY RAND() cannot be combined with ordering by fields");
            }
            out.push_str(" ORDER BY RAND()");
        } else if !statement.order_by.is_empty() {
            out.push_str(" ORDER BY ").push(&statement.order_by);
        }

        if let Some(limit) = &statement.limit {
            out.push_str(" LIMIT ").push(limit);
        }

        if let Some(start) = &statement.start {
            out.push_str(" START AT ").push(start);
        }

        if !statement.fetch.is_empty() {
            out.push_str(" FETCH ").push(&statement.fetch);
        }

        if let Some(timeout) = &statement.timeout {
            out.push_str(" TIMEOUT ").push_inlined(timeout);
        }

        match statement.explain_mode {
            Some(ExplainMode::Simple) => {
                out.push_str(" EXPLAIN");
            }
            Some(ExplainMode::Full) => {
                out.push_str(" EXPLAIN FULL");
            }
            None => {}
        }
    }
}

impl_statement!(SelectStatement);

/// A mini version of the select statement used as Model convenience method for building select statements.
#[derive(Debug, Clone)]
pub struct SelectStatementMini<T: Model>(SelectStatement, PhantomData<fn() -> T>);

impl<T: Model> SelectStatementMini<T> {
    /// Order the results by the given fields
    pub fn order_by(mut self, orderables: impl Into<Orderables>) -> Self {
        self.0 = self.0.order_by(orderables);
        self
    }

    /// Starts the result at the offset
    pub fn start(mut self, start: impl Into<NumberLike>) -> Self {
        self.0 = self.0.start(start);
        self
    }

    /// Limits the number of results returned
    pub fn limit(mut self, limit: impl Into<NumberLike>) -> Self {
        self.0 = self.0.limit(limit);
        self
    }

    /// Orders the results randomly: `ORDER BY RAND()`.
    pub fn order_rand(mut self) -> Self {
        self.0 = self.0.order_rand();
        self
    }
}

impl<T: Model> From<SelectStatement> for SelectStatementMini<T> {
    fn from(value: SelectStatement) -> Self {
        Self(value, PhantomData)
    }
}

impl<T: Model> Statement for SelectStatementMini<T> {
    fn render_body(&self, out: &mut Renderer) {
        self.0.render_body(out);
    }
}

impl_statement!([T: Model] SelectStatementMini<T>);

impl<T> ReturnableStandard<T> for SelectStatementMini<T>
where
    T: Model + Send + Sync,
{
    fn set_return_type(mut self, return_type: ReturnType) -> Self {
        if let ReturnType::Projections(Projections(projections)) = return_type {
            self.0.0.projections.extend(projections);
        }
        self
    }
}

/// A `SELECT VALUE count …` statement returning a single number.
#[derive(Debug, Clone)]
pub struct SelectStatementCount(SelectStatement);

impl SelectStatementCount {
    /// Gets count of the records that would be returned by the select statement.
    /// Defaults to zero if there is no result.
    pub async fn get(&self, db: Surreal<impl Connection>) -> Result<usize> {
        Ok(self.0.return_one(db).await?.unwrap_or_default())
    }
}

impl Statement for SelectStatementCount {
    fn render_body(&self, out: &mut Renderer) {
        self.0.render_body(out);
    }
}

impl_statement!(SelectStatementCount);

impl ReturnableSelect for SelectStatementCount {}

impl From<SelectStatement> for SelectStatementCount {
    fn from(value: SelectStatement) -> Self {
        Self(value)
    }
}
