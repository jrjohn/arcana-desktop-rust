use arcana_core::{AppResult, today};
use arcana_domain::{Order, OrderStatus};
use rust_decimal::Decimal;

use super::{Input, Loaded, Task};
use crate::services::UiServices;

/// C# loads up to 100 customers for the lookup.
const CUSTOMER_LOOKUP: u32 = 100;
/// C# `SearchProductsAsync` default.
const PRODUCT_RESULTS: u32 = 20;

async fn load(svc: &UiServices, id: Option<i32>) -> AppResult<Loaded> {
    let customers = svc.customers.search_customers("", CUSTOMER_LOOKUP).await?;
    let order = match id {
        Some(id) => svc.orders.get_order_by_id(id).await?,
        None => Order {
            order_number: svc.orders.generate_order_number().await?,
            order_date: today(),
            status: OrderStatus::Draft,
            tax_rate: Decimal::from(5),
            ..Order::default()
        },
    };
    Ok(Loaded { customers, order })
}

/// Execute an order-detail task.
pub async fn run(task: Task, svc: UiServices) -> Input {
    let msg = |e: arcana_core::AppError| e.to_user_message();
    match task {
        Task::Load(id) => Input::Loaded(load(&svc, id).await.map_err(msg)),
        Task::SearchProducts(q) => Input::ProductsFound(
            svc.products
                .search_products(&q, PRODUCT_RESULTS)
                .await
                .map_err(msg),
        ),
        Task::Save { order, is_new } => {
            let r = if is_new {
                svc.orders.create_order(*order).await
            } else {
                svc.orders.update_order(*order).await
            };
            Input::Saved(r.map_err(msg))
        }
    }
}
