//! Hook types for the extension API.
//!
//! Extensions register event callbacks via [`HookRegistry::on_pre_tool_use`] etc.
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
}

/// An event emitted when a session starts.
#[derive(Debug, Clone)]
pub struct SessionStartEvent {
    /// The current working directory.
    pub cwd: String,
}

/// The action a hook callback returns.
#[derive(Debug, Clone)]
pub enum HookAction {
    /// Allow the tool call to proceed.
    Allow,

    /// Block the tool call with a reason shown to the model.
    Block { reason: String },
}

impl HookAction {
    /// Whether this action blocks the tool call.
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
}

impl HookRegistry {
    /// Create an empty registry. Called by the extension API internally.
    pub fn new() -> Self {
        Self {
            pre_tool_use: Vec::new(),
            post_tool_use: Vec::new(),
            session_start: Vec::new(),
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
    /// Cannot block (the tool already ran). Use for observation, logging,
    /// or triggering side effects.
    pub fn on_post_tool_use(
        &mut self,
        handler: Box<dyn Fn(&PostToolUseEvent) + Send + Sync>,
    ) {
        self.post_tool_use.push(handler);
    }

    /// Register a session-start callback.
    ///
    /// Called once when a new session begins. Use for initialization,
    /// loading external state, or greeting the user.
    pub fn on_session_start(
        &mut self,
        handler: Box<dyn Fn(&SessionStartEvent) + Send + Sync>,
    ) {
        self.session_start.push(handler);
    }

    /// Run all pre-tool-use hooks. Returns `Block` if any hook blocks.
    ///
    /// Called by the hooks dispatcher after command-based hooks.
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
    ///
    /// Called by the hooks dispatcher after command-based hooks.
    pub fn dispatch_post_tool_use(&self, event: &PostToolUseEvent) {
        for handler in &self.post_tool_use {
            handler(event);
        }
    }

    /// Run all session-start hooks.
    ///
    /// Called by the hooks dispatcher after command-based hooks.
    pub fn dispatch_session_start(&self, event: &SessionStartEvent) {
        for handler in &self.session_start {
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
}
