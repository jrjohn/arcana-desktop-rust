//! Business entities: `Customer`, `Product`, `ProductCategory`, `Order`, `OrderItem`.
//!
//! Navigation properties are kept (as owned, normally-empty collections / boxed options) so a
//! repository can return an aggregate the same way an EF `Include` does.

use arcana_core::{Timestamp, int_enum, today};
use rust_decimal::Decimal;

use super::base::{BaseEntity, impl_base_entity};

/// C# `Customer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer {
    pub base: BaseEntity,
    pub code: String,
    pub name: String,
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub tax_id: Option<String>,
    pub credit_limit: Decimal,
    pub balance: Decimal,
    pub is_active: bool,
    pub notes: Option<String>,
    pub orders: Vec<Order>,
}

impl Default for Customer {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            code: String::new(),
            name: String::new(),
            contact_name: None,
            phone: None,
            email: None,
            address: None,
            city: None,
            state: None,
            postal_code: None,
            country: None,
            tax_id: None,
            credit_limit: Decimal::ZERO,
            balance: Decimal::ZERO,
            is_active: true,
            notes: None,
            orders: Vec::new(),
        }
    }
}

/// C# `Product`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub base: BaseEntity,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Option<i32>,
    pub unit: String,
    pub price: Decimal,
    pub cost: Decimal,
    pub stock_quantity: Decimal,
    pub min_stock_level: Decimal,
    pub max_stock_level: Decimal,
    pub is_active: bool,
    pub barcode: Option<String>,
    pub weight: Option<Decimal>,
    pub category: Option<Box<ProductCategory>>,
}

impl Default for Product {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            code: String::new(),
            name: String::new(),
            description: None,
            category_id: None,
            unit: "PCS".to_owned(),
            price: Decimal::ZERO,
            cost: Decimal::ZERO,
            stock_quantity: Decimal::ZERO,
            min_stock_level: Decimal::ZERO,
            max_stock_level: Decimal::ZERO,
            is_active: true,
            barcode: None,
            weight: None,
            category: None,
        }
    }
}

/// C# `ProductCategory` (self-referencing tree).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductCategory {
    pub base: BaseEntity,
    pub code: String,
    pub name: String,
    pub parent_id: Option<i32>,
    pub sort_order: i32,
    pub parent: Option<Box<ProductCategory>>,
    pub children: Vec<ProductCategory>,
    pub products: Vec<Product>,
}

int_enum! {
    /// C# `OrderStatus`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub enum OrderStatus {
        #[default]
        Draft = 0,
        Pending = 1,
        Confirmed = 2,
        Processing = 3,
        Shipped = 4,
        Delivered = 5,
        Completed = 6,
        Cancelled = 7,
    }
}

int_enum! {
    /// C# `PaymentStatus`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub enum PaymentStatus {
        #[default]
        Unpaid = 0,
        PartialPaid = 1,
        Paid = 2,
        Refunded = 3,
    }
}

int_enum! {
    /// C# `PaymentMethod`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub enum PaymentMethod {
        #[default]
        Cash = 0,
        CreditCard = 1,
        BankTransfer = 2,
        Check = 3,
        Credit = 4,
    }
}

/// C# `Order` aggregate root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub base: BaseEntity,
    pub order_number: String,
    pub order_date: Timestamp,
    pub customer_id: i32,
    pub customer_name: String,
    pub status: OrderStatus,
    pub payment_status: PaymentStatus,
    pub payment_method: PaymentMethod,
    pub subtotal: Decimal,
    pub tax_rate: Decimal,
    pub tax_amount: Decimal,
    pub discount_amount: Decimal,
    pub shipping_cost: Decimal,
    pub total_amount: Decimal,
    pub paid_amount: Decimal,
    pub shipping_address: Option<String>,
    pub shipping_city: Option<String>,
    pub shipping_postal_code: Option<String>,
    pub expected_delivery_date: Option<Timestamp>,
    pub actual_delivery_date: Option<Timestamp>,
    pub notes: Option<String>,
    pub customer: Option<Box<Customer>>,
    pub items: Vec<OrderItem>,
}

impl Default for Order {
    /// `OrderDate = DateTime.Today`, `TaxRate = 5`, enums at their first value.
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            order_number: String::new(),
            order_date: today(),
            customer_id: 0,
            customer_name: String::new(),
            status: OrderStatus::default(),
            payment_status: PaymentStatus::default(),
            payment_method: PaymentMethod::default(),
            subtotal: Decimal::ZERO,
            tax_rate: Decimal::from(5),
            tax_amount: Decimal::ZERO,
            discount_amount: Decimal::ZERO,
            shipping_cost: Decimal::ZERO,
            total_amount: Decimal::ZERO,
            paid_amount: Decimal::ZERO,
            shipping_address: None,
            shipping_city: None,
            shipping_postal_code: None,
            expected_delivery_date: None,
            actual_delivery_date: None,
            notes: None,
            customer: None,
            items: Vec::new(),
        }
    }
}

impl Order {
    /// Recompute `subtotal`, `tax_amount`, `total_amount` from the items (C# `CalculateTotals`).
    ///
    /// Tax is rounded to 2 places with banker's rounding, matching C# `Math.Round(decimal, 2)`.
    pub fn calculate_totals(&mut self) {
        self.subtotal = self.items.iter().map(OrderItem::line_total).sum();
        self.tax_amount = (self.subtotal * self.tax_rate / Decimal::ONE_HUNDRED).round_dp(2);
        self.total_amount =
            self.subtotal + self.tax_amount + self.shipping_cost - self.discount_amount;
    }
}

/// C# `OrderItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderItem {
    pub base: BaseEntity,
    pub order_id: i32,
    pub line_number: i32,
    pub product_id: i32,
    pub product_code: String,
    pub product_name: String,
    pub unit: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub discount_percent: Decimal,
    pub notes: Option<String>,
    pub product: Option<Box<Product>>,
}

impl Default for OrderItem {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            order_id: 0,
            line_number: 0,
            product_id: 0,
            product_code: String::new(),
            product_name: String::new(),
            unit: "PCS".to_owned(),
            quantity: Decimal::ZERO,
            unit_price: Decimal::ZERO,
            discount_percent: Decimal::ZERO,
            notes: None,
            product: None,
        }
    }
}

impl OrderItem {
    /// `round(quantity * unit_price * (1 - discount_percent / 100), 2)` (C# computed
    /// `LineTotal`; not persisted).
    #[must_use]
    pub fn line_total(&self) -> Decimal {
        (self.quantity
            * self.unit_price
            * (Decimal::ONE - self.discount_percent / Decimal::ONE_HUNDRED))
            .round_dp(2)
    }
}

impl_base_entity!(Customer, Product, ProductCategory, Order, OrderItem);
