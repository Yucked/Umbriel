use inquire::error::InquireError;

use crate::migrations::*;

/// The questions generating a migration may ask; the defaults ask on the terminal.
pub trait Prompter
where
    Self: std::fmt::Debug,
{
    /// Whether to write a migration although the schemas do not differ, e.g. to fill in by hand.
    fn prompt_empty_migrations_trigger(&self) -> Result<bool, InquireError> {
        inquire::Confirm::new("Are you sure you want to generate an empty migration? (y/n)")
            .with_default(false)
            .with_help_message("This is good if you want to write out some queries manually")
            .prompt()
    }

    /// Whether a single field replaced by another, without `old_name`, is renamed (its values
    /// copied) or deleted.
    fn prompt_single_field_rename_or_delete(
        &self,
        delete_option: SingleFieldChangeType,
        rename_option: SingleFieldChangeType,
    ) -> Result<SingleFieldChangeType, InquireError> {
        inquire::Select::new(
            "Do you want to rename this field or delete the old one completely without \
            transferring the data",
            vec![delete_option, rename_option],
        )
        .with_help_message("Use the arrow keys to navigate. Press enter to select.")
        .prompt()
    }
}

/// Asks on the terminal.
#[derive(Debug, Default, Clone)]
pub struct RealPrompter;

impl Prompter for RealPrompter {}

/// An answer to whether a replaced field is renamed or deleted.
#[derive(Debug, Clone, Copy)]
pub enum RenameOrDelete {
    /// Rename: copy the old field's values into the new one.
    Rename,
    /// Delete the old field and its values.
    Delete,
}

/// Answers every question with preset answers, for tests and non-interactive runs.
#[derive(Debug, Clone, Copy)]
pub struct MockPrompter {
    /// Whether a migration is written when the schemas do not differ.
    pub allow_empty_migrations_gen: bool,

    /// What happens to a single field replaced by another without `old_name`.
    pub rename_or_delete_single_field_change: RenameOrDelete,
}

impl MockPrompter {
    /// Answers with `allow_empty_migrations_gen` and `rename_or_delete_single_field_change`.
    pub fn new(
        allow_empty_migrations_gen: bool,
        rename_or_delete_single_field_change: RenameOrDelete,
    ) -> Self {
        Self {
            allow_empty_migrations_gen,
            rename_or_delete_single_field_change,
        }
    }
}

impl Default for MockPrompter {
    fn default() -> Self {
        Self {
            allow_empty_migrations_gen: true,
            rename_or_delete_single_field_change: RenameOrDelete::Rename,
        }
    }
}

impl Prompter for MockPrompter {
    fn prompt_empty_migrations_trigger(&self) -> Result<bool, InquireError> {
        Ok(self.allow_empty_migrations_gen)
    }

    fn prompt_single_field_rename_or_delete(
        &self,
        delete_option: SingleFieldChangeType,
        rename_option: SingleFieldChangeType,
    ) -> Result<SingleFieldChangeType, InquireError> {
        match self.rename_or_delete_single_field_change {
            RenameOrDelete::Rename => Ok(rename_option),
            RenameOrDelete::Delete => Ok(delete_option),
        }
    }
}
