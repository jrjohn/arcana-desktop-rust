//! C# `TokenServiceTests` (17) plus C#-compatibility and tick-conversion tests.

use std::collections::BTreeSet;
use std::time::Duration;

use arcana_core::security::{AuthenticatedUser, TokenService};
use arcana_core::utc_now;
use arcana_infra::security::{TokenServiceImpl, TokenServiceOptions, from_ticks, to_ticks};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chrono::TimeDelta;

const KEY: &[u8] = b"test-only-signing-key-0123456789";

fn service() -> TokenServiceImpl {
    TokenServiceImpl::new(
        KEY,
        TokenServiceOptions {
            access_token_lifetime: Duration::from_hours(1),
            refresh_token_lifetime: Duration::from_hours(7 * 24),
        },
    )
    .unwrap()
}

fn user() -> AuthenticatedUser {
    AuthenticatedUser {
        id: 42,
        username: "testuser".into(),
        display_name: "Test User".into(),
        email: Some("test@example.com".into()),
        roles: vec!["Admin".into()],
        permissions: BTreeSet::from(["read".to_owned(), "write".to_owned()]),
    }
}

fn close(a: chrono::NaiveDateTime, b: chrono::NaiveDateTime) -> bool {
    (a - b).abs() <= TimeDelta::seconds(5)
}

// GenerateAccessToken_ValidUser_ShouldReturnToken
#[test]
fn generate_access_token_valid_user_should_return_token() {
    let r = service().generate_access_token(&user());
    assert_ne!(r.token.trim().len(), 0);
    assert!(r.expires_at > utc_now());
}

// GenerateAccessToken_ShouldExpireInApproximatelyOneHour
#[test]
fn generate_access_token_should_expire_in_approximately_one_hour() {
    let before = utc_now();
    let r = service().generate_access_token(&user());
    assert!(close(r.expires_at, before + TimeDelta::hours(1)));
}

// GenerateAccessToken_TokenHasTwoParts
#[test]
fn generate_access_token_token_has_two_parts() {
    let r = service().generate_access_token(&user());
    assert_eq!(r.token.split('.').count(), 2);
    let payload = B64.decode(r.token.split('.').next().unwrap()).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(json["UserId"], 42);
    assert_eq!(json["Username"], "testuser");
    assert!(json["ExpiresAt"].as_i64().unwrap() > json["IssuedAt"].as_i64().unwrap());
}

// GenerateRefreshToken_ShouldReturnRandomToken
#[test]
fn generate_refresh_token_should_return_random_token() {
    let (a, b) = (
        service().generate_refresh_token(),
        service().generate_refresh_token(),
    );
    assert!(!a.token.is_empty() && !b.token.is_empty());
    assert_ne!(a.token, b.token);
    assert_eq!(B64.decode(&a.token).unwrap().len(), 32);
}

// GenerateRefreshToken_ShouldExpireInSevenDays
#[test]
fn generate_refresh_token_should_expire_in_seven_days() {
    let before = utc_now();
    assert!(close(
        service().generate_refresh_token().expires_at,
        before + TimeDelta::days(7)
    ));
}

// ValidateAccessToken_ValidToken_ShouldReturnValidResult
#[test]
fn validate_access_token_valid_token_should_return_valid_result() {
    let s = service();
    let v = s.validate_access_token(&s.generate_access_token(&user()).token);
    assert!(v.is_valid);
    assert_eq!(
        (v.user_id, v.username.as_deref()),
        (Some(42), Some("testuser"))
    );
}

// ValidateAccessToken_EmptyToken_ShouldReturnInvalid
#[test]
fn validate_access_token_empty_token_should_return_invalid() {
    let v = service().validate_access_token("");
    assert!(!v.is_valid);
    assert_eq!(v.failure_reason.as_deref(), Some("Token is empty"));
}

// ValidateAccessToken_WhitespaceToken_ShouldReturnInvalid
#[test]
fn validate_access_token_whitespace_token_should_return_invalid() {
    assert!(!service().validate_access_token("   ").is_valid);
}

// ValidateAccessToken_MalformedToken_ShouldReturnInvalid
#[test]
fn validate_access_token_malformed_token_should_return_invalid() {
    let v = service().validate_access_token("not.a.valid.token");
    assert!(!v.is_valid);
    assert_eq!(v.failure_reason.as_deref(), Some("Invalid token format"));
    let v = service().validate_access_token("%%%.sig");
    assert!(
        v.failure_reason
            .unwrap()
            .starts_with("Token validation failed:")
    );
}

// ValidateAccessToken_TamperedSignature_ShouldReturnInvalid
#[test]
fn validate_access_token_tampered_signature_should_return_invalid() {
    let s = service();
    let token = s.generate_access_token(&user()).token;
    let payload = token.split('.').next().unwrap();
    let v = s.validate_access_token(&format!("{payload}.INVALIDSIGNATURE"));
    assert!(!v.is_valid);
    assert_eq!(v.failure_reason.as_deref(), Some("Invalid signature"));
}

// ValidateAccessToken_TamperedPayload_ShouldReturnInvalid
#[test]
fn validate_access_token_tampered_payload_should_return_invalid() {
    let s = service();
    let token = s.generate_access_token(&user()).token;
    let sig = token.split('.').nth(1).unwrap();
    let fake =
        B64.encode(r#"{"userId":999,"username":"hacker","expiresAt":9999999999999,"issuedAt":0}"#);
    assert!(!s.validate_access_token(&format!("{fake}.{sig}")).is_valid);
}

// ValidateAccessToken_ExpiredToken_ShouldReturnInvalid
#[tokio::test]
async fn validate_access_token_expired_token_should_return_invalid() {
    let s = TokenServiceImpl::new(
        KEY,
        TokenServiceOptions {
            access_token_lifetime: Duration::from_millis(1),
            ..TokenServiceOptions::default()
        },
    )
    .unwrap();
    let token = s.generate_access_token(&user());
    tokio::time::sleep(Duration::from_millis(50)).await;
    let v = s.validate_access_token(&token.token);
    assert!(!v.is_valid && v.is_expired);
    assert_eq!(v.failure_reason.as_deref(), Some("Token expired"));
}

// ValidateRefreshToken_MatchingTokens_ShouldReturnTrue
#[test]
fn validate_refresh_token_matching_tokens_should_return_true() {
    let s = service();
    let t = s.generate_refresh_token().token;
    assert!(s.validate_refresh_token(&t, &t));
}

// ValidateRefreshToken_DifferentTokens_ShouldReturnFalse
#[test]
fn validate_refresh_token_different_tokens_should_return_false() {
    let s = service();
    assert!(!s.validate_refresh_token(
        &s.generate_refresh_token().token,
        &s.generate_refresh_token().token
    ));
}

// ValidateRefreshToken_EmptyToken_ShouldReturnFalse
#[test]
fn validate_refresh_token_empty_token_should_return_false() {
    let s = service();
    assert!(!s.validate_refresh_token("", "sometoken"));
    assert!(!s.validate_refresh_token("sometoken", ""));
}

// TokenService_DefaultOptions_ShouldWork
// Deviation: C# derives a key from machine + user name when none is configured; the key is
// always injected here, so "default" means default lifetimes. An empty key is rejected.
#[test]
fn token_service_default_options_should_work() {
    let s = TokenServiceImpl::new(KEY, TokenServiceOptions::default()).unwrap();
    assert_eq!(s.options(), TokenServiceOptions::default());
    assert_eq!(s.options().access_token_lifetime, Duration::from_hours(1));
    assert!(
        s.validate_access_token(&s.generate_access_token(&user()).token)
            .is_valid
    );
    assert!(TokenServiceImpl::new(b"", TokenServiceOptions::default()).is_err());
    assert!(!format!("{s:?}").contains("test-only"));
}

// TokenService_WithSecretKey_ShouldWork
#[test]
fn token_service_with_secret_key_should_work() {
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).unwrap();
    let s = TokenServiceImpl::from_base64_key(&B64.encode(key), TokenServiceOptions::default())
        .unwrap();
    assert!(
        s.validate_access_token(&s.generate_access_token(&user()).token)
            .is_valid
    );
    assert_eq!(
        TokenServiceImpl::from_base64_key("not base64!", TokenServiceOptions::default())
            .unwrap_err()
            .code(),
        arcana_core::ErrorCode::InvalidFormat
    );
}

// New: a token produced by the C# algorithm (payload JSON + HMAC-SHA256 over its base64,
// computed independently with Python `hmac`) validates here, and one signed with another key
// does not.
#[test]
fn accepts_token_signed_like_csharp() {
    let key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
    let token = "eyJVc2VySWQiOjQyLCJVc2VybmFtZSI6InRlc3R1c2VyIiwiRXhwaXJlc0F0IjozMTU1Mzc4OTc1OTk5OTk5OTk5LCJJc3N1ZWRBdCI6NjM4MDAwMDAwMDAwMDAwMDAwfQ==.vSHkKUQusBt498B0VJnh5A5w8FYzH5t/nwNxMwDuQHM=";
    let s = TokenServiceImpl::from_base64_key(key, TokenServiceOptions::default()).unwrap();
    let v = s.validate_access_token(token);
    assert!(v.is_valid, "{:?}", v.failure_reason);
    assert_eq!(
        (v.user_id, v.username.as_deref()),
        (Some(42), Some("testuser"))
    );
    assert!(!service().validate_access_token(token).is_valid);
}

// New: .NET tick conversion.
#[test]
fn ticks_round_trip_and_match_dotnet() {
    let unix_epoch = chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc();
    assert_eq!(to_ticks(unix_epoch), 621_355_968_000_000_000);
    let now = utc_now();
    let back = from_ticks(to_ticks(now)).unwrap();
    assert!((back - now).abs() < TimeDelta::microseconds(1));
    assert!(from_ticks(3_155_378_975_999_999_999).is_some());
    assert!(from_ticks(i64::MAX).is_none());
    assert!(from_ticks(-1).is_none());
}
