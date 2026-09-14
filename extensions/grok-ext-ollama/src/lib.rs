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
//! provider, tool, and hook.

use std::collections::HashMap;

use xai_grok_extension_api::prelude::*;

// ── Provider registration ───────────────────────────────────────────────────

/// Register Ollama as a custom model provider.
fn register_ollama_provider(registry: &mut ProviderRegistry) {
    let base_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".into());

    registry.register(ProviderConfig {
        name: "ollama".into(),
        base_url: format!("{}/v1", base_url),
        api_backend: ApiBackend::ChatCompletions,
        auth_env_var: None,
        extra_headers: HashMap::new(),
        models: vec![
            ModelConfig {
                id: "llama3.1".into(),
                name: "ollama: Llama 3.1".into(),
                context_window: 128_000,
                max_completion_tokens: Some(4096),
                reasoning: false,
                temperature: None,
                top_p: None,
            },
            ModelConfig {
                id: "codellama".into(),
                name: "ollama: Code Llama".into(),
                context_window: 16_384,
                max_completion_tokens: Some(4096),
                reasoning: false,
                temperature: None,
                top_p: None,
            },
            ModelConfig {
                id: "mistral".into(),
                name: "ollama: Mistral".into(),
                context_window: 32_000,
                max_completion_tokens: Some(4096),
                reasoning: false,
                temperature: None,
                top_p: None,
            },
        ],
    });
}

// ── Custom tool: ollama_list_models ─────────────────────────────────────────

/// Tool that lists locally available Ollama models.
///
/// In a real implementation, this would call Ollama's `/api/tags` endpoint.
/// For this example, we return hardcoded models.
#[derive(Debug, Default)]
pub struct OllamaListModelsTool;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListModelsInput {
    /// Optional substring filter.
    #[schemars(description = "Filter models by name substring (optional).")]
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListModelsOutput {
    /// Names of available Ollama models.
    pub models: Vec<String>,
    /// Base URL that was queried.
    pub base_url: String,
}

#[async_trait::async_trait]
impl ExtensionTool for OllamaListModelsTool {
    type Args = ListModelsInput;
    type Output = ListModelsOutput;

    fn id(&self) -> String {
        "ollama_list_models".into()
    }

    fn description(&self) -> String {
        "List locally available Ollama models. Use this to discover what \
         models are installed before selecting one for a task."
            .into()
    }

    async fn run(
        &self,
        _ctx: &dyn ExtensionContext,
        input: ListModelsInput,
    ) -> Result<ListModelsOutput, xai_grok_extension_api::tool::ToolError> {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".into());

        // In a real implementation, we'd call:
        // let resp = reqwest::get(format!("{}/api/tags", base_url)).await?;
        // For this example, return hardcoded models.
        let mut models = vec![
            "llama3.1".to_string(),
            "codellama".to_string(),
            "mistral".to_string(),
        ];

        if let Some(filter) = &input.filter {
            models.retain(|m| m.contains(filter));
        }

        Ok(ListModelsOutput {
            models,
            base_url,
        })
    }
}

impl ExtensionToolMetadata for OllamaListModelsTool {
    fn kind(&self) -> ExtensionToolKind {
        ExtensionToolKind::Read
    }

    fn namespace(&self) -> ExtensionToolNamespace {
        ExtensionToolNamespace::Extension("ollama".into())
    }

    fn description_template(&self) -> &str {
        "List locally available Ollama models."
    }

    fn is_read_only(&self) -> bool {
        true
    }
}

// ── Safety hook ─────────────────────────────────────────────────────────────

/// Register a pre-tool-use hook that blocks dangerous Ollama commands.
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

// ── Command registration ────────────────────────────────────────────────────

/// Register the `/ollama` command for listing models interactively.
fn register_ollama_commands(registry: &mut CommandRegistry) {
    registry.register_command(CommandDef {
        name: "ollama".into(),
        description: "List available Ollama models".into(),
        handler: Box::new(|_args, ctx| {
            Box::pin(async move {
                let base_url = std::env::var("OLLAMA_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".into());

                ctx.ui().notify(
                    &format!("Ollama endpoint: {}", base_url),
                    NotifyLevel::Info,
                );

                // In a real implementation, we'd call the API and show results
                ctx.ui().notify(
                    "Available models: llama3.1, codellama, mistral",
                    NotifyLevel::Info,
                );
            })
        }),
        autocomplete: None,
    });
}

// ── Auto-registration via #[ctor] ───────────────────────────────────────────

/// Process-level initialization. Runs before `main()` thanks to `#[ctor]`.
#[ctor::ctor]
fn init() {
    // Register the Ollama provider
    register_provider_pack(register_ollama_provider);

    // Register the ollama_list_models tool
    register_tool_pack(|builder| {
        builder.register(OllamaListModelsTool);
    });

    // Register the safety hook
    register_hook_pack(register_ollama_hooks);

    // Register the /ollama command
    register_command_pack(register_ollama_commands);
}
