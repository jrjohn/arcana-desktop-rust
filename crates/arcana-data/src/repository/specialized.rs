//! Specialised repositories (C# `CustomerRepository`, `ProductRepository`, `OrderRepository`
//! and their `Impl`s): the generic repository plus DAO-backed finders.

use std::sync::Arc;

use arcana_core::{AppResult, PageRequest, PagedResult, Repository, Timestamp};
use arcana_domain::{Customer, Order, OrderItem, OrderStatus, Product, ProductCategory};
use async_trait::async_trait;

use super::generic::{
    SqliteRepository, begin_implicit, delegate_repository, finish_implicit, insert_on, update_on,
};
use crate::dao::{
    CustomerDao, CustomerDaoImpl, OrderDao, OrderDaoImpl, OrderFilter, ProductDao, ProductDaoImpl,
};
use crate::db::Db;

/// C# `CustomerRepository`.
#[async_trait]
pub trait CustomerRepository: Repository<Customer> {
    async fn get_by_code(&self, code: &str) -> AppResult<Option<Customer>>;
    async fn search(&self, search_term: &str, max_results: u32) -> AppResult<Vec<Customer>>;
    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool>;
}

/// C# `CustomerRepositoryImpl`.
#[derive(Clone)]
pub struct CustomerRepositoryImpl {
    base: SqliteRepository<Customer>,
    dao: Arc<dyn CustomerDao>,
}

impl CustomerRepositoryImpl {
    #[must_use]
    pub fn new(db: Db, dao: Arc<dyn CustomerDao>) -> Self {
        Self {
            base: SqliteRepository::new(db),
            dao,
        }
    }

    /// Wire the default DAO on the same database.
    #[must_use]
    pub fn with_db(db: Db) -> Self {
        Self::new(db.clone(), Arc::new(CustomerDaoImpl::new(db)))
    }

    async fn add_impl(&self, e: Customer) -> AppResult<Customer> {
        self.base.add(e).await
    }
    async fn add_range_impl(&self, e: Vec<Customer>) -> AppResult<()> {
        self.base.add_range(e).await
    }
    async fn update_impl(&self, e: &mut Customer) -> AppResult<()> {
        self.base.update(e).await
    }
}

delegate_repository!(CustomerRepositoryImpl, Customer);

#[async_trait]
impl CustomerRepository for CustomerRepositoryImpl {
    async fn get_by_code(&self, code: &str) -> AppResult<Option<Customer>> {
        self.dao.find_by_code(code).await
    }
    async fn search(&self, search_term: &str, max_results: u32) -> AppResult<Vec<Customer>> {
        self.dao.search(search_term, max_results).await
    }
    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        self.dao.code_exists(code, exclude_id).await
    }
}

/// C# `ProductRepository`.
#[async_trait]
pub trait ProductRepository: Repository<Product> {
    async fn get_by_code(&self, code: &str) -> AppResult<Option<Product>>;
    async fn get_by_barcode(&self, barcode: &str) -> AppResult<Option<Product>>;
    async fn search(&self, search_term: &str, max_results: u32) -> AppResult<Vec<Product>>;
    async fn get_by_category(&self, category_id: i32) -> AppResult<Vec<Product>>;
    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool>;
    async fn get_categories(&self) -> AppResult<Vec<ProductCategory>>;
}

/// C# `ProductRepositoryImpl`.
#[derive(Clone)]
pub struct ProductRepositoryImpl {
    base: SqliteRepository<Product>,
    dao: Arc<dyn ProductDao>,
}

impl ProductRepositoryImpl {
    #[must_use]
    pub fn new(db: Db, dao: Arc<dyn ProductDao>) -> Self {
        Self {
            base: SqliteRepository::new(db),
            dao,
        }
    }

    /// Wire the default DAO on the same database.
    #[must_use]
    pub fn with_db(db: Db) -> Self {
        Self::new(db.clone(), Arc::new(ProductDaoImpl::new(db)))
    }

    async fn add_impl(&self, e: Product) -> AppResult<Product> {
        self.base.add(e).await
    }
    async fn add_range_impl(&self, e: Vec<Product>) -> AppResult<()> {
        self.base.add_range(e).await
    }
    async fn update_impl(&self, e: &mut Product) -> AppResult<()> {
        self.base.update(e).await
    }
}

delegate_repository!(ProductRepositoryImpl, Product);

#[async_trait]
impl ProductRepository for ProductRepositoryImpl {
    async fn get_by_code(&self, code: &str) -> AppResult<Option<Product>> {
        self.dao.find_by_code(code).await
    }
    async fn get_by_barcode(&self, barcode: &str) -> AppResult<Option<Product>> {
        self.dao.find_by_barcode(barcode).await
    }
    async fn search(&self, search_term: &str, max_results: u32) -> AppResult<Vec<Product>> {
        self.dao.search(search_term, max_results).await
    }
    async fn get_by_category(&self, category_id: i32) -> AppResult<Vec<Product>> {
        self.dao.find_by_category(category_id).await
    }
    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        self.dao.code_exists(code, exclude_id).await
    }
    async fn get_categories(&self) -> AppResult<Vec<ProductCategory>> {
        self.dao.find_all_categories().await
    }
}

/// C# `OrderRepository`.
#[async_trait]
pub trait OrderRepository: Repository<Order> {
    async fn get_by_id_with_items(&self, id: i32) -> AppResult<Option<Order>>;
    async fn get_by_order_number(&self, order_number: &str) -> AppResult<Option<Order>>;
    async fn get_by_customer_id(&self, customer_id: i32) -> AppResult<Vec<Order>>;
    async fn get_by_date_range(&self, from: Timestamp, to: Timestamp) -> AppResult<Vec<Order>>;
    async fn get_by_status(&self, status: OrderStatus) -> AppResult<Vec<Order>>;
    async fn generate_order_number(&self) -> AppResult<String>;
    async fn get_paged_with_filters(
        &self,
        request: &PageRequest,
        filter: &OrderFilter,
    ) -> AppResult<PagedResult<Order>>;
}

/// C# `OrderRepositoryImpl`.
///
/// EF Core persists the whole aggregate on `Add`/`Update` (the `Items` graph). Without an ORM
/// that is done explicitly here: `add` inserts the items; `update` inserts new items (`Id == 0`)
/// and updates existing ones. Like EF, items removed from the collection are not deleted.
#[derive(Clone)]
pub struct OrderRepositoryImpl {
    base: SqliteRepository<Order>,
    dao: Arc<dyn OrderDao>,
}

impl OrderRepositoryImpl {
    #[must_use]
    pub fn new(db: Db, dao: Arc<dyn OrderDao>) -> Self {
        Self {
            base: SqliteRepository::new(db),
            dao,
        }
    }

    /// Wire the default DAO on the same database.
    #[must_use]
    pub fn with_db(db: Db) -> Self {
        Self::new(db.clone(), Arc::new(OrderDaoImpl::new(db)))
    }

    async fn save_items(
        db: &Db,
        conn: &mut sqlx::SqliteConnection,
        order_id: i32,
        items: &mut [OrderItem],
    ) -> AppResult<()> {
        for item in items {
            item.order_id = order_id;
            if item.base.id == 0 {
                insert_on(db, conn, item).await?;
            } else {
                update_on(db, conn, item).await?;
            }
        }
        Ok(())
    }

    async fn add_impl(&self, mut order: Order) -> AppResult<Order> {
        let db = self.base.db();
        let mut conn = db.lock().await?;
        let started = begin_implicit(db, &mut conn).await?;
        let result = async {
            insert_on(db, &mut conn, &mut order).await?;
            Self::save_items(db, &mut conn, order.base.id, &mut order.items).await
        }
        .await;
        finish_implicit(&mut conn, started, result).await?;
        Ok(order)
    }

    async fn add_range_impl(&self, orders: Vec<Order>) -> AppResult<()> {
        let db = self.base.db();
        let mut conn = db.lock().await?;
        let started = begin_implicit(db, &mut conn).await?;
        let result = async {
            for mut order in orders {
                insert_on(db, &mut conn, &mut order).await?;
                Self::save_items(db, &mut conn, order.base.id, &mut order.items).await?;
            }
            Ok(())
        }
        .await;
        finish_implicit(&mut conn, started, result).await
    }

    async fn update_impl(&self, order: &mut Order) -> AppResult<()> {
        let db = self.base.db();
        let mut conn = db.lock().await?;
        let started = begin_implicit(db, &mut conn).await?;
        let result = async {
            update_on(db, &mut conn, order).await?;
            Self::save_items(db, &mut conn, order.base.id, &mut order.items).await
        }
        .await;
        finish_implicit(&mut conn, started, result).await
    }
}

delegate_repository!(OrderRepositoryImpl, Order);

#[async_trait]
impl OrderRepository for OrderRepositoryImpl {
    async fn get_by_id_with_items(&self, id: i32) -> AppResult<Option<Order>> {
        self.dao.find_by_id_with_items(id).await
    }
    async fn get_by_order_number(&self, order_number: &str) -> AppResult<Option<Order>> {
        self.dao.find_by_order_number(order_number).await
    }
    async fn get_by_customer_id(&self, customer_id: i32) -> AppResult<Vec<Order>> {
        self.dao.find_by_customer_id(customer_id).await
    }
    async fn get_by_date_range(&self, from: Timestamp, to: Timestamp) -> AppResult<Vec<Order>> {
        self.dao.find_by_date_range(from, to).await
    }
    async fn get_by_status(&self, status: OrderStatus) -> AppResult<Vec<Order>> {
        self.dao.find_by_status(status).await
    }
    async fn generate_order_number(&self) -> AppResult<String> {
        self.dao.generate_order_number().await
    }
    async fn get_paged_with_filters(
        &self,
        request: &PageRequest,
        filter: &OrderFilter,
    ) -> AppResult<PagedResult<Order>> {
        self.dao.find_paged_with_filters(request, filter).await
    }
}
