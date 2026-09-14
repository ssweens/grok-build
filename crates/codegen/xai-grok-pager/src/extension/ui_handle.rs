//! Extension UI handle implementation.
//!
//! Bridges extension UI requests to the ratatui event loop via channels.
//! Extensions call methods on `UiHandle` which send requests through a channel;
//! the TUI event loop processes them and sends responses back.

use std::future::Future;
use std::pin::Pin;

use tokio::sync::{mpsc, oneshot};

use xai_grok_extension_api::ui::{NotifyLevel, RunMode, UiHandle};

/// UI request sent from extension to TUI event loop.
#[derive(Debug)]
pub enum UiRequest {
    /// Show a selection dialog.
    Select {
        title: String,
        options: Vec<String>,
        response: oneshot::Sender<Option<usize>>,
    },
    /// Show a confirmation dialog.
    Confirm {
        title: String,
        body: String,
        response: oneshot::Sender<bool>,
    },
    /// Show a text input dialog.
    Input {
        title: String,
        placeholder: String,
        response: oneshot::Sender<Option<String>>,
    },
    /// Show a non-blocking notification.
    Notify {
        message: String,
        level: NotifyLevel,
    },
    /// Set a persistent status in the footer.
    SetStatus {
        key: String,
        message: Option<String>,
    },
    /// Set a widget above/below the editor.
    SetWidget {
        key: String,
        lines: Option<Vec<String>>,
    },
}

/// Channel-based UI handle for TUI mode.
///
/// Sends UI requests to the ratatui event loop and awaits responses.
pub struct TuiUiHandle {
    request_tx: mpsc::UnboundedSender<UiRequest>,
    mode: RunMode,
}

impl TuiUiHandle {
    /// Create a new TUI UI handle.
    ///
    /// Returns the handle and a receiver for UI requests.
    /// The receiver should be processed by the TUI event loop.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<UiRequest>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                request_tx: tx,
                mode: RunMode::Tui,
            },
            rx,
        )
    }

    /// Create a UI handle for a specific mode.
    pub fn with_mode(mode: RunMode) -> (Self, Option<mpsc::UnboundedReceiver<UiRequest>>) {
        if matches!(mode, RunMode::Tui | RunMode::Rpc) {
            let (handle, rx) = Self::new();
            (Self { mode, ..handle }, Some(rx))
        } else {
            let (tx, _) = mpsc::unbounded_channel();
            (
                Self {
                    request_tx: tx,
                    mode,
                },
                None,
            )
        }
    }
}

impl UiHandle for TuiUiHandle {
    fn select<'a>(
        &'a self,
        title: &'a str,
        options: &'a [&str],
    ) -> Pin<Box<dyn Future<Output = Option<usize>> + Send + 'a>> {
        Box::pin(async move {
            if !self.has_ui() {
                return None;
            }
            let (tx, rx) = oneshot::channel();
            let request = UiRequest::Select {
                title: title.to_string(),
                options: options.iter().map(|s| s.to_string()).collect(),
                response: tx,
            };
            if self.request_tx.send(request).is_err() {
                return None;
            }
            rx.await.ok().flatten()
        })
    }

    fn confirm<'a>(
        &'a self,
        title: &'a str,
        body: &'a str,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            if !self.has_ui() {
                return true; // Default to confirmed in non-interactive mode
            }
            let (tx, rx) = oneshot::channel();
            let request = UiRequest::Confirm {
                title: title.to_string(),
                body: body.to_string(),
                response: tx,
            };
            if self.request_tx.send(request).is_err() {
                return true;
            }
            rx.await.unwrap_or(true)
        })
    }

    fn input<'a>(
        &'a self,
        title: &'a str,
        placeholder: &'a str,
    ) -> Pin<Box<dyn Future<Output = Option<String>> + Send + 'a>> {
        Box::pin(async move {
            if !self.has_ui() {
                return None;
            }
            let (tx, rx) = oneshot::channel();
            let request = UiRequest::Input {
                title: title.to_string(),
                placeholder: placeholder.to_string(),
                response: tx,
            };
            if self.request_tx.send(request).is_err() {
                return None;
            }
            rx.await.ok().flatten()
        })
    }

    fn notify(&self, message: &str, level: NotifyLevel) {
        if !self.has_ui() {
            return;
        }
        let _ = self.request_tx.send(UiRequest::Notify {
            message: message.to_string(),
            level,
        });
    }

    fn set_status(&self, key: &str, message: Option<&str>) {
        if !self.has_ui() {
            return;
        }
        let _ = self.request_tx.send(UiRequest::SetStatus {
            key: key.to_string(),
            message: message.map(|s| s.to_string()),
        });
    }

    fn set_widget(&self, key: &str, lines: Option<Vec<String>>) {
        if !self.has_ui() {
            return;
        }
        let _ = self.request_tx.send(UiRequest::SetWidget {
            key: key.to_string(),
            lines,
        });
    }

    fn has_ui(&self) -> bool {
        matches!(self.mode, RunMode::Tui | RunMode::Rpc)
    }

    fn mode(&self) -> RunMode {
        self.mode
    }
}

/// No-op UI handle for headless/JSON modes.
pub struct NoOpUiHandle {
    mode: RunMode,
}

impl NoOpUiHandle {
    pub fn new(mode: RunMode) -> Self {
        Self { mode }
    }
}

impl UiHandle for NoOpUiHandle {
    fn select<'a>(
        &'a self,
        _title: &'a str,
        _options: &'a [&str],
    ) -> Pin<Box<dyn Future<Output = Option<usize>> + Send + 'a>> {
        Box::pin(async { None })
    }

    fn confirm<'a>(
        &'a self,
        _title: &'a str,
        _body: &'a str,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async { true })
    }

    fn input<'a>(
        &'a self,
        _title: &'a str,
        _placeholder: &'a str,
    ) -> Pin<Box<dyn Future<Output = Option<String>> + Send + 'a>> {
        Box::pin(async { None })
    }

    fn notify(&self, _message: &str, _level: NotifyLevel) {}

    fn set_status(&self, _key: &str, _message: Option<&str>) {}

    fn set_widget(&self, _key: &str, _lines: Option<Vec<String>>) {}

    fn has_ui(&self) -> bool {
        false
    }

    fn mode(&self) -> RunMode {
        self.mode
    }
}
