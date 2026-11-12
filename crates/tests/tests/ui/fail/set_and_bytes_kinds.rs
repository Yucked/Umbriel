use umbriel::{
    ArrayLike, NumberLike, SetLike, StrandLike,
    functions::{bytes, set, type_},
};

// Sets are not arrays and bytes are not strings, and each set function returns its own kind.
fn main() {
    // A `Vec` is an array, not a set.
    let _ = set::len!(vec![1, 2]);
    // A string is not bytes.
    let _ = bytes::len!("text");
    // A set is not an array.
    let _ = ArrayLike::from(type_::set!(vec![1, 2]));
    // Bytes are not a string.
    let _ = StrandLike::from(type_::bytes!("text"));
    // `set::len` is an `int`, `set::join` a string and `set::union` a set.
    let _ = SetLike::from(set::len!(type_::set!(vec![1])));
    let _ = SetLike::from(set::join!(type_::set!(vec![1]), ","));
    let _ = NumberLike::from(set::union!(type_::set!(vec![1]), type_::set!(vec![2])));
}
