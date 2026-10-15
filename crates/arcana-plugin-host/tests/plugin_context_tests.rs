//! C# `PluginContextTests` (21 → 15). "Resolves X from the service provider" becomes "the
//! context holds the very same shared service instance".

#[path = "support/host.rs"]
mod host;

use std::path::Path;
use std::sync::Arc;

use arcana_plugin_api::{PluginPermission, Subscription};
use host::{Fixture, fixture};

fn same<T: ?Sized, U: ?Sized>(a: &Arc<T>, b: &Arc<U>) -> bool {
    std::ptr::eq(Arc::as_ptr(a).cast::<()>(), Arc::as_ptr(b).cast::<()>())
}

fn ctx(f: &Fixture, id: &str) -> arcana_plugin_api::PluginContext {
    arcana_plugin_api::PluginContext::new(
        id,
        Path::new("/plugins/test").to_path_buf(),
        Path::new("/data/test").to_path_buf(),
        PluginPermission::BASIC_PLUGIN,
        &f.services,
    )
}

// Constructor_ShouldSetPluginId
#[test]
fn constructor_should_set_plugin_id() {
    assert_eq!(ctx(&fixture(), "test.plugin").plugin_id, "test.plugin");
}

// Constructor_ShouldSetPluginPath
#[test]
fn constructor_should_set_plugin_path() {
    assert_eq!(ctx(&fixture(), "p").plugin_path, Path::new("/plugins/test"));
}

// Constructor_ShouldSetDataPath
#[test]
fn constructor_should_set_data_path() {
    assert_eq!(ctx(&fixture(), "p").data_path, Path::new("/data/test"));
}

macro_rules! resolves {
    ($name:ident, $field:ident, $fix:ident) => {
        #[test]
        fn $name() {
            let f = fixture();
            let c = ctx(&f, "p");
            assert!(same(&c.$field, &f.$fix));
        }
    };
}

// Constructor_ShouldResolveCommandService
resolves!(
    constructor_should_resolve_command_service,
    commands,
    commands
);
// Constructor_ShouldResolveMessageBus
resolves!(constructor_should_resolve_message_bus, message_bus, bus);
// Constructor_ShouldResolveEventAggregator
resolves!(constructor_should_resolve_event_aggregator, events, events);
// Constructor_ShouldResolveMenuRegistry
resolves!(constructor_should_resolve_menu_registry, menus, menus);
// Constructor_ShouldResolveViewRegistry
resolves!(constructor_should_resolve_view_registry, views, views);
// Constructor_ShouldResolveNavGraph
resolves!(constructor_should_resolve_nav_graph, nav_graph, nav);
// Constructor_ShouldResolveLocalization
resolves!(constructor_should_resolve_localization, localization, l10n);

// Constructor_ShouldResolveWindowService
#[test]
fn constructor_should_resolve_window_service() {
    let f = fixture();
    assert!(same(&ctx(&f, "p").window, &f.services.window));
}

// Constructor_ShouldResolveSharedState
#[test]
fn constructor_should_resolve_shared_state() {
    let f = fixture();
    assert!(same(&ctx(&f, "p").shared_state, &f.services.shared_state));
}

// Constructor_Subscriptions_ShouldBeEmptyInitially
#[test]
fn constructor_subscriptions_should_be_empty_initially() {
    assert_eq!(ctx(&fixture(), "p").subscription_count(), 0);
}

// Subscriptions_ShouldBeModifiable
#[test]
fn subscriptions_should_be_modifiable() {
    let c = ctx(&fixture(), "p");
    c.track(Subscription::empty());
    assert_eq!(c.subscription_count(), 1);
}

// Constructor_WithDifferentPluginIds_ShouldCreateUniqueContexts
#[test]
fn constructor_with_different_plugin_ids_should_create_unique_contexts() {
    let f = fixture();
    let a = ctx(&f, "plugin.one");
    let b = ctx(&f, "plugin.two");
    assert_eq!(a.plugin_id, "plugin.one");
    assert_eq!(b.plugin_id, "plugin.two");
    a.track(Subscription::empty());
    assert_eq!(b.subscription_count(), 0);
}

// Not ported (6):
// - Constructor_ShouldResolveLogger / Constructor_ShouldCreateLoggerWithPluginId: no ILogger
//   field; plugins log through `tracing`.
// - Constructor_ShouldResolveNavigation: `INavigationService` was folded into `NavGraph`
//   (covered by Constructor_ShouldResolveNavGraph).
// - GetService_* (2) / GetRequiredService_*: no service locator; every service is a field.
