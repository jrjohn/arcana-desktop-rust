//! Entity capability traits (C# `Arcana.Core.Common.Entity`).
//!
//! C# exposes each capability as get/set properties. Here every capability is a small value
//! struct reached through `&` / `&mut` accessors, so code that only cares about, say, soft
//! deletion can work on any entity generically.

use chrono::NaiveDateTime;
use uuid::Uuid;

/// Timestamps are stored as UTC wall-clock time without an offset, like C# `DateTime.UtcNow`.
pub type Timestamp = NaiveDateTime;

/// Current UTC time (C# `DateTime.UtcNow`).
#[must_use]
pub fn utc_now() -> Timestamp {
    chrono::Utc::now().naive_utc()
}

/// Local midnight of the current day (C# `DateTime.Today`).
#[must_use]
pub fn today() -> Timestamp {
    chrono::Local::now()
        .date_naive()
        .and_time(chrono::NaiveTime::MIN)
}

/// An entity with an identifier (C# `Entity<TKey>`).
pub trait Entity<K> {
    fn id(&self) -> K;
}

/// Who created / modified a row and when (C# `AuditableEntity` properties).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditInfo {
    pub created_at: Timestamp,
    pub created_by: Option<String>,
    pub modified_at: Option<Timestamp>,
    pub modified_by: Option<String>,
}

impl Default for AuditInfo {
    /// `CreatedAt = DateTime.UtcNow`, everything else empty (C# `BaseEntity` initialisers).
    fn default() -> Self {
        Self {
            created_at: utc_now(),
            created_by: None,
            modified_at: None,
            modified_by: None,
        }
    }
}

/// C# `AuditableEntity`.
pub trait AuditableEntity {
    fn audit(&self) -> &AuditInfo;
    fn audit_mut(&mut self) -> &mut AuditInfo;
}

/// Soft-delete marker (C# `SoftDeletable` properties).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoftDeleteInfo {
    pub is_deleted: bool,
    pub deleted_at: Option<Timestamp>,
    pub deleted_by: Option<String>,
}

/// C# `SoftDeletable`.
pub trait SoftDeletable {
    fn soft_delete(&self) -> &SoftDeleteInfo;
    fn soft_delete_mut(&mut self) -> &mut SoftDeleteInfo;
}

/// Optimistic-concurrency token (C# `ConcurrencyAware`).
pub trait ConcurrencyAware {
    fn row_version(&self) -> &[u8];
    fn set_row_version(&mut self, version: Vec<u8>);
}

/// Offline-sync bookkeeping (C# `Syncable` properties).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncInfo {
    pub sync_id: Uuid,
    pub last_sync_at: Option<Timestamp>,
    pub is_pending_sync: bool,
}

impl Default for SyncInfo {
    /// `SyncId = Guid.NewGuid()`, `IsPendingSync = true` (C# `BaseEntity` initialisers).
    fn default() -> Self {
        Self {
            sync_id: Uuid::new_v4(),
            last_sync_at: None,
            is_pending_sync: true,
        }
    }
}

/// C# `Syncable`.
pub trait Syncable {
    fn sync(&self) -> &SyncInfo;
    fn sync_mut(&mut self) -> &mut SyncInfo;
}

/// Every capability at once (C# `FullEntity<TKey>`).
pub trait FullEntity<K>:
    Entity<K> + AuditableEntity + SoftDeletable + ConcurrencyAware + Syncable
{
}

impl<K, T> FullEntity<K> for T where
    T: Entity<K> + AuditableEntity + SoftDeletable + ConcurrencyAware + Syncable
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Doc {
        audit: AuditInfo,
        deleted: SoftDeleteInfo,
        sync: SyncInfo,
        version: Vec<u8>,
    }
    impl Entity<u8> for Doc {
        fn id(&self) -> u8 {
            7
        }
    }
    impl AuditableEntity for Doc {
        fn audit(&self) -> &AuditInfo {
            &self.audit
        }
        fn audit_mut(&mut self) -> &mut AuditInfo {
            &mut self.audit
        }
    }
    impl SoftDeletable for Doc {
        fn soft_delete(&self) -> &SoftDeleteInfo {
            &self.deleted
        }
        fn soft_delete_mut(&mut self) -> &mut SoftDeleteInfo {
            &mut self.deleted
        }
    }
    impl ConcurrencyAware for Doc {
        fn row_version(&self) -> &[u8] {
            &self.version
        }
        fn set_row_version(&mut self, version: Vec<u8>) {
            self.version = version;
        }
    }
    impl Syncable for Doc {
        fn sync(&self) -> &SyncInfo {
            &self.sync
        }
        fn sync_mut(&mut self) -> &mut SyncInfo {
            &mut self.sync
        }
    }

    fn touch<E: FullEntity<u8>>(e: &mut E) -> u8 {
        e.audit_mut().modified_by = Some("me".into());
        e.soft_delete_mut().is_deleted = true;
        e.sync_mut().is_pending_sync = false;
        e.set_row_version(vec![1, 2]);
        e.id()
    }

    #[test]
    fn defaults_match_base_entity_initialisers() {
        let before = utc_now();
        let d = Doc::default();
        assert!(d.audit().created_at >= before);
        assert!(d.sync().is_pending_sync);
        assert!(!d.sync().sync_id.is_nil());
        assert!(!d.soft_delete().is_deleted);
        assert_ne!(SyncInfo::default().sync_id, SyncInfo::default().sync_id);
        assert_eq!(today().time(), chrono::NaiveTime::MIN);
    }

    #[test]
    fn full_entity_is_blanket_implemented() {
        let mut d = Doc::default();
        assert_eq!(touch(&mut d), 7);
        assert_eq!(d.audit().modified_by.as_deref(), Some("me"));
        assert!(d.soft_delete().is_deleted);
        assert!(!d.sync().is_pending_sync);
        assert_eq!(d.row_version(), &[1, 2]);
    }
}
