//! Port of C# `tests/Arcana.Domain.Tests/Validation/OrderValidatorTests.cs` (6 tests).

use arcana_domain::{
    Order, OrderItem, OrderItemValidator, OrderStatus, OrderValidator, ValidationResult, Validator,
};
use rust_decimal_macros::dec;

fn create_valid_order() -> Order {
    Order {
        order_number: "ORD202401010001".into(),
        order_date: arcana_core::today(),
        customer_id: 1,
        customer_name: "Test Customer".into(),
        status: OrderStatus::Draft,
        tax_rate: dec!(5),
        items: vec![OrderItem {
            line_number: 1,
            product_id: 1,
            product_code: "PROD001".into(),
            product_name: "Test Product".into(),
            unit: "PCS".into(),
            quantity: dec!(1),
            unit_price: dec!(100),
            ..OrderItem::default()
        }],
        ..Order::default()
    }
}

fn has(r: &ValidationResult, prop: &str) -> bool {
    r.errors.iter().any(|e| e.property_name == prop)
}

fn with(f: impl FnOnce(&mut Order)) -> ValidationResult {
    let mut o = create_valid_order();
    f(&mut o);
    OrderValidator.validate(&o)
}

/// C#: `Validate_ValidOrder_ShouldPass`
#[test]
fn validate_valid_order_should_pass() {
    assert!(OrderValidator.validate(&create_valid_order()).is_valid());
}

/// C#: `Validate_EmptyOrderNumber_ShouldFail`
#[test]
fn validate_empty_order_number_should_fail() {
    let r = with(|o| o.order_number = String::new());
    assert!(!r.is_valid() && has(&r, "OrderNumber"));
}

/// C#: `Validate_NoCustomer_ShouldFail`
#[test]
fn validate_no_customer_should_fail() {
    let r = with(|o| o.customer_id = 0);
    assert!(!r.is_valid() && has(&r, "CustomerId"));
}

/// C#: `Validate_EmptyItems_ShouldFail`
#[test]
fn validate_empty_items_should_fail() {
    let r = with(|o| o.items.clear());
    assert!(!r.is_valid() && has(&r, "Items"));
}

/// C#: `Validate_NegativeTaxRate_ShouldFail`
#[test]
fn validate_negative_tax_rate_should_fail() {
    let r = with(|o| o.tax_rate = dec!(-5));
    assert!(!r.is_valid() && has(&r, "TaxRate"));
}

/// C#: `Validate_TaxRateOver100_ShouldFail`
#[test]
fn validate_tax_rate_over_100_should_fail() {
    let r = with(|o| o.tax_rate = dec!(150));
    assert!(!r.is_valid() && has(&r, "TaxRate"));
}

// ─── Extra (not in C#): every remaining rule of OrderValidator / OrderItemValidator ──

#[test]
fn extra_empty_items_reports_both_csharp_rules() {
    let r = with(|o| o.items.clear());
    let msgs: Vec<_> = r.errors.iter().map(|e| e.error_message.as_str()).collect();
    assert_eq!(
        msgs,
        [
            "Order items cannot be empty",
            "Order must have at least one item"
        ]
    );
}

#[test]
fn extra_header_rules() {
    assert!(has(
        &with(|o| o.order_number = "X".repeat(21)),
        "OrderNumber"
    ));
    assert!(with(|o| o.order_number = "X".repeat(20)).is_valid());
    assert!(has(&with(|o| o.customer_name = " ".into()), "CustomerName"));
    let min = chrono::NaiveDate::from_ymd_opt(1, 1, 1)
        .unwrap()
        .and_time(chrono::NaiveTime::MIN);
    assert!(has(&with(|o| o.order_date = min), "OrderDate"));
    assert!(has(
        &with(|o| o.discount_amount = dec!(-1)),
        "DiscountAmount"
    ));
    assert!(has(&with(|o| o.shipping_cost = dec!(-1)), "ShippingCost"));
    assert!(with(|o| o.tax_rate = dec!(100)).is_valid());
    assert!(with(|o| o.tax_rate = dec!(0)).is_valid());
}

#[test]
fn extra_item_rules_are_reported_with_indexed_property_names() {
    let r = with(|o| {
        let i = &mut o.items[0];
        i.product_id = 0;
        i.product_code = String::new();
        i.product_name = String::new();
        i.quantity = dec!(0);
        i.unit_price = dec!(-1);
        i.discount_percent = dec!(101);
    });
    let props: Vec<_> = r.errors.iter().map(|e| e.property_name.as_str()).collect();
    assert_eq!(
        props,
        [
            "Items[0].ProductId",
            "Items[0].ProductCode",
            "Items[0].ProductName",
            "Items[0].Quantity",
            "Items[0].UnitPrice",
            "Items[0].DiscountPercent"
        ]
    );
    let neg = with(|o| o.items[0].discount_percent = dec!(-1));
    assert_eq!(
        neg.errors[0].error_message,
        "Discount percent cannot be negative"
    );
}

#[test]
fn extra_order_item_validator_standalone() {
    let ok = create_valid_order().items.remove(0);
    assert!(OrderItemValidator.validate(&ok).is_valid());
    let bad = OrderItem::default();
    let r = OrderItemValidator.validate(&bad);
    assert!(has(&r, "ProductId") && has(&r, "Quantity"));
}
