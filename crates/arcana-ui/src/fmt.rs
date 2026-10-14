//! Display formatting shared by views and view models (C# value converters).

use arcana_core::Timestamp;
use arcana_domain::{OrderStatus, PaymentMethod, PaymentStatus};
use chrono::NaiveDate;
use rust_decimal::Decimal;

/// `d` rounded to `dp` places with `,` thousands separators (C# `ToString("N{dp}")`).
#[must_use]
pub fn number(d: Decimal, dp: u32) -> String {
    let r = d.round_dp(dp);
    let neg = r.is_sign_negative() && !r.is_zero();
    let s = format!("{:.*}", dp as usize, r.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), ""), |(i, f)| (i, f));
    let mut grouped = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sign = if neg { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac}")
    }
}

/// `$` + whole-number amount, as on the C# order summary.
#[must_use]
pub fn money(d: Decimal) -> String {
    format!("${}", number(d, 0))
}

/// `yyyy/MM/dd` (C# `DateFormatConverter` default).
#[must_use]
pub fn date(ts: Timestamp) -> String {
    ts.format("%Y/%m/%d").to_string()
}

/// `yyyy-MM-dd` for editable date fields.
#[must_use]
pub fn iso_date(ts: Timestamp) -> String {
    ts.format("%Y-%m-%d").to_string()
}

/// Parse `yyyy-MM-dd` or `yyyy/MM/dd`.
#[must_use]
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y/%m/%d"))
        .ok()
}

/// Parse a decimal typed by the user (`,` separators allowed); empty means zero.
#[must_use]
pub fn parse_decimal(s: &str) -> Option<Decimal> {
    let cleaned: String = s.trim().chars().filter(|c| *c != ',').collect();
    if cleaned.is_empty() {
        return Some(Decimal::ZERO);
    }
    cleaned.parse().ok()
}

/// Localization key of an order status (C# `StatusLocalizationConverter`, `order.status`).
#[must_use]
pub fn order_status_key(s: OrderStatus) -> &'static str {
    match s {
        OrderStatus::Draft => "order.status.draft",
        OrderStatus::Pending => "order.status.pending",
        OrderStatus::Confirmed => "order.status.confirmed",
        OrderStatus::Processing => "order.status.processing",
        OrderStatus::Shipped => "order.status.shipped",
        OrderStatus::Delivered => "order.status.delivered",
        OrderStatus::Completed => "order.status.completed",
        OrderStatus::Cancelled => "order.status.cancelled",
    }
}

/// Localization key of a payment status (`payment.status`).
#[must_use]
pub fn payment_status_key(s: PaymentStatus) -> &'static str {
    match s {
        PaymentStatus::Unpaid => "payment.status.unpaid",
        PaymentStatus::PartialPaid => "payment.status.partialPaid",
        PaymentStatus::Paid => "payment.status.paid",
        PaymentStatus::Refunded => "payment.status.refunded",
    }
}

/// Localization key of a payment method.
#[must_use]
pub fn payment_method_key(m: PaymentMethod) -> &'static str {
    match m {
        PaymentMethod::Cash => "payment.cash",
        PaymentMethod::CreditCard => "payment.creditCard",
        PaymentMethod::BankTransfer => "payment.bankTransfer",
        PaymentMethod::Check => "payment.check",
        PaymentMethod::Credit => "payment.credit",
    }
}

/// C# `OrderListViewModel.ChangeStatusAsync` progression; terminal states stay put.
#[must_use]
pub fn next_status(s: OrderStatus) -> OrderStatus {
    match s {
        OrderStatus::Draft => OrderStatus::Pending,
        OrderStatus::Pending => OrderStatus::Confirmed,
        OrderStatus::Confirmed => OrderStatus::Processing,
        OrderStatus::Processing => OrderStatus::Shipped,
        OrderStatus::Shipped => OrderStatus::Delivered,
        OrderStatus::Delivered => OrderStatus::Completed,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn number_groups_thousands_and_rounds() {
        assert_eq!(number(dec!(65963.4), 0), "65,963");
        assert_eq!(number(dec!(1234567.891), 2), "1,234,567.89");
        assert_eq!(number(dec!(999), 0), "999");
        assert_eq!(number(dec!(-1500), 0), "-1,500");
        assert_eq!(number(dec!(0), 1), "0.0");
        assert_eq!(money(dec!(62822)), "$62,822");
    }

    #[test]
    fn dates_and_decimals_parse() {
        let d = parse_date("2025-12-01").unwrap();
        assert_eq!(parse_date("2025/12/01"), Some(d));
        assert_eq!(parse_date("nope"), None);
        let ts = d.and_hms_opt(0, 0, 0).unwrap();
        assert_eq!(date(ts), "2025/12/01");
        assert_eq!(iso_date(ts), "2025-12-01");
        assert_eq!(parse_decimal(" 1,200.5 "), Some(dec!(1200.5)));
        assert_eq!(parse_decimal(""), Some(Decimal::ZERO));
        assert_eq!(parse_decimal("x"), None);
    }

    #[test]
    fn status_keys_and_progression() {
        for s in OrderStatus::ALL {
            assert!(order_status_key(*s).starts_with("order.status."));
        }
        for s in PaymentStatus::ALL {
            assert!(payment_status_key(*s).starts_with("payment.status."));
        }
        for m in PaymentMethod::ALL {
            assert!(payment_method_key(*m).starts_with("payment."));
        }
        assert_eq!(next_status(OrderStatus::Draft), OrderStatus::Pending);
        assert_eq!(next_status(OrderStatus::Delivered), OrderStatus::Completed);
        assert_eq!(next_status(OrderStatus::Completed), OrderStatus::Completed);
        assert_eq!(next_status(OrderStatus::Cancelled), OrderStatus::Cancelled);
        let mut s = OrderStatus::Pending;
        for _ in 0..4 {
            s = next_status(s);
        }
        assert_eq!(s, OrderStatus::Delivered);
        assert_eq!(next_status(OrderStatus::Confirmed), OrderStatus::Processing);
        assert_eq!(next_status(OrderStatus::Processing), OrderStatus::Shipped);
        assert_eq!(next_status(OrderStatus::Shipped), OrderStatus::Delivered);
    }
}
