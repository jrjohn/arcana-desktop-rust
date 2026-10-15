//! C# `Arcana.Plugins.Services.SharedStateStore`.

use std::any::Any;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_plugin_api::{SharedStateStore, StateHandler, Subscription};

type Value = Arc<dyn Any + Send + Sync>;

#[derive(Default)]
struct Inner {
    next_id: u64,
    state: HashMap<String, Value>,
    watchers: HashMap<String, Vec<(u64, StateHandler)>>,
}

/// Cross-plugin key/value store with change watchers. `Set` notifies with the new value,
/// `Remove` with `None` (C# `default`); watcher panics are ignored.
#[derive(Clone, Default)]
pub struct SharedStateStoreImpl {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for SharedStateStoreImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedStateStoreImpl")
    }
}

impl SharedStateStoreImpl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn notify(&self, key: &str, value: Option<&Value>) {
        let snapshot: Vec<StateHandler> = self
            .lock()
            .watchers
            .get(key)
            .map(|v| v.iter().map(|(_, h)| h.clone()).collect())
            .unwrap_or_default();
        for h in snapshot {
            let v = value.map(|v| &**v as &(dyn Any + Send + Sync));
            let _ = catch_unwind(AssertUnwindSafe(|| h(v)));
        }
    }
}

impl SharedStateStore for SharedStateStoreImpl {
    fn get_any(&self, key: &str) -> Option<Value> {
        self.lock().state.get(key).cloned()
    }

    fn set_any(&self, key: &str, value: Value) {
        self.lock().state.insert(key.to_string(), value.clone());
        self.notify(key, Some(&value));
    }

    fn remove(&self, key: &str) -> bool {
        let removed = self.lock().state.remove(key).is_some();
        if removed {
            self.notify(key, None);
        }
        removed
    }

    fn contains_key(&self, key: &str) -> bool {
        self.lock().state.contains_key(key)
    }

    fn on_change_any(&self, key: &str, handler: StateHandler) -> Subscription {
        let id = {
            let mut g = self.lock();
            g.next_id += 1;
            let id = g.next_id;
            g.watchers
                .entry(key.to_string())
                .or_default()
                .push((id, handler));
            id
        };
        let inner = Arc::downgrade(&self.inner);
        let key = key.to_string();
        Subscription::new(move || {
            if let Some(inner) = inner.upgrade()
                && let Some(v) = inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .watchers
                    .get_mut(&key)
            {
                v.retain(|(i, _)| *i != id);
            }
        })
    }
}
