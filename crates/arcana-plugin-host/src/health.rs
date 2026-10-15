//! C# `PluginHealthMonitor`, `PluginHealthMetrics`, `PluginHealthChangedEventArgs`.
//!
//! Differences from C#: no internal timer (the C# tick only logged; the host calls
//! [`PluginHealthMonitor::check_health`] on its own schedule), and every status is persisted as a
//! `PluginHealthSnapshots` row when a [`PluginVersionStore`] is attached (C# never called
//! `RecordHealthSnapshotAsync`).

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use arcana_plugin_api::{
    HealthCheckResult, HealthState, PluginHealthCheck, PluginHealthStatus, PluginState,
};
use chrono::{DateTime, Utc};

use crate::version_store::PluginVersionStore;

const MAX_OPERATIONS: usize = 1000;
const ERROR_WINDOW: Duration = Duration::from_mins(5);
const MIB: i64 = 1024 * 1024;

/// What the monitor needs to know about a plugin (the fields of C# `PluginInfo` it reads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthTarget {
    pub id: String,
    pub name: String,
    pub state: PluginState,
    pub memory_usage_bytes: i64,
    pub error_count: i32,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl HealthTarget {
    #[must_use]
    pub fn new(id: &str, name: &str, state: PluginState) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            state,
            memory_usage_bytes: 0,
            error_count: 0,
            last_error_at: None,
            last_error: None,
        }
    }
}

/// C# `PluginHealthChangedEventArgs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHealthChanged {
    pub plugin_id: String,
    pub old_state: HealthState,
    pub new_state: HealthState,
}

pub type HealthChangedHandler = Arc<dyn Fn(&PluginHealthChanged) + Send + Sync>;

/// C# `PluginHealthMetrics`.
#[derive(Debug, Clone)]
pub struct PluginHealthMetrics {
    pub plugin_id: String,
    pub total_operations: u64,
    pub total_errors: u64,
    pub last_operation_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    recent: VecDeque<(Instant, bool)>,
}

impl PluginHealthMetrics {
    #[must_use]
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            total_operations: 0,
            total_errors: 0,
            last_operation_at: None,
            last_error_at: None,
            last_error: None,
            recent: VecDeque::new(),
        }
    }

    fn push(&mut self, success: bool) {
        self.recent.push_back((Instant::now(), success));
        while self.recent.len() > MAX_OPERATIONS {
            self.recent.pop_front();
        }
    }

    pub fn record_success(&mut self) {
        self.total_operations += 1;
        self.last_operation_at = Some(Utc::now());
        self.push(true);
    }

    pub fn record_error(&mut self, message: &str) {
        self.total_operations += 1;
        self.total_errors += 1;
        let now = Utc::now();
        self.last_operation_at = Some(now);
        self.last_error_at = Some(now);
        self.last_error = Some(message.to_string());
        self.push(false);
    }

    /// C# `GetErrorRate(window)`: failed / all operations inside the window, 0 when none.
    #[must_use]
    pub fn error_rate(&self, window: Duration) -> f64 {
        let now = Instant::now();
        let (mut total, mut failed) = (0u32, 0u32);
        for (t, ok) in &self.recent {
            if now.duration_since(*t) <= window {
                total += 1;
                failed += u32::from(!ok);
            }
        }
        if total == 0 {
            0.0
        } else {
            f64::from(failed) / f64::from(total)
        }
    }
}

#[derive(Default)]
struct Inner {
    cache: BTreeMap<String, PluginHealthStatus>,
    metrics: BTreeMap<String, PluginHealthMetrics>,
    handlers: Vec<HealthChangedHandler>,
}

/// C# `PluginHealthMonitor`.
#[derive(Default)]
pub struct PluginHealthMonitor {
    inner: Mutex<Inner>,
    store: Option<Arc<dyn PluginVersionStore>>,
}

impl std::fmt::Debug for PluginHealthMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginHealthMonitor")
            .field("persistent", &self.store.is_some())
            .finish_non_exhaustive()
    }
}

fn overall(checks: &[HealthCheckResult]) -> HealthState {
    if checks.iter().all(|c| c.state == HealthState::Healthy) {
        HealthState::Healthy
    } else if checks.iter().any(|c| c.state == HealthState::Unhealthy) {
        HealthState::Unhealthy
    } else {
        HealthState::Degraded
    }
}

fn check(name: &str, state: HealthState, message: String, duration: Duration) -> HealthCheckResult {
    HealthCheckResult {
        check_name: name.to_string(),
        state,
        message: Some(message),
        duration,
    }
}

impl PluginHealthMonitor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Persist every computed status through `store`.
    #[must_use]
    pub fn with_store(store: Arc<dyn PluginVersionStore>) -> Self {
        Self {
            inner: Mutex::default(),
            store: Some(store),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// C# `HealthChanged += handler`.
    pub fn on_health_changed(&self, handler: HealthChangedHandler) {
        self.lock().handlers.push(handler);
    }

    /// Compute the status synchronously (no persistence). Split out so the plugin's
    /// `&dyn PluginHealthCheck` never lives across an `.await`.
    #[must_use]
    pub fn evaluate(
        &self,
        target: &HealthTarget,
        plugin_check: Option<&dyn PluginHealthCheck>,
    ) -> PluginHealthStatus {
        let started = Instant::now();
        let mut checks = vec![check(
            "State",
            if target.state == PluginState::Active {
                HealthState::Healthy
            } else {
                HealthState::Degraded
            },
            format!("Plugin state: {:?}", target.state),
            Duration::ZERO,
        )];
        if let Some(hc) = plugin_check {
            let t = Instant::now();
            match hc.check_health() {
                Ok(result) => {
                    checks.push(HealthCheckResult {
                        check_name: "PluginHealthCheck".to_string(),
                        state: result.state,
                        message: result.message,
                        duration: t.elapsed(),
                    });
                    checks.extend(result.checks);
                }
                Err(e) => return self.failed(target, &e.to_string(), checks, started),
            }
        }
        let rate = self
            .lock()
            .metrics
            .entry(target.id.clone())
            .or_insert_with(|| PluginHealthMetrics::new(&target.id))
            .error_rate(ERROR_WINDOW);
        checks.push(check(
            "ErrorRate",
            if rate < 0.01 {
                HealthState::Healthy
            } else if rate < 0.1 {
                HealthState::Degraded
            } else {
                HealthState::Unhealthy
            },
            format!("Error rate: {:.2}%", rate * 100.0),
            Duration::ZERO,
        ));
        let mem = target.memory_usage_bytes;
        #[allow(clippy::cast_precision_loss, reason = "display only")]
        let mem_mb = mem as f64 / 1024.0 / 1024.0;
        checks.push(check(
            "Memory",
            if mem < 100 * MIB {
                HealthState::Healthy
            } else if mem < 500 * MIB {
                HealthState::Degraded
            } else {
                HealthState::Unhealthy
            },
            format!("Memory usage: {mem_mb:.2} MB"),
            Duration::ZERO,
        ));
        let state = overall(&checks);
        let mut status = PluginHealthStatus::new(&target.id, &target.name, state);
        status.message = Some(
            if state == HealthState::Healthy {
                "All checks passed"
            } else {
                "Some checks failed"
            }
            .to_string(),
        );
        status.response_time = Some(started.elapsed());
        status.memory_usage_bytes = mem;
        status.error_count = target.error_count;
        status.last_error_at = target.last_error_at;
        status.last_error.clone_from(&target.last_error);
        status.details = checks;
        let previous = self
            .lock()
            .cache
            .insert(target.id.clone(), status.clone())
            .map(|p| p.state);
        if previous != Some(state) {
            let event = PluginHealthChanged {
                plugin_id: target.id.clone(),
                old_state: previous.unwrap_or(HealthState::Unknown),
                new_state: state,
            };
            let handlers = self.lock().handlers.clone();
            for h in &handlers {
                h(&event);
            }
        }
        status
    }

    /// C# `catch` branch: unhealthy, error count bumped, cached without an event.
    fn failed(
        &self,
        target: &HealthTarget,
        message: &str,
        checks: Vec<HealthCheckResult>,
        started: Instant,
    ) -> PluginHealthStatus {
        tracing::error!(plugin_id = %target.id, error = message, "health check failed");
        let mut status = PluginHealthStatus::new(&target.id, &target.name, HealthState::Unhealthy);
        status.message = Some(format!("Health check failed: {message}"));
        status.response_time = Some(started.elapsed());
        status.error_count = target.error_count + 1;
        status.last_error_at = Some(Utc::now());
        status.last_error = Some(message.to_string());
        status.details = checks;
        self.lock().cache.insert(target.id.clone(), status.clone());
        status
    }

    /// Persist a status computed by [`Self::evaluate`] (logged, never fails the check).
    pub async fn persist(&self, status: &PluginHealthStatus) {
        if let Some(store) = &self.store
            && let Err(e) = store
                .record_health_snapshot(&status.plugin_id, status)
                .await
        {
            tracing::warn!(plugin_id = %status.plugin_id, error = %e, "failed to persist health snapshot");
        }
    }

    /// C# `CheckHealthAsync` for a plugin without its own health check.
    pub async fn check_health(&self, target: &HealthTarget) -> PluginHealthStatus {
        let status = self.evaluate(target, None);
        self.persist(&status).await;
        status
    }

    /// C# `GetCachedHealth`.
    #[must_use]
    pub fn cached_health(&self, plugin_id: &str) -> Option<PluginHealthStatus> {
        self.lock().cache.get(plugin_id).cloned()
    }

    /// C# `RecordError`.
    pub fn record_error(&self, plugin_id: &str, message: &str) {
        self.lock()
            .metrics
            .entry(plugin_id.to_string())
            .or_insert_with(|| PluginHealthMetrics::new(plugin_id))
            .record_error(message);
    }

    /// C# `RecordSuccess`.
    pub fn record_success(&self, plugin_id: &str) {
        self.lock()
            .metrics
            .entry(plugin_id.to_string())
            .or_insert_with(|| PluginHealthMetrics::new(plugin_id))
            .record_success();
    }

    /// C# `GetMetrics`.
    #[must_use]
    pub fn metrics(&self, plugin_id: &str) -> Option<PluginHealthMetrics> {
        self.lock().metrics.get(plugin_id).cloned()
    }

    /// Forget a plugin (uninstall).
    pub fn remove(&self, plugin_id: &str) {
        let mut inner = self.lock();
        inner.cache.remove(plugin_id);
        inner.metrics.remove(plugin_id);
    }
}
