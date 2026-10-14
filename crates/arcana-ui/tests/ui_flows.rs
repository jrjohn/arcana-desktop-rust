//! `egui_kittest` UI tests of the main flows, driving the real shell through AccessKit:
//! open order list → open detail in a tab → edit → save; switch language; switch theme.
//! No renderer is needed (runs headless in CI).

use std::sync::Arc;
use std::time::Duration;

use arcana_infra::network::{ProbeNetworkMonitor, StaticProbe};
use arcana_infra::security::TokenServiceOptions;
use arcana_infra::wiring::{ArcanaConfig, ArcanaServices};
use arcana_ui::nav::Route;
use arcana_ui::pages::Page;
use arcana_ui::{Shell, UiServices};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const WAIT: Duration = Duration::from_secs(10);

struct Env {
    _rt: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

fn harness() -> (Harness<'static, Shell>, Env) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config = ArcanaConfig {
        token_key: b"kittest-key-0123456789abcdefghij".to_vec(),
        token_options: TokenServiceOptions::default(),
        settings_path: dir.path().join("prefs.json"),
        app_settings_path: dir.path().join("settings.json"),
    };
    let svc = rt.block_on(async {
        let s = ArcanaServices::in_memory(config).await.unwrap();
        arcana_infra::seed::seed_demo_data(&s).await.unwrap();
        s
    });
    let ui = UiServices::from_arcana(&svc, Arc::new(ProbeNetworkMonitor::new(StaticProbe(true))));
    ui.app_settings.set_language_code("en-US");
    let shell = Shell::new(ui, rt.handle().clone());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.show(ui), shell);
    settle(&mut h);
    (h, Env { _rt: rt, _dir: dir })
}

/// Let async tasks finish and the UI catch up.
fn settle(h: &mut Harness<'_, Shell>) {
    for _ in 0..3 {
        h.run_steps(2);
        h.state_mut().settle(WAIT);
    }
    h.run_steps(2);
}

#[test]
fn order_list_detail_edit_save() {
    let (mut h, _env) = harness();
    h.get_by_label_contains("Order Management").click();
    settle(&mut h);
    assert_eq!(h.state().focused_route(), Some(Route::OrderList));
    for col in [
        "Order Number",
        "Order Date",
        "Customer",
        "Amount",
        "Payment Status",
        "Order Status",
        "Actions",
    ] {
        assert!(h.query_all_by_label_contains(col).next().is_some(), "{col}");
    }
    assert!(h.query_all_by_label("Showing 1-10 of 10").next().is_some());

    // The first row's edit button opens the order in a new tab.
    h.get_all_by_label("✏").next().unwrap().click();
    settle(&mut h);
    let tab = h.state().focused_tab().unwrap();
    assert!(matches!(
        h.state().focused_route(),
        Some(Route::OrderDetail { id: Some(_), .. })
    ));
    assert!(h.query_all_by_label("Order Information").next().is_some());
    assert!(h.query_all_by_label("Order Summary").next().is_some());

    h.get_by_label("✏ Edit").click();
    h.run_steps(2);
    let before = match h.state_mut().page_mut(tab) {
        Some(Page::OrderDetail(vm)) => {
            assert!(vm.state.is_editing);
            vm.state.order.total_amount
        }
        _ => panic!("not an order detail"),
    };
    h.get_all_by_label("+").next().unwrap().click();
    h.run_steps(2);
    h.get_by_label("💾 Save").click();
    settle(&mut h);
    match h.state_mut().page_mut(tab) {
        Some(Page::OrderDetail(vm)) => {
            assert!(
                !vm.state.is_editing && !vm.state.is_dirty,
                "{:?}",
                vm.state.validation
            );
            assert!(vm.state.order.total_amount > before);
        }
        _ => panic!("not an order detail"),
    }
}

#[test]
fn new_order_shows_domain_validation() {
    let (mut h, _env) = harness();
    h.get_by_label("➕\n\nNew Order").click();
    settle(&mut h);
    h.get_by_label("💾 Save").click();
    h.run_steps(2);
    assert!(
        h.query_all_by_label_contains("Please select a customer")
            .next()
            .is_some()
    );
}

#[test]
fn switch_language_live() {
    let (mut h, _env) = harness();
    h.get_by_label_contains("Settings").click();
    settle(&mut h);
    h.get_by_value("English").click();
    h.run_steps(2);
    h.get_by_label("繁體中文").click();
    settle(&mut h);
    assert_eq!(h.state().services().localization.current_culture(), "zh-TW");
    assert!(
        h.query_all_by_label_contains("首頁").next().is_some(),
        "side nav switched"
    );
    assert!(
        h.query_all_by_label("設定").next().is_some(),
        "page title switched"
    );
    h.get_by_value("繁體中文").click();
    h.run_steps(2);
    h.get_by_label("日本語").click();
    settle(&mut h);
    assert_eq!(h.state().services().app_settings.language_code(), "ja-JP");
    assert!(h.query_all_by_label_contains("ホーム").next().is_some());
}

#[test]
fn switch_theme_and_accent() {
    let (mut h, _env) = harness();
    h.get_by_label_contains("Settings").click();
    settle(&mut h);
    h.get_by_label("Dark").click();
    settle(&mut h);
    assert_eq!(h.state().theme_id(), "Dark");
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    h.get_by_label("Forest Green").click();
    settle(&mut h);
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    assert_eq!(h.state().services().app_settings.theme_id(), "ForestGreen");
    h.get_by_label("accent-1").click();
    settle(&mut h);
    assert_eq!(h.state().accent(), Some(1));
}

#[test]
fn plugin_manager_lists_built_ins_and_menus_work() {
    let (mut h, _env) = harness();
    h.get_by_label_contains("Plugin Manager").click();
    settle(&mut h);
    for name in [
        "Core Menu",
        "System",
        "Order Module",
        "Customer Module",
        "Product Module",
    ] {
        assert!(h.query_all_by_label(name).next().is_some(), "{name}");
    }
    h.get_by_label("Order Module").click();
    h.run_steps(2);
    assert!(h.query_all_by_label("arcana.module.order").next().is_some());
    // Menu bar → Business → product list goes through the command registry.
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Business")
        .click();
    h.run_steps(2);
    h.get_by_label("Product Management").click();
    settle(&mut h);
    assert_eq!(h.state().focused_route(), Some(Route::ProductList));
    assert!(h.query_all_by_label("P001").next().is_some());
}
