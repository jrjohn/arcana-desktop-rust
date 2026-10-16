//! Data-access objects: the hand-written queries behind the specialised repositories
//! (C# `Arcana.Data.Dao` + `Dao.Impl`).

use std::fmt::Write as _;
use std::sync::Arc;

use arcana_core::{AppError, AppResult, ErrorCode, PageRequest, PagedResult, Timestamp, today};
use arcana_domain::{Customer, Order, OrderItem, OrderStatus, Product, ProductCategory};
use async_trait::async_trait;

use crate::db::Db;
use crate::row::{
    SqlValue, TableEntity, ToSql, fetch, fetch_one, limit_offset, order_by, scalar_i64, to_u64,
};

fn select<T: TableEntity>(where_and_rest: &str) -> String {
    format!(
        "{} WHERE {} AND {where_and_rest}",
        T::select_sql(),
        T::filter_sql()
    )
}

/// Search terms are matched case-insensitively like C# `ToLower().Contains(term)`, which
/// EF translates to `instr(lower(col), term)` (`SQLite` `lower` folds ASCII only).
fn search_term(term: &str) -> String {
    term.to_lowercase()
}

/// C# `CustomerDao`.
#[async_trait]
pub trait CustomerDao: Send + Sync {
    async fn find_by_code(&self, code: &str) -> AppResult<Option<Customer>>;
    async fn search(&self, search_term: &str, max_results: u32) -> AppResult<Vec<Customer>>;
    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool>;
}

/// C# `CustomerDaoImpl`.
#[derive(Clone)]
pub struct CustomerDaoImpl {
    db: Db,
}

impl CustomerDaoImpl {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CustomerDao for CustomerDaoImpl {
    async fn find_by_code(&self, code: &str) -> AppResult<Option<Customer>> {
        let sql = select::<Customer>("\"Code\" = ? LIMIT 1");
        fetch_one(&self.db, &sql, vec![code.to_owned().to_sql()]).await
    }

    async fn search(&self, search_term_raw: &str, max_results: u32) -> AppResult<Vec<Customer>> {
        let limit = SqlValue::Int(i64::from(max_results));
        if search_term_raw.trim().is_empty() {
            let sql = select::<Customer>("\"IsActive\" = 1 ORDER BY \"Name\" LIMIT ?");
            return fetch(&self.db, &sql, vec![limit]).await;
        }
        let term = SqlValue::Text(search_term(search_term_raw));
        let sql = select::<Customer>(
            "\"IsActive\" = 1 AND (instr(lower(\"Code\"), ?) > 0 OR instr(lower(\"Name\"), ?) > 0 \
             OR (\"Phone\" IS NOT NULL AND instr(\"Phone\", ?) > 0)) ORDER BY \"Name\" LIMIT ?",
        );
        fetch(
            &self.db,
            &sql,
            vec![term.clone(), term.clone(), term, limit],
        )
        .await
    }

    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        code_exists::<Customer>(&self.db, code, exclude_id).await
    }
}

async fn code_exists<T: TableEntity>(
    db: &Db,
    code: &str,
    exclude_id: Option<i32>,
) -> AppResult<bool> {
    let mut sql = format!(
        "SELECT COUNT(*) FROM \"{}\" WHERE {} AND \"Code\" = ?",
        T::TABLE,
        T::filter_sql()
    );
    let mut params = vec![code.to_owned().to_sql()];
    if let Some(id) = exclude_id {
        sql.push_str(" AND \"Id\" <> ?");
        params.push(id.to_sql());
    }
    Ok(scalar_i64(db, &sql, params).await? > 0)
}

/// C# `ProductDao`.
#[async_trait]
pub trait ProductDao: Send + Sync {
    async fn find_by_code(&self, code: &str) -> AppResult<Option<Product>>;
    async fn find_by_barcode(&self, barcode: &str) -> AppResult<Option<Product>>;
    async fn search(&self, search_term: &str, max_results: u32) -> AppResult<Vec<Product>>;
    async fn find_by_category(&self, category_id: i32) -> AppResult<Vec<Product>>;
    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool>;
    async fn find_all_categories(&self) -> AppResult<Vec<ProductCategory>>;
}

/// C# `ProductDaoImpl`.
#[derive(Clone)]
pub struct ProductDaoImpl {
    db: Db,
}

impl ProductDaoImpl {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ProductDao for ProductDaoImpl {
    async fn find_by_code(&self, code: &str) -> AppResult<Option<Product>> {
        let sql = select::<Product>("\"Code\" = ? LIMIT 1");
        fetch_one(&self.db, &sql, vec![code.to_owned().to_sql()]).await
    }

    async fn find_by_barcode(&self, barcode: &str) -> AppResult<Option<Product>> {
        let sql = select::<Product>("\"Barcode\" = ? LIMIT 1");
        fetch_one(&self.db, &sql, vec![barcode.to_owned().to_sql()]).await
    }

    async fn search(&self, search_term_raw: &str, max_results: u32) -> AppResult<Vec<Product>> {
        let limit = SqlValue::Int(i64::from(max_results));
        if search_term_raw.trim().is_empty() {
            let sql = select::<Product>("\"IsActive\" = 1 ORDER BY \"Name\" LIMIT ?");
            return fetch(&self.db, &sql, vec![limit]).await;
        }
        let term = SqlValue::Text(search_term(search_term_raw));
        let sql = select::<Product>(
            "\"IsActive\" = 1 AND (instr(lower(\"Code\"), ?) > 0 OR instr(lower(\"Name\"), ?) > 0 \
             OR (\"Barcode\" IS NOT NULL AND instr(\"Barcode\", ?) > 0)) ORDER BY \"Name\" LIMIT ?",
        );
        fetch(
            &self.db,
            &sql,
            vec![term.clone(), term.clone(), term, limit],
        )
        .await
    }

    async fn find_by_category(&self, category_id: i32) -> AppResult<Vec<Product>> {
        let sql = select::<Product>("\"CategoryId\" = ? AND \"IsActive\" = 1 ORDER BY \"Name\"");
        fetch(&self.db, &sql, vec![category_id.to_sql()]).await
    }

    async fn code_exists(&self, code: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        code_exists::<Product>(&self.db, code, exclude_id).await
    }

    async fn find_all_categories(&self) -> AppResult<Vec<ProductCategory>> {
        let sql = select::<ProductCategory>("1 ORDER BY \"SortOrder\", \"Name\"");
        fetch(&self.db, &sql, vec![]).await
    }
}

/// Filters for [`OrderDao::find_paged_with_filters`] (the optional C# parameters).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderFilter {
    pub status: Option<OrderStatus>,
    pub customer_id: Option<i32>,
    pub from_date: Option<Timestamp>,
    pub to_date: Option<Timestamp>,
}

/// C# `OrderDao`.
#[async_trait]
pub trait OrderDao: Send + Sync {
    /// The order with its non-deleted items and its customer.
    async fn find_by_id_with_items(&self, id: i32) -> AppResult<Option<Order>>;
    /// The order with its non-deleted items.
    async fn find_by_order_number(&self, order_number: &str) -> AppResult<Option<Order>>;
    async fn find_by_customer_id(&self, customer_id: i32) -> AppResult<Vec<Order>>;
    async fn find_by_date_range(&self, from: Timestamp, to: Timestamp) -> AppResult<Vec<Order>>;
    async fn find_by_status(&self, status: OrderStatus) -> AppResult<Vec<Order>>;
    /// `ORD{yyyyMMdd}{NNNN}`, one past the highest number used today (deleted orders included).
    async fn generate_order_number(&self) -> AppResult<String>;
    async fn find_paged_with_filters(
        &self,
        request: &PageRequest,
        filter: &OrderFilter,
    ) -> AppResult<PagedResult<Order>>;
}

/// C# `OrderDaoImpl`.
#[derive(Clone)]
pub struct OrderDaoImpl {
    db: Db,
}

impl OrderDaoImpl {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    async fn load_items(&self, order: &mut Order) -> AppResult<()> {
        let sql = select::<OrderItem>("\"OrderId\" = ? ORDER BY \"LineNumber\", \"Id\"");
        order.items = fetch(&self.db, &sql, vec![order.base.id.to_sql()]).await?;
        Ok(())
    }

    async fn by_desc_date(&self, condition: &str, params: Vec<SqlValue>) -> AppResult<Vec<Order>> {
        let sql = select::<Order>(&format!("{condition} ORDER BY \"OrderDate\" DESC"));
        fetch(&self.db, &sql, params).await
    }
}

#[async_trait]
impl OrderDao for OrderDaoImpl {
    async fn find_by_id_with_items(&self, id: i32) -> AppResult<Option<Order>> {
        let sql = select::<Order>("\"Id\" = ?");
        let Some(mut order) = fetch_one::<Order>(&self.db, &sql, vec![id.to_sql()]).await? else {
            return Ok(None);
        };
        self.load_items(&mut order).await?;
        let csql = select::<Customer>("\"Id\" = ?");
        order.customer = fetch_one::<Customer>(&self.db, &csql, vec![order.customer_id.to_sql()])
            .await?
            .map(Box::new);
        Ok(Some(order))
    }

    async fn find_by_order_number(&self, order_number: &str) -> AppResult<Option<Order>> {
        let sql = select::<Order>("\"OrderNumber\" = ? LIMIT 1");
        let found =
            fetch_one::<Order>(&self.db, &sql, vec![order_number.to_owned().to_sql()]).await?;
        match found {
            Some(mut order) => {
                self.load_items(&mut order).await?;
                Ok(Some(order))
            }
            None => Ok(None),
        }
    }

    async fn find_by_customer_id(&self, customer_id: i32) -> AppResult<Vec<Order>> {
        self.by_desc_date("\"CustomerId\" = ?", vec![customer_id.to_sql()])
            .await
    }

    async fn find_by_date_range(&self, from: Timestamp, to: Timestamp) -> AppResult<Vec<Order>> {
        self.by_desc_date(
            "\"OrderDate\" >= ? AND \"OrderDate\" <= ?",
            vec![from.to_sql(), to.to_sql()],
        )
        .await
    }

    async fn find_by_status(&self, status: OrderStatus) -> AppResult<Vec<Order>> {
        self.by_desc_date("\"Status\" = ?", vec![status.to_sql()])
            .await
    }

    async fn generate_order_number(&self) -> AppResult<String> {
        let prefix = format!("ORD{}", today().format("%Y%m%d"));
        // IgnoreQueryFilters in C#: deleted orders still reserve their numbers.
        let sql = format!(
            "{} WHERE substr(\"OrderNumber\", 1, length(?)) = ? ORDER BY \"OrderNumber\" DESC LIMIT 1",
            Order::select_sql()
        );
        let p = SqlValue::Text(prefix.clone());
        let last = fetch_one::<Order>(&self.db, &sql, vec![p.clone(), p]).await?;
        let Some(last) = last else {
            return Ok(format!("{prefix}0001"));
        };
        let suffix = last.order_number.get(prefix.len()..).unwrap_or_default();
        let n: u32 = suffix.parse().map_err(|_| {
            AppError::data(
                ErrorCode::DataCorruption,
                format!(
                    "order number '{}' has a non-numeric suffix",
                    last.order_number
                ),
            )
        })?;
        Ok(format!("{prefix}{:04}", n + 1))
    }

    async fn find_paged_with_filters(
        &self,
        request: &PageRequest,
        filter: &OrderFilter,
    ) -> AppResult<PagedResult<Order>> {
        let mut cond = String::from("1");
        let mut params = Vec::new();
        if let Some(s) = filter.status {
            cond.push_str(" AND \"Status\" = ?");
            params.push(s.to_sql());
        }
        if let Some(c) = filter.customer_id {
            cond.push_str(" AND \"CustomerId\" = ?");
            params.push(c.to_sql());
        }
        if let Some(f) = filter.from_date {
            cond.push_str(" AND \"OrderDate\" >= ?");
            params.push(f.to_sql());
        }
        if let Some(t) = filter.to_date {
            cond.push_str(" AND \"OrderDate\" <= ?");
            params.push(t.to_sql());
        }
        let order = order_by::<Order>(request)?
            .unwrap_or_else(|| " ORDER BY \"OrderDate\" DESC".to_owned());
        let count_sql = format!(
            "SELECT COUNT(*) FROM \"Orders\" WHERE {} AND {cond}",
            Order::filter_sql()
        );
        let total = scalar_i64(&self.db, &count_sql, params.clone()).await?;
        let mut sql = select::<Order>(&cond);
        let _ = write!(sql, "{order} LIMIT ? OFFSET ?");
        params.extend(limit_offset(request));
        let items = fetch(&self.db, &sql, params).await?;
        Ok(PagedResult::new(
            items,
            request.page,
            request.page_size,
            to_u64(total),
        ))
    }
}

/// Default [`CustomerDao`] on `db` (factory: callers depend on the trait, not the impl).
#[must_use]
pub fn customer_dao(db: Db) -> Arc<dyn CustomerDao> {
    Arc::new(CustomerDaoImpl::new(db))
}

/// Default [`ProductDao`] on `db`.
#[must_use]
pub fn product_dao(db: Db) -> Arc<dyn ProductDao> {
    Arc::new(ProductDaoImpl::new(db))
}

/// Default [`OrderDao`] on `db`.
#[must_use]
pub fn order_dao(db: Db) -> Arc<dyn OrderDao> {
    Arc::new(OrderDaoImpl::new(db))
}
