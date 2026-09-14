//! # xai-grok-extension-api
//!
//! Public API for grok-build extensions. This crate is a thin facade that
//! defines the traits and registration functions for the four extension
//! surfaces: tools, providers, hooks, and commands.
//!
//! ## Extension Surfaces
//!
//! | Surface | Registration Function | Pack Type | When Consumed |
//! |---------|----------------------|-----------|---------------|
//! | Tools | [`register_tool_pack`] | [`ToolPack`] | `ToolRegistryBuilder::new()` |
//! | Providers | [`register_provider_pack`] | [`ProviderPack`] | Model resolution |
//! | Hooks | [`register_hook_pack`] | [`HookPack`] | Hooks dispatcher |
//! | Commands | [`register_command_pack`] | [`CommandPack`] | Pager command dispatch |
//!
//! ## Usage
//!
//! Extensions depend on this crate, implement the traits, and self-register
//! via `#[ctor]`:
//!
//! ```rust,ignore
//! use xai_grok_extension_api::prelude::*;
//!
//! #[derive(Debug, Default)]
//! pub struct MyTool;
//!
//! // ... implement ExtensionTool + ExtensionToolMetadata ...
//!
//! #[ctor::ctor]
//! fn init() {
//!     register_tool_pack(|builder| {
//!         builder.register::<MyTool>();
//!     });
//! }
//! ```

use std::sync::OnceLock;

use parking_lot::Mutex;

// ── Re-exports ──────────────────────────────────────────────────────────────

/// Tool runtime types (Tool trait, ToolCallContext, ToolError, ListToolsContext).
pub use xai_tool_runtime;

/// Tool protocol types (ToolId, ToolCapabilities, ToolScope).
pub use xai_tool_protocol;

/// Tool description types (ToolDescription).
pub use xai_tool_types;

/// Schema generation.
pub use schemars;

/// Serialization.
pub use serde;

/// Futures for async handlers.
pub use futures;

// ── Submodules ──────────────────────────────────────────────────────────────

pub mod provider;
pub mod hook;
pub mod command;
pub mod ui;
pub mod session;
pub mod agent;
pub mod context;
pub mod theme;
pub mod types;
pub mod tool;

// ── Convenience re-exports ──────────────────────────────────────────────────

pub use provider::{ApiBackend, ModelConfig, ProviderConfig, ProviderRegistry};
pub use hook::{
    HookAction, HookRegistry, PreToolUseEvent, PostToolUseEvent,
    BeforeAgentStartEvent, BeforeAgentStartResult, BeforeProviderRequestEvent,
    SessionStartEvent,
};
pub use command::{CommandDef, CommandRegistry, FlagDef, FlagType, ShortcutDef};
pub use ui::{UiHandle, NotifyLevel, CustomComponentCtx, RunMode};
pub use session::{SessionHandle, ExtensionMessage, SendOptions, DeliverMode, SessionEntry};
pub use agent::AgentHandle;
pub use context::ExtensionContext;
pub use theme::Theme;
pub use types::AutocompleteItem;
pub use tool::{
    ExtensionTool, ExtensionToolMetadata, ExtensionToolRegistry,
    ExtensionToolKind, ExtensionToolNamespace,
};

// ── Convenience re-export of the prelude ────────────────────────────────────

/// Convenience re-export of the most common imports.
pub mod prelude {
    pub use super::{
        register_command_pack, register_hook_pack, register_provider_pack, register_tool_pack,
        agent::AgentHandle,
        command::{CommandDef, CommandRegistry, FlagDef, FlagType, ShortcutDef},
        context::ExtensionContext,
        hook::{
            BeforeAgentStartEvent, BeforeAgentStartResult, BeforeProviderRequestEvent,
            HookAction, HookRegistry, PostToolUseEvent, PreToolUseEvent, SessionStartEvent,
        },
        provider::{ApiBackend, ModelConfig, ProviderConfig, ProviderRegistry},
        session::{DeliverMode, ExtensionMessage, SendOptions, SessionEntry, SessionHandle},
        theme::Theme,
        tool::{
            ExtensionTool, ExtensionToolMetadata, ExtensionToolRegistry,
            ExtensionToolKind, ExtensionToolNamespace,
        },
        types::AutocompleteItem,
        ui::{CustomComponentCtx, NotifyLevel, RunMode, UiHandle},
    };
    pub use super::xai_tool_runtime::Tool;
    pub use super::xai_tool_protocol::{ToolCapabilities, ToolId};
    pub use super::xai_tool_runtime::{ListToolsContext, ToolCallContext, ToolError};
    pub use super::xai_tool_types::ToolDescription;
    pub use super::schemars::JsonSchema;
    pub use super::serde::{Deserialize, Serialize};
}

// ── Tool pack registration ──────────────────────────────────────────────────

/// A tool pack: a function that contributes tool registrations to a builder.
///
/// The pack function receives a `&mut ExtensionToolRegistry` and can call
/// `builder.register::<T>()` for any tool that implements [`ExtensionTool`]
/// and [`ExtensionToolMetadata`].
///
/// # Ordering Contract
///
/// MUST be called before the first `ExtensionToolRegistry::new()` in the process.
/// Packs registered after a builder has been constructed do not retroactively
/// apply to that builder.
pub type ToolPack = fn(&mut ExtensionToolRegistry);

static TOOL_PACKS: OnceLock<Mutex<Vec<ToolPack>>> = OnceLock::new();

fn tool_packs() -> &'static Mutex<Vec<ToolPack>> {
    TOOL_PACKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register an out-of-tree tool pack.
///
/// See [`ToolPack`] for the ordering contract.
pub fn register_tool_pack(pack: ToolPack) {
    tool_packs().lock().push(pack);
}

/// Drain registered tool packs and collect all extension tools.
///
/// Called by the runtime crate during `ToolRegistryBuilder::new()`.
/// Returns a populated `ExtensionToolRegistry` with all extension tools.
pub fn drain_extension_tools() -> ExtensionToolRegistry {
    let mut registry = ExtensionToolRegistry::new();
    for pack in TOOL_PACKS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .drain(..)
    {
        pack(&mut registry);
    }
    registry
}

// ── Provider pack registration ──────────────────────────────────────────────

/// A provider pack: a function that contributes model/provider registrations.
pub type ProviderPack = fn(&mut ProviderRegistry);

static PROVIDER_PACKS: OnceLock<Mutex<Vec<ProviderPack>>> = OnceLock::new();

fn provider_packs() -> &'static Mutex<Vec<ProviderPack>> {
    PROVIDER_PACKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register an out-of-tree provider pack.
pub fn register_provider_pack(pack: ProviderPack) {
    provider_packs().lock().push(pack);
}

/// Drain registered provider packs. Called by the runtime crate.
pub fn drain_provider_packs() -> Vec<ProviderPack> {
    PROVIDER_PACKS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .drain(..)
        .collect()
}

/// Collect all registered provider packs into a single `ProviderRegistry`.
pub fn collect_providers() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    for pack in provider_packs().lock().iter() {
        pack(&mut registry);
    }
    registry
}

/// Refresh extension providers by re-running all registered packs.
///
/// Called by `/reload` to pick up changes from config files (e.g.,
/// `~/.grok/settings/dynamic-models.json`) or servers that came online.
///
/// Returns a new `ProviderRegistry` with refreshed providers.
pub fn refresh_providers() -> ProviderRegistry {
    tracing::info!("extension: refreshing providers");
    collect_providers()
}

// ── Hook pack registration ──────────────────────────────────────────────────

/// A hook pack: a function that contributes event callbacks.
pub type HookPack = fn(&mut HookRegistry);

static HOOK_PACKS: OnceLock<Mutex<Vec<HookPack>>> = OnceLock::new();

fn hook_packs() -> &'static Mutex<Vec<HookPack>> {
    HOOK_PACKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register an out-of-tree hook pack.
pub fn register_hook_pack(pack: HookPack) {
    hook_packs().lock().push(pack);
}

/// Drain registered hook packs. Called by the runtime crate.
pub fn drain_hook_packs() -> Vec<HookPack> {
    HOOK_PACKS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .drain(..)
        .collect()
}

/// Collect all registered hook packs into a single `HookRegistry`.
pub fn collect_hooks() -> HookRegistry {
    let mut registry = HookRegistry::new();
    for pack in hook_packs().lock().iter() {
        pack(&mut registry);
    }
    registry
}

// ── Extension hook dispatch helpers ─────────────────────────────────────────

use std::path::Path;
use parking_lot::RwLock;

/// Global hook registry, populated once at startup.
static EXTENSION_HOOKS: OnceLock<RwLock<Option<HookRegistry>>> = OnceLock::new();

/// Initialize the global extension hooks from registered packs.
/// Called once at startup by the runtime.
pub fn init_extension_hooks() {
    let registry = collect_hooks();
    let storage = EXTENSION_HOOKS.get_or_init(|| RwLock::new(None));
    *storage.write() = Some(registry);
}

/// Check if any extension pre-tool-use hooks are registered.
pub fn has_pre_tool_use_hooks() -> bool {
    let storage = EXTENSION_HOOKS.get_or_init(|| RwLock::new(None));
    storage
        .read()
        .as_ref()
        .map(|r| r.has_pre_tool_use_hooks())
        .unwrap_or(false)
}

/// Dispatch pre-tool-use hooks from extensions.
/// Returns `HookAction::Allow` if no hooks block.
pub fn dispatch_pre_tool_use(
    tool_name: &str,
    input: &serde_json::Value,
    cwd: &Path,
) -> HookAction {
    let storage = EXTENSION_HOOKS.get_or_init(|| RwLock::new(None));
    let guard = storage.read();
    if let Some(registry) = guard.as_ref() {
        let event = hook::PreToolUseEvent {
            tool_name: tool_name.to_string(),
            input: input.clone(),
            cwd: cwd.to_path_buf(),
        };
        registry.dispatch_pre_tool_use(&event)
    } else {
        HookAction::Allow
    }
}

// ── Command pack registration ───────────────────────────────────────────────

/// A command pack: a function that contributes commands, shortcuts, and flags.
pub type CommandPack = fn(&mut CommandRegistry);

static COMMAND_PACKS: OnceLock<Mutex<Vec<CommandPack>>> = OnceLock::new();

fn command_packs() -> &'static Mutex<Vec<CommandPack>> {
    COMMAND_PACKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register an out-of-tree command pack.
pub fn register_command_pack(pack: CommandPack) {
    command_packs().lock().push(pack);
}

/// Drain registered command packs. Called by the runtime crate.
pub fn drain_command_packs() -> Vec<CommandPack> {
    COMMAND_PACKS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .drain(..)
        .collect()
}

/// Collect all registered command packs into a single `CommandRegistry`.
pub fn collect_commands() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    for pack in command_packs().lock().iter() {
        pack(&mut registry);
    }
    registry
}
