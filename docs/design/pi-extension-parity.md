# Pi Extension SDK → Grok Build Extension System

> **Companion to:** [extensions.md](./extensions.md)
> **Status:** Design proposal
> **Date:** 2025-07-18

This document maps every pi extension SDK capability to what grok-build would
need to support equivalent functionality.

## Capability Matrix

| Pi Capability | Pi API | Grok Build Equivalent | Status |
|---------------|--------|----------------------|--------|
| **Tools** | `pi.registerTool()` | `TOOL_PACKS` + `builder.register::<T>()` | Exists, zero consumers |
| **Providers** | `pi.registerProvider()` | `PROVIDER_PACKS` + `ProviderRegistry` | New seam needed |
| **Hooks** | `pi.on("tool_call", ...)` | `HOOK_PACKS` + `HookRegistry` | New seam needed |
| **Commands** | `pi.registerCommand()` | `COMMAND_PACKS` + `CommandRegistry` | New seam needed |
| **UI: Select** | `ctx.ui.select()` | `UiHandle::select()` | New trait needed |
| **UI: Confirm** | `ctx.ui.confirm()` | `UiHandle::confirm()` | New trait needed |
| **UI: Input** | `ctx.ui.input()` | `UiHandle::input()` | New trait needed |
| **UI: Notify** | `ctx.ui.notify()` | `UiHandle::notify()` | New trait needed |
| **UI: Status** | `ctx.ui.setStatus()` | `UiHandle::set_status()` | New trait needed |
| **UI: Widget** | `ctx.ui.setWidget()` | `UiHandle::set_widget()` | New trait needed |
| **UI: Custom** | `ctx.ui.custom()` | `UiHandle::custom_component()` | New trait needed |
| **Session State** | `pi.appendEntry()` | `SessionHandle::append_entry()` | New trait needed |
| **Session Name** | `pi.setSessionName()` | `SessionHandle::set_name()` | New trait needed |
| **Context Injection** | `before_agent_start` | `HookRegistry::on_before_agent_start()` | New hook event |
| **System Prompt** | `ctx.getSystemPrompt()` | `AgentHandle::system_prompt()` | New trait needed |
| **Header Mutation** | `before_provider_headers` | `HookRegistry::on_before_provider_request()` | New hook event |
| **Payload Mutation** | `before_provider_request` | `HookRegistry::on_before_provider_request()` | New hook event |
| **Custom Rendering** | `renderCall`/`renderResult` | `ToolMetadata::render_call()`/`render_result()` | Extend trait |
| **Shortcuts** | `pi.registerShortcut()` | `COMMAND_PACKS` (with keybinding) | New seam needed |
| **Flags** | `pi.registerFlag()` | `COMMAND_PACKS` (with flag type) | New seam needed |
| **Dynamic Tools** | `pi.setActiveTools()` | `ToolBridge::set_active_tools()` | Expose existing |
| **Message Sending** | `pi.sendMessage()` | `SessionHandle::send_message()` | New trait needed |
| **Provider Unregister** | `pi.unregisterProvider()` | `ProviderRegistry::unregister()` | Add method |

## Architecture: The `ExtensionContext` Trait

Pi's `ctx` object bundles UI, session, model, and signal access into one context
passed to every handler. Grok Build needs the same pattern — a trait that
extensions receive in tool execution, hook callbacks, and command handlers.

```rust
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

    /// Abort signal for the current agent turn, if any.
    fn signal(&self) -> Option<&tokio_util::sync::CancellationToken>;
}
```

## UI Interactions

Pi's `ctx.ui` provides dialogs, notifications, status bars, widgets, and custom
TUI components. Grok Build's TUI is ratatui-based, so the UI handle needs to
bridge into the ratatui rendering loop.

### The `UiHandle` Trait

```rust
/// UI interaction handle. Modeled after pi's `ctx.ui`.
///
/// In TUI mode, methods are backed by the ratatui rendering loop.
/// In headless/JSON mode, methods are no-ops or return defaults.
pub trait UiHandle: Send + Sync {
    /// Show a selection dialog. Returns the chosen index, or None on cancel.
    fn select(&self, title: &str, options: &[&str]) -> BoxFuture<Option<usize>>;

    /// Show a confirmation dialog. Returns true if confirmed.
    fn confirm(&self, title: &str, body: &str) -> BoxFuture<bool>;

    /// Show a text input dialog. Returns the entered text, or None on cancel.
    fn input(&self, title: &str, placeholder: &str) -> BoxFuture<Option<String>>;

    /// Show a non-blocking notification.
    fn notify(&self, message: &str, level: NotifyLevel);

    /// Set a persistent status in the footer. Pass None to clear.
    fn set_status(&self, key: &str, message: Option<&str>);

    /// Set a widget above/below the editor.
    fn set_widget(&self, key: &str, lines: Option<Vec<String>>);

    /// Show a custom ratatui component. Blocks until `done()` is called.
    ///
    /// The closure receives a `CustomComponentCtx` with the terminal size,
    /// theme, and a `done` callback. It returns a `Box<dyn ratatui::Widget>`.
    ///
    /// Only available in TUI mode. Returns None in other modes.
    fn custom_component<F, R>(&self, render: F) -> BoxFuture<Option<R>>
    where
        F: FnOnce(&CustomComponentCtx) -> R + Send + 'static,
        R: Send + 'static;

    /// Access the current theme for styled output.
    fn theme(&self) -> &dyn Theme;
}

pub enum NotifyLevel {
    Info,
    Warning,
    Error,
}

pub struct CustomComponentCtx {
    /// Terminal width/height.
    pub terminal_size: (u16, u16),
    /// Theme for styling.
    pub theme: Box<dyn Theme>,
    /// Call this to close the component and return a value.
    pub done: Box<dyn FnOnce() + Send>,
}
```

### Implementation: Bridging to ratatui

The TUI mode implementation would use a channel-based approach:

```
Extension calls ctx.ui.confirm("Delete?", "Are you sure?")
  │
  ▼
UiHandle implementation sends a UiRequest::Confirm { title, body }
  │
  ▼
TUI event loop receives the request, renders a modal dialog
  │
  ▼
User presses Enter (confirm) or Escape (cancel)
  │
  ▼
TUI event loop sends the response back
  │
  ▼
UiHandle implementation resolves the future with the response
```

This is the same pattern pi uses — the extension runtime and the TUI runtime
communicate via channels, with the TUI owning the actual rendering.

## Session State

Pi's `pi.appendEntry()` persists extension data that survives restarts. Grok
Build's session is stored in a journal (`xai-sqlite-journal`), so the session
handle would write custom entries there.

```rust
/// Session state handle. Modeled after pi's `ctx.sessionManager`.
pub trait SessionHandle: Send + Sync {
    /// Persist a custom entry. Does not participate in LLM context.
    fn append_entry(&self, custom_type: &str, data: serde_json::Value);

    /// Get all entries in the current branch.
    fn get_entries(&self) -> Vec<SessionEntry>;

    /// Set the session display name.
    fn set_name(&self, name: &str);

    /// Get the session display name.
    fn name(&self) -> Option<&str>;

    /// Send a message to the LLM context.
    fn send_message(&self, message: ExtensionMessage, options: SendOptions);

    /// Send a user message (as if typed by the user).
    fn send_user_message(&self, content: &str, options: SendOptions);
}

pub struct ExtensionMessage {
    pub custom_type: String,
    pub content: String,
    pub display: bool,
    pub details: serde_json::Value,
}

pub struct SendOptions {
    pub deliver_as: DeliverMode,
    pub trigger_turn: bool,
}

pub enum DeliverMode {
    /// Queue while streaming, deliver after current tool batch.
    Steer,
    /// Wait for agent to finish all tools.
    FollowUp,
    /// Queue for next user prompt.
    NextTurn,
}
```

## Agent Access

Pi's `ctx.getSystemPrompt()` and `pi.getActiveTools()` give extensions access
to the agent state. Grok Build's `Agent` already holds the system prompt and
tool bridge.

```rust
/// Agent state handle.
pub trait AgentHandle: Send + Sync {
    /// The current rendered system prompt.
    fn system_prompt(&self) -> &str;

    /// Currently active tool names.
    fn active_tools(&self) -> Vec<&str>;

    /// All registered tool names (including inactive).
    fn all_tools(&self) -> Vec<&str>;

    /// Enable/disable tools dynamically.
    fn set_active_tools(&self, names: &[&str]);

    /// Set the current model.
    fn set_model(&self, model: &str) -> bool;

    /// Get the current thinking level.
    fn thinking_level(&self) -> &str;

    /// Set the thinking level.
    fn set_thinking_level(&self, level: &str);
}
```

## Commands, Shortcuts, and Flags

Pi's `pi.registerCommand()`, `pi.registerShortcut()`, and `pi.registerFlag()`
all follow the same pattern — register a handler that the shell/pager invokes
at the right time. Grok Build already has slash commands in the pager; this
would extend that system.

```rust
/// Command registration in a command pack.
pub struct CommandRegistry {
    commands: Vec<CommandDef>,
    shortcuts: Vec<ShortcutDef>,
    flags: Vec<FlagDef>,
}

pub struct CommandDef {
    pub name: String,
    pub description: String,
    pub handler: Box<dyn Fn(&str, &dyn ExtensionContext) -> BoxFuture<()> + Send + Sync>,
    pub autocomplete: Option<Box<dyn Fn(&str) -> Vec<AutocompleteItem> + Send + Sync>>,
}

pub struct ShortcutDef {
    pub key: String,  // e.g., "ctrl+shift+p"
    pub description: String,
    pub handler: Box<dyn Fn(&dyn ExtensionContext) -> BoxFuture<()> + Send + Sync>,
}

pub struct FlagDef {
    pub name: String,
    pub description: String,
    pub flag_type: FlagType,
    pub default: serde_json::Value,
}

pub enum FlagType {
    Boolean,
    String,
    Integer,
}
```

Registration:

```rust
#[ctor::ctor]
fn init() {
    register_command_pack(|registry| {
        registry.register_command(CommandDef {
            name: "stats".into(),
            description: "Show session statistics".into(),
            handler: Box::new(|args, ctx| {
                Box::pin(async move {
                    let count = ctx.session().get_entries().len();
                    ctx.ui().notify(&format!("{} entries", count), NotifyLevel::Info);
                })
            }),
            autocomplete: None,
        });
    });
}
```

## Custom Rendering

Pi extensions can provide `renderCall` and `renderResult` for custom TUI
display of tool calls. Grok Build's TUI is ratatui-based, so the rendering
trait would return ratatui widgets.

```rust
/// Extended tool metadata for custom rendering.
///
/// Optional — tools that don't implement this use the default renderer.
pub trait ToolRenderer: Send + Sync {
    /// Render the tool call header (name + arguments).
    ///
    /// Returns a ratatui `Widget` or `None` for default rendering.
    fn render_call(
        &self,
        args: &serde_json::Value,
        theme: &dyn Theme,
        expanded: bool,
    ) -> Option<Box<dyn ratatui::Widget>>;

    /// Render the tool result.
    ///
    /// Returns a ratatui `Widget` or `None` for default rendering.
    fn render_result(
        &self,
        result: &serde_json::Value,
        theme: &dyn Theme,
        expanded: bool,
        is_partial: bool,
    ) -> Option<Box<dyn ratatui::Widget>>;
}
```

## Context Injection & Prompt Modification

Pi's `before_agent_start` event lets extensions inject messages and modify the
system prompt. This is the most powerful hook — it's how extensions change the
agent's behavior per-turn.

```rust
// In the HookRegistry:
pub struct BeforeAgentStartEvent {
    pub prompt: String,
    pub system_prompt: String,
}

pub struct BeforeAgentStartResult {
    /// Inject a message into the LLM context.
    pub message: Option<ExtensionMessage>,
    /// Modify the system prompt (chained across extensions).
    pub system_prompt: Option<String>,
}

impl HookRegistry {
    pub fn on_before_agent_start(
        &mut self,
        handler: Box<dyn Fn(&BeforeAgentStartEvent) -> BeforeAgentStartResult + Send + Sync>,
    );
}
```

## Provider Request Mutation

Pi's `before_provider_headers` and `before_provider_request` let extensions
modify outgoing HTTP requests. This is useful for corporate proxies, custom
auth headers, and request logging.

```rust
// In the HookRegistry:
pub struct BeforeProviderRequestEvent {
    pub model: String,
    pub provider: String,
    pub headers: std::collections::HashMap<String, String>,
    pub payload: serde_json::Value,
}

impl HookRegistry {
    pub fn on_before_provider_request(
        &mut self,
        handler: Box<dyn Fn(&mut BeforeProviderRequestEvent) + Send + Sync>,
    );
}
```

## Full Pack Registration Summary

The complete set of pack types an extension crate can register:

```rust
// Tools
register_tool_pack(|builder: &mut ToolRegistryBuilder| { ... });

// Providers
register_provider_pack(|registry: &mut ProviderRegistry| { ... });

// Hooks (events)
register_hook_pack(|registry: &mut HookRegistry| { ... });

// Commands, shortcuts, flags
register_command_pack(|registry: &mut CommandRegistry| { ... });
```

A single extension crate typically registers multiple packs:

```rust
#[ctor::ctor]
fn init() {
    register_tool_pack(|builder| {
        builder.register::<MyTool>();
    });
    register_provider_pack(|registry| {
        registry.register(my_provider_config());
    });
    register_hook_pack(|registry| {
        registry.on_pre_tool_use(Box::new(|event| { ... }));
        registry.on_before_agent_start(Box::new(|event| { ... }));
    });
    register_command_pack(|registry| {
        registry.register_command(my_command());
    });
}
```

## Implementation Phases

### Phase 1: Tools (existing)
- `TOOL_PACKS` already works. Document the pattern. Build a test extension.

### Phase 2: Providers + Hooks
- Add `PROVIDER_PACKS` and `HOOK_PACKS` seams.
- Add `ProviderRegistry`, `HookRegistry`, event types.
- Wire into model resolver and hooks dispatcher.

### Phase 3: UI Handle
- Define the `UiHandle` trait.
- Implement TUI backend (channel-based bridge to ratatui event loop).
- Implement headless/no-op backend for JSON/headless mode.
- Wire into `ToolCallContext` extensions so tools can access `ctx.ui()`.

### Phase 4: Session + Agent Handle
- Define `SessionHandle` and `AgentHandle` traits.
- Wire into the existing `Agent`, `ToolBridge`, and session journal.
- Expose via `ToolCallContext` extensions.

### Phase 5: Commands + Rendering
- Add `COMMAND_PACKS` seam.
- Extend `ToolMetadata` with optional `ToolRenderer` trait.
- Wire into the pager's command dispatch and scrollback rendering.

## Open Questions

1. **Ratatui widget ownership.** `render_call`/`render_result` return
   `Box<dyn Widget>`, but ratatui's `Widget` trait takes `&self` by value.
   Need to decide between `Widget` (consumed each frame) vs `StatefulWidget`
   (persistent state across frames).

2. **Custom component lifecycle.** Pi's `ctx.ui.custom()` temporarily replaces
   the editor. In ratatui terms, this means pushing a new `Widget` onto a
   component stack. Need to define the stack protocol and focus management.

3. **Async in hook callbacks.** Pi's hooks are async (can call `fetch`, etc.).
   Grok Build's hooks should support async too, but the pre-tool-use hook
   needs to complete before the tool runs. Use `BoxFuture` for hooks that
   need async, synchronous closures for fast checks.

4. **Extension configuration.** Should extensions read from `config.toml`?
   A `[extensions.my-ext]` section? Or should extensions manage their own
   config files?

5. **Hot reload.** Pi supports `/reload` for extensions. Grok Build's
   compile-time extensions can't hot-reload. Should we support a
   `/reload` for the dynamic surfaces (providers, hooks) even if tools
   require recompilation?

## Reference

- [Pi Extension SDK](https://pi.earendil.works/docs/extensions) — full API reference
- [Pi Custom Providers](https://pi.earendil.works/docs/custom-provider) — provider registration
- [Pi TUI Components](https://pi.earendil.works/docs/tui) — custom rendering
- [extensions.md](./extensions.md) — grok-build extension system design
