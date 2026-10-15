//! `IDisposable` returned by every `Register*` / `Subscribe*` call.

use std::fmt;

/// Undoes one registration when [`Subscription::dispose`] is called.
///
/// Unlike an RAII guard, dropping it does **not** unregister — same as forgetting to dispose a
/// C# `IDisposable`. Plugins hand subscriptions to [`crate::PluginContext::track`] so the host
/// disposes them on deactivation (C# `IPluginContext.Subscriptions`).
#[must_use = "keep the subscription or hand it to PluginContext::track"]
pub struct Subscription(Option<Box<dyn FnOnce() + Send>>);

impl Subscription {
    pub fn new(undo: impl FnOnce() + Send + 'static) -> Self {
        Self(Some(Box::new(undo)))
    }
    /// A subscription with nothing to undo.
    pub fn empty() -> Self {
        Self(None)
    }
    /// Combine several into one (C# `RegisterMenuItems` composite disposable).
    pub fn all(items: Vec<Self>) -> Self {
        Self::new(move || items.into_iter().for_each(Self::dispose))
    }
    pub fn dispose(mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

impl fmt::Debug for Subscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() {
            "Subscription(active)"
        } else {
            "Subscription(empty)"
        })
    }
}
