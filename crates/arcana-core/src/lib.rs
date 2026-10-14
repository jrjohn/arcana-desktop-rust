//! Core building blocks shared by every layer (C# `Arcana.Core`).
//!
//! Nothing here depends on storage, UI or plugins.

pub mod entity;
pub mod error;
pub mod extensions;
mod macros;
pub mod network;
pub mod paging;
pub mod repository;
pub mod security;

pub use entity::{
    AuditInfo, AuditableEntity, ConcurrencyAware, Entity, FullEntity, SoftDeletable,
    SoftDeleteInfo, SyncInfo, Syncable, Timestamp, today, utc_now,
};
pub use error::{AppError, AppResult, ErrorCode};
pub use paging::{PageRequest, PagedResult};
pub use repository::{
    Predicate, Repository, TransactionScope, UnitOfWork, UnitOfWorkFactory, UnitOfWorkResult,
};
