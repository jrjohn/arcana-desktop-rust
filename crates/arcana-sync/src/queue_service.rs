//! [`SyncService`] backed by the `SyncQueue` table and a [`SyncTransport`].
//!
//! One sync round: push every pending (or failed, under the retry limit) queue row, mark the
//! rows completed — or failed with the error and a bumped retry count — then pull the other
//! nodes' changes and hand each to the [`ChangeApplier`].

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use arcana_core::{AppError, AppResult, Repository, Timestamp, utc_now};
use arcana_data::{Db, SqliteRepository, SyncQueueItem, SyncStatus};
use async_trait::async_trait;
use tokio::task::JoinHandle;

use crate::{
    ChangeApplier, SyncChange, SyncCompleted, SyncCompletedHandler, SyncOperationType, SyncService,
    SyncState, SyncStateChanged, SyncStateHandler, SyncTransport,
};

/// Tuning for [`QueueSyncService`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOptions {
    /// This replica's node id.
    pub node_id: String,
    /// Background interval used by [`SyncService::start`].
    pub interval: Duration,
    /// Rows that failed this many times are no longer retried.
    pub max_retries: i32,
}

impl SyncOptions {
    /// Defaults: 5 minutes (C# `AppSettings.SyncFrequencyMinutes`), 5 retries.
    #[must_use]
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            interval: Duration::from_mins(5),
            max_retries: 5,
        }
    }
}

struct Inner {
    options: SyncOptions,
    queue: SqliteRepository<SyncQueueItem>,
    transport: Arc<dyn SyncTransport>,
    applier: Arc<dyn ChangeApplier>,
    state: Mutex<SyncState>,
    last_sync: Mutex<Option<Timestamp>>,
    state_handlers: Mutex<Vec<SyncStateHandler>>,
    completed_handlers: Mutex<Vec<SyncCompletedHandler>>,
    task: Mutex<Option<JoinHandle<()>>>,
    round: tokio::sync::Mutex<()>,
}

/// Queue-table + transport implementation of [`SyncService`]. Cloning shares the service.
#[derive(Clone)]
pub struct QueueSyncService {
    inner: Arc<Inner>,
}

fn lock<T>(m: &Mutex<T>) -> AppResult<MutexGuard<'_, T>> {
    m.lock().map_err(|e| AppError::Unknown {
        message: e.to_string(),
        inner: None,
    })
}

fn is_network(e: &AppError) -> bool {
    matches!(e, AppError::Network { .. })
}

impl QueueSyncService {
    #[must_use]
    pub fn new(
        db: Db,
        transport: Arc<dyn SyncTransport>,
        applier: Arc<dyn ChangeApplier>,
        options: SyncOptions,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                options,
                queue: SqliteRepository::new(db),
                transport,
                applier,
                state: Mutex::new(SyncState::Idle),
                last_sync: Mutex::new(None),
                state_handlers: Mutex::new(Vec::new()),
                completed_handlers: Mutex::new(Vec::new()),
                task: Mutex::new(None),
                round: tokio::sync::Mutex::new(()),
            }),
        }
    }

    #[must_use]
    pub fn options(&self) -> &SyncOptions {
        &self.inner.options
    }

    /// Whether the background loop is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        lock(&self.inner.task).is_ok_and(|t| t.as_ref().is_some_and(|h| !h.is_finished()))
    }

    fn set_state(&self, new_state: SyncState, message: Option<String>) -> AppResult<()> {
        let old_state = std::mem::replace(&mut *lock(&self.inner.state)?, new_state);
        if old_state != new_state {
            let e = SyncStateChanged {
                old_state,
                new_state,
                message,
            };
            for h in lock(&self.inner.state_handlers)?.iter() {
                h(&e);
            }
        }
        Ok(())
    }

    fn complete(&self, e: &SyncCompleted) -> AppResult<()> {
        for h in lock(&self.inner.completed_handlers)?.iter() {
            h(e);
        }
        Ok(())
    }

    async fn due_items(&self) -> AppResult<Vec<SyncQueueItem>> {
        let max = self.inner.options.max_retries;
        let mut items = self
            .inner
            .queue
            .find(&|i: &SyncQueueItem| {
                i.status == SyncStatus::Pending
                    || (i.status == SyncStatus::Failed && i.retry_count < max)
            })
            .await?;
        items.sort_by_key(|i| i.id);
        Ok(items)
    }

    /// A failed round: record it, move to `Offline`/`Error`, notify, return the error.
    fn fail_round(&self, e: AppError, started: Instant, synced: u32, failed: u32) -> AppError {
        let state = if is_network(&e) {
            SyncState::Offline
        } else {
            SyncState::Error
        };
        let _ = self.set_state(state, Some(e.message().to_owned()));
        let _ = self.complete(&SyncCompleted {
            success: false,
            items_synced: synced,
            items_failed: failed,
            duration: started.elapsed(),
            error_message: Some(e.message().to_owned()),
        });
        e
    }

    async fn round(&self) -> AppResult<()> {
        let started = Instant::now();
        let node = self.inner.options.node_id.clone();
        let mut items = self.due_items().await?;
        let changes: Vec<SyncChange> = items
            .iter()
            .map(|i| SyncChange {
                entity_type: i.entity_type.clone(),
                entity_id: i.entity_id.clone(),
                operation: i.operation.into(),
                payload: i.payload.clone(),
                origin_node: node.clone(),
            })
            .collect();
        let pushed = u32::try_from(items.len()).unwrap_or(u32::MAX);

        if !changes.is_empty() {
            let outcome = self.inner.transport.push(&changes).await;
            for item in &mut items {
                if let Err(e) = &outcome {
                    item.status = SyncStatus::Failed;
                    item.retry_count += 1;
                    item.last_error = Some(e.message().to_owned());
                } else {
                    item.status = SyncStatus::Completed;
                    item.last_error = None;
                }
                self.inner.queue.update(item).await?;
            }
            if let Err(e) = outcome {
                return Err(self.fail_round(e, started, 0, pushed));
            }
        }

        let pulled = match self.inner.transport.pull(&node).await {
            Ok(p) => p,
            Err(e) => return Err(self.fail_round(e, started, pushed, 0)),
        };
        let mut applied = 0u32;
        let mut first_error = None;
        let mut failed = 0u32;
        for change in &pulled {
            match self.inner.applier.apply(change).await {
                Ok(()) => applied += 1,
                Err(e) => {
                    failed += 1;
                    first_error.get_or_insert(e);
                }
            }
        }
        *lock(&self.inner.last_sync)? = Some(utc_now());
        if let Some(e) = first_error {
            return Err(self.fail_round(e, started, pushed + applied, failed));
        }
        self.set_state(SyncState::Idle, None)?;
        self.complete(&SyncCompleted {
            success: true,
            items_synced: pushed + applied,
            items_failed: 0,
            duration: started.elapsed(),
            error_message: None,
        })
    }
}

#[async_trait]
impl SyncService for QueueSyncService {
    fn state(&self) -> SyncState {
        lock(&self.inner.state).map_or(SyncState::Error, |s| *s)
    }

    fn last_sync_time(&self) -> Option<Timestamp> {
        lock(&self.inner.last_sync).ok().and_then(|t| *t)
    }

    async fn pending_count(&self) -> AppResult<u64> {
        Ok(self.due_items().await?.len() as u64)
    }

    async fn start(&self) -> AppResult<()> {
        let mut task = lock(&self.inner.task)?;
        if task.as_ref().is_some_and(|h| !h.is_finished()) {
            return Ok(());
        }
        // The loop holds a weak handle so dropping the last service stops it.
        let weak = Arc::downgrade(&self.inner);
        let interval = self.inner.options.interval;
        *task = Some(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let Some(inner) = weak.upgrade() else { break };
                // Failures are already reported through state + SyncCompleted.
                let _ = QueueSyncService { inner }.sync_now().await;
            }
        }));
        Ok(())
    }

    async fn stop(&self) -> AppResult<()> {
        if let Some(h) = lock(&self.inner.task)?.take() {
            h.abort();
        }
        Ok(())
    }

    async fn sync_now(&self) -> AppResult<()> {
        let _round = self.inner.round.lock().await;
        self.set_state(SyncState::Syncing, None)?;
        self.round().await
    }

    async fn queue_for_sync(
        &self,
        entity_type: &str,
        entity_id: &str,
        payload: Option<String>,
        operation: SyncOperationType,
    ) -> AppResult<()> {
        if entity_type.trim().is_empty() || entity_id.trim().is_empty() {
            return Err(AppError::validation(
                "Entity type and id are required",
                vec![],
            ));
        }
        self.inner
            .queue
            .add(SyncQueueItem {
                entity_type: entity_type.to_owned(),
                entity_id: entity_id.to_owned(),
                operation: operation.into(),
                payload,
                ..SyncQueueItem::default()
            })
            .await
            .map(|_| ())
    }

    fn on_state_changed(&self, handler: SyncStateHandler) {
        if let Ok(mut hs) = lock(&self.inner.state_handlers) {
            hs.push(handler);
        }
    }

    fn on_sync_completed(&self, handler: SyncCompletedHandler) {
        if let Ok(mut hs) = lock(&self.inner.completed_handlers) {
            hs.push(handler);
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(mut t) = self.task.lock()
            && let Some(h) = t.take()
        {
            h.abort();
        }
    }
}
