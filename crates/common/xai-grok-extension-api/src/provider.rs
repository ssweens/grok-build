//! Provider types for the extension API.
//!
//! Extensions register custom model providers via [`ProviderRegistry::register`].
//! The registry is collected at startup and fed into the model resolution chain.

use std::collections::HashMap;

/// A custom model provider registered by an extension.
///
/// Grok Build's model resolution chain checks extension providers after
/// config.toml but before remote settings and compiled-in defaults:
///
/// ```text
/// CLI flag > ENV var > config.toml > provider packs > remote settings > defaults
/// ```
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Unique provider name (e.g., "ollama", "vllm", "my-company").
    /// Used in model references: `--model ollama/llama3.1`.
    pub name: String,

    /// API endpoint URL (e.g., "http://localhost:11434/v1").
    pub base_url: String,

    /// Which API protocol this provider speaks.
    pub api_backend: ApiBackend,

    /// Environment variable containing the API key, if any.
    /// `None` for providers that don't require auth (e.g., local Ollama).
    pub auth_env_var: Option<String>,

    /// Extra request headers. Values can use env var interpolation (`$ENV_VAR`).
    pub extra_headers: HashMap<String, String>,

    /// Models available from this provider.
    pub models: Vec<ModelConfig>,
}

/// API protocol for a custom provider.
///
/// Maps to the existing `ApiBackend` enum in `xai-grok-sampling-types`.
/// The sampler already dispatches on this to select the request/response format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiBackend {
    /// OpenAI Chat Completions API (`/v1/chat/completions`).
    /// Works with Ollama, vLLM, LM Studio, and most OpenAI-compatible endpoints.
    ChatCompletions,

    /// OpenAI Responses API (`/v1/responses`).
    /// For providers that support the newer Responses format.
    Responses,

    /// Anthropic Messages API (`/v1/messages`).
    /// For Anthropic-compatible providers.
    Messages,
}

/// Configuration for a single model offered by a custom provider.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Model identifier used in API requests (e.g., "llama3.1").
    /// Referenced as `provider/model` on the CLI: `--model ollama/llama3.1`.
    pub id: String,

    /// Human-readable display name (e.g., "Llama 3.1").
    pub name: String,

    /// Total context window size in tokens.
    pub context_window: u64,

    /// Maximum output tokens. `None` uses the provider's default.
    pub max_completion_tokens: Option<u32>,

    /// Whether this model supports extended thinking / reasoning.
    pub reasoning: bool,

    /// Temperature override. `None` uses the provider's default.
    pub temperature: Option<f32>,

    /// Top-p override. `None` uses the provider's default.
    pub top_p: Option<f32>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            context_window: 128_000,
            max_completion_tokens: Some(4096),
            reasoning: false,
            temperature: None,
            top_p: None,
        }
    }
}

/// Registry of extension-provided model providers.
///
/// Populated by provider packs at startup. The model resolver queries this
/// when the user specifies a `provider/model` reference or when no built-in
/// model matches.
#[derive(Debug)]
pub struct ProviderRegistry {
    providers: HashMap<String, ProviderConfig>,
}

impl ProviderRegistry {
    /// Create an empty registry. Called by the extension API internally.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a custom provider. If a provider with the same name already
    /// exists, the new registration wins (last-write-wins by registration order).
    pub fn register(&mut self, config: ProviderConfig) {
        tracing::info!(
            provider = config.name,
            models = config.models.len(),
            "extension: registered provider"
        );
        self.providers.insert(config.name.clone(), config);
    }

    /// Remove a provider by name. Returns the removed provider, if any.
    pub fn unregister(&mut self, name: &str) -> Option<ProviderConfig> {
        self.providers.remove(name)
    }

    /// Look up a provider by name.
    pub fn get(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// Look up a specific model across all registered providers.
    ///
    /// Accepts both `"model_id"` (searches all providers) and
    /// `"provider/model_id"` (searches the named provider).
    pub fn resolve_model(&self, reference: &str) -> Option<(&ProviderConfig, &ModelConfig)> {
        if let Some((provider_name, model_id)) = reference.split_once('/') {
            let provider = self.providers.get(provider_name)?;
            let model = provider.models.iter().find(|m| m.id == model_id)?;
            Some((provider, model))
        } else {
            for provider in self.providers.values() {
                if let Some(model) = provider.models.iter().find(|m| m.id == reference) {
                    return Some((provider, model));
                }
            }
            None
        }
    }

    /// All registered provider names.
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// All models across all providers, with their provider name.
    pub fn all_models(&self) -> Vec<(&str, &ModelConfig)> {
        self.providers
            .values()
            .flat_map(|p| p.models.iter().map(move |m| (p.name.as_str(), m)))
            .collect()
    }

    /// Whether any providers are registered.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }
}
