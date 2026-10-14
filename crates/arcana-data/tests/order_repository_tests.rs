//! Port of C# `tests/Arcana.Data.Tests/Repository/OrderRepositoryTests.cs` (9 tests).
//! C# used EF's in-memory provider; this uses a real in-memory `SQLite` database
//! (`sqlite::memory:`) with the production migrations applied.

use arcana_core::{Repository, today};
use arcana_data::{CustomerRepositoryImpl, Db, OrderRepository, OrderRepositoryImpl};
use arcana_domain::{Customer, Order, OrderStatus};
use chrono::Duration;
use rust_decimal_macros::dec;

struct Fixture {
    db: Db,
    repository: OrderRepositoryImpl,
}

fn order(id: i32, number: &str, days_ago: i64, status: OrderStatus) -> Order {
    let mut o = Order {
        order_number: number.into(),
        order_date: today() - Duration::days(days_ago),
        customer_id: 1,
        customer_name: "Test Customer".into(),
        status,
        ..Order::default()
    };
    o.base.id = id;
    o
}

async fn setup() -> Fixture {
    let db = Db::in_memory().await.unwrap();
    let mut customer = Customer {
        code: "CUST001".into(),
        name: "Test Customer".into(),
        ..Customer::default()
    };
    customer.base.id = 1;
    CustomerRepositoryImpl::with_db(db.clone())
        .add(customer)
        .await
        .unwrap();
    let repository = OrderRepositoryImpl::with_db(db.clone());
    let mut o1 = order(1, "ORD20240101001", 0, OrderStatus::Draft);
    o1.total_amount = dec!(1000);
    let mut o2 = order(2, "ORD20240101002", 1, OrderStatus::Confirmed);
    o2.total_amount = dec!(2000);
    let mut o3 = order(3, "ORD20240101003", 2, OrderStatus::Completed);
    o3.total_amount = dec!(3000);
    repository.add_range(vec![o1, o2, o3]).await.unwrap();
    Fixture { db, repository }
}

/// C#: `GetByIdAsync_ExistingOrder_ShouldReturnOrder`
#[tokio::test]
async fn get_by_id_async_existing_order_should_return_order() {
    let f = setup().await;
    let order = f.repository.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(order.order_number, "ORD20240101001");
}

/// C#: `GetByIdAsync_NonExistingOrder_ShouldReturnNull`
#[tokio::test]
async fn get_by_id_async_non_existing_order_should_return_null() {
    let f = setup().await;
    assert_eq!(f.repository.get_by_id(999).await.unwrap(), None);
}

/// C#: `GetByOrderNumberAsync_ExistingOrder_ShouldReturnOrder`
#[tokio::test]
async fn get_by_order_number_async_existing_order_should_return_order() {
    let f = setup().await;
    let order = f
        .repository
        .get_by_order_number("ORD20240101001")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(order.base.id, 1);
}

/// C#: `GetByStatusAsync_ShouldReturnOrdersWithStatus`
#[tokio::test]
async fn get_by_status_async_should_return_orders_with_status() {
    let f = setup().await;
    let orders = f
        .repository
        .get_by_status(OrderStatus::Draft)
        .await
        .unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].order_number, "ORD20240101001");
}

/// C#: `GetByCustomerIdAsync_ShouldReturnCustomerOrders`
#[tokio::test]
async fn get_by_customer_id_async_should_return_customer_orders() {
    let f = setup().await;
    assert_eq!(f.repository.get_by_customer_id(1).await.unwrap().len(), 3);
}

/// C#: `GetByDateRangeAsync_ShouldReturnOrdersInRange`
#[tokio::test]
async fn get_by_date_range_async_should_return_orders_in_range() {
    let f = setup().await;
    let orders = f
        .repository
        .get_by_date_range(today() - Duration::days(1), today())
        .await
        .unwrap();
    assert_eq!(orders.len(), 2);
}

/// C#: `GenerateOrderNumberAsync_ShouldGenerateSequentialNumber`
#[tokio::test]
async fn generate_order_number_async_should_generate_sequential_number() {
    let f = setup().await;
    let n = f.repository.generate_order_number().await.unwrap();
    assert!(n.starts_with(&format!("ORD{}", today().format("%Y%m%d"))));
}

/// C#: `AddAsync_ShouldAddNewOrder`
#[tokio::test]
async fn add_async_should_add_new_order() {
    let f = setup().await;
    let new_order = order(0, "ORD20240102001", 0, OrderStatus::Draft);
    let result = f.repository.add(new_order).await.unwrap();
    assert!(result.base.id > 0);
    assert!(
        f.repository
            .get_by_id(result.base.id)
            .await
            .unwrap()
            .is_some()
    );
}

/// C#: `DeleteAsync_ShouldSoftDelete`
#[tokio::test]
async fn delete_async_should_soft_delete() {
    let f = setup().await;
    let mut order = f.repository.get_by_id(1).await.unwrap().unwrap();
    f.repository.delete(&mut order).await.unwrap();
    // C# reads with IgnoreQueryFilters(); here: raw SQL on the shared connection.
    let mut conn = f.db.lock().await.unwrap();
    let (deleted,): (bool,) = sqlx::query_as("SELECT IsDeleted FROM Orders WHERE Id = 1")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert!(deleted);
}
