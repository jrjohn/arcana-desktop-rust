//! Extra tests (no C# counterpart): the DAO-backed finders of the customer, product and order
//! repositories, the inherited generic operations on each, and order-item persistence.

use std::sync::Arc;

use arcana_core::{ErrorCode, PageRequest, Repository, today};
use arcana_data::{
    CustomerDaoImpl, CustomerRepository, CustomerRepositoryImpl, Db, OrderDaoImpl, OrderFilter,
    OrderRepository, OrderRepositoryImpl, ProductDaoImpl, ProductRepository, ProductRepositoryImpl,
    SqliteRepository,
};
use arcana_domain::{Customer, Order, OrderItem, OrderStatus, Product, ProductCategory};
use chrono::Duration;
use rust_decimal_macros::dec;

fn customer(code: &str, name: &str, phone: Option<&str>, active: bool) -> Customer {
    Customer {
        code: code.into(),
        name: name.into(),
        phone: phone.map(Into::into),
        is_active: active,
        ..Customer::default()
    }
}

fn product(code: &str, name: &str, barcode: Option<&str>, category: Option<i32>) -> Product {
    Product {
        code: code.into(),
        name: name.into(),
        barcode: barcode.map(Into::into),
        category_id: category,
        ..Product::default()
    }
}

/// Every inherited `Repository` operation, through the specialised repository.
async fn exercise_inherited<T, R>(repo: &R, make: impl Fn(&str) -> T, rename: impl Fn(&mut T))
where
    T: Send + Sync + Clone + 'static,
    R: Repository<T>,
{
    repo.add_range(vec![make("X1"), make("X2")]).await.unwrap();
    let mut a = repo.add(make("X3")).await.unwrap();
    assert_eq!(repo.get_all().await.unwrap().len(), 3);
    assert_eq!(repo.count(None).await.unwrap(), 3);
    assert_eq!(
        repo.get_paged(&PageRequest::new(1, 2))
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    assert_eq!(repo.find(&|_| true).await.unwrap().len(), 3);
    assert!(repo.first_or_default(&|_| true).await.unwrap().is_some());
    assert!(repo.any(&|_| true).await.unwrap());
    rename(&mut a);
    repo.update(&mut a).await.unwrap();
    repo.delete(&mut a).await.unwrap();
    assert_eq!(repo.count(Some(&|_| true)).await.unwrap(), 2);
    let all = repo.get_paged(&PageRequest::default()).await.unwrap();
    assert_eq!(all.total_count, 2);
}

#[tokio::test]
async fn customer_repository_finders() {
    let db = Db::in_memory().await.unwrap();
    let repo = CustomerRepositoryImpl::new(db.clone(), Arc::new(CustomerDaoImpl::new(db.clone())));
    repo.add_range(vec![
        customer("C002", "Zeta Corp", Some("0912-000"), true),
        customer("C001", "alpha ltd", None, true),
        customer("C003", "Gone Inc", Some("0912-111"), false),
    ])
    .await
    .unwrap();
    assert_eq!(
        repo.get_by_code("C001").await.unwrap().unwrap().name,
        "alpha ltd"
    );
    assert_eq!(repo.get_by_code("nope").await.unwrap(), None);

    let blank: Vec<_> = repo
        .search("  ", 20)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.code)
        .collect();
    // active only, ordered by Name with SQLite's binary collation (as EF Core on SQLite):
    // "Zeta Corp" sorts before "alpha ltd".
    assert_eq!(blank, ["C002", "C001"]);
    assert_eq!(repo.search("", 1).await.unwrap().len(), 1, "max_results");
    let by_name: Vec<_> = repo
        .search("ALPHA", 20)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.code)
        .collect();
    assert_eq!(by_name, ["C001"], "case-insensitive name match");
    assert_eq!(
        repo.search("c002", 20).await.unwrap().len(),
        1,
        "code match"
    );
    assert_eq!(
        repo.search("0912", 20).await.unwrap().len(),
        1,
        "phone match, inactive excluded"
    );

    let id = repo.get_by_code("C001").await.unwrap().unwrap().base.id;
    assert!(repo.code_exists("C001", None).await.unwrap());
    assert!(!repo.code_exists("C001", Some(id)).await.unwrap());
    assert!(!repo.code_exists("C999", None).await.unwrap());

    let fresh = CustomerRepositoryImpl::with_db(Db::in_memory().await.unwrap());
    exercise_inherited(&fresh, |c| customer(c, c, None, true), |c| c.name.push('!')).await;
}

#[tokio::test]
async fn product_repository_finders() {
    let db = Db::in_memory().await.unwrap();
    let cats = SqliteRepository::<ProductCategory>::new(db.clone());
    let mk = |code: &str, name: &str, sort: i32| ProductCategory {
        code: code.into(),
        name: name.into(),
        sort_order: sort,
        ..ProductCategory::default()
    };
    let tools = cats.add(mk("T", "Tools", 2)).await.unwrap();
    cats.add(mk("B", "Books", 1)).await.unwrap();
    cats.add(mk("A", "Art", 2)).await.unwrap();
    let mut gone = cats.add(mk("G", "Gone", 0)).await.unwrap();
    cats.delete(&mut gone).await.unwrap();

    let repo = ProductRepositoryImpl::new(db.clone(), Arc::new(ProductDaoImpl::new(db.clone())));
    let mut inactive = product("P3", "Hammer Old", Some("999"), Some(tools.base.id));
    inactive.is_active = false;
    repo.add_range(vec![
        product("P2", "Wrench", Some("4711"), Some(tools.base.id)),
        product("P1", "Hammer", None, Some(tools.base.id)),
        inactive,
    ])
    .await
    .unwrap();

    assert_eq!(
        repo.get_by_code("P1").await.unwrap().unwrap().name,
        "Hammer"
    );
    assert_eq!(
        repo.get_by_barcode("4711").await.unwrap().unwrap().code,
        "P2"
    );
    assert_eq!(repo.get_by_barcode("0").await.unwrap(), None);
    let names: Vec<_> = repo
        .get_by_category(tools.base.id)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["Hammer", "Wrench"]);
    assert_eq!(repo.search("", 20).await.unwrap().len(), 2);
    assert_eq!(repo.search("hAmMeR", 20).await.unwrap().len(), 1);
    assert_eq!(repo.search("471", 20).await.unwrap()[0].code, "P2");
    assert_eq!(repo.search("p2", 20).await.unwrap().len(), 1);
    assert!(repo.code_exists("P1", None).await.unwrap());
    let p1 = repo.get_by_code("P1").await.unwrap().unwrap().base.id;
    assert!(!repo.code_exists("P1", Some(p1)).await.unwrap());
    let cat_names: Vec<_> = repo
        .get_categories()
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(
        cat_names,
        ["Books", "Art", "Tools"],
        "SortOrder then Name, deleted excluded"
    );

    let fresh = ProductRepositoryImpl::with_db(Db::in_memory().await.unwrap());
    exercise_inherited(&fresh, |c| product(c, c, None, None), |p| p.name.push('!')).await;
}

async fn order_fixture() -> (Db, OrderRepositoryImpl, i32, i32) {
    let db = Db::in_memory().await.unwrap();
    let c = CustomerRepositoryImpl::with_db(db.clone())
        .add(customer("C", "Cust", None, true))
        .await
        .unwrap();
    let c2 = CustomerRepositoryImpl::with_db(db.clone())
        .add(customer("D", "Other", None, true))
        .await
        .unwrap();
    let p = ProductRepositoryImpl::with_db(db.clone())
        .add(product("P", "Prod", None, None))
        .await
        .unwrap();
    assert_eq!(c2.base.id, c.base.id + 1);
    let repo = OrderRepositoryImpl::new(db.clone(), Arc::new(OrderDaoImpl::new(db.clone())));
    (db, repo, c.base.id, p.base.id)
}

fn new_order(number: &str, customer_id: i32, days_ago: i64, status: OrderStatus) -> Order {
    Order {
        order_number: number.into(),
        order_date: today() - Duration::days(days_ago),
        customer_id,
        customer_name: "Cust".into(),
        status,
        ..Order::default()
    }
}

fn item(line: i32, product_id: i32) -> OrderItem {
    OrderItem {
        line_number: line,
        product_id,
        product_code: "P".into(),
        product_name: "Prod".into(),
        quantity: dec!(2),
        unit_price: dec!(10),
        ..OrderItem::default()
    }
}

#[tokio::test]
async fn order_aggregate_items_are_persisted_and_loaded() {
    let (db, repo, cid, pid) = order_fixture().await;
    let mut o = new_order("ORD1", cid, 0, OrderStatus::Draft);
    o.items = vec![item(1, pid), item(2, pid)];
    o.calculate_totals();
    let mut saved = repo.add(o).await.unwrap();
    assert!(
        saved
            .items
            .iter()
            .all(|i| i.base.id > 0 && i.order_id == saved.base.id)
    );

    let loaded = repo
        .get_by_id_with_items(saved.base.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.items.len(), 2);
    assert_eq!(loaded.subtotal, dec!(40));
    assert_eq!(loaded.customer.as_ref().map(|c| c.code.as_str()), Some("C"));
    assert_eq!(repo.get_by_id_with_items(999).await.unwrap(), None);

    // update: modify one existing item, add a new one
    saved.items[0].quantity = dec!(5);
    saved.items.push(item(3, pid));
    repo.update(&mut saved).await.unwrap();
    let loaded = repo.get_by_order_number("ORD1").await.unwrap().unwrap();
    assert_eq!(loaded.items.len(), 3);
    assert_eq!(loaded.items[0].quantity, dec!(5));
    assert_eq!(repo.get_by_order_number("missing").await.unwrap(), None);

    // soft-deleted items are not loaded
    let items = SqliteRepository::<OrderItem>::new(db.clone());
    let mut first = loaded.items[0].clone();
    items.delete(&mut first).await.unwrap();
    let loaded = repo
        .get_by_id_with_items(saved.base.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.items.len(), 2);

    // duplicate line number violates the (OrderId, LineNumber) unique index → nothing written
    let mut bad = new_order("ORD2", cid, 0, OrderStatus::Draft);
    bad.items = vec![item(1, pid), item(1, pid)];
    assert_eq!(
        repo.add(bad).await.unwrap_err().code(),
        ErrorCode::DuplicateEntry
    );
    assert_eq!(repo.get_by_order_number("ORD2").await.unwrap(), None);

    // add_range persists each order's items
    let mut a = new_order("ORD3", cid, 0, OrderStatus::Draft);
    a.items = vec![item(1, pid)];
    repo.add_range(vec![a]).await.unwrap();
    assert_eq!(
        repo.get_by_order_number("ORD3")
            .await
            .unwrap()
            .unwrap()
            .items
            .len(),
        1
    );
    let mut b = new_order("ORD4", cid, 0, OrderStatus::Draft);
    b.items = vec![item(1, pid), item(1, pid)];
    assert!(repo.add_range(vec![b]).await.is_err());

    // update of a missing order fails before touching items
    let mut ghost = new_order("G", cid, 0, OrderStatus::Draft);
    ghost.base.id = 999;
    assert_eq!(
        repo.update(&mut ghost).await.unwrap_err().code(),
        ErrorCode::ConflictError
    );
}

#[tokio::test]
async fn order_number_generation() {
    let (db, repo, cid, _) = order_fixture().await;
    let prefix = format!("ORD{}", today().format("%Y%m%d"));
    assert_eq!(
        repo.generate_order_number().await.unwrap(),
        format!("{prefix}0001")
    );
    let mut o = repo
        .add(new_order(
            &format!("{prefix}0007"),
            cid,
            0,
            OrderStatus::Draft,
        ))
        .await
        .unwrap();
    assert_eq!(
        repo.generate_order_number().await.unwrap(),
        format!("{prefix}0008")
    );
    // deleted orders still reserve their number (IgnoreQueryFilters in C#)
    repo.delete(&mut o).await.unwrap();
    assert_eq!(
        repo.generate_order_number().await.unwrap(),
        format!("{prefix}0008")
    );
    // a malformed suffix is reported, not panicked on (C# int.Parse throws)
    SqliteRepository::<Order>::new(db)
        .add(new_order(
            &format!("{prefix}XYZ"),
            cid,
            0,
            OrderStatus::Draft,
        ))
        .await
        .unwrap();
    let err = repo.generate_order_number().await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::DataCorruption);
}

#[tokio::test]
async fn order_paged_filters_and_sorting() {
    let (_db, repo, cid, _) = order_fixture().await;
    let other = cid + 1;
    repo.add_range(vec![
        new_order("A", cid, 0, OrderStatus::Draft),
        new_order("B", cid, 1, OrderStatus::Confirmed),
        new_order("C", other, 2, OrderStatus::Draft),
        new_order("D", cid, 5, OrderStatus::Draft),
    ])
    .await
    .unwrap();
    let req = PageRequest::new(1, 10);
    let all = repo
        .get_paged_with_filters(&req, &OrderFilter::default())
        .await
        .unwrap();
    let nums: Vec<_> = all.items.iter().map(|o| o.order_number.as_str()).collect();
    assert_eq!(nums, ["A", "B", "C", "D"], "default: newest first");

    let f = OrderFilter {
        status: Some(OrderStatus::Draft),
        customer_id: Some(cid),
        from_date: Some(today() - Duration::days(3)),
        to_date: Some(today()),
    };
    let page = repo.get_paged_with_filters(&req, &f).await.unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items[0].order_number, "A");

    let mut sorted = PageRequest::new(2, 2);
    sorted.sort_by = Some("OrderNumber".into());
    let p2 = repo
        .get_paged_with_filters(&sorted, &OrderFilter::default())
        .await
        .unwrap();
    let nums: Vec<_> = p2.items.iter().map(|o| o.order_number.as_str()).collect();
    assert_eq!(nums, ["C", "D"]);
    assert_eq!(p2.total_count, 4);
    sorted.sort_by = Some("Bogus".into());
    assert!(
        repo.get_paged_with_filters(&sorted, &OrderFilter::default())
            .await
            .is_err()
    );

    let fresh_db = Db::in_memory().await.unwrap();
    let fresh = OrderRepositoryImpl::with_db(fresh_db.clone());
    // inherited ops need a customer row for the FK
    CustomerRepositoryImpl::with_db(fresh_db)
        .add(customer("C", "C", None, true))
        .await
        .unwrap();
    exercise_inherited(
        &fresh,
        |n| new_order(n, 1, 0, OrderStatus::Draft),
        |o| o.notes = Some("x".into()),
    )
    .await;
}
