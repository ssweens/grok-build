//! Hook types for the extension API.
//!
//! Extensions register event callbacks via [`HookRegistry`].
//! The hooks dispatcher calls these alongside command-based hooks from
//! `xai-grok-hooks`.
//!
//! ## Execution Order
//!
//! 1. Command-based hooks from `xai-grok-hooks` (existing behavior)
//! 2. Extension hook callbacks from `HOOK_PACKS` (this module)
//!
//! Both paths produce `HookAction::Allow | Block`. If any hook blocks,
//! the tool call is denied.

use std::path::PathBuf;

/// An event emitted before a tool executes.
///
/// Extensions receive this in `on_pre_tool_use` callbacks and can
/// inspect the tool name and input to decide whether to allow or block.
#[derive(Debug, Clone)]
pub struct PreToolUseEvent {
    /// Name of the tool about to execute (e.g., "bash", "read_file").
    pub tool_name: String,

    /// The tool's input parameters as JSON.
    pub input: serde_json::Value,

    /// Current working directory.
    pub cwd: PathBuf,
}

/// An event emitted after a tool executes.
///
/// Extensions receive this in `on_post_tool_use` callbacks for
/// observation, logging, or side effects. Cannot block (the tool
/// already ran).
#[derive(Debug, Clone)]
pub struct PostToolUseEvent {
    /// Name of the tool that executed.
    pub tool_name: String,

    /// The tool's input parameters as JSON.
    pub input: serde_json::Value,

    /// The tool's output as JSON.
    pub output: serde_json::Value,

    /// Whether the tool execution resulted in an error.
    pub is_error: bool,

    /// Current working directory.
    pub cwd: PathBuf,
}

/// An event emitted when a session starts.
#[derive(Debug, Clone)]
pub struct SessionStartEvent {
    /// The current working directory.
    pub cwd: PathBuf,

    /// Session start reason.
    pub reason: SessionStartReason,
}

/// Why a session is starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStartReason {
    /// Fresh startup.
    Startup,
    /// New session within an existing process.
    New,
    /// Resuming a previous session.
    Resume,
    /// Extension reload.
    Reload,
}

/// An event emitted before the agent starts processing a prompt.
///
/// Extensions can inject messages and modify the system prompt.
#[derive(Debug, Clone)]
pub struct BeforeAgentStartEvent {
    /// The user's prompt text.
    pub prompt: String,

    /// The current system prompt (may have been modified by earlier hooks).
    pub system_prompt: String,

    /// Current working directory.
    pub cwd: PathBuf,
}

/// Result from a `before_agent_start` hook.
#[derive(Debug, Clone, Default)]
pub struct BeforeAgentStartResult {
    /// Inject a message into the LLM context.
    pub message: Option<ExtensionMessage>,

    /// Modify the system prompt (chained across extensions).
    pub system_prompt: Option<String>,
}

/// A message that can be injected into the LLM context.
#[derive(Debug, Clone)]
pub struct ExtensionMessage {
    /// Extension-defined type for routing and rendering.
    pub custom_type: String,

    /// Message content (sent to the LLM).
    pub content: String,

    /// Whether to display in the TUI.
    pub display: bool,

    /// Additional structured data (for rendering, not sent to LLM).
    pub details: serde_json::Value,
}

/// An event emitted before a provider request is sent.
///
/// Extensions can mutate headers and payload.
#[derive(Debug, Clone)]
pub struct BeforeProviderRequestEvent {
    /// The model being requested.
    pub model: String,

    /// The provider name.
    pub provider: String,

    /// Request headers (mutable).
    pub headers: std::collections::HashMap<String, String>,

    /// The request payload as JSON (mutable).
    pub payload: serde_json::Value,
}

/// The action a hook callback returns.
#[derive(Debug, Clone)]
pub enum HookAction {
    /// Allow the operation to proceed.
    Allow,

    /// Block the operation with a reason shown to the model.
    Block { reason: String },
}

impl HookAction {
    /// Whether this action blocks the operation.
    pub fn is_blocked(&self) -> bool {
        matches!(self, HookAction::Block { .. })
    }

    /// Extract the block reason, if any.
    pub fn block_reason(&self) -> Option<&str> {
        match self {
            HookAction::Block { reason } => Some(reason),
            _ => None,
        }
    }
}

/// Registry of extension-provided event callbacks.
///
/// Populated by hook packs at startup. The hooks dispatcher queries this
/// alongside command-based hooks from `xai-grok-hooks`.
pub struct HookRegistry {
    pre_tool_use: Vec<Box<dyn Fn(&PreToolUseEvent) -> HookAction + Send + Sync>>,
    post_tool_use: Vec<Box<dyn Fn(&PostToolUseEvent) + Send + Sync>>,
    session_start: Vec<Box<dyn Fn(&SessionStartEvent) + Send + Sync>>,
    before_agent_start: Vec<Box<dyn Fn(&BeforeAgentStartEvent) -> BeforeAgentStartResult + Send + Sync>>,
    before_provider_request: Vec<Box<dyn Fn(&mut BeforeProviderRequestEvent) + Send + Sync>>,
}

impl HookRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            pre_tool_use: Vec::new(),
            post_tool_use: Vec::new(),
            session_start: Vec::new(),
            before_agent_start: Vec::new(),
            before_provider_request: Vec::new(),
        }
    }

    /// Register a pre-tool-use callback.
    ///
    /// Called before each tool execution. If any callback returns
    /// `HookAction::Block`, the tool call is denied and the reason
    /// is shown to the model.
    ///
    /// Callbacks run in registration order. The first `Block` wins.
    pub fn on_pre_tool_use(
        &mut self,
        handler: Box<dyn Fn(&PreToolUseEvent) -> HookAction + Send + Sync>,
    ) {
        self.pre_tool_use.push(handler);
    }

    /// Register a post-tool-use callback.
    ///
    /// Called after each tool execution, regardless of success/failure.
    /// Cannot block (the tool already ran).
    pub fn on_post_tool_use(
        &mut self,
        handler: Box<dyn Fn(&PostToolUseEvent) + Send + Sync>,
    ) {
        self.post_tool_use.push(handler);
    }

    /// Register a session-start callback.
    ///
    /// Called once when a new session begins.
    pub fn on_session_start(
        &mut self,
        handler: Box<dyn Fn(&SessionStartEvent) + Send + Sync>,
    ) {
        self.session_start.push(handler);
    }

    /// Register a before-agent-start callback.
    ///
    /// Called before the agent processes a prompt. Can inject messages
    /// and modify the system prompt. Chained across extensions.
    pub fn on_before_agent_start(
        &mut self,
        handler: Box<dyn Fn(&BeforeAgentStartEvent) -> BeforeAgentStartResult + Send + Sync>,
    ) {
        self.before_agent_start.push(handler);
    }

    /// Register a before-provider-request callback.
    ///
    /// Called before an API request is sent. Can mutate headers and payload.
    pub fn on_before_provider_request(
        &mut self,
        handler: Box<dyn Fn(&mut BeforeProviderRequestEvent) + Send + Sync>,
    ) {
        self.before_provider_request.push(handler);
    }

    /// Run all pre-tool-use hooks. Returns `Block` if any hook blocks.
    pub fn dispatch_pre_tool_use(&self, event: &PreToolUseEvent) -> HookAction {
        for handler in &self.pre_tool_use {
            let action = handler(event);
            if action.is_blocked() {
                return action;
            }
        }
        HookAction::Allow
    }

    /// Run all post-tool-use hooks.
    pub fn dispatch_post_tool_use(&self, event: &PostToolUseEvent) {
        for handler in &self.post_tool_use {
            handler(event);
        }
    }

    /// Run all session-start hooks.
    pub fn dispatch_session_start(&self, event: &SessionStartEvent) {
        for handler in &self.session_start {
            handler(event);
        }
    }

    /// Run all before-agent-start hooks, chaining results.
    pub fn dispatch_before_agent_start(&self, event: &BeforeAgentStartEvent) -> BeforeAgentStartResult {
        let mut result = BeforeAgentStartResult::default();
        let mut current_event = event.clone();

        for handler in &self.before_agent_start {
            let handler_result = handler(&current_event);

            // Chain system prompt modifications
            if let Some(new_prompt) = handler_result.system_prompt {
                current_event.system_prompt = new_prompt.clone();
                result.system_prompt = Some(new_prompt);
            }

            // Collect messages (last one wins if multiple)
            if let Some(msg) = handler_result.message {
                result.message = Some(msg);
            }
        }

        result
    }

    /// Run all before-provider-request hooks.
    pub fn dispatch_before_provider_request(&self, event: &mut BeforeProviderRequestEvent) {
        for handler in &self.before_provider_request {
            handler(event);
        }
    }

    /// Whether any pre-tool-use hooks are registered.
    pub fn has_pre_tool_use_hooks(&self) -> bool {
        !self.pre_tool_use.is_empty()
    }

    /// Whether any post-tool-use hooks are registered.
    pub fn has_post_tool_use_hooks(&self) -> bool {
        !self.post_tool_use.is_empty()
    }

    /// Whether any session-start hooks are registered.
    pub fn has_session_start_hooks(&self) -> bool {
        !self.session_start.is_empty()
    }

    /// Whether any before-agent-start hooks are registered.
    pub fn has_before_agent_start_hooks(&self) -> bool {
        !self.before_agent_start.is_empty()
    }

    /// Whether any before-provider-request hooks are registered.
    pub fn has_before_provider_request_hooks(&self) -> bool {
        !self.before_provider_request.is_empty()
    }

    /// Whether any hooks are registered at all.
    pub fn is_empty(&self) -> bool {
        self.pre_tool_use.is_empty()
            && self.post_tool_use.is_empty()
            && self.session_start.is_empty()
            && self.before_agent_start.is_empty()
            && self.before_provider_request.is_empty()
    }
}
