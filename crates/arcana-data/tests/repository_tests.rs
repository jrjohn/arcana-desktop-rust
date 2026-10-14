//! Extra tests (no C# counterpart): the generic repository against every mapped table, the
//! migration constraints, and error mapping.

use arcana_core::{ErrorCode, PageRequest, Repository, utc_now};
use arcana_data::{Db, SqliteRepository, SyncOperation, SyncQueueItem, SyncStatus, TableEntity};
use arcana_domain::{
    AppPermission, AuditEventType, AuditLog, Customer, Order, OrderItem, OrderStatus,
    PaymentMethod, PaymentStatus, Product, ProductCategory, Role, RolePermission, User,
    UserPermission, UserRole,
};
use rust_decimal_macros::dec;

async fn db() -> Db {
    Db::in_memory().await.unwrap()
}

async fn add<T: TableEntity>(db: &Db, e: T) -> T {
    SqliteRepository::<T>::new(db.clone()).add(e).await.unwrap()
}

async fn roundtrip<T: TableEntity + PartialEq + std::fmt::Debug>(db: &Db, e: T) -> T {
    let saved = add(db, e).await;
    let loaded = SqliteRepository::<T>::new(db.clone())
        .get_by_id(saved.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded, saved, "{} round trip", T::TABLE);
    saved
}

fn customer(code: &str, name: &str) -> Customer {
    Customer {
        code: code.into(),
        name: name.into(),
        ..Customer::default()
    }
}

#[tokio::test]
async fn business_tables_round_trip_all_columns() {
    let db = db().await;
    let now = utc_now();
    let cat = roundtrip(
        &db,
        ProductCategory {
            code: "C".into(),
            name: "Cat".into(),
            sort_order: 3,
            ..ProductCategory::default()
        },
    )
    .await;
    let child = roundtrip(
        &db,
        ProductCategory {
            code: "C2".into(),
            name: "Child".into(),
            parent_id: Some(cat.base.id),
            ..ProductCategory::default()
        },
    )
    .await;
    assert_eq!(child.parent_id, Some(cat.base.id));
    let mut cust = customer("CUST", "Cust");
    cust.contact_name = Some("Ann".into());
    cust.phone = Some("1".into());
    cust.email = Some("a@b".into());
    cust.address = Some("addr".into());
    cust.city = Some("city".into());
    cust.state = Some("st".into());
    cust.postal_code = Some("300".into());
    cust.country = Some("TW".into());
    cust.tax_id = Some("T".into());
    cust.credit_limit = dec!(1234.56);
    cust.balance = dec!(-7.25);
    cust.notes = Some("n".into());
    cust.base.audit.created_by = Some("me".into());
    cust.base.audit.modified_by = Some("you".into());
    cust.base.sync.last_sync_at = Some(now);
    let cust = roundtrip(&db, cust).await;
    let prod = roundtrip(
        &db,
        Product {
            code: "P".into(),
            name: "Prod".into(),
            description: Some("d".into()),
            category_id: Some(cat.base.id),
            price: dec!(9.99),
            cost: dec!(5),
            stock_quantity: dec!(1.2345),
            min_stock_level: dec!(1),
            max_stock_level: dec!(10),
            barcode: Some("123".into()),
            weight: Some(dec!(0.5)),
            ..Product::default()
        },
    )
    .await;
    let ord = roundtrip(
        &db,
        Order {
            order_number: "ORD1".into(),
            customer_id: cust.base.id,
            customer_name: "Cust".into(),
            status: OrderStatus::Shipped,
            payment_status: PaymentStatus::PartialPaid,
            payment_method: PaymentMethod::BankTransfer,
            subtotal: dec!(1),
            tax_amount: dec!(0.05),
            discount_amount: dec!(0.5),
            shipping_cost: dec!(2),
            total_amount: dec!(2.55),
            paid_amount: dec!(1),
            shipping_address: Some("a".into()),
            shipping_city: Some("c".into()),
            shipping_postal_code: Some("p".into()),
            expected_delivery_date: Some(now),
            actual_delivery_date: Some(now),
            notes: Some("n".into()),
            ..Order::default()
        },
    )
    .await;
    roundtrip(
        &db,
        OrderItem {
            order_id: ord.base.id,
            line_number: 1,
            product_id: prod.base.id,
            product_code: "P".into(),
            product_name: "Prod".into(),
            quantity: dec!(2),
            unit_price: dec!(9.99),
            discount_percent: dec!(10),
            notes: Some("x".into()),
            ..OrderItem::default()
        },
    )
    .await;
}

#[tokio::test]
async fn identity_and_sync_tables_round_trip_all_columns() {
    let db = db().await;
    let now = utc_now();
    let user = roundtrip(
        &db,
        User {
            username: "bob".into(),
            email: Some("b@x".into()),
            display_name: "Bob".into(),
            password_hash: "h".into(),
            password_salt: Some("s".into()),
            is_locked: true,
            locked_until: Some(now),
            failed_login_attempts: 3,
            last_login_at: Some(now),
            password_changed_at: Some(now),
            must_change_password: true,
            refresh_token: Some("r".into()),
            refresh_token_expires_at: Some(now),
            ..User::default()
        },
    )
    .await;
    let role = roundtrip(
        &db,
        Role {
            name: "Admin".into(),
            display_name: "Administrator".into(),
            description: Some("d".into()),
            is_system: true,
            priority: 100,
            ..Role::default()
        },
    )
    .await;
    let perm = roundtrip(
        &db,
        AppPermission {
            code: "orders.view".into(),
            display_name: "View".into(),
            description: Some("d".into()),
            category: "Orders".into(),
            is_system: true,
            ..AppPermission::default()
        },
    )
    .await;
    roundtrip(
        &db,
        UserRole {
            user_id: user.base.id,
            role_id: role.base.id,
            assigned_by: Some("x".into()),
            expires_at: Some(now),
            ..UserRole::default()
        },
    )
    .await;
    roundtrip(
        &db,
        RolePermission {
            role_id: role.base.id,
            permission_id: perm.base.id,
            granted_by: Some("x".into()),
            ..RolePermission::default()
        },
    )
    .await;
    roundtrip(
        &db,
        UserPermission {
            user_id: user.base.id,
            permission_id: perm.base.id,
            is_granted: false,
            assigned_by: Some("x".into()),
            expires_at: Some(now),
            ..UserPermission::default()
        },
    )
    .await;
    roundtrip(
        &db,
        AuditLog {
            event_type: AuditEventType::AccessDenied,
            user_id: Some(user.base.id),
            username: Some("bob".into()),
            client_info: Some("ci".into()),
            resource: Some("orders".into()),
            action: "view".into(),
            details: Some("d".into()),
            is_success: false,
            error_message: Some("e".into()),
            ..AuditLog::default()
        },
    )
    .await;
}

#[tokio::test]
async fn sync_queue_round_trips_all_columns() {
    let db = db().await;
    roundtrip(
        &db,
        SyncQueueItem {
            entity_type: "Order".into(),
            entity_id: "1".into(),
            operation: SyncOperation::Update,
            payload: Some("{}".into()),
            retry_count: 2,
            last_error: Some("x".into()),
            status: SyncStatus::Failed,
            ..SyncQueueItem::default()
        },
    )
    .await;
}

#[tokio::test]
async fn add_sets_audit_and_sync_and_generated_id() {
    let db = db().await;
    let before = utc_now();
    let mut c = customer("A", "A");
    c.base.sync.is_pending_sync = false;
    let c = add(&db, c).await;
    assert!(c.base.id > 0);
    assert!(c.base.audit.created_at >= before);
    assert!(c.base.sync.is_pending_sync);
}

#[tokio::test]
async fn queries_and_predicates() {
    let db = db().await;
    let repo = SqliteRepository::<Customer>::new(db.clone());
    repo.add_range(vec![
        customer("B", "Bravo"),
        customer("A", "Alpha"),
        customer("C", "Charlie"),
    ])
    .await
    .unwrap();
    assert_eq!(repo.get_all().await.unwrap().len(), 3);
    assert_eq!(repo.count(None).await.unwrap(), 3);
    assert_eq!(
        repo.count(Some(&|c: &Customer| c.code.as_str() > "A"))
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        repo.find(&|c: &Customer| c.code == "C")
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        repo.first_or_default(&|c: &Customer| c.name.starts_with('B'))
            .await
            .unwrap()
            .map(|c| c.code),
        Some("B".into())
    );
    assert!(repo.any(&|c: &Customer| c.code == "A").await.unwrap());
    assert!(!repo.any(&|c: &Customer| c.code == "Z").await.unwrap());

    let mut req = PageRequest::new(1, 2);
    req.sort_by = Some("Name".into());
    let page = repo.get_paged(&req).await.unwrap();
    let names: Vec<_> = page.items.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "Bravo"]);
    assert_eq!((page.total_count, page.total_pages()), (3, 2));
    req.descending = true;
    req.page = 2;
    let page = repo.get_paged(&req).await.unwrap();
    assert_eq!(page.items[0].name, "Alpha");
    let unsorted = repo.get_paged(&PageRequest::default()).await.unwrap();
    assert_eq!(unsorted.items.len(), 3);
    req.sort_by = Some("Name; DROP TABLE Customers".into());
    let err = repo.get_paged(&req).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::QueryFailed);
}

#[tokio::test]
async fn update_sets_modified_and_missing_row_is_conflict() {
    let db = db().await;
    let repo = SqliteRepository::<Customer>::new(db.clone());
    let mut c = repo.add(customer("A", "A")).await.unwrap();
    c.name = "Renamed".into();
    c.base.sync.is_pending_sync = false;
    repo.update(&mut c).await.unwrap();
    assert!(c.base.audit.modified_at.is_some());
    assert!(c.base.sync.is_pending_sync);
    assert_eq!(
        repo.get_by_id(c.base.id).await.unwrap().unwrap().name,
        "Renamed"
    );
    let mut ghost = customer("G", "G");
    ghost.base.id = 999;
    let err = repo.update(&mut ghost).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::ConflictError);
}

#[tokio::test]
async fn soft_delete_hides_rows_but_audit_logs_and_sync_queue_differ() {
    let db = db().await;
    let repo = SqliteRepository::<Customer>::new(db.clone());
    let c = repo.add(customer("A", "A")).await.unwrap();
    repo.delete_by_id(c.base.id).await.unwrap();
    repo.delete_by_id(12345).await.unwrap(); // missing: no-op, like C#
    assert_eq!(repo.get_by_id(c.base.id).await.unwrap(), None);
    assert_eq!(repo.count(None).await.unwrap(), 0);

    // AuditLog has no query filter in C#: a soft-deleted log stays visible.
    let logs = SqliteRepository::<AuditLog>::new(db.clone());
    let mut log = logs
        .add(AuditLog {
            action: "x".into(),
            ..AuditLog::default()
        })
        .await
        .unwrap();
    logs.delete(&mut log).await.unwrap();
    let still = logs.get_by_id(log.base.id).await.unwrap().unwrap();
    assert!(still.base.soft_delete.is_deleted);
    assert!(still.base.soft_delete.deleted_at.is_some());

    // SyncQueueItem is not soft-deletable: delete removes the row.
    let queue = SqliteRepository::<SyncQueueItem>::new(db.clone());
    let mut item = queue.add(SyncQueueItem::default()).await.unwrap();
    queue.delete(&mut item).await.unwrap();
    let mut conn = db.lock().await.unwrap();
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM SyncQueue")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn constraints_map_to_error_codes_and_add_range_is_atomic() {
    let db = db().await;
    let repo = SqliteRepository::<Customer>::new(db.clone());
    repo.add(customer("A", "A")).await.unwrap();
    let err = repo.add(customer("A", "Again")).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::DuplicateEntry);

    let err = repo
        .add_range(vec![customer("B", "B"), customer("B", "B2")])
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::DuplicateEntry);
    assert_eq!(
        repo.count(None).await.unwrap(),
        1,
        "first insert rolled back"
    );

    let orders = SqliteRepository::<Order>::new(db.clone());
    let err = orders
        .add(Order {
            order_number: "X".into(),
            customer_id: 42,
            customer_name: "nobody".into(),
            ..Order::default()
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ReferentialIntegrity);
}

#[tokio::test]
async fn unknown_enum_value_in_storage_is_data_corruption() {
    let db = db().await;
    let c = add(&db, customer("A", "A")).await;
    let o = add(
        &db,
        Order {
            order_number: "O".into(),
            customer_id: c.base.id,
            customer_name: "A".into(),
            ..Order::default()
        },
    )
    .await;
    {
        let mut conn = db.lock().await.unwrap();
        sqlx::query("UPDATE Orders SET Status = 99")
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    let err = SqliteRepository::<Order>::new(db.clone())
        .get_by_id(o.base.id)
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::DataCorruption);
}

#[tokio::test]
async fn repository_handle_exposes_its_db_and_clones() {
    let db = db().await;
    let repo = SqliteRepository::<Customer>::new(db.clone());
    let copy = repo.clone();
    copy.add(customer("A", "A")).await.unwrap();
    assert!(!repo.db().in_transaction());
    assert_eq!(repo.count(None).await.unwrap(), 1);
}

#[tokio::test]
async fn file_database_persists_across_connections() {
    let dir = std::env::temp_dir().join(format!("arcana-data-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let url = format!("sqlite://{}", dir.join("app.db").display());
    {
        let db = Db::connect(&url).await.unwrap();
        add(&db, customer("A", "A")).await;
    }
    let db = Db::connect(&url).await.unwrap(); // migrations are idempotent
    assert_eq!(
        SqliteRepository::<Customer>::new(db)
            .count(None)
            .await
            .unwrap(),
        1
    );
    std::fs::remove_dir_all(dir).unwrap();
}
