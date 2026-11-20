//! Graph arrows (`orm/src/query/arrow.rs`).

use umbriel::{Arrow, Render};

#[test]
fn both_renders_two_way() {
    assert_eq!(Arrow::Right.to_debug_sql(), "->");
    assert_eq!(Arrow::Left.to_debug_sql(), "<-");
    assert_eq!(Arrow::Both.to_debug_sql(), "<->");
}
