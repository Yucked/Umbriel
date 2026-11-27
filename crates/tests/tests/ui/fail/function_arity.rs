use umbriel::functions::string;

// A variadic function takes at least one argument. Fixed-arity functions are plain functions,
// whose arity the compiler checks.
fn main() {
    let _ = string::join!();
}
