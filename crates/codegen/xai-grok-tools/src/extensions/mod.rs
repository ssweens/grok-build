//! Extension tool bridge.
//!
//! Bridges `xai_grok_extension_api::ExtensionTool` to `xai_tool_runtime::Tool`
//! so extension tools can be registered with the real `ToolRegistryBuilder`.

use xai_grok_extension_api::tool::{
    ExtensionToolKind, ExtensionToolNamespace,
};

use crate::types::tool::{ToolKind, ToolNamespace};

/// Register all extension tools from the extension tool registry into the
/// real `ToolRegistryBuilder`.
///
/// Called by `ToolRegistryBuilder::new()` after registering built-in tools.
pub fn register_extension_tools(builder: &mut crate::registry::types::ToolRegistryBuilder) {
    // Drain the extension tool registry
    let ext_registry = xai_grok_extension_api::drain_extension_tools();

    for tool_info in ext_registry.tool_info() {
        let kind = match tool_info.kind {
            ExtensionToolKind::Read => ToolKind::Read,
            ExtensionToolKind::Edit => ToolKind::Edit,
            ExtensionToolKind::Search => ToolKind::Search,
            ExtensionToolKind::Execute => ToolKind::Execute,
            ExtensionToolKind::Web => ToolKind::WebSearch,
            ExtensionToolKind::Other => ToolKind::Other,
        };

        let namespace = match &tool_info.namespace {
            ExtensionToolNamespace::GrokBuild => ToolNamespace::GrokBuild,
            ExtensionToolNamespace::OpenCode => ToolNamespace::OpenCode,
            // Extension namespaces map to GrokBuild since that's the
            // host namespace for extension tools
            ExtensionToolNamespace::Extension(_) => ToolNamespace::GrokBuild,
        };

        tracing::info!(
            tool_id = tool_info.id,
            kind = ?kind,
            namespace = ?namespace,
            "registering extension tool"
        );

        builder.register_extension_tool(
            tool_info.id,
            tool_info.description,
            kind,
            namespace,
            tool_info.is_read_only,
            tool_info.input_schema,
        );
    }
}
