//! Agent state handle for the extension API.

/// Agent state handle.
///
/// Provides access to the agent's current state — system prompt,
/// active tools, model, and thinking level.
pub trait AgentHandle: Send + Sync {
    /// The current rendered system prompt.
    fn system_prompt(&self) -> &str;

    /// Currently active tool names.
    fn active_tools(&self) -> Vec<&str>;

    /// All registered tool names (including inactive).
    fn all_tools(&self) -> Vec<&str>;

    /// Enable/disable tools dynamically.
    ///
    /// Names must already be registered; unknown names are ignored.
    fn set_active_tools(&self, names: &[&str]);

    /// The current model identifier.
    fn current_model(&self) -> &str;

    /// Get the current thinking level.
    fn thinking_level(&self) -> &str;

    /// Set the thinking level.
    fn set_thinking_level(&self, level: &str);
}
