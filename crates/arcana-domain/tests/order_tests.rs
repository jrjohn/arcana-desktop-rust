//! Port of C# `tests/Arcana.Domain.Tests/Entities/OrderTests.cs` (6 tests).

use arcana_domain::{Order, OrderItem};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn item(quantity: Decimal, unit_price: Decimal, discount_percent: Decimal) -> OrderItem {
    OrderItem {
        quantity,
        unit_price,
        discount_percent,
        ..OrderItem::default()
    }
}

fn order(items: Vec<OrderItem>) -> Order {
    Order {
        tax_rate: dec!(5),
        items,
        ..Order::default()
    }
}

/// C#: `CalculateTotals_SingleItem_ShouldCalculateCorrectly`
#[test]
fn calculate_totals_single_item_should_calculate_correctly() {
    let mut o = order(vec![item(dec!(2), dec!(100), dec!(0))]);
    o.calculate_totals();
    assert_eq!(o.subtotal, dec!(200));
    assert_eq!(o.tax_amount, dec!(10));
    assert_eq!(o.total_amount, dec!(210));
}

/// C#: `CalculateTotals_MultipleItems_ShouldCalculateCorrectly`
#[test]
fn calculate_totals_multiple_items_should_calculate_correctly() {
    let mut o = order(vec![
        item(dec!(2), dec!(100), dec!(0)),
        item(dec!(3), dec!(50), dec!(0)),
    ]);
    o.calculate_totals();
    assert_eq!(o.subtotal, dec!(350));
    assert_eq!(o.tax_amount, dec!(17.5));
    assert_eq!(o.total_amount, dec!(367.5));
}

/// C#: `CalculateTotals_WithDiscount_ShouldCalculateCorrectly`
#[test]
fn calculate_totals_with_discount_should_calculate_correctly() {
    let mut o = order(vec![item(dec!(2), dec!(100), dec!(0))]);
    o.discount_amount = dec!(50);
    o.calculate_totals();
    assert_eq!(o.subtotal, dec!(200));
    assert_eq!(o.tax_amount, dec!(10));
    assert_eq!(o.total_amount, dec!(160)); // 200 + 10 - 50
}

/// C#: `CalculateTotals_WithShipping_ShouldCalculateCorrectly`
#[test]
fn calculate_totals_with_shipping_should_calculate_correctly() {
    let mut o = order(vec![item(dec!(2), dec!(100), dec!(0))]);
    o.shipping_cost = dec!(100);
    o.calculate_totals();
    assert_eq!(o.subtotal, dec!(200));
    assert_eq!(o.tax_amount, dec!(10));
    assert_eq!(o.total_amount, dec!(310)); // 200 + 10 + 100
}

/// C#: `OrderItem_LineTotal_ShouldCalculateCorrectly`
#[test]
fn order_item_line_total_should_calculate_correctly() {
    assert_eq!(item(dec!(3), dec!(100), dec!(10)).line_total(), dec!(270));
}

/// C#: `OrderItem_LineTotal_NoDiscount_ShouldCalculateCorrectly`
#[test]
fn order_item_line_total_no_discount_should_calculate_correctly() {
    assert_eq!(item(dec!(5), dec!(200), dec!(0)).line_total(), dec!(1000));
}

// ─── Extra (not in C#) ──────────────────────────────────────────────────────

/// C# `Math.Round(decimal, 2)` rounds half to even; so must we.
#[test]
fn extra_tax_rounding_is_bankers_rounding() {
    // 0.125 * 100% tax → 0.125 → 0.12 (even), 0.135 → 0.14
    let mut o = order(vec![item(dec!(1), dec!(0.125), dec!(0))]);
    o.tax_rate = dec!(100);
    o.calculate_totals();
    assert_eq!(o.tax_amount, dec!(0.12));
    o.items = vec![item(dec!(1), dec!(0.135), dec!(0))];
    o.calculate_totals();
    assert_eq!(o.tax_amount, dec!(0.14));
}

#[test]
fn extra_order_defaults_match_csharp() {
    let o = Order::default();
    assert_eq!(o.tax_rate, dec!(5));
    assert_eq!(o.order_date, arcana_core::today());
    assert_eq!(o.status, arcana_domain::OrderStatus::Draft);
    assert_eq!(o.payment_status, arcana_domain::PaymentStatus::Unpaid);
    assert_eq!(o.payment_method, arcana_domain::PaymentMethod::Cash);
    assert_eq!(OrderItem::default().unit, "PCS");
}

#[test]
fn extra_enum_values_match_csharp() {
    use arcana_domain::{AuditEventType, OrderStatus, PaymentMethod, PaymentStatus};
    assert_eq!(OrderStatus::Cancelled.value(), 7);
    assert_eq!(OrderStatus::from_value(4), Some(OrderStatus::Shipped));
    assert_eq!(OrderStatus::ALL.len(), 8);
    assert_eq!(PaymentStatus::Refunded.value(), 3);
    assert_eq!(PaymentMethod::from_value(4), Some(PaymentMethod::Credit));
    assert_eq!(PaymentMethod::from_value(5), None);
    assert_eq!(AuditEventType::PluginUninstalled.value(), 29);
    assert_eq!(AuditEventType::ALL.len(), 30);
}
