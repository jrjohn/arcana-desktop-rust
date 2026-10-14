//! The sync service contract (C# `SyncService.cs`).
//!
//! C# declares the interface and never implements it; [`crate::QueueSyncService`] is the
//! implementation added by this port.

use std::time::Duration;

use arcana_core::{AppResult, Timestamp};
use arcana_data::SyncOperation;
use async_trait::async_trait;
use serde::Serialize;

use crate::SyncableEntity;

/// Service state (C# `SyncState`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SyncState {
    #[default]
    Idle,
    Syncing,
    Error,
    Offline,
}

/// Kind of queued change (C# `SyncOperationType`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
pub enum SyncOperationType {
    #[default]
    Create,
    Update,
    Delete,
}

impl From<SyncOperationType> for SyncOperation {
    fn from(op: SyncOperationType) -> Self {
        match op {
            SyncOperationType::Create => Self::Create,
            SyncOperationType::Update => Self::Update,
            SyncOperationType::Delete => Self::Delete,
        }
    }
}

impl From<SyncOperation> for SyncOperationType {
    fn from(op: SyncOperation) -> Self {
        match op {
            SyncOperation::Create => Self::Create,
            SyncOperation::Update => Self::Update,
            SyncOperation::Delete => Self::Delete,
        }
    }
}

/// Payload of a state change (C# `SyncStateChangedEventArgs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncStateChanged {
    pub old_state: SyncState,
    pub new_state: SyncState,
    pub message: Option<String>,
}

/// Payload of a finished sync (C# `SyncCompletedEventArgs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCompleted {
    pub success: bool,
    pub items_synced: u32,
    pub items_failed: u32,
    pub duration: Duration,
    pub error_message: Option<String>,
}

pub type SyncStateHandler = Box<dyn Fn(&SyncStateChanged) + Send + Sync>;
pub type SyncCompletedHandler = Box<dyn Fn(&SyncCompleted) + Send + Sync>;

/// C# `SyncService`. Events become subscription methods; `PendingCount` is async because it
/// reads the queue table.
///
/// C#'s generic `QueueForSyncAsync<T>(entity, op)` is not object-safe; the trait method takes
/// the already-identified change and [`SyncServiceExt::queue_entity`] provides the typed form.
#[async_trait]
pub trait SyncService: Send + Sync {
    fn state(&self) -> SyncState;
    fn last_sync_time(&self) -> Option<Timestamp>;
    async fn pending_count(&self) -> AppResult<u64>;
    async fn start(&self) -> AppResult<()>;
    async fn stop(&self) -> AppResult<()>;
    async fn sync_now(&self) -> AppResult<()>;
    async fn queue_for_sync(
        &self,
        entity_type: &str,
        entity_id: &str,
        payload: Option<String>,
        operation: SyncOperationType,
    ) -> AppResult<()>;
    fn on_state_changed(&self, handler: SyncStateHandler);
    fn on_sync_completed(&self, handler: SyncCompletedHandler);
}

/// Typed queueing on top of any [`SyncService`].
#[async_trait]
pub trait SyncServiceExt: SyncService {
    /// Queue `entity` (serialised as JSON) under `entity_type` with its sync id.
    ///
    /// # Errors
    /// `DataCorruption` when the entity cannot be serialised; otherwise as
    /// [`SyncService::queue_for_sync`].
    async fn queue_entity<T>(
        &self,
        entity_type: &str,
        entity: &T,
        operation: SyncOperationType,
    ) -> AppResult<()>
    where
        T: SyncableEntity + Serialize + Sync,
    {
        let payload = serde_json::to_string(entity).map_err(|e| {
            arcana_core::AppError::data(arcana_core::ErrorCode::DataCorruption, e.to_string())
        })?;
        self.queue_for_sync(entity_type, &entity.sync_id(), Some(payload), operation)
            .await
    }
}

impl<S: SyncService + ?Sized> SyncServiceExt for S {}
