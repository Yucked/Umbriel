use umbriel::statements::{bearer_access, define_access, record_access};

// Record access, and bearer access for records, exist on a database only: on the root or a
// namespace the builders don't offer them.
fn main() {
    let _ = define_access("account").on_root().record(record_access());
    let _ = define_access("account")
        .on_namespace()
        .bearer_for_record(bearer_access());
}
