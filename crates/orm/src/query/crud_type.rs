use crate::keywords::keywords;

keywords! {
    /// CRUD operation type
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CrudType: render {
        /// Creates a new record in the database
        Create => "create",
        /// Retrieves existing records from the database
        Select => "select",
        /// Updates existing records in the database
        Update => "update",
        /// Deletes existing records from the database
        Delete => "delete",
    }
}
