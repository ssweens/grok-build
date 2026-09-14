//! Extension system integration for the pager.
//!
//! This module provides the pager-side implementations of the extension API
//! traits: UI handle, session handle, agent handle, and extension context.

pub mod agent_handle;
pub mod context;
pub mod session_handle;
pub mod ui_handle;

pub use agent_handle::PagerAgentHandle;
pub use context::PagerExtensionContext;
pub use session_handle::PagerSessionHandle;
pub use ui_handle::{NoOpUiHandle, TuiUiHandle};
