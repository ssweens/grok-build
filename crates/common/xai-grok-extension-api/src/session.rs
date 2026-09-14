//! Session state types for the extension API.

use std::path::PathBuf;

/// A message that can be injected into the LLM context.
#[derive(Debug, Clone)]
pub struct ExtensionMessage {
    /// Extension-defined type for routing and rendering.
    pub custom_type: String,

    /// Message content (sent to the LLM).
    pub content: String,

    /// Whether to display in the TUI.
    pub display: bool,

    /// Additional structured data.
    pub details: serde_json::Value,
}

/// Options for sending messages.
#[derive(Debug, Clone)]
pub struct SendOptions {
    /// Delivery mode.
    pub deliver_as: DeliverMode,

    /// Whether to trigger an LLM response immediately.
    pub trigger_turn: bool,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            deliver_as: DeliverMode::Steer,
            trigger_turn: false,
        }
    }
}

/// Message delivery mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliverMode {
    /// Queue while streaming, deliver after current tool batch.
    Steer,
    /// Wait for agent to finish all tools.
    FollowUp,
    /// Queue for next user prompt.
    NextTurn,
}

/// A session entry (message, tool result, custom, etc.).
#[derive(Debug, Clone)]
pub struct SessionEntry {
    /// Entry ID.
    pub id: String,

    /// Entry type.
    pub entry_type: SessionEntryType,

    /// Timestamp (unix millis).
    pub timestamp: u64,
}

/// Session entry types.
#[derive(Debug, Clone)]
pub enum SessionEntryType {
    /// A user message.
    User { content: String },

    /// An assistant message.
    Assistant { content: String },

    /// A tool result.
    ToolResult {
        tool_name: String,
        output: serde_json::Value,
    },

    /// A custom entry from an extension.
    Custom {
        custom_type: String,
        data: serde_json::Value,
    },
}

/// Session state handle.
pub trait SessionHandle: Send + Sync {
    /// Persist a custom entry. Does not participate in LLM context.
    fn append_entry(&self, custom_type: &str, data: serde_json::Value);

    /// Get all entries in the current branch.
    fn get_entries(&self) -> Vec<SessionEntry>;

    /// Set the session display name.
    fn set_name(&self, name: &str);

    /// Get the session display name.
    fn name(&self) -> Option<String>;

    /// Get the session file path, if any.
    fn session_file(&self) -> Option<PathBuf>;

    /// Send a message to the LLM context.
    fn send_message(&self, message: ExtensionMessage, options: SendOptions);

    /// Send a user message (as if typed by the user).
    fn send_user_message(&self, content: &str, options: SendOptions);
}
