//! C# `OrderServiceTests` (30).

mod common;

use std::sync::Arc;

use arcana_core::{ErrorCode, PageRequest, today};
use arcana_domain::services::OrderService;
use arcana_domain::{BaseEntity, Order, OrderItem, OrderStatus};
use arcana_infra::OrderServiceImpl;
use chrono::TimeDelta;
use common::{Fake, FakeValidator};
use rust_decimal_macros::dec;

fn order(id: i32, number: &str, customer_id: i32) -> Order {
    Order {
        base: BaseEntity::with_id(id),
        order_number: number.into(),
        customer_id,
        customer_name: "Test Customer".into(),
        ..Order::default()
    }
}

/// One fixed instance: entities get a random `SyncId`, so equality needs the same one.
static FIXTURE: std::sync::LazyLock<Order> = std::sync::LazyLock::new(|| order(1, "ORD-001", 10));

fn o1() -> Order {
    FIXTURE.clone()
}

fn svc(repo: &Arc<Fake<Order>>) -> OrderServiceImpl {
    OrderServiceImpl::new(repo.clone(), Arc::new(FakeValidator::default()))
}

fn failing(method: &'static str) -> Arc<Fake<Order>> {
    let repo = Fake::with(vec![o1()]);
    repo.fail(method);
    repo
}

// GetOrdersAsync_ShouldReturnPagedResult
#[tokio::test]
async fn get_orders_should_return_paged_result() {
    let page = svc(&Fake::with(vec![o1()]))
        .get_orders(&PageRequest::default())
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
}

// GetOrdersAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_orders_repository_throws_should_return_failure() {
    let e = svc(&failing("get_paged"))
        .get_orders(&PageRequest::default())
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::QueryFailed, "Failed to get orders")
    );
}

// GetOrderByIdAsync_ExistingOrder_ShouldReturnSuccess
#[tokio::test]
async fn get_order_by_id_existing_order_should_return_success() {
    let repo = Fake::with(vec![o1()]);
    assert_eq!(svc(&repo).get_order_by_id(1).await.unwrap(), o1());
    assert!(repo.called("get_by_id_with_items"));
}

// GetOrderByIdAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn get_order_by_id_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .get_order_by_id(99)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::NotFound, "Order with ID 99 not found")
    );
}

// GetOrderByIdAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_order_by_id_repository_throws_should_return_failure() {
    let e = svc(&failing("get_by_id_with_items"))
        .get_order_by_id(1)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::QueryFailed, "Failed to get order")
    );
}

// GetOrderByNumberAsync_ExistingNumber_ShouldReturnSuccess
#[tokio::test]
async fn get_order_by_number_existing_number_should_return_success() {
    assert_eq!(
        svc(&Fake::with(vec![o1()]))
            .get_order_by_number("ORD-001")
            .await
            .unwrap(),
        o1()
    );
}

// GetOrderByNumberAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn get_order_by_number_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .get_order_by_number("INVALID")
        .await
        .unwrap_err();
    assert_eq!(e.message(), "Order INVALID not found");
}

// GetOrderByNumberAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_order_by_number_repository_throws_should_return_failure() {
    let e = svc(&failing("get_by_order_number"))
        .get_order_by_number("ORD-001")
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::QueryFailed);
}

// CreateOrderAsync_Valid_ShouldReturnSuccess
#[tokio::test]
async fn create_order_valid_should_return_success() {
    let repo = Fake::with(vec![]);
    let created = svc(&repo).create_order(o1()).await.unwrap();
    assert_eq!(created.order_number, "ORD-001");
    assert!(!repo.called("generate_order_number"));
}

// CreateOrderAsync_WithEmptyOrderNumber_ShouldGenerateNumber
#[tokio::test]
async fn create_order_with_empty_order_number_should_generate_number() {
    let repo = Fake::with(vec![]);
    "ORD-AUTO-001".clone_into(&mut repo.next_number.lock().unwrap());
    let mut o = o1();
    o.order_number.clear();
    let created = svc(&repo).create_order_in_place(&mut o).await.unwrap();
    assert_eq!(o.order_number, "ORD-AUTO-001");
    assert_eq!(created.order_number, "ORD-AUTO-001");
}

// CreateOrderAsync_ValidationFails_ShouldReturnFailure
#[tokio::test]
async fn create_order_validation_fails_should_return_failure() {
    let repo = Fake::with(vec![]);
    let s = OrderServiceImpl::new(
        repo.clone(),
        FakeValidator::failing("CustomerId", "Customer required"),
    );
    let e = s.create_order(o1()).await.unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::ValidationFailed, "Order validation failed")
    );
    assert!(!repo.called("add"));
}

// CreateOrderAsync_WithItems_ShouldSetLineNumbers
#[tokio::test]
async fn create_order_with_items_should_set_line_numbers() {
    let mut o = o1();
    o.items = vec![
        OrderItem {
            product_id: 1,
            quantity: dec!(2),
            unit_price: dec!(100),
            ..OrderItem::default()
        },
        OrderItem {
            product_id: 2,
            quantity: dec!(1),
            unit_price: dec!(50),
            ..OrderItem::default()
        },
    ];
    let created = svc(&Fake::with(vec![]))
        .create_order_in_place(&mut o)
        .await
        .unwrap();
    assert_eq!(o.items[0].line_number, 1);
    assert_eq!(o.items[1].line_number, 2);
    assert_eq!(created.subtotal, dec!(250));
}

// CreateOrderAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn create_order_repository_throws_should_return_failure() {
    let e = svc(&failing("add")).create_order(o1()).await.unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::DatabaseError, "Failed to create order")
    );
}

// UpdateOrderAsync_Valid_ShouldReturnSuccess
#[tokio::test]
async fn update_order_valid_should_return_success() {
    let repo = Fake::with(vec![o1()]);
    let mut changed = o1();
    changed.notes = Some("n".into());
    let updated = svc(&repo).update_order(changed).await.unwrap();
    assert_eq!(updated.notes.as_deref(), Some("n"));
    assert_eq!(repo.items.lock().unwrap()[0].notes.as_deref(), Some("n"));
}

// UpdateOrderAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn update_order_not_found_should_return_failure() {
    assert_eq!(
        svc(&Fake::with(vec![]))
            .update_order(o1())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
}

// UpdateOrderAsync_ValidationFails_ShouldReturnFailure
#[tokio::test]
async fn update_order_validation_fails_should_return_failure() {
    let s = OrderServiceImpl::new(
        Fake::with(vec![o1()]),
        FakeValidator::failing("OrderNumber", "Required"),
    );
    assert_eq!(
        s.update_order(o1()).await.unwrap_err().code(),
        ErrorCode::ValidationFailed
    );
}

// UpdateOrderAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn update_order_repository_throws_should_return_failure() {
    let e = svc(&failing("update"))
        .update_order(o1())
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::DatabaseError, "Failed to update order")
    );
}

// DeleteOrderAsync_ExistingOrder_ShouldReturnSuccess
#[tokio::test]
async fn delete_order_existing_order_should_return_success() {
    let repo = Fake::with(vec![o1()]);
    svc(&repo).delete_order(1).await.unwrap();
    assert!(repo.items.lock().unwrap().is_empty());
}

// DeleteOrderAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn delete_order_not_found_should_return_failure() {
    assert_eq!(
        svc(&Fake::with(vec![]))
            .delete_order(99)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
}

// DeleteOrderAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn delete_order_repository_throws_should_return_failure() {
    let e = svc(&failing("delete")).delete_order(1).await.unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::DatabaseError, "Failed to delete order")
    );
}

// ChangeStatusAsync_ExistingOrder_ShouldUpdateStatus
#[tokio::test]
async fn change_status_existing_order_should_update_status() {
    let repo = Fake::with(vec![o1()]);
    let o = svc(&repo)
        .change_status(1, OrderStatus::Confirmed)
        .await
        .unwrap();
    assert_eq!(o.status, OrderStatus::Confirmed);
    assert_eq!(repo.items.lock().unwrap()[0].status, OrderStatus::Confirmed);
}

// ChangeStatusAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn change_status_not_found_should_return_failure() {
    assert_eq!(
        svc(&Fake::with(vec![]))
            .change_status(99, OrderStatus::Confirmed)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
}

// ChangeStatusAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn change_status_repository_throws_should_return_failure() {
    let e = svc(&failing("update"))
        .change_status(1, OrderStatus::Cancelled)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::DatabaseError, "Failed to change order status")
    );
}

// GetOrdersByCustomerAsync_ShouldReturnOrders
#[tokio::test]
async fn get_orders_by_customer_should_return_orders() {
    let repo = Fake::with(vec![o1(), order(2, "ORD-002", 10), order(3, "ORD-003", 11)]);
    assert_eq!(
        svc(&repo).get_orders_by_customer(10).await.unwrap().len(),
        2
    );
}

// GetOrdersByCustomerAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_orders_by_customer_repository_throws_should_return_failure() {
    let e = svc(&failing("get_by_customer_id"))
        .get_orders_by_customer(10)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::QueryFailed, "Failed to get orders")
    );
}

// GetOrdersByDateRangeAsync_ShouldReturnOrders
#[tokio::test]
async fn get_orders_by_date_range_should_return_orders() {
    let to = today();
    let from = to - TimeDelta::days(7);
    let repo = Fake::with(vec![o1()]);
    assert_eq!(
        svc(&repo)
            .get_orders_by_date_range(from, to)
            .await
            .unwrap()
            .len(),
        1
    );
}

// GetOrdersByDateRangeAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_orders_by_date_range_repository_throws_should_return_failure() {
    let to = today();
    let e = svc(&failing("get_by_date_range"))
        .get_orders_by_date_range(to - TimeDelta::days(7), to)
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::QueryFailed);
}

// GetOrdersByStatusAsync_ShouldReturnMatchingOrders
#[tokio::test]
async fn get_orders_by_status_should_return_matching_orders() {
    let repo = Fake::with(vec![o1()]);
    assert_eq!(
        svc(&repo)
            .get_orders_by_status(o1().status)
            .await
            .unwrap()
            .len(),
        1
    );
}

// GetOrdersByStatusAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_orders_by_status_repository_throws_should_return_failure() {
    let e = svc(&failing("get_by_status"))
        .get_orders_by_status(OrderStatus::Pending)
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::QueryFailed);
}

// GenerateOrderNumberAsync_ShouldReturnGeneratedNumber
#[tokio::test]
async fn generate_order_number_should_return_generated_number() {
    let repo = Fake::with(vec![]);
    "ORD-20260001".clone_into(&mut repo.next_number.lock().unwrap());
    assert_eq!(
        svc(&repo).generate_order_number().await.unwrap(),
        "ORD-20260001"
    );
}
