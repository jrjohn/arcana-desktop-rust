//! Port of C# `tests/Arcana.Domain.Tests/Validation/ProductValidatorTests.cs` (26 tests).

use arcana_domain::{Product, ProductValidator, ValidationResult, Validator};
use rust_decimal_macros::dec;

fn create_valid_product() -> Product {
    Product {
        code: "P001".into(),
        name: "Test Widget".into(),
        unit: "PCS".into(),
        price: dec!(100),
        cost: dec!(60),
        stock_quantity: dec!(50),
        min_stock_level: dec!(5),
        max_stock_level: dec!(200),
        is_active: true,
        ..Product::default()
    }
}

fn has(r: &ValidationResult, prop: &str) -> bool {
    r.errors.iter().any(|e| e.property_name == prop)
}

fn with(f: impl FnOnce(&mut Product)) -> ValidationResult {
    let mut p = create_valid_product();
    f(&mut p);
    ProductValidator.validate(&p)
}

/// C#: `Validate_ValidProduct_ShouldPass`
#[test]
fn validate_valid_product_should_pass() {
    assert!(
        ProductValidator
            .validate(&create_valid_product())
            .is_valid()
    );
}

/// C#: `Validate_EmptyCode_ShouldFail`
#[test]
fn validate_empty_code_should_fail() {
    let r = with(|p| p.code = String::new());
    assert!(!r.is_valid() && has(&r, "Code"));
}

/// C#: `Validate_CodeTooLong_ShouldFail`
#[test]
fn validate_code_too_long_should_fail() {
    let r = with(|p| p.code = "X".repeat(51));
    assert!(!r.is_valid() && has(&r, "Code"));
}

/// C#: `Validate_CodeExactlyMaxLength_ShouldPass`
#[test]
fn validate_code_exactly_max_length_should_pass() {
    assert!(with(|p| p.code = "X".repeat(50)).is_valid());
}

/// C#: `Validate_EmptyName_ShouldFail`
#[test]
fn validate_empty_name_should_fail() {
    let r = with(|p| p.name = String::new());
    assert!(!r.is_valid() && has(&r, "Name"));
}

/// C#: `Validate_NameTooLong_ShouldFail`
#[test]
fn validate_name_too_long_should_fail() {
    let r = with(|p| p.name = "A".repeat(201));
    assert!(!r.is_valid() && has(&r, "Name"));
}

/// C#: `Validate_NameExactlyMaxLength_ShouldPass`
#[test]
fn validate_name_exactly_max_length_should_pass() {
    assert!(with(|p| p.name = "A".repeat(200)).is_valid());
}

/// C#: `Validate_EmptyUnit_ShouldFail`
#[test]
fn validate_empty_unit_should_fail() {
    let r = with(|p| p.unit = String::new());
    assert!(!r.is_valid() && has(&r, "Unit"));
}

/// C#: `Validate_UnitTooLong_ShouldFail`
#[test]
fn validate_unit_too_long_should_fail() {
    let r = with(|p| p.unit = "TOOLONGUNIT".into());
    assert!(!r.is_valid() && has(&r, "Unit"));
}

/// C#: `Validate_UnitExactlyMaxLength_ShouldPass`
#[test]
fn validate_unit_exactly_max_length_should_pass() {
    assert!(with(|p| p.unit = "1234567890".into()).is_valid());
}

/// C#: `Validate_NegativePrice_ShouldFail`
#[test]
fn validate_negative_price_should_fail() {
    let r = with(|p| p.price = dec!(-1));
    assert!(!r.is_valid() && has(&r, "Price"));
}

/// C#: `Validate_ZeroPrice_ShouldPass`
#[test]
fn validate_zero_price_should_pass() {
    assert!(with(|p| p.price = dec!(0)).is_valid());
}

/// C#: `Validate_NegativeCost_ShouldFail`
#[test]
fn validate_negative_cost_should_fail() {
    let r = with(|p| p.cost = dec!(-5));
    assert!(!r.is_valid() && has(&r, "Cost"));
}

/// C#: `Validate_ZeroCost_ShouldPass`
#[test]
fn validate_zero_cost_should_pass() {
    assert!(with(|p| p.cost = dec!(0)).is_valid());
}

/// C#: `Validate_NegativeStockQuantity_ShouldFail`
#[test]
fn validate_negative_stock_quantity_should_fail() {
    let r = with(|p| p.stock_quantity = dec!(-10));
    assert!(!r.is_valid() && has(&r, "StockQuantity"));
}

/// C#: `Validate_ZeroStockQuantity_ShouldPass`
#[test]
fn validate_zero_stock_quantity_should_pass() {
    assert!(with(|p| p.stock_quantity = dec!(0)).is_valid());
}

/// C#: `Validate_NegativeMinStockLevel_ShouldFail`
#[test]
fn validate_negative_min_stock_level_should_fail() {
    let r = with(|p| p.min_stock_level = dec!(-1));
    assert!(!r.is_valid() && has(&r, "MinStockLevel"));
}

/// C#: `Validate_ZeroMinStockLevel_ShouldPass`
#[test]
fn validate_zero_min_stock_level_should_pass() {
    assert!(with(|p| p.min_stock_level = dec!(0)).is_valid());
}

/// C#: `Validate_MaxStockLevelLessThanMin_ShouldFail`
#[test]
fn validate_max_stock_level_less_than_min_should_fail() {
    let r = with(|p| {
        p.min_stock_level = dec!(50);
        p.max_stock_level = dec!(10);
    });
    assert!(!r.is_valid() && has(&r, "MaxStockLevel"));
}

/// C#: `Validate_MaxStockLevelEqualToMin_ShouldPass`
#[test]
fn validate_max_stock_level_equal_to_min_should_pass() {
    assert!(
        with(|p| {
            p.min_stock_level = dec!(10);
            p.max_stock_level = dec!(10);
        })
        .is_valid()
    );
}

/// C#: `Validate_ZeroMaxStockLevel_ShouldPass`
#[test]
fn validate_zero_max_stock_level_should_pass() {
    assert!(
        with(|p| {
            p.min_stock_level = dec!(100);
            p.max_stock_level = dec!(0);
        })
        .is_valid()
    );
}

/// C#: `Validate_NullBarcode_ShouldPass`
#[test]
fn validate_null_barcode_should_pass() {
    assert!(with(|p| p.barcode = None).is_valid());
}

/// C#: `Validate_ValidBarcode_ShouldPass`
#[test]
fn validate_valid_barcode_should_pass() {
    assert!(with(|p| p.barcode = Some("1234567890123".into())).is_valid());
}

/// C#: `Validate_BarcodeTooLong_ShouldFail`
#[test]
fn validate_barcode_too_long_should_fail() {
    let r = with(|p| p.barcode = Some("1".repeat(51)));
    assert!(!r.is_valid() && has(&r, "Barcode"));
}

/// C#: `Validate_BarcodeExactlyMaxLength_ShouldPass`
#[test]
fn validate_barcode_exactly_max_length_should_pass() {
    assert!(with(|p| p.barcode = Some("1".repeat(50))).is_valid());
}

/// C#: `Validate_MultipleInvalid_ShouldReturnMultipleErrors`
#[test]
fn validate_multiple_invalid_should_return_multiple_errors() {
    let p = Product {
        code: String::new(),
        name: String::new(),
        unit: String::new(),
        price: dec!(-1),
        cost: dec!(-5),
        stock_quantity: dec!(0),
        min_stock_level: dec!(0),
        max_stock_level: dec!(0),
        ..Product::default()
    };
    let r = ProductValidator.validate(&p);
    assert!(!r.is_valid());
    assert!(r.errors.len() > 3);
}

// ─── Extra (not in C#) ──────────────────────────────────────────────────────

#[test]
fn extra_messages_match_csharp_exactly() {
    let r = with(|p| p.min_stock_level = dec!(50));
    assert!(r.is_valid());
    let r = with(|p| {
        p.min_stock_level = dec!(50);
        p.max_stock_level = dec!(10);
    });
    assert_eq!(
        r.errors[0].error_message,
        "Maximum stock level cannot be less than minimum stock level"
    );
    let r = with(|p| p.unit = "TOOLONGUNIT".into());
    assert_eq!(
        r.errors[0].error_message,
        "Unit cannot exceed 10 characters"
    );
}
