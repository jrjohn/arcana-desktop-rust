//! C# `PluginBase`: the state machine and helper methods every built-in plugin shares.
//!
//! A plugin implements [`PluginBehavior`] (the C# `override`s) and is wrapped in
//! [`BasePlugin`], which implements [`Plugin`].

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::path::Path;

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_plugin_api::{
    ApplicationEvent, CommandArgs, EventAggregatorExt, MenuItemDefinition, MessageBusExt, Plugin,
    PluginContext, PluginHealthCheck, PluginHotReload, PluginMetadata, PluginState,
    PluginUpgradeable, ViewDefinition,
};

/// The overridable parts of C# `PluginBase`.
pub trait PluginBehavior: Send + 'static {
    fn metadata(&self) -> PluginMetadata;
    /// C# `OnActivateAsync`.
    ///
    /// # Errors
    /// Fails activation; the plugin ends in [`PluginState::Error`].
    fn on_activate(&mut self, _base: &mut PluginBase) -> AppResult<()> {
        Ok(())
    }
    /// C# `RegisterContributions` (runs after `on_activate`).
    ///
    /// # Errors
    /// Fails activation.
    fn register_contributions(&mut self, _base: &mut PluginBase) -> AppResult<()> {
        Ok(())
    }
    /// C# `OnDeactivateAsync`.
    ///
    /// # Errors
    /// Fails deactivation; the plugin ends in [`PluginState::Error`].
    fn on_deactivate(&mut self, _base: &mut PluginBase) -> AppResult<()> {
        Ok(())
    }
    /// Embedded locale files (C# `LoadEmbeddedLocalization`): culture -> JSON text.
    fn embedded_locales(&self) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }
    fn as_health_check(&self) -> Option<&dyn PluginHealthCheck> {
        None
    }
    fn as_hot_reload(&mut self) -> Option<&mut dyn PluginHotReload> {
        None
    }
    fn as_upgradeable(&mut self) -> Option<&mut dyn PluginUpgradeable> {
        None
    }
    /// Downcast hook for tests and the UI.
    fn as_any(&self) -> &dyn Any;
}

/// State + context shared by every plugin (C# `PluginBase` fields and helpers).
#[derive(Debug)]
pub struct PluginBase {
    metadata: PluginMetadata,
    state: PluginState,
    context: Option<PluginContext>,
}

fn no_context() -> AppError {
    AppError::Plugin {
        code: ErrorCode::PluginLoadFailed,
        message: "Plugin is not activated (no context)".to_string(),
        plugin_id: None,
    }
}

impl PluginBase {
    #[must_use]
    pub fn new(metadata: PluginMetadata) -> Self {
        Self {
            metadata,
            state: PluginState::NotLoaded,
            context: None,
        }
    }

    #[must_use]
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    #[must_use]
    pub fn state(&self) -> PluginState {
        self.state
    }
    #[must_use]
    pub fn context(&self) -> Option<&PluginContext> {
        self.context.as_ref()
    }

    fn ctx(&self) -> AppResult<&PluginContext> {
        self.context.as_ref().ok_or_else(no_context)
    }

    /// C# `RegisterCommand(string, Func<Task>)`.
    ///
    /// # Errors
    /// No context, or the command service rejects the id.
    pub fn register_command(
        &self,
        command_id: &str,
        handler: impl Fn(&CommandArgs) -> AppResult<()> + Send + Sync + 'static,
    ) -> AppResult<()> {
        let ctx = self.ctx()?;
        let sub = ctx
            .commands
            .register_command(command_id, std::sync::Arc::new(handler))?;
        ctx.track(sub);
        Ok(())
    }

    /// # Errors
    /// No context.
    pub fn register_menu_item(&self, item: MenuItemDefinition) -> AppResult<()> {
        let ctx = self.ctx()?;
        ctx.track(ctx.menus.register_menu_item(item)?);
        Ok(())
    }

    /// # Errors
    /// No context.
    pub fn register_menu_items(&self, items: Vec<MenuItemDefinition>) -> AppResult<()> {
        let ctx = self.ctx()?;
        ctx.track(ctx.menus.register_menu_items(items)?);
        Ok(())
    }

    /// # Errors
    /// No context.
    pub fn register_view(&self, view: ViewDefinition) -> AppResult<()> {
        let ctx = self.ctx()?;
        ctx.track(ctx.views.register_view(view)?);
        Ok(())
    }

    /// # Errors
    /// No context.
    pub fn register_view_factory(
        &self,
        view_id: &str,
        factory: arcana_plugin_api::ViewFactory,
    ) -> AppResult<()> {
        let ctx = self.ctx()?;
        ctx.track(ctx.views.register_view_factory(view_id, factory));
        Ok(())
    }

    /// C# `Subscribe<TMessage>`.
    ///
    /// # Errors
    /// No context.
    pub fn subscribe<T: Any + Send + Sync>(
        &self,
        handler: impl Fn(&T) + Send + Sync + 'static,
    ) -> AppResult<()> {
        let ctx = self.ctx()?;
        ctx.track(ctx.message_bus.subscribe(handler));
        Ok(())
    }

    /// C# `SubscribeToEvent<TEvent>`.
    ///
    /// # Errors
    /// No context.
    pub fn subscribe_to_event<E: ApplicationEvent>(
        &self,
        handler: impl Fn(&E) + Send + Sync + 'static,
    ) -> AppResult<()> {
        let ctx = self.ctx()?;
        ctx.track(ctx.events.subscribe(handler));
        Ok(())
    }

    /// C# `PublishAsync<TMessage>`.
    ///
    /// # Errors
    /// No context.
    pub fn publish<T: Any + Send + Sync>(&self, message: &T) -> AppResult<()> {
        self.ctx()?.message_bus.publish(message);
        Ok(())
    }

    /// C# `L(key)`: the key itself when there is no context.
    #[must_use]
    pub fn l(&self, key: &str) -> String {
        self.context.as_ref().map_or_else(
            || key.to_string(),
            |c| c.localization.get_for_plugin(&self.metadata.id, key),
        )
    }

    /// C# `L(key, args)`.
    #[must_use]
    pub fn l_with(&self, key: &str, args: &[&dyn Display]) -> String {
        self.context.as_ref().map_or_else(
            || key.to_string(),
            |c| {
                c.localization
                    .get_for_plugin_with(&self.metadata.id, key, args)
            },
        )
    }

    /// C# `RegisterResources` (no-op without context, like C# `Context?.`).
    pub fn register_resources(&self, culture: &str, resources: HashMap<String, String>) {
        if let Some(c) = &self.context {
            c.localization
                .register_plugin_resources(&self.metadata.id, culture, resources);
        }
    }

    fn register_json(&self, culture: &str, json: &str) -> bool {
        match serde_json::from_str::<HashMap<String, String>>(json) {
            Ok(res) => {
                tracing::info!(plugin = %self.metadata.id, %culture, count = res.len(), "locale");
                self.register_resources(culture, res);
                true
            }
            Err(e) => {
                tracing::error!(plugin = %self.metadata.id, %culture, error = %e, "locale");
                false
            }
        }
    }

    /// C# `LoadExternalLocalizationAsync`: every `*.json` in `base` (default
    /// `<plugin_path>/locales`); returns how many files were registered.
    pub fn load_external_localization(&self, base: Option<&Path>) -> usize {
        let dir = match (base, &self.context) {
            (Some(b), _) => b.to_path_buf(),
            (None, Some(c)) => c.plugin_path.join("locales"),
            (None, None) => return 0,
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            tracing::warn!(path = %dir.display(), "Locales directory not found");
            return 0;
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        files
            .iter()
            .filter(|p| {
                let culture = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                std::fs::read_to_string(p).is_ok_and(|json| self.register_json(culture, &json))
            })
            .count()
    }

    /// C# `LoadLocalizationFileAsync`.
    pub fn load_localization_file(&self, culture: &str, path: &Path) -> bool {
        if let Ok(json) = std::fs::read_to_string(path) {
            self.register_json(culture, &json)
        } else {
            tracing::warn!(path = %path.display(), "Localization file not found");
            false
        }
    }

    /// C# `LoadEmbeddedLocalization`.
    #[must_use]
    pub fn load_embedded_localization(&self, locales: &[(&str, &str)]) -> usize {
        locales
            .iter()
            .filter(|(c, json)| self.register_json(c, json))
            .count()
    }
}

/// [`Plugin`] implementation around a [`PluginBehavior`].
#[derive(Debug)]
pub struct BasePlugin<B> {
    behavior: B,
    base: PluginBase,
}

impl<B: PluginBehavior> BasePlugin<B> {
    pub fn new(behavior: B) -> Self {
        let base = PluginBase::new(behavior.metadata());
        Self { behavior, base }
    }
    pub fn boxed(behavior: B) -> Box<dyn Plugin> {
        Box::new(Self::new(behavior))
    }
    #[must_use]
    pub fn behavior(&self) -> &B {
        &self.behavior
    }
    #[must_use]
    pub fn base(&self) -> &PluginBase {
        &self.base
    }
}

impl<B: PluginBehavior> Plugin for BasePlugin<B> {
    fn metadata(&self) -> &PluginMetadata {
        &self.base.metadata
    }

    fn state(&self) -> PluginState {
        self.base.state
    }

    /// C# `ActivateAsync`: Activating → `on_activate` → `register_contributions` → Active;
    /// any error → Error (and the error is returned).
    fn activate(&mut self, context: &PluginContext) -> AppResult<()> {
        self.base.state = PluginState::Activating;
        self.base.context = Some(context.clone());
        let locales = self.behavior.embedded_locales();
        let _ = self.base.load_embedded_localization(&locales);
        let result = self
            .behavior
            .on_activate(&mut self.base)
            .and_then(|()| self.behavior.register_contributions(&mut self.base));
        self.base.state = if result.is_ok() {
            PluginState::Active
        } else {
            PluginState::Error
        };
        result
    }

    /// C# `DeactivateAsync`: Deactivating → `on_deactivate` → dispose subscriptions → Deactivated.
    fn deactivate(&mut self) -> AppResult<()> {
        self.base.state = PluginState::Deactivating;
        match self.behavior.on_deactivate(&mut self.base) {
            Ok(()) => {
                if let Some(c) = &self.base.context {
                    c.dispose_subscriptions();
                }
                self.base.state = PluginState::Deactivated;
                Ok(())
            }
            Err(e) => {
                self.base.state = PluginState::Error;
                Err(e)
            }
        }
    }

    fn resources(&self) -> BTreeMap<String, BTreeMap<String, String>> {
        BTreeMap::new()
    }

    fn as_health_check(&self) -> Option<&dyn PluginHealthCheck> {
        self.behavior.as_health_check()
    }
    fn as_hot_reload(&mut self) -> Option<&mut dyn PluginHotReload> {
        self.behavior.as_hot_reload()
    }
    fn as_upgradeable(&mut self) -> Option<&mut dyn PluginUpgradeable> {
        self.behavior.as_upgradeable()
    }
}
