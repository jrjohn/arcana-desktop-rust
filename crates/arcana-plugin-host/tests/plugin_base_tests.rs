//! C# `PluginBaseTests` (21). Mocks become the real host services; log tests and
//! `ConfigureServices` have no Rust counterpart (see bottom).

#[path = "support/host.rs"]
mod host;

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arcana_core::{AppError, AppResult};
use arcana_plugin_api::{
    ApplicationEvent, ApplicationEventBase, CommandService, Plugin, PluginMetadata, PluginState,
    PluginType, Subscription,
};
use arcana_plugin_host::plugin_base::{BasePlugin, PluginBase, PluginBehavior};
use chrono::{DateTime, Utc};
use host::{Fixture, fixture, tmp};

fn meta(id: &str, name: &str) -> PluginMetadata {
    PluginMetadata::new(id, name, semver::Version::new(1, 0, 0), PluginType::Module)
}

#[derive(Default)]
struct Simple;
impl PluginBehavior for Simple {
    fn metadata(&self) -> PluginMetadata {
        meta("test.simple", "Simple Test Plugin")
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Default)]
struct Tracking {
    on_activate: bool,
    on_deactivate: bool,
    contributions: bool,
}
impl PluginBehavior for Tracking {
    fn metadata(&self) -> PluginMetadata {
        meta("test.tracking", "Tracking Plugin")
    }
    fn on_activate(&mut self, _: &mut PluginBase) -> AppResult<()> {
        self.on_activate = true;
        Ok(())
    }
    fn on_deactivate(&mut self, _: &mut PluginBase) -> AppResult<()> {
        self.on_deactivate = true;
        Ok(())
    }
    fn register_contributions(&mut self, _: &mut PluginBase) -> AppResult<()> {
        self.contributions = true;
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct Throws {
    on_activate: bool,
}
impl PluginBehavior for Throws {
    fn metadata(&self) -> PluginMetadata {
        meta("test.throw", "Throw")
    }
    fn on_activate(&mut self, _: &mut PluginBase) -> AppResult<()> {
        if self.on_activate {
            Err(AppError::validation("Activation failed!", vec![]))
        } else {
            Ok(())
        }
    }
    fn on_deactivate(&mut self, _: &mut PluginBase) -> AppResult<()> {
        Err(AppError::validation("Deactivation failed!", vec![]))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct TestMessage;

#[derive(Default)]
struct TestEvent(ApplicationEventBase);
impl ApplicationEvent for TestEvent {
    fn timestamp(&self) -> DateTime<Utc> {
        self.0.timestamp
    }
    fn source_plugin_id(&self) -> Option<&str> {
        self.0.source_plugin_id.as_deref()
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Command,
    Subscribe,
    SubscribeEvent,
}
struct Registering(Kind);
impl PluginBehavior for Registering {
    fn metadata(&self) -> PluginMetadata {
        meta("test.reg", "Registering")
    }
    fn register_contributions(&mut self, base: &mut PluginBase) -> AppResult<()> {
        match self.0 {
            Kind::Command => base.register_command("test.command", |_| Ok(())),
            Kind::Subscribe => base.subscribe::<TestMessage>(|_| {}),
            Kind::SubscribeEvent => base.subscribe_to_event::<TestEvent>(|_| {}),
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn activate<B: PluginBehavior>(f: &Fixture, p: &mut BasePlugin<B>) -> AppResult<()> {
    let dir = tmp();
    p.activate(&f.context("test", dir.path()))
}

// InitialState_ShouldBeNotLoaded
#[test]
fn initial_state_should_be_not_loaded() {
    assert_eq!(BasePlugin::new(Simple).state(), PluginState::NotLoaded);
}

// ActivateAsync_ShouldTransitionToActiveState
#[test]
fn activate_should_transition_to_active_state() {
    let f = fixture();
    let mut p = BasePlugin::new(Simple);
    assert!(activate(&f, &mut p).is_ok());
    assert_eq!(p.state(), PluginState::Active);
}

// DeactivateAsync_ShouldTransitionToDeactivatedState
#[test]
fn deactivate_should_transition_to_deactivated_state() {
    let f = fixture();
    let mut p = BasePlugin::new(Simple);
    assert!(activate(&f, &mut p).is_ok());
    assert!(p.deactivate().is_ok());
    assert_eq!(p.state(), PluginState::Deactivated);
}

// ActivateAsync_WhenActivationThrows_ShouldTransitionToErrorState
#[test]
fn activate_when_activation_throws_should_transition_to_error_state() {
    let f = fixture();
    let mut p = BasePlugin::new(Throws { on_activate: true });
    assert!(activate(&f, &mut p).is_err());
    assert_eq!(p.state(), PluginState::Error);
}

// DeactivateAsync_WhenDeactivationThrows_ShouldTransitionToErrorState
#[test]
fn deactivate_when_deactivation_throws_should_transition_to_error_state() {
    let f = fixture();
    let mut p = BasePlugin::new(Throws { on_activate: false });
    assert!(activate(&f, &mut p).is_ok());
    assert!(p.deactivate().is_err());
    assert_eq!(p.state(), PluginState::Error);
}

// ActivateAsync_ShouldCallOnActivateAsync
#[test]
fn activate_should_call_on_activate() {
    let f = fixture();
    let mut p = BasePlugin::new(Tracking::default());
    assert!(activate(&f, &mut p).is_ok());
    assert!(p.behavior().on_activate);
}

// DeactivateAsync_ShouldCallOnDeactivateAsync
#[test]
fn deactivate_should_call_on_deactivate() {
    let f = fixture();
    let mut p = BasePlugin::new(Tracking::default());
    assert!(activate(&f, &mut p).is_ok());
    assert!(p.deactivate().is_ok());
    assert!(p.behavior().on_deactivate);
}

// ActivateAsync_ShouldCallRegisterContributions
#[test]
fn activate_should_call_register_contributions() {
    let f = fixture();
    let mut p = BasePlugin::new(Tracking::default());
    assert!(activate(&f, &mut p).is_ok());
    assert!(p.behavior().contributions);
}

fn counting(n: &Arc<AtomicUsize>) -> Subscription {
    let n = n.clone();
    Subscription::new(move || {
        n.fetch_add(1, Ordering::SeqCst);
    })
}

// DeactivateAsync_ShouldDisposeAllSubscriptions
#[test]
fn deactivate_should_dispose_all_subscriptions() {
    let f = fixture();
    let mut p = BasePlugin::new(Simple);
    assert!(activate(&f, &mut p).is_ok());
    let n = Arc::new(AtomicUsize::new(0));
    let ctx = p.base().context().cloned();
    let Some(ctx) = ctx else { panic!("no context") };
    ctx.track(counting(&n));
    ctx.track(counting(&n));
    assert!(p.deactivate().is_ok());
    assert_eq!(n.load(Ordering::SeqCst), 2);
}

// DeactivateAsync_ShouldClearSubscriptions
#[test]
fn deactivate_should_clear_subscriptions() {
    let f = fixture();
    let mut p = BasePlugin::new(Simple);
    assert!(activate(&f, &mut p).is_ok());
    let Some(ctx) = p.base().context().cloned() else {
        panic!("no context")
    };
    ctx.track(Subscription::empty());
    assert!(p.deactivate().is_ok());
    assert_eq!(ctx.subscription_count(), 0);
}

// DisposeAsync_WhenActive_ShouldDeactivate — Rust: `PluginHost::unload` is the dispose path.
#[test]
fn dispose_when_active_should_deactivate() {
    let f = fixture();
    let dir = tmp();
    let host = f.host(dir.path());
    host.register_builtin(BasePlugin::boxed(Simple));
    assert!(host.activate("test.simple").is_ok());
    // unload = C# DisposeAsync: deactivates an active plugin, then drops it.
    let states = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = states.clone();
    host.on_state_changed(Arc::new(move |e| {
        s2.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(e.new_state);
    }));
    assert!(host.unload("test.simple"));
    assert_eq!(
        *states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [PluginState::Deactivated]
    );
}

// DisposeAsync_WhenNotActive_ShouldNotCallDeactivate
#[test]
fn dispose_when_not_active_should_not_call_deactivate() {
    let f = fixture();
    let dir = tmp();
    let host = f.host(dir.path());
    host.register_builtin(BasePlugin::boxed(Tracking::default()));
    // The host only deactivates Active plugins; a never-activated plugin stays NotLoaded.
    assert!(host.unload("test.tracking"));
    assert!(host.plugin("test.tracking").is_none());
}

// RegisterCommand_ShouldCallCommandServiceAndAddToSubscriptions
#[test]
fn register_command_should_call_command_service_and_add_to_subscriptions() {
    let f = fixture();
    let mut p = BasePlugin::new(Registering(Kind::Command));
    assert!(activate(&f, &mut p).is_ok());
    assert!(f.commands.has_command("test.command"));
    assert_eq!(
        p.base()
            .context()
            .map(arcana_plugin_api::PluginContext::subscription_count),
        Some(1)
    );
    assert!(p.deactivate().is_ok());
    assert!(!f.commands.has_command("test.command"));
}

// Subscribe_ShouldAddToSubscriptions
#[test]
fn subscribe_should_add_to_subscriptions() {
    let f = fixture();
    let mut p = BasePlugin::new(Registering(Kind::Subscribe));
    assert!(activate(&f, &mut p).is_ok());
    assert_eq!(
        p.base()
            .context()
            .map(arcana_plugin_api::PluginContext::subscription_count),
        Some(1)
    );
}

// SubscribeToEvent_ShouldAddToSubscriptions
#[test]
fn subscribe_to_event_should_add_to_subscriptions() {
    let f = fixture();
    let mut p = BasePlugin::new(Registering(Kind::SubscribeEvent));
    assert!(activate(&f, &mut p).is_ok());
    assert_eq!(
        p.base()
            .context()
            .map(arcana_plugin_api::PluginContext::subscription_count),
        Some(1)
    );
}

// PublishAsync_ShouldCallMessageBus
#[test]
fn publish_should_call_message_bus() {
    use arcana_plugin_api::MessageBusExt;
    let f = fixture();
    let n = Arc::new(AtomicUsize::new(0));
    let n2 = n.clone();
    let _keep = f.bus.subscribe::<TestMessage>(move |_| {
        n2.fetch_add(1, Ordering::SeqCst);
    });
    let mut p = BasePlugin::new(Simple);
    assert!(activate(&f, &mut p).is_ok());
    assert!(p.base().publish(&TestMessage).is_ok());
    assert_eq!(n.load(Ordering::SeqCst), 1);
}

// Metadata_ShouldReturnCorrectValues
#[test]
fn metadata_should_return_correct_values() {
    let p = BasePlugin::new(Simple);
    assert_eq!(p.metadata().id, "test.simple");
    assert_eq!(p.metadata().name, "Simple Test Plugin");
    assert_eq!(p.metadata().plugin_type, PluginType::Module);
}

// Rust extra: helpers without a context fail instead of NullReferenceException.
#[test]
fn helpers_before_activation_return_error() {
    let p = BasePlugin::new(Simple);
    assert!(p.base().publish(&TestMessage).is_err());
    assert!(p.base().register_command("x", |_| Ok(())).is_err());
}

// Not ported (4):
// - ConfigureServices_ShouldNotThrow: no DI container; plugins register through the context.
// - LogInfo/LogWarning/LogError_ShouldCallLogger*: no injected ILogger; plugins use `tracing`
//   directly, which needs no plugin-level helper to verify.
