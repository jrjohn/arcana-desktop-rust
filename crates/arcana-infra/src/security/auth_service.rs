//! Login, tokens, lockout and password management (C# `AuthServiceImpl`).

use std::sync::Arc;
use std::time::Duration;

use arcana_core::security::{
    AuthResult, AuthService, AuthenticatedUser, CurrentUserService, PasswordHasher, TokenService,
};
use arcana_core::{AppError, AppResult, ErrorCode, Repository, Timestamp, utc_now};
use arcana_data::Db;
use arcana_domain::{AuditEventType, User};
use async_trait::async_trait;
use chrono::TimeDelta;

use super::audit::{AuditEntry, AuditLogger};
use super::identity::IdentityStore;

/// Failed attempts before the account locks.
pub const MAX_FAILED_ATTEMPTS: i32 = 5;
/// How long an automatic lock lasts.
pub const LOCKOUT_DURATION: Duration = Duration::from_mins(15);
const TEMP_PASSWORD_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789!@#$%";

fn auth(code: ErrorCode, message: impl Into<String>) -> AppError {
    AppError::Auth {
        code,
        message: message.into(),
    }
}

fn not_found() -> AppError {
    AppError::data(ErrorCode::NotFound, "User not found")
}

fn later(d: Duration) -> Timestamp {
    let now = utc_now();
    TimeDelta::from_std(d)
        .ok()
        .and_then(|d| now.checked_add_signed(d))
        .unwrap_or(Timestamp::MAX)
}

/// C# `AuthServiceImpl`.
pub struct AuthServiceImpl {
    store: IdentityStore,
    audit: AuditLogger,
    hasher: Arc<dyn PasswordHasher>,
    tokens: Arc<dyn TokenService>,
    current: Arc<dyn CurrentUserService>,
}

impl std::fmt::Debug for AuthServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthServiceImpl")
    }
}

impl AuthServiceImpl {
    #[must_use]
    pub fn new(
        db: Db,
        hasher: Arc<dyn PasswordHasher>,
        tokens: Arc<dyn TokenService>,
        current: Arc<dyn CurrentUserService>,
    ) -> Self {
        Self {
            store: IdentityStore::new(&db),
            audit: AuditLogger::new(db),
            hasher,
            tokens,
            current,
        }
    }

    async fn log(
        &self,
        user_id: Option<i32>,
        username: &str,
        event_type: AuditEventType,
        details: Option<String>,
        is_success: bool,
    ) -> AppResult<()> {
        self.audit
            .write(AuditEntry {
                event_type,
                user_id,
                username: Some(username.to_owned()),
                details,
                is_success,
                ..AuditEntry::default()
            })
            .await
            .map(|_| ())
    }

    async fn find_user(
        &self,
        pred: &(dyn Fn(&User) -> bool + Send + Sync),
    ) -> AppResult<Option<User>> {
        self.store.users.first_or_default(pred).await
    }

    async fn require_user(&self, user_id: i32) -> AppResult<User> {
        self.store
            .users
            .get_by_id(user_id)
            .await?
            .ok_or_else(not_found)
    }

    fn actor(&self) -> String {
        self.current.username().unwrap_or_default()
    }

    /// Issue tokens, store the refresh token and set the current user.
    async fn issue(&self, mut user: User) -> AppResult<AuthResult> {
        let authenticated: AuthenticatedUser =
            self.store.graph(user.clone()).await?.authenticated_user();
        let access = self.tokens.generate_access_token(&authenticated);
        let refresh = self.tokens.generate_refresh_token();
        user.refresh_token = Some(refresh.token.clone());
        user.refresh_token_expires_at = Some(refresh.expires_at);
        self.store.users.update(&mut user).await?;
        self.current.set_current_user(Some(authenticated.clone()));
        Ok(AuthResult {
            user: authenticated,
            access_token: access.token,
            refresh_token: refresh.token,
            access_token_expires_at: access.expires_at,
            refresh_token_expires_at: refresh.expires_at,
            must_change_password: user.must_change_password,
        })
    }

    fn generate_temporary_password() -> AppResult<String> {
        let mut random = [0u8; 12];
        getrandom::getrandom(&mut random).map_err(|e| AppError::Unknown {
            message: format!("random source failed: {e}"),
            inner: None,
        })?;
        Ok(random
            .iter()
            .map(|b| char::from(TEMP_PASSWORD_CHARS[usize::from(*b) % TEMP_PASSWORD_CHARS.len()]))
            .collect())
    }
}

#[async_trait]
impl AuthService for AuthServiceImpl {
    async fn authenticate(&self, username: &str, password: &str) -> AppResult<AuthResult> {
        if username.trim().is_empty() || password.trim().is_empty() {
            return Err(AppError::validation(
                "Username and password are required",
                vec![],
            ));
        }
        let Some(mut user) = self.find_user(&|u: &User| u.username == username).await? else {
            tracing::warn!("Login failed: user {username} not found");
            self.log(
                None,
                username,
                AuditEventType::LoginFailed,
                Some("User not found".into()),
                false,
            )
            .await?;
            return Err(auth(
                ErrorCode::InvalidCredentials,
                "Invalid username or password",
            ));
        };

        if user.is_locked {
            match user.locked_until {
                Some(until) if until > utc_now() => {
                    tracing::warn!("Login failed: user {username} is locked until {until}");
                    return Err(auth(
                        ErrorCode::AccountLocked,
                        format!("Account is locked until {}", until.format("%Y-%m-%d %H:%M")),
                    ));
                }
                // Lock expired — or had no end, which C# also treats as expired.
                _ => {
                    user.is_locked = false;
                    user.locked_until = None;
                    user.failed_login_attempts = 0;
                }
            }
        }

        if !user.is_active {
            tracing::warn!("Login failed: user {username} is inactive");
            return Err(auth(ErrorCode::AuthenticationFailed, "Account is inactive"));
        }

        if !self.hasher.verify_password(password, &user.password_hash)? {
            user.failed_login_attempts += 1;
            if user.failed_login_attempts >= MAX_FAILED_ATTEMPTS {
                user.is_locked = true;
                user.locked_until = Some(later(LOCKOUT_DURATION));
                tracing::warn!(
                    "User {username} locked after {} failed attempts",
                    user.failed_login_attempts
                );
                self.log(
                    Some(user.base.id),
                    username,
                    AuditEventType::AccountLocked,
                    Some(format!(
                        "Locked after {} failed attempts",
                        user.failed_login_attempts
                    )),
                    true,
                )
                .await?;
            }
            self.store.users.update(&mut user).await?;
            self.log(
                Some(user.base.id),
                username,
                AuditEventType::LoginFailed,
                Some("Invalid password".into()),
                false,
            )
            .await?;
            return Err(auth(
                ErrorCode::InvalidCredentials,
                "Invalid username or password",
            ));
        }

        if self.hasher.needs_rehash(&user.password_hash) {
            user.password_hash = self.hasher.hash_password(password)?;
        }
        user.last_login_at = Some(utc_now());
        user.failed_login_attempts = 0;
        let id = user.base.id;
        let result = self.issue(user).await?;
        self.log(Some(id), username, AuditEventType::LoginSuccess, None, true)
            .await?;
        tracing::info!("User {username} logged in successfully");
        Ok(result)
    }

    async fn refresh_token(&self, refresh_token: &str) -> AppResult<AuthResult> {
        if refresh_token.trim().is_empty() {
            return Err(auth(
                ErrorCode::AuthenticationFailed,
                "Refresh token is required",
            ));
        }
        let Some(user) = self
            .find_user(&|u: &User| u.refresh_token.as_deref() == Some(refresh_token) && u.is_active)
            .await?
        else {
            return Err(auth(ErrorCode::TokenExpired, "Invalid refresh token"));
        };
        // C#: `RefreshTokenExpiresAt < UtcNow` — a missing expiry compares false (not expired).
        if user.refresh_token_expires_at.is_some_and(|e| e < utc_now()) {
            return Err(auth(ErrorCode::TokenExpired, "Refresh token expired"));
        }
        let (id, name) = (user.base.id, user.username.clone());
        let result = self.issue(user).await?;
        self.log(Some(id), &name, AuditEventType::TokenRefreshed, None, true)
            .await?;
        Ok(result)
    }

    async fn logout(&self, user_id: i32) -> AppResult<bool> {
        let mut user = self.require_user(user_id).await?;
        user.refresh_token = None;
        user.refresh_token_expires_at = None;
        self.store.users.update(&mut user).await?;
        self.log(
            Some(user_id),
            &user.username,
            AuditEventType::Logout,
            None,
            true,
        )
        .await?;
        if self.current.user_id() == Some(user_id) {
            self.current.clear_current_user();
        }
        tracing::info!("User {} logged out", user.username);
        Ok(true)
    }

    async fn change_password(
        &self,
        user_id: i32,
        current_password: &str,
        new_password: &str,
    ) -> AppResult<bool> {
        if current_password.trim().is_empty() || new_password.trim().is_empty() {
            return Err(AppError::validation(
                "Current and new passwords are required",
                vec![],
            ));
        }
        if new_password.chars().count() < 8 {
            return Err(AppError::validation(
                "Password must be at least 8 characters",
                vec![],
            ));
        }
        let mut user = self.require_user(user_id).await?;
        if !self
            .hasher
            .verify_password(current_password, &user.password_hash)?
        {
            self.log(
                Some(user_id),
                &user.username,
                AuditEventType::PasswordChanged,
                Some("Invalid current password".into()),
                false,
            )
            .await?;
            return Err(auth(
                ErrorCode::InvalidCredentials,
                "Current password is incorrect",
            ));
        }
        user.password_hash = self.hasher.hash_password(new_password)?;
        user.password_changed_at = Some(utc_now());
        user.must_change_password = false;
        user.refresh_token = None;
        user.refresh_token_expires_at = None;
        self.store.users.update(&mut user).await?;
        self.log(
            Some(user_id),
            &user.username,
            AuditEventType::PasswordChanged,
            None,
            true,
        )
        .await?;
        tracing::info!("Password changed for user {}", user.username);
        Ok(true)
    }

    async fn reset_password(&self, user_id: i32) -> AppResult<String> {
        let mut user = self.require_user(user_id).await?;
        let temp = Self::generate_temporary_password()?;
        user.password_hash = self.hasher.hash_password(&temp)?;
        user.must_change_password = true;
        user.password_changed_at = Some(utc_now());
        user.refresh_token = None;
        user.refresh_token_expires_at = None;
        self.store.users.update(&mut user).await?;
        self.log(
            Some(user_id),
            &user.username,
            AuditEventType::PasswordResetCompleted,
            Some(format!("Reset by {}", self.actor())),
            true,
        )
        .await?;
        tracing::info!("Password reset for user {}", user.username);
        Ok(temp)
    }

    async fn validate_token(&self, token: &str) -> AppResult<AuthenticatedUser> {
        let v = self.tokens.validate_access_token(token);
        if !v.is_valid {
            return Err(auth(
                ErrorCode::TokenExpired,
                v.failure_reason.unwrap_or_else(|| "Invalid token".into()),
            ));
        }
        let id = v.user_id;
        let user = self
            .find_user(&|u: &User| Some(u.base.id) == id && u.is_active)
            .await?
            .ok_or_else(|| {
                auth(
                    ErrorCode::AuthenticationFailed,
                    "User not found or inactive",
                )
            })?;
        Ok(self.store.graph(user).await?.authenticated_user())
    }

    async fn lock_account(&self, user_id: i32, duration: Option<Duration>) -> AppResult<bool> {
        let mut user = self.require_user(user_id).await?;
        user.is_locked = true;
        user.locked_until = duration.map(later);
        user.refresh_token = None;
        user.refresh_token_expires_at = None;
        self.store.users.update(&mut user).await?;
        self.log(
            Some(user_id),
            &user.username,
            AuditEventType::AccountLocked,
            Some(format!("Locked by {}", self.actor())),
            true,
        )
        .await?;
        tracing::info!("User {} account locked", user.username);
        Ok(true)
    }

    async fn unlock_account(&self, user_id: i32) -> AppResult<bool> {
        let mut user = self.require_user(user_id).await?;
        user.is_locked = false;
        user.locked_until = None;
        user.failed_login_attempts = 0;
        self.store.users.update(&mut user).await?;
        self.log(
            Some(user_id),
            &user.username,
            AuditEventType::AccountUnlocked,
            Some(format!("Unlocked by {}", self.actor())),
            true,
        )
        .await?;
        tracing::info!("User {} account unlocked", user.username);
        Ok(true)
    }
}
