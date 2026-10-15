//! C# `ViewRegistryTests` (24). Ported 22. Not ported:
//! `CreateViewInstance_WithViewClass_ShouldUseActivator` (no reflection / `Activator`; views
//! are built only through factories) and `Constructor_WithLogger_ShouldWork` (no injected
//! logger). `ViewsChanged` is checked through `revision()`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arcana_core::AppError;
use arcana_plugin_api::{ViewDefinition, ViewFactory, ViewInstance, ViewRegistry, validate_view};
use arcana_plugin_host::services::view::ViewRegistryImpl;

fn valid(id: &str) -> ViewDefinition {
    ViewDefinition {
        title_key: Some(format!("{}.title", id.to_lowercase())),
        view_class_name: Some("object".into()),
        ..ViewDefinition::new(id, &format!("Title for {id}"))
    }
}

fn factory(v: &'static str) -> ViewFactory {
    Arc::new(move || Box::new(v) as ViewInstance)
}

// RegisterView_ValidView_ShouldRegisterSuccessfully
#[test]
fn register_valid() {
    let r = ViewRegistryImpl::new();
    let _s = r.register_view(valid("TestPage")).unwrap();
    assert!(r.view("TestPage").is_some());
}

// RegisterView_DuplicateId_ShouldThrowInvalidOperationException
#[test]
fn register_duplicate() {
    let r = ViewRegistryImpl::new();
    let _s = r.register_view(valid("TestPage")).unwrap();
    assert!(matches!(
        r.register_view(valid("TestPage")),
        Err(AppError::Plugin { .. })
    ));
}

// RegisterView_InvalidId_ShouldThrowContributionValidationException
#[test]
fn register_invalid_id() {
    let v = ViewDefinition {
        id: "123invalid".into(),
        ..valid("x")
    };
    let e = ViewRegistryImpl::new().register_view(v).unwrap_err();
    assert!(matches!(e, AppError::Validation { ref errors, .. } if !errors.is_empty()));
}

// RegisterView_EmptyTitle_ShouldThrowContributionValidationException
#[test]
fn register_empty_title() {
    let v = ViewDefinition {
        title: String::new(),
        ..valid("TestPage")
    };
    let e = ViewRegistryImpl::new().register_view(v).unwrap_err();
    assert!(
        matches!(e, AppError::Validation { ref errors, .. } if errors.iter().any(|m| m.contains("Title")))
    );
}

// RegisterView_ShouldRaiseViewsChangedEvent
#[test]
fn register_bumps_revision() {
    let r = ViewRegistryImpl::new();
    let before = r.revision();
    let _s = r.register_view(valid("TestPage")).unwrap();
    assert!(r.revision() > before);
}

// RegisterView_WithWarnings_ShouldLogWarnings — asserted on the validator's warning (what the
// registry logs through `tracing`).
#[test]
fn register_without_title_key_warns() {
    let v = ViewDefinition {
        title_key: None,
        ..valid("TestPage")
    };
    let w = validate_view(&v).warnings;
    assert_eq!(w.iter().filter(|m| m.contains("TitleKey")).count(), 1);
    assert!(ViewRegistryImpl::new().register_view(v).is_ok());
}

// RegisterView_NoViewClass_ShouldLogWarning
#[test]
fn register_without_view_class_warns() {
    let v = ViewDefinition {
        view_class_name: None,
        ..valid("TestPage")
    };
    let w = validate_view(&v).warnings;
    assert_eq!(w.iter().filter(|m| m.contains("ViewClass")).count(), 1);
    assert!(ViewRegistryImpl::new().register_view(v).is_ok());
}

// RegisterViewFactory_ShouldStoreFactory
#[test]
fn factory_is_used() {
    let r = ViewRegistryImpl::new();
    let _s = r.register_view(valid("TestPage")).unwrap();
    let n = Arc::new(AtomicUsize::new(0));
    let n2 = n.clone();
    let _f = r.register_view_factory(
        "TestPage",
        Arc::new(move || {
            n2.fetch_add(1, Ordering::SeqCst);
            Box::new(()) as ViewInstance
        }),
    );
    let _ = r.create_view_instance("TestPage");
    assert_eq!(n.load(Ordering::SeqCst), 1);
}

// RegisterViewFactory_Dispose_ShouldRemoveFactory — C# then falls back to
// `Activator.CreateInstance(ViewClass)` and gets an instance; without reflection the Rust
// registry returns `None` once the factory is gone.
#[test]
fn factory_dispose_removes() {
    let r = ViewRegistryImpl::new();
    let _s = r.register_view(valid("TestPage")).unwrap();
    r.register_view_factory("TestPage", factory("x")).dispose();
    assert!(r.create_view_instance("TestPage").is_none());
}

// GetView_ExistingId_ShouldReturnView
#[test]
fn get_existing() {
    let r = ViewRegistryImpl::new();
    let _s = r.register_view(valid("TestPage")).unwrap();
    assert_eq!(
        r.view("TestPage").map(|v| v.id).as_deref(),
        Some("TestPage")
    );
}

// GetView_NonExistingId_ShouldReturnNull
#[test]
fn get_missing() {
    assert!(ViewRegistryImpl::new().view("NonExisting").is_none());
}

// GetAllViews_ShouldReturnAllRegisteredViews
#[test]
fn all_views() {
    let r = ViewRegistryImpl::new();
    let _s: Vec<_> = ["Page1", "Page2", "Page3"]
        .iter()
        .map(|p| r.register_view(valid(p)).unwrap())
        .collect();
    assert_eq!(r.all_views().len(), 3);
}

// GetAllViews_ShouldOrderByOrderProperty
#[test]
fn all_views_ordered() {
    let r = ViewRegistryImpl::new();
    let _s: Vec<_> = [("Page3", 30), ("Page1", 10), ("Page2", 20)]
        .iter()
        .map(|(id, o)| {
            r.register_view(ViewDefinition {
                order: *o,
                ..valid(id)
            })
            .unwrap()
        })
        .collect();
    let ids: Vec<_> = r.all_views().into_iter().map(|v| v.id).collect();
    assert_eq!(ids, ["Page1", "Page2", "Page3"]);
}

fn module_view(id: &str, module: &str, default_tab: bool, tab_order: i32) -> ViewDefinition {
    ViewDefinition {
        module_id: Some(module.into()),
        is_module_default_tab: default_tab,
        module_tab_order: tab_order,
        ..valid(id)
    }
}

// GetModuleDefaultTabs_ShouldFilterByModuleIdAndIsDefault
#[test]
fn module_default_tabs_filtered() {
    let r = ViewRegistryImpl::new();
    let _a = r
        .register_view(module_view("OrderListPage", "OrderModule", true, 0))
        .unwrap();
    let _b = r
        .register_view(module_view("OrderDetailPage", "OrderModule", false, 0))
        .unwrap();
    let _c = r
        .register_view(module_view("CustomerListPage", "CustomerModule", true, 0))
        .unwrap();
    let o = r.module_default_tabs("OrderModule");
    let c = r.module_default_tabs("CustomerModule");
    assert_eq!(o.len(), 1);
    assert_eq!(o[0].id, "OrderListPage");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].id, "CustomerListPage");
}

// GetModuleDefaultTabs_ShouldOrderByModuleTabOrder
#[test]
fn module_default_tabs_ordered() {
    let r = ViewRegistryImpl::new();
    let _a = r
        .register_view(module_view("Tab2", "TestModule", true, 2))
        .unwrap();
    let _b = r
        .register_view(module_view("Tab1", "TestModule", true, 1))
        .unwrap();
    let ids: Vec<_> = r
        .module_default_tabs("TestModule")
        .into_iter()
        .map(|v| v.id)
        .collect();
    assert_eq!(ids, ["Tab1", "Tab2"]);
}

// CreateViewInstance_WithFactory_ShouldUseFactory
#[test]
fn create_with_factory() {
    let r = ViewRegistryImpl::new();
    let _s = r.register_view(valid("TestPage")).unwrap();
    let _f = r.register_view_factory("TestPage", factory("Factory Created"));
    let inst = r.create_view_instance("TestPage").unwrap();
    assert_eq!(inst.downcast_ref::<&str>(), Some(&"Factory Created"));
}

// CreateViewInstance_NoViewClassOrFactory_ShouldReturnNull
#[test]
fn create_without_factory_is_none() {
    let r = ViewRegistryImpl::new();
    let _s = r
        .register_view(ViewDefinition {
            view_class_name: None,
            ..valid("TestPage")
        })
        .unwrap();
    assert!(r.create_view_instance("TestPage").is_none());
}

// CreateViewInstance_NonExistingView_ShouldReturnNull
#[test]
fn create_missing_is_none() {
    assert!(
        ViewRegistryImpl::new()
            .create_view_instance("NonExisting")
            .is_none()
    );
}

// Dispose_Subscription_ShouldRemoveView
#[test]
fn dispose_removes() {
    let r = ViewRegistryImpl::new();
    r.register_view(valid("TestPage")).unwrap().dispose();
    assert!(r.view("TestPage").is_none());
}

// Dispose_Subscription_ShouldRaiseViewsChangedEvent
#[test]
fn dispose_bumps_revision() {
    let r = ViewRegistryImpl::new();
    let s = r.register_view(valid("TestPage")).unwrap();
    let before = r.revision();
    s.dispose();
    assert!(r.revision() > before);
}

// Dispose_Subscription_ShouldAlsoRemoveFactory
#[test]
fn dispose_removes_factory() {
    let r = ViewRegistryImpl::new();
    let s = r.register_view(valid("TestPage")).unwrap();
    let _f = r.register_view_factory("TestPage", factory("Factory"));
    s.dispose();
    assert!(r.create_view_instance("TestPage").is_none());
}

// Constructor_WithoutLogger_ShouldWork
#[test]
fn constructor() {
    assert!(ViewRegistryImpl::default().all_views().is_empty());
}
