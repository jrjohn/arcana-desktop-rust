//! Security contracts (C# `Arcana.Core.Security`). Implementations arrive in milestone 3
//! (`arcana-infra`).
//!
//! The C# `[RequiresPermission]` / `[RequiresRole]` attributes are reflection metadata with no
//! consumer in the C# Core/Domain/Data layers; they are not ported here.

use std::collections::BTreeSet;
use std::time::Duration;

use async_trait::async_trait;

use crate::{AppResult, Timestamp};

/// The signed-in user as seen by the rest of the app (C# `AuthenticatedUser`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
    pub permissions: BTreeSet<String>,
}

/// Successful login / refresh (C# `AuthResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthResult {
    pub user: AuthenticatedUser,
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: Timestamp,
    pub refresh_token_expires_at: Timestamp,
    pub must_change_password: bool,
}

/// C# `AuthService`.
#[async_trait]
pub trait AuthService: Send + Sync {
    async fn authenticate(&self, username: &str, password: &str) -> AppResult<AuthResult>;
    async fn refresh_token(&self, refresh_token: &str) -> AppResult<AuthResult>;
    async fn logout(&self, user_id: i32) -> AppResult<bool>;
    async fn change_password(
        &self,
        user_id: i32,
        current_password: &str,
        new_password: &str,
    ) -> AppResult<bool>;
    async fn reset_password(&self, user_id: i32) -> AppResult<String>;
    async fn validate_token(&self, token: &str) -> AppResult<AuthenticatedUser>;
    async fn lock_account(&self, user_id: i32, duration: Option<Duration>) -> AppResult<bool>;
    async fn unlock_account(&self, user_id: i32) -> AppResult<bool>;
}

/// A generated token (C# `TokenResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenResult {
    pub token: String,
    pub expires_at: Timestamp,
}

/// Outcome of validating an access token (C# `TokenValidationResult`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenValidationResult {
    pub is_valid: bool,
    pub user_id: Option<i32>,
    pub username: Option<String>,
    pub failure_reason: Option<String>,
    pub is_expired: bool,
}

impl TokenValidationResult {
    #[must_use]
    pub fn valid(user_id: i32, username: impl Into<String>) -> Self {
        Self {
            is_valid: true,
            user_id: Some(user_id),
            username: Some(username.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn invalid(reason: impl Into<String>, is_expired: bool) -> Self {
        Self {
            is_valid: false,
            failure_reason: Some(reason.into()),
            is_expired,
            ..Self::default()
        }
    }
}

/// C# `TokenService`.
pub trait TokenService: Send + Sync {
    fn generate_access_token(&self, user: &AuthenticatedUser) -> TokenResult;
    fn generate_refresh_token(&self) -> TokenResult;
    fn validate_access_token(&self, token: &str) -> TokenValidationResult;
    fn validate_refresh_token(&self, token: &str, stored_token: &str) -> bool;
}

/// C# `PasswordHasher`.
pub trait PasswordHasher: Send + Sync {
    fn hash_password(&self, password: &str) -> String;
    fn verify_password(&self, password: &str, hashed_password: &str) -> bool;
    fn needs_rehash(&self, hashed_password: &str) -> bool;
}

/// C# `AuthorizationService`.
#[async_trait]
pub trait AuthorizationService: Send + Sync {
    async fn has_permission(&self, user_id: i32, permission: &str) -> AppResult<bool>;
    async fn has_any_permission(&self, user_id: i32, permissions: &[&str]) -> AppResult<bool>;
    async fn has_all_permissions(&self, user_id: i32, permissions: &[&str]) -> AppResult<bool>;
    async fn is_in_role(&self, user_id: i32, role: &str) -> AppResult<bool>;
    async fn is_in_any_role(&self, user_id: i32, roles: &[&str]) -> AppResult<bool>;
    async fn get_user_permissions(&self, user_id: i32) -> AppResult<BTreeSet<String>>;
    async fn get_user_roles(&self, user_id: i32) -> AppResult<Vec<String>>;
    async fn authorize(
        &self,
        user_id: i32,
        permission: &str,
        resource: Option<&str>,
    ) -> AppResult<bool>;
}

/// Payload of a current-user change (C# `CurrentUserChangedEventArgs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUserChanged {
    pub previous_user: Option<AuthenticatedUser>,
    pub new_user: Option<AuthenticatedUser>,
}

/// Handler for current-user changes (the C# `CurrentUserChanged` event).
pub type CurrentUserHandler = Box<dyn Fn(&CurrentUserChanged) + Send + Sync>;

/// C# `CurrentUserService`. The C# event becomes a subscription method.
pub trait CurrentUserService: Send + Sync {
    fn is_authenticated(&self) -> bool;
    fn current_user(&self) -> Option<AuthenticatedUser>;
    fn user_id(&self) -> Option<i32>;
    fn username(&self) -> Option<String>;
    fn set_current_user(&self, user: Option<AuthenticatedUser>);
    fn clear_current_user(&self);
    fn has_permission(&self, permission: &str) -> bool;
    fn has_any_permission(&self, permissions: &[&str]) -> bool;
    fn has_all_permissions(&self, permissions: &[&str]) -> bool;
    fn is_in_role(&self, role: &str) -> bool;
    fn is_in_any_role(&self, roles: &[&str]) -> bool;
    fn on_current_user_changed(&self, handler: CurrentUserHandler);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_validation_constructors() {
        let v = TokenValidationResult::valid(5, "bob");
        assert!(v.is_valid);
        assert_eq!((v.user_id, v.username.as_deref()), (Some(5), Some("bob")));
        assert_eq!(v.failure_reason, None);
        let i = TokenValidationResult::invalid("expired", true);
        assert!(!i.is_valid && i.is_expired);
        assert_eq!(i.failure_reason.as_deref(), Some("expired"));
        assert_eq!(i.user_id, None);
    }

    /// The contracts must stay usable as trait objects (milestone 3 injects them that way).
    #[test]
    fn contracts_are_dyn_compatible() {
        fn accept(
            _: Option<&dyn AuthService>,
            _: Option<&dyn TokenService>,
            _: Option<&dyn PasswordHasher>,
            _: Option<&dyn AuthorizationService>,
            _: Option<&dyn CurrentUserService>,
        ) -> bool {
            true
        }
        assert!(accept(None, None, None, None, None));
        let now = crate::utc_now();
        let user = AuthenticatedUser {
            id: 1,
            username: "a".into(),
            display_name: "A".into(),
            email: None,
            roles: vec!["User".into()],
            permissions: BTreeSet::from(["orders.view".to_owned()]),
        };
        let r = AuthResult {
            user: user.clone(),
            access_token: "t".into(),
            refresh_token: "r".into(),
            access_token_expires_at: now,
            refresh_token_expires_at: now,
            must_change_password: false,
        };
        let changed = CurrentUserChanged {
            previous_user: None,
            new_user: Some(r.user.clone()),
        };
        assert_eq!(changed.new_user, Some(user));
        let t = TokenResult {
            token: "x".into(),
            expires_at: now,
        };
        assert_eq!(t.token, "x");
    }
}
