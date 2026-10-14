//! RBAC + per-user permissions (C# `AuthorizationServiceImpl`).

use std::collections::BTreeSet;

use arcana_core::security::AuthorizationService;
use arcana_core::{AppError, AppResult, ErrorCode, Repository};
use arcana_data::Db;
use arcana_domain::AuditEventType;
use async_trait::async_trait;

use super::audit::{AuditEntry, AuditLogger};
use super::identity::{IdentityStore, role_matches};

/// C# `AuthorizationServiceImpl`.
#[derive(Clone)]
pub struct AuthorizationServiceImpl {
    store: IdentityStore,
    audit: AuditLogger,
}

impl std::fmt::Debug for AuthorizationServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthorizationServiceImpl")
    }
}

impl AuthorizationServiceImpl {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self {
            store: IdentityStore::new(&db),
            audit: AuditLogger::new(db),
        }
    }
}

#[async_trait]
impl AuthorizationService for AuthorizationServiceImpl {
    async fn has_permission(&self, user_id: i32, permission: &str) -> AppResult<bool> {
        Ok(self
            .get_user_permissions(user_id)
            .await?
            .contains(permission))
    }

    async fn has_any_permission(&self, user_id: i32, permissions: &[&str]) -> AppResult<bool> {
        let have = self.get_user_permissions(user_id).await?;
        Ok(permissions.iter().any(|p| have.contains(*p)))
    }

    async fn has_all_permissions(&self, user_id: i32, permissions: &[&str]) -> AppResult<bool> {
        let have = self.get_user_permissions(user_id).await?;
        Ok(permissions.iter().all(|p| have.contains(*p)))
    }

    async fn is_in_role(&self, user_id: i32, role: &str) -> AppResult<bool> {
        Ok(role_matches(&self.get_user_roles(user_id).await?, role))
    }

    async fn is_in_any_role(&self, user_id: i32, roles: &[&str]) -> AppResult<bool> {
        let have = self.get_user_roles(user_id).await?;
        Ok(roles.iter().any(|r| role_matches(&have, r)))
    }

    async fn get_user_permissions(&self, user_id: i32) -> AppResult<BTreeSet<String>> {
        match self.store.users.get_by_id(user_id).await? {
            None => Ok(BTreeSet::new()),
            Some(user) => Ok(self.store.graph(user).await?.permissions(true)),
        }
    }

    async fn get_user_roles(&self, user_id: i32) -> AppResult<Vec<String>> {
        self.store.role_names(user_id).await
    }

    async fn authorize(
        &self,
        user_id: i32,
        permission: &str,
        resource: Option<&str>,
    ) -> AppResult<bool> {
        if self.has_permission(user_id, permission).await? {
            return Ok(true);
        }
        tracing::warn!(
            user_id,
            permission,
            resource,
            "Authorization denied for user {user_id} on permission {permission}"
        );
        self.audit
            .write(AuditEntry {
                event_type: AuditEventType::AccessDenied,
                user_id: Some(user_id),
                resource: resource.map(str::to_owned),
                action: Some(permission.to_owned()),
                is_success: false,
                ..AuditEntry::default()
            })
            .await?;
        Err(AppError::Auth {
            code: ErrorCode::Forbidden,
            message: format!("Access denied: {permission}"),
        })
    }
}
