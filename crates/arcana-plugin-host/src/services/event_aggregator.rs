//! C# `Arcana.Plugins.Services.EventAggregator`.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_plugin_api::{AnyHandler, EventAggregator, Subscription};

#[derive(Default)]
struct Inner {
    next_id: u64,
    subscribers: HashMap<TypeId, Vec<(u64, AnyHandler)>>,
}

/// Typed application-event fan-out. Exact-type dispatch (C# keys on `typeof(TEvent)`, so a
/// subscriber to a base type does not see derived events); panicking handlers are isolated.
#[derive(Clone, Default)]
pub struct EventAggregatorImpl {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for EventAggregatorImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EventAggregatorImpl")
    }
}

impl EventAggregatorImpl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl EventAggregator for EventAggregatorImpl {
    fn publish_any(&self, type_id: TypeId, event: &(dyn Any + Send + Sync)) {
        let snapshot: Vec<AnyHandler> = self
            .lock()
            .subscribers
            .get(&type_id)
            .map(|v| v.iter().map(|(_, h)| h.clone()).collect())
            .unwrap_or_default();
        for h in snapshot {
            let _ = catch_unwind(AssertUnwindSafe(|| h(event)));
        }
    }

    fn subscribe_any(&self, type_id: TypeId, handler: AnyHandler) -> Subscription {
        let id = {
            let mut g = self.lock();
            g.next_id += 1;
            let id = g.next_id;
            g.subscribers
                .entry(type_id)
                .or_default()
                .push((id, handler));
            id
        };
        let inner = Arc::downgrade(&self.inner);
        Subscription::new(move || {
            if let Some(inner) = inner.upgrade()
                && let Some(v) = inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .subscribers
                    .get_mut(&type_id)
            {
                v.retain(|(i, _)| *i != id);
            }
        })
    }
}
