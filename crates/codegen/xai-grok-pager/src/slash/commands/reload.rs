//! `/reload` — Reload extension providers and skills.
//!
//! Refreshes dynamic model providers from config files and re-discovers
//! skills from disk. Use after changing `~/.grok/settings/dynamic-models.json`
//! or when servers come online.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// The `/reload` command.
pub struct ReloadCommand;

impl SlashCommand for ReloadCommand {
    fn name(&self) -> &str {
        "reload"
    }

    fn description(&self) -> &str {
        "Reload extension providers and skills"
    }

    fn usage(&self) -> &str {
        "/reload"
    }

    fn takes_args(&self) -> bool {
        false
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        // Refresh extension providers
        let providers = xai_grok_extension_api::refresh_providers();
        let provider_count = providers.len();
        let model_count = providers.all_models().len();

        // The actual model catalog update happens in the shell via the
        // provider bridge. For now, we return a message showing what was
        // reloaded. The full integration would trigger a model catalog
        // refresh in the shell.
        //
        // TODO: Wire into shell's model catalog refresh RPC when available.
        // For now, restart is required for model changes to take effect.

        CommandResult::Message(format!(
            "Reloaded {} extension provider(s) with {} model(s). \
             Note: model catalog changes require restart to take effect.",
            provider_count, model_count
        ))
    }
}
