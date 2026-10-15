//! C# `Arcana.Plugins.Services.MessageBus`.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use arcana_core::AppResult;
use arcana_plugin_api::{AnyHandler, AnyRequestHandler, MessageBus, Subscription};

#[derive(Default)]
struct Inner {
    next_id: u64,
    subscribers: HashMap<TypeId, Vec<(u64, AnyHandler)>>,
    handlers: HashMap<(TypeId, TypeId), AnyRequestHandler>,
}

/// In-process pub/sub plus request/response.
///
/// C# behaviour kept: subscribers run in subscription order on a snapshot, a failing (here:
/// panicking) subscriber does not stop the rest, `RegisterHandler` silently replaces an
/// existing handler for the same request/response pair, and a request with no handler or one
/// that exceeds its timeout yields `None`.
#[derive(Clone, Default)]
pub struct MessageBusImpl {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for MessageBusImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MessageBusImpl")
    }
}

impl MessageBusImpl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Subscriber count for a message type (diagnostics / tests).
    #[must_use]
    pub fn subscriber_count(&self, type_id: TypeId) -> usize {
        self.lock().subscribers.get(&type_id).map_or(0, Vec::len)
    }
}

impl MessageBus for MessageBusImpl {
    fn publish_any(&self, type_id: TypeId, message: &(dyn Any + Send + Sync)) {
        let snapshot: Vec<AnyHandler> = self
            .lock()
            .subscribers
            .get(&type_id)
            .map(|v| v.iter().map(|(_, h)| h.clone()).collect())
            .unwrap_or_default();
        for h in snapshot {
            let _ = catch_unwind(AssertUnwindSafe(|| h(message)));
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

    fn request_any(
        &self,
        request_type: TypeId,
        response_type: TypeId,
        request: &(dyn Any + Send + Sync),
        timeout: Option<Duration>,
    ) -> AppResult<Option<Box<dyn Any + Send>>> {
        let handler = self
            .lock()
            .handlers
            .get(&(request_type, response_type))
            .cloned();
        let Some(handler) = handler else {
            return Ok(None);
        };
        // Handlers are synchronous, so the timeout cannot pre-empt; a reply that arrives late
        // is discarded, which is what the C# caller observes (`default`).
        let start = Instant::now();
        let out = handler(request)?;
        if timeout.is_some_and(|t| start.elapsed() > t) {
            return Ok(None);
        }
        Ok(Some(out))
    }

    fn register_handler_any(
        &self,
        request_type: TypeId,
        response_type: TypeId,
        handler: AnyRequestHandler,
    ) -> AppResult<Subscription> {
        let key = (request_type, response_type);
        // C# `_handlers[key] = handler` — last registration wins.
        self.lock().handlers.insert(key, handler);
        let inner = Arc::downgrade(&self.inner);
        Ok(Subscription::new(move || {
            if let Some(inner) = inner.upgrade() {
                // C# `TryRemove(key)` removes whatever is registered now, even a newer
                // handler. Kept as-is (quirk).
                inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .handlers
                    .remove(&key);
            }
        }))
    }
}
