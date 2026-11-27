use umbriel::statements::kill;

// SurrealDB parses `KILL` with a UUID or a parameter only; a string is a parse error, so the
// builder doesn't take one.
fn main() {
    let _ = kill("6f2c2a4e-0000-0000-0000-000000000000");
}
