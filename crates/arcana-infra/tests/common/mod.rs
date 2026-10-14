//! Test doubles standing in for the C# Moq mocks: an in-memory repository whose methods can
//! be told to fail, and a validator with scripted failures.

#![allow(dead_code, reason = "each test binary uses a different subset")]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use arcana_core::{
    AppError, AppResult, ErrorCode, PageRequest, PagedResult, Predicate, Repository, Timestamp,
};
use arcana_data::{CustomerRepository, OrderFilter, OrderRepository, ProductRepository};
use arcana_domain::{
    Customer, Order, OrderStatus, Product, ProductCategory, ValidationFailure, ValidationResult,
    Validator,
};
use async_trait::async_trait;

pub trait Keyed: Clone + Send + Sync + 'static {
    fn key(&self) -> i32;
}

impl Keyed for Customer {
    fn key(&self) -> i32 {
        self.base.id
    }
}
impl Keyed for Product {
    fn key(&self) -> i32 {
        self.base.id
    }
}
impl Keyed for Order {
    fn key(&self) -> i32 {
        self.base.id
    }
}

/// In-memory repository (the Moq `Mock<XRepository>`).
pub struct Fake<T> {
    pub items: Mutex<Vec<T>>,
    failing: Mutex<HashSet<&'static str>>,
    pub code_exists: Mutex<bool>,
    pub code_exists_args: Mutex<Vec<(String, Option<i32>)>>,
    pub next_number: Mutex<String>,
    pub categories: Mutex<Vec<ProductCategory>>,
    pub calls: Mutex<Vec<&'static str>>,
}

impl<T> Default for Fake<T> {
    fn default() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            failing: Mutex::new(HashSet::new()),
            code_exists: Mutex::new(false),
            code_exists_args: Mutex::new(Vec::new()),
            next_number: Mutex::new(String::new()),
            categories: Mutex::new(Vec::new()),
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl<T: Keyed> Fake<T> {
    pub fn with(items: Vec<T>) -> Arc<Self> {
        let f = Self::default();
        *f.items.lock().unwrap() = items;
        Arc::new(f)
    }

    /// Make `method` return `Err("db error")` (Moq `.ThrowsAsync(new Exception("db error"))`).
    pub fn fail(&self, method: &'static str) {
        self.failing.lock().unwrap().insert(method);
    }

    fn check(&self, method: &'static str) -> AppResult<()> {
        self.calls.lock().unwrap().push(method);
        if self.failing.lock().unwrap().contains(method) {
            Err(AppError::data(ErrorCode::DatabaseError, "db error"))
        } else {
            Ok(())
        }
    }

    fn all(&self) -> Vec<T> {
        self.items.lock().unwrap().clone()
    }

    pub fn called(&self, method: &'static str) -> bool {
        self.calls.lock().unwrap().contains(&method)
    }
}

#[async_trait]
impl<T: Keyed> Repository<T> for Fake<T> {
    async fn get_by_id(&self, id: i32) -> AppResult<Option<T>> {
        self.check("get_by_id")?;
        Ok(self.all().into_iter().find(|e| e.key() == id))
    }
    async fn get_all(&self) -> AppResult<Vec<T>> {
        self.check("get_all")?;
        Ok(self.all())
    }
    async fn get_paged(&self, request: &PageRequest) -> AppResult<PagedResult<T>> {
        self.check("get_paged")?;
        let all = self.all();
        let n = all.len() as u64;
        Ok(PagedResult::new(all, request.page, request.page_size, n))
    }
    async fn find(&self, predicate: Predicate<'_, T>) -> AppResult<Vec<T>> {
        self.check("find")?;
        Ok(self.all().into_iter().filter(|e| predicate(e)).collect())
    }
    async fn first_or_default(&self, predicate: Predicate<'_, T>) -> AppResult<Option<T>> {
        self.check("first_or_default")?;
        Ok(self.all().into_iter().find(|e| predicate(e)))
    }
    async fn any(&self, predicate: Predicate<'_, T>) -> AppResult<bool> {
        self.check("any")?;
        Ok(self.all().iter().any(predicate))
    }
    async fn count(&self, predicate: Option<Predicate<'_, T>>) -> AppResult<u64> {
        self.check("count")?;
        Ok(self
            .all()
            .iter()
            .filter(|e| predicate.is_none_or(|p| p(e)))
            .count() as u64)
    }
    async fn add(&self, entity: T) -> AppResult<T> {
        self.check("add")?;
        self.items.lock().unwrap().push(entity.clone());
        Ok(entity)
    }
    async fn add_range(&self, entities: Vec<T>) -> AppResult<()> {
        self.check("add_range")?;
        self.items.lock().unwrap().extend(entities);
        Ok(())
    }
    async fn update(&self, entity: &mut T) -> AppResult<()> {
        self.check("update")?;
        let mut items = self.items.lock().unwrap();
        if let Some(slot) = items.iter_mut().find(|e| e.key() == entity.key()) {
            *slot = entity.clone();
        }
        Ok(())
    }
    async fn delete(&self, entity: &mut T) -> AppResult<()> {
        self.check("delete")?;
        self.items
            .lock()
            .unwrap()
            .retain(|e| e.key() != entity.key());
        Ok(())
    }
    async fn delete_by_id(&self, id: i32) -> AppResult<()> {
        self.check("delete_by_id")?;
        self.items.lock().unwrap().retain(|e| e.key() != id);
        Ok(())
    }
}

impl<T: Keyed> Fake<T> {
    fn record_code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        self.check("code_exists")?;
        self.code_exists_args
            .lock()
            .unwrap()
            .push((code.to_owned(), exclude_id));
        Ok(*self.code_exists.lock().unwrap())
    }
}

#[async_trait]
impl CustomerRepository for Fake<Customer> {
    async fn get_by_code(&self, code: &str) -> AppResult<Option<Customer>> {
        self.check("get_by_code")?;
        Ok(self.all().into_iter().find(|c| c.code == code))
    }
    async fn search(&self, _search_term: &str, max_results: u32) -> AppResult<Vec<Customer>> {
        self.check("search")?;
        Ok(self.all().into_iter().take(max_results as usize).collect())
    }
    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        self.record_code_exists(code, exclude_id)
    }
}

#[async_trait]
impl ProductRepository for Fake<Product> {
    async fn get_by_code(&self, code: &str) -> AppResult<Option<Product>> {
        self.check("get_by_code")?;
        Ok(self.all().into_iter().find(|p| p.code == code))
    }
    async fn get_by_barcode(&self, barcode: &str) -> AppResult<Option<Product>> {
        self.check("get_by_barcode")?;
        Ok(self
            .all()
            .into_iter()
            .find(|p| p.barcode.as_deref() == Some(barcode)))
    }
    async fn search(&self, _search_term: &str, max_results: u32) -> AppResult<Vec<Product>> {
        self.check("search")?;
        Ok(self.all().into_iter().take(max_results as usize).collect())
    }
    async fn get_by_category(&self, category_id: i32) -> AppResult<Vec<Product>> {
        self.check("get_by_category")?;
        Ok(self
            .all()
            .into_iter()
            .filter(|p| p.category_id == Some(category_id))
            .collect())
    }
    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        self.record_code_exists(code, exclude_id)
    }
    async fn get_categories(&self) -> AppResult<Vec<ProductCategory>> {
        self.check("get_categories")?;
        Ok(self.categories.lock().unwrap().clone())
    }
}

#[async_trait]
impl OrderRepository for Fake<Order> {
    async fn get_by_id_with_items(&self, id: i32) -> AppResult<Option<Order>> {
        self.check("get_by_id_with_items")?;
        Ok(self.all().into_iter().find(|o| o.base.id == id))
    }
    async fn get_by_order_number(&self, order_number: &str) -> AppResult<Option<Order>> {
        self.check("get_by_order_number")?;
        Ok(self
            .all()
            .into_iter()
            .find(|o| o.order_number == order_number))
    }
    async fn get_by_customer_id(&self, customer_id: i32) -> AppResult<Vec<Order>> {
        self.check("get_by_customer_id")?;
        Ok(self
            .all()
            .into_iter()
            .filter(|o| o.customer_id == customer_id)
            .collect())
    }
    async fn get_by_date_range(&self, from: Timestamp, to: Timestamp) -> AppResult<Vec<Order>> {
        self.check("get_by_date_range")?;
        Ok(self
            .all()
            .into_iter()
            .filter(|o| o.order_date >= from && o.order_date <= to)
            .collect())
    }
    async fn get_by_status(&self, status: OrderStatus) -> AppResult<Vec<Order>> {
        self.check("get_by_status")?;
        Ok(self
            .all()
            .into_iter()
            .filter(|o| o.status == status)
            .collect())
    }
    async fn generate_order_number(&self) -> AppResult<String> {
        self.check("generate_order_number")?;
        Ok(self.next_number.lock().unwrap().clone())
    }
    async fn get_paged_with_filters(
        &self,
        request: &PageRequest,
        _filter: &OrderFilter,
    ) -> AppResult<PagedResult<Order>> {
        self.get_paged(request).await
    }
}

/// Validator returning scripted failures (Moq `Mock<IValidator<T>>`); passes by default.
#[derive(Default)]
pub struct FakeValidator {
    failures: Mutex<Vec<(String, String)>>,
}

impl FakeValidator {
    pub fn failing(property: &str, message: &str) -> Arc<Self> {
        let v = Self::default();
        v.failures
            .lock()
            .unwrap()
            .push((property.into(), message.into()));
        Arc::new(v)
    }
}

impl<T> Validator<T> for FakeValidator {
    fn validate(&self, _item: &T) -> ValidationResult {
        ValidationResult {
            errors: self
                .failures
                .lock()
                .unwrap()
                .iter()
                .map(|(p, m)| ValidationFailure {
                    property_name: p.clone(),
                    error_message: m.clone(),
                })
                .collect(),
        }
    }
}
