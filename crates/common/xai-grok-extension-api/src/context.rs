//! Extension context: bundles UI, session, agent, and signal access.
//!
//! Provided to tool `run()` calls, hook callbacks, and command handlers.
//! Modeled after pi's `ExtensionContext`.

use std::path::Path;

use crate::agent::AgentHandle;
use crate::session::SessionHandle;
use crate::ui::{RunMode, UiHandle};

/// The extension context: bundles UI, session, and agent access.
///
/// Provided to tool `run()` calls, hook callbacks, and command handlers.
/// Modeled after pi's `ExtensionContext`.
pub trait ExtensionContext: Send + Sync {
    /// UI interactions (dialogs, notifications, status).
    fn ui(&self) -> &dyn UiHandle;

    /// Session state access (entries, name, persistence).
    fn session(&self) -> &dyn SessionHandle;

    /// Agent access (system prompt, active tools).
    fn agent(&self) -> &dyn AgentHandle;

    /// Current working directory.
    fn cwd(&self) -> &Path;

    /// Current run mode: TUI, RPC, JSON, or headless.
    fn mode(&self) -> RunMode;

    /// Whether UI interactions are available (true for TUI/RPC, false for JSON/headless).
    fn has_ui(&self) -> bool {
        matches!(self.mode(), RunMode::Tui | RunMode::Rpc)
    }
}
