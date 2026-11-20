//! `functions!` (`orm/src/functions/catalogue.rs`): one representative call for each argument
//! form and naming rule a catalogue entry can use. Each namespace's own file runs its functions on
//! the server.

use umbriel::{
    Field, ValueLike, count,
    functions::{array, rand as rand_fn, string, time},
};
use umbriel_tests::*;

#[test]
fn every_argument_form_renders() {
    calls!(
        // `a: T`: fixed arguments.
        (string::len!(Field::new("name")), "string::len(name)"),
        // `a: T; b: U, c: V`: optional trailing arguments, given or left out.
        (string::slice!("Alice", 1), "string::slice('Alice', 1)"),
        (
            string::slice!("Alice", 1, 3),
            "string::slice('Alice', 1, 3)"
        ),
        (
            array::fill!(Field::new("goals"), 0),
            "array::fill(goals, 0)"
        ),
        (
            array::fill!(Field::new("goals"), 0, 1, 2),
            "array::fill(goals, 0, 1, 2)"
        ),
        // `; b: U`: only optional arguments.
        (count!(), "count()"),
        (count!(vec![1, 0]), "count([1, 0])"),
        // `; [b: U, c: V]`: both arguments or neither.
        (rand_fn::float!(), "rand::float()"),
        (rand_fn::float!(1, 2), "rand::float(1, 2)"),
        // `.. values: T`: any number of arguments.
        (array::concat!(vec![1], vec![2]), "array::concat([1], [2])"),
    );
}

#[test]
fn names_that_are_not_rust_names_render_as_surrealql() {
    calls!(
        // `name = sql_name`.
        (rand_fn::enum_!("a", "b"), "rand::enum('a', 'b')"),
        (
            time::format_!(Field::new("created"), "%Y"),
            "time::format(created, '%Y')"
        ),
        // `Receiver.rust_name = sql_method`.
        (
            ValueLike::from(Field::new("input")).to_string_value(),
            "input.to_string()"
        ),
    );
}
