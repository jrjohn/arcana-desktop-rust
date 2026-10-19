//! Shell logic without rendering: navigation, tabs, effects, commands, dialogs.

use std::sync::Arc;
use std::time::Duration;

use arcana_infra::network::{ProbeNetworkMonitor, StaticProbe};
use arcana_infra::security::TokenServiceOptions;
use arcana_infra::wiring::{ArcanaConfig, ArcanaServices};

use super::*;
use crate::effect::NoticeLevel;
use crate::pages::{order_detail, order_list};

const WAIT: Duration = Duration::from_secs(10);

struct Fixture {
    shell: Shell,
    _rt: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config = ArcanaConfig {
        token_key: b"shell-test-key-0123456789abcdef!".to_vec(),
        token_options: TokenServiceOptions::default(),
        settings_path: dir.path().join("prefs.json"),
        app_settings_path: dir.path().join("settings.json"),
    };
    let svc = rt.block_on(async {
        let s = ArcanaServices::in_memory(config).await.unwrap();
        arcana_infra::seed::seed_demo_data(&s).await.unwrap();
        s
    });
    let ui = UiServices::from_arcana(
        &svc,
        Arc::new(ProbeNetworkMonitor::new(StaticProbe(true))),
        dir.path(),
    );
    ui.app_settings.set_language_code("en-US");
    let mut shell = Shell::new(ui, rt.handle().clone());
    shell.settle(WAIT);
    Fixture {
        shell,
        _rt: rt,
        _dir: dir,
    }
}

fn order_list_state(shell: &mut Shell, tab: TabId) -> order_list::State {
    match shell.page_mut(tab) {
        Some(Page::OrderList(vm)) => vm.state.clone(),
        other => panic!("{other:?}"),
    }
}

fn order_detail_state(shell: &mut Shell, tab: TabId) -> order_detail::State {
    match shell.page_mut(tab) {
        Some(Page::OrderDetail(vm)) => vm.state.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn starts_on_home_with_dashboard_loaded() {
    let mut f = fixture();
    assert_eq!(f.shell.focused_route(), Some(Route::Home));
    let home = f.shell.focused_tab().unwrap();
    let Some(Page::Home(vm)) = f.shell.page_mut(home) else {
        panic!()
    };
    assert_eq!(vm.state.dashboard.total_customers, 5);
    assert_eq!(vm.state.dashboard.recent.len(), 5);
    assert!(!vm.state.loading);
    assert_eq!(f.shell.theme_id(), "System");
    assert!(format!("{:?}", f.shell).contains("Shell"));
}

#[test]
fn open_order_list_then_detail_in_new_tab_edit_and_save() {
    let mut f = fixture();
    f.shell.execute(CommandId::OrderList);
    f.shell.settle(WAIT);
    let list = f.shell.focused_tab().unwrap();
    let s = order_list_state(&mut f.shell, list);
    assert_eq!(s.total_count, 10);
    assert_eq!(s.orders.len(), 10);
    // Same command focuses the existing tab instead of opening another.
    f.shell.execute(CommandId::Home);
    f.shell.execute(CommandId::OrderList);
    assert_eq!(f.shell.focused_tab(), Some(list));
    assert_eq!(f.shell.tabs().len(), 2);

    let id = s.orders[0].base.id;
    f.shell
        .send(list, PageMsg::OrderList(order_list::Input::EditOrder(id)));
    f.shell.settle(WAIT);
    let detail = f.shell.focused_tab().unwrap();
    assert_ne!(detail, list);
    let d = order_detail_state(&mut f.shell, detail);
    assert_eq!(d.order.base.id, id);
    assert_ne!(d.order.items.len(), 0);
    let before = d.order.total_amount;

    f.shell
        .send(detail, PageMsg::OrderDetail(order_detail::Input::Edit));
    f.shell.send(
        detail,
        PageMsg::OrderDetail(order_detail::Input::IncreaseQuantity(0)),
    );
    assert!(f.shell.tabs()[&detail].page.is_dirty());
    f.shell.execute(CommandId::Save);
    f.shell.settle(WAIT);
    let d = order_detail_state(&mut f.shell, detail);
    assert!(!d.is_editing && !d.is_dirty, "{:?}", d.validation);
    assert!(d.order.total_amount > before);
    assert!(
        f.shell
            .toasts()
            .iter()
            .any(|t| t.notice.level == NoticeLevel::Success)
    );
    // The list refreshed via DataChanged.
    let s = order_list_state(&mut f.shell, list);
    assert!(
        s.orders
            .iter()
            .any(|o| o.base.id == id && o.total_amount == d.order.total_amount)
    );
    // Opening the same order again focuses its tab.
    f.shell
        .navigate(&crate::nav::NavGraph::to_order_detail(id, true), None);
    assert_eq!(f.shell.focused_tab(), Some(detail));
}

#[test]
fn new_order_validation_messages_come_from_domain() {
    let mut f = fixture();
    f.shell.execute(CommandId::NewOrder);
    f.shell.settle(WAIT);
    let tab = f.shell.focused_tab().unwrap();
    let d = order_detail_state(&mut f.shell, tab);
    assert!(d.is_new && d.order.order_number.starts_with("ORD"));
    f.shell.execute(CommandId::Save);
    let d = order_detail_state(&mut f.shell, tab);
    assert!(
        d.validation
            .contains(&"Please select a customer".to_owned())
    );
    // Cancel on a clean new order closes the tab.
    f.shell
        .send(tab, PageMsg::OrderDetail(order_detail::Input::Cancel));
    assert!(!f.shell.tabs().contains_key(&tab));
}

#[test]
fn create_full_order_through_the_ui_model() {
    let mut f = fixture();
    f.shell.execute(CommandId::NewOrder);
    f.shell.settle(WAIT);
    let tab = f.shell.focused_tab().unwrap();
    let customer = order_detail_state(&mut f.shell, tab).customers[0].base.id;
    f.shell.send(
        tab,
        PageMsg::OrderDetail(order_detail::Input::SelectCustomer(customer)),
    );
    f.shell.send(
        tab,
        PageMsg::OrderDetail(order_detail::Input::ProductSearchChanged("P00".into())),
    );
    f.shell.send(
        tab,
        PageMsg::OrderDetail(order_detail::Input::SearchProducts),
    );
    f.shell.settle(WAIT);
    let product = order_detail_state(&mut f.shell, tab).products[0].base.id;
    f.shell.send(
        tab,
        PageMsg::OrderDetail(order_detail::Input::AddItem(product)),
    );
    f.shell
        .send(tab, PageMsg::OrderDetail(order_detail::Input::Save));
    f.shell.settle(WAIT);
    let d = order_detail_state(&mut f.shell, tab);
    assert!(!d.is_new && d.order_id.is_some(), "{:?}", d.validation);
    assert_eq!(
        f.shell.tabs()[&tab].page.route(),
        Route::OrderDetail {
            id: d.order_id,
            read_only: false
        }
    );
}

#[test]
fn delete_from_list_asks_then_removes() {
    let mut f = fixture();
    f.shell.execute(CommandId::OrderList);
    f.shell.settle(WAIT);
    let list = f.shell.focused_tab().unwrap();
    let id = order_list_state(&mut f.shell, list).orders[0].base.id;
    f.shell
        .send(list, PageMsg::OrderList(order_list::Input::DeleteOrder(id)));
    assert!(matches!(f.shell.dialog(), Some(Dialog::Delete { .. })));
    f.shell.answer_dialog(false);
    assert!(f.shell.dialog().is_none());
    f.shell
        .send(list, PageMsg::OrderList(order_list::Input::DeleteOrder(id)));
    f.shell.answer_dialog(true);
    f.shell.settle(WAIT);
    assert_eq!(order_list_state(&mut f.shell, list).total_count, 9);
    f.shell.answer_dialog(true);
}

#[test]
fn dirty_tab_close_asks_and_discard_confirms() {
    let mut f = fixture();
    f.shell.execute(CommandId::NewOrder);
    f.shell.settle(WAIT);
    let tab = f.shell.focused_tab().unwrap();
    f.shell.send(
        tab,
        PageMsg::OrderDetail(order_detail::Input::NotesChanged("x".into())),
    );
    f.shell.execute(CommandId::CloseTab);
    assert_eq!(f.shell.dialog(), Some(&Dialog::CloseDirty { tab }));
    f.shell.answer_dialog(true);
    assert!(!f.shell.tabs().contains_key(&tab));

    f.shell.execute(CommandId::NewOrder);
    f.shell.settle(WAIT);
    let tab = f.shell.focused_tab().unwrap();
    f.shell.send(
        tab,
        PageMsg::OrderDetail(order_detail::Input::NotesChanged("x".into())),
    );
    f.shell
        .send(tab, PageMsg::OrderDetail(order_detail::Input::Cancel));
    assert_eq!(f.shell.dialog(), Some(&Dialog::Discard { tab }));
    f.shell.answer_dialog(true);
    assert!(!f.shell.tabs().contains_key(&tab));
}

#[test]
fn pop_out_dock_back_reorder_and_within_tab_navigation() {
    let mut f = fixture();
    let home = f.shell.focused_tab().unwrap();
    f.shell.execute(CommandId::OrderList);
    f.shell.execute(CommandId::Settings);
    let settings_tab = f.shell.focused_tab().unwrap();
    assert_eq!(f.shell.docked_order().len(), 3);
    f.shell.reorder(settings_tab, 0);
    assert_eq!(f.shell.docked_order()[0], settings_tab);

    f.shell.execute(CommandId::PopOut);
    assert_eq!(f.shell.floating(), &[settings_tab]);
    assert_eq!(f.shell.docked_order().len(), 2);
    f.shell.pop_out(settings_tab);
    // Navigating to a floating document does not re-dock it.
    f.shell.execute(CommandId::Settings);
    assert_eq!(f.shell.floating(), &[settings_tab]);
    f.shell.dock_back(settings_tab);
    assert_eq!(f.shell.floating().len(), 0);
    assert_eq!(f.shell.docked_order().len(), 3);

    f.shell
        .navigate(&NavRequest::within_tab(Route::PluginManager), Some(home));
    assert_eq!(f.shell.tabs()[&home].page.route(), Route::PluginManager);
    assert!(f.shell.back(home));
    assert_eq!(f.shell.tabs()[&home].page.route(), Route::Home);
    assert!(!f.shell.back(home));

    f.shell.close_tab(settings_tab, false);
    assert_eq!(f.shell.tabs().len(), 2);
    f.shell.close_tab(TabId(999), false);
    f.shell.reorder(TabId(999), 0);
    f.shell.dock_back(TabId(999));
}

#[test]
fn theme_accent_and_language_effects_persist() {
    use crate::pages::settings::Input as S;
    let mut f = fixture();
    f.shell.execute(CommandId::Settings);
    let tab = f.shell.focused_tab().unwrap();
    f.shell
        .send(tab, PageMsg::Settings(S::SelectTheme("Dark".into())));
    f.shell
        .send(tab, PageMsg::Settings(S::SelectAccent(Some(3))));
    f.shell
        .send(tab, PageMsg::Settings(S::SelectLanguage("zh-TW".into())));
    f.shell
        .send(tab, PageMsg::Settings(S::FrequencyChanged(30)));
    f.shell.settle(WAIT);
    let svc = f.shell.services();
    assert_eq!(svc.app_settings.theme_id(), "Dark");
    assert_eq!(svc.app_settings.language_code(), "zh-TW");
    assert_eq!(svc.localization.current_culture(), "zh-TW");
    assert_eq!(
        svc.settings.get_value(ACCENT_KEY),
        Some(serde_json::json!(3))
    );
    assert_eq!(svc.app_settings.sync_frequency_minutes(), 30);
    assert_eq!((f.shell.theme_id(), f.shell.accent()), ("Dark", Some(3)));
    f.shell.send(tab, PageMsg::Settings(S::SelectAccent(None)));
    assert_eq!(f.shell.accent(), None);
}

#[test]
fn commands_cover_every_registry_entry() {
    let mut f = fixture();
    for c in crate::commands::COMMANDS {
        let _ = f.shell.is_enabled(c.id);
        if c.id != CommandId::CloseTab {
            f.shell.execute(c.id);
        }
    }
    f.shell.settle(WAIT);
    assert!(f.shell.dialog().is_some(), "About opened a dialog");
    f.shell.answer_dialog(true);
    assert!(!f.shell.is_enabled(CommandId::Undo));
    // Refresh on the focused list page reloads it.
    f.shell.execute(CommandId::ProductList);
    f.shell.settle(WAIT);
    f.shell.execute(CommandId::Refresh);
    f.shell.settle(WAIT);
    let tab = f.shell.focused_tab().unwrap();
    let Some(Page::ProductList(vm)) = f.shell.page_mut(tab) else {
        panic!()
    };
    assert_eq!(vm.state.total_count, 8);
}

#[test]
fn product_and_customer_crud_through_shell() {
    use crate::pages::entity_form::Input as F;
    let mut f = fixture();
    f.shell.execute(CommandId::NewCustomer);
    f.shell.settle(WAIT);
    let tab = f.shell.focused_tab().unwrap();
    f.shell.send(
        tab,
        PageMsg::CustomerDetail(F::FieldChanged(0, "C900".into())),
    );
    f.shell.send(
        tab,
        PageMsg::CustomerDetail(F::FieldChanged(1, "Acme".into())),
    );
    f.shell.send(tab, PageMsg::CustomerDetail(F::Save));
    f.shell.settle(WAIT);
    let Some(Page::CustomerDetail(vm)) = f.shell.page_mut(tab) else {
        panic!()
    };
    assert!(!vm.state.is_new, "{:?}", vm.state.validation);
    let id = vm.state.id.unwrap();
    f.shell.send(tab, PageMsg::CustomerDetail(F::Delete));
    f.shell.answer_dialog(true);
    f.shell.settle(WAIT);
    assert!(!f.shell.tabs().contains_key(&tab), "deleted form closes");
    assert!(
        f.shell
            .find_tab(&Route::CustomerDetail { id: Some(id) })
            .is_none()
    );

    f.shell.execute(CommandId::ProductList);
    f.shell.settle(WAIT);
    let list = f.shell.focused_tab().unwrap();
    let Some(Page::ProductList(vm)) = f.shell.page_mut(list) else {
        panic!()
    };
    let pid = vm.state.rows[0].base.id;
    f.shell.send(
        list,
        PageMsg::ProductList(crate::pages::entity_list::Input::Open(pid)),
    );
    f.shell.settle(WAIT);
    let form = f.shell.focused_tab().unwrap();
    let Some(Page::ProductDetail(vm)) = f.shell.page_mut(form) else {
        panic!()
    };
    assert_eq!(vm.state.options.len(), 3, "categories loaded");
    f.shell.send(
        form,
        PageMsg::ProductDetail(F::FieldChanged(5, "999".into())),
    );
    f.shell.execute(CommandId::Save);
    f.shell.settle(WAIT);
    let Some(Page::ProductList(vm)) = f.shell.page_mut(list) else {
        panic!()
    };
    assert!(
        vm.state
            .rows
            .iter()
            .any(|p| p.base.id == pid && p.price == rust_decimal::Decimal::from(999))
    );
    f.shell.send(
        list,
        PageMsg::ProductList(crate::pages::entity_list::Input::SearchTextChanged(
            "Mouse".into(),
        )),
    );
    f.shell.send(
        list,
        PageMsg::ProductList(crate::pages::entity_list::Input::Search),
    );
    f.shell.settle(WAIT);
    let Some(Page::ProductList(vm)) = f.shell.page_mut(list) else {
        panic!()
    };
    assert_eq!(vm.state.rows.len(), 1);

    f.shell.execute(CommandId::CustomerList);
    f.shell.settle(WAIT);
    let cl = f.shell.focused_tab().unwrap();
    f.shell.send(
        cl,
        PageMsg::CustomerList(crate::pages::entity_list::Input::SearchTextChanged(
            "Taipei".into(),
        )),
    );
    f.shell.send(
        cl,
        PageMsg::CustomerList(crate::pages::entity_list::Input::Search),
    );
    f.shell.settle(WAIT);
    let Some(Page::CustomerList(vm)) = f.shell.page_mut(cl) else {
        panic!()
    };
    assert_eq!(vm.state.rows.len(), 1);
}

#[test]
fn order_list_filters_hit_the_services() {
    let mut f = fixture();
    f.shell.execute(CommandId::OrderList);
    f.shell.settle(WAIT);
    let list = f.shell.focused_tab().unwrap();
    let all = order_list_state(&mut f.shell, list).orders;
    let customer = all[0].customer_name.clone();
    let expected = all.iter().filter(|o| o.customer_name == customer).count() as u64;
    f.shell.send(
        list,
        PageMsg::OrderList(order_list::Input::SearchTextChanged(customer)),
    );
    f.shell
        .send(list, PageMsg::OrderList(order_list::Input::Search));
    f.shell.settle(WAIT);
    assert_eq!(order_list_state(&mut f.shell, list).total_count, expected);

    let status = all[0].status;
    let expected = all.iter().filter(|o| o.status == status).count() as u64;
    f.shell
        .send(list, PageMsg::OrderList(order_list::Input::ClearFilter));
    f.shell.settle(WAIT);
    f.shell.send(
        list,
        PageMsg::OrderList(order_list::Input::StatusFilterChanged(Some(status))),
    );
    f.shell.settle(WAIT);
    assert_eq!(order_list_state(&mut f.shell, list).total_count, expected);

    f.shell
        .send(list, PageMsg::OrderList(order_list::Input::ClearFilter));
    f.shell.settle(WAIT);
    let today = arcana_core::today();
    f.shell.send(
        list,
        PageMsg::OrderList(order_list::Input::FromDateChanged(crate::fmt::iso_date(
            today - chrono::Duration::days(40),
        ))),
    );
    f.shell.send(
        list,
        PageMsg::OrderList(order_list::Input::ToDateChanged(crate::fmt::iso_date(
            today,
        ))),
    );
    f.shell
        .send(list, PageMsg::OrderList(order_list::Input::Search));
    f.shell.settle(WAIT);
    assert_eq!(order_list_state(&mut f.shell, list).total_count, 10);

    let target = all
        .iter()
        .find(|o| o.status != arcana_domain::OrderStatus::Completed)
        .unwrap();
    f.shell.send(
        list,
        PageMsg::OrderList(order_list::Input::ChangeStatus(target.base.id)),
    );
    f.shell.settle(WAIT);
    let s = order_list_state(&mut f.shell, list);
    let now = s
        .orders
        .iter()
        .find(|o| o.base.id == target.base.id)
        .unwrap();
    assert_eq!(now.status, crate::fmt::next_status(target.status));
}

#[test]
fn plugin_confirm_waits_for_the_answer_and_replies_through_the_command() {
    use arcana_plugin_api::WindowService as _;
    use std::sync::Mutex;
    let mut f = fixture();
    let rt = f.shell.services().plugins.clone();
    let answers: Arc<Mutex<Vec<bool>>> = Arc::default();
    let sink = answers.clone();
    let _sub = rt
        .host
        .services
        .commands
        .register_command(
            "test.reply",
            Arc::new(move |args| {
                let yes = args
                    .first()
                    .and_then(|a| a.downcast_ref::<bool>())
                    .copied()
                    .unwrap_or_default();
                sink.lock().unwrap().push(yes);
                Ok(())
            }),
        )
        .unwrap();
    rt.window.show_confirm("T1", "first?", Some("test.reply"));
    rt.window.show_confirm("T2", "second?", Some("test.reply"));
    f.shell.pump_plugins();
    assert!(matches!(f.shell.dialog(), Some(Dialog::PluginConfirm { title, .. }) if title == "T1"));
    assert_eq!(answers.lock().unwrap().len(), 0, "nothing answered yet");
    f.shell.answer_dialog(true);
    assert!(matches!(f.shell.dialog(), Some(Dialog::PluginConfirm { title, .. }) if title == "T2"));
    f.shell.answer_dialog(false);
    assert!(f.shell.dialog().is_none());
    assert_eq!(*answers.lock().unwrap(), vec![true, false]);
    rt.window.set_status_bar_item("p.status", Some("busy"));
    f.shell.pump_plugins();
    assert_eq!(
        f.shell.status_items().get("p.status").map(String::as_str),
        Some("busy")
    );
}

#[test]
fn home_quick_actions_and_function_tree_come_from_contributions() {
    use arcana_plugin_api::MenuLocation;
    let mut f = fixture();
    let home = f.shell.find_tab(&Route::Home).unwrap();
    let Some(Page::Home(vm)) = f.shell.page_mut(home) else {
        panic!()
    };
    let cmds: Vec<String> = vm
        .state
        .quick_actions
        .iter()
        .map(|a| a.command.clone())
        .collect();
    assert!(cmds.contains(&"order.new".to_owned()), "{cmds:?}");
    assert!(cmds.contains(&"flowchart.new".to_owned()), "{cmds:?}");
    let tree = super::plugins::function_tree(
        &f.shell
            .services()
            .plugins
            .host
            .services
            .menus
            .all_menu_items(),
        &["OrderListPage"],
    );
    assert!(
        tree.iter()
            .any(|c| c.command.as_deref() == Some("flowchart.new"))
    );
    let module = f
        .shell
        .contributed(MenuLocation::ModuleQuickAccess, Some("OrderModule"));
    assert!(module.iter().any(|c| c.id == "module.order.new"));
    // Clicking a contributed quick action runs the plugin command.
    let before = f.shell.tabs().len();
    f.shell.send(
        home,
        PageMsg::Home(crate::pages::home::Input::RunCommand(
            "flowchart.new".into(),
        )),
    );
    assert_eq!(f.shell.tabs().len(), before + 1);
}
