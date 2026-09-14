//! Extension tool traits and registry.
//!
//! This module defines the extension-specific versions of the tool traits.
//! Extensions implement [`ExtensionTool`] and [`ExtensionToolMetadata`],
//! then register via [`ExtensionToolRegistry`].
//!
//! The runtime crate bridges these to the real `xai-grok-tools` types
//! when constructing the `ToolRegistryBuilder`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ── Tool trait ──────────────────────────────────────────────────────────────

/// Extension tool trait. Mirrors `xai_tool_runtime::Tool` but is defined
/// here so extensions don't need to depend on `xai-grok-tools`.
///
/// Extensions implement this alongside [`ExtensionToolMetadata`].
/// The runtime crate bridges to the real `Tool` trait when registering
/// with the `ToolRegistryBuilder`.
#[async_trait]
pub trait ExtensionTool: Send + Sync + 'static {
    /// Typed input. Must be deserializable from JSON for wire dispatch.
    type Args: for<'de> Deserialize<'de> + JsonSchema + Send + 'static;

    /// Typed output. Must be serializable.
    type Output: Serialize + Send + 'static;

    /// Stable identity used by the runtime to route to this tool.
    fn id(&self) -> String;

    /// Model-facing description.
    fn description(&self) -> String;

    /// Execute the tool.
    async fn run(
        &self,
        ctx: &dyn crate::context::ExtensionContext,
        input: Self::Args,
    ) -> Result<Self::Output, ToolError>;
}

/// Tool execution error.
#[derive(Debug, Clone)]
pub struct ToolError {
    /// Error code for programmatic handling.
    pub code: String,

    /// Human-readable error message.
    pub message: String,
}

impl ToolError {
    /// Create a new tool error.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Create an execution error.
    pub fn execution(tool_id: &str, message: impl Into<String>) -> Self {
        Self {
            code: format!("{}.execution", tool_id),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ToolError {}

// ── Tool metadata trait ─────────────────────────────────────────────────────

/// Extension tool metadata. Mirrors `xai_grok_tools::types::tool_metadata::ToolMetadata`.
///
/// Extensions implement this alongside [`ExtensionTool`].
pub trait ExtensionToolMetadata: Send + Sync {
    /// High-level category.
    fn kind(&self) -> ExtensionToolKind;

    /// Namespace grouping.
    fn namespace(&self) -> ExtensionToolNamespace;

    /// Description template (may contain placeholders).
    fn description_template(&self) -> &str;

    /// Whether the tool is read-only.
    fn is_read_only(&self) -> bool {
        self.kind().is_read_only()
    }
}

/// Tool kind categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtensionToolKind {
    /// Read-only operations (file reads, searches, lists).
    Read,
    /// File editing operations.
    Edit,
    /// Search operations.
    Search,
    /// Command execution.
    Execute,
    /// Web operations.
    Web,
    /// Other/unknown.
    Other,
}

impl ExtensionToolKind {
    /// Whether this kind is read-only by default.
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::Read | Self::Search)
    }
}

/// Tool namespace grouping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExtensionToolNamespace {
    /// Built-in grok-build tools.
    GrokBuild,
    /// OpenCode-compatible tools.
    OpenCode,
    /// Extension-provided tools (with extension name).
    Extension(String),
}

// ── Tool registry ───────────────────────────────────────────────────────────

/// A type-erased tool entry in the extension tool registry.
struct ErasedTool {
    id: String,
    description: String,
    kind: ExtensionToolKind,
    namespace: ExtensionToolNamespace,
    is_read_only: bool,
    /// The tool's JSON schema for input parameters.
    input_schema: serde_json::Value,
    /// Type-erased execution function.
    execute: Box<
        dyn Fn(
                serde_json::Value,
                Arc<dyn crate::context::ExtensionContext>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<serde_json::Value, ToolError>> + Send>,
            > + Send
            + Sync,
    >,
}

impl std::fmt::Debug for ErasedTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErasedTool")
            .field("id", &self.id)
            .field("description", &self.description)
            .field("kind", &self.kind)
            .finish()
    }
}

/// Extension tool registry. Populated by tool packs at startup.
///
/// The runtime crate iterates registered tools and bridges them to
/// the real `xai-grok-tools` `ToolRegistryBuilder`.
#[derive(Debug)]
pub struct ExtensionToolRegistry {
    tools: HashMap<String, ErasedTool>,
}

impl ExtensionToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register an extension tool.
    ///
    /// The tool must implement both [`ExtensionTool`] and [`ExtensionToolMetadata`].
    pub fn register<T>(&mut self, tool: T)
    where
        T: ExtensionTool + ExtensionToolMetadata + 'static,
    {
        let id = tool.id();
        let description = tool.description();
        let kind = tool.kind();
        let namespace = tool.namespace();
        let is_read_only = <T as ExtensionToolMetadata>::is_read_only(&tool);

        // Generate JSON schema from the Args type
        let input_schema = schemars::schema_for!(T::Args);
        let input_schema =
            serde_json::to_value(&input_schema).unwrap_or(serde_json::Value::Object(Default::default()));

        let tool = Arc::new(tool);
        let tool_clone = tool.clone();

        let execute: Box<
            dyn Fn(
                    serde_json::Value,
                    Arc<dyn crate::context::ExtensionContext>,
                ) -> std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = Result<serde_json::Value, ToolError>,
                            > + Send,
                    >,
                > + Send
                + Sync,
        > = Box::new(move |input, ctx| {
            let tool = tool_clone.clone();
            Box::pin(async move {
                let args: T::Args = serde_json::from_value(input).map_err(|e| {
                    ToolError::new("invalid_args", format!("Failed to parse args: {}", e))
                })?;
                let output = tool.run(&*ctx, args).await?;
                serde_json::to_value(output).map_err(|e| {
                    ToolError::new(
                        "serialization_error",
                        format!("Failed to serialize output: {}", e),
                    )
                })
            })
        });

        tracing::info!(
            tool_id = id,
            kind = ?kind,
            namespace = ?namespace,
            "extension: registered tool"
        );

        self.tools.insert(
            id,
            ErasedTool {
                id: tool.id(),
                description,
                kind,
                namespace,
                is_read_only,
                input_schema,
                execute,
            },
        );
    }

    /// All registered tool IDs.
    pub fn tool_ids(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Execute a tool by ID.
    pub async fn execute(
        &self,
        tool_id: &str,
        input: serde_json::Value,
        ctx: Arc<dyn crate::context::ExtensionContext>,
    ) -> Result<serde_json::Value, ToolError> {
        let tool = self
            .tools
            .get(tool_id)
            .ok_or_else(|| ToolError::new("not_found", format!("Tool not found: {}", tool_id)))?;
        (tool.execute)(input, ctx).await
    }

    /// Get tool info for listing.
    pub fn tool_info(&self) -> Vec<ToolInfo> {
        self.tools
            .values()
            .map(|t| ToolInfo {
                id: t.id.clone(),
                description: t.description.clone(),
                kind: t.kind,
                namespace: t.namespace.clone(),
                is_read_only: t.is_read_only,
                input_schema: t.input_schema.clone(),
            })
            .collect()
    }
}

/// Information about a registered extension tool.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub id: String,
    pub description: String,
    pub kind: ExtensionToolKind,
    pub namespace: ExtensionToolNamespace,
    pub is_read_only: bool,
    pub input_schema: serde_json::Value,
}
