//! C# `Arcana.Plugins.Services.CommandServiceImpl`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_plugin_api::{
    CommandArgs, CommandHandler, CommandService, Subscription, validate_command_id,
};

/// `ContributionValidationException` as an [`AppError::Validation`].
pub(crate) fn validation_error(prefix: &str, errors: Vec<String>) -> AppError {
    AppError::validation(format!("{prefix}: {}", errors.join("; ")), errors)
}

/// C# `InvalidOperationException`.
pub(crate) fn invalid_operation(message: String) -> AppError {
    AppError::Plugin {
        code: ErrorCode::BadRequest,
        message,
        plugin_id: None,
    }
}

#[derive(Default)]
struct Inner {
    /// Insertion order, for a deterministic [`CommandService::commands`] (C#
    /// `ConcurrentDictionary.Keys` has no defined order).
    order: Vec<String>,
    commands: HashMap<String, CommandHandler>,
}

/// Command registry. Duplicate ids are rejected; ids are validated with
/// [`validate_command_id`].
#[derive(Clone, Default)]
pub struct CommandServiceImpl {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for CommandServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandServiceImpl")
            .field("commands", &self.commands())
            .finish()
    }
}

impl CommandServiceImpl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// C# `RegisterCommand<T>`: the first argument downcast to `T`, `None` when missing or of
    /// another type (C# passes `default`).
    ///
    /// # Errors
    /// See [`CommandService::register_command`].
    pub fn register_typed<T: 'static>(
        &self,
        command_id: &str,
        handler: impl Fn(Option<&T>) -> AppResult<()> + Send + Sync + 'static,
    ) -> AppResult<Subscription> {
        self.register_command(
            command_id,
            Arc::new(move |args: &CommandArgs| {
                handler(args.first().and_then(|a| a.downcast_ref::<T>()))
            }),
        )
    }
}

impl CommandService for CommandServiceImpl {
    fn register_command(
        &self,
        command_id: &str,
        handler: CommandHandler,
    ) -> AppResult<Subscription> {
        let v = validate_command_id(command_id);
        if !v.is_valid {
            return Err(validation_error("Command validation failed", v.errors));
        }
        {
            let mut g = self.lock();
            if g.commands.contains_key(command_id) {
                return Err(invalid_operation(format!(
                    "Command already registered: {command_id}"
                )));
            }
            g.commands.insert(command_id.to_string(), handler);
            g.order.push(command_id.to_string());
        }
        tracing::debug!("Registered command: {command_id}");
        let inner = Arc::downgrade(&self.inner);
        let id = command_id.to_string();
        Ok(Subscription::new(move || {
            if let Some(inner) = inner.upgrade() {
                let mut g = inner.lock().unwrap_or_else(PoisonError::into_inner);
                g.commands.remove(&id);
                g.order.retain(|c| *c != id);
            }
        }))
    }

    fn execute(&self, command_id: &str, args: CommandArgs) -> AppResult<()> {
        let handler = self.lock().commands.get(command_id).cloned();
        let handler =
            handler.ok_or_else(|| invalid_operation(format!("Command not found: {command_id}")))?;
        handler(&args)
    }

    fn commands(&self) -> Vec<String> {
        self.lock().order.clone()
    }

    fn has_command(&self, command_id: &str) -> bool {
        self.lock().commands.contains_key(command_id)
    }
}
