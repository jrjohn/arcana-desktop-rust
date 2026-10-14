//! The order list query and its in-memory evaluation.
//!
//! Without filters the database pages and sorts (`OrderService::get_orders`). With filters
//! the candidate set comes from the narrowest service query (date range, else status, else
//! all) and is filtered, sorted and paged here — `OrderService` has no combined filter call.

use arcana_core::{PageRequest, PagedResult};
use arcana_domain::{Order, OrderStatus};
use chrono::NaiveDate;

/// Sortable columns (header click).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    OrderNumber,
    OrderDate,
    Customer,
    Amount,
    PaymentStatus,
    Status,
}

impl SortColumn {
    /// Database column for server-side sorting.
    #[must_use]
    pub fn column(self) -> &'static str {
        match self {
            Self::OrderNumber => "OrderNumber",
            Self::OrderDate => "OrderDate",
            Self::Customer => "CustomerName",
            Self::Amount => "TotalAmount",
            Self::PaymentStatus => "PaymentStatus",
            Self::Status => "Status",
        }
    }
}

/// Everything the list asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderQuery {
    pub page: u32,
    pub page_size: u32,
    pub search: String,
    pub status: Option<OrderStatus>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub sort: SortColumn,
    pub descending: bool,
}

impl OrderQuery {
    /// No filter at all → the database can page.
    #[must_use]
    pub fn is_unfiltered(&self) -> bool {
        self.search.is_empty() && self.status.is_none() && self.from.is_none() && self.to.is_none()
    }

    /// Server-side page request.
    #[must_use]
    pub fn page_request(&self) -> PageRequest {
        let mut r = PageRequest::new(self.page, self.page_size);
        r.sort_by = Some(self.sort.column().to_owned());
        r.descending = self.descending;
        r
    }

    fn matches(&self, o: &Order) -> bool {
        let q = self.search.to_lowercase();
        let text_ok = q.is_empty()
            || o.order_number.to_lowercase().contains(&q)
            || o.customer_name.to_lowercase().contains(&q);
        let day = o.order_date.date();
        text_ok
            && self.status.is_none_or(|s| o.status == s)
            && self.from.is_none_or(|f| day >= f)
            && self.to.is_none_or(|t| day <= t)
    }

    /// Filter, sort and page `candidates`.
    #[must_use]
    pub fn apply(&self, candidates: Vec<Order>) -> PagedResult<Order> {
        let mut rows: Vec<Order> = candidates.into_iter().filter(|o| self.matches(o)).collect();
        rows.sort_by(|a, b| {
            let ord = match self.sort {
                SortColumn::OrderNumber => a.order_number.cmp(&b.order_number),
                SortColumn::OrderDate => a.order_date.cmp(&b.order_date),
                SortColumn::Customer => a.customer_name.cmp(&b.customer_name),
                SortColumn::Amount => a.total_amount.cmp(&b.total_amount),
                SortColumn::PaymentStatus => {
                    a.payment_status.value().cmp(&b.payment_status.value())
                }
                SortColumn::Status => a.status.value().cmp(&b.status.value()),
            };
            if self.descending { ord.reverse() } else { ord }
        });
        let total = rows.len() as u64;
        let skip = usize::try_from(self.page_request().skip()).unwrap_or(usize::MAX);
        let items = rows
            .into_iter()
            .skip(skip)
            .take(self.page_size as usize)
            .collect();
        PagedResult::new(items, self.page, self.page_size, total)
    }
}
