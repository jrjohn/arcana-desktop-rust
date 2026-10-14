//! Port of C# `tests/Arcana.Domain.Tests/Validation/CustomerValidatorTests.cs` (22 tests).

use arcana_domain::{Customer, CustomerValidator, ValidationResult, Validator};
use rust_decimal_macros::dec;

fn create_valid_customer() -> Customer {
    Customer {
        code: "CUST001".into(),
        name: "Test Customer".into(),
        email: Some("test@example.com".into()),
        phone: Some("0912345678".into()),
        credit_limit: dec!(10000),
        ..Customer::default()
    }
}

fn validate(c: &Customer) -> ValidationResult {
    CustomerValidator.validate(c)
}

fn has(r: &ValidationResult, prop: &str) -> bool {
    r.errors.iter().any(|e| e.property_name == prop)
}

fn has_msg(r: &ValidationResult, prop: &str, needle: &str) -> bool {
    r.errors
        .iter()
        .any(|e| e.property_name == prop && e.error_message.contains(needle))
}

fn with(f: impl FnOnce(&mut Customer)) -> ValidationResult {
    let mut c = create_valid_customer();
    f(&mut c);
    validate(&c)
}

/// C#: `Validate_ValidCustomer_ShouldPass`
#[test]
fn validate_valid_customer_should_pass() {
    assert!(validate(&create_valid_customer()).is_valid());
}

/// C#: `Validate_MinimalValidCustomer_ShouldPass`
#[test]
fn validate_minimal_valid_customer_should_pass() {
    let c = Customer {
        code: "C001".into(),
        name: "Minimal".into(),
        ..Customer::default()
    };
    assert!(validate(&c).is_valid());
}

/// C#: `Validate_EmptyCode_ShouldFail`
#[test]
fn validate_empty_code_should_fail() {
    let r = with(|c| c.code = String::new());
    assert!(!r.is_valid() && has(&r, "Code"));
}

/// C#: `Validate_CodeExceeds20Chars_ShouldFail`
#[test]
fn validate_code_exceeds_20_chars_should_fail() {
    let r = with(|c| c.code = "A".repeat(21));
    assert!(!r.is_valid() && has_msg(&r, "Code", "20"));
}

/// C#: `Validate_CodeExactly20Chars_ShouldPass`
#[test]
fn validate_code_exactly_20_chars_should_pass() {
    assert!(with(|c| c.code = "A".repeat(20)).is_valid());
}

/// C#: `Validate_EmptyName_ShouldFail`
#[test]
fn validate_empty_name_should_fail() {
    let r = with(|c| c.name = String::new());
    assert!(!r.is_valid() && has(&r, "Name"));
}

/// C#: `Validate_NameExceeds100Chars_ShouldFail`
#[test]
fn validate_name_exceeds_100_chars_should_fail() {
    let r = with(|c| c.name = "N".repeat(101));
    assert!(!r.is_valid() && has_msg(&r, "Name", "100"));
}

/// C#: `Validate_NameExactly100Chars_ShouldPass`
#[test]
fn validate_name_exactly_100_chars_should_pass() {
    assert!(with(|c| c.name = "N".repeat(100)).is_valid());
}

/// C#: `Validate_InvalidEmail_ShouldFail`
#[test]
fn validate_invalid_email_should_fail() {
    let r = with(|c| c.email = Some("not-an-email".into()));
    assert!(!r.is_valid() && has(&r, "Email"));
}

/// C#: `Validate_ValidEmail_ShouldPass`
#[test]
fn validate_valid_email_should_pass() {
    assert!(with(|c| c.email = Some("user@domain.com".into())).is_valid());
}

/// C#: `Validate_NullEmail_ShouldPass`
#[test]
fn validate_null_email_should_pass() {
    assert!(with(|c| c.email = None).is_valid());
}

/// C#: `Validate_EmptyEmail_ShouldPass`
#[test]
fn validate_empty_email_should_pass() {
    assert!(with(|c| c.email = Some(String::new())).is_valid());
}

/// C#: `Validate_PhoneExceeds20Chars_ShouldFail`
#[test]
fn validate_phone_exceeds_20_chars_should_fail() {
    let r = with(|c| c.phone = Some("1".repeat(21)));
    assert!(!r.is_valid() && has_msg(&r, "Phone", "20"));
}

/// C#: `Validate_PhoneExactly20Chars_ShouldPass`
#[test]
fn validate_phone_exactly_20_chars_should_pass() {
    assert!(with(|c| c.phone = Some("1".repeat(20))).is_valid());
}

/// C#: `Validate_NullPhone_ShouldPass`
#[test]
fn validate_null_phone_should_pass() {
    assert!(with(|c| c.phone = None).is_valid());
}

/// C#: `Validate_EmptyPhone_ShouldPass`
#[test]
fn validate_empty_phone_should_pass() {
    assert!(with(|c| c.phone = Some(String::new())).is_valid());
}

/// C#: `Validate_NegativeCreditLimit_ShouldFail`
#[test]
fn validate_negative_credit_limit_should_fail() {
    let r = with(|c| c.credit_limit = dec!(-1));
    assert!(!r.is_valid() && has(&r, "CreditLimit"));
}

/// C#: `Validate_ZeroCreditLimit_ShouldPass`
#[test]
fn validate_zero_credit_limit_should_pass() {
    assert!(with(|c| c.credit_limit = dec!(0)).is_valid());
}

/// C#: `Validate_PositiveCreditLimit_ShouldPass`
#[test]
fn validate_positive_credit_limit_should_pass() {
    assert!(with(|c| c.credit_limit = dec!(999999)).is_valid());
}

/// C#: `Validate_TaxIdExceeds20Chars_ShouldFail`
#[test]
fn validate_tax_id_exceeds_20_chars_should_fail() {
    let r = with(|c| c.tax_id = Some("T".repeat(21)));
    assert!(!r.is_valid() && has(&r, "TaxId"));
}

/// C#: `Validate_TaxIdExactly20Chars_ShouldPass`
#[test]
fn validate_tax_id_exactly_20_chars_should_pass() {
    assert!(with(|c| c.tax_id = Some("T".repeat(20))).is_valid());
}

/// C#: `Validate_NullTaxId_ShouldPass`
#[test]
fn validate_null_tax_id_should_pass() {
    assert!(with(|c| c.tax_id = None).is_valid());
}

// ─── Extra (not in C#) ──────────────────────────────────────────────────────

/// `NotEmpty` in `FluentValidation` also rejects whitespace-only strings.
#[test]
fn extra_whitespace_code_fails_with_exact_message() {
    let r = with(|c| c.code = "   ".into());
    assert_eq!(r.errors.len(), 1);
    assert_eq!(r.errors[0].error_message, "Customer code cannot be empty");
}

#[test]
fn extra_into_result_maps_to_validation_error() {
    assert_eq!(validate(&create_valid_customer()).into_result(), Ok(()));
    let err = with(|c| c.credit_limit = dec!(-1))
        .into_result()
        .unwrap_err();
    assert_eq!(err.code(), arcana_core::ErrorCode::ValidationFailed);
    assert_eq!(err.to_user_message(), "Credit limit cannot be negative");
}
