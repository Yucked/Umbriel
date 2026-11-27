use crate::{Field, Model, Raw, Table};

/// The definitions of a table that migrations gather besides the model's own table and fields
/// ([`Model::define_table`] and [`Model::define_fields`]): its events and indexes, written with
/// the statement builders. Both default to none, so a model without them implements it with
/// `impl TableResources for Model {}`.
pub trait TableResources
where
    Self: Model,
{
    /// The table's `DEFINE EVENT` statements.
    fn events_definitions() -> Vec<Raw> {
        vec![]
    }

    /// The table's `DEFINE INDEX` statements.
    fn indexes_definitions() -> Vec<Raw> {
        vec![]
    }
}

/// A trait for field changes.
#[derive(Debug, Clone)]
pub struct FieldChangeMeta {
    /// The table where the field is located.
    pub table: Table,
    /// The old name of the field.
    pub old_name: Field,
    /// The new name of the field.
    pub new_name: Field,
}

/// Implements `DbResources::tables` and `tables_fields_meta` for the listed models: each model's
/// table, then its fields, indexes and events, in that dependency order.
#[macro_export]
macro_rules! create_table_resources {
    ($($struct_table: ident),*) => {
        fn tables(&self) -> ::std::vec::Vec<$crate::Raw> {
            let mut tables = ::std::vec::Vec::new();
            $(
                tables.push(<$struct_table as $crate::Model>::define_table());
                tables.extend(<$struct_table as $crate::Model>::define_fields());
                tables.extend(<$struct_table as $crate::TableResources>::indexes_definitions());
                tables.extend(<$struct_table as $crate::TableResources>::events_definitions());
            )*
            tables
        }


        fn tables_fields_meta(&self) -> ::std::collections::HashMap<$crate::Table, ::std::vec::Vec<$crate::FieldMetadata>> {
            let mut meta = ::std::collections::HashMap::<$crate::Table, ::std::vec::Vec<$crate::FieldMetadata>>::new();
            $(
                meta.insert(<$struct_table as $crate::Model>::table(), <$struct_table as $crate::Model>::get_field_meta());
            )*
            meta
        }


    };
}
