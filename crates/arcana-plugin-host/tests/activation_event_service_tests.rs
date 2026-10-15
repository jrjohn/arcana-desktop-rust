//! C# `ActivationEventServiceTests` (22). The C# `ManifestService` mock is unused by the
//! service and has no Rust counterpart.

use std::sync::{Arc, Mutex, PoisonError};

use arcana_core::{AppError, ErrorCode};
use arcana_plugin_api::{ActivationEventType, activation_events as ae};
use arcana_plugin_host::activation::{ActivationEventService, PluginActivated};

fn ev(s: &str) -> Vec<String> {
    vec![s.to_string()]
}

/// Service with a callback recording activated ids.
fn recording() -> (ActivationEventService, Arc<Mutex<Vec<String>>>) {
    let s = ActivationEventService::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let l = log.clone();
    s.set_activation_callback(Arc::new(move |id| {
        l.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(id.to_string());
        Ok(())
    }));
    (s, log)
}

fn got(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    log.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

// RegisterPendingPlugin_WithEvents_ShouldBePending
#[test]
fn register_with_events_pending() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    assert!(s.is_pending_activation("plugin-a"));
}

// RegisterPendingPlugin_WithNoEvents_ShouldNotBePending
#[test]
fn register_no_events_not_pending() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-a", &[]);
    assert!(!s.is_pending_activation("plugin-a"));
}

// IsPendingActivation_UnregisteredPlugin_ShouldReturnFalse
#[test]
fn unregistered_not_pending() {
    assert!(!ActivationEventService::new().is_pending_activation("nonexistent-plugin"));
}

// MarkActivated_ShouldRemoveFromPending
#[test]
fn mark_activated_removes() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.mark_activated("plugin-a");
    assert!(!s.is_pending_activation("plugin-a"));
}

// GetPendingPlugins_ShouldReturnAllPending
#[test]
fn pending_returns_all() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.register_pending_plugin("plugin-b", &ev(&ae::for_command("test")));
    let p = s.pending_plugins();
    assert!(p.contains_key("plugin-a") && p.contains_key("plugin-b"));
}

// GetPendingPlugins_ShouldExcludeActivated
#[test]
fn pending_excludes_activated() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.register_pending_plugin("plugin-b", &ev(ae::ON_STARTUP));
    s.mark_activated("plugin-a");
    let p = s.pending_plugins();
    assert!(!p.contains_key("plugin-a") && p.contains_key("plugin-b"));
}

// FireAsync_StartupEvent_ShouldActivateOnStartupPlugins
#[test]
fn fire_startup_activates() {
    let (s, log) = recording();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.fire_type(ActivationEventType::OnStartup, None);
    assert_eq!(got(&log), ["plugin-a"]);
}

// FireAsync_StartupEvent_ShouldMarkPluginAsActivated
#[test]
fn fire_startup_marks_activated() {
    let (s, _) = recording();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.fire_type(ActivationEventType::OnStartup, None);
    assert!(!s.is_pending_activation("plugin-a"));
}

// FireAsync_CommandEvent_ShouldActivateMatchingPlugin
#[test]
fn fire_command_matches() {
    let (s, log) = recording();
    s.register_pending_plugin("plugin-cmd", &ev(&ae::for_command("order.new")));
    s.fire_type(ActivationEventType::OnCommand, Some("order.new"));
    assert_eq!(got(&log), ["plugin-cmd"]);
}

// FireAsync_CommandEvent_WrongCommand_ShouldNotActivate
#[test]
fn fire_wrong_command() {
    let (s, log) = recording();
    s.register_pending_plugin("plugin-cmd", &ev(&ae::for_command("order.new")));
    s.fire_type(ActivationEventType::OnCommand, Some("customer.edit"));
    assert!(got(&log).is_empty());
}

// FireAsync_WildcardPlugin_ShouldActivateOnAnyEvent
#[test]
fn fire_wildcard() {
    let (s, log) = recording();
    s.register_pending_plugin("wildcard-plugin", &ev(ae::STAR));
    s.fire_type(ActivationEventType::OnStartup, None);
    assert_eq!(got(&log), ["wildcard-plugin"]);
}

// FireAsync_AlreadyActivatedPlugin_ShouldNotActivateAgain
#[test]
fn fire_twice_activates_once() {
    let (s, log) = recording();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.fire_type(ActivationEventType::OnStartup, None);
    s.fire_type(ActivationEventType::OnStartup, None);
    assert_eq!(got(&log).len(), 1);
}

// FireAsync_NoCallback_ShouldNotThrow
#[test]
fn fire_without_callback() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    assert!(s.fire_type(ActivationEventType::OnStartup, None).is_empty());
}

// FireAsync_RaisesPluginActivatedEvent
#[test]
fn fire_raises_event() {
    let (s, _) = recording();
    let seen: Arc<Mutex<Option<PluginActivated>>> = Arc::default();
    let c = seen.clone();
    s.on_plugin_activated(Arc::new(move |e| {
        *c.lock().unwrap_or_else(PoisonError::into_inner) = Some(e.clone());
    }));
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.fire_type(ActivationEventType::OnStartup, None);
    let e = seen.lock().unwrap_or_else(PoisonError::into_inner).clone();
    assert_eq!(
        e.map(|e| (e.plugin_id, e.activation_event)),
        Some(("plugin-a".into(), ae::ON_STARTUP.into()))
    );
}

// FireAsync_CallbackThrows_ShouldNotThrow
#[test]
fn fire_callback_error_is_swallowed() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-err", &ev(ae::ON_STARTUP));
    s.set_activation_callback(Arc::new(|_| {
        Err(AppError::data(
            ErrorCode::PluginLoadFailed,
            "activation failed",
        ))
    }));
    assert!(s.fire_type(ActivationEventType::OnStartup, None).is_empty());
    // Still pending: a failed activation is not marked (C# only marks on success).
    assert!(s.is_pending_activation("plugin-err"));
}

// FireAsync_StringOverload_ShouldWork
#[test]
fn fire_string_overload() {
    let (s, log) = recording();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.fire(ae::ON_STARTUP);
    assert_eq!(got(&log), ["plugin-a"]);
}

// FireAsync_ViewEvent_ShouldActivateMatchingViewPlugin
#[test]
fn fire_view() {
    let (s, log) = recording();
    s.register_pending_plugin("view-plugin", &ev(&ae::for_view("OrderPage")));
    s.fire_type(ActivationEventType::OnView, Some("OrderPage"));
    assert_eq!(got(&log), ["view-plugin"]);
}

// FireAsync_LanguageEvent_ShouldActivateMatchingPlugin
#[test]
fn fire_language() {
    let (s, log) = recording();
    s.register_pending_plugin("lang-plugin", &ev(&ae::for_language("zh-TW")));
    s.fire_type(ActivationEventType::OnLanguage, Some("zh-TW"));
    assert_eq!(got(&log), ["lang-plugin"]);
}

// GetPluginsForEvent_ReturnsMatchingPendingPlugins
#[test]
fn plugins_for_event_matching() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.register_pending_plugin("plugin-b", &ev(&ae::for_command("test")));
    assert_eq!(
        s.plugins_for_event(ActivationEventType::OnStartup, None),
        ["plugin-a"]
    );
}

// GetPluginsForEvent_ExcludesActivatedPlugins
#[test]
fn plugins_for_event_excludes_activated() {
    let s = ActivationEventService::new();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.mark_activated("plugin-a");
    assert!(
        s.plugins_for_event(ActivationEventType::OnStartup, None)
            .is_empty()
    );
}

// FireAsync_MultiplePlugins_AllShouldActivate
#[test]
fn fire_multiple() {
    let (s, log) = recording();
    for id in ["plugin-1", "plugin-2", "plugin-3"] {
        s.register_pending_plugin(id, &ev(ae::ON_STARTUP));
    }
    s.fire_type(ActivationEventType::OnStartup, None);
    assert_eq!(got(&log), ["plugin-1", "plugin-2", "plugin-3"]);
}

// FireAsync_UnknownEventType_ShouldNotActivateAnyPlugin
#[test]
fn fire_unrelated() {
    let (s, log) = recording();
    s.register_pending_plugin("plugin-a", &ev(ae::ON_STARTUP));
    s.fire_type(ActivationEventType::OnCommand, Some("unrelated.command"));
    assert!(got(&log).is_empty());
}
