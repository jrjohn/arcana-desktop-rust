//! C# `ActivationEventService`: plugins waiting for an activation event.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use arcana_core::AppResult;
use arcana_plugin_api::{ActivationEventType, activation_events};

use crate::manifest_service::event_matches;

/// C# `PluginActivatedEventArgs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginActivated {
    pub plugin_id: String,
    pub activation_event: String,
    pub activation_time: Duration,
}

pub type ActivationCallback = Arc<dyn Fn(&str) -> AppResult<()> + Send + Sync>;
pub type ActivatedHandler = Arc<dyn Fn(&PluginActivated) + Send + Sync>;

#[derive(Default)]
struct Inner {
    pending: BTreeMap<String, Vec<String>>,
    activated: BTreeSet<String>,
    callback: Option<ActivationCallback>,
    handlers: Vec<ActivatedHandler>,
}

/// C# `ActivationEventService`.
#[derive(Default)]
pub struct ActivationEventService {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for ActivationEventService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivationEventService")
            .field("pending", &self.lock().pending.len())
            .finish_non_exhaustive()
    }
}

impl ActivationEventService {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn set_activation_callback(&self, callback: ActivationCallback) {
        self.lock().callback = Some(callback);
    }

    /// C# `PluginActivated += ...`.
    pub fn on_plugin_activated(&self, handler: ActivatedHandler) {
        self.lock().handlers.push(handler);
    }

    /// Registers only when `events` is non-empty (C# behaviour).
    pub fn register_pending_plugin(&self, plugin_id: &str, events: &[String]) {
        if !events.is_empty() {
            self.lock()
                .pending
                .insert(plugin_id.to_string(), events.to_vec());
        }
    }

    pub fn mark_activated(&self, plugin_id: &str) {
        let mut inner = self.lock();
        inner.pending.remove(plugin_id);
        inner.activated.insert(plugin_id.to_string());
    }

    /// Forget a plugin entirely (uninstall / reinstall).
    pub fn forget(&self, plugin_id: &str) {
        let mut inner = self.lock();
        inner.pending.remove(plugin_id);
        inner.activated.remove(plugin_id);
    }

    /// C# `FireAsync(ActivationEventType, string?)`. `Unknown`, `Star` and
    /// `OnWorkspaceContains` map to no event string in C# and fire nothing.
    pub fn fire_type(&self, kind: ActivationEventType, argument: Option<&str>) -> Vec<String> {
        match kind {
            ActivationEventType::Unknown
            | ActivationEventType::Star
            | ActivationEventType::OnWorkspaceContains => Vec::new(),
            other => self.fire(&activation_events::format(other, argument)),
        }
    }

    /// C# `FireAsync(string)`. Returns the plugins whose activation succeeded through the
    /// callback; without a callback nothing is activated (C# logs and returns).
    pub fn fire(&self, event: &str) -> Vec<String> {
        let (kind, arg) = activation_events::parse(event);
        let targets = self.plugins_for_event(kind, arg.as_deref());
        let mut done = Vec::new();
        for id in targets {
            let (callback, already) = {
                let inner = self.lock();
                (inner.callback.clone(), inner.activated.contains(&id))
            };
            let Some(callback) = callback else { continue };
            if already {
                continue;
            }
            let started = Instant::now();
            match callback(&id) {
                Ok(()) => {
                    self.mark_activated(&id);
                    let e = PluginActivated {
                        plugin_id: id.clone(),
                        activation_event: event.to_string(),
                        activation_time: started.elapsed(),
                    };
                    let handlers = self.lock().handlers.clone();
                    for h in &handlers {
                        h(&e);
                    }
                    done.push(id);
                }
                Err(err) => {
                    tracing::error!(plugin = %id, %event, error = %err, "activation failed");
                }
            }
        }
        done
    }

    /// C# `GetPluginsForEvent` (skips already-activated plugins).
    #[must_use]
    pub fn plugins_for_event(
        &self,
        kind: ActivationEventType,
        argument: Option<&str>,
    ) -> Vec<String> {
        let inner = self.lock();
        inner
            .pending
            .iter()
            .filter(|(id, _)| !inner.activated.contains(*id))
            .filter(|(_, events)| event_matches(events, kind, argument))
            .map(|(id, _)| id.clone())
            .collect()
    }

    #[must_use]
    pub fn is_pending_activation(&self, plugin_id: &str) -> bool {
        let inner = self.lock();
        inner.pending.contains_key(plugin_id) && !inner.activated.contains(plugin_id)
    }

    #[must_use]
    pub fn pending_plugins(&self) -> BTreeMap<String, Vec<String>> {
        let inner = self.lock();
        inner
            .pending
            .iter()
            .filter(|(id, _)| !inner.activated.contains(*id))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}
