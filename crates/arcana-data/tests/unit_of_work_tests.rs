//! Extra tests (no C# counterpart in Arcana.Data.Tests): unit of work, transaction scope and
//! factory semantics, mirroring the behaviour of C# `UnitOfWorkImpl` / `TransactionScopeImpl`.

use arcana_core::{ErrorCode, Repository, UnitOfWork, UnitOfWorkFactory};
use arcana_data::{
    CustomerRepository, Db, OrderRepository, ProductRepository, SqliteUnitOfWork,
    SqliteUnitOfWorkFactory, SyncQueueItem,
};
use arcana_domain::{Customer, Product};

fn customer(code: &str) -> Customer {
    Customer {
        code: code.into(),
        name: code.into(),
        ..Customer::default()
    }
}

async fn customers(db: &Db) -> u64 {
    SqliteUnitOfWork::new(db.clone())
        .customers()
        .count(None)
        .await
        .unwrap()
}

#[tokio::test]
async fn commit_without_transaction_returns_zero_and_guards_reuse() {
    let db = Db::in_memory().await.unwrap();
    let mut uow = SqliteUnitOfWork::new(db.clone());
    uow.customers().add(customer("A")).await.unwrap();
    assert!(!uow.has_changes(), "autocommitted writes are not pending");
    assert_eq!(uow.commit().await.unwrap(), 0);
    assert!(uow.is_committed());
    let e = uow.commit().await.unwrap_err();
    assert_eq!(e.message(), "This unit of work has already been committed");

    let mut rolled = SqliteUnitOfWork::new(db.clone());
    rolled.rollback().await.unwrap();
    rolled.rollback().await.unwrap(); // idempotent
    assert!(rolled.is_rolled_back());
    let e = rolled.commit().await.unwrap_err();
    assert_eq!(e.message(), "This unit of work has been rolled back");
    assert_eq!(e.code(), ErrorCode::DatabaseError);
}

#[tokio::test]
async fn transaction_commit_persists_and_counts_rows() {
    let db = Db::in_memory().await.unwrap();
    let mut uow = SqliteUnitOfWork::new(db.clone());
    let scope = uow.begin_transaction().await.unwrap();
    assert!(scope.is_active());
    assert!(!scope.transaction_id().is_nil());
    uow.customers()
        .add_range(vec![customer("A"), customer("B")])
        .await
        .unwrap();
    uow.products()
        .add(Product {
            code: "P".into(),
            name: "P".into(),
            ..Product::default()
        })
        .await
        .unwrap();
    assert!(uow.has_changes());
    assert_eq!(uow.commit().await.unwrap(), 3);
    drop(scope); // already committed through the unit: dropping must not roll back
    assert_eq!(customers(&db).await, 2);
}

#[tokio::test]
async fn transaction_rollback_discards_writes() {
    let db = Db::in_memory().await.unwrap();
    let mut uow = SqliteUnitOfWork::new(db.clone());
    let _scope = uow.begin_transaction().await.unwrap();
    uow.customers().add(customer("A")).await.unwrap();
    uow.rollback().await.unwrap();
    assert!(uow.is_rolled_back());
    assert!(!db.in_transaction());
    assert_eq!(customers(&db).await, 0);
}

#[tokio::test]
async fn only_one_transaction_at_a_time() {
    let db = Db::in_memory().await.unwrap();
    let mut uow = SqliteUnitOfWork::new(db.clone());
    let _scope = uow.begin_transaction().await.unwrap();
    let Err(e) = uow.begin_transaction().await else {
        panic!("second transaction must fail")
    };
    assert_eq!(e.message(), "A transaction is already in progress");
}

#[tokio::test]
async fn scope_commit_and_rollback() {
    let db = Db::in_memory().await.unwrap();
    let mut uow = SqliteUnitOfWork::new(db.clone());
    let mut scope = uow.begin_transaction().await.unwrap();
    uow.customers().add(customer("A")).await.unwrap();
    scope.commit().await.unwrap();
    assert!(!scope.is_active());
    let e = scope.commit().await.unwrap_err();
    assert_eq!(e.message(), "Transaction is no longer active");
    scope.rollback().await.unwrap(); // inactive: no-op
    assert_eq!(customers(&db).await, 1);

    let mut scope = uow.begin_transaction().await.unwrap();
    uow.customers().add(customer("B")).await.unwrap();
    scope.rollback().await.unwrap();
    assert!(!scope.is_active());
    assert_eq!(customers(&db).await, 1);
}

#[tokio::test]
async fn dropping_an_active_scope_rolls_back() {
    let db = Db::in_memory().await.unwrap();
    let mut uow = SqliteUnitOfWork::new(db.clone());
    {
        let _scope = uow.begin_transaction().await.unwrap();
        uow.customers().add(customer("A")).await.unwrap();
    }
    assert!(!db.in_transaction());
    assert_eq!(customers(&db).await, 0);
}

#[tokio::test]
async fn dropping_an_uncommitted_unit_rolls_back() {
    let db = Db::in_memory().await.unwrap();
    let factory = SqliteUnitOfWorkFactory::new(db.clone());
    {
        let uow = factory.create_with_transaction().await.unwrap();
        assert!(db.in_transaction());
        uow.customers().add(customer("A")).await.unwrap();
    }
    assert_eq!(customers(&db).await, 0);

    let mut uow = factory.create_with_transaction().await.unwrap();
    uow.customers().add(customer("B")).await.unwrap();
    assert_eq!(uow.commit().await.unwrap(), 1);
    drop(uow);
    assert_eq!(customers(&db).await, 1);

    let plain = factory.create();
    assert!(!plain.is_committed() && !plain.is_rolled_back());
}

#[tokio::test]
async fn failed_commit_rolls_back() {
    let db = Db::in_memory().await.unwrap();
    let mut uow = SqliteUnitOfWork::new(db.clone());
    let _scope = uow.begin_transaction().await.unwrap();
    {
        // End the transaction behind the unit's back so its COMMIT fails.
        let mut conn = db.lock().await.unwrap();
        sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();
    }
    assert!(uow.commit().await.is_err());
    assert!(uow.is_rolled_back());
    assert!(!uow.is_committed());
}

#[tokio::test]
async fn unit_hands_out_repositories_on_its_connection() {
    let db = Db::in_memory().await.unwrap();
    let mut uow = SqliteUnitOfWork::new(db.clone());
    let _scope = uow.begin_transaction().await.unwrap();
    let queue = uow.repository::<SyncQueueItem>();
    queue.add(SyncQueueItem::default()).await.unwrap();
    assert!(uow.customers().get_by_code("x").await.unwrap().is_none());
    assert_eq!(uow.products().get_categories().await.unwrap().len(), 0);
    assert_eq!(uow.orders().get_by_customer_id(1).await.unwrap().len(), 0);
    assert_eq!(uow.commit().await.unwrap(), 1);
    assert_eq!(queue.count(None).await.unwrap(), 1);
}
