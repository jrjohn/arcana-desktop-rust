//! C# `BaseEntity<TKey>`, `BaseEntity` (int key) and `BaseGuidEntity`.

use arcana_core::{
    AuditInfo, AuditableEntity, Entity, SoftDeletable, SoftDeleteInfo, SyncInfo, Syncable,
};
use uuid::Uuid;

/// Fields every persisted domain entity carries: key, audit, soft delete, sync.
///
/// C# models this as an abstract base class; Rust entities embed it as `base`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseEntity<K = i32> {
    pub id: K,
    pub audit: AuditInfo,
    pub soft_delete: SoftDeleteInfo,
    pub sync: SyncInfo,
}

/// C# `BaseGuidEntity`: a `Guid`-keyed base whose id defaults to a fresh `Guid`.
pub type BaseGuidEntity = BaseEntity<Uuid>;

impl<K> BaseEntity<K> {
    #[must_use]
    pub fn with_id(id: K) -> Self {
        Self {
            id,
            audit: AuditInfo::default(),
            soft_delete: SoftDeleteInfo::default(),
            sync: SyncInfo::default(),
        }
    }
}

impl Default for BaseEntity<i32> {
    fn default() -> Self {
        Self::with_id(0)
    }
}

impl Default for BaseEntity<Uuid> {
    fn default() -> Self {
        Self::with_id(Uuid::new_v4())
    }
}

impl<K: Copy> Entity<K> for BaseEntity<K> {
    fn id(&self) -> K {
        self.id
    }
}

impl<K> AuditableEntity for BaseEntity<K> {
    fn audit(&self) -> &AuditInfo {
        &self.audit
    }
    fn audit_mut(&mut self) -> &mut AuditInfo {
        &mut self.audit
    }
}

impl<K> SoftDeletable for BaseEntity<K> {
    fn soft_delete(&self) -> &SoftDeleteInfo {
        &self.soft_delete
    }
    fn soft_delete_mut(&mut self) -> &mut SoftDeleteInfo {
        &mut self.soft_delete
    }
}

impl<K> Syncable for BaseEntity<K> {
    fn sync(&self) -> &SyncInfo {
        &self.sync
    }
    fn sync_mut(&mut self) -> &mut SyncInfo {
        &mut self.sync
    }
}

/// Access to the embedded [`BaseEntity`] (int key). Implemented by every domain entity.
pub trait HasBase {
    fn base(&self) -> &BaseEntity;
    fn base_mut(&mut self) -> &mut BaseEntity;
}

/// Implement [`HasBase`] plus the core capability traits by delegating to the `base` field.
macro_rules! impl_base_entity {
    ($($t:ty),+ $(,)?) => {$(
        impl $crate::entities::HasBase for $t {
            fn base(&self) -> &$crate::entities::BaseEntity {
                &self.base
            }
            fn base_mut(&mut self) -> &mut $crate::entities::BaseEntity {
                &mut self.base
            }
        }
        impl arcana_core::Entity<i32> for $t {
            fn id(&self) -> i32 {
                self.base.id
            }
        }
        impl arcana_core::AuditableEntity for $t {
            fn audit(&self) -> &arcana_core::AuditInfo {
                &self.base.audit
            }
            fn audit_mut(&mut self) -> &mut arcana_core::AuditInfo {
                &mut self.base.audit
            }
        }
        impl arcana_core::SoftDeletable for $t {
            fn soft_delete(&self) -> &arcana_core::SoftDeleteInfo {
                &self.base.soft_delete
            }
            fn soft_delete_mut(&mut self) -> &mut arcana_core::SoftDeleteInfo {
                &mut self.base.soft_delete
            }
        }
        impl arcana_core::Syncable for $t {
            fn sync(&self) -> &arcana_core::SyncInfo {
                &self.base.sync
            }
            fn sync_mut(&mut self) -> &mut arcana_core::SyncInfo {
                &mut self.base.sync
            }
        }
    )+};
}
pub(crate) use impl_base_entity;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_base_defaults() {
        let mut b = BaseEntity::<i32>::default();
        assert_eq!(b.id(), 0);
        assert!(b.sync().is_pending_sync);
        assert!(!b.soft_delete().is_deleted);
        b.audit_mut().created_by = Some("x".into());
        b.soft_delete_mut().is_deleted = true;
        b.sync_mut().is_pending_sync = false;
        assert_eq!(b.audit().created_by.as_deref(), Some("x"));
        assert!(b.soft_delete.is_deleted && !b.sync.is_pending_sync);
    }

    #[test]
    fn guid_base_gets_fresh_id() {
        let a = BaseGuidEntity::default();
        let b = BaseGuidEntity::default();
        assert!(!a.id().is_nil());
        assert_ne!(a.id, b.id);
    }
}
