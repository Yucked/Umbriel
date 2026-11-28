use std::sync::{Arc, Mutex, PoisonError};

use crate::migrations::*;

/// The questions generating, pruning and resetting migrations may ask. `RealPrompter`, behind the
/// `cli` feature, asks on the terminal; [`MockPrompter`] answers with preset answers. A prompt
/// that fails, such as one on a closed terminal, is [`MigrationError::PromptError`].
pub trait Prompter
where
    Self: std::fmt::Debug,
{
    /// Whether to write a migration although the schemas do not differ, e.g. to fill in by hand.
    fn prompt_empty_migrations_trigger(&self) -> MigrationResult<bool>;

    /// Whether a single field replaced by another, without `old_name`, is renamed (its values
    /// copied) or deleted: one of the two options.
    fn prompt_single_field_rename_or_delete(
        &self,
        delete_option: SingleFieldChangeType,
        rename_option: SingleFieldChangeType,
    ) -> MigrationResult<SingleFieldChangeType>;

    /// Whether to go ahead with `action` on `items`, such as the files a prune deletes.
    fn confirm(&self, action: &str, items: &[String]) -> MigrationResult<bool>;
}

/// Asks `prompter` to confirm `action` on `items`, unless `yes` or there is nothing to confirm.
///
/// # Errors
///
/// [`MigrationError::Declined`] when the user declines, and the prompt's own failure.
pub(crate) fn confirmed(
    prompter: &(impl Prompter + ?Sized),
    yes: bool,
    action: &str,
    items: &[String],
) -> MigrationResult<()> {
    if yes || items.is_empty() || prompter.confirm(action, items)? {
        return Ok(());
    }
    Err(MigrationError::Declined)
}

/// Asks on the terminal.
#[cfg(feature = "cli")]
#[derive(Debug, Default, Clone)]
pub struct RealPrompter;

#[cfg(feature = "cli")]
impl RealPrompter {
    fn failed(error: inquire::error::InquireError) -> MigrationError {
        MigrationError::PromptError(Box::new(error))
    }
}

#[cfg(feature = "cli")]
impl Prompter for RealPrompter {
    fn prompt_empty_migrations_trigger(&self) -> MigrationResult<bool> {
        inquire::Confirm::new("Are you sure you want to generate an empty migration? (y/n)")
            .with_default(false)
            .with_help_message("This is good if you want to write out some queries manually")
            .prompt()
            .map_err(Self::failed)
    }

    fn prompt_single_field_rename_or_delete(
        &self,
        delete_option: SingleFieldChangeType,
        rename_option: SingleFieldChangeType,
    ) -> MigrationResult<SingleFieldChangeType> {
        inquire::Select::new(
            "Do you want to rename this field or delete the old one completely without \
            transferring the data",
            vec![delete_option, rename_option],
        )
        .with_help_message("Use the arrow keys to navigate. Press enter to select.")
        .prompt()
        .map_err(Self::failed)
    }

    /// Lists `items` and asks, defaulting to no.
    fn confirm(&self, action: &str, items: &[String]) -> MigrationResult<bool> {
        println!("{action}:");
        for item in items {
            println!("  {item}");
        }
        inquire::Confirm::new("Go ahead? (y/n)")
            .with_default(false)
            .prompt()
            .map_err(Self::failed)
    }
}

/// An answer to whether a replaced field is renamed or deleted.
#[derive(Debug, Clone, Copy)]
pub enum RenameOrDelete {
    /// Rename: copy the old field's values into the new one.
    Rename,
    /// Delete the old field and its values.
    Delete,
}

/// Answers every question with preset answers, for tests and non-interactive runs. Clones share
/// the record of what was asked to confirm.
#[derive(Debug, Clone)]
pub struct MockPrompter {
    /// Whether a migration is written when the schemas do not differ.
    pub allow_empty_migrations_gen: bool,

    /// What happens to a single field replaced by another without `old_name`.
    pub rename_or_delete_single_field_change: RenameOrDelete,

    /// The answer to every confirmation.
    pub confirm: bool,

    /// The items of each confirmation asked, in order.
    confirmations: Arc<Mutex<Vec<Vec<String>>>>,
}

impl MockPrompter {
    /// Answers with `allow_empty_migrations_gen` and `rename_or_delete_single_field_change`, and
    /// confirms everything.
    pub fn new(
        allow_empty_migrations_gen: bool,
        rename_or_delete_single_field_change: RenameOrDelete,
    ) -> Self {
        Self {
            allow_empty_migrations_gen,
            rename_or_delete_single_field_change,
            confirm: true,
            confirmations: Arc::default(),
        }
    }

    /// Answers every confirmation with `confirm`.
    pub fn confirming(mut self, confirm: bool) -> Self {
        self.confirm = confirm;
        self
    }

    /// The items of each confirmation this prompter, or a clone of it, was asked, in order.
    pub fn confirmations(&self) -> Vec<Vec<String>> {
        self.confirmations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Default for MockPrompter {
    fn default() -> Self {
        Self::new(true, RenameOrDelete::Rename)
    }
}

impl Prompter for MockPrompter {
    fn prompt_empty_migrations_trigger(&self) -> MigrationResult<bool> {
        Ok(self.allow_empty_migrations_gen)
    }

    fn prompt_single_field_rename_or_delete(
        &self,
        delete_option: SingleFieldChangeType,
        rename_option: SingleFieldChangeType,
    ) -> MigrationResult<SingleFieldChangeType> {
        match self.rename_or_delete_single_field_change {
            RenameOrDelete::Rename => Ok(rename_option),
            RenameOrDelete::Delete => Ok(delete_option),
        }
    }

    fn confirm(&self, _action: &str, items: &[String]) -> MigrationResult<bool> {
        self.confirmations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(items.to_vec());
        Ok(self.confirm)
    }
}
