//! UI interaction types for the extension API.
//!
//! Extensions interact with the user via the [`UiHandle`] trait.
//! In TUI mode, methods are backed by the ratatui rendering loop.
//! In headless/JSON mode, methods are no-ops or return defaults.

use std::future::Future;
use std::pin::Pin;

/// A boxed future returned by UI methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Notification severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyLevel {
    Info,
    Warning,
    Error,
}

/// Current run mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Full TUI with terminal rendering.
    Tui,
    /// RPC mode (JSON protocol over stdio).
    Rpc,
    /// JSON mode (event stream to stdout).
    Json,
    /// Headless print mode (`-p`).
    Print,
}

/// Context passed to custom component renderers.
pub struct CustomComponentCtx {
    /// Terminal width/height.
    pub terminal_size: (u16, u16),

    /// Theme for styling.
    pub theme: Box<dyn crate::theme::Theme>,
}

/// UI interaction handle. Modeled after pi's `ctx.ui`.
///
/// In TUI mode, methods are backed by the ratatui rendering loop.
/// In headless/JSON mode, methods are no-ops or return defaults.
pub trait UiHandle: Send + Sync {
    /// Show a selection dialog. Returns the chosen index, or None on cancel.
    ///
    /// In non-interactive mode, returns `None` immediately.
    fn select<'a>(&'a self, title: &'a str, options: &'a [&'a str]) -> BoxFuture<'a, Option<usize>>;

    /// Show a confirmation dialog. Returns true if confirmed.
    ///
    /// In non-interactive mode, returns `true` immediately.
    fn confirm<'a>(&'a self, title: &'a str, body: &'a str) -> BoxFuture<'a, bool>;

    /// Show a text input dialog. Returns the entered text, or None on cancel.
    ///
    /// In non-interactive mode, returns `None` immediately.
    fn input<'a>(&'a self, title: &'a str, placeholder: &'a str) -> BoxFuture<'a, Option<String>>;

    /// Show a non-blocking notification.
    fn notify(&self, message: &str, level: NotifyLevel);

    /// Set a persistent status in the footer. Pass None to clear.
    fn set_status(&self, key: &str, message: Option<&str>);

    /// Set a widget above/below the editor.
    fn set_widget(&self, key: &str, lines: Option<Vec<String>>);

    /// Whether UI interactions are available.
    fn has_ui(&self) -> bool {
        matches!(self.mode(), RunMode::Tui | RunMode::Rpc)
    }

    /// Current run mode.
    fn mode(&self) -> RunMode;
}
