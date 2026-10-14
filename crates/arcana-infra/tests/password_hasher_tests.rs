//! C# `PasswordHasherTests` (17; the two `[Theory]` rows each are folded into one test per
//! theory, as in M2) plus C#-compatibility vectors.

use arcana_core::ErrorCode;
use arcana_core::security::PasswordHasher;
use arcana_infra::security::{HASH_SIZE, ITERATIONS, PasswordHasherImpl, SALT_SIZE};
use base64::Engine;

fn h() -> PasswordHasherImpl {
    PasswordHasherImpl::new()
}

// HashPassword_ValidPassword_ShouldReturnNonEmptyHash
#[test]
fn hash_password_valid_password_should_return_non_empty_hash() {
    assert!(
        !h().hash_password("MySecureP@ssw0rd")
            .unwrap()
            .trim()
            .is_empty()
    );
}

// HashPassword_SamePasswordTwice_ShouldReturnDifferentHashes
#[test]
fn hash_password_same_password_twice_should_return_different_hashes() {
    assert_ne!(
        h().hash_password("password123").unwrap(),
        h().hash_password("password123").unwrap()
    );
}

// HashPassword_ReturnsFormatWithFourParts
#[test]
fn hash_password_returns_format_with_four_parts() {
    let hash = h().hash_password("password").unwrap();
    let parts: Vec<_> = hash.split(':').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "1");
    assert_eq!(parts[1], ITERATIONS.to_string());
    let b64 = base64::engine::general_purpose::STANDARD;
    assert_eq!(b64.decode(parts[2]).unwrap().len(), SALT_SIZE);
    assert_eq!(b64.decode(parts[3]).unwrap().len(), HASH_SIZE);
}

// HashPassword_EmptyOrWhitespace_ShouldThrow ([InlineData("")], [InlineData(" ")])
#[test]
fn hash_password_empty_or_whitespace_should_throw() {
    for p in ["", " "] {
        assert_eq!(
            h().hash_password(p).unwrap_err().code(),
            ErrorCode::ValidationFailed
        );
    }
}

// VerifyPassword_CorrectPassword_ShouldReturnTrue
#[test]
fn verify_password_correct_password_should_return_true() {
    let hash = h().hash_password("MySecret123!").unwrap();
    assert!(h().verify_password("MySecret123!", &hash).unwrap());
}

// VerifyPassword_WrongPassword_ShouldReturnFalse
#[test]
fn verify_password_wrong_password_should_return_false() {
    let hash = h().hash_password("correctPassword").unwrap();
    assert!(!h().verify_password("wrongPassword", &hash).unwrap());
}

// VerifyPassword_InvalidHashFormat_ShouldReturnFalse
#[test]
fn verify_password_invalid_hash_format_should_return_false() {
    assert!(!h().verify_password("password", "not-a-valid-hash").unwrap());
}

// VerifyPassword_InvalidBase64InHash_ShouldReturnFalse
#[test]
fn verify_password_invalid_base64_in_hash_should_return_false() {
    assert!(
        !h().verify_password("password", "1:100000:!!!invalid!!!:!!!invalid!!!")
            .unwrap()
    );
    // Non-numeric / zero iterations are "no match" too (C# would throw for 0).
    assert!(!h().verify_password("password", "1:abc:AAAA:AAAA").unwrap());
    assert!(!h().verify_password("password", "1:0:AAAA:AAAA").unwrap());
}

// VerifyPassword_WrongVersion_ShouldReturnFalse
#[test]
fn verify_password_wrong_version_should_return_false() {
    assert!(
        !h().verify_password("password", "99:100000:abc:def")
            .unwrap()
    );
}

// VerifyPassword_EmptyPassword_ShouldThrow ([InlineData("")], [InlineData(" ")])
#[test]
fn verify_password_empty_password_should_throw() {
    let hash = h().hash_password("realpassword").unwrap();
    for p in ["", " "] {
        assert!(h().verify_password(p, &hash).is_err());
    }
}

// VerifyPassword_EmptyHash_ShouldThrow ([InlineData("")], [InlineData(" ")])
#[test]
fn verify_password_empty_hash_should_throw() {
    for hash in ["", " "] {
        assert_eq!(
            h().verify_password("password", hash).unwrap_err().code(),
            ErrorCode::ValidationFailed
        );
    }
}

// NeedsRehash_ValidCurrentHash_ShouldReturnFalse
#[test]
fn needs_rehash_valid_current_hash_should_return_false() {
    assert!(!h().needs_rehash(&h().hash_password("password").unwrap()));
}

// NeedsRehash_NullOrEmpty_ShouldReturnTrue
#[test]
fn needs_rehash_null_or_empty_should_return_true() {
    assert!(h().needs_rehash(""));
    assert!(h().needs_rehash("   "));
}

// NeedsRehash_InvalidFormat_ShouldReturnTrue
#[test]
fn needs_rehash_invalid_format_should_return_true() {
    assert!(h().needs_rehash("invalid-hash"));
    assert!(h().needs_rehash("x:100000:abc:def"));
    assert!(h().needs_rehash("1:many:abc:def"));
}

// NeedsRehash_WrongVersion_ShouldReturnTrue
#[test]
fn needs_rehash_wrong_version_should_return_true() {
    assert!(h().needs_rehash("0:100000:abc:def"));
}

// NeedsRehash_LowIterations_ShouldReturnTrue
#[test]
fn needs_rehash_low_iterations_should_return_true() {
    assert!(h().needs_rehash("1:1000:abc:def"));
}

// VerifyPassword_MultipleCorrectRoundTrips
#[test]
fn verify_password_multiple_correct_round_trips() {
    for p in [
        "simple",
        "P@ssw0rd!123",
        "日本語パスワード",
        "very long password with spaces and special chars !@#$%^&*()",
    ] {
        let hash = h().hash_password(p).unwrap();
        assert!(h().verify_password(p, &hash).unwrap(), "{p}");
    }
}

// New: hashes produced with the C# algorithm parameters (Rfc2898DeriveBytes.Pbkdf2,
// SHA-256, 100 000 iterations, 16-byte salt, 32-byte hash, "1:iter:salt:hash"). Generated
// independently with Python `hashlib.pbkdf2_hmac`, which implements the same RFC 8018 PBKDF2.
#[test]
fn verifies_hashes_produced_with_csharp_parameters() {
    let vectors = [
        (
            "P@ssw0rd!",
            "1:100000:AAECAwQFBgcICQoLDA0ODw==:SITyk+A7SDamHaJCup6ffYgBPRdXfQ7Lk7MHn4q02+0=",
        ),
        (
            "日本語パスワード",
            "1:100000:paWlpaWlpaWlpaWlpaWlpQ==:KeSDLv+7dODm8dBKtgO5zTy8dp6pMrc5QD7WnKmTLUg=",
        ),
    ];
    for (password, hash) in vectors {
        assert!(h().verify_password(password, hash).unwrap(), "{password}");
        assert!(!h().verify_password("wrong", hash).unwrap());
        assert!(!h().needs_rehash(hash));
    }
    // An older, lower-iteration hash still verifies (iterations come from the hash) but is
    // flagged for rehash.
    let legacy = "1:10000:AAECAwQFBgcICQoLDA0ODw==:ytt5+rfAYyyO52xFMVtWIdr45UW1wpd59zviwcrOZ6M=";
    assert!(h().verify_password("legacy", legacy).unwrap());
    assert!(h().needs_rehash(legacy));
}
