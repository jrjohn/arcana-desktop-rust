use arcana_core::{AppResult, PageRequest, today};
use arcana_domain::OrderStatus;
use chrono::Duration;

use super::{Dashboard, Input, RECENT_COUNT, Task};
use crate::services::UiServices;

async fn load(svc: &UiServices) -> AppResult<Dashboard> {
    let start = today();
    let end = start + Duration::days(1) - Duration::seconds(1);
    let todays = svc.orders.get_orders_by_date_range(start, end).await?;
    let pending = svc
        .orders
        .get_orders_by_status(OrderStatus::Pending)
        .await?;
    let customers = svc.customers.get_customers(&PageRequest::new(1, 1)).await?;
    let mut recent_req = PageRequest::new(1, RECENT_COUNT);
    recent_req.sort_by = Some("OrderDate".to_owned());
    recent_req.descending = true;
    let recent = svc.orders.get_orders(&recent_req).await?;
    Ok(Dashboard {
        today_orders: todays.len(),
        today_revenue: todays.iter().map(|o| o.total_amount).sum(),
        pending_orders: pending.len(),
        total_customers: customers.total_count,
        recent: recent.items,
    })
}

/// Execute a home task.
pub async fn run(task: Task, svc: UiServices) -> Input {
    match task {
        Task::LoadDashboard => Input::Loaded(load(&svc).await.map_err(|e| e.to_user_message())),
    }
}
