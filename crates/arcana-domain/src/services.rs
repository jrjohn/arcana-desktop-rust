//! Domain service contracts (C# `Arcana.Domain.Services`). Implementations live in
//! `arcana-infra` (milestone 3).
//!
//! C# returns a bare `bool` / `string` from `CustomerCodeExistsAsync`, `GenerateOrderNumberAsync`
//! etc.; here they return `AppResult` so storage failures are not swallowed.

use arcana_core::{AppResult, PageRequest, PagedResult, Timestamp};
use async_trait::async_trait;

use crate::entities::{Customer, Order, OrderStatus, Product, ProductCategory};

/// C# `CustomerService`.
#[async_trait]
pub trait CustomerService: Send + Sync {
    async fn get_customers(&self, request: &PageRequest) -> AppResult<PagedResult<Customer>>;
    async fn get_customer_by_id(&self, id: i32) -> AppResult<Customer>;
    async fn get_customer_by_code(&self, code: &str) -> AppResult<Customer>;
    async fn search_customers(
        &self,
        search_term: &str,
        max_results: u32,
    ) -> AppResult<Vec<Customer>>;
    async fn create_customer(&self, customer: Customer) -> AppResult<Customer>;
    async fn update_customer(&self, customer: Customer) -> AppResult<Customer>;
    async fn delete_customer(&self, id: i32) -> AppResult<()>;
    async fn customer_code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool>;
}

/// C# `ProductService`.
#[async_trait]
pub trait ProductService: Send + Sync {
    async fn get_products(&self, request: &PageRequest) -> AppResult<PagedResult<Product>>;
    async fn get_product_by_id(&self, id: i32) -> AppResult<Product>;
    async fn get_product_by_code(&self, code: &str) -> AppResult<Product>;
    async fn search_products(&self, search_term: &str, max_results: u32)
    -> AppResult<Vec<Product>>;
    async fn get_products_by_category(&self, category_id: i32) -> AppResult<Vec<Product>>;
    async fn create_product(&self, product: Product) -> AppResult<Product>;
    async fn update_product(&self, product: Product) -> AppResult<Product>;
    async fn delete_product(&self, id: i32) -> AppResult<()>;
    async fn get_categories(&self) -> AppResult<Vec<ProductCategory>>;
}

/// C# `OrderService`.
#[async_trait]
pub trait OrderService: Send + Sync {
    async fn get_orders(&self, request: &PageRequest) -> AppResult<PagedResult<Order>>;
    async fn get_order_by_id(&self, id: i32) -> AppResult<Order>;
    async fn get_order_by_number(&self, order_number: &str) -> AppResult<Order>;
    async fn create_order(&self, order: Order) -> AppResult<Order>;
    async fn update_order(&self, order: Order) -> AppResult<Order>;
    async fn delete_order(&self, id: i32) -> AppResult<()>;
    async fn change_status(&self, id: i32, new_status: OrderStatus) -> AppResult<Order>;
    async fn generate_order_number(&self) -> AppResult<String>;
    async fn get_orders_by_customer(&self, customer_id: i32) -> AppResult<Vec<Order>>;
    async fn get_orders_by_date_range(
        &self,
        from: Timestamp,
        to: Timestamp,
    ) -> AppResult<Vec<Order>>;
    async fn get_orders_by_status(&self, status: OrderStatus) -> AppResult<Vec<Order>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contracts must stay usable as trait objects (milestone 3 injects them that way).
    #[test]
    fn contracts_are_dyn_compatible() {
        fn accept(
            _: Option<&dyn CustomerService>,
            _: Option<&dyn ProductService>,
            _: Option<&dyn OrderService>,
        ) -> bool {
            true
        }
        assert!(accept(None, None, None));
    }
}
