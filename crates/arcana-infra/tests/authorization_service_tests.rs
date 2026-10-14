//! C# `AuthorizationServiceTests` (19), against an in-memory `SQLite` database.

use arcana_core::security::AuthorizationService;
use arcana_core::{ErrorCode, Repository, utc_now};
use arcana_data::{Db, SqliteRepository};
use arcana_domain::{
    AppPermission, AuditEventType, Role, RolePermission, User, UserPermission, UserRole,
};
use arcana_infra::security::{AuditLogger, AuthorizationServiceImpl};
use chrono::TimeDelta;

struct Ctx {
    db: Db,
    service: AuthorizationServiceImpl,
}

async fn ctx() -> Ctx {
    let db = Db::in_memory().await.unwrap();
    Ctx {
        service: AuthorizationServiceImpl::new(db.clone()),
        db,
    }
}

impl Ctx {
    fn repo<T: arcana_data::TableEntity>(&self) -> SqliteRepository<T> {
        SqliteRepository::new(self.db.clone())
    }

    async fn permission(&self, code: &str) -> AppPermission {
        self.repo()
            .add(AppPermission {
                code: code.into(),
                display_name: code.into(),
                category: "Test".into(),
                ..AppPermission::default()
            })
            .await
            .unwrap()
    }

    async fn grant(&self, role_id: i32, permission_id: i32) {
        self.repo::<RolePermission>()
            .add(RolePermission {
                role_id,
                permission_id,
                ..RolePermission::default()
            })
            .await
            .unwrap();
    }

    async fn direct(&self, user_id: i32, permission_id: i32, is_granted: bool) {
        self.repo::<UserPermission>()
            .add(UserPermission {
                user_id,
                permission_id,
                is_granted,
                ..UserPermission::default()
            })
            .await
            .unwrap();
    }

    /// C# `SetupUserWithRolePermissionAsync`.
    async fn setup(&self, role_name: &str, permission_code: &str) -> (User, Role, AppPermission) {
        let permission = self.permission(permission_code).await;
        let role = self
            .repo()
            .add(Role {
                name: role_name.into(),
                display_name: role_name.into(),
                ..Role::default()
            })
            .await
            .unwrap();
        self.grant(role.base.id, permission.base.id).await;
        let user = self
            .repo()
            .add(User {
                username: "testuser".into(),
                display_name: "testuser".into(),
                password_hash: "hash".into(),
                is_active: true,
                ..User::default()
            })
            .await
            .unwrap();
        self.repo::<UserRole>()
            .add(UserRole {
                user_id: user.base.id,
                role_id: role.base.id,
                ..UserRole::default()
            })
            .await
            .unwrap();
        (user, role, permission)
    }

    async fn default_setup(&self) -> (User, Role, AppPermission) {
        self.setup("Admin", "orders.view").await
    }
}

// HasPermissionAsync_UserWithPermission_ShouldReturnTrue
#[tokio::test]
async fn has_permission_user_with_permission_should_return_true() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(
        c.service
            .has_permission(u.base.id, "orders.view")
            .await
            .unwrap()
    );
}

// HasPermissionAsync_UserWithoutPermission_ShouldReturnFalse
#[tokio::test]
async fn has_permission_user_without_permission_should_return_false() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(
        !c.service
            .has_permission(u.base.id, "admin.delete")
            .await
            .unwrap()
    );
}

// HasPermissionAsync_NonExistentUser_ShouldReturnFalse
#[tokio::test]
async fn has_permission_non_existent_user_should_return_false() {
    let c = ctx().await;
    assert!(!c.service.has_permission(9999, "orders.view").await.unwrap());
}

// HasAnyPermissionAsync_UserHasOneOfRequested_ShouldReturnTrue
#[tokio::test]
async fn has_any_permission_user_has_one_of_requested_should_return_true() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(
        c.service
            .has_any_permission(u.base.id, &["orders.view", "admin.delete"])
            .await
            .unwrap()
    );
}

// HasAnyPermissionAsync_UserHasNoneOfRequested_ShouldReturnFalse
#[tokio::test]
async fn has_any_permission_user_has_none_of_requested_should_return_false() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(
        !c.service
            .has_any_permission(u.base.id, &["admin.delete", "admin.create"])
            .await
            .unwrap()
    );
}

// HasAllPermissionsAsync_UserHasAll_ShouldReturnTrue
#[tokio::test]
async fn has_all_permissions_user_has_all_should_return_true() {
    let c = ctx().await;
    let (u, role, _) = c.default_setup().await;
    let p2 = c.permission("orders.create").await;
    c.grant(role.base.id, p2.base.id).await;
    assert!(
        c.service
            .has_all_permissions(u.base.id, &["orders.view", "orders.create"])
            .await
            .unwrap()
    );
}

// HasAllPermissionsAsync_UserMissingOne_ShouldReturnFalse
#[tokio::test]
async fn has_all_permissions_user_missing_one_should_return_false() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(
        !c.service
            .has_all_permissions(u.base.id, &["orders.view", "admin.delete"])
            .await
            .unwrap()
    );
}

// IsInRoleAsync_UserInRole_ShouldReturnTrue
#[tokio::test]
async fn is_in_role_user_in_role_should_return_true() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(c.service.is_in_role(u.base.id, "Admin").await.unwrap());
    assert!(c.service.is_in_role(u.base.id, "admin").await.unwrap());
}

// IsInRoleAsync_UserNotInRole_ShouldReturnFalse
#[tokio::test]
async fn is_in_role_user_not_in_role_should_return_false() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(!c.service.is_in_role(u.base.id, "Manager").await.unwrap());
}

// IsInAnyRoleAsync_UserInAtLeastOne_ShouldReturnTrue
#[tokio::test]
async fn is_in_any_role_user_in_at_least_one_should_return_true() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(
        c.service
            .is_in_any_role(u.base.id, &["Admin", "Manager"])
            .await
            .unwrap()
    );
}

// IsInAnyRoleAsync_UserInNone_ShouldReturnFalse
#[tokio::test]
async fn is_in_any_role_user_in_none_should_return_false() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(
        !c.service
            .is_in_any_role(u.base.id, &["Manager", "Guest"])
            .await
            .unwrap()
    );
}

// GetUserPermissionsAsync_ReturnsCorrectSet
#[tokio::test]
async fn get_user_permissions_returns_correct_set() {
    let c = ctx().await;
    let (u, _, p) = c.default_setup().await;
    assert!(
        c.service
            .get_user_permissions(u.base.id)
            .await
            .unwrap()
            .contains(&p.code)
    );
}

// GetUserPermissionsAsync_NonExistentUser_ReturnsEmpty
#[tokio::test]
async fn get_user_permissions_non_existent_user_returns_empty() {
    let c = ctx().await;
    assert!(
        c.service
            .get_user_permissions(9999)
            .await
            .unwrap()
            .is_empty()
    );
}

// GetUserRolesAsync_ReturnsCorrectRoles
#[tokio::test]
async fn get_user_roles_returns_correct_roles() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert_eq!(
        c.service.get_user_roles(u.base.id).await.unwrap(),
        ["Admin"]
    );
}

// GetUserRolesAsync_NonExistentUser_ReturnsEmpty
#[tokio::test]
async fn get_user_roles_non_existent_user_returns_empty() {
    let c = ctx().await;
    assert!(c.service.get_user_roles(9999).await.unwrap().is_empty());
}

// AuthorizeAsync_UserHasPermission_ShouldReturnSuccess
#[tokio::test]
async fn authorize_user_has_permission_should_return_success() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    assert!(
        c.service
            .authorize(u.base.id, "orders.view", None)
            .await
            .unwrap()
    );
    assert!(
        AuditLogger::new(c.db.clone())
            .entries()
            .await
            .unwrap()
            .is_empty()
    );
}

// AuthorizeAsync_UserLacksPermission_ShouldReturnFailure
#[tokio::test]
async fn authorize_user_lacks_permission_should_return_failure() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    let e = c
        .service
        .authorize(u.base.id, "admin.delete", Some("resource/123"))
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::Forbidden, "Access denied: admin.delete")
    );
    let audit = AuditLogger::new(c.db.clone()).entries().await.unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].event_type, AuditEventType::AccessDenied);
    assert_eq!(audit[0].resource.as_deref(), Some("resource/123"));
    assert_eq!(audit[0].action, "admin.delete");
    assert!(!audit[0].is_success);
}

// GetUserPermissionsAsync_DirectUserPermission_Granted_IsIncluded
#[tokio::test]
async fn get_user_permissions_direct_user_permission_granted_is_included() {
    let c = ctx().await;
    let (u, _, _) = c.default_setup().await;
    let p = c.permission("special.access").await;
    c.direct(u.base.id, p.base.id, true).await;
    assert!(
        c.service
            .get_user_permissions(u.base.id)
            .await
            .unwrap()
            .contains("special.access")
    );
}

// GetUserPermissionsAsync_DirectUserPermission_Revoked_IsExcluded
#[tokio::test]
async fn get_user_permissions_direct_user_permission_revoked_is_excluded() {
    let c = ctx().await;
    let (u, _, p) = c.default_setup().await;
    c.direct(u.base.id, p.base.id, false).await;
    assert!(
        !c.service
            .get_user_permissions(u.base.id)
            .await
            .unwrap()
            .contains(&p.code)
    );
}

// New: expired role assignments and expired direct grants are ignored.
#[tokio::test]
async fn expired_assignments_are_ignored() {
    let c = ctx().await;
    let (u, role, _) = c.default_setup().await;
    let mut link = c.repo::<UserRole>().get_all().await.unwrap().remove(0);
    link.expires_at = Some(utc_now() - TimeDelta::hours(1));
    c.repo::<UserRole>().update(&mut link).await.unwrap();
    assert!(
        c.service
            .get_user_roles(u.base.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        c.service
            .get_user_permissions(u.base.id)
            .await
            .unwrap()
            .is_empty()
    );
    let p = c.permission("temp").await;
    c.repo::<UserPermission>()
        .add(UserPermission {
            user_id: u.base.id,
            permission_id: p.base.id,
            expires_at: Some(utc_now() - TimeDelta::hours(1)),
            ..UserPermission::default()
        })
        .await
        .unwrap();
    assert!(
        c.service
            .get_user_permissions(u.base.id)
            .await
            .unwrap()
            .is_empty()
    );
    let _ = role;
}
