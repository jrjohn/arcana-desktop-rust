//! `PluginHealthMonitor` — C# has no tests for it; these are Rust additions covering the
//! check composition, change events, error-rate metrics and snapshot persistence.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_plugin_api::{HealthState, PluginHealthCheck, PluginHealthCheckResult, PluginState};
use arcana_plugin_host::health::{HealthTarget, PluginHealthChanged, PluginHealthMonitor};
use arcana_plugin_host::version_store::{InMemoryPluginVersionStore, PluginVersionStore};

struct Check(AppResult<PluginHealthCheckResult>);
impl PluginHealthCheck for Check {
    fn check_health(&self) -> AppResult<PluginHealthCheckResult> {
        self.0.clone()
    }
}

fn active() -> HealthTarget {
    HealthTarget::new("p", "P", PluginState::Active)
}

#[test]
fn active_plugin_without_errors_is_healthy() {
    let m = PluginHealthMonitor::new();
    let s = m.evaluate(&active(), None);
    assert_eq!(s.state, HealthState::Healthy);
    assert_eq!(s.message.as_deref(), Some("All checks passed"));
    let names: Vec<_> = s.details.iter().map(|d| d.check_name.as_str()).collect();
    assert_eq!(names, ["State", "ErrorRate", "Memory"]);
}

#[test]
fn inactive_plugin_is_degraded() {
    let m = PluginHealthMonitor::new();
    let s = m.evaluate(&HealthTarget::new("p", "P", PluginState::Loaded), None);
    assert_eq!(s.state, HealthState::Degraded);
}

#[test]
fn plugin_check_result_and_sub_checks_are_included() {
    let m = PluginHealthMonitor::new();
    let s = m.evaluate(
        &active(),
        Some(&Check(Ok(PluginHealthCheckResult::unhealthy("db down")))),
    );
    assert_eq!(s.state, HealthState::Unhealthy);
    assert!(
        s.details
            .iter()
            .any(|d| d.check_name == "PluginHealthCheck")
    );
}

#[test]
fn failing_plugin_check_is_unhealthy_and_counts_an_error() {
    let m = PluginHealthMonitor::new();
    let err = AppError::data(ErrorCode::Unknown, "boom");
    let s = m.evaluate(&active(), Some(&Check(Err(err))));
    assert_eq!(s.state, HealthState::Unhealthy);
    assert_eq!(s.error_count, 1);
    assert_eq!(s.last_error.as_deref(), Some("boom"));
    assert_eq!(m.cached_health("p").unwrap().state, HealthState::Unhealthy);
}

#[test]
fn high_memory_is_degraded_then_unhealthy() {
    let m = PluginHealthMonitor::new();
    let mut t = active();
    t.memory_usage_bytes = 200 * 1024 * 1024;
    assert_eq!(m.evaluate(&t, None).state, HealthState::Degraded);
    t.memory_usage_bytes = 600 * 1024 * 1024;
    assert_eq!(m.evaluate(&t, None).state, HealthState::Unhealthy);
}

#[test]
fn error_rate_drives_state_and_metrics() {
    let m = PluginHealthMonitor::new();
    for _ in 0..9 {
        m.record_success("p");
    }
    m.record_error("p", "x");
    let metrics = m.metrics("p").unwrap();
    assert_eq!((metrics.total_operations, metrics.total_errors), (10, 1));
    assert_eq!(metrics.last_error.as_deref(), Some("x"));
    // 10% → not < 0.1 → Unhealthy (C# thresholds: <1% healthy, <10% degraded).
    assert_eq!(m.evaluate(&active(), None).state, HealthState::Unhealthy);
    assert!(m.metrics("unknown").is_none());
}

#[test]
fn health_changed_fires_only_on_state_change() {
    let m = PluginHealthMonitor::new();
    let events: Arc<Mutex<Vec<PluginHealthChanged>>> = Arc::default();
    let e = events.clone();
    m.on_health_changed(Arc::new(move |ev| e.lock().unwrap().push(ev.clone())));
    let _ = m.evaluate(&active(), None);
    let _ = m.evaluate(&active(), None);
    let _ = m.evaluate(&HealthTarget::new("p", "P", PluginState::Error), None);
    let ev = events.lock().unwrap();
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0].old_state, HealthState::Unknown);
    assert_eq!(ev[1].new_state, HealthState::Degraded);
}

#[tokio::test]
async fn check_health_persists_snapshots_when_a_store_is_attached() {
    let store = Arc::new(InMemoryPluginVersionStore::new());
    let m = PluginHealthMonitor::with_store(store.clone());
    let _ = m.check_health(&active()).await;
    let _ = m.check_health(&active()).await;
    assert_eq!(store.health_snapshot_count("p").await.unwrap(), 2);
    m.remove("p");
    assert!(m.cached_health("p").is_none());
}
