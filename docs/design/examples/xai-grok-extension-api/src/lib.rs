//! # xai-grok-extension-api
//!
//! Public API for grok-build extensions. This crate is a thin facade that
//! re-exports the traits and types extensions need, plus registration functions
//! for the three extension surfaces: tools, providers, and hooks.
//!
//! ## Extension Surfaces
//!
//! | Surface | Registration Function | Pack Type | When Consumed |
//! |---------|----------------------|-----------|---------------|
//! | Tools | `register_tool_pack` | `ToolPack` | `ToolRegistryBuilder::new()` |
//! | Providers | `register_provider_pack` | `ProviderPack` | Model resolution |
//! | Hooks | `register_hook_pack` | `HookPack` | Hooks dispatcher |
//!
//! ## Usage
//!
//! Extensions depend on this crate, implement the traits, and self-register
//! via `#[ctor]`:
//!
//! ```rust,no_run
//! use xai_grok_extension_api::prelude::*;
//!
//! #[derive(Debug, Default)]
//! pub struct MyTool;
//!
//! // ... implement Tool + ToolMetadata ...
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

/// Tool metadata (ToolMetadata trait, ToolKind, ToolNamespace).
pub use xai_grok_tools::types::tool_metadata::ToolMetadata;
pub use xai_grok_tools::types::tool::{ToolKind, ToolNamespace};

/// Tool registry (ToolRegistryBuilder — what tool packs receive).
pub use xai_grok_tools::registry::types::ToolRegistryBuilder;

/// Schema generation.
pub use schemars;

/// Serialization.
pub use serde;

/// Convenience re-export for the most common imports.
pub mod prelude {
    pub use crate::{
        register_hook_pack, register_provider_pack, register_tool_pack,
        hook::{HookAction, HookRegistry, PreToolUseEvent},
        provider::{ApiBackend, ModelConfig, ProviderConfig, ProviderRegistry},
    };
    pub use crate::xai_tool_runtime::Tool;
    pub use crate::ToolMetadata;
    pub use crate::schemars::JsonSchema;
    pub use crate::serde::{Deserialize, Serialize};
    pub use crate::xai_tool_protocol::{ToolCapabilities, ToolId};
    pub use crate::xai_tool_runtime::{ListToolsContext, ToolCallContext, ToolError};
    pub use crate::xai_tool_types::ToolDescription;
    pub use crate::{ToolKind, ToolNamespace};
}

// ── Provider types ──────────────────────────────────────────────────────────

pub mod provider;

// ── Hook types ──────────────────────────────────────────────────────────────

pub mod hook;

// ── Tool pack registration ──────────────────────────────────────────────────

/// A tool pack: a function that contributes tool registrations to a builder.
///
/// See [`xai_grok_tools::registry::types::TOOL_PACKS`] for the full ordering
/// contract. In short: packs MUST be registered before the first
/// `ToolRegistryBuilder::new()` in the process.
pub type ToolPack = fn(&mut ToolRegistryBuilder);

static TOOL_PACKS: OnceLock<Mutex<Vec<ToolPack>>> = OnceLock::new();

fn tool_packs() -> &'static Mutex<Vec<ToolPack>> {
    TOOL_PACKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register an out-of-tree tool pack.
///
/// This delegates to the existing `TOOL_PACKS` mechanism in `xai-grok-tools`.
/// The pack function receives a `&mut ToolRegistryBuilder` and can call
/// `builder.register::<T>()` for any tool that implements `Tool + ToolMetadata`.
///
/// # Ordering Contract
///
/// MUST be called before the first `ToolRegistryBuilder::new()` in the process.
/// Packs registered after a builder has been constructed do not retroactively
/// apply to that builder.
///
/// # Example
///
/// ```rust,no_run
/// use xai_grok_extension_api::prelude::*;
///
/// #[ctor::ctor]
/// fn init() {
///     register_tool_pack(|builder| {
///         builder.register::<MyTool>();
///     });
/// }
/// ```
pub fn register_tool_pack(pack: ToolPack) {
    // Delegate to the canonical location in xai-grok-tools
    xai_grok_tools::registry::types::register_tool_pack(pack);
}

// ── Provider pack registration ──────────────────────────────────────────────

/// A provider pack: a function that contributes model/provider registrations.
///
/// Packs MUST be registered before the first model resolution in the process.
pub type ProviderPack = fn(&mut provider::ProviderRegistry);

static PROVIDER_PACKS: OnceLock<Mutex<Vec<ProviderPack>>> = OnceLock::new();

fn provider_packs() -> &'static Mutex<Vec<ProviderPack>> {
    PROVIDER_PACKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register an out-of-tree provider pack.
///
/// The pack function receives a `&mut ProviderRegistry` and can call
/// `registry.register(config)` to add custom model providers.
///
/// # Example
///
/// ```rust,no_run
/// use xai_grok_extension_api::prelude::*;
///
/// fn register_my_provider(registry: &mut ProviderRegistry) {
///     registry.register(ProviderConfig {
///         name: "my-provider".into(),
///         base_url: "https://api.example.com/v1".into(),
///         api_backend: ApiBackend::ChatCompletions,
///         auth_env_var: Some("MY_API_KEY".into()),
///         models: vec![ModelConfig {
///             id: "my-model".into(),
///             name: "My Model".into(),
///             context_window: 128_000,
///             max_completion_tokens: Some(4096),
///         }],
///     });
/// }
///
/// #[ctor::ctor]
/// fn init() {
///     register_provider_pack(register_my_provider);
/// }
/// ```
pub fn register_provider_pack(pack: ProviderPack) {
    provider_packs().lock().push(pack);
}

/// Collect all registered provider packs into a single `ProviderRegistry`.
///
/// Called by the model resolver before checking remote settings and defaults.
/// Packs are iterated in registration order.
pub fn collect_providers() -> provider::ProviderRegistry {
    let mut registry = provider::ProviderRegistry::new();
    for pack in provider_packs().lock().iter() {
        pack(&mut registry);
    }
    registry
}

// ── Hook pack registration ──────────────────────────────────────────────────

/// A hook pack: a function that contributes event callbacks.
///
/// Packs MUST be registered before the first hook dispatch in the process.
pub type HookPack = fn(&mut hook::HookRegistry);

static HOOK_PACKS: OnceLock<Mutex<Vec<HookPack>>> = OnceLock::new();

fn hook_packs() -> &'static Mutex<Vec<HookPack>> {
    HOOK_PACKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register an out-of-tree hook pack.
///
/// The pack function receives a `&mut HookRegistry` and can call
/// `registry.on_pre_tool_use(...)` etc. to add event callbacks.
///
/// # Example
///
/// ```rust,no_run
/// use xai_grok_extension_api::prelude::*;
///
/// fn register_my_hooks(registry: &mut HookRegistry) {
///     registry.on_pre_tool_use(Box::new(|event| {
///         if event.tool_name == "bash" {
///             // Block dangerous commands
///         }
///         HookAction::Allow
///     }));
/// }
///
/// #[ctor::ctor]
/// fn init() {
///     register_hook_pack(register_my_hooks);
/// }
/// ```
pub fn register_hook_pack(pack: HookPack) {
    hook_packs().lock().push(pack);
}

/// Collect all registered hook packs into a single `HookRegistry`.
///
/// Called by the hooks dispatcher before running command-based hooks.
/// Packs are iterated in registration order.
pub fn collect_hooks() -> hook::HookRegistry {
    let mut registry = hook::HookRegistry::new();
    for pack in hook_packs().lock().iter() {
        pack(&mut registry);
    }
    registry
}
