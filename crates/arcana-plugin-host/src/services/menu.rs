//! C# `Arcana.Plugins.Services.MenuRegistry`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_core::AppResult;
use arcana_plugin_api::{
    MenuItemDefinition, MenuLocation, MenuRegistry, Subscription, validate_menu_item,
};

use super::command::{invalid_operation, validation_error};

#[derive(Default)]
struct Inner {
    /// Insertion order, so equal `order` values sort stably and deterministically.
    items: Vec<MenuItemDefinition>,
}

/// Menu contributions. `MenusChanged` is the [`MenuRegistry::revision`] counter.
#[derive(Clone, Default)]
pub struct MenuRegistryImpl {
    inner: Arc<Mutex<Inner>>,
    revision: Arc<AtomicU64>,
}

impl std::fmt::Debug for MenuRegistryImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuRegistryImpl")
            .field("items", &self.lock().items.len())
            .finish()
    }
}

fn validate(item: &MenuItemDefinition) -> AppResult<()> {
    let r = validate_menu_item(item);
    for w in &r.warnings {
        tracing::warn!("Menu item validation warning: {w}");
    }
    if r.is_valid {
        Ok(())
    } else {
        Err(validation_error("Menu item validation failed", r.errors))
    }
}

fn sorted(mut v: Vec<MenuItemDefinition>) -> Vec<MenuItemDefinition> {
    v.sort_by_key(|m| m.order);
    v
}

impl MenuRegistryImpl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn changed(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
    fn remover(&self, ids: Vec<String>) -> Subscription {
        let inner = Arc::downgrade(&self.inner);
        let revision = self.revision.clone();
        Subscription::new(move || {
            if let Some(inner) = inner.upgrade() {
                inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .items
                    .retain(|m| !ids.contains(&m.id));
                revision.fetch_add(1, Ordering::SeqCst);
            }
        })
    }
}

impl MenuRegistry for MenuRegistryImpl {
    fn register_menu_item(&self, item: MenuItemDefinition) -> AppResult<Subscription> {
        validate(&item)?;
        {
            let mut g = self.lock();
            if g.items.iter().any(|m| m.id == item.id) {
                return Err(invalid_operation(format!(
                    "Menu item already registered: {}",
                    item.id
                )));
            }
            g.items.push(item.clone());
        }
        self.changed();
        Ok(self.remover(vec![item.id]))
    }

    /// C# quirk kept: batch registration overwrites existing ids instead of rejecting them,
    /// and disposing the batch removes those ids even if they were registered before.
    fn register_menu_items(&self, items: Vec<MenuItemDefinition>) -> AppResult<Subscription> {
        for i in &items {
            validate(i)?;
        }
        {
            let mut g = self.lock();
            for item in &items {
                if let Some(slot) = g.items.iter_mut().find(|m| m.id == item.id) {
                    *slot = item.clone();
                } else {
                    g.items.push(item.clone());
                }
            }
        }
        self.changed();
        Ok(self.remover(items.into_iter().map(|i| i.id).collect()))
    }

    fn menu_items(&self, location: MenuLocation) -> Vec<MenuItemDefinition> {
        sorted(
            self.lock()
                .items
                .iter()
                .filter(|m| m.location == location)
                .cloned()
                .collect(),
        )
    }

    fn menu_items_for_module(
        &self,
        location: MenuLocation,
        module_id: &str,
    ) -> Vec<MenuItemDefinition> {
        sorted(
            self.lock()
                .items
                .iter()
                .filter(|m| m.location == location && m.module_id.as_deref() == Some(module_id))
                .cloned()
                .collect(),
        )
    }

    fn all_menu_items(&self) -> Vec<MenuItemDefinition> {
        sorted(self.lock().items.clone())
    }

    fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }
}
