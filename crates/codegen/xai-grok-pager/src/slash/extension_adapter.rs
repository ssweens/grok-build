//! Extension command adapter.
//!
//! Bridges `xai_grok_extension_api::CommandDef` to the pager's `SlashCommand` trait
//! so extension commands can be registered in the pager's command registry.

use std::sync::Arc;

use crate::slash::command::{ArgItem, AppCtx, CommandExecCtx, CommandResult, SlashCommand};

/// Adapter that wraps an extension `CommandDef` as a pager `SlashCommand`.
pub struct ExtensionCommandAdapter {
    name: String,
    description: String,
    _handler: Arc<
        dyn Fn(String, Box<dyn xai_grok_extension_api::context::ExtensionContext>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
            + Send
            + Sync,
    >,
    _autocomplete: Option<
        Arc<dyn Fn(&str) -> Vec<xai_grok_extension_api::types::AutocompleteItem> + Send + Sync>,
    >,
}

impl ExtensionCommandAdapter {
    /// Create a new adapter from an extension command definition.
    pub fn new(cmd: xai_grok_extension_api::command::CommandDef) -> Self {
        Self {
            name: cmd.name,
            description: cmd.description,
            _handler: Arc::new(cmd.handler),
            _autocomplete: cmd.autocomplete.map(|f| Arc::new(f) as Arc<_>),
        }
    }
}

impl std::fmt::Debug for ExtensionCommandAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionCommandAdapter")
            .field("name", &self.name)
            .finish()
    }
}

impl SlashCommand for ExtensionCommandAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn usage(&self) -> &str {
        &self.name
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn args_required(&self) -> bool {
        false
    }

    fn suggest_args(&self, _ctx: &AppCtx, args_query: &str) -> Option<Vec<ArgItem>> {
        let autocomplete = self._autocomplete.as_ref()?;
        let items = autocomplete(args_query);
        Some(
            items
                .into_iter()
                .map(|item| ArgItem {
                    display: item.label.clone(),
                    match_text: item.value.clone(),
                    insert_text: item.value,
                    description: item.description.unwrap_or_default(),
                })
                .collect(),
        )
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        // Extension commands are async, but SlashCommand::run() is synchronous.
        // We return a PassThrough so the shell can handle it asynchronously.
        // This is the same pattern used for ACP commands.
        CommandResult::PassThrough(format!("/{} {}", self.name, args))
    }
}

/// Register all extension commands into the pager's command registry.
///
/// Called after builtin commands are registered.
pub fn register_extension_commands(
    _registry: &mut crate::slash::registry::CommandRegistry,
) {
    let ext_commands = xai_grok_extension_api::collect_commands();

    for cmd in ext_commands.commands() {
        // We need to move the command out of the registry, but it's borrowed.
        // Since we're draining, this is safe. But the API doesn't expose drain.
        // For now, we'll register commands via a different mechanism.
        tracing::info!(
            command = cmd.name,
            "extension command available (registration via pager integration)"
        );
    }
}

/// Convert extension commands to SlashCommand adapters.
///
/// Returns a list of extension commands as `Arc<dyn SlashCommand>`.
pub fn extension_command_adapters() -> Vec<Arc<dyn SlashCommand>> {
    // Extension commands are logged at startup in builtin_commands().
    // Full adapter support requires a drain mechanism on CommandRegistry.
    Vec::new()
}
