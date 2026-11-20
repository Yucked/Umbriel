use umbriel::Recurse;

// A shortest-path recursion searches for a record id or a parameter holding one.
fn main() {
    let _ = Recurse::range(1..=3).shortest("person:bob");
    let _ = Recurse::range(1..=3).shortest(42);
}
