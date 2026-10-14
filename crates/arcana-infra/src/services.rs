//! Domain service implementations (C# `Arcana.Infrastructure.Services.Impl`).
//!
//! The C# methods wrap every repository call in `try/catch` and turn exceptions into
//! `AppError.Data(QueryFailed | DatabaseError, "Failed to …", ex)`. Repository errors arrive
//! here as `Err` and are mapped the same way: reads → `QueryFailed`, writes → `DatabaseError`,
//! the original message kept as `inner`. Validation, not-found and duplicate-code results are
//! returned unchanged.

use std::sync::Arc;

use arcana_core::{AppError, AppResult, ErrorCode, PageRequest, PagedResult, Timestamp};
use arcana_data::{CustomerRepository, OrderRepository, ProductRepository};
use arcana_domain::services::{CustomerService, OrderService, ProductService};
use arcana_domain::{Customer, Order, OrderStatus, Product, ProductCategory, Validator};
use async_trait::async_trait;

/// A validator usable behind an `Arc`.
pub type SharedValidator<T> = Arc<dyn Validator<T> + Send + Sync>;

fn failed(code: ErrorCode, message: &str, e: &AppError) -> AppError {
    tracing::error!(error = %e, "{message}");
    AppError::Data {
        code,
        message: message.to_owned(),
        inner: Some(e.to_string()),
    }
}

fn read<T>(r: AppResult<T>, message: &str) -> AppResult<T> {
    r.map_err(|e| failed(ErrorCode::QueryFailed, message, &e))
}

fn write<T>(r: AppResult<T>, message: &str) -> AppResult<T> {
    r.map_err(|e| failed(ErrorCode::DatabaseError, message, &e))
}

fn not_found(message: String) -> AppError {
    AppError::data(ErrorCode::NotFound, message)
}

fn duplicate(what: &str, code: &str) -> AppError {
    AppError::Validation {
        code: ErrorCode::DuplicateEntry,
        message: format!("{what} code already exists"),
        errors: vec![format!("{what} code {code} already exists")],
    }
}

fn validate<T>(validator: &SharedValidator<T>, item: &T, what: &str) -> AppResult<()> {
    let result = validator.validate(item);
    if result.is_valid() {
        Ok(())
    } else {
        Err(AppError::validation(
            format!("{what} validation failed"),
            result.errors.into_iter().map(|e| e.error_message).collect(),
        ))
    }
}

/// C# `CustomerServiceImpl`.
pub struct CustomerServiceImpl {
    repo: Arc<dyn CustomerRepository>,
    validator: SharedValidator<Customer>,
}

impl CustomerServiceImpl {
    #[must_use]
    pub fn new(repo: Arc<dyn CustomerRepository>, validator: SharedValidator<Customer>) -> Self {
        Self { repo, validator }
    }
}

#[async_trait]
impl CustomerService for CustomerServiceImpl {
    async fn get_customers(&self, request: &PageRequest) -> AppResult<PagedResult<Customer>> {
        read(
            self.repo.get_paged(request).await,
            "Failed to get customers",
        )
    }

    async fn get_customer_by_id(&self, id: i32) -> AppResult<Customer> {
        read(self.repo.get_by_id(id).await, "Failed to get customer")?
            .ok_or_else(|| not_found(format!("Customer with ID {id} not found")))
    }

    async fn get_customer_by_code(&self, code: &str) -> AppResult<Customer> {
        read(self.repo.get_by_code(code).await, "Failed to get customer")?
            .ok_or_else(|| not_found(format!("Customer {code} not found")))
    }

    async fn search_customers(
        &self,
        search_term: &str,
        max_results: u32,
    ) -> AppResult<Vec<Customer>> {
        read(
            self.repo.search(search_term, max_results).await,
            "Failed to search customers",
        )
    }

    async fn create_customer(&self, customer: Customer) -> AppResult<Customer> {
        const MSG: &str = "Failed to create customer";
        if write(self.customer_code_exists(&customer.code, None).await, MSG)? {
            return Err(duplicate("Customer", &customer.code));
        }
        validate(&self.validator, &customer, "Customer")?;
        let code = customer.code.clone();
        let created = write(self.repo.add(customer).await, MSG)?;
        tracing::info!("Created customer {code}");
        Ok(created)
    }

    async fn update_customer(&self, mut customer: Customer) -> AppResult<Customer> {
        const MSG: &str = "Failed to update customer";
        let id = customer.base.id;
        if write(self.repo.get_by_id(id).await, MSG)?.is_none() {
            return Err(not_found(format!("Customer with ID {id} not found")));
        }
        if write(
            self.customer_code_exists(&customer.code, Some(id)).await,
            MSG,
        )? {
            return Err(duplicate("Customer", &customer.code));
        }
        validate(&self.validator, &customer, "Customer")?;
        write(self.repo.update(&mut customer).await, MSG)?;
        tracing::info!("Updated customer {}", customer.code);
        Ok(customer)
    }

    async fn delete_customer(&self, id: i32) -> AppResult<()> {
        const MSG: &str = "Failed to delete customer";
        let mut customer = write(self.repo.get_by_id(id).await, MSG)?
            .ok_or_else(|| not_found(format!("Customer with ID {id} not found")))?;
        write(self.repo.delete(&mut customer).await, MSG)?;
        tracing::info!("Deleted customer {}", customer.code);
        Ok(())
    }

    async fn customer_code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        self.repo.code_exists(code, exclude_id).await
    }
}

/// C# `ProductServiceImpl`.
pub struct ProductServiceImpl {
    repo: Arc<dyn ProductRepository>,
    validator: SharedValidator<Product>,
}

impl ProductServiceImpl {
    #[must_use]
    pub fn new(repo: Arc<dyn ProductRepository>, validator: SharedValidator<Product>) -> Self {
        Self { repo, validator }
    }
}

#[async_trait]
impl ProductService for ProductServiceImpl {
    async fn get_products(&self, request: &PageRequest) -> AppResult<PagedResult<Product>> {
        read(self.repo.get_paged(request).await, "Failed to get products")
    }

    async fn get_product_by_id(&self, id: i32) -> AppResult<Product> {
        read(self.repo.get_by_id(id).await, "Failed to get product")?
            .ok_or_else(|| not_found(format!("Product with ID {id} not found")))
    }

    async fn get_product_by_code(&self, code: &str) -> AppResult<Product> {
        read(self.repo.get_by_code(code).await, "Failed to get product")?
            .ok_or_else(|| not_found(format!("Product {code} not found")))
    }

    async fn search_products(
        &self,
        search_term: &str,
        max_results: u32,
    ) -> AppResult<Vec<Product>> {
        read(
            self.repo.search(search_term, max_results).await,
            "Failed to search products",
        )
    }

    async fn get_products_by_category(&self, category_id: i32) -> AppResult<Vec<Product>> {
        read(
            self.repo.get_by_category(category_id).await,
            "Failed to get products",
        )
    }

    async fn create_product(&self, product: Product) -> AppResult<Product> {
        const MSG: &str = "Failed to create product";
        if write(self.repo.code_exists(&product.code, None).await, MSG)? {
            return Err(duplicate("Product", &product.code));
        }
        validate(&self.validator, &product, "Product")?;
        let code = product.code.clone();
        let created = write(self.repo.add(product).await, MSG)?;
        tracing::info!("Created product {code}");
        Ok(created)
    }

    async fn update_product(&self, mut product: Product) -> AppResult<Product> {
        const MSG: &str = "Failed to update product";
        let id = product.base.id;
        if write(self.repo.get_by_id(id).await, MSG)?.is_none() {
            return Err(not_found(format!("Product with ID {id} not found")));
        }
        if write(self.repo.code_exists(&product.code, Some(id)).await, MSG)? {
            return Err(duplicate("Product", &product.code));
        }
        validate(&self.validator, &product, "Product")?;
        write(self.repo.update(&mut product).await, MSG)?;
        tracing::info!("Updated product {}", product.code);
        Ok(product)
    }

    async fn delete_product(&self, id: i32) -> AppResult<()> {
        const MSG: &str = "Failed to delete product";
        let mut product = write(self.repo.get_by_id(id).await, MSG)?
            .ok_or_else(|| not_found(format!("Product with ID {id} not found")))?;
        write(self.repo.delete(&mut product).await, MSG)?;
        tracing::info!("Deleted product {}", product.code);
        Ok(())
    }

    async fn get_categories(&self) -> AppResult<Vec<ProductCategory>> {
        read(self.repo.get_categories().await, "Failed to get categories")
    }
}

/// C# `OrderServiceImpl`. C# also injects a `CustomerRepository` it never uses; it is not
/// carried over.
pub struct OrderServiceImpl {
    repo: Arc<dyn OrderRepository>,
    validator: SharedValidator<Order>,
}

impl OrderServiceImpl {
    #[must_use]
    pub fn new(repo: Arc<dyn OrderRepository>, validator: SharedValidator<Order>) -> Self {
        Self { repo, validator }
    }

    /// C# `CreateOrderAsync` mutates the caller's order (number, totals, line numbers) before
    /// failing or saving. Rust takes ownership, so this variant exposes that behaviour: the
    /// order is updated in place and the saved copy returned.
    ///
    /// # Errors
    /// As [`OrderService::create_order`].
    pub async fn create_order_in_place(&self, order: &mut Order) -> AppResult<Order> {
        const MSG: &str = "Failed to create order";
        if order.order_number.is_empty() {
            order.order_number = write(self.repo.generate_order_number().await, MSG)?;
        }
        order.calculate_totals();
        validate(&self.validator, order, "Order")?;
        for (n, item) in (1..).zip(order.items.iter_mut()) {
            item.line_number = n;
        }
        let created = write(self.repo.add(order.clone()).await, MSG)?;
        tracing::info!("Created order {}", order.order_number);
        Ok(created)
    }

    /// Load an order for a write operation (`NotFound` when missing).
    async fn load(&self, id: i32, msg: &str) -> AppResult<Order> {
        write(self.repo.get_by_id(id).await, msg)?
            .ok_or_else(|| not_found(format!("Order with ID {id} not found")))
    }
}

#[async_trait]
impl OrderService for OrderServiceImpl {
    async fn get_orders(&self, request: &PageRequest) -> AppResult<PagedResult<Order>> {
        read(self.repo.get_paged(request).await, "Failed to get orders")
    }

    async fn get_order_by_id(&self, id: i32) -> AppResult<Order> {
        read(
            self.repo.get_by_id_with_items(id).await,
            "Failed to get order",
        )?
        .ok_or_else(|| not_found(format!("Order with ID {id} not found")))
    }

    async fn get_order_by_number(&self, order_number: &str) -> AppResult<Order> {
        read(
            self.repo.get_by_order_number(order_number).await,
            "Failed to get order",
        )?
        .ok_or_else(|| not_found(format!("Order {order_number} not found")))
    }

    async fn create_order(&self, mut order: Order) -> AppResult<Order> {
        self.create_order_in_place(&mut order).await
    }

    async fn update_order(&self, mut order: Order) -> AppResult<Order> {
        const MSG: &str = "Failed to update order";
        self.load(order.base.id, MSG).await?;
        order.calculate_totals();
        validate(&self.validator, &order, "Order")?;
        write(self.repo.update(&mut order).await, MSG)?;
        tracing::info!("Updated order {}", order.order_number);
        Ok(order)
    }

    async fn delete_order(&self, id: i32) -> AppResult<()> {
        const MSG: &str = "Failed to delete order";
        let mut order = self.load(id, MSG).await?;
        write(self.repo.delete(&mut order).await, MSG)?;
        tracing::info!("Deleted order {}", order.order_number);
        Ok(())
    }

    async fn change_status(&self, id: i32, new_status: OrderStatus) -> AppResult<Order> {
        const MSG: &str = "Failed to change order status";
        let mut order = self.load(id, MSG).await?;
        order.status = new_status;
        write(self.repo.update(&mut order).await, MSG)?;
        tracing::info!(
            "Changed order {} status to {new_status:?}",
            order.order_number
        );
        Ok(order)
    }

    async fn generate_order_number(&self) -> AppResult<String> {
        self.repo.generate_order_number().await
    }

    async fn get_orders_by_customer(&self, customer_id: i32) -> AppResult<Vec<Order>> {
        read(
            self.repo.get_by_customer_id(customer_id).await,
            "Failed to get orders",
        )
    }

    async fn get_orders_by_date_range(
        &self,
        from: Timestamp,
        to: Timestamp,
    ) -> AppResult<Vec<Order>> {
        read(
            self.repo.get_by_date_range(from, to).await,
            "Failed to get orders",
        )
    }

    async fn get_orders_by_status(&self, status: OrderStatus) -> AppResult<Vec<Order>> {
        read(
            self.repo.get_by_status(status).await,
            "Failed to get orders",
        )
    }
}
