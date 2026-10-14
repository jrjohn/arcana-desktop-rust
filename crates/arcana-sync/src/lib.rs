//! CRDTs, conflict resolution and the sync service (C# `Arcana.Sync`).
//!
//! C# ships only the CRDT data structures and an unimplemented `SyncService` interface.
//! This crate adds [`QueueSyncService`] (queue table + [`SyncTransport`]) and an in-memory
//! [`LoopbackHub`] transport; no network protocol is defined.

mod conflict;
mod lww;
mod mv_register;
mod queue_service;
mod service;
mod syncable;
mod transport;
mod vector_clock;

pub use conflict::{
    ConflictResolutionResult, ConflictResolutionStrategy, ConflictResolver, FieldDescriptor,
    FieldPolicy, Mergeable, SyncConflict,
};
pub use lww::{LwwMap, LwwRegister};
pub use mv_register::MvRegister;
pub use queue_service::{QueueSyncService, SyncOptions};
pub use service::{
    SyncCompleted, SyncCompletedHandler, SyncOperationType, SyncService, SyncServiceExt, SyncState,
    SyncStateChanged, SyncStateHandler,
};
pub use syncable::{SyncConflictRecord, SyncMeta, SyncMetadata, SyncableEntity};
pub use transport::{ChangeApplier, LoopbackHub, LoopbackTransport, SyncChange, SyncTransport};
pub use vector_clock::{CausalRelation, VectorClock};
