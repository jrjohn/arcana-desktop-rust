//! C# `MenuRegistryTests`. Ported all but `Constructor_WithLogger_ShouldWork` (no injected
//! logger). `MenusChanged` is checked through `revision()`.

use arcana_core::AppError;
use arcana_plugin_api::{MenuItemDefinition, MenuLocation, MenuRegistry, validate_menu_item};
use arcana_plugin_host::services::menu::MenuRegistryImpl;

fn valid(id: &str) -> MenuItemDefinition {
    MenuItemDefinition::new(id, &format!("Title for {id}"), MenuLocation::MainMenu)
}

// RegisterMenuItem_ValidItem_ShouldRegisterSuccessfully
#[test]
fn register_valid() {
    let r = MenuRegistryImpl::new();
    let item = valid("menu.test");
    let _s = r.register_menu_item(item.clone()).unwrap();
    assert!(r.all_menu_items().contains(&item));
}

// RegisterMenuItem_DuplicateId_ShouldThrowInvalidOperationException
#[test]
fn register_duplicate() {
    let r = MenuRegistryImpl::new();
    let _s = r.register_menu_item(valid("menu.test")).unwrap();
    assert!(matches!(
        r.register_menu_item(valid("menu.test")),
        Err(AppError::Plugin { .. })
    ));
}

// RegisterMenuItem_InvalidId_ShouldThrowContributionValidationException
#[test]
fn register_invalid_id() {
    let r = MenuRegistryImpl::new();
    let e = r
        .register_menu_item(MenuItemDefinition::new(
            "123invalid",
            "Test",
            MenuLocation::MainMenu,
        ))
        .unwrap_err();
    assert!(matches!(e, AppError::Validation { ref errors, .. } if !errors.is_empty()));
}

// RegisterMenuItem_EmptyTitle_ShouldThrowContributionValidationException
#[test]
fn register_empty_title() {
    let r = MenuRegistryImpl::new();
    let e = r
        .register_menu_item(MenuItemDefinition::new(
            "menu.test",
            "",
            MenuLocation::MainMenu,
        ))
        .unwrap_err();
    assert!(
        matches!(e, AppError::Validation { ref errors, .. } if errors.iter().any(|m| m.contains("Title")))
    );
}

// RegisterMenuItem_ShouldRaiseMenusChangedEvent
#[test]
fn register_bumps_revision() {
    let r = MenuRegistryImpl::new();
    let before = r.revision();
    let _s = r.register_menu_item(valid("menu.test")).unwrap();
    assert!(r.revision() > before);
}

// RegisterMenuItem_WithWarnings_ShouldLogWarnings — the warning text that the registry logs
// (via `tracing`) is asserted on the validator result; registration still succeeds.
#[test]
fn register_with_warning_succeeds() {
    let r = MenuRegistryImpl::new();
    let item = valid("menu.test").with_order(-1);
    let w = validate_menu_item(&item).warnings;
    assert_eq!(w.iter().filter(|m| m.contains("negative order")).count(), 1);
    assert!(r.register_menu_item(item).is_ok());
}

// RegisterMenuItems_ValidItems_ShouldRegisterAll
#[test]
fn register_many() {
    let r = MenuRegistryImpl::new();
    let _s = r
        .register_menu_items(vec![
            valid("menu.test1"),
            valid("menu.test2"),
            valid("menu.test3"),
        ])
        .unwrap();
    assert_eq!(r.all_menu_items().len(), 3);
}

// RegisterMenuItems_OneInvalidItem_ShouldNotRegisterAny
#[test]
fn register_many_one_invalid() {
    let r = MenuRegistryImpl::new();
    let e = r.register_menu_items(vec![
        valid("menu.test1"),
        MenuItemDefinition::new("123invalid", "Test", MenuLocation::MainMenu),
        valid("menu.test3"),
    ]);
    assert!(matches!(e, Err(AppError::Validation { .. })));
    assert!(r.all_menu_items().is_empty());
}

// RegisterMenuItems_ShouldRaiseMenusChangedEventOnce
#[test]
fn register_many_bumps_once() {
    let r = MenuRegistryImpl::new();
    let before = r.revision();
    let _s = r
        .register_menu_items(vec![valid("menu.test1"), valid("menu.test2")])
        .unwrap();
    assert_eq!(r.revision(), before + 1);
}

// Dispose_Subscription_ShouldRemoveMenuItem
#[test]
fn dispose_removes() {
    let r = MenuRegistryImpl::new();
    r.register_menu_item(valid("menu.test")).unwrap().dispose();
    assert!(r.all_menu_items().is_empty());
}

// Dispose_Subscription_ShouldRaiseMenusChangedEvent
#[test]
fn dispose_bumps_revision() {
    let r = MenuRegistryImpl::new();
    let s = r.register_menu_item(valid("menu.test")).unwrap();
    let before = r.revision();
    s.dispose();
    assert!(r.revision() > before);
}

// Dispose_MultipleItemsSubscription_ShouldRemoveAllItems
#[test]
fn dispose_many() {
    let r = MenuRegistryImpl::new();
    r.register_menu_items(vec![valid("menu.test1"), valid("menu.test2")])
        .unwrap()
        .dispose();
    assert!(r.all_menu_items().is_empty());
}

// GetMenuItems_ByLocation_ShouldFilterCorrectly
#[test]
fn filter_by_location() {
    let r = MenuRegistryImpl::new();
    let _s = r
        .register_menu_items(vec![
            MenuItemDefinition::new("main1", "Main 1", MenuLocation::MainMenu),
            MenuItemDefinition::new("file1", "File 1", MenuLocation::FileMenu),
            MenuItemDefinition::new("main2", "Main 2", MenuLocation::MainMenu),
        ])
        .unwrap();
    assert_eq!(r.menu_items(MenuLocation::MainMenu).len(), 2);
    assert_eq!(r.menu_items(MenuLocation::FileMenu).len(), 1);
}

// GetMenuItems_ByLocationAndModuleId_ShouldFilterCorrectly
#[test]
fn filter_by_module() {
    let r = MenuRegistryImpl::new();
    let q = MenuLocation::ModuleQuickAccess;
    let _s = r
        .register_menu_items(vec![
            MenuItemDefinition::new("mod1.item1", "Item 1", q).with_module("Module1"),
            MenuItemDefinition::new("mod1.item2", "Item 2", q).with_module("Module1"),
            MenuItemDefinition::new("mod2.item1", "Item 1", q).with_module("Module2"),
        ])
        .unwrap();
    assert_eq!(r.menu_items_for_module(q, "Module1").len(), 2);
    assert_eq!(r.menu_items_for_module(q, "Module2").len(), 1);
}

// GetMenuItems_ShouldOrderByOrderProperty
#[test]
fn ordered_by_order() {
    let r = MenuRegistryImpl::new();
    let m = MenuLocation::MainMenu;
    let _s = r
        .register_menu_items(vec![
            MenuItemDefinition::new("item3", "Third", m).with_order(30),
            MenuItemDefinition::new("item1", "First", m).with_order(10),
            MenuItemDefinition::new("item2", "Second", m).with_order(20),
        ])
        .unwrap();
    let ids: Vec<_> = r.menu_items(m).into_iter().map(|i| i.id).collect();
    assert_eq!(ids, ["item1", "item2", "item3"]);
}

// Constructor_WithoutLogger_ShouldWork
#[test]
fn constructor() {
    assert!(MenuRegistryImpl::default().all_menu_items().is_empty());
}
