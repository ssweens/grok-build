//! Command types for the extension API.
//!
//! Extensions register slash commands, keyboard shortcuts, and CLI flags
//! via [`CommandRegistry`]. The pager's command dispatch queries this
//! alongside built-in commands.

use std::future::Future;
use std::pin::Pin;

use crate::types::AutocompleteItem;

/// A boxed future returned by command and shortcut handlers.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A slash command registered by an extension.
///
/// Commands are invoked by the user typing `/name` in the prompt.
/// They receive the arguments string and the extension context.
pub struct CommandDef {
    /// Command name (without the leading `/`).
    pub name: String,

    /// Human-readable description shown in help.
    pub description: String,

    /// The command handler.
    pub handler: Box<dyn Fn(String, Box<dyn crate::context::ExtensionContext>) -> BoxFuture<'static, ()> + Send + Sync>,

    /// Optional autocomplete provider for command arguments.
    pub autocomplete: Option<Box<dyn Fn(&str) -> Vec<AutocompleteItem> + Send + Sync>>,
}

impl std::fmt::Debug for CommandDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandDef")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

/// A keyboard shortcut registered by an extension.
pub struct ShortcutDef {
    /// Key combination (e.g., "ctrl+shift+p").
    pub key: String,

    /// Human-readable description.
    pub description: String,

    /// The shortcut handler.
    pub handler: Box<dyn Fn(Box<dyn crate::context::ExtensionContext>) -> BoxFuture<'static, ()> + Send + Sync>,
}

impl std::fmt::Debug for ShortcutDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShortcutDef")
            .field("key", &self.key)
            .field("description", &self.description)
            .finish()
    }
}

/// A CLI flag registered by an extension.
#[derive(Debug, Clone)]
pub struct FlagDef {
    /// Flag name (e.g., "plan").
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// The flag type.
    pub flag_type: FlagType,

    /// Default value.
    pub default: serde_json::Value,
}

/// Supported flag types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagType {
    Boolean,
    String,
    Integer,
}

/// Registry of extension-provided commands, shortcuts, and flags.
///
/// Populated by command packs at startup.
pub struct CommandRegistry {
    commands: Vec<CommandDef>,
    shortcuts: Vec<ShortcutDef>,
    flags: Vec<FlagDef>,
}

impl CommandRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            shortcuts: Vec::new(),
            flags: Vec::new(),
        }
    }

    /// Register a slash command.
    pub fn register_command(&mut self, command: CommandDef) {
        tracing::info!(
            command = command.name,
            "extension: registered command"
        );
        self.commands.push(command);
    }

    /// Register a keyboard shortcut.
    pub fn register_shortcut(&mut self, shortcut: ShortcutDef) {
        tracing::info!(
            key = shortcut.key,
            "extension: registered shortcut"
        );
        self.shortcuts.push(shortcut);
    }

    /// Register a CLI flag.
    pub fn register_flag(&mut self, flag: FlagDef) {
        tracing::info!(
            flag = flag.name,
            "extension: registered flag"
        );
        self.flags.push(flag);
    }

    /// All registered commands.
    pub fn commands(&self) -> &[CommandDef] {
        &self.commands
    }

    /// All registered shortcuts.
    pub fn shortcuts(&self) -> &[ShortcutDef] {
        &self.shortcuts
    }

    /// All registered flags.
    pub fn flags(&self) -> &[FlagDef] {
        &self.flags
    }

    /// Find a command by name.
    pub fn find_command(&self, name: &str) -> Option<&CommandDef> {
        self.commands.iter().find(|c| c.name == name)
    }

    /// Find a shortcut by key combination.
    pub fn find_shortcut(&self, key: &str) -> Option<&ShortcutDef> {
        self.shortcuts.iter().find(|s| s.key == key)
    }

    /// Find a flag by name.
    pub fn find_flag(&self, name: &str) -> Option<&FlagDef> {
        self.flags.iter().find(|f| f.name == name)
    }

    /// Whether any commands are registered.
    pub fn has_commands(&self) -> bool {
        !self.commands.is_empty()
    }

    /// Whether any shortcuts are registered.
    pub fn has_shortcuts(&self) -> bool {
        !self.shortcuts.is_empty()
    }

    /// Whether any flags are registered.
    pub fn has_flags(&self) -> bool {
        !self.flags.is_empty()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.shortcuts.is_empty() && self.flags.is_empty()
    }
}
