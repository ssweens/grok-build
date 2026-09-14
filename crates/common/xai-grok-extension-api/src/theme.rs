//! Theme types for the extension API.
//!
//! Extensions receive a [`Theme`] reference in render callbacks and
//! custom component contexts.

/// Theme for styled terminal output.
///
/// Implementations bridge to the TUI's actual theme system.
pub trait Theme: Send + Sync {
    /// Apply a foreground color to text.
    fn fg(&self, color: &str, text: &str) -> String;

    /// Apply a background color to text.
    fn bg(&self, color: &str, text: &str) -> String;

    /// Make text bold.
    fn bold(&self, text: &str) -> String;

    /// Make text italic.
    fn italic(&self, text: &str) -> String;

    /// Make text dim/muted.
    fn dim(&self, text: &str) -> String;
}

/// Standard theme color names.
pub mod colors {
    pub const ACCENT: &str = "accent";
    pub const SUCCESS: &str = "success";
    pub const ERROR: &str = "error";
    pub const WARNING: &str = "warning";
    pub const MUTED: &str = "muted";
    pub const DIM: &str = "dim";
    pub const TOOL_TITLE: &str = "toolTitle";
}
