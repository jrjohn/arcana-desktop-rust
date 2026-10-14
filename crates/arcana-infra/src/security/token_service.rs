//! HMAC-SHA256 signed access tokens (C# `TokenServiceImpl`).
//!
//! Token = `base64(payloadJson) + "." + base64(HMACSHA256(key, base64(payloadJson)))`, the
//! payload being `{"UserId":..,"Username":..,"ExpiresAt":ticks,"IssuedAt":ticks}` with .NET
//! ticks (100 ns since 0001-01-01 UTC). Tokens are interchangeable with the C# app given the
//! same key.
//!
//! Deviation: C# falls back to a key derived from machine + user name when none is
//! configured. That key is guessable, so here the key is always injected.

use std::time::Duration;

use arcana_core::security::{AuthenticatedUser, TokenResult, TokenService, TokenValidationResult};
use arcana_core::{AppError, AppResult, ErrorCode, Timestamp, utc_now};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chrono::{NaiveDate, TimeDelta};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// Lifetimes (C# `TokenServiceOptions` minus the key, which is passed separately).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenServiceOptions {
    pub access_token_lifetime: Duration,
    pub refresh_token_lifetime: Duration,
}

impl Default for TokenServiceOptions {
    /// 1 hour / 7 days, as in C#.
    fn default() -> Self {
        Self {
            access_token_lifetime: Duration::from_hours(1),
            refresh_token_lifetime: Duration::from_hours(7 * 24),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TokenPayload {
    user_id: i32,
    username: String,
    expires_at: i64,
    issued_at: i64,
}

/// 0001-01-01T00:00:00, the .NET tick origin.
fn tick_epoch() -> Timestamp {
    // A valid literal date; the fallback is unreachable.
    NaiveDate::from_ymd_opt(1, 1, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .unwrap_or_default()
}

/// .NET `DateTime.Ticks` of a UTC timestamp.
#[must_use]
pub fn to_ticks(t: Timestamp) -> i64 {
    let d = t - tick_epoch();
    d.num_seconds() * 10_000_000 + i64::from(d.subsec_nanos() / 100)
}

/// .NET `DateTime.MaxValue.Ticks`.
pub const MAX_TICKS: i64 = 3_155_378_975_999_999_999;

/// Inverse of [`to_ticks`]; `None` outside the .NET `DateTime` range (where C# throws).
#[must_use]
pub fn from_ticks(ticks: i64) -> Option<Timestamp> {
    if !(0..=MAX_TICKS).contains(&ticks) {
        return None;
    }
    let secs = ticks.div_euclid(10_000_000);
    let nanos = u32::try_from(ticks.rem_euclid(10_000_000) * 100).ok()?;
    tick_epoch().checked_add_signed(TimeDelta::new(secs, nanos)?)
}

fn add(t: Timestamp, d: Duration) -> Timestamp {
    TimeDelta::from_std(d)
        .ok()
        .and_then(|d| t.checked_add_signed(d))
        .unwrap_or(t)
}

/// C# `TokenServiceImpl`.
#[derive(Clone)]
pub struct TokenServiceImpl {
    key: Vec<u8>,
    options: TokenServiceOptions,
}

impl std::fmt::Debug for TokenServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenServiceImpl")
            .field("key", &"<redacted>")
            .field("options", &self.options)
            .finish()
    }
}

impl TokenServiceImpl {
    /// A service signing with `key`.
    ///
    /// # Errors
    /// `ValidationFailed` when `key` is empty.
    pub fn new(key: &[u8], options: TokenServiceOptions) -> AppResult<Self> {
        if key.is_empty() {
            return Err(AppError::validation("token signing key is empty", vec![]));
        }
        Ok(Self {
            key: key.to_vec(),
            options,
        })
    }

    /// A service from a base64 key (the C# `SecretKey` option format).
    ///
    /// # Errors
    /// `InvalidFormat` when the key is not base64; see [`TokenServiceImpl::new`].
    pub fn from_base64_key(key: &str, options: TokenServiceOptions) -> AppResult<Self> {
        let key = B64.decode(key).map_err(|e| AppError::Validation {
            code: ErrorCode::InvalidFormat,
            message: format!("token key is not base64: {e}"),
            errors: vec![],
        })?;
        Self::new(&key, options)
    }

    #[must_use]
    pub fn options(&self) -> TokenServiceOptions {
        self.options
    }

    fn signature(&self, data: &str) -> String {
        // HMAC accepts keys of any length; `new_from_slice` cannot fail for it.
        let mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.key).map(|mut m| {
            m.update(data.as_bytes());
            m.finalize().into_bytes()
        });
        mac.map(|b| B64.encode(b)).unwrap_or_default()
    }

    fn validate(&self, token: &str) -> Result<TokenValidationResult, String> {
        let parts: Vec<&str> = token.split('.').collect();
        let [payload_b64, signature] = parts.as_slice() else {
            return Ok(TokenValidationResult::invalid(
                "Invalid token format",
                false,
            ));
        };
        let payload = B64.decode(payload_b64).map_err(|e| e.to_string())?;
        let expected = self.signature(payload_b64);
        if !bool::from(signature.as_bytes().ct_eq(expected.as_bytes())) {
            return Ok(TokenValidationResult::invalid("Invalid signature", false));
        }
        let payload: TokenPayload = serde_json::from_slice(&payload).map_err(|e| e.to_string())?;
        let expires = from_ticks(payload.expires_at).ok_or("expiry out of range")?;
        if utc_now() > expires {
            return Ok(TokenValidationResult::invalid("Token expired", true));
        }
        Ok(TokenValidationResult::valid(
            payload.user_id,
            payload.username,
        ))
    }
}

impl TokenService for TokenServiceImpl {
    fn generate_access_token(&self, user: &AuthenticatedUser) -> TokenResult {
        let now = utc_now();
        let expires_at = add(now, self.options.access_token_lifetime);
        let payload = TokenPayload {
            user_id: user.id,
            username: user.username.clone(),
            expires_at: to_ticks(expires_at),
            issued_at: to_ticks(now),
        };
        // Serialising a struct of ints and a string cannot fail.
        let json = serde_json::to_string(&payload).unwrap_or_default();
        let payload_b64 = B64.encode(json);
        let signature = self.signature(&payload_b64);
        TokenResult {
            token: format!("{payload_b64}.{signature}"),
            expires_at,
        }
    }

    fn generate_refresh_token(&self) -> TokenResult {
        let mut bytes = [0u8; 32];
        // On the (practically impossible) failure of the OS RNG return an empty token, which
        // `validate_refresh_token` always rejects.
        let token = match getrandom::getrandom(&mut bytes) {
            Ok(()) => B64.encode(bytes),
            Err(_) => String::new(),
        };
        TokenResult {
            token,
            expires_at: add(utc_now(), self.options.refresh_token_lifetime),
        }
    }

    fn validate_access_token(&self, token: &str) -> TokenValidationResult {
        if token.trim().is_empty() {
            return TokenValidationResult::invalid("Token is empty", false);
        }
        self.validate(token).unwrap_or_else(|e| {
            TokenValidationResult::invalid(format!("Token validation failed: {e}"), false)
        })
    }

    fn validate_refresh_token(&self, token: &str, stored_token: &str) -> bool {
        if token.trim().is_empty() || stored_token.trim().is_empty() {
            return false;
        }
        bool::from(token.as_bytes().ct_eq(stored_token.as_bytes()))
    }
}
