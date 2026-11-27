/// A macro to create a vector of heterogeous/diverse surrealdb value
#[macro_export]
macro_rules! array {
    ($( $val:expr ),*) => {{
        vec![
            $( $crate::types::SurrealValue::into_value($val) ),*
        ]
    }};
}
