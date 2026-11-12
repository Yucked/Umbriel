//! `search::*` (`orm/src/functions/search.rs`, owned by #19): rendering, moved unchanged from the
//! former `functions.rs`.

use umbriel::functions::search;
use umbriel_tests::*;

#[test]
fn rendering() {
    call(&search::score!(1), "search::score(1)");
    call(&search::offsets!(1), "search::offsets(1)");
    call(
        &search::highlight!("<b>", "</b>", 1),
        "search::highlight('<b>', '</b>', 1)",
    );
}
