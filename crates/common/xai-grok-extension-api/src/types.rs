//! Shared types for the extension API.

/// An autocomplete suggestion item.
#[derive(Debug, Clone)]
pub struct AutocompleteItem {
    /// The value to insert.
    pub value: String,

    /// Short label shown in the autocomplete menu.
    pub label: String,

    /// Optional longer description.
    pub description: Option<String>,
}
