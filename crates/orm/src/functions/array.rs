//! Array functions, `array::*`.
//!
//! ```rust
//! use umbriel::{*, functions::array};
//! let tags = Field::new("tags");
//! assert_eq!(array::len!(tags.clone()).to_debug_sql(), "array::len(tags)");
//! assert_eq!(ArrayLike::from(tags).distinct().to_debug_sql(), "tags.distinct()");
//! ```

use crate::kinds;

functions! { $ "array" "::", array, array;
    /// The array with `value` appended unless it is already there.
    add(array: ArrayLike, value: ValueLike) -> kinds::Array [ArrayLike.add];
    /// Whether every element is truthy, or matches `check` (a value or a closure).
    all(array: ArrayLike; check: ValueLike) -> kinds::Bool [ArrayLike.all];
    /// Whether any element is truthy, or matches `check` (a value or a closure).
    any(array: ArrayLike; check: ValueLike) -> kinds::Bool [ArrayLike.any];
    /// The array with `value` appended.
    append(array: ArrayLike, value: ValueLike) -> kinds::Array [ArrayLike.append];
    /// The element at `index`; a negative index counts from the end.
    at(array: ArrayLike, index: NumberLike) -> kinds::Any [ArrayLike.at];
    /// The element-wise `AND` of two arrays.
    boolean_and(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Bool>
        [ArrayLike.boolean_and];
    /// Each element negated.
    boolean_not(array: ArrayLike) -> kinds::Array<kinds::Bool> [ArrayLike.boolean_not];
    /// The element-wise `OR` of two arrays.
    boolean_or(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Bool> [ArrayLike.boolean_or];
    /// The element-wise `XOR` of two arrays.
    boolean_xor(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Bool>
        [ArrayLike.boolean_xor];
    /// The array in chunks of `size`.
    clump(array: ArrayLike, size: NumberLike) -> kinds::Array<kinds::Array> [ArrayLike.clump];
    /// Every pair of an element of `a` with an element of `b`.
    combine(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Array> [ArrayLike.combine];
    /// The elements of `a` that are not in `b`.
    complement(a: ArrayLike, b: ArrayLike) -> kinds::Array [ArrayLike.complement];
    /// The arrays joined into one.
    concat(.. arrays: ArrayLike) -> kinds::Array [ArrayLike.concat];
    /// The elements in only one of the arrays.
    difference(a: ArrayLike, b: ArrayLike) -> kinds::Array [ArrayLike.difference];
    /// The array without duplicates.
    distinct(array: ArrayLike) -> kinds::Array [ArrayLike.distinct];
    /// Whether every element is truthy, or matches `check`; the same as `all`.
    every(array: ArrayLike; check: ValueLike) -> kinds::Bool [ArrayLike.every];
    /// The array with its elements from `start` (an index or a range) to `end` set to `value`.
    fill(array: ArrayLike, value: ValueLike; start: ValueLike, end: NumberLike) -> kinds::Array
        [ArrayLike.fill];
    /// The elements equal to `check`, or for which the closure `check` is truthy.
    filter(array: ArrayLike, check: ValueLike) -> kinds::Array [ArrayLike.filter];
    /// The indexes of the elements equal to `check`, or for which the closure is truthy.
    filter_index(array: ArrayLike, check: ValueLike) -> kinds::Array<kinds::Int>
        [ArrayLike.filter_index];
    /// The first element equal to `check`, or for which the closure is truthy; `NONE` without
    /// one.
    find(array: ArrayLike, check: ValueLike) -> kinds::Any [ArrayLike.find];
    /// The index of the first element equal to `check`, or for which the closure is truthy;
    /// `NONE` without one.
    find_index(array: ArrayLike, check: ValueLike) -> kinds::Any [ArrayLike.find_index];
    /// The first element.
    first(array: ArrayLike) -> kinds::Any [ArrayLike.first];
    /// The nested arrays flattened one level.
    flatten(array: ArrayLike) -> kinds::Array [ArrayLike.flatten];
    /// The closure applied to an accumulator, starting at `init`, and each element.
    fold(array: ArrayLike, init: ValueLike, closure: ClosureLike) -> kinds::Any
        [ArrayLike.fold];
    /// The distinct elements of the nested arrays, flattened.
    group(array: ArrayLike) -> kinds::Array [ArrayLike.group];
    /// Whether any element is truthy, or matches `check`; the same as `any`.
    includes(array: ArrayLike; check: ValueLike) -> kinds::Bool [ArrayLike.includes];
    /// The index of the first element matching `check`; the same as `find_index`.
    index_of(array: ArrayLike, check: ValueLike) -> kinds::Any [ArrayLike.index_of];
    /// The array with `value` inserted at `index`, or appended.
    insert(array: ArrayLike, value: ValueLike; index: NumberLike) -> kinds::Array
        [ArrayLike.insert];
    /// The elements in both arrays.
    intersect(a: ArrayLike, b: ArrayLike) -> kinds::Array [ArrayLike.intersect];
    /// Whether the array is empty.
    is_empty(array: ArrayLike) -> kinds::Bool [ArrayLike.is_empty];
    /// The elements as strings, joined with `separator`.
    join(array: ArrayLike, separator: StrandLike) -> kinds::String [ArrayLike.join];
    /// The last element.
    last(array: ArrayLike) -> kinds::Any [ArrayLike.last];
    /// The number of elements.
    len(array: ArrayLike) -> kinds::Int [ArrayLike.len];
    /// The element-wise `AND` of two arrays, keeping the values.
    logical_and(a: ArrayLike, b: ArrayLike) -> kinds::Array [ArrayLike.logical_and];
    /// The element-wise `OR` of two arrays, keeping the values.
    logical_or(a: ArrayLike, b: ArrayLike) -> kinds::Array [ArrayLike.logical_or];
    /// The element-wise `XOR` of two arrays, keeping the values.
    logical_xor(a: ArrayLike, b: ArrayLike) -> kinds::Array [ArrayLike.logical_xor];
    /// The closure applied to each element (and its index, as a second parameter).
    map(array: ArrayLike, closure: ClosureLike) -> kinds::Array [ArrayLike.map];
    /// Whether each element equals `value`.
    matches(array: ArrayLike, value: ValueLike) -> kinds::Array<kinds::Bool>
        [ArrayLike.matches];
    /// The greatest element.
    max(array: ArrayLike) -> kinds::Any [ArrayLike.max];
    /// The least element.
    min(array: ArrayLike) -> kinds::Any [ArrayLike.min];
    /// The last element, removed.
    pop(array: ArrayLike) -> kinds::Any [ArrayLike.pop];
    /// The array with `value` prepended.
    prepend(array: ArrayLike, value: ValueLike) -> kinds::Array [ArrayLike.prepend];
    /// The array with `value` appended.
    push(array: ArrayLike, value: ValueLike) -> kinds::Array [ArrayLike.push];
    /// The integers of the range `start`, or from `start` to `end` exclusive.
    range(start: ValueLike; end: NumberLike) -> kinds::Array<kinds::Int>;
    /// The closure applied to an accumulator, starting at the first element, and each other
    /// element.
    reduce(array: ArrayLike, closure: ClosureLike) -> kinds::Any [ArrayLike.reduce];
    /// The array without the element at `index`.
    remove(array: ArrayLike, index: NumberLike) -> kinds::Array [ArrayLike.remove];
    /// An array of `value` repeated `count` times. Its method form is on every value except
    /// strings, where `.repeat()` is `string::repeat`.
    repeat(value: ValueLike, count: NumberLike) -> kinds::Array [ValueLike.repeat];
    /// The array reversed.
    reverse(array: ArrayLike) -> kinds::Array [ArrayLike.reverse];
    /// `length` consecutive integers from `start`, or `start` integers from 0.
    sequence(start: NumberLike; length: NumberLike) -> kinds::Array<kinds::Int>;
    /// The array shuffled.
    shuffle(array: ArrayLike) -> kinds::Array [ArrayLike.shuffle];
    /// The elements from the index `start` up to the index `end`, exclusive, or in the range
    /// `start`.
    slice(array: ArrayLike; start: ValueLike, end: NumberLike) -> kinds::Array
        [ArrayLike.slice];
    /// Whether any element matches `check`; the same as `any`.
    some(array: ArrayLike; check: ValueLike) -> kinds::Bool [ArrayLike.some];
    /// The array sorted; `order` is `'asc'` (the default), `'desc'` or `false` (unsorted).
    sort(array: ArrayLike; order: ValueLike) -> kinds::Array [ArrayLike.sort];
    /// The array sorted lexically, strings compared case-insensitively.
    sort_lexical(array: ArrayLike; order: ValueLike) -> kinds::Array [ArrayLike.sort_lexical];
    /// The array sorted naturally, numbers in strings compared as numbers.
    sort_natural(array: ArrayLike; order: ValueLike) -> kinds::Array [ArrayLike.sort_natural];
    /// The array sorted naturally and lexically.
    sort_natural_lexical(array: ArrayLike; order: ValueLike) -> kinds::Array
        [ArrayLike.sort_natural_lexical];
    /// The array with the elements at `from` and `to` swapped.
    swap(array: ArrayLike, from: NumberLike, to: NumberLike) -> kinds::Array [ArrayLike.swap];
    /// The nested arrays transposed.
    transpose(array: ArrayLike) -> kinds::Array [ArrayLike.transpose];
    /// The distinct elements of both arrays.
    union(a: ArrayLike, b: ArrayLike) -> kinds::Array [ArrayLike.union];
    /// Every run of `size` consecutive elements.
    windows(array: ArrayLike, size: NumberLike) -> kinds::Array<kinds::Array>
        [ArrayLike.windows];
}

/// Sorting functions, `array::sort::*`.
pub mod sort {
    use crate::kinds;

    functions! { $ "array::sort" "::", array_sort, array::sort;
        /// The array sorted in ascending order.
        asc(array: ArrayLike) -> kinds::Array [ArrayLike.sort_asc];
        /// The array sorted in descending order.
        desc(array: ArrayLike) -> kinds::Array [ArrayLike.sort_desc];
    }
}
