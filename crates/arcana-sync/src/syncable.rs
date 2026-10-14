//! Syncable entities and sync bookkeeping records (C# `SyncableEntity.cs`).

use arcana_core::{AppResult, Timestamp, utc_now};

use crate::{SyncOperationType, VectorClock};

/// An entity that carries CRDT sync metadata (C# `SyncableEntity`).
///
/// The C# abstract class `SyncableEntityBase` supplies storage plus `GetVectorClock`,
/// `SetVectorClock` and `IncrementClock`; here those are provided trait methods over the
/// accessors, and [`SyncMeta`] is a ready-made storage struct to embed.
pub trait SyncableEntity {
    fn sync_id(&self) -> String;
    fn meta(&self) -> &SyncMeta;
    fn meta_mut(&mut self) -> &mut SyncMeta;

    /// The stored clock; empty when none is stored (C# `GetVectorClock`).
    ///
    /// # Errors
    /// `DataCorruption` when the stored JSON is invalid.
    fn vector_clock(&self) -> AppResult<VectorClock> {
        match self.meta().vector_clock_json.as_deref() {
            None | Some("") => Ok(VectorClock::new()),
            Some(json) => VectorClock::deserialize(json),
        }
    }

    /// Store `clock` (C# `SetVectorClock`).
    fn set_vector_clock(&mut self, clock: &VectorClock) {
        self.meta_mut().vector_clock_json = Some(clock.serialize());
    }

    /// Bump this node's entry and stamp the modification (C# `IncrementClock`).
    ///
    /// # Errors
    /// `DataCorruption` when the stored clock JSON is invalid.
    fn increment_clock(&mut self, node_id: &str) -> AppResult<()> {
        let clock = self.vector_clock()?.increment(node_id);
        self.set_vector_clock(&clock);
        let meta = self.meta_mut();
        meta.modified_by_node_id = Some(node_id.to_owned());
        meta.modified_at = utc_now();
        Ok(())
    }
}

/// Storage for [`SyncableEntity`] (the fields of C# `SyncableEntityBase`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncMeta {
    pub vector_clock_json: Option<String>,
    pub modified_at: Timestamp,
    pub modified_by_node_id: Option<String>,
    pub has_conflict: bool,
    pub conflicting_version_json: Option<String>,
}

impl Default for SyncMeta {
    fn default() -> Self {
        Self {
            vector_clock_json: None,
            modified_at: utc_now(),
            modified_by_node_id: None,
            has_conflict: false,
            conflicting_version_json: None,
        }
    }
}

/// Per-entity sync tracking (C# `SyncMetadata`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncMetadata {
    pub entity_type: String,
    pub entity_id: String,
    pub vector_clock_json: String,
    pub modified_at: Timestamp,
    pub modified_by_node_id: String,
    pub operation: SyncOperationType,
    pub is_synced: bool,
    pub synced_at: Option<Timestamp>,
    pub retry_count: i32,
    pub last_error: Option<String>,
}

/// A conflict kept for manual resolution (C# `SyncConflictRecord`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncConflictRecord {
    pub id: i32,
    pub entity_type: String,
    pub entity_id: String,
    pub local_version_json: String,
    pub remote_version_json: String,
    pub local_clock_json: String,
    pub remote_clock_json: String,
    pub detected_at: Timestamp,
    pub is_resolved: bool,
    pub resolved_at: Option<Timestamp>,
    pub resolved_by_node_id: Option<String>,
    pub resolution_strategy: Option<String>,
}

impl SyncConflictRecord {
    /// A new unresolved record detected now (C# object initializer defaults).
    #[must_use]
    pub fn new(
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        local: (String, &VectorClock),
        remote: (String, &VectorClock),
    ) -> Self {
        Self {
            id: 0,
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            local_version_json: local.0,
            remote_version_json: remote.0,
            local_clock_json: local.1.serialize(),
            remote_clock_json: remote.1.serialize(),
            detected_at: utc_now(),
            is_resolved: false,
            resolved_at: None,
            resolved_by_node_id: None,
            resolution_strategy: None,
        }
    }

    /// Mark resolved by `node_id` using `strategy`.
    pub fn resolve(&mut self, node_id: &str, strategy: &str) {
        self.is_resolved = true;
        self.resolved_at = Some(utc_now());
        self.resolved_by_node_id = Some(node_id.to_owned());
        self.resolution_strategy = Some(strategy.to_owned());
    }
}
