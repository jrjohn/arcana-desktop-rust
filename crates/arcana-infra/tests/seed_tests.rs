//! Demo data seeding (C# `App.SeedSampleDataAsync`) — not a C# test; M4 addition.

use arcana_core::PageRequest;
use arcana_data::Db;
use arcana_infra::security::TokenServiceOptions;
use arcana_infra::seed::seed_demo_data;
use arcana_infra::wiring::{ArcanaConfig, ArcanaServices};

fn config(dir: &std::path::Path) -> ArcanaConfig {
    ArcanaConfig {
        token_key: b"seed-test-key-0123456789abcdef!!".to_vec(),
        token_options: TokenServiceOptions::default(),
        settings_path: dir.join("prefs.json"),
        app_settings_path: dir.join("settings.json"),
    }
}

#[tokio::test]
async fn seeds_once_with_csharp_counts() {
    let dir = tempfile::tempdir().unwrap();
    let svc = ArcanaServices::new(Db::in_memory().await.unwrap(), config(dir.path())).unwrap();
    assert!(seed_demo_data(&svc).await.unwrap());
    assert!(
        !seed_demo_data(&svc).await.unwrap(),
        "second run is a no-op"
    );

    let page = PageRequest::new(1, 100);
    assert_eq!(
        svc.customers
            .get_customers(&page)
            .await
            .unwrap()
            .total_count,
        5
    );
    assert_eq!(
        svc.products.get_products(&page).await.unwrap().total_count,
        8
    );
    assert_eq!(svc.products.get_categories().await.unwrap().len(), 3);
    let orders = svc.orders.get_orders(&page).await.unwrap();
    assert_eq!(orders.total_count, 10);
    for o in &orders.items {
        let full = svc.orders.get_order_by_id(o.base.id).await.unwrap();
        assert!((1..=5).contains(&full.items.len()));
        assert!(full.order_number.starts_with("ORD-"));
        assert!(full.total_amount > rust_decimal::Decimal::ZERO);
    }
}

#[tokio::test]
async fn in_memory_wiring_works() {
    let dir = tempfile::tempdir().unwrap();
    let svc = ArcanaServices::in_memory(config(dir.path())).await.unwrap();
    assert!(seed_demo_data(&svc).await.unwrap());
}

#[tokio::test]
async fn open_creates_a_file_database() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", dir.path().join("a.db").display());
    let svc = ArcanaServices::open(&url, config(dir.path()))
        .await
        .unwrap();
    assert!(seed_demo_data(&svc).await.unwrap());
    drop(svc);
    let again = ArcanaServices::open(&url, config(dir.path()))
        .await
        .unwrap();
    assert!(!seed_demo_data(&again).await.unwrap(), "data persisted");
}
