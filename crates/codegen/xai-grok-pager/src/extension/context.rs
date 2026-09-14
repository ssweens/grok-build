//! Extension context implementation.
//!
//! Bundles UI, session, and agent handles into a single context object
//! that extensions receive during tool execution, hook callbacks, and
//! command handlers.

use std::path::{Path, PathBuf};

use xai_grok_extension_api::agent::AgentHandle;
use xai_grok_extension_api::context::ExtensionContext;
use xai_grok_extension_api::session::SessionHandle;
use xai_grok_extension_api::ui::{RunMode, UiHandle};

/// Extension context implementation for the pager.
///
/// Bundles UI, session, and agent handles into a single context object.
pub struct PagerExtensionContext {
    ui: Box<dyn UiHandle>,
    session: Box<dyn SessionHandle>,
    agent: Box<dyn AgentHandle>,
    cwd: PathBuf,
    mode: RunMode,
}

impl PagerExtensionContext {
    /// Create a new extension context.
    pub fn new(
        ui: Box<dyn UiHandle>,
        session: Box<dyn SessionHandle>,
        agent: Box<dyn AgentHandle>,
        cwd: PathBuf,
        mode: RunMode,
    ) -> Self {
        Self {
            ui,
            session,
            agent,
            cwd,
            mode,
        }
    }
}

impl ExtensionContext for PagerExtensionContext {
    fn ui(&self) -> &dyn UiHandle {
        &*self.ui
    }

    fn session(&self) -> &dyn SessionHandle {
        &*self.session
    }

    fn agent(&self) -> &dyn AgentHandle {
        &*self.agent
    }

    fn cwd(&self) -> &Path {
        &self.cwd
    }

    fn mode(&self) -> RunMode {
        self.mode
    }

    fn has_ui(&self) -> bool {
        self.ui.has_ui()
    }
}
