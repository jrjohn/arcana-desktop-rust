//! PBKDF2 password hashing (C# `PasswordHasherImpl`).
//!
//! Format and parameters are identical to C#, so hashes move between the two apps:
//! `version:iterations:base64(salt):base64(hash)` = `1:100000:<16-byte salt>:<32-byte hash>`,
//! PBKDF2-HMAC-SHA256.

use arcana_core::security::PasswordHasher;
use arcana_core::{AppError, AppResult};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// 128-bit salt.
pub const SALT_SIZE: usize = 16;
/// 256-bit hash.
pub const HASH_SIZE: usize = 32;
/// OWASP recommendation used by C#.
pub const ITERATIONS: u32 = 100_000;
/// Format version.
pub const CURRENT_VERSION: u32 = 1;
const DELIMITER: char = ':';

/// C# `PasswordHasherImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PasswordHasherImpl;

impl PasswordHasherImpl {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

fn blank(what: &str) -> AppError {
    AppError::validation(format!("{what} must not be empty or whitespace"), vec![])
}

fn derive(password: &str, salt: &[u8], iterations: u32, len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut out);
    out
}

impl PasswordHasher for PasswordHasherImpl {
    fn hash_password(&self, password: &str) -> AppResult<String> {
        if password.trim().is_empty() {
            return Err(blank("password"));
        }
        let mut salt = [0u8; SALT_SIZE];
        getrandom::getrandom(&mut salt).map_err(|e| AppError::Unknown {
            message: format!("random source failed: {e}"),
            inner: None,
        })?;
        let hash = derive(password, &salt, ITERATIONS, HASH_SIZE);
        Ok(format!(
            "{CURRENT_VERSION}{DELIMITER}{ITERATIONS}{DELIMITER}{}{DELIMITER}{}",
            B64.encode(salt),
            B64.encode(hash)
        ))
    }

    fn verify_password(&self, password: &str, hashed_password: &str) -> AppResult<bool> {
        if password.trim().is_empty() {
            return Err(blank("password"));
        }
        if hashed_password.trim().is_empty() {
            return Err(blank("hashedPassword"));
        }
        let parts: Vec<&str> = hashed_password.split(DELIMITER).collect();
        let [version, iterations, salt, hash] = parts.as_slice() else {
            return Ok(false);
        };
        if version.parse::<u32>().ok() != Some(CURRENT_VERSION) {
            return Ok(false);
        }
        // C# lets PBKDF2 throw on a zero/negative count; a malformed hash is just "no match".
        let Some(iterations) = iterations.parse::<u32>().ok().filter(|n| *n > 0) else {
            return Ok(false);
        };
        let (Ok(salt), Ok(stored)) = (B64.decode(salt), B64.decode(hash)) else {
            return Ok(false);
        };
        let computed = derive(password, &salt, iterations, stored.len());
        Ok(bool::from(stored.ct_eq(&computed)))
    }

    fn needs_rehash(&self, hashed_password: &str) -> bool {
        if hashed_password.trim().is_empty() {
            return true;
        }
        let parts: Vec<&str> = hashed_password.split(DELIMITER).collect();
        let [version, iterations, _, _] = parts.as_slice() else {
            return true;
        };
        !matches!(version.parse::<u32>(), Ok(v) if v >= CURRENT_VERSION)
            || !matches!(iterations.parse::<u32>(), Ok(n) if n >= ITERATIONS)
    }
}
