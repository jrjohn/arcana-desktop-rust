//! Transport abstraction and an in-memory loopback implementation.
//!
//! C# has no transport, protocol or server. This port defines the smallest contract a
//! [`crate::QueueSyncService`] needs — push local changes, pull everyone else's — and does not
//! invent a wire protocol. [`LoopbackHub`] is an in-process relay used by tests (and usable for
//! demos): every replica connected to the same hub sees every other replica's changes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use arcana_core::{AppError, AppResult, ErrorCode};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::SyncOperationType;

/// One change travelling between replicas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncChange {
    pub entity_type: String,
    pub entity_id: String,
    pub operation: SyncOperationType,
    pub payload: Option<String>,
    /// Node that produced the change.
    pub origin_node: String,
}

/// Moves changes between this replica and the others.
#[async_trait]
pub trait SyncTransport: Send + Sync {
    /// Publish local changes.
    async fn push(&self, changes: &[SyncChange]) -> AppResult<()>;
    /// Changes from other nodes not yet seen by `node_id`.
    async fn pull(&self, node_id: &str) -> AppResult<Vec<SyncChange>>;
}

/// Applies a pulled change to local state (merge/conflict resolution lives here).
#[async_trait]
pub trait ChangeApplier: Send + Sync {
    async fn apply(&self, change: &SyncChange) -> AppResult<()>;
}

#[derive(Debug)]
struct HubState {
    log: Vec<SyncChange>,
    cursors: HashMap<String, usize>,
    online: bool,
}

/// In-memory relay shared by loopback transports.
#[derive(Debug, Clone)]
pub struct LoopbackHub {
    state: Arc<Mutex<HubState>>,
}

impl Default for LoopbackHub {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(HubState {
                log: Vec::new(),
                cursors: HashMap::new(),
                online: true,
            })),
        }
    }
}

impl LoopbackHub {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A transport attached to this hub.
    #[must_use]
    pub fn transport(&self) -> LoopbackTransport {
        LoopbackTransport { hub: self.clone() }
    }

    /// Simulate losing / regaining connectivity: while offline every call fails with
    /// `NetworkUnavailable`.
    ///
    /// # Errors
    /// `Unknown` if the hub lock is poisoned.
    pub fn set_online(&self, online: bool) -> AppResult<()> {
        self.lock()?.online = online;
        Ok(())
    }

    /// Every change ever pushed, in order.
    ///
    /// # Errors
    /// `Unknown` if the hub lock is poisoned.
    pub fn log(&self) -> AppResult<Vec<SyncChange>> {
        Ok(self.lock()?.log.clone())
    }

    fn lock(&self) -> AppResult<MutexGuard<'_, HubState>> {
        self.state.lock().map_err(|e| AppError::Unknown {
            message: e.to_string(),
            inner: None,
        })
    }

    fn online(&self) -> AppResult<MutexGuard<'_, HubState>> {
        let guard = self.lock()?;
        if guard.online {
            Ok(guard)
        } else {
            Err(AppError::Network {
                code: ErrorCode::NetworkUnavailable,
                message: "loopback hub is offline".into(),
                inner: None,
            })
        }
    }
}

/// [`SyncTransport`] over a [`LoopbackHub`].
#[derive(Debug, Clone)]
pub struct LoopbackTransport {
    hub: LoopbackHub,
}

#[async_trait]
impl SyncTransport for LoopbackTransport {
    async fn push(&self, changes: &[SyncChange]) -> AppResult<()> {
        self.hub.online()?.log.extend_from_slice(changes);
        Ok(())
    }

    async fn pull(&self, node_id: &str) -> AppResult<Vec<SyncChange>> {
        let mut state = self.hub.online()?;
        let len = state.log.len();
        let from = state.cursors.insert(node_id.to_owned(), len).unwrap_or(0);
        Ok(state.log[from..]
            .iter()
            .filter(|c| c.origin_node != node_id)
            .cloned()
            .collect())
    }
}
