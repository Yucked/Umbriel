use umbriel::{Field, NumberLike, StrandLike, cast_dynamic, types::Kind};

// A dynamic cast is untyped, so it enters every typed expression unchecked: the documented
// escape hatch for kinds no marker names.
fn main() {
    let either = cast_dynamic(
        Kind::Either(vec![Kind::Int, Kind::String]),
        Field::new("value"),
    );
    let _ = NumberLike::from(either.clone());
    let _ = StrandLike::from(either);
}
