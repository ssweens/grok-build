//! Extension agent handle implementation.
//!
//! Bridges extension agent operations to the pager's agent state.

use xai_grok_extension_api::agent::AgentHandle;

/// Agent handle that wraps the pager's agent state.
///
/// Provides read access to system prompt, active tools, and model info.
pub struct PagerAgentHandle {
    system_prompt: String,
    active_tools: Vec<String>,
    all_tools: Vec<String>,
    current_model: String,
    thinking_level: String,
}

impl PagerAgentHandle {
    /// Create a new agent handle with the given state.
    pub fn new(
        system_prompt: String,
        active_tools: Vec<String>,
        all_tools: Vec<String>,
        current_model: String,
        thinking_level: String,
    ) -> Self {
        Self {
            system_prompt,
            active_tools,
            all_tools,
            current_model,
            thinking_level,
        }
    }

    /// Create an empty agent handle for testing.
    pub fn empty() -> Self {
        Self {
            system_prompt: String::new(),
            active_tools: Vec::new(),
            all_tools: Vec::new(),
            current_model: String::new(),
            thinking_level: "off".to_string(),
        }
    }
}

impl AgentHandle for PagerAgentHandle {
    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    fn active_tools(&self) -> Vec<&str> {
        self.active_tools.iter().map(|s| s.as_str()).collect()
    }

    fn all_tools(&self) -> Vec<&str> {
        self.all_tools.iter().map(|s| s.as_str()).collect()
    }

    fn set_active_tools(&self, _names: &[&str]) {
        // In a real implementation, this would update the agent's tool set.
        // For now, this is a no-op since we can't mutate the agent state
        // through a shared reference.
        tracing::warn!("set_active_tools not yet implemented for extension agent handle");
    }

    fn current_model(&self) -> &str {
        &self.current_model
    }

    fn thinking_level(&self) -> &str {
        &self.thinking_level
    }

    fn set_thinking_level(&self, _level: &str) {
        // In a real implementation, this would update the thinking level.
        tracing::warn!("set_thinking_level not yet implemented for extension agent handle");
    }
}
