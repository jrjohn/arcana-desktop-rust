//! Demo data on first run (C# `App.SeedSampleDataAsync`).
//!
//! Same 5 customers, 3 categories, 8 products and 10 orders with 1–5 items each. C# uses
//! `new Random(42)`; .NET's generator is not reproduced here, so a small deterministic LCG
//! picks customers/products/statuses instead — the shape of the data is the same, the exact
//! picks differ. Unlike C# (which deletes and recreates the database on every start), this
//! only seeds an empty database.

use arcana_core::{AppResult, Repository, today};
use arcana_data::SqliteRepository;
use arcana_domain::{
    Customer, Order, OrderItem, OrderStatus, PaymentMethod, PaymentStatus, Product, ProductCategory,
};
use chrono::Duration;
use rust_decimal::Decimal;

use crate::wiring::ArcanaServices;

/// Deterministic linear congruential generator (Numerical Recipes constants).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let hi = usize::try_from(self.0 >> 33).unwrap_or(0);
        hi % below.max(1)
    }

    fn range(&mut self, lo: usize, hi_exclusive: usize) -> usize {
        lo + self.next(hi_exclusive - lo)
    }
}

#[allow(clippy::too_many_arguments)]
fn customer(
    code: &str,
    name: &str,
    contact: &str,
    phone: &str,
    email: &str,
    address: &str,
    city: &str,
    tax_id: &str,
    credit: i64,
) -> Customer {
    Customer {
        code: code.to_owned(),
        name: name.to_owned(),
        contact_name: Some(contact.to_owned()),
        phone: Some(phone.to_owned()),
        email: Some(email.to_owned()),
        address: Some(address.to_owned()),
        city: Some(city.to_owned()),
        tax_id: Some(tax_id.to_owned()),
        credit_limit: Decimal::from(credit),
        is_active: true,
        ..Customer::default()
    }
}

fn customers() -> Vec<Customer> {
    vec![
        customer(
            "C001",
            "Taipei Tech Co., Ltd.",
            "David Wang",
            "02-2345-6789",
            "wang@taipei-tech.com",
            "100 Xinyi Road, Xinyi District",
            "Taipei",
            "12345678",
            100_000,
        ),
        customer(
            "C002",
            "Hsinchu Electronics Inc.",
            "Sarah Lee",
            "03-5678-1234",
            "lee@hsinchu-elec.com",
            "200 Guangfu Road, East District",
            "Hsinchu",
            "23456789",
            200_000,
        ),
        customer(
            "C003",
            "Taichung Precision Industries",
            "Michelle Zhang",
            "04-2345-6789",
            "zhang@taichung-precision.com",
            "300 Industrial Road, Xitun District",
            "Taichung",
            "34567890",
            150_000,
        ),
        customer(
            "C004",
            "Kaohsiung Trading Co.",
            "Michael Chen",
            "07-7654-3210",
            "chen@kaohsiung-trade.com",
            "400 Zhongshan Road, Qianzhen District",
            "Kaohsiung",
            "45678901",
            80_000,
        ),
        customer(
            "C005",
            "Taoyuan Logistics Center",
            "Jennifer Lin",
            "03-3456-7890",
            "lin@taoyuan-logistics.com",
            "500 Zhongzheng Road, Zhongli District",
            "Taoyuan",
            "56789012",
            120_000,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn product(
    code: &str,
    name: &str,
    cat: i32,
    unit: &str,
    price: i64,
    cost: i64,
    stock: i64,
    min: i64,
) -> Product {
    Product {
        code: code.to_owned(),
        name: name.to_owned(),
        category_id: Some(cat),
        unit: unit.to_owned(),
        price: Decimal::from(price),
        cost: Decimal::from(cost),
        stock_quantity: Decimal::from(stock),
        min_stock_level: Decimal::from(min),
        is_active: true,
        ..Product::default()
    }
}

/// Seed when the database has no customers. Returns whether anything was written.
///
/// # Errors
/// Any storage or validation failure from the services.
pub async fn seed_demo_data(services: &ArcanaServices) -> AppResult<bool> {
    let existing = services
        .customers
        .get_customers(&arcana_core::PageRequest::new(1, 1))
        .await?;
    if existing.total_count > 0 {
        return Ok(false);
    }

    let mut created_customers = Vec::new();
    for c in customers() {
        created_customers.push(services.customers.create_customer(c).await?);
    }

    let products = seed_products(services).await?;
    seed_orders(services, &created_customers, &products).await?;
    Ok(true)
}

async fn seed_products(services: &ArcanaServices) -> AppResult<Vec<Product>> {
    let cats = SqliteRepository::<ProductCategory>::new(services.db.clone());
    let mut cat_ids = Vec::new();
    for (i, (code, name)) in [
        ("ELEC", "Electronics"),
        ("COMP", "Computer Peripherals"),
        ("OFFICE", "Office Supplies"),
    ]
    .into_iter()
    .enumerate()
    {
        let c = cats
            .add(ProductCategory {
                code: code.to_owned(),
                name: name.to_owned(),
                sort_order: i32::try_from(i + 1).unwrap_or(0),
                ..ProductCategory::default()
            })
            .await?;
        cat_ids.push(c.base.id);
    }

    let (elec, comp, office) = (cat_ids[0], cat_ids[1], cat_ids[2]);
    let mut products = Vec::new();
    for p in [
        product("P001", "Laptop 15-inch", elec, "pc", 35_000, 28_000, 50, 10),
        product("P002", "Wireless Mouse", comp, "pc", 800, 400, 200, 50),
        product(
            "P003",
            "Mechanical Keyboard",
            comp,
            "pc",
            2_500,
            1_500,
            100,
            20,
        ),
        product("P004", "27-inch Monitor", elec, "pc", 12_000, 8_000, 30, 5),
        product(
            "P005",
            "USB Flash Drive 64GB",
            comp,
            "pc",
            350,
            150,
            500,
            100,
        ),
        product("P006", "Printer", office, "pc", 8_000, 5_000, 25, 5),
        product(
            "P007",
            "A4 Copy Paper (500 sheets)",
            office,
            "pack",
            120,
            80,
            1_000,
            200,
        ),
        product("P008", "Webcam", comp, "pc", 1_500, 800, 80, 15),
    ] {
        products.push(services.products.create_product(p).await?);
    }
    Ok(products)
}

async fn seed_orders(
    services: &ArcanaServices,
    created_customers: &[Customer],
    products: &[Product],
) -> AppResult<()> {
    let statuses = [
        OrderStatus::Draft,
        OrderStatus::Pending,
        OrderStatus::Confirmed,
        OrderStatus::Processing,
        OrderStatus::Shipped,
        OrderStatus::Completed,
    ];
    let payments = [
        PaymentStatus::Unpaid,
        PaymentStatus::PartialPaid,
        PaymentStatus::Paid,
    ];
    let mut rng = Lcg(42);
    let month = chrono::Local::now().format("%Y%m").to_string();
    for i in 1..=10 {
        let c = &created_customers[rng.next(created_customers.len())];
        let days = i64::try_from(rng.range(1, 30)).unwrap_or(1);
        let order_date = today() - Duration::days(days);
        let payment_status = payments[rng.next(payments.len())];
        let mut order = Order {
            order_number: format!("ORD-{month}-{i:04}"),
            order_date,
            customer_id: c.base.id,
            customer_name: c.name.clone(),
            status: statuses[rng.next(statuses.len())],
            payment_status,
            payment_method: PaymentMethod::ALL[rng.next(3)],
            tax_rate: Decimal::from(5),
            shipping_address: c.address.clone(),
            shipping_city: c.city.clone(),
            expected_delivery_date: Some(
                order_date + Duration::days(i64::try_from(rng.range(3, 14)).unwrap_or(3)),
            ),
            notes: (i % 3 == 0).then(|| "Urgent, please prioritize".to_owned()),
            ..Order::default()
        };
        let mut used = Vec::new();
        for j in 1..=rng.range(1, 6) {
            let mut idx = rng.next(products.len());
            while used.contains(&idx) {
                idx = (idx + 1) % products.len();
            }
            used.push(idx);
            let p = &products[idx];
            order.items.push(OrderItem {
                line_number: i32::try_from(j).unwrap_or(1),
                product_id: p.base.id,
                product_code: p.code.clone(),
                product_name: p.name.clone(),
                unit: p.unit.clone(),
                quantity: Decimal::from(rng.range(1, 10)),
                unit_price: p.price,
                discount_percent: Decimal::from(rng.next(3) * 5),
                ..OrderItem::default()
            });
        }
        order.calculate_totals();
        order.paid_amount = match payment_status {
            PaymentStatus::Paid => order.total_amount,
            PaymentStatus::PartialPaid => (order.total_amount * Decimal::new(5, 1)).round_dp(0),
            _ => Decimal::ZERO,
        };
        services.orders.create_order(order).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcg_is_deterministic_and_in_range() {
        let mut a = Lcg(42);
        let mut b = Lcg(42);
        for _ in 0..100 {
            let x = a.range(3, 14);
            assert_eq!(x, b.range(3, 14));
            assert!((3..14).contains(&x));
        }
        assert_eq!(Lcg(1).next(0), 0);
    }
}
