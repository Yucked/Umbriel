// array::add() Adds an item to an array if it doesn't exist
// array::all() Checks whether all array values are truthy
// array::any() Checks whether any array value is truthy
// array::at()	Returns value for X index, or in reverse for a negative index
// array::append() Appends an item to the end of an array
// array::boolean_and()	Perform the AND bitwise operations on two arrays
// array::boolean_or()	Perform the OR bitwise operations on two arrays
// array::boolean_xor()	Perform the XOR bitwise operations on two arrays
// array::boolean_not()	Perform the NOT bitwise operations on an array
// array::combine()	Combines all values from two arrays together
// array::complement() Returns the complement of two arrays
// array::clump()	Returns the original array split into multiple arrays of X size
// array::concat()	Returns the merged values from two arrays
// array::difference()	Returns the difference between two arrays
// array::distinct()	Returns the unique items in an array
// array::find_index()	Returns the index of the first occurrence of X value
// array::filter_index()	Find the indexes of all occurrences of all matching X value
// array::first()	Returns the first item in an array
// array::last()	Returns the last item in an array
// array::find() Finds the first item in an array that matches a condition
// array::flatten() Flattens multiple arrays into a single array
// array::group() Flattens and returns the unique items in an array
// array::insert() Inserts an item at the end of an array, or in a specific position
// array::intersect()	Returns the values which intersect two arrays
// array::len()	Returns the length of an array
// array::logical_and()	Performs the AND logical operations on two arrays
// array::logical_or()	Performs the OR logical operations on two arrays
// array::logical_xor()	Performs the XOR logical operations on two arrays
// array::retain(): TODO: Check: listed in  v10 release note but doesnt seme to be yet implemented
// array::max() Returns the maximum item in an array
// array::min() Returns the minimum item in an array
// array::matches()	Returns an array of boolenasjj
// array::pop() Returns the last item from an array
// array::prepend() Prepends an item to the beginning of an array
// array::push() Appends an item to the end of an array
// array::remove() Removes an item at a specific position from an array
// array::reverse() Reverses the sorting order of an array
// array::sort()	Sorts the values in an array in ascending or descending order
// array::slice()	Returns a slice of an array
// array::join
// array::sort::asc()	Sorts the values in an array in ascending order
// array::sort::desc()	Sorts the values in an array in descending order
// array::transpose()	Performs 2d array transposition on two arrays
// array::union()	Returns the unique merged values from two arrays

use crate::{ArrayLike, Function, NumberLike, ValueLike};

/// The array::add function adds an item to an array only if it doesn't exist.
/// array::add(array, value) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::add(["one", "two"], "three");
/// ["one", "two", "three"]
pub fn add_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::add")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::add function adds an item to an array only if it doesn't exist.
/// # Arguments
/// * `arr` - The array to add the value to. Could be an array, `Field` or `Param`
/// * `value` - The value to add to the array. any supported surrealdb value, `Field` or `Param`
/// # Example
/// ```rust
/// use umbriel::{*, functions::array};
/// array::add!(vec![1, 2, 3], 4);
/// array::add!(&[1, 2, 3], 4);
/// array::add!(arr![1, "alice", 3], 4);
///
/// let numbers = Field::new("numbers");
/// let value = Param::new("value");
/// let result = array::add!(numbers, value);
///
/// assert_eq!(
///    result.to_debug_sql(),
///    "array::add(numbers, $value)"
/// );
/// ```
#[macro_export]
macro_rules! array_add {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::add_fn($arr, $value)
    };
}
pub use array_add as add;

/// The array::all function checks whether all array values are truthy.
///
/// array::all(array) -> bool
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::all([1, 2, 3, NONE, 'SurrealDB', 5]);
/// false
pub fn all_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::all").arg(arr.into())
}

/// The array::all function checks whether all array values are truthy.
///
/// # Arguments
/// * `arr` - The array to check. Could be an array, `Field` or `Param`
///
/// # Example
/// ```rust
/// use umbriel::{*, functions::array};
/// array::all!(vec![1, 2, 3, 4, 5]);
/// array::all!(&[1, 2, 3, 4, 5]);
/// array::all!(arr![1, 2, 3, 4, 5]);
///
/// # let numbers_field = Field::new("numbers");
/// let result = array::all!(numbers_field);
///
/// assert_eq!(
///   result.to_debug_sql(),
///   "array::all(numbers)"
///   );
///   ```
#[macro_export]
macro_rules! array_all {
    ( $arr:expr ) => {
        $crate::functions::array::all_fn($arr)
    };
}
pub use array_all as all;

/// The array::any function checks whether any array values are truthy.
///
/// array::any(array) -> bool
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::any([1, 2, 3, NONE, 'SurrealDB', 5]);
/// true
pub fn any_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::any").arg(arr.into())
}

/// The array::any function checks whether any array values are truthy.
///
/// # Arguments
/// * `arr` - The array to check. Could be an array, `Field` or `Param`
///
/// # Example
/// ```rust
/// use umbriel::{*, functions::array};
/// array::any!(vec![1, 2, 3, 4, 5]);
/// array::any!(&[1, 2, 3, 4, 5]);
/// array::any!(arr![1, 2, 3, 4, 5]);
///
/// let numbers = Field::new("numbers");
/// let result = array::any!(numbers);
/// assert_eq!(
///  result.to_debug_sql(),
///  "array::any(numbers)"
///  );
///  ```
#[macro_export]
macro_rules! array_any {
    ( $arr:expr ) => {
        $crate::functions::array::any_fn($arr)
    };
}
pub use array_any as any;

/// The array::append function appends a value to the end of an array.
///
/// array::append(array, value) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::append([1,2,3,4], 5);
/// [1,2,3,4,5]
pub fn append_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::append")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::append function appends a value to the end of an array.
///
/// # Arguments
/// * `arr` - The array to append to. Could be an array, `Field` or `Param`
/// * `value` - The value to append. Could be a `Value`, `Field` or `Param`
///
/// # Example
/// ```rust
/// use umbriel::{*, functions::array};
/// array::append!(vec![1, 2, 3, 4, 5], 6);
/// array::append!(&[1, 2, 3, 4, 5], 6);
/// array::append!(arr![1, 2, 3, 4, 5], 6);
///
/// # let value_field = Field::new("value_field");
/// # let value_param = Param::new("value_param");
/// array::append!(vec![1, 2, 3, 4, 5], value_field);
/// array::append!(vec![1, 2, 3, 4, 5], value_param);
///
/// # let numbers = Field::new("numbers");
/// # let item = Field::new("item");
/// let result = array::append!(numbers, item);
/// assert_eq!(
///         result.to_debug_sql(),
///         "array::append(numbers, item)"
///     );
/// ```
#[macro_export]
macro_rules! array_append {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::append_fn($arr, $value)
    };
}
pub use array_append as append;

fn create_single_array_arg_helper(
    arr: impl Into<ArrayLike>,
    function_name: &'static str,
) -> Function {
    Function::new(function_name).arg(arr.into())
}

macro_rules! create_fn_with_single_array_arg {
    ($(#[$attr:meta])* => $function_name:expr) => {
        paste::paste! {
            $(#[$attr])*
            pub fn [<$function_name _fn>](arr: impl Into<$crate::ArrayLike>) -> $crate::Function {
                // Pathed: this module exports its own `concat` macro (`array::concat`).
                create_single_array_arg_helper(arr, ::core::concat!("array::", $function_name))
            }

            $(#[$attr])*
            #[macro_export]
            macro_rules! [<array_ $function_name>] {
                ( $arr:expr ) => {
                    $crate::functions::array::[<$function_name _fn>]($arr)
                };
            }
            pub use [<array_ $function_name>] as [<$function_name>];

        }
    };
}

create_fn_with_single_array_arg!(
    /// The array::boolean_not function performs the NOT bitwise operations on an array.
    ///
    /// # Arguments
    /// * `arr` - The array to perform the NOT bitwise operations on. Could be an array, `Field` or `Param`
    ///
    /// # Example
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// array::boolean_not!(vec![1, 2, 3, 4, 5]);
    /// array::boolean_not!(&[1, 2, 3, 4, 5]);
    /// array::boolean_not!(arr![1, 2, 3, 4, 5]);
    ///
    /// let numbers = Field::new("numbers");
    /// let result = array::boolean_not!(numbers);
    /// assert_eq!(
    ///  result.to_debug_sql(),
    ///  "array::boolean_not(numbers)"
    ///  );
    ///  ```
    =>
    "boolean_not"
);

create_fn_with_single_array_arg!(
    /// The array::first function returns the first item in an array.
    ///
    /// # Arguments
    /// * `arr` - The array to get the first item from. Could be an array, `Field` or `Param`
    ///
    /// # Example
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// array::first!(vec![1, 2, 3, 4, 5]);
    /// array::first!(&[1, 2, 3, 4, 5]);
    /// array::first!(arr![1, 2, 3, 4, 5]);
    ///
    /// let numbers = Field::new("numbers");
    /// let result = array::first!(numbers);
    /// assert_eq!(
    ///  result.to_debug_sql(),
    ///  "array::first(numbers)"
    ///  );
    ///  ```
    =>
    "first"
);

create_fn_with_single_array_arg!(
    /// The array::last function returns the last item in an array.
    ///
    /// # Arguments
    /// * `arr` - The array to get the last item from. Could be an array, `Field` or `Param`
    ///
    /// # Example
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// array::last!(vec![1, 2, 3, 4, 5]);
    /// array::last!(&[1, 2, 3, 4, 5]);
    /// array::last!(arr![1, 2, 3, 4, 5]);
    ///
    /// let numbers = Field::new("numbers");
    /// let result = array::last!(numbers);
    /// assert_eq!(
    ///  result.to_debug_sql(),
    ///  "array::last(numbers)"
    ///  );
    ///  ```
    =>
    "last"
);

create_fn_with_single_array_arg!(
    /// The array::max function returns the maximum item in an array.
    ///
    /// # Arguments
    /// * `arr` - The array to get the maximum item from. Could be an array, `Field` or `Param`
    ///
    /// # Example
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// array::max!(vec![1, 2, 3, 4, 5]);
    /// array::max!(&[1, 2, 3, 4, 5]);
    /// array::max!(arr![1, 2, 3, 4, 5]);
    ///
    /// let numbers = Field::new("numbers");
    /// let result = array::max!(numbers);
    /// assert_eq!(
    ///  result.to_debug_sql(),
    ///  "array::max(numbers)"
    ///  );
    ///  ```
    =>
    "max"
);

create_fn_with_single_array_arg!(
    /// The array::min function returns the minimum item in an array.
    ///
    /// # Arguments
    /// * `arr` - The array to get the minimum item from. Could be an array, `Field` or `Param`
    ///
    /// # Example
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// array::min!(vec![1, 2, 3, 4, 5]);
    /// array::min!(&[1, 2, 3, 4, 5]);
    /// array::min!(arr![1, 2, 3, 4, 5]);
    ///
    /// let numbers = Field::new("numbers");
    /// let result = array::min!(numbers);
    /// assert_eq!(
    ///  result.to_debug_sql(),
    ///  "array::min(numbers)"
    ///  );
    ///  ```
    =>
    "min"
);

fn create_two_array_args_helper(
    arr1: impl Into<ArrayLike>,
    arr2: impl Into<ArrayLike>,
    function_name: &'static str,
) -> Function {
    Function::new(function_name)
        .arg(arr1.into())
        .arg(arr2.into())
}

macro_rules! create_fn_with_two_array_args {
    ($(#[$attr:meta])* => $function_name:expr) => {
        paste::paste! {
            $(#[$attr])*
            pub fn [<$function_name _fn>](arr1: impl Into<$crate::ArrayLike>, arr2: impl Into<$crate::ArrayLike>) -> $crate::Function {
                create_two_array_args_helper(arr1, arr2, ::core::concat!("array::", $function_name))
            }

            $(#[$attr])*
            #[macro_export]
            macro_rules! [<array_ $function_name>] {
                ( $arr1:expr, $arr2:expr ) => {
                    $crate::functions::array::[<$function_name _fn>]($arr1, $arr2)
                };
            }
            pub use [<array_ $function_name>] as [<$function_name>];

        }
    };
}

create_fn_with_two_array_args!(
    /// "The array::combine function combines all values from two arrays together, returning an array of arrays.
    ///
    ///   # Arguments
    ///   * `arr1` - The first array to combine. A vector, field or param.
    ///   * `arr2` - The second array to combine. A vector, field or param.
    ///
    /// # Examples
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::combine!(vec![1, 2, 3], vec![4, 5, 6]);
    /// array::combine!(own_goals, goals);
    /// array::combine!(&[1, 2, 3], &[4, 5, 6]);
    ///
    /// // It is also aliased as array_combine;
    /// array_combine!(&[1, 2, 3], &[4, 5, 6]);
    /// ```
=>
    "combine"
);

create_fn_with_two_array_args!(
    /// The array::concat function merges two arrays together, returning an array which may contain duplicate values. If you want to remove duplicate values from the resulting array, then use the array::union() function
    ///
    ///   # Arguments
    ///   * `arr1` - The first array to concat. A vector, field or param.
    ///   * `arr2` - The second array to concat. A vector, field or param.
    /// # Examples
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::concat!(vec![1, 2, 3], vec![4, 5, 6]);
    /// array::concat!(own_goals, goals);
    /// array::concat!(&[1, 2, 3], &[4, 5, 6]);
    ///
    /// // It is also aliased as array_concat;
    /// array_concat!(&[1, 2, 3], &[4, 5, 6]);
    /// ```
    =>
    "concat"
);

create_fn_with_two_array_args!(
    /// The array::union function combines two arrays together, removing duplicate values, and returning a single array.
    /// # Examples
    ///
    /// # Arguments
    ///  * `arr1` -  A vector, field or param.
    ///  * `arr2` -  A vector, field or param.
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::union!(vec![1, 2, 3], vec![4, 5, 6]);
    /// array::union!(own_goals, goals);
    /// array::union!(&[1, 2, 3], &[4, 5, 6]);
    /// // It is also aliased as array_union;
    /// array_union!(&[1, 2, 3], &[4, 5, 6]);
    /// ```
    =>
    "union"
);

create_fn_with_two_array_args!(
    /// The array::difference determines the difference between two arrays, returning a single array
    /// containing items which are not in both arrays.
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::difference!(vec![1, 2, 3], vec![4, 5, 6]);
    /// array::difference!(own_goals, goals);
    /// array::difference!(&[1, 2, 3], &[4, 5, 6]);
    ///
    /// // It is also aliased as array_difference;
    /// array_difference!(&[1, 2, 3], &[4, 5, 6]);
    /// ```
    =>
    "difference"
);

create_fn_with_two_array_args!(
    /// The array::intersect function calculates the values which intersect two arrays, returning a
    /// single array containing the values which are in both arrays.
    /// array::intersect(array, array) -> array
    /// The following example shows this function, and its output, when used in a select statement:
    ///
    /// SELECT * FROM array::intersect([1,2,3,4], [3,4,5,6]);
    /// [3,4]
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::intersect!(vec![1, 2, 3], vec![4, 5, 6]);
    /// array::intersect!(own_goals, goals);
    /// array::intersect!(&[1, 2, 3], &[4, 5, 6]);
    ///
    /// // It is also aliased as array_intersect;
    /// array_intersect!(&[1, 2, 3], &[4, 5, 6]);
    /// ```
    =>
    "intersect"
);

create_fn_with_two_array_args!(
    /// The array::complement function returns the complement of two arrays, returning a single array containing items which are not in the second array.
    /// array::complement(array, array) -> array
    /// The following example shows this function, and its output, when used in a select statement:
    ///
    /// SELECT * FROM array::complement([1,2,3,4], [3,4,5,6]);
    /// [1,2]
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::complement!(vec![1, 2, 3, 4], vec![3, 4, 5, 6]);
    /// array::complement!(own_goals, goals);
    /// array::complement!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    ///
    /// // It is also aliased as array_complement;
    /// array_complement!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    /// ```
    =>
    "complement"
);

create_fn_with_two_array_args!(
    /// The array::boolean_and function performs the AND bitwise operations on two arrays.
    /// array::boolean_and(array, array) -> array
    /// The following example shows this function, and its output, when used in a select statement:
    ///
    /// SELECT * FROM array::boolean_and([1,2,3,4], [3,4,5,6]);
    /// [1,2,3,4]
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::boolean_and!(vec![1, 2, 3, 4], vec![3, 4, 5, 6]);
    /// array::boolean_and!(own_goals, goals);
    /// array::boolean_and!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    ///
    /// // It is also aliased as array_boolean_and;
    /// array_boolean_and!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    /// ```
    =>
    "boolean_and"
);

create_fn_with_two_array_args!(
    /// The array::boolean_or function performs the OR bitwise operations on two arrays.
    /// array::boolean_or(array, array) -> array
    /// The following example shows this function, and its output, when used in a select statement:
    ///
    /// SELECT * FROM array::boolean_or([1,2,3,4], [3,4,5,6]);
    /// [3,4,5,6]
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::boolean_or!(vec![1, 2, 3, 4], vec![3, 4, 5, 6]);
    /// array::boolean_or!(own_goals, goals);
    /// array::boolean_or!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    ///
    /// // It is also aliased as array_boolean_or;
    /// array_boolean_or!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    /// ```
    =>
    "boolean_or"
);

create_fn_with_two_array_args!(
    /// The array::boolean_xor function performs the XOR bitwise operations on two arrays.
    /// array::boolean_xor(array, array) -> array
    /// The following example shows this function, and its output, when used in a select statement:
    ///
    /// SELECT * FROM array::boolean_xor([1,2,3,4], [3,4,5,6]);
    /// [2,5,6]
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    ///
    /// array::boolean_xor!(vec![1, 2, 3, 4], vec![3, 4, 5, 6]);
    /// array::boolean_xor!(own_goals, goals);
    /// array::boolean_xor!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    ///
    /// // It is also aliased as array_boolean_xor;
    /// array_boolean_xor!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    /// ```
    =>
    "boolean_xor"
);

create_fn_with_two_array_args!(
    /// The array::logical_and function performs the AND logical operations on two arrays.
    /// array::logical_and(array, array) -> array
    /// The following example shows this function, and its output, when used in a select statement:
    ///
    /// SELECT * FROM array::logical_and([1,2,3,4], [3,4,5,6]);
    /// [3,4]
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    ///
    /// array::logical_and!(vec![1, 2, 3, 4], vec![3, 4, 5, 6]);
    /// array::logical_and!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    ///
    /// // It is also aliased as array_logical_and;
    /// array_logical_and!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    /// ```
    =>
    "logical_and"
);

create_fn_with_two_array_args!(
    /// The array::logical_or function performs the OR logical operations on two arrays.
    /// array::logical_or(array, array) -> array
    /// The following example shows this function, and its output, when used in a select statement:
    ///
    /// SELECT * FROM array::logical_or([1,2,3,4], [3,4,5,6]);
    /// [1,2,3,4,5,6]
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    ///
    /// array::logical_or!(vec![1, 2, 3, 4], vec![3, 4, 5, 6]);
    /// array::logical_or!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    ///
    /// // It is also aliased as array_logical_or;
    /// array_logical_or!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    /// ```
    =>
    "logical_or"
);

create_fn_with_two_array_args!(
    /// The array::logical_xor function performs the XOR logical operations on two arrays.
    /// array::logical_xor(array, array) -> array
    /// The following example shows this function, and its output, when used in a select statement:
    ///
    /// SELECT * FROM array::logical_xor([1,2,3,4], [3,4,5,6]);
    /// [1,2,5,6]
    ///
    /// # Arguments
    /// * `arr1` -  A vector, field or param.
    /// * `arr2` -  A vector, field or param.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use umbriel::{*, functions::array};
    ///
    /// array::logical_xor!(vec![1, 2, 3, 4], vec![3, 4, 5, 6]);
    /// array::logical_xor!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    ///
    /// // It is also aliased as array_logical_xor;
    /// array_logical_xor!(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    /// ```
    =>
    "logical_xor"
);

/// The array::transpose function transposes an array of arrays, returning a single array containing the transposed values.
/// array::transpose(array) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::transpose([[1,2,3],[4,5,6],[7,8,9]]);
pub fn transpose_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::transpose").arg(arr.into())
}

/// The array::transpose function transposes an array of arrays, returning a single array containing the transposed values.
/// # Arguments
/// * `arr` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::transpose!(vec![1, 2, 3, 4, 5]);
/// array::transpose!(own_goals);
/// array::transpose!(&[1, 2, 3, 4, 5]);
/// array::transpose!(vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]]);
/// ```
#[macro_export]
macro_rules! array_transpose {
    ( $arr:expr ) => {
        $crate::functions::array::transpose_fn($arr)
    };
}
pub use array_transpose as transpose;

/// The array::matches function returns true if the array matches the specified pattern.
pub fn matches_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::matches")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::matches function returns true if the array matches the specified pattern.
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `value` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::matches!(vec![1, 2, 3, 4, 5], 2);
/// array::matches!(own_goals, goals);
/// array::matches!(&[1, 2, 3, 4, 5], 2);
/// // It is also aliased as array_matches;
/// array_matches!(&[1, 2, 3, 4, 5], 2);
/// ```
#[macro_export]
macro_rules! array_matches {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::matches_fn($arr, $value)
    };
}
pub use array_matches as matches;

/// The array:at function returns the value at the specified index in an array.
pub fn at_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::at").arg(arr.into()).arg(value.into())
}

/// The array:at function returns the value at the specified index in an array.
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `value` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::at!(vec![1, 2, 3, 4], 2);
/// array::at!(own_goals, goals);
/// array::at!(&[1, 2, 3, 4], 2);
/// // It is also aliased as array_at;
/// array_at!(&[1, 2, 3, 4], 2);
/// ```
#[macro_export]
macro_rules! array_at {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::at_fn($arr, $value)
    };
}
pub use array_at as at;

/// The array::clump function returns the original array split into sub-arrays of size. Similar to slice::chunks
pub fn clump_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::clump")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::clump function returns the original array split into sub-arrays of size. Similar to slice::chunks
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `value` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::clump!(vec![1, 2, 3, 4], 2);
/// array::clump!(own_goals, goals);
/// array::clump!(&[1, 2, 3, 4], 2);
/// // It is also aliased as array_clump;
/// array_clump!(&[1, 2, 3, 4], 2);
/// ```
#[macro_export]
macro_rules! array_clump {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::clump_fn($arr, $value)
    };
}
pub use array_clump as clump;

/// The array::distinct function calculates the unique values in an array, returning a single array.
pub fn distinct_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::distinct").arg(arr.into())
}

/// The array::distinct function calculates the unique values in an array, returning a single
/// array.
/// # Arguments
/// * `arr` -  A vector, field or param.
///
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
///
/// array::distinct!(vec![1, 2, 3]);
/// array::distinct!(&[1, 2, 3]);
/// array::distinct!(own_goals);
/// array::distinct!(goals);
///
/// // It is also aliased as array_distinct;
/// array_distinct!(vec![1, 2, 3]);
/// ```
#[macro_export]
macro_rules! array_distinct {
    ( $arr:expr ) => {
        $crate::functions::array::distinct_fn($arr)
    };
}
pub use array_distinct as distinct;

/// The array::find function returns the first value in an array which matches the given value.
pub fn find_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::find")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::find function returns the first value in an array which matches the given value.
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `value` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::find!(vec![1, 2, 3, 4], 2);
/// array::find!(own_goals, goals);
/// array::find!(&[1, 2, 3, 4], 2);
/// // It is also aliased as array_find;
/// array_find!(&[1, 2, 3, 4], 2);
/// ```
#[macro_export]
macro_rules! array_find {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::find_fn($arr, $value)
    };
}
pub use array_find as find;

/// The array::find_index function returns the index of the first value in an array which matches the given value.
pub fn find_index_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::find_index")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::find_index function returns the index of the first value in an array which matches the given value.
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `value` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::find_index!(vec![1, 2, 3, 4], 2);
/// array::find_index!(own_goals, goals);
/// array::find_index!(&[1, 2, 3, 4], 2);
/// // It is also aliased as array_find_index;
/// array_find_index!(&[1, 2, 3, 4], 2);
/// ```
#[macro_export]
macro_rules! array_find_index {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::find_index_fn($arr, $value)
    };
}
pub use array_find_index as find_index;

/// The array::filter_index function returns the index of the first value in an array which matches the given value.
pub fn filter_index_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::filter_index")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::filter_index function returns the index of the first value in an array which matches the given value.
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `value` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::filter_index!(vec![1, 2, 3, 4], 2);
/// array::filter_index!(own_goals, goals);
/// array::filter_index!(&[1, 2, 3, 4], 2);
/// // It is also aliased as array_filter_index;
/// array_filter_index!(&[1, 2, 3, 4], 2);
/// ```
#[macro_export]
macro_rules! array_filter_index {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::filter_index_fn($arr, $value)
    };
}
pub use array_filter_index as filter_index;

/// The array::flatten flattens an array of arrays, returning a new array with all sub-array elements concatenated into it.
///
/// array::flatten(array) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::flatten([ [1,2], [3, 4], 'SurrealDB', [5, 6, [7, 8]] ]);
/// [1, 2, 3, 4, 'SurrealDB', 5, 6, [7, 8]]
pub fn flatten_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::flatten").arg(arr.into())
}

/// The array::flatten flattens an array of arrays, returning a new array with all sub-array
/// elements concatenated into it.
/// # Arguments
/// * `arr` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// array::flatten!(array![vec![1, 2], vec![3, 4], "SurrealDB", vec![5, 6]]);
/// array::flatten!(arr![vec![1, 2], vec![3, 4], "SurrealDB", vec![5, 6]]);
///
/// # let own_goals_field = Field::new("own_goals_field");
/// # let goals_param = Param::new("goals_param");
/// array::flatten!(own_goals_field);
/// array::flatten!(goals_param);
/// // It is also aliased as array_flatten;
/// array_flatten!(arr![vec![1, 2], vec![3, 4], "SurrealDB", vec![5, 6]]);
/// array_flatten!(array![vec![1, 2], vec![3, 4], "SurrealDB", vec![5, 6]]);
/// ```
/// # Output
/// ```json
/// [1, 2, 3, 4, "SurrealDB", 5, 6, [7, 8]]
/// ```
#[macro_export]
macro_rules! array_flatten {
    ( $arr:expr ) => {
        $crate::functions::array::flatten_fn($arr)
    };
}
pub use array_flatten as flatten;

/// The array::group function flattens and returns the unique items in an array.
///
/// array::group(array) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::group([1, 2, 3, 4, [3,5,6], [2,4,5,6], 7, 8, 8, 9]);
/// [1,2,3,4,5,6,7,8,9]
pub fn group_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::group").arg(arr.into())
}

/// The array::group function flattens and returns the unique items in an array.
///
/// array::group(array) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::group([1, 2, 3, 4, [3,5,6], [2,4,5,6], 7, 8, 8, 9]);
/// [1,2,3,4,5,6,7,8,9]
///
/// # Arguments
/// * `arr` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// array::group!(array![1, 2, 3, 4, array![3, 5, 6], vec![2, 4, 5, 6], 7, 8, 8, 9]);
/// # let own_goals_field = Field::new("own_goals_field");
/// # let goals_param = Param::new("goals_param");
/// array::group!(own_goals_field);
/// array::group!(goals_param);
/// // It is also aliased as array_group;
/// array_group!(arr![1, 2, 3, 4, vec![3, 5, 6], vec![2, 4, 5, 6], 7, 8, 8, 9]);
/// ```
/// # Output
/// ```json
/// [1,2,3,4,5,6,7,8,9]
/// ```
#[macro_export]
macro_rules! array_group {
    ( $arr:expr ) => {
        $crate::functions::array::group_fn($arr)
    };
}
pub use array_group as group;

/// The array::insert function inserts a value into an array at a specific position.
///
/// array::insert(array, value, number) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::insert([1,2,3,4], 5, 2);
/// [1,2,5,3,4]
pub fn insert_fn(
    arr: impl Into<ArrayLike>,
    value: impl Into<ValueLike>,
    index: impl Into<NumberLike>,
) -> Function {
    Function::new("array::insert")
        .arg(arr.into())
        .arg(value.into())
        .arg(index.into())
}

/// The array::insert function inserts a value into an array at a specific position.
/// array::insert(array, value, number) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::insert([1,2,3,4], 5, 2);
/// [1,2,5,3,4]
///
/// # Arguments
///
/// * `arr` -  A vector, `Field`, `Param`
/// * `value` -  A value, `Field`, `Param`
/// * `index` -  A number, `Field`, `Param`
///
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// array::insert!(arr![1, 2, 3, 4], 5, 2);
/// array::insert!(array![1, 2, 3, 4], 5, 2);
/// # let own_goals_field = Field::new("own_goals_field");
/// # let goals_param = Param::new("goals_param");
/// array::insert!(own_goals_field, 5, 2);
/// array::insert!(goals_param, 5, 2);
/// // It is also aliased as array_insert;
/// array_insert!(arr![1, 2, 3, 4], 5, 2);
/// ```
/// # Output
/// ```json
/// [1,2,5,3,4]
/// ```
#[macro_export]
macro_rules! array_insert {
    ( $arr:expr, $value:expr, $index:expr ) => {
        $crate::functions::array::insert_fn($arr, $value, $index)
    };
}
pub use array_insert as insert;

/// The array::len function calculates the length of an array, returning a number. This function
/// includes all items when counting the number of items in the array. If you want to only count
/// truthy values, then use the count() function.
pub fn len_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::len").arg(arr.into())
}

/// The array::len function calculates the length of an array, returning a number.
/// This function includes all items when counting the number of items in the array.
/// If you want to only count truthy values, then use the count() function.
///
/// # Arguments
/// * `arr` -  A vector, field or param.
///
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
///
/// array::len!(vec![1, 2, 3]);
/// array::len!(&[1, 2, 3]);
/// array::len!(own_goals);
/// array::len!(goals);
/// // It is also aliased as array_len;
/// array_len!(vec![1, 2, 3]);
/// ```
#[macro_export]
macro_rules! array_len {
    ( $arr:expr ) => {
        $crate::functions::array::len_fn($arr)
    };
}
pub use array_len as len;

/// The array::pop function removes a value from the end of an array and returns it.
///
/// array::pop(array) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::pop([1,2,3,4]);
/// 4
pub fn pop_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::pop").arg(arr.into())
}

/// The array::pop function removes a value from the end of an array and returns it.
/// array::pop(array) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::pop([1,2,3,4]);
/// 4
/// # Arguments
/// * `arr` -  A vector, field or param.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// array::pop!(vec![1, 2, 3]);
/// array::pop!(&[1, 2, 3]);
/// # let own_goals_field = Field::new("own_goals_field");
/// # let goals_param = Param::new("goals_param");
/// array::pop!(own_goals_field);
/// array::pop!(goals_param);
/// // It is also aliased as array_pop;
/// array_pop!(vec![1, 2, 3]);
/// ```
#[macro_export]
macro_rules! array_pop {
    ( $arr:expr ) => {
        $crate::functions::array::pop_fn($arr)
    };
}
pub use array_pop as pop;

/// The array::prepend function prepends a value to the end of an array.
///
/// array::prepend(array, value) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::prepend([1,2,3,4], 5);
/// [5,1,2,3,4]
pub fn prepend_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::prepend")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::prepend function prepends a value to the end of an array.
/// array::prepend(array, value) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::prepend([1,2,3,4], 5);
/// [5,1,2,3,4]
/// j
/// # Arguments
/// * `arr` -  An array, `Field` or `Param`
/// * `value` -  A value, `Field` or `Param`
///
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// array::prepend!(vec![1, 2, 3], 4);
/// array::prepend!(&[1, 2, 3], 4);
/// # let own_goals_field = Field::new("own_goals_field");
/// # let goals_param = Param::new("goals_param");
/// array::prepend!(own_goals_field, 4);
/// array::prepend!(goals_param, 4);
/// // It is also aliased as array_prepend;
/// array_prepend!(vec![1, 2, 3], 4);
/// ```
#[macro_export]
macro_rules! array_prepend {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::prepend_fn($arr, $value)
    };
}
pub use array_prepend as prepend;

/// The array::push function appends a value to the end of an array.
///
/// array::push(array, value) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::push([1,2,3,4], 5);
/// [1,2,3,4,5]
/// The ordering of the array.
pub fn push_fn(arr: impl Into<ArrayLike>, value: impl Into<ValueLike>) -> Function {
    Function::new("array::push")
        .arg(arr.into())
        .arg(value.into())
}

/// The array::push function appends a value to the end of an array.
/// array::push(array, value) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::push([1,2,3,4], 5);
/// [1,2,3,4,5]
///
/// # Arguments
/// * `arr` -  An array, `Field` or `Param`
/// * `value` -  A value, `Field` or `Param`
///
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// array::push!(vec![1, 2, 3], 4);
/// array::push!(&[1, 2, 3], 4);
/// # let own_goals_field = Field::new("own_goals_field");
/// # let goals_param = Param::new("goals_param");
/// array::push!(own_goals_field, 4);
/// array::push!(goals_param, 4);
/// // It is also aliased as array_push;
/// array_push!(vec![1, 2, 3], 4);
/// ```
#[macro_export]
macro_rules! array_push {
    ( $arr:expr, $value:expr ) => {
        $crate::functions::array::push_fn($arr, $value)
    };
}
pub use array_push as push;

/// The array::remove function removes an item from a specific position in an array.
///
/// array::remove(array, number) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::remove([1,2,3,4,5], 2);
/// [1,2,4,5]
pub fn remove_fn(arr: impl Into<ArrayLike>, number: impl Into<ValueLike>) -> Function {
    Function::new("array::remove")
        .arg(arr.into())
        .arg(number.into())
}

/// The array::remove function removes an item from a specific position in an array.
/// array::remove(array, number) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::remove([1,2,3,4,5], 2);
/// [1,2,4,5]
///
/// # Arguments
/// * `arr` -  An array, `Field` or `Param`
/// * `number` -  A number, `Field` or `Param`
///
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// array::remove!(vec![1, 2, 3, 4, 5], 2);
/// array::remove!(&[1, 2, 3, 4, 5], 2);
/// # let own_goals_field = Field::new("own_goals_field");
/// # let goals_param = Param::new("goals_param");
/// array::remove!(own_goals_field, 2);
/// array::remove!(goals_param, 2);
/// // It is also aliased as array_remove;
/// array_remove!(vec![1, 2, 3, 4, 5], 2);
/// ```
#[macro_export]
macro_rules! array_remove {
    ( $arr:expr, $number:expr ) => {
        $crate::functions::array::remove_fn($arr, $number)
    };
}
pub use array_remove as remove;

/// The array::reverse function appends a value to the end of an array.
///
/// array::reverse(array) -> array
/// The following example shows this function, and its output, when used in a select statement:
///
/// SELECT * FROM array::reverse([1,2,3,4,5]);
/// [5,4,3,2,1]
pub fn reverse_fn(arr: impl Into<ArrayLike>) -> Function {
    Function::new("array::reverse").arg(arr.into())
}

/// The array::reverse function appends a value to the end of an array.
/// array::reverse(array) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::reverse([1,2,3,4,5]);
/// [5,4,3,2,1]
///
/// # Arguments
/// * `arr` -  An array, `Field` or `Param`
///
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// array::reverse!(vec![1, 2, 3, 4, 5]);
/// array::reverse!(&[1, 2, 3, 4, 5]);
/// # let own_goals_field = Field::new("own_goals_field");
/// # let goals_param = Param::new("goals_param");
///     
/// array::reverse!(own_goals_field);
/// array::reverse!(goals_param);
/// // It is also aliased as array_reverse;
/// array_reverse!(vec![1, 2, 3, 4, 5]);
/// ```
#[macro_export]
macro_rules! array_reverse {
    ( $arr:expr ) => {
        $crate::functions::array::reverse_fn($arr)
    };
}
pub use array_reverse as reverse;

/// The array::sort function sorts an array in ascending or descending order.
#[derive(Debug, Clone, PartialEq)]
pub enum Ordering {
    /// Sort the array in ascending order.
    Asc,
    /// Sort the array in descending order.
    Desc,
    /// Do not sort the array.
    False,
    /// Do not sort the array.
    Empty,
}

/// The array::sort function calculates the length of an array, returning a number. This function
/// includes all items when counting the number of items in the array. If you want to only count
/// truthy values, then use the count() function.
pub fn sort_fn(arr: impl Into<ArrayLike>, ordering: Ordering) -> Function {
    let function = Function::new("array::sort").arg(arr.into());
    match ordering {
        Ordering::Asc => function.arg(ValueLike::from("asc")),
        Ordering::Desc => function.arg(ValueLike::from("desc")),
        Ordering::False => function.arg(ValueLike::from(false)),
        Ordering::Empty => function,
    }
}
/// The array::sort function calculates the length of an array, returning a number. This function
/// includes all items when counting the number of items in the array. If you want to only count
/// truthy values, then use the count() function.
///
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `ordering` -  The ordering of the array.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
///
/// array::sort!(vec![1, 2, 3], "asc");
/// array::sort!(&[1, 2, 3], "desc");
/// array::sort!(own_goals, false);
/// array::sort!(goals, array::Ordering::Empty);
/// // It is also aliased as array_sort;
/// array_sort!(vec![1, 2, 3], "asc");
/// ```
#[macro_export]
macro_rules! array_sort {
    ( $arr:expr, "asc" ) => {
        $crate::functions::array::sort_fn($arr, $crate::functions::array::Ordering::Asc)
    };
    ( $arr:expr, "desc" ) => {
        $crate::functions::array::sort_fn($arr, $crate::functions::array::Ordering::Desc)
    };
    ( $arr:expr, false ) => {
        $crate::functions::array::sort_fn($arr, $crate::functions::array::Ordering::False)
    };
    ( $arr:expr, $ordering:expr ) => {
        $crate::functions::array::sort_fn($arr, $ordering)
    };
    ( $arr:expr ) => {
        $crate::functions::array::sort_fn($arr, $crate::functions::array::Ordering::Empty)
    };
}
pub use array_sort as sort;

/// This module contains functions for sorting arrays.
pub mod sort {
    use crate::*;

    /// The array::sort::asc function is a shorthand convenience function for the array::sort function, to sort values in an array in ascending order.
    pub fn asc_fn(arr: impl Into<ArrayLike>) -> Function {
        Function::new("array::sort::asc").arg(arr.into())
    }

    /// The array::sort::asc function is a shorthand convenience function for the array::sort function, to sort values in an array in ascending order.
    /// # Arguments
    /// * `arr` -  A vector, field or param.
    /// # Examples
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    /// array::sort::asc!(vec![1, 2, 3]);
    /// array::sort::asc!(&[1, 2, 3]);
    /// array::sort::asc!(own_goals);
    /// array::sort::asc!(goals);
    ///
    /// // It is also aliased as array_sort_asc;
    /// array_sort_asc!(vec![1, 2, 3]);
    /// ```
    #[macro_export]
    macro_rules! array_sort_asc {
        ( $arr:expr ) => {
            $crate::functions::array::sort::asc_fn($arr)
        };
    }
    pub use array_sort_asc as asc;

    /// The array::sort::desc function is a shorthand convenience function for the array::sort
    /// function, to sort values in an array in descending order.
    pub fn desc_fn(arr: impl Into<ArrayLike>) -> Function {
        Function::new("array::sort::desc").arg(arr.into())
    }

    /// The array::sort::desc function is a shorthand convenience function for the array::sort function, to sort values in an array in descending order.
    /// # Arguments
    /// * `arr` -  A vector, field or param.
    /// # Examples
    /// ```rust
    /// use umbriel::{*, functions::array};
    /// let own_goals = Field::new("own_goals");
    /// let goals = Param::new("goals");
    /// array::sort::desc!(vec![1, 2, 3]);
    /// array::sort::desc!(&[1, 2, 3]);
    /// array::sort::desc!(own_goals);
    /// array::sort::desc!(goals);
    /// // It is also aliased as array_sort_desc;
    /// array_sort_desc!(vec![1, 2, 3]);
    /// ```
    #[macro_export]
    macro_rules! array_sort_desc {
        ( $arr:expr ) => {
            $crate::functions::array::sort::desc_fn($arr)
        };
    }
    pub use array_sort_desc as desc;
}

/// The array::slice function returns a portion of an array.
/// array::slice(array, start, len) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::slice([1,2,3,4,5], 2, 4);
/// [3,4]
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `start` -  The start of the slice.
/// * `len` -  length of the slice.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::slice!(vec![1, 2, 3, 4, 5], 2, 4);
/// array::slice!(&[1, 2, 3, 4, 5], 2, 4);
/// array::slice!(own_goals, 2, 4);
/// array::slice!(goals, 2, 4);
/// // It is also aliased as array_slice;
/// array_slice!(vec![1, 2, 3, 4, 5], 2, 4);
/// ```
pub fn slice_fn(
    arr: impl Into<ArrayLike>,
    start: impl Into<NumberLike>,
    len: impl Into<NumberLike>,
) -> Function {
    Function::new("array::slice")
        .arg(arr.into())
        .arg(start.into())
        .arg(len.into())
}

/// Slice function returns a portion of an array.
/// array::slice(array, start, len) -> array
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::slice([1,2,3,4,5], 2, 4);
/// [3,4]
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `start` -  The start of the slice.
/// * `len` -  length of the slice.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::slice!(vec![1, 2, 3, 4, 5], 2, 4);
/// array::slice!(&[1, 2, 3, 4, 5], 2, 4);
/// array::slice!(own_goals, 2, 4);
/// array::slice!(goals, 2, 4);
/// // It is also aliased as array_slice;
/// array_slice!(vec![1, 2, 3, 4, 5], 2, 4);
/// ```
#[macro_export]
macro_rules! array_slice {
    ( $arr:expr, $start:expr, $end:expr ) => {
        $crate::functions::array::slice_fn($arr, $start, $end)
    };
}

pub use array_slice as slice;

/// The array::join function joins the elements of an array into a string, and returns the string.
pub fn join_fn(arr: impl Into<ArrayLike>, separator: impl Into<ValueLike>) -> Function {
    Function::new("array::join")
        .arg(arr.into())
        .arg(separator.into())
}

/// The array::join function joins the elements of an array into a string, and returns the string.
/// array::join(array, separator) -> string
/// The following example shows this function, and its output, when used in a select statement:
/// SELECT * FROM array::join([1,2,3,4,5], ',');
/// 1,2,3,4,5
/// # Arguments
/// * `arr` -  A vector, field or param.
/// * `separator` -  The separator of the array.
/// # Examples
/// ```rust
/// use umbriel::{*, functions::array};
/// let own_goals = Field::new("own_goals");
/// let goals = Param::new("goals");
/// array::join!(vec![1, 2, 3, 4, 5], ",");
/// array::join!(&[1, 2, 3, 4, 5], ",");
/// array::join!(own_goals, ",");
/// array::join!(goals, ",");
/// // It is also aliased as array_join;
/// array_join!(vec![1, 2, 3, 4, 5], ",");
/// ```
#[macro_export]
macro_rules! array_join {
    ( $arr:expr, $separator:expr ) => {
        $crate::functions::array::join_fn($arr, $separator)
    };
}
pub use array_join as join;
