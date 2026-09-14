//! Extension session handle implementation.
//!
//! Bridges extension session operations to the pager's session management.

use std::path::PathBuf;

use tokio::sync::mpsc;

use xai_grok_extension_api::session::{
    ExtensionMessage, SendOptions, SessionEntry, SessionHandle,
};

/// Session request sent from extension to pager.
#[derive(Debug)]
pub enum SessionRequest {
    /// Append a custom entry to the session.
    AppendEntry {
        custom_type: String,
        data: serde_json::Value,
    },
    /// Send a message to the LLM context.
    SendMessage {
        message: ExtensionMessage,
        options: SendOptions,
    },
    /// Send a user message.
    SendUserMessage {
        content: String,
        options: SendOptions,
    },
    /// Set the session name.
    SetName { name: String },
}

/// Channel-based session handle.
///
/// Sends session requests to the pager's session manager.
pub struct PagerSessionHandle {
    request_tx: mpsc::UnboundedSender<SessionRequest>,
    session_id: Option<String>,
    session_file: Option<PathBuf>,
}

impl PagerSessionHandle {
    /// Create a new session handle.
    ///
    /// Returns the handle and a receiver for session requests.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<SessionRequest>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                request_tx: tx,
                session_id: None,
                session_file: None,
            },
            rx,
        )
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, id: String) -> Self {
        self.session_id = Some(id);
        self
    }

    /// Set the session file path.
    pub fn with_session_file(mut self, path: PathBuf) -> Self {
        self.session_file = Some(path);
        self
    }
}

impl SessionHandle for PagerSessionHandle {
    fn append_entry(&self, custom_type: &str, data: serde_json::Value) {
        let _ = self.request_tx.send(SessionRequest::AppendEntry {
            custom_type: custom_type.to_string(),
            data,
        });
    }

    fn get_entries(&self) -> Vec<SessionEntry> {
        // In a real implementation, this would query the session journal.
        // For now, return empty since we can't synchronously access the journal.
        Vec::new()
    }

    fn set_name(&self, name: &str) {
        let _ = self.request_tx.send(SessionRequest::SetName {
            name: name.to_string(),
        });
    }

    fn name(&self) -> Option<String> {
        // In a real implementation, this would read from the session state.
        None
    }

    fn session_file(&self) -> Option<PathBuf> {
        self.session_file.clone()
    }

    fn send_message(&self, message: ExtensionMessage, options: SendOptions) {
        let _ = self.request_tx.send(SessionRequest::SendMessage {
            message,
            options,
        });
    }

    fn send_user_message(&self, content: &str, options: SendOptions) {
        let _ = self.request_tx.send(SessionRequest::SendUserMessage {
            content: content.to_string(),
            options,
        });
    }
}
