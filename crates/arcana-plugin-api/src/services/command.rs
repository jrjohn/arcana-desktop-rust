//! `CommandService` and `CommandDefinition`.

use std::any::Any;
use std::sync::Arc;

use arcana_core::AppResult;

use crate::services::Subscription;

/// C# `params object?[] args`.
pub type CommandArgs = Vec<Box<dyn Any + Send>>;
/// C# `Func<object?[], Task>`.
pub type CommandHandler = Arc<dyn Fn(&CommandArgs) -> AppResult<()> + Send + Sync>;

/// C# `CommandDefinition`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandDefinition {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub shortcut: Option<String>,
    pub when: Option<String>,
}

/// C# `CommandService`.
pub trait CommandService: Send + Sync {
    /// # Errors
    /// Invalid command id (C# `ArgumentException`).
    fn register_command(
        &self,
        command_id: &str,
        handler: CommandHandler,
    ) -> AppResult<Subscription>;
    /// # Errors
    /// Unknown command (C# `InvalidOperationException`) or the handler's own error.
    fn execute(&self, command_id: &str, args: CommandArgs) -> AppResult<()>;
    fn commands(&self) -> Vec<String>;
    fn has_command(&self, command_id: &str) -> bool;
}
