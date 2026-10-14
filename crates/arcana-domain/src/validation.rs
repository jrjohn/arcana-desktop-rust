//! Validators with the same rules and messages as the C# `FluentValidation` validators
//! (`CustomerValidator`, `ProductValidator`, `OrderValidator`, `OrderItemValidator`).
//!
//! `FluentValidation` semantics reproduced here:
//! - Every rule in a chain is evaluated (default `CascadeMode.Continue`).
//! - `NotEmpty` on a string rejects null, `""` and whitespace-only strings.
//! - `MaximumLength` counts UTF-16 code units (`string.Length`).
//! - `EmailAddress` (default ASP.NET-Core-compatible mode) only checks for exactly one `@`
//!   that is neither the first nor the last character.
//! - `NotEmpty` on a `DateTime` rejects `default(DateTime)` (0001-01-01 00:00:00).
//! - Child-collection failures are reported as `Items[i].Property`.

use arcana_core::{AppError, AppResult, Timestamp};
use chrono::{Datelike, NaiveTime, Timelike};
use rust_decimal::Decimal;

use crate::entities::{Customer, Order, OrderItem, Product};

/// One failed rule (C# `ValidationFailure`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFailure {
    pub property_name: String,
    pub error_message: String,
}

/// All failures of one validation run (C# `ValidationResult`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationResult {
    pub errors: Vec<ValidationFailure>,
}

impl ValidationResult {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// `Ok(())` when valid, otherwise `AppError::Validation` carrying every message.
    ///
    /// # Errors
    /// Returns [`AppError::Validation`] when at least one rule failed.
    pub fn into_result(self) -> AppResult<()> {
        if self.is_valid() {
            Ok(())
        } else {
            Err(AppError::validation(
                "Validation failed",
                self.errors.into_iter().map(|e| e.error_message).collect(),
            ))
        }
    }

    fn fail(&mut self, property: &str, message: &str) {
        self.errors.push(ValidationFailure {
            property_name: property.to_owned(),
            error_message: message.to_owned(),
        });
    }

    fn check(&mut self, ok: bool, property: &str, message: &str) {
        if !ok {
            self.fail(property, message);
        }
    }
}

/// C# `AbstractValidator<T>.Validate`.
pub trait Validator<T> {
    fn validate(&self, item: &T) -> ValidationResult;
}

fn not_empty(s: &str) -> bool {
    !s.trim().is_empty()
}

fn max_len(s: &str, max: usize) -> bool {
    s.encode_utf16().count() <= max
}

fn has_value(s: Option<&String>) -> Option<&str> {
    s.map(String::as_str).filter(|v| !v.is_empty())
}

fn is_email(s: &str) -> bool {
    match (s.find('@'), s.rfind('@')) {
        (Some(first), Some(last)) => first > 0 && first == last && first != s.len() - 1,
        _ => false,
    }
}

fn is_default_datetime(d: Timestamp) -> bool {
    d.year() == 1 && d.ordinal() == 1 && d.time() == NaiveTime::MIN && d.nanosecond() == 0
}

/// C# `CustomerValidator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CustomerValidator;

impl Validator<Customer> for CustomerValidator {
    fn validate(&self, c: &Customer) -> ValidationResult {
        let mut r = ValidationResult::default();
        r.check(not_empty(&c.code), "Code", "Customer code cannot be empty");
        r.check(
            max_len(&c.code, 20),
            "Code",
            "Customer code cannot exceed 20 characters",
        );
        r.check(not_empty(&c.name), "Name", "Customer name cannot be empty");
        r.check(
            max_len(&c.name, 100),
            "Name",
            "Customer name cannot exceed 100 characters",
        );
        if let Some(email) = has_value(c.email.as_ref()) {
            r.check(is_email(email), "Email", "Email format is invalid");
        }
        if let Some(phone) = has_value(c.phone.as_ref()) {
            r.check(
                max_len(phone, 20),
                "Phone",
                "Phone number cannot exceed 20 characters",
            );
        }
        r.check(
            c.credit_limit >= Decimal::ZERO,
            "CreditLimit",
            "Credit limit cannot be negative",
        );
        if let Some(tax_id) = has_value(c.tax_id.as_ref()) {
            r.check(
                max_len(tax_id, 20),
                "TaxId",
                "Tax ID cannot exceed 20 characters",
            );
        }
        r
    }
}

/// C# `ProductValidator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProductValidator;

impl Validator<Product> for ProductValidator {
    fn validate(&self, p: &Product) -> ValidationResult {
        let mut r = ValidationResult::default();
        r.check(not_empty(&p.code), "Code", "Product code cannot be empty");
        r.check(
            max_len(&p.code, 50),
            "Code",
            "Product code cannot exceed 50 characters",
        );
        r.check(not_empty(&p.name), "Name", "Product name cannot be empty");
        r.check(
            max_len(&p.name, 200),
            "Name",
            "Product name cannot exceed 200 characters",
        );
        r.check(not_empty(&p.unit), "Unit", "Unit cannot be empty");
        r.check(
            max_len(&p.unit, 10),
            "Unit",
            "Unit cannot exceed 10 characters",
        );
        r.check(
            p.price >= Decimal::ZERO,
            "Price",
            "Price cannot be negative",
        );
        r.check(p.cost >= Decimal::ZERO, "Cost", "Cost cannot be negative");
        r.check(
            p.stock_quantity >= Decimal::ZERO,
            "StockQuantity",
            "Stock quantity cannot be negative",
        );
        r.check(
            p.min_stock_level >= Decimal::ZERO,
            "MinStockLevel",
            "Minimum stock level cannot be negative",
        );
        if p.max_stock_level > Decimal::ZERO {
            r.check(
                p.max_stock_level >= p.min_stock_level,
                "MaxStockLevel",
                "Maximum stock level cannot be less than minimum stock level",
            );
        }
        if let Some(barcode) = has_value(p.barcode.as_ref()) {
            r.check(
                max_len(barcode, 50),
                "Barcode",
                "Barcode cannot exceed 50 characters",
            );
        }
        r
    }
}

/// C# `OrderItemValidator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct OrderItemValidator;

impl OrderItemValidator {
    fn validate_into(item: &OrderItem, prefix: &str, r: &mut ValidationResult) {
        let name = |p: &str| format!("{prefix}{p}");
        r.check(
            item.product_id > 0,
            &name("ProductId"),
            "Please select a product",
        );
        r.check(
            not_empty(&item.product_code),
            &name("ProductCode"),
            "Product code cannot be empty",
        );
        r.check(
            not_empty(&item.product_name),
            &name("ProductName"),
            "Product name cannot be empty",
        );
        r.check(
            item.quantity > Decimal::ZERO,
            &name("Quantity"),
            "Quantity must be greater than 0",
        );
        r.check(
            item.unit_price >= Decimal::ZERO,
            &name("UnitPrice"),
            "Unit price cannot be negative",
        );
        r.check(
            item.discount_percent >= Decimal::ZERO,
            &name("DiscountPercent"),
            "Discount percent cannot be negative",
        );
        r.check(
            item.discount_percent <= Decimal::ONE_HUNDRED,
            &name("DiscountPercent"),
            "Discount percent cannot exceed 100%",
        );
    }
}

impl Validator<OrderItem> for OrderItemValidator {
    fn validate(&self, item: &OrderItem) -> ValidationResult {
        let mut r = ValidationResult::default();
        Self::validate_into(item, "", &mut r);
        r
    }
}

/// C# `OrderValidator` (includes `RuleForEach(Items).SetValidator(OrderItemValidator)`).
#[derive(Debug, Clone, Copy, Default)]
pub struct OrderValidator;

impl Validator<Order> for OrderValidator {
    fn validate(&self, o: &Order) -> ValidationResult {
        let mut r = ValidationResult::default();
        r.check(
            not_empty(&o.order_number),
            "OrderNumber",
            "Order number cannot be empty",
        );
        r.check(
            max_len(&o.order_number, 20),
            "OrderNumber",
            "Order number cannot exceed 20 characters",
        );
        r.check(o.customer_id > 0, "CustomerId", "Please select a customer");
        r.check(
            not_empty(&o.customer_name),
            "CustomerName",
            "Customer name cannot be empty",
        );
        r.check(
            !is_default_datetime(o.order_date),
            "OrderDate",
            "Order date cannot be empty",
        );
        r.check(!o.items.is_empty(), "Items", "Order items cannot be empty");
        r.check(
            !o.items.is_empty(),
            "Items",
            "Order must have at least one item",
        );
        r.check(
            o.tax_rate >= Decimal::ZERO,
            "TaxRate",
            "Tax rate cannot be negative",
        );
        r.check(
            o.tax_rate <= Decimal::ONE_HUNDRED,
            "TaxRate",
            "Tax rate cannot exceed 100%",
        );
        r.check(
            o.discount_amount >= Decimal::ZERO,
            "DiscountAmount",
            "Discount amount cannot be negative",
        );
        r.check(
            o.shipping_cost >= Decimal::ZERO,
            "ShippingCost",
            "Shipping cost cannot be negative",
        );
        for (i, item) in o.items.iter().enumerate() {
            OrderItemValidator::validate_into(item, &format!("Items[{i}]."), &mut r);
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_rule_matches_fluent_validation_default_mode() {
        assert!(is_email("a@b"));
        assert!(!is_email("@b"));
        assert!(!is_email("a@"));
        assert!(!is_email("a@b@c"));
        assert!(!is_email("plain"));
    }

    #[test]
    fn not_empty_rejects_whitespace_and_max_len_counts_utf16() {
        assert!(!not_empty("   "));
        assert!(max_len("😀", 2));
        assert!(!max_len("😀", 1));
    }

    #[test]
    fn default_datetime_detection() {
        let min = chrono::NaiveDate::from_ymd_opt(1, 1, 1)
            .map(|d| d.and_time(NaiveTime::MIN))
            .unwrap();
        assert!(is_default_datetime(min));
        assert!(!is_default_datetime(arcana_core::today()));
    }
}
