//! C# `Arcana.Plugins.Services.ViewRegistry`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_core::AppResult;
use arcana_plugin_api::{
    Subscription, ViewDefinition, ViewFactory, ViewInstance, ViewRegistry, validate_view,
};

use super::command::{invalid_operation, validation_error};

#[derive(Default)]
struct Inner {
    views: Vec<ViewDefinition>,
    factories: HashMap<String, ViewFactory>,
}

/// View contributions and factories. `ViewsChanged` is the [`ViewRegistry::revision`] counter.
///
/// C# falls back to `Activator.CreateInstance(view.ViewClass)`; Rust has no reflection, so a
/// view without a factory yields `None`.
#[derive(Clone, Default)]
pub struct ViewRegistryImpl {
    inner: Arc<Mutex<Inner>>,
    revision: Arc<AtomicU64>,
}

impl std::fmt::Debug for ViewRegistryImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewRegistryImpl")
            .field("views", &self.lock().views.len())
            .finish()
    }
}

impl ViewRegistryImpl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ViewRegistry for ViewRegistryImpl {
    fn register_view(&self, view: ViewDefinition) -> AppResult<Subscription> {
        let r = validate_view(&view);
        for w in &r.warnings {
            tracing::warn!("View validation warning: {w}");
        }
        if !r.is_valid {
            return Err(validation_error("View validation failed", r.errors));
        }
        {
            let mut g = self.lock();
            if g.views.iter().any(|v| v.id == view.id) {
                return Err(invalid_operation(format!(
                    "View already registered: {}",
                    view.id
                )));
            }
            g.views.push(view.clone());
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
        let inner = Arc::downgrade(&self.inner);
        let revision = self.revision.clone();
        let id = view.id;
        Ok(Subscription::new(move || {
            if let Some(inner) = inner.upgrade() {
                let mut g = inner.lock().unwrap_or_else(PoisonError::into_inner);
                g.views.retain(|v| v.id != id);
                // C#: disposing the view also drops its factory.
                g.factories.remove(&id);
                revision.fetch_add(1, Ordering::SeqCst);
            }
        }))
    }

    /// C# behaviour: replaces any existing factory, needs no registered view, and does not
    /// raise `ViewsChanged`.
    fn register_view_factory(&self, view_id: &str, factory: ViewFactory) -> Subscription {
        self.lock().factories.insert(view_id.to_string(), factory);
        let inner = Arc::downgrade(&self.inner);
        let id = view_id.to_string();
        Subscription::new(move || {
            if let Some(inner) = inner.upgrade() {
                inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .factories
                    .remove(&id);
            }
        })
    }

    fn view(&self, view_id: &str) -> Option<ViewDefinition> {
        self.lock().views.iter().find(|v| v.id == view_id).cloned()
    }

    fn all_views(&self) -> Vec<ViewDefinition> {
        let mut v = self.lock().views.clone();
        v.sort_by_key(|v| v.order);
        v
    }

    fn module_default_tabs(&self, module_id: &str) -> Vec<ViewDefinition> {
        let mut v: Vec<_> = self
            .lock()
            .views
            .iter()
            .filter(|v| v.module_id.as_deref() == Some(module_id) && v.is_module_default_tab)
            .cloned()
            .collect();
        v.sort_by_key(|v| v.module_tab_order);
        v
    }

    fn create_view_instance(&self, view_id: &str) -> Option<ViewInstance> {
        let f = self.lock().factories.get(view_id).cloned();
        f.map(|f| f())
    }

    fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }
}
