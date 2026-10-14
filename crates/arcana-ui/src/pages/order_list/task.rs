use arcana_core::{AppResult, PageRequest, PagedResult};
use arcana_domain::Order;
use chrono::NaiveTime;

use super::{Input, OrderQuery, Task};
use crate::services::UiServices;

/// Upper bound for the in-memory filter path.
const MAX_SCAN: u32 = 10_000;

async fn query(svc: &UiServices, q: &OrderQuery) -> AppResult<PagedResult<Order>> {
    if q.is_unfiltered() {
        return svc.orders.get_orders(&q.page_request()).await;
    }
    let candidates = if q.from.is_some() || q.to.is_some() {
        let from = q
            .from
            .or(chrono::NaiveDate::from_ymd_opt(1, 1, 1))
            .unwrap_or_default()
            .and_time(NaiveTime::MIN);
        let to =
            q.to.or(chrono::NaiveDate::from_ymd_opt(9999, 12, 31))
                .and_then(|d| d.and_hms_opt(23, 59, 59))
                .unwrap_or_default();
        svc.orders.get_orders_by_date_range(from, to).await?
    } else if let Some(s) = q.status {
        svc.orders.get_orders_by_status(s).await?
    } else {
        svc.orders
            .get_orders(&PageRequest::new(1, MAX_SCAN))
            .await?
            .items
    };
    Ok(q.apply(candidates))
}

/// Execute an order-list task.
pub async fn run(task: Task, svc: UiServices) -> Input {
    let msg = |e: arcana_core::AppError| e.to_user_message();
    match task {
        Task::Query(q) => Input::Loaded(query(&svc, &q).await.map_err(msg)),
        Task::Delete { id, order_number } => Input::Deleted(
            svc.orders
                .delete_order(id)
                .await
                .map(|()| order_number)
                .map_err(msg),
        ),
        Task::ChangeStatus { id, status } => Input::StatusChanged(
            svc.orders
                .change_status(id, status)
                .await
                .map(|_| ())
                .map_err(msg),
        ),
    }
}
