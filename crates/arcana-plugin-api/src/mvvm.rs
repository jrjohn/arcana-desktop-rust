//! MVVM helpers (C# `Arcana.Plugins.Contracts.Mvvm`): `EffectSubject<T>`, `EffectSubject`,
//! `Unit`, `ReactiveViewModelBase`.
//!
//! C# swallows exceptions thrown by subscribers; the Rust equivalent of a throwing subscriber is
//! a panicking one, so [`EffectSubject::emit`] catches unwinds per subscriber.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_core::AppResult;

use crate::services::Subscription;

type Handler<T> = Arc<dyn Fn(&T) + Send + Sync>;

struct Inner<T> {
    next_id: u64,
    subscribers: Vec<(u64, Handler<T>)>,
    disposed: bool,
}

/// C# `EffectSubject<T>`: hot, non-replaying, one-shot effect stream.
pub struct EffectSubject<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Default for EffectSubject<T> {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                next_id: 0,
                subscribers: Vec::new(),
                disposed: false,
            })),
        }
    }
}

impl<T> std::fmt::Debug for EffectSubject<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EffectSubject")
    }
}

impl<T: 'static> EffectSubject<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner<T>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Deliver to a snapshot of current subscribers; a panicking subscriber does not stop the
    /// others. No-op after [`EffectSubject::dispose`].
    pub fn emit(&self, value: &T) {
        let snapshot: Vec<Handler<T>> = {
            let g = self.lock();
            if g.disposed {
                return;
            }
            g.subscribers.iter().map(|(_, h)| h.clone()).collect()
        };
        for h in snapshot {
            let _ = catch_unwind(AssertUnwindSafe(|| h(value)));
        }
    }

    /// C# `Subscribe(Action<T>)`; an empty subscription once disposed.
    pub fn subscribe(&self, on_next: impl Fn(&T) + Send + Sync + 'static) -> Subscription {
        let id = {
            let mut g = self.lock();
            if g.disposed {
                return Subscription::empty();
            }
            g.next_id += 1;
            let id = g.next_id;
            g.subscribers.push((id, Arc::new(on_next)));
            id
        };
        let inner = Arc::downgrade(&self.inner);
        Subscription::new(move || {
            if let Some(inner) = inner.upgrade() {
                inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .subscribers
                    .retain(|(i, _)| *i != id);
            }
        })
    }

    /// C# `Subscribe(Action<T>, Action<Exception>)`: errors from `on_next` go to `on_error`.
    pub fn subscribe_with_error<E: 'static>(
        &self,
        on_next: impl Fn(&T) -> Result<(), E> + Send + Sync + 'static,
        on_error: impl Fn(E) + Send + Sync + 'static,
    ) -> Subscription {
        self.subscribe(move |v| {
            if let Err(e) = on_next(v) {
                on_error(e);
            }
        })
    }

    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.lock().subscribers.len()
    }

    #[must_use]
    pub fn is_disposed(&self) -> bool {
        self.lock().disposed
    }

    /// Clear subscribers and stop delivering; idempotent.
    pub fn dispose(&self) {
        let mut g = self.lock();
        g.disposed = true;
        g.subscribers.clear();
    }
}

impl EffectSubject<Unit> {
    /// The non-generic C# `EffectSubject.Emit()`.
    pub fn fire(&self) {
        self.emit(&Unit);
    }
}

/// C# `readonly struct Unit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Unit;

impl Unit {
    pub const DEFAULT: Self = Self;
}

/// C# `ReactiveViewModelBase`'s disposable list.
#[derive(Debug, Default)]
pub struct DisposeBag {
    items: Vec<Subscription>,
    disposed: bool,
}

impl DisposeBag {
    /// C# `AddDisposable`.
    pub fn add(&mut self, s: Subscription) {
        self.items.push(s);
    }
    /// C# `Dispose()`; idempotent.
    pub fn dispose(&mut self) {
        if self.disposed {
            return;
        }
        self.items.drain(..).for_each(Subscription::dispose);
        self.disposed = true;
    }
    #[must_use]
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
}

/// C# `ReactiveViewModelBase` lifecycle hooks (the `ObservableObject` part is egui's
/// immediate-mode redraw and has no counterpart).
pub trait ReactiveViewModel {
    fn bag(&mut self) -> &mut DisposeBag;
    /// # Errors
    /// Implementation-defined.
    fn initialize(&mut self) -> AppResult<()> {
        Ok(())
    }
    /// # Errors
    /// Implementation-defined.
    fn cleanup(&mut self) -> AppResult<()> {
        Ok(())
    }
    fn add_disposable(&mut self, s: Subscription) {
        self.bag().add(s);
    }
    fn dispose(&mut self) {
        self.bag().dispose();
    }
}
