//! Extra tests (no C# counterpart): identity entity defaults and the built-in role and
//! permission catalogues.

use std::collections::BTreeSet;

use arcana_core::{AuditableEntity, Entity, SoftDeletable, Syncable};
use arcana_domain::{
    AppPermission, AuditEventType, AuditLog, HasBase, Role, RolePermission, User, UserPermission,
    UserRole, system_permissions, system_roles,
};

#[test]
fn identity_defaults_match_csharp_initialisers() {
    let u = User::default();
    assert!(u.is_active && !u.is_locked && !u.must_change_password);
    assert_eq!(u.failed_login_attempts, 0);
    assert!(UserPermission::default().is_granted);
    assert_eq!(AuditLog::default().event_type, AuditEventType::LoginSuccess);
    assert!(!AuditLog::default().is_success);
    assert_eq!(Role::default().user_roles.len(), 0);
    assert_eq!(AppPermission::default().role_permissions.len(), 0);
    assert!(UserRole::default().user.is_none());
    assert!(RolePermission::default().permission.is_none());
}

#[test]
fn entities_expose_core_capabilities() {
    let mut r = Role::default();
    r.base_mut().id = 9;
    r.audit_mut().created_by = Some("seed".into());
    r.soft_delete_mut().is_deleted = true;
    r.sync_mut().is_pending_sync = false;
    assert_eq!(r.id(), 9);
    assert_eq!(r.base().id, 9);
    assert_eq!(r.audit().created_by.as_deref(), Some("seed"));
    assert!(r.soft_delete().is_deleted);
    assert!(!r.sync().is_pending_sync);
}

#[test]
fn catalogues_are_complete_and_unique() {
    assert_eq!(
        system_roles::ALL,
        ["Administrator", "Manager", "User", "Guest"]
    );
    let perms: BTreeSet<_> = system_permissions::ALL.into_iter().collect();
    assert_eq!(perms.len(), 33);
    assert!(perms.contains(system_permissions::PRODUCTS_MANAGE_STOCK));
    assert_eq!(system_permissions::AUDIT_LOGS_VIEW, "audit.view");
}
