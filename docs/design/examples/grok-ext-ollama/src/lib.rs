//! # grok-ext-ollama
//!
//! Grok Build extension that adds:
//! - **Ollama provider** — registers local Ollama models for use as custom providers
//! - **ollama_list_models tool** — lets the agent discover available Ollama models
//! - **Safety hook** — blocks `ollama rm` commands to prevent accidental model deletion
//!
//! ## Usage
//!
//! Add to your workspace or binary crate's `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! grok-ext-ollama = { path = "path/to/grok-ext-ollama" }
//! ```
//!
//! That's it. The `#[ctor]` function runs before `main()`, registering the
//! provider, tool, and hook. No configuration needed beyond having Ollama
//! running locally.
//!
//! ## Configuration
//!
//! - `OLLAMA_BASE_URL` — override the Ollama endpoint (default: `http://localhost:11434`)
//! - Ollama models are discovered at startup from the `/api/tags` endpoint

use xai_grok_extension_api::prelude::*;

// ── Provider registration ───────────────────────────────────────────────────

/// Register Ollama as a custom model provider.
///
/// Reads `OLLAMA_BASE_URL` from the environment, defaulting to localhost.
/// Models are registered statically here; for dynamic discovery at runtime,
/// see the `ollama_list_models` tool below.
fn register_ollama_provider(registry: &mut ProviderRegistry) {
    let base_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".into());

    registry.register(ProviderConfig {
        name: "ollama".into(),
        base_url: format!("{}/v1", base_url),
        api_backend: ApiBackend::ChatCompletions,
        auth_env_var: None, // Ollama doesn't require auth by default
        models: vec![
            ModelConfig {
                id: "llama3.1".into(),
                name: "Llama 3.1".into(),
                context_window: 128_000,
                max_completion_tokens: Some(4096),
            },
            ModelConfig {
                id: "codellama".into(),
                name: "Code Llama".into(),
                context_window: 16_384,
                max_completion_tokens: Some(4096),
            },
            ModelConfig {
                id: "mistral".into(),
                name: "Mistral".into(),
                context_window: 32_000,
                max_completion_tokens: Some(4096),
            },
        ],
    });
}

// ── Custom tool: ollama_list_models ─────────────────────────────────────────

/// Tool that queries Ollama's `/api/tags` endpoint to list locally available
/// models. Useful when the agent needs to discover what's installed before
/// selecting a model for a task.
#[derive(Debug, Default)]
pub struct OllamaListModelsTool;

/// Input schema for the ollama_list_models tool.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ListModelsInput {
    /// Optional substring filter. Only models whose name contains this
    /// string will be returned.
    #[schemars(description = "Filter models by name substring (optional).")]
    pub filter: Option<String>,
}

/// Output from the ollama_list_models tool.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListModelsOutput {
    /// Names of available Ollama models.
    pub models: Vec<String>,
    /// Base URL that was queried.
    pub base_url: String,
}

impl xai_grok_extension_api::ToolMetadata for OllamaListModelsTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn tool_namespace(&self) -> ToolNamespace {
        // Extensions should use their own namespace.
        // This would be a new variant like ToolNamespace::Extension("ollama")
        // or similar. Using GrokBuild as placeholder until the namespace
        // question is resolved.
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "List locally available Ollama models. Use this to discover what \
         models are installed before selecting one for a task. Returns model \
         names that can be used with the ollama provider."
    }

    fn is_read_only(&self) -> bool {
        true
    }
}

impl xai_grok_extension_api::Tool for OllamaListModelsTool {
    type Args = ListModelsInput;
    type Output = ListModelsOutput;

    fn id(&self) -> xai_grok_extension_api::xai_tool_protocol::ToolId {
        xai_grok_extension_api::xai_tool_protocol::ToolId::new("ollama_list_models")
            .expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_grok_extension_api::xai_tool_runtime::ListToolsContext,
    ) -> xai_grok_extension_api::xai_tool_types::ToolDescription {
        xai_grok_extension_api::xai_tool_types::ToolDescription::new(
            "ollama_list_models",
            <Self as xai_grok_extension_api::ToolMetadata>::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_grok_extension_api::xai_tool_protocol::ToolCapabilities {
        xai_grok_extension_api::xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            ..Default::default()
        }
    }

    async fn run(
        &self,
        _ctx: xai_grok_extension_api::xai_tool_runtime::ToolCallContext,
        input: ListModelsInput,
    ) -> Result<ListModelsOutput, xai_grok_extension_api::xai_tool_runtime::ToolError> {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".into());

        let url = format!("{}/api/tags", base_url);

        let resp = reqwest::get(&url).await.map_err(|e| {
            xai_grok_extension_api::xai_tool_runtime::ToolError::execution(
                xai_grok_extension_api::xai_tool_protocol::ToolId::new("ollama_list_models")
                    .expect("valid"),
                format!("Failed to connect to Ollama at {}: {}", url, e),
            )
        })?;

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            xai_grok_extension_api::xai_tool_runtime::ToolError::execution(
                xai_grok_extension_api::xai_tool_protocol::ToolId::new("ollama_list_models")
                    .expect("valid"),
                format!("Failed to parse Ollama response: {}", e),
            )
        })?;

        let mut models: Vec<String> = body["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        // Ollama returns full names like "llama3.1:latest";
                        // strip the tag for cleaner display
                        m["name"]
                            .as_str()
                            .map(|name| name.split(':').next().unwrap_or(name).to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Deduplicate (llama3.1:latest and llama3.1:7b both become "llama3.1")
        models.sort();
        models.dedup();

        if let Some(filter) = &input.filter {
            models.retain(|m| m.contains(filter));
        }

        Ok(ListModelsOutput {
            models,
            base_url,
        })
    }
}

// ── Safety hook ─────────────────────────────────────────────────────────────

/// Register a pre-tool-use hook that blocks dangerous Ollama commands.
///
/// Prevents accidental model deletion via `ollama rm` — the agent must
/// explicitly list models first and get user confirmation.
fn register_ollama_hooks(registry: &mut HookRegistry) {
    registry.on_pre_tool_use(Box::new(|event| {
        // Only intercept bash commands
        if event.tool_name != "bash" {
            return HookAction::Allow;
        }

        let Some(cmd) = event.input.get("command").and_then(|v| v.as_str()) else {
            return HookAction::Allow;
        };

        // Block ollama rm to prevent accidental model deletion
        if cmd.contains("ollama rm") || cmd.contains("ollama delete") {
            return HookAction::Block {
                reason: "Blocked by grok-ext-ollama: model deletion requires \
                         explicit user confirmation. Use 'ollama list' first."
                    .into(),
            };
        }

        HookAction::Allow
    }));
}

// ── Auto-registration via #[ctor] ───────────────────────────────────────────

/// Process-level initialization. Runs before `main()` thanks to `#[ctor]`.
///
/// Registers:
/// 1. The ollama provider (custom model endpoint)
/// 2. The ollama_list_models tool
/// 3. The safety hook for ollama commands
///
/// All registrations are idempotent and ordering-safe — the extension API
/// guarantees packs are collected before the first builder/resolver runs.
#[ctor::ctor]
fn init() {
    // Register the Ollama provider
    xai_grok_extension_api::register_provider_pack(register_ollama_provider);

    // Register the ollama_list_models tool
    xai_grok_extension_api::register_tool_pack(|builder| {
        builder.register::<OllamaListModelsTool>();
    });

    // Register the safety hook
    xai_grok_extension_api::register_hook_pack(register_ollama_hooks);
}
