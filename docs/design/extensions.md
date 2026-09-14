# Grok Build Extension System

> **Status:** Design proposal
> **Author:** sweeney
> **Date:** 2025-07-18

## Problem

Grok Build's plugin system is **declarative** — plugins are directories with static
assets (skills, hooks, MCP configs, agents). The runtime discovers and loads them
at startup, but plugins cannot execute code, register custom model providers, or
dynamically augment the agent at runtime.

Compare this to pi coding agent's extension system, where a TypeScript module
exports a factory function that receives an `ExtensionAPI` and can call
`pi.registerProvider()`, `pi.registerTool()`, `pi.on('tool_call', ...)` —
dynamically augmenting the agent at runtime.

Grok Build needs an equivalent **imperative** extension surface for:

- **Custom model providers** — Ollama, LM Studio, vLLM, corporate proxies, any
  OpenAI-compatible endpoint
- **Custom tools** — domain-specific tools that ship with a crate, not a
  directory of markdown
- **Event hooks in Rust** — pre/post tool-use interception with typed callbacks,
  not shell commands

## Design Goals

1. **Extensions are Rust crates.** Add a dependency, get new functionality.
   No ABI contracts, no dynamic loading, no plugin discovery at runtime.
2. **Compile-time safety.** The extension's tool schema, provider config, and
   hook signatures are checked by the Rust compiler.
3. **Zero-cost when unused.** Extensions that aren't linked don't exist in the
   binary. No runtime scanning, no dead code.
4. **Self-registering.** Extensions use `#[ctor]` to register themselves at
   link time. The main binary doesn't need to know about them.
5. **Thin facade.** The `xai-grok-extension-api` crate re-exports only what
   extensions need — no transitive pull of the entire workspace.

## Existing Foundation

Grok Build already has a `TOOL_PACKS` mechanism in
`xai-grok-tools/src/registry/types.rs` that is exactly this pattern — but with
zero consumers:

```rust
/// Process-global registry of external "tool packs" — functions that
/// contribute additional tool registrations into every
/// [`ToolRegistryBuilder::new`].
///
/// This inverts the dependency for harness code that must live outside
/// this crate: instead of `xai-grok-tools` referencing an out-of-tree tool
/// pack, the pack calls [`register_tool_pack`] at startup and registers
/// itself here.
///
/// # Ordering contract
/// [`register_tool_pack`] MUST run before the FIRST `ToolRegistryBuilder::new()`
/// in the process. Packs registered after a builder has been constructed
/// do not retroactively apply to that builder.
pub type ToolPack = fn(&mut ToolRegistryBuilder);
static TOOL_PACKS: OnceLock<Mutex<Vec<ToolPack>>> = OnceLock::new();

pub fn register_tool_pack(pack: ToolPack) {
    tool_packs().lock().push(pack);
}
```

At `ToolRegistryBuilder::new()` construction time:

```rust
for pack in tool_packs().lock().iter() {
    pack(&mut b);
}
```

The `register<T>()` and `register_with_params<T, P>()` methods are already `pub`
specifically so out-of-tree tool packs can call them. The trait bounds are:

```rust
pub fn register<T>(&mut self)
where
    T: Tool + ToolMetadata + Debug + Default + Send + Sync + 'static,
    T::Args: DeserializeOwned + JsonSchema + Into<ToolInput>,
    T::Output: Serialize + DeserializeOwned + Into<ToolOutput>,
```

### What Exists vs. What's Missing

| Surface | Status | Notes |
|---------|--------|-------|
| Tools | `TOOL_PACKS` exists, zero consumers | `register<T>()` is already pub |
| Providers/Models | No equivalent seam | `default_models.json` is compile-time embedded |
| Hooks/Events | Command-based only (`xai-grok-hooks`) | No Rust callback path |

## Architecture

### Three Extension Surfaces

```
┌─────────────────────────────────────────────────────────────┐
│                    Extension Crate                           │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Tool Pack    │  │ Provider Pack│  │  Hook Pack   │      │
│  │              │  │              │  │              │      │
│  │ register::<  │  │ registry.    │  │ registry.    │      │
│  │   MyTool>()  │  │  register()  │  │  on_pre_tool │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │               │
│    #[ctor::ctor]     #[ctor::ctor]     #[ctor::ctor]       │
│    register_tool     register_         register_           │
│    _pack(...)        provider_pack(    hook_pack(...)       │
│                      ...)                                   │
└─────────┬─────────────────┬─────────────────┬──────────────┘
          │                 │                 │
          ▼                 ▼                 ▼
┌─────────────────┐ ┌──────────────┐ ┌──────────────┐
│ TOOL_PACKS      │ │PROVIDER_PACKS│ │ HOOK_PACKS   │
│ (OnceLock)      │ │ (OnceLock)   │ │ (OnceLock)   │
└────────┬────────┘ └──────┬───────┘ └──────┬───────┘
         │                 │                │
         ▼                 ▼                ▼
┌─────────────────┐ ┌──────────────┐ ┌──────────────┐
│ToolRegistryBuilder│ │ModelResolver │ │HookDispatcher│
│ .new() iterates │ │ .resolve()   │ │ .dispatch()  │
│ packs           │ │ checks packs │ │ runs packs   │
└─────────────────┘ └──────────────┘ └──────────────┘
```

### The `xai-grok-extension-api` Crate

A thin facade crate that re-exports what extensions need without pulling in the
entire workspace. Extensions depend on this, not on `xai-grok-tools` directly.

```
xai-grok-extension-api/
├── Cargo.toml
└── src/
    ├── lib.rs              # Re-exports + pack registration functions
    ├── tool.rs             # Tool trait re-exports + ToolMetadata
    ├── provider.rs         # ProviderConfig, ProviderRegistry, ModelConfig
    └── hook.rs             # HookRegistry, PreToolUseEvent, HookAction
```

**Dependencies** (minimal):

```toml
[dependencies]
xai-tool-runtime = { path = "../crates/common/xai-tool-runtime" }
xai-tool-protocol = { path = "../crates/common/xai-tool-protocol" }
xai-tool-types = { path = "../crates/common/xai-tool-types" }
xai-grok-tools = { path = "../crates/codegen/xai-grok-tools" }
schemars = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
```

Note: `ctor` is **not** a dependency of the facade — it's a dependency of each
extension crate. The facade only provides the registration functions.

## Extension Surfaces

### 1. Tools

An extension crate implements `Tool` + `ToolMetadata` and registers via
`register_tool_pack`. This is the existing `TOOL_PACKS` pattern — no changes
to grok-build needed.

**Traits to implement:**

```rust
// From xai-tool-runtime
pub trait Tool: Send + Sync {
    type Args: DeserializeOwned + JsonSchema + Send + 'static;
    type Output: Serialize + ToolOutput + Send + 'static;

    fn id(&self) -> ToolId;
    fn description(&self, ctx: &ListToolsContext) -> ToolDescription;
    fn capabilities(&self) -> ToolCapabilities { ... }

    // Implement one of:
    async fn run(&self, ctx: ToolCallContext, input: Self::Args)
        -> Result<Self::Output, ToolError>;
    // OR for streaming:
    fn execute(&self, ctx: ToolCallContext, input: Self::Args)
        -> ToolStream<Self::Output>;
}

// From xai-grok-tools::types::tool_metadata
pub trait ToolMetadata: Send + Sync {
    fn kind(&self) -> ToolKind;           // Read, Edit, Search, Execute, ...
    fn tool_namespace(&self) -> ToolNamespace;
    fn description_template(&self) -> &str;
    // Optional overrides:
    fn is_read_only(&self) -> bool { ... }
    fn requires_expr(&self) -> Expr<ToolRequirement> { ... }
}
```

**Registration:**

```rust
#[ctor::ctor]
fn init() {
    xai_grok_extension_api::register_tool_pack(|builder| {
        builder.register::<MyCustomTool>();
    });
}
```

### 2. Providers / Models

A new `PROVIDER_PACKS` seam, modeled after `TOOL_PACKS`. This is the primary
gap — `default_models.json` is compile-time embedded with no runtime extension
point.

**New types in `xai-grok-extension-api`:**

```rust
pub struct ProviderConfig {
    pub name: String,                          // e.g. "ollama"
    pub base_url: String,                      // e.g. "http://localhost:11434/v1"
    pub api_backend: ApiBackend,               // ChatCompletions | Responses | Messages
    pub auth_env_var: Option<String>,          // e.g. Some("OLLAMA_API_KEY")
    pub models: Vec<ModelConfig>,
}

pub struct ModelConfig {
    pub id: String,                            // e.g. "llama3.1"
    pub name: String,                          // e.g. "Llama 3.1"
    pub context_window: u64,
    pub max_completion_tokens: Option<u32>,
}

pub enum ApiBackend {
    ChatCompletions,  // /v1/chat/completions
    Responses,        // /v1/responses
    Messages,         // /v1/messages (Anthropic)
}
```

**Registration:**

```rust
#[ctor::ctor]
fn init() {
    xai_grok_extension_api::register_provider_pack(|registry| {
        registry.register(ProviderConfig {
            name: "ollama".into(),
            base_url: "http://localhost:11434/v1".into(),
            api_backend: ApiBackend::ChatCompletions,
            auth_env_var: None,
            models: vec![
                ModelConfig {
                    id: "llama3.1".into(),
                    name: "Llama 3.1".into(),
                    context_window: 128_000,
                    max_completion_tokens: Some(4096),
                },
            ],
        });
    });
}
```

**Integration point:** The model resolution chain in `xai-grok-models` becomes:

```
CLI flag > ENV var > config.toml > provider packs > remote settings > compiled-in defaults
```

The `SamplerConfig` already has all the knobs (`base_url`, `api_backend`,
`auth_scheme`, `extra_headers`, `context_window`) — the provider registry just
needs to populate them from the extension-provided `ProviderConfig`.

### 3. Hooks / Events

A new `HOOK_PACKS` seam that provides typed Rust callbacks alongside the
existing command-based hooks in `xai-grok-hooks`.

**New types in `xai-grok-extension-api`:**

```rust
pub struct PreToolUseEvent {
    pub tool_name: String,
    pub input: serde_json::Value,
}

pub struct PostToolUseEvent {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub is_error: bool,
}

pub enum HookAction {
    Allow,
    Block { reason: String },
}

pub struct HookRegistry {
    pre_tool_use: Vec<Box<dyn Fn(&PreToolUseEvent) -> HookAction + Send + Sync>>,
    post_tool_use: Vec<Box<dyn Fn(&PostToolUseEvent) + Send + Sync>>,
    session_start: Vec<Box<dyn Fn() + Send + Sync>>,
}
```

**Registration:**

```rust
#[ctor::ctor]
fn init() {
    xai_grok_extension_api::register_hook_pack(|registry| {
        registry.on_pre_tool_use(Box::new(|event| {
            if event.tool_name == "bash" {
                if let Some(cmd) = event.input.get("command").and_then(|v| v.as_str()) {
                    if cmd.contains("rm -rf /") {
                        return HookAction::Block {
                            reason: "Blocked dangerous command".into(),
                        };
                    }
                }
            }
            HookAction::Allow
        }));
    });
}
```

**Integration point:** The hooks dispatcher in `xai-grok-hooks` runs
command-based hooks first, then iterates `HOOK_PACKS` callbacks. Both paths
produce `HookAction::Allow | Block`.

## Extension Crate Template

A minimal extension crate:

```
grok-ext-my-extension/
├── Cargo.toml
└── src/
    └── lib.rs
```

```toml
# Cargo.toml
[package]
name = "grok-ext-my-extension"
version = "0.1.0"
edition = "2024"

[dependencies]
xai-grok-extension-api = { path = "../xai-grok-extension-api" }
```

```rust
// src/lib.rs
use xai_grok_extension_api::prelude::*;

// ── Tool ────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct MyTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MyToolInput {
    #[schemars(description = "The input value.")]
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MyToolOutput {
    pub result: String,
}

impl ToolMetadata for MyTool {
    fn kind(&self) -> ToolKind { ToolKind::Read }
    fn tool_namespace(&self) -> ToolNamespace { ToolNamespace::GrokBuild }
    fn description_template(&self) -> &str { "My custom tool description." }
}

impl Tool for MyTool {
    type Args = MyToolInput;
    type Output = MyToolOutput;

    fn id(&self) -> ToolId {
        ToolId::new("my_tool").expect("valid")
    }

    fn description(&self, _ctx: &ListToolsContext) -> ToolDescription {
        ToolDescription::new("my_tool", <Self as ToolMetadata>::description_template(self))
    }

    async fn run(
        &self,
        _ctx: ToolCallContext,
        input: MyToolInput,
    ) -> Result<MyToolOutput, ToolError> {
        Ok(MyToolOutput {
            result: format!("processed: {}", input.value),
        })
    }
}

// ── Provider ────────────────────────────────────────────────────────

fn register_provider(registry: &mut ProviderRegistry) {
    registry.register(ProviderConfig {
        name: "my-provider".into(),
        base_url: "https://api.example.com/v1".into(),
        api_backend: ApiBackend::ChatCompletions,
        auth_env_var: Some("MY_PROVIDER_API_KEY".into(),
        models: vec![ModelConfig {
            id: "my-model".into(),
            name: "My Model".into(),
            context_window: 128_000,
            max_completion_tokens: Some(4096),
        }],
    });
}

// ── Hook ────────────────────────────────────────────────────────────

fn register_hooks(registry: &mut HookRegistry) {
    registry.on_pre_tool_use(Box::new(|event| {
        // Custom pre-tool-use logic
        HookAction::Allow
    }));
}

// ── Auto-registration ───────────────────────────────────────────────

#[ctor::ctor]
fn init() {
    register_tool_pack(|builder| {
        builder.register::<MyTool>();
    });
    register_provider_pack(register_provider);
    register_hook_pack(register_hooks);
}
```

## Workspace Integration

Extensions are added as workspace members:

```toml
# Root Cargo.toml
[workspace]
members = [
    # ...existing members...
    "extensions/grok-ext-ollama",
    "extensions/grok-ext-my-company",
]
```

Or as path/git dependencies in the binary crate:

```toml
# crates/codegen/xai-grok-pager-bin/Cargo.toml
[dependencies]
grok-ext-ollama = { path = "../../../extensions/grok-ext-ollama" }
```

Extensions that are not listed are not linked. No runtime discovery, no dead code.

## Comparison with Pi's Extension System

| Aspect | Pi Extensions | Grok Build Extensions |
|--------|--------------|----------------------|
| Language | TypeScript | Rust |
| Discovery | `~/.pi/agent/extensions/*.ts` auto-discovered | Workspace member or path dep, compiled in |
| Registration | `pi.registerProvider()`, `pi.registerTool()` | `register_tool_pack()`, `register_provider_pack()` |
| Type safety | Runtime (TypeBox schemas) | Compile-time (Rust traits + schemars) |
| Hot reload | `/reload` re-scans extensions | Recompile required |
| Event hooks | `pi.on("tool_call", ...)` | `HookRegistry.on_pre_tool_use(...)` |
| Provider API | `pi.registerProvider("ollama", {...})` | `ProviderRegistry.register(ProviderConfig {...})` |
| Distribution | npm packages, git repos | Cargo crates, git deps |
| Sandbox | Same process, full access | Same process, full access |

Pi's system is more dynamic (hot reload, filesystem discovery) but less safe
(runtime type errors, no compile-time checks). Grok Build's system trades
dynamism for safety — extensions are as native as built-in tools.

## Implementation Plan

### Phase 1: Foundation (no behavior change)

1. Create `xai-grok-extension-api` crate with re-exports only
2. Verify `TOOL_PACKS` works end-to-end with a test extension
3. Document the tool registration pattern

### Phase 2: Provider Registry

1. Add `PROVIDER_PACKS` to `xai-grok-extension-api`
2. Add `ProviderConfig`, `ModelConfig`, `ApiBackend` types
3. Wire into the model resolution chain in `xai-grok-models`:
   - CLI flag > ENV var > config.toml > **provider packs** > remote settings > defaults
4. Convert `default_models.json` compiled-in models to a "built-in provider pack"

### Phase 3: Hook Callbacks

1. Add `HOOK_PACKS` to `xai-grok-extension-api`
2. Add `HookRegistry`, `PreToolUseEvent`, `HookAction` types
3. Wire into the hooks dispatcher in `xai-grok-hooks`:
   - Command-based hooks run first (existing behavior)
   - Then iterates `HOOK_PACKS` callbacks
4. Both paths produce `Allow | Block`

### Phase 4: First-Party Extensions

1. Build `grok-ext-ollama` as a reference extension
2. Document the extension authoring workflow
3. Add CI checks for extension crate compilation

## Open Questions

1. **Namespace for extension tools.** Currently `ToolNamespace` has `GrokBuild`,
   `Cursor`, `OpenCode`. Extensions need their own namespace, or a generic
   `Extension(String)` variant. The namespace affects tool ID construction
   (e.g., `"Ollama:list_models"` vs `"GrokBuild:list_models"`).

2. **Provider auth integration.** Should `ProviderConfig.auth_env_var` feed into
   the existing `AuthCredentialProvider` trait, or should extensions manage their
   own auth? The former is cleaner but requires changes to `xai-grok-auth`.

3. **Hook ordering.** If multiple extensions register `on_pre_tool_use` hooks,
   what's the ordering? Registration order (deterministic) is simplest.

4. **Extension configuration.** Should extensions be able to read from
   `config.toml`? A `[extensions.ollama]` section with extension-specific keys?

5. **Dynamic model discovery.** Some providers (Ollama, vLLM) expose a
   `/v1/models` endpoint. Should the extension API support async model
   discovery, or is static config sufficient?

## Status

**All components wired and compiling:**

| Component | Location | Status |
|-----------|----------|--------|
| Extension API crate | `crates/common/xai-grok-extension-api/` | ✅ Complete |
| Example extension | `extensions/grok-ext-ollama/` | ✅ Complete |
| Tool bridge | `crates/codegen/xai-grok-tools/src/extensions/` | ✅ Wired into `ToolRegistryBuilder::new()` |
| Hook bridge | `crates/codegen/xai-grok-hooks/src/dispatcher.rs` | ✅ Wired into `dispatch_pre_tool_use()` |
| Provider bridge | `crates/codegen/xai-grok-shell/src/agent/config.rs` | ✅ Wired into `resolve_model_list()` |
| Command bridge | `crates/codegen/xai-grok-pager/src/slash/commands/mod.rs` | ✅ Wired into `builtin_commands()` |
| UI implementation | `crates/codegen/xai-grok-pager/src/extension/` | ✅ Complete |

## Build Verification

```bash
cargo check -p xai-grok-extension-api -p grok-ext-ollama -p xai-grok-tools \
  -p xai-grok-hooks -p xai-grok-shell -p xai-grok-pager
# Finished `dev` profile — all 6 crates compile clean
```

## References

- [Pi Extension System](https://pi.earendil.works/docs/extensions) — the
  reference implementation for imperative agent extensions
- [Pi Custom Providers](https://pi.earendil.works/docs/custom-provider) —
  provider registration API design
- `xai-grok-tools/src/registry/types.rs` — existing `TOOL_PACKS` mechanism
- `xai-grok-agent/src/plugins/` — existing declarative plugin system
- `xai-grok-hooks/` — existing command-based hooks system
- `xai-grok-models/` — compile-time model defaults
