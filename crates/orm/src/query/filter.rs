use std::{borrow::Cow, sync::Arc};

use crate::{Conditional, Render, Renderer, ValueLike};

use crate::Empty;

/// Creates a new filter from a given `filterable` input.
///
/// # Arguments
///
/// * `filterable` - A value that can be converted into a `Filter`.
///
/// # Example
///
/// ```rust
/// # use umbriel::*;
/// # let age = Field::new("age");
/// # let name = Field::new("name");
/// # let title = Field::new("title");
///
/// let filter = cond(age.greater_than(18))
///                 .and(name.contains("Charlie"))
///                 .or(title.equal("Professor"));
/// ```
pub fn cond(filterable: impl Conditional) -> Filter {
    Filter::new(filterable)
}

/// Creates a new filter from a given `filterable` input.
/// This is a macro version of the `cond` function.
/// It is useful for creating filters.
/// It is also useful for creating compound conditions.
///
/// # Arguments
/// * `filterable` - A value that can be converted into a `Filter`.
///
/// # Example
///
/// ```rust
/// # use umbriel::*;
/// # let age = Field::new("age");
/// # let name = Field::new("name");
/// # let title = Field::new("title");
///
/// let filter = cond!((age > 18) AND (name = "Charlie") OR (title == "Professor"));
/// let filter_simple = cond!(age > 18);
/// let filter_mixed = cond!((age.or(4).or(545).or(232)) OR (title = "Professor") AND (age < 100));
/// ```
#[macro_export]
macro_rules! cond {
    // Base patterns
    ($field:ident IS $value:expr) => {
        $field.is($value)
    };
    ($field:ident is $value:expr) => {
        $field.is($value)
    };
    ($field:ident = $value:expr) => {
        $field.equal($value)
    };
    ($field:ident != $value:expr) => {
        $field.not_equal($value)
    };
    ($field:ident == $value:expr) => {
        $field.exactly_equal($value)
    };
    ($field:ident ?= $value:expr) => {
        $field.any_equal($value)
    };
    ($field:ident *= $value:expr) => {
        $field.all_equal($value)
    };
    ($field:ident > $value:expr) => {
        $field.greater_than($value)
    };
    ($field:ident >= $value:expr) => {
        $field.greater_than_or_equal($value)
    };
    ($field:ident < $value:expr) => {
        $field.less_than($value)
    };
    ($field:ident <= $value:expr) => {
        $field.less_than_or_equal($value)
    };
    ($field:ident + $value:expr) => {
        $field.add($value)
    };
    ($field:ident - $value:expr) => {
        $field.subtract($value)
    };
    ($field:ident * $value:expr) => {
        $field.multiply($value)
    };
    ($field:ident / $value:expr) => {
        $field.divide($value)
    };
    ($field:ident ** $value:expr) => {
        $field.power($value)
    };
    ($field:ident && $value:expr) => {
        $field.truthy_and($value)
    };
    ($field:ident || $value:expr) => {
        $field.truthy_or($value)
    };
    ($field:ident and $value:expr) => {
        $field.and($value)
    };
    ($field:ident or $value:expr) => {
        $field.or($value)
    };
    ($field:ident AND $value:expr) => {
        $field.and($value)
    };
    ($field:ident OR $value:expr) => {
        $field.or($value)
    };
    ($field:ident is not $value:expr) => {
        $field.is_not($value)
    };
    ($field:ident IS NOT $value:expr) => {
        $field.is_not($value)
    };
    ($field:ident CONTAINS $value:expr) => {
        $field.contains($value)
    };

    ($field:ident contains $value:expr) => {
        $field.contains($value)
    };

    ($field:ident CONTAINSNOT $value:expr) => {
        $field.contains_not($value)
    };

    ($field:ident containsnot $value:expr) => {
        $field.contains_not($value)
    };

    ($field:ident CONTAINSALL $value:expr) => {
        $field.contains_all($value)
    };

    ($field:ident containsall $value:expr) => {
        $field.contains_all($value)
    };

    ($field:ident CONTAINSANY $value:expr) => {
        $field.contains_any($value)
    };

    ($field:ident containsany $value:expr) => {
        $field.contains_any($value)
    };

    ($field:ident CONTAINSNONE $value:expr) => {
        $field.contains_none($value)
    };

    ($field:ident containsnone $value:expr) => {
        $field.contains_none($value)
    };

    ($field:ident INSIDE $value:expr) => {
        $field.inside($value)
    };

    ($field:ident inside $value:expr) => {
        $field.inside($value)
    };

    ($field:ident IN $value:expr) => {
        $field.inside($value)
    };

    ($field:ident in $value:expr) => {
        $field.inside($value)
    };

    ($field:ident NOTINSIDE $value:expr) => {
        $field.not_inside($value)
    };

    ($field:ident notinside $value:expr) => {
        $field.not_inside($value)
    };

    ($field:ident ALLINSIDE $value:expr) => {
        $field.all_inside($value)
    };

    ($field:ident allinside $value:expr) => {
        $field.all_inside($value)
    };

    ($field:ident ANYINSIDE $value:expr) => {
        $field.any_inside($value)
    };

    ($field:ident anyinside $value:expr) => {
        $field.any_inside($value)
    };

    ($field:ident NONEINSIDE $value:expr) => {
        $field.none_inside($value)
    };

    ($field:ident noneinside $value:expr) => {
        $field.none_inside($value)
    };

    ($field:ident OUTSIDE $value:expr) => {
        $field.outside($value)
    };

    ($field:ident outside $value:expr) => {
        $field.outside($value)
    };

    ($field:ident INTERSECTS $value:expr) => {
        $field.intersects($value)
    };

    ($field:ident intersects $value:expr) => {
        $field.intersects($value)
    };

    // Base patterns
    (($field:ident IS $value:expr)) => {
        $field.is($value)
    };
    (($field:ident = $value:expr)) => {
        $field.equal($value)
    };
    (($field:ident != $value:expr)) => {
        $field.not_equal($value)
    };
    (($field:ident == $value:expr)) => {
        $field.exactly_equal($value)
    };
    (($field:ident ?= $value:expr)) => {
        $field.any_equal($value)
    };
    (($field:ident *= $value:expr)) => {
        $field.all_equal($value)
    };
    (($field:ident > $value:expr)) => {
        $field.greater_than($value)
    };
    (($field:ident >= $value:expr)) => {
        $field.greater_than_or_equal($value)
    };
    (($field:ident < $value:expr)) => {
        $field.less_than($value)
    };
    (($field:ident <= $value:expr)) => {
        $field.less_than_or_equal($value)
    };
    (($field:ident + $value:expr)) => {
        $field.add($value)
    };
    (($field:ident - $value:expr)) => {
        $field.subtract($value)
    };
    (($field:ident * $value:expr)) => {
        $field.multiply($value)
    };
    (($field:ident / $value:expr)) => {
        $field.divide($value)
    };
    (($field:ident ** $value:expr)) => {
        $field.power($value)
    };
    (($field:ident && $value:expr)) => {
        $field.truthy_and($value)
    };
    (($field:ident || $value:expr)) => {
        $field.truthy_or($value)
    };
    (($field:ident AND $value:expr)) => {
        $field.and($value)
    };
    (($field:ident and $value:expr)) => {
        $field.and($value)
    };
    (($field:ident OR $value:expr)) => {
        $field.or($value)
    };
    (($field:ident or $value:expr)) => {
        $field.or($value)
    };
    (($field:ident IS NOT $value:expr)) => {
        $field.is_not($value)
    };

    (($field:ident is not $value:expr)) => {
        $field.is_not($value)
    };

    (($field:ident CONTAINS $value:expr)) => {
        $field.contains($value)
    };

    (($field:ident contains $value:expr)) => {
        $field.contains($value)
    };

    (($field:ident CONTAINSNOT $value:expr)) => {
        $field.contains_not($value)
    };

    (($field:ident containsnot $value:expr)) => {
        $field.contains_not($value)
    };

    (($field:ident CONTAINSALL $value:expr)) => {
        $field.contains_all($value)
    };

    (($field:ident containsall $value:expr)) => {
        $field.contains_all($value)
    };

    (($field:ident CONTAINSANY $value:expr)) => {
        $field.contains_any($value)
    };

    (($field:ident containsany $value:expr)) => {
        $field.contains_any($value)
    };

    (($field:ident CONTAINSNONE $value:expr)) => {
        $field.contains_none($value)
    };


    (($field:ident containsnone $value:expr)) => {
        $field.contains_none($value)
    };


    (($field:ident INSIDE $value:expr)) => {
        $field.inside($value)
    };


    (($field:ident inside $value:expr)) => {
        $field.inside($value)
    };


    (($field:ident IN $value:expr)) => {
        $field.inside($value)
    };

    (($field:ident in $value:expr)) => {
        $field.inside($value)
    };


    (($field:ident NOTINSIDE $value:expr)) => {
        $field.not_inside($value)
    };

    (($field:ident notinside $value:expr)) => {
        $field.not_inside($value)
    };


    (($field:ident ALLINSIDE $value:expr)) => {
        $field.all_inside($value)
    };

    (($field:ident allinside $value:expr)) => {
        $field.all_inside($value)
    };

    (($field:ident ANYINSIDE $value:expr)) => {
        $field.any_inside($value)
    };

    (($field:ident anyinside $value:expr)) => {
        $field.any_inside($value)
    };

    (($field:ident NONEINSIDE $value:expr)) => {
        $field.none_inside($value)
    };

    (($field:ident noneinside $value:expr)) => {
        $field.none_inside($value)
    };

    (($field:ident OUTSIDE $value:expr)) => {
        $field.outside($value)
    };

    (($field:ident outside $value:expr)) => {
        $field.outside($value)
    };

    (($field:ident INTERSECTS $value:expr)) => {
        $field.intersects($value)
    };

    (($field:ident intersects $value:expr)) => {
        $field.intersects($value)
    };


    ($left:tt AND $right:tt) => {
        $crate::cond(cond!($left)).and(cond!($right))
    };
    ($left:tt OR $right:tt) => {
        $crate::cond(cond!($left)).or(cond!($right))
    };


    // Handling recursive connectors
    ($left:tt AND $middle:tt AND $($tail:tt)*) => {
        $crate::cond!($left AND $middle).and($crate::cond!($($tail)*))
    };
    ($left:tt OR $middle:tt OR $($tail:tt)*) => {
        $crate::cond!($left OR $middle).or($crate::cond!($($tail)*))
    };
    ($left:tt AND $middle:tt OR $($tail:tt)*) => {
        $crate::cond!($left AND $middle).or($crate::cond!($($tail)*))
    };
    ($left:tt OR $middle:tt AND $($tail:tt)*) => {
        $crate::cond!($left OR $middle).and($crate::cond!($($tail)*))
    };

    ($left:tt and $middle:tt and $($tail:tt)*) => {
        $crate::cond!($left AND $middle).and($crate::cond!($($tail)*))
    };
    ($left:tt or $middle:tt or $($tail:tt)*) => {
        $crate::cond!($left OR $middle).or($crate::cond!($($tail)*))
    };
    ($left:tt and $middle:tt or $($tail:tt)*) => {
        $crate::cond!($left AND $middle).or($crate::cond!($($tail)*))
    };
    ($left:tt or $middle:tt and $($tail:tt)*) => {
        $crate::cond!($left OR $middle).and($crate::cond!($($tail)*))
    };

    // Base condition (a catch-all at the end)
    ($base:tt) => {
        $base
    };
}

/// This module provides functionality for building complex filters for database queries.
///
/// A `Filter` struct represents a filter that can be composed of subfilters using logical
/// operators like `AND` and `OR`. Filters can be created using the `empty` function or by
/// converting a string using `Filter::new`.
///
/// The `cond` function is used to create a new filter from a given `filterable` input, which
/// can be of type `Filter`.
///
/// Methods on a `Filter` instance are used to combine filters with logical operators or to
/// modify the filter using methods like `parenthesized`.
///
/// Combining filters groups the left-hand side, so `cond(a).or(b).and(c)` renders as
/// `((a) OR (b)) AND (c)`: conditions apply in the order they are chained.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    /// Shared, so a filter reused in several conditions or queries is never copied.
    node: Arc<FilterNode>,
    /// Already a single parenthesized group, so combining needs no extra parentheses.
    grouped: bool,
}

#[derive(Debug, PartialEq)]
enum FilterNode {
    /// A condition as written.
    Condition(ValueLike),
    /// `filter`, parenthesized unless it already is a group.
    Group(Filter),
    /// `(left) AND (right)`: the left filter as a group, then the right condition.
    Combined {
        left: Filter,
        operator: &'static str,
        right: ValueLike,
    },
}

impl Filter {
    /// Creates a new `Filter` instance.
    pub fn new(query: impl Conditional) -> Self {
        let grouped = query.is_grouped();
        Self {
            node: Arc::new(FilterNode::Condition(query.into())),
            grouped,
        }
    }

    /// Combines the current filter with another filter using a logical OR operator.
    ///
    /// # Arguments
    ///
    /// * `filter` - The filter to be combined with the current filter.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// # let title = Field::new("title");
    /// cond(age.greater_than(18))
    ///     .or(title.equal("Professor"));
    /// ```
    pub fn or(self, filter: impl Conditional) -> Self {
        self.combine(" OR ", filter)
    }

    /// Combines this `Filter` instance with another using the `AND` operator.
    ///
    /// # Arguments
    ///
    /// * `filter` - The `Filter` instance to combine with this one.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// # let title = Field::new("title");
    /// cond(age.greater_than(18))
    ///     .and(title.equal("Professor"));
    /// ```
    pub fn and(self, filter: impl Conditional) -> Self {
        self.combine(" AND ", filter)
    }

    /// Wraps this `Filter` instance in parenthesis.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// # let title = Field::new("title");
    ///
    /// let filter = cond(age.greater_than(18))
    ///     .or(title.equal("Professor"));
    /// let filter = filter.parenthesized();
    /// assert_eq!(filter.to_debug_sql(), "((age > 18) OR (title = 'Professor'))");
    /// ```
    pub fn parenthesized(&self) -> Self {
        Filter {
            node: Arc::new(FilterNode::Group(self.clone())),
            grouped: true,
        }
    }

    fn render_group(&self, out: &mut Renderer) {
        if self.grouped {
            self.render(out);
        } else {
            out.push_str("(").push(self).push_str(")");
        }
    }

    fn combine(self, operator: &'static str, filter: impl Conditional) -> Self {
        Filter {
            node: Arc::new(FilterNode::Combined {
                left: self,
                operator,
                right: filter.into(),
            }),
            grouped: false,
        }
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::new(Empty)
    }
}

impl<'a> From<Cow<'a, Filter>> for Filter {
    fn from(value: Cow<'a, Filter>) -> Self {
        match value {
            Cow::Borrowed(v) => v.clone(),
            Cow::Owned(v) => v,
        }
    }
}

impl From<Option<Self>> for Filter {
    fn from(value: Option<Filter>) -> Self {
        match value {
            Some(v) => v,
            None => Empty.into(),
        }
    }
}

impl From<Empty> for Filter {
    fn from(_value: Empty) -> Self {
        Filter::new(Empty)
    }
}

impl Render for Filter {
    fn render(&self, out: &mut Renderer) {
        match &*self.node {
            FilterNode::Condition(condition) => {
                out.push(condition);
            }
            FilterNode::Group(filter) => filter.render_group(out),
            FilterNode::Combined {
                left,
                operator,
                right,
            } => {
                left.render_group(out);
                out.push_str(operator)
                    .push_str("(")
                    .push(right)
                    .push_str(")");
            }
        }
    }
}

impl Conditional for Filter {
    fn is_grouped(&self) -> bool {
        self.grouped
    }
}
