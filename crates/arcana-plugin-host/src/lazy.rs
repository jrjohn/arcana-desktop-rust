//! C# `LazyContributionService`: register a not-yet-loaded plugin's manifest contributions
//! with placeholders that activate the plugin on first use.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_core::AppResult;
use arcana_plugin_api::{
    ActivationEventType, CommandService, LocalizationService, ManifestMenuDefinition,
    ManifestViewDefinition, MenuItemDefinition, MenuLocation, MenuRegistry, PluginManifest,
    Subscription, ViewDefinition, ViewRegistry, ViewType,
};

use crate::activation::ActivationEventService;

#[derive(Default)]
struct Inner {
    view_to_plugin: BTreeMap<String, String>,
    command_to_plugin: BTreeMap<String, String>,
    menu_to_plugin: BTreeMap<String, String>,
    disposables: BTreeMap<String, Vec<Subscription>>,
}

/// C# `LazyContributionService`.
pub struct LazyContributionService {
    menus: Arc<dyn MenuRegistry>,
    views: Arc<dyn ViewRegistry>,
    commands: Arc<dyn CommandService>,
    activation: Arc<ActivationEventService>,
    localization: Arc<dyn LocalizationService>,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for LazyContributionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LazyContributionService")
            .finish_non_exhaustive()
    }
}

impl LazyContributionService {
    #[must_use]
    pub fn new(
        menus: Arc<dyn MenuRegistry>,
        views: Arc<dyn ViewRegistry>,
        commands: Arc<dyn CommandService>,
        activation: Arc<ActivationEventService>,
        localization: Arc<dyn LocalizationService>,
    ) -> Self {
        Self {
            menus,
            views,
            commands,
            activation,
            localization,
            inner: Mutex::default(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn title(&self, key: Option<&str>, title: Option<&str>, id: &str) -> String {
        match key.filter(|k| !k.is_empty()) {
            Some(k) => self.localization.get_from_any_plugin(k),
            None => title.unwrap_or(id).to_string(),
        }
    }

    /// C# `RegisterManifestContributionsAsync`.
    pub fn register_manifest_contributions(&self, manifest: &PluginManifest, plugin_dir: &Path) {
        let id = manifest.id.clone();
        self.load_localization(manifest, plugin_dir);
        let mut subs = Vec::new();
        let mut views = Vec::new();
        let mut menus = Vec::new();
        let mut commands = Vec::new();
        if let Some(c) = &manifest.contributes {
            for v in c.views.iter().flatten() {
                match self.register_lazy_view(v) {
                    Ok(sub) => subs.push(sub),
                    Err(e) => tracing::warn!(view = %v.id, error = %e, "lazy view"),
                }
                views.push(v.id.clone());
            }
            for m in c.menus.iter().flatten() {
                match self.menus.register_menu_item(self.convert_menu(m)) {
                    Ok(sub) => subs.push(sub),
                    Err(e) => tracing::warn!(menu = %m.id, error = %e, "lazy menu"),
                }
                menus.push(m.id.clone());
            }
            for cmd in c.commands.iter().flatten() {
                match self.register_lazy_command(&cmd.id) {
                    Ok(s) => {
                        subs.push(s);
                        commands.push(cmd.id.clone());
                    }
                    Err(e) => tracing::warn!(command = %cmd.id, error = %e, "lazy command"),
                }
            }
        }
        let mut inner = self.lock();
        for v in views {
            inner.view_to_plugin.insert(v, id.clone());
        }
        for m in menus {
            inner.menu_to_plugin.insert(m, id.clone());
        }
        for c in commands {
            inner.command_to_plugin.insert(c, id.clone());
        }
        inner.disposables.insert(id, subs);
    }

    fn load_localization(&self, manifest: &PluginManifest, dir: &Path) {
        for (culture, rel) in manifest.l10n.iter().flatten() {
            let Ok(json) = std::fs::read_to_string(dir.join(rel)) else {
                continue;
            };
            match serde_json::from_str::<HashMap<String, String>>(&json) {
                Ok(res) => {
                    self.localization
                        .register_plugin_resources(&manifest.id, culture, res);
                }
                Err(e) => tracing::warn!(plugin = %manifest.id, %culture, error = %e, "l10n"),
            }
        }
    }

    fn register_lazy_view(&self, v: &ManifestViewDefinition) -> AppResult<Subscription> {
        let def = ViewDefinition {
            id: v.id.clone(),
            title: self.title(Some(&v.title_key), v.title.as_deref(), &v.id),
            title_key: Some(v.title_key.clone()).filter(|k| !k.is_empty()),
            icon: v.icon.clone(),
            view_type: ViewType::parse(&v.view_type).unwrap_or_default(),
            view_class_name: v.view_class.clone(),
            can_have_multiple_instances: v.can_have_multiple_instances,
            category: v.category.clone(),
            order: v.order,
            module_id: v.module_id.clone(),
            is_module_default_tab: v.is_module_default_tab,
            module_tab_order: v.module_tab_order,
        };
        let view_sub = self.views.register_view(def)?;
        let activation = self.activation.clone();
        let view_id = v.id.clone();
        // C#: fires onView and returns null; the caller retries once the real view exists.
        let factory_sub = self.views.register_view_factory(
            &v.id,
            Arc::new(move || {
                activation.fire_type(ActivationEventType::OnView, Some(&view_id));
                Box::new(()) as arcana_plugin_api::ViewInstance
            }),
        );
        Ok(Subscription::all(vec![view_sub, factory_sub]))
    }

    fn convert_menu(&self, m: &ManifestMenuDefinition) -> MenuItemDefinition {
        MenuItemDefinition {
            id: m.id.clone(),
            title: self.title(m.title_key.as_deref(), m.title.as_deref(), &m.id),
            location: MenuLocation::parse(&m.location).unwrap_or(MenuLocation::MainMenu),
            parent_id: m.parent_id.clone(),
            icon: m.icon.clone(),
            tooltip: None,
            shortcut: m.shortcut.clone(),
            command: m.command.clone(),
            order: m.order,
            group: m.group.clone(),
            when: m.when.clone(),
            is_separator: m.is_separator,
            children: m
                .children
                .iter()
                .flatten()
                .map(|c| self.convert_menu(c))
                .collect(),
            module_id: m.module_id.clone(),
        }
    }

    fn register_lazy_command(&self, command_id: &str) -> AppResult<Subscription> {
        let activation = self.activation.clone();
        let commands = Arc::downgrade(&self.commands);
        let id = command_id.to_string();
        self.commands.register_command(
            command_id,
            Arc::new(move |_args| {
                activation.fire_type(ActivationEventType::OnCommand, Some(&id));
                // C# re-executes with the original args; `CommandArgs` is not `Clone`, so the
                // re-run gets no arguments.
                if let Some(c) = commands.upgrade()
                    && c.has_command(&id)
                {
                    return c.execute(&id, Vec::new());
                }
                Ok(())
            }),
        )
    }

    /// C# `OnPluginActivated`: drop the placeholders, the plugin registered the real ones.
    pub fn on_plugin_activated(&self, plugin_id: &str) {
        let subs = {
            let mut inner = self.lock();
            inner.view_to_plugin.retain(|_, p| p != plugin_id);
            inner.command_to_plugin.retain(|_, p| p != plugin_id);
            inner.menu_to_plugin.retain(|_, p| p != plugin_id);
            inner.disposables.remove(plugin_id)
        };
        subs.into_iter().flatten().for_each(Subscription::dispose);
    }

    #[must_use]
    pub fn plugin_for_view(&self, view_id: &str) -> Option<String> {
        self.lock().view_to_plugin.get(view_id).cloned()
    }

    #[must_use]
    pub fn plugin_for_command(&self, command_id: &str) -> Option<String> {
        self.lock().command_to_plugin.get(command_id).cloned()
    }
}
