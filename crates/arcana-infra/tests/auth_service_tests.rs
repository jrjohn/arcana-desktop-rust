//! C# `AuthServiceTests` (27): the real service on an in-memory `SQLite` database, with
//! scripted password hasher / token service doubles (the C# Moq mocks).

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arcana_core::security::{
    AuthService, AuthenticatedUser, CurrentUserService, PasswordHasher, TokenResult, TokenService,
    TokenValidationResult,
};
use arcana_core::{AppResult, ErrorCode, Repository, utc_now};
use arcana_data::{Db, SqliteRepository};
use arcana_domain::{AuditEventType, AuditLog, Role, User, UserRole};
use arcana_infra::security::{AuditLogger, AuthServiceImpl, CurrentUserServiceImpl};
use chrono::TimeDelta;

/// Mock<PasswordHasher>: `verify` answers per (password, hash); unset pairs → `default`.
#[derive(Default)]
struct Hasher {
    verify: Mutex<HashMap<(String, String), bool>>,
    default: Mutex<bool>,
    hash: Mutex<String>,
}

impl PasswordHasher for Hasher {
    fn hash_password(&self, _password: &str) -> AppResult<String> {
        Ok(self.hash.lock().unwrap().clone())
    }
    fn verify_password(&self, password: &str, hashed: &str) -> AppResult<bool> {
        Ok(self
            .verify
            .lock()
            .unwrap()
            .get(&(password.to_owned(), hashed.to_owned()))
            .copied()
            .unwrap_or(*self.default.lock().unwrap()))
    }
    fn needs_rehash(&self, _hashed: &str) -> bool {
        false
    }
}

/// Mock<TokenService> with `SetupDefaultTokenMocks`.
struct Tokens {
    validate: Mutex<HashMap<String, TokenValidationResult>>,
}

impl TokenService for Tokens {
    fn generate_access_token(&self, _user: &AuthenticatedUser) -> TokenResult {
        TokenResult {
            token: "access-token-123".into(),
            expires_at: utc_now() + TimeDelta::hours(1),
        }
    }
    fn generate_refresh_token(&self) -> TokenResult {
        TokenResult {
            token: "refresh-token-xyz".into(),
            expires_at: utc_now() + TimeDelta::days(7),
        }
    }
    fn validate_access_token(&self, token: &str) -> TokenValidationResult {
        self.validate
            .lock()
            .unwrap()
            .get(token)
            .cloned()
            .unwrap_or_else(|| TokenValidationResult::valid(1, "testuser"))
    }
    fn validate_refresh_token(&self, token: &str, stored: &str) -> bool {
        token == stored
    }
}

struct Ctx {
    db: Db,
    hasher: Arc<Hasher>,
    tokens: Arc<Tokens>,
    current: Arc<CurrentUserServiceImpl>,
    service: AuthServiceImpl,
}

async fn ctx() -> Ctx {
    let db = Db::in_memory().await.unwrap();
    let hasher = Arc::new(Hasher::default());
    let tokens = Arc::new(Tokens {
        validate: Mutex::new(HashMap::new()),
    });
    let current = Arc::new(CurrentUserServiceImpl::new());
    let service = AuthServiceImpl::new(db.clone(), hasher.clone(), tokens.clone(), current.clone());
    Ctx {
        db,
        hasher,
        tokens,
        current,
        service,
    }
}

impl Ctx {
    fn users(&self) -> SqliteRepository<User> {
        SqliteRepository::new(self.db.clone())
    }

    async fn create_user(&self, is_active: bool, is_locked: bool, roles: &[&str]) -> User {
        let user = self
            .users()
            .add(User {
                username: "testuser".into(),
                display_name: "Test User".into(),
                email: Some("test@example.com".into()),
                password_hash: "hashed-password".into(),
                is_active,
                is_locked,
                ..User::default()
            })
            .await
            .unwrap();
        for name in roles {
            let role = SqliteRepository::<Role>::new(self.db.clone())
                .add(Role {
                    name: (*name).into(),
                    display_name: (*name).into(),
                    ..Role::default()
                })
                .await
                .unwrap();
            SqliteRepository::<UserRole>::new(self.db.clone())
                .add(UserRole {
                    user_id: user.base.id,
                    role_id: role.base.id,
                    ..UserRole::default()
                })
                .await
                .unwrap();
        }
        user
    }

    async fn reload(&self, id: i32) -> User {
        self.users().get_by_id(id).await.unwrap().unwrap()
    }

    async fn save(&self, mut user: User) {
        self.users().update(&mut user).await.unwrap();
    }

    fn verifies(&self, password: &str, hash: &str, ok: bool) {
        self.hasher
            .verify
            .lock()
            .unwrap()
            .insert((password.into(), hash.into()), ok);
    }

    async fn audit(&self) -> Vec<AuditLog> {
        AuditLogger::new(self.db.clone()).entries().await.unwrap()
    }
}

// AuthenticateAsync_ValidCredentials_ShouldReturnSuccess
#[tokio::test]
async fn authenticate_valid_credentials_should_return_success() {
    let c = ctx().await;
    let user = c.create_user(true, false, &["Admin"]).await;
    c.verifies("password123", &user.password_hash, true);
    let r = c
        .service
        .authenticate("testuser", "password123")
        .await
        .unwrap();
    assert_eq!(r.access_token, "access-token-123");
    assert_eq!(r.refresh_token, "refresh-token-xyz");
    assert_eq!(r.user.roles, ["Admin"]);
    assert_eq!(c.current.username().as_deref(), Some("testuser"));
    let stored = c.reload(user.base.id).await;
    assert_eq!(stored.refresh_token.as_deref(), Some("refresh-token-xyz"));
    assert!(stored.last_login_at.is_some());
    assert_eq!(
        c.audit().await.last().unwrap().event_type,
        AuditEventType::LoginSuccess
    );
}

// AuthenticateAsync_UserNotFound_ShouldReturnFailure
#[tokio::test]
async fn authenticate_user_not_found_should_return_failure() {
    let c = ctx().await;
    let e = c
        .service
        .authenticate("nonexistent", "password")
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::InvalidCredentials);
    let audit = c.audit().await;
    assert_eq!(audit[0].event_type, AuditEventType::LoginFailed);
    assert_eq!(audit[0].details.as_deref(), Some("User not found"));
    assert_eq!(audit[0].action, "LoginFailed");
}

// AuthenticateAsync_WrongPassword_ShouldReturnFailure
#[tokio::test]
async fn authenticate_wrong_password_should_return_failure() {
    let c = ctx().await;
    let user = c.create_user(true, false, &[]).await;
    c.verifies("wrongpass", &user.password_hash, false);
    let e = c
        .service
        .authenticate("testuser", "wrongpass")
        .await
        .unwrap_err();
    assert_eq!(e.message(), "Invalid username or password");
}

// AuthenticateAsync_EmptyUsername_ShouldReturnFailure
#[tokio::test]
async fn authenticate_empty_username_should_return_failure() {
    let c = ctx().await;
    assert_eq!(
        c.service
            .authenticate("", "password")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ValidationFailed
    );
}

// AuthenticateAsync_EmptyPassword_ShouldReturnFailure
#[tokio::test]
async fn authenticate_empty_password_should_return_failure() {
    let c = ctx().await;
    assert!(c.service.authenticate("testuser", "").await.is_err());
}

// AuthenticateAsync_InactiveUser_ShouldReturnFailure
#[tokio::test]
async fn authenticate_inactive_user_should_return_failure() {
    let c = ctx().await;
    c.create_user(false, false, &[]).await;
    *c.hasher.default.lock().unwrap() = true;
    let e = c
        .service
        .authenticate("testuser", "password")
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::AuthenticationFailed, "Account is inactive")
    );
}

// AuthenticateAsync_LockedUser_ShouldReturnFailure
#[tokio::test]
async fn authenticate_locked_user_should_return_failure() {
    let c = ctx().await;
    let mut user = c.create_user(true, true, &[]).await;
    user.locked_until = Some(utc_now() + TimeDelta::hours(1));
    c.save(user).await;
    let e = c
        .service
        .authenticate("testuser", "password")
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::AccountLocked);
    assert!(e.message().starts_with("Account is locked until "));
}

// AuthenticateAsync_ExpiredLock_ShouldSucceedWithCorrectPassword
#[tokio::test]
async fn authenticate_expired_lock_should_succeed_with_correct_password() {
    let c = ctx().await;
    let mut user = c.create_user(true, true, &[]).await;
    user.locked_until = Some(utc_now() - TimeDelta::hours(1));
    c.save(user.clone()).await;
    c.verifies("password", &user.password_hash, true);
    c.service
        .authenticate("testuser", "password")
        .await
        .unwrap();
    assert!(!c.reload(user.base.id).await.is_locked);
}

// AuthenticateAsync_MultipleWrongPasswords_ShouldIncrementFailedAttempts
#[tokio::test]
async fn authenticate_multiple_wrong_passwords_should_increment_failed_attempts() {
    let c = ctx().await;
    let user = c.create_user(true, false, &[]).await;
    let _ = c.service.authenticate("testuser", "wrong1").await;
    let _ = c.service.authenticate("testuser", "wrong2").await;
    assert_eq!(c.reload(user.base.id).await.failed_login_attempts, 2);
}

// AuthenticateAsync_FiveWrongPasswords_ShouldLockAccount
#[tokio::test]
async fn authenticate_five_wrong_passwords_should_lock_account() {
    let c = ctx().await;
    let user = c.create_user(true, false, &[]).await;
    for _ in 0..5 {
        let _ = c.service.authenticate("testuser", "wrong").await;
    }
    let u = c.reload(user.base.id).await;
    assert!(u.is_locked);
    assert!(u.locked_until.is_some());
    assert!(
        c.audit()
            .await
            .iter()
            .any(|a| a.event_type == AuditEventType::AccountLocked)
    );
}

// LogoutAsync_ExistingUser_ShouldClearRefreshToken
#[tokio::test]
async fn logout_existing_user_should_clear_refresh_token() {
    let c = ctx().await;
    let mut user = c.create_user(true, false, &[]).await;
    user.refresh_token = Some("some-token".into());
    c.save(user.clone()).await;
    c.current.set_current_user(Some(AuthenticatedUser {
        id: user.base.id,
        username: "testuser".into(),
        display_name: String::new(),
        email: None,
        roles: vec![],
        permissions: BTreeSet::new(),
    }));
    assert!(c.service.logout(user.base.id).await.unwrap());
    assert_eq!(c.reload(user.base.id).await.refresh_token, None);
    assert!(!c.current.is_authenticated());
}

// LogoutAsync_NonExistentUser_ShouldReturnFailure
#[tokio::test]
async fn logout_non_existent_user_should_return_failure() {
    let c = ctx().await;
    assert_eq!(
        c.service.logout(9999).await.unwrap_err().code(),
        ErrorCode::NotFound
    );
}

// LockAccountAsync_ExistingUser_ShouldLockAccount
#[tokio::test]
async fn lock_account_existing_user_should_lock_account() {
    let c = ctx().await;
    let user = c.create_user(true, false, &[]).await;
    assert!(c.service.lock_account(user.base.id, None).await.unwrap());
    let u = c.reload(user.base.id).await;
    assert!(u.is_locked);
    assert_eq!(u.locked_until, None);
}

// LockAccountAsync_WithDuration_ShouldSetLockedUntil
#[tokio::test]
async fn lock_account_with_duration_should_set_locked_until() {
    let c = ctx().await;
    let user = c.create_user(true, false, &[]).await;
    c.service
        .lock_account(user.base.id, Some(Duration::from_hours(2)))
        .await
        .unwrap();
    let until = c.reload(user.base.id).await.locked_until.unwrap();
    assert!((until - (utc_now() + TimeDelta::hours(2))).abs() < TimeDelta::seconds(5));
}

// LockAccountAsync_NonExistentUser_ShouldReturnFailure
#[tokio::test]
async fn lock_account_non_existent_user_should_return_failure() {
    let c = ctx().await;
    assert!(c.service.lock_account(9999, None).await.is_err());
}

// UnlockAccountAsync_LockedUser_ShouldUnlockAccount
#[tokio::test]
async fn unlock_account_locked_user_should_unlock_account() {
    let c = ctx().await;
    let mut user = c.create_user(true, true, &[]).await;
    user.locked_until = Some(utc_now() + TimeDelta::hours(1));
    user.failed_login_attempts = 5;
    c.save(user.clone()).await;
    assert!(c.service.unlock_account(user.base.id).await.unwrap());
    let u = c.reload(user.base.id).await;
    assert!(!u.is_locked);
    assert_eq!(u.locked_until, None);
    assert_eq!(u.failed_login_attempts, 0);
    assert_eq!(
        c.audit().await.last().unwrap().details.as_deref(),
        Some("Unlocked by ")
    );
}

// UnlockAccountAsync_NonExistentUser_ShouldReturnFailure
#[tokio::test]
async fn unlock_account_non_existent_user_should_return_failure() {
    let c = ctx().await;
    assert!(c.service.unlock_account(9999).await.is_err());
}

// ChangePasswordAsync_CorrectCurrentPassword_ShouldSucceed
#[tokio::test]
async fn change_password_correct_current_password_should_succeed() {
    let c = ctx().await;
    let user = c.create_user(true, false, &[]).await;
    c.verifies("oldpass", &user.password_hash, true);
    "new-hash".clone_into(&mut c.hasher.hash.lock().unwrap());
    assert!(
        c.service
            .change_password(user.base.id, "oldpass", "newpass12")
            .await
            .unwrap()
    );
    let u = c.reload(user.base.id).await;
    assert_eq!(u.password_hash, "new-hash");
    assert!(!u.must_change_password && u.password_changed_at.is_some());
}

// ChangePasswordAsync_WrongCurrentPassword_ShouldFail
#[tokio::test]
async fn change_password_wrong_current_password_should_fail() {
    let c = ctx().await;
    let user = c.create_user(true, false, &[]).await;
    c.verifies("wrongpass", &user.password_hash, false);
    // C# passes "newpass" (7 chars), which already fails the length rule.
    assert!(
        c.service
            .change_password(user.base.id, "wrongpass", "newpass")
            .await
            .is_err()
    );
    let e = c
        .service
        .change_password(user.base.id, "wrongpass", "newpass12")
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::InvalidCredentials);
}

// ChangePasswordAsync_NonExistentUser_ShouldFail
#[tokio::test]
async fn change_password_non_existent_user_should_fail() {
    let c = ctx().await;
    assert!(
        c.service
            .change_password(9999, "oldpass", "newpass")
            .await
            .is_err()
    );
    assert_eq!(
        c.service
            .change_password(9999, "oldpass", "newpass12")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        c.service
            .change_password(9999, "", "newpass12")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ValidationFailed
    );
}

// ResetPasswordAsync_ExistingUser_ShouldSucceed
#[tokio::test]
async fn reset_password_existing_user_should_succeed() {
    let c = ctx().await;
    let user = c.create_user(true, false, &[]).await;
    "hashed-temp".clone_into(&mut c.hasher.hash.lock().unwrap());
    let temp = c.service.reset_password(user.base.id).await.unwrap();
    assert_eq!(temp.chars().count(), 12);
    let u = c.reload(user.base.id).await;
    assert_eq!(u.password_hash, "hashed-temp");
    assert!(u.must_change_password);
}

// ResetPasswordAsync_NonExistentUser_ShouldFail
#[tokio::test]
async fn reset_password_non_existent_user_should_fail() {
    let c = ctx().await;
    assert!(c.service.reset_password(9999).await.is_err());
}

// ValidateTokenAsync_ValidToken_ShouldReturnUser
#[tokio::test]
async fn validate_token_valid_token_should_return_user() {
    let c = ctx().await;
    let mut user = c.create_user(true, false, &[]).await;
    user.refresh_token = Some("some-refresh".into());
    c.save(user.clone()).await;
    c.tokens.validate.lock().unwrap().insert(
        "valid-token".into(),
        TokenValidationResult::valid(user.base.id, "testuser"),
    );
    assert_eq!(
        c.service
            .validate_token("valid-token")
            .await
            .unwrap()
            .username,
        "testuser"
    );
}

// ValidateTokenAsync_InvalidToken_ShouldReturnFailure
#[tokio::test]
async fn validate_token_invalid_token_should_return_failure() {
    let c = ctx().await;
    c.tokens.validate.lock().unwrap().insert(
        "bad-token".into(),
        TokenValidationResult::invalid("bad token", false),
    );
    let e = c.service.validate_token("bad-token").await.unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::TokenExpired, "bad token")
    );
    // A valid token for a user that does not exist.
    let e = c.service.validate_token("other").await.unwrap_err();
    assert_eq!(e.code(), ErrorCode::AuthenticationFailed);
}

// RefreshTokenAsync_ValidRefreshToken_ShouldSucceed
#[tokio::test]
async fn refresh_token_valid_refresh_token_should_succeed() {
    let c = ctx().await;
    let mut user = c.create_user(true, false, &[]).await;
    user.refresh_token = Some("stored-refresh-token".into());
    user.refresh_token_expires_at = Some(utc_now() + TimeDelta::days(7));
    c.save(user.clone()).await;
    let r = c
        .service
        .refresh_token("stored-refresh-token")
        .await
        .unwrap();
    assert_eq!(r.refresh_token, "refresh-token-xyz");
    assert_eq!(
        c.reload(user.base.id).await.refresh_token.as_deref(),
        Some("refresh-token-xyz")
    );
    assert_eq!(
        c.audit().await.last().unwrap().event_type,
        AuditEventType::TokenRefreshed
    );
}

// RefreshTokenAsync_ExpiredRefreshToken_ShouldFail
#[tokio::test]
async fn refresh_token_expired_refresh_token_should_fail() {
    let c = ctx().await;
    let mut user = c.create_user(true, false, &[]).await;
    user.refresh_token = Some("expired-token".into());
    user.refresh_token_expires_at = Some(utc_now() - TimeDelta::days(1));
    c.save(user).await;
    let e = c.service.refresh_token("expired-token").await.unwrap_err();
    assert_eq!(e.message(), "Refresh token expired");
}

// RefreshTokenAsync_NoMatchingUser_ShouldFail
#[tokio::test]
async fn refresh_token_no_matching_user_should_fail() {
    let c = ctx().await;
    let e = c.service.refresh_token("unknown-token").await.unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::TokenExpired, "Invalid refresh token")
    );
    assert_eq!(
        c.service.refresh_token(" ").await.unwrap_err().code(),
        ErrorCode::AuthenticationFailed
    );
}
