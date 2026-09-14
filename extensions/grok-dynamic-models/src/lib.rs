//! # grok-dynamic-models
//!
//! Dynamic model discovery for Grok Build — fetches models from any
//! OpenAI-compatible API server at startup.
//!
//! ## Configuration
//!
//! Create `~/.grok/settings/dynamic-models.json`:
//!
//! ```json
//! [
//!   {
//!     "provider": "local-llm",
//!     "baseUrl": "http://localhost:11434/v1",
//!     "apiKey": "MY_API_KEY",
//!     "api": "chat_completions",
//!     "models": {
//!       "llama3.1": {
//!         "name": "Llama 3.1",
//!         "contextWindow": 128000,
//!         "maxTokens": 4096,
//!         "reasoning": false
//!       }
//!     }
//!   }
//! ]
//! ```
//!
//! ## Fields
//!
//! | Field | Required | Description |
//! |-------|----------|-------------|
//! | `provider` | Yes | Provider name shown in model selector |
//! | `baseUrl` | Yes | Server URL including /v1 if needed |
//! | `apiKey` | No | Env var name, or literal key |
//! | `api` | No | API type: `chat_completions`, `responses`, `messages` (default: `chat_completions`) |
//! | `models` | No | Per-model metadata keyed by model ID |
//!
//! ## Discovery
//!
//! 1. Fetch all model IDs from `GET {baseUrl}/models`
//! 2. Union with model IDs listed in config `models`
//! 3. For each: use configured fields if present, otherwise use defaults
//!
//! Servers that are unreachable at startup fall back to only the explicitly
//! configured models (if any).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use xai_grok_extension_api::prelude::*;

// ── Config types ────────────────────────────────────────────────────────────

/// Per-model override from config.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelOverride {
    /// Human-readable display name.
    name: Option<String>,
    /// Whether the model supports extended thinking.
    reasoning: Option<bool>,
    /// Context window size in tokens.
    context_window: Option<u64>,
    /// Maximum output tokens.
    max_tokens: Option<u32>,
    /// Temperature override.
    temperature: Option<f32>,
    /// Top-p override.
    top_p: Option<f32>,
}

/// Server configuration from the config file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerConfig {
    /// Provider name (e.g., "local-llm", "ollama").
    provider: String,
    /// Server URL (e.g., "http://localhost:11434/v1").
    base_url: String,
    /// API key env var name or literal key.
    api_key: Option<String>,
    /// API type: "chat_completions", "responses", "messages".
    api: Option<String>,
    /// Per-model metadata overrides.
    models: Option<HashMap<String, ModelOverride>>,
}

/// Response from GET /models endpoint.
#[derive(Debug, Clone, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

/// A single model entry from the /models endpoint.
#[derive(Debug, Clone, Deserialize)]
struct ModelEntry {
    id: String,
    name: Option<String>,
}

// ── Provider name abbreviations ────────────────────────────────────────────

/// Abbreviate provider names for compact display in the model picker.
fn abbreviate_provider(name: &str) -> &str {
    match name {
        "corral-local" => "corral",
        "local-dgx" => "dgx",
        "local-dgx-proto" => "dgx-p",
        "local-llm" => "llm",
        "nvidproxy" => "nvid",
        "clinepass" => "cline",
        _ => name,
    }
}

// ── Config loading ──────────────────────────────────────────────────────────

/// Load config from ~/.grok/settings/dynamic-models.json.
fn load_config() -> Vec<ServerConfig> {
    let config_path = config_path();
    if !config_path.exists() {
        return Vec::new();
    }

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                path = %config_path.display(),
                error = %e,
                "dynamic-models: failed to read config"
            );
            return Vec::new();
        }
    };

    match serde_json::from_str::<Vec<ServerConfig>>(&content) {
        Ok(configs) => configs,
        Err(e) => {
            tracing::warn!(
                path = %config_path.display(),
                error = %e,
                "dynamic-models: failed to parse config"
            );
            Vec::new()
        }
    }
}

/// Config file path: ~/.grok/settings/dynamic-models.json
pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".grok")
        .join("settings")
        .join("dynamic-models.json")
}

// ── API key resolution ──────────────────────────────────────────────────────

/// Resolve an API key from config.
///
/// Supports:
/// - `$ENV_VAR` or `${ENV_VAR}` — environment variable
/// - Literal string — used as-is
/// - `none` or empty — no auth
fn resolve_api_key(api_key: Option<&str>) -> Option<String> {
    let key = api_key?;
    if key.is_empty() || key == "none" {
        return None;
    }

    // Check for env var reference
    if key.starts_with('$') {
        let env_name = key.trim_start_matches('$').trim_matches('{').trim_matches('}');
        return std::env::var(env_name).ok();
    }

    // Check if it's an env var name (all uppercase with underscores)
    if key.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
        if let Ok(val) = std::env::var(key) {
            return Some(val);
        }
    }

    // Use as literal key
    Some(key.to_string())
}

// ── Model fetching ──────────────────────────────────────────────────────────

/// Fetch models from a server's /models endpoint (blocking).
fn fetch_models_blocking(base_url: &str, api_key: Option<&str>) -> Result<Vec<ModelEntry>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let mut request = ureq::get(&url)
        .set("Accept", "application/json");

    if let Some(key) = api_key {
        request = request.set("Authorization", &format!("Bearer {}", key));
    }

    let response = request
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {}", status));
    }

    let body: ModelsResponse = response
        .into_json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(body.data)
}

// ── Provider registration ───────────────────────────────────────────────────

/// Parse API backend string to ApiBackend enum.
fn parse_api_backend(api: Option<&str>) -> ApiBackend {
    match api.unwrap_or("chat_completions") {
        "responses" => ApiBackend::Responses,
        "messages" => ApiBackend::Messages,
        _ => ApiBackend::ChatCompletions,
    }
}

/// Register a provider with discovered and configured models.
fn register_provider(
    registry: &mut ProviderRegistry,
    config: &ServerConfig,
    discovered: &[ModelEntry],
    _api_key: Option<String>,
) {
    let api_backend = parse_api_backend(config.api.as_deref());

    // Union of discovered IDs and explicitly configured IDs
    let mut all_ids: Vec<String> = discovered.iter().map(|m| m.id.clone()).collect();
    if let Some(models) = &config.models {
        for id in models.keys() {
            if !all_ids.contains(id) {
                all_ids.push(id.clone());
            }
        }
    }
    all_ids.sort();

    if all_ids.is_empty() {
        tracing::info!(
            provider = config.provider,
            "dynamic-models: no models found"
        );
        return;
    }

    let discovered_map: HashMap<&str, &ModelEntry> = discovered
        .iter()
        .map(|m| (m.id.as_str(), m))
        .collect();

    let models: Vec<ModelConfig> = all_ids
        .iter()
        .map(|id| {
            let override_ = config.models.as_ref().and_then(|m| m.get(id));
            let discovered = discovered_map.get(id.as_str());

            ModelConfig {
                id: id.clone(),
                name: format!("{}: {}", abbreviate_provider(&config.provider),
                    override_
                        .and_then(|o| o.name.clone())
                        .or_else(|| discovered.and_then(|d| d.name.clone()))
                        .unwrap_or_else(|| id.clone())
                ),
                context_window: override_
                    .and_then(|o| o.context_window)
                    .unwrap_or(128_000),
                max_completion_tokens: override_.and_then(|o| o.max_tokens),
                reasoning: override_.and_then(|o| o.reasoning).unwrap_or(false),
                temperature: override_.and_then(|o| o.temperature),
                top_p: override_.and_then(|o| o.top_p),
            }
        })
        .collect();

    let mut extra_headers = HashMap::new();
    extra_headers.insert("Accept".to_string(), "application/json".to_string());

    let provider_config = ProviderConfig {
        name: config.provider.clone(),
        base_url: config.base_url.trim_end_matches('/').to_string(),
        api_backend,
        auth_env_var: None, // We resolve the key ourselves
        extra_headers,
        models,
    };

    let model_count = provider_config.models.len();
    let discovered_count = discovered.len();
    let configured_count = config.models.as_ref().map(|m| m.len()).unwrap_or(0);

    registry.register(provider_config);

    tracing::info!(
        provider = config.provider,
        discovered = discovered_count,
        configured = configured_count,
        total = model_count,
        api = ?api_backend,
        "dynamic-models: registered provider"
    );
}

// ── Extension entry point ───────────────────────────────────────────────────

/// The extension's provider pack function.
fn dynamic_models_provider_pack(registry: &mut ProviderRegistry) {
    let configs = load_config();
    if configs.is_empty() {
        tracing::debug!("dynamic-models: no config found at {}", config_path().display());
        return;
    }

    tracing::info!(
        count = configs.len(),
        config = %config_path().display(),
        "dynamic-models: loading providers"
    );

    // We need to fetch models synchronously since provider packs are sync.
    // Use blocking reqwest for the HTTP calls.
    for config in &configs {
        let api_key = resolve_api_key(config.api_key.as_deref());

        // Fetch models from the server
        let discovered = match fetch_models_blocking(&config.base_url, api_key.as_deref()) {
            Ok(models) => models,
            Err(err) => {
                tracing::warn!(
                    provider = config.provider,
                    base_url = config.base_url,
                    error = %err,
                    "dynamic-models: failed to fetch models, using configured only"
                );
                // Fall back to configured-only models
                Vec::new()
            }
        };

        register_provider(registry, config, &discovered, api_key);
    }
}

/// Auto-registration via #[ctor].
#[ctor::ctor]
fn init() {
    register_provider_pack(dynamic_models_provider_pack);
}
