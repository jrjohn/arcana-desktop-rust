//! C# `ContributionValidatorsTests.cs` (5 classes): 27 methods / 62 cases incl. Theory rows.
//! Ported 26 methods. Not ported: `ContributionValidationExceptionTests.
//! Constructor_WithInnerException_ShouldSetInnerException` (the exception maps to
//! `AppError::Validation`, which carries no inner error). C# `null` Theory rows become `""`
//! (`&str`/`String` cannot be null).

use arcana_core::AppError;
use arcana_plugin_api::{
    ContributionValidationResult, MenuItemDefinition, MenuLocation, ViewDefinition,
    validate_command_id, validate_menu_item, validate_view,
};

fn menu(id: &str, title: &str) -> MenuItemDefinition {
    MenuItemDefinition::new(id, title, MenuLocation::MainMenu)
}

fn view(id: &str, title: &str) -> ViewDefinition {
    ViewDefinition {
        title_key: Some("test.key".into()),
        view_class_name: Some("object".into()),
        ..ViewDefinition::new(id, title)
    }
}

fn has(list: &[String], needle: &str) -> bool {
    list.iter().any(|e| e.contains(needle))
}

// ── MenuItemValidatorTests ──

// Validate_ValidMenuItem_ShouldReturnSuccess
#[test]
fn menu_valid() {
    let r = validate_menu_item(&menu("menu.file.new", "New File"));
    assert!(r.is_valid && r.errors.is_empty());
}

// Validate_ValidSeparator_ShouldReturnSuccess
#[test]
fn menu_valid_separator() {
    let mut i = MenuItemDefinition::new("menu.file.separator1", "", MenuLocation::FileMenu);
    i.is_separator = true;
    assert!(validate_menu_item(&i).is_valid);
}

// Validate_ValidIdFormats_ShouldReturnSuccess ×6
#[test]
fn menu_valid_ids() {
    for id in [
        "menu.file",
        "menu.file.new",
        "menu_file_new",
        "menu-file-new",
        "menuFileNew",
        "Menu123",
    ] {
        assert!(validate_menu_item(&menu(id, "Test")).is_valid, "{id}");
    }
}

// Validate_EmptyOrNullId_ShouldReturnError ×3
#[test]
fn menu_empty_id() {
    for id in ["", "   ", ""] {
        let r = validate_menu_item(&menu(id, "Test"));
        assert!(!r.is_valid && has(&r.errors, "Id is required"), "{id:?}");
    }
}

// Validate_InvalidIdFormat_ShouldReturnError ×4
#[test]
fn menu_invalid_id_format() {
    for id in ["123menu", "_menu", ".menu", "-menu"] {
        let r = validate_menu_item(&menu(id, "Test"));
        assert!(!r.is_valid && has(&r.errors, "invalid format"), "{id}");
    }
}

// Validate_EmptyTitleNonSeparator_ShouldReturnError ×3
#[test]
fn menu_empty_title() {
    for t in ["", "   ", ""] {
        let r = validate_menu_item(&menu("menu.test", t));
        assert!(!r.is_valid && has(&r.errors, "requires a Title"), "{t:?}");
    }
}

// Validate_NegativeOrder_ShouldReturnWarning
#[test]
fn menu_negative_order_warns() {
    let r = validate_menu_item(&menu("menu.test", "Test").with_order(-1));
    assert!(r.is_valid && has(&r.warnings, "negative order"));
}

// Validate_ModuleQuickAccessWithoutModuleId_ShouldReturnWarning
#[test]
fn menu_quick_access_without_module_warns() {
    let i = MenuItemDefinition::new(
        "module.quick.action",
        "Quick Action",
        MenuLocation::ModuleQuickAccess,
    );
    let r = validate_menu_item(&i);
    assert!(r.is_valid);
    assert!(
        r.warnings
            .iter()
            .any(|w| w.contains("ModuleQuickAccess") && w.contains("ModuleId"))
    );
}

// Validate_NonStandardCommandFormat_ShouldReturnWarning ×2
#[test]
fn menu_non_standard_command_warns() {
    for c in ["123command", "command with space"] {
        let r = validate_menu_item(&menu("menu.test", "Test").with_command(c));
        assert!(r.is_valid && has(&r.warnings, "non-standard format"), "{c}");
    }
}

// ── ViewValidatorTests ──

// Validate_ValidView_ShouldReturnSuccess
#[test]
fn view_valid() {
    let r = validate_view(&view("OrderListPage", "Order List"));
    assert!(r.is_valid && r.errors.is_empty());
}

// Validate_ValidIdFormats_ShouldReturnSuccess ×4
#[test]
fn view_valid_ids() {
    for id in [
        "HomePage",
        "Order_List_Page",
        "Page123",
        "CustomerDetailPage",
    ] {
        assert!(validate_view(&view(id, "Test")).is_valid, "{id}");
    }
}

// Validate_EmptyOrNullId_ShouldReturnError ×3
#[test]
fn view_empty_id() {
    for id in ["", "   ", ""] {
        let r = validate_view(&ViewDefinition::new(id, "Test"));
        assert!(!r.is_valid && has(&r.errors, "Id is required"), "{id:?}");
    }
}

// Validate_InvalidIdFormat_ShouldReturnError ×4
#[test]
fn view_invalid_id_format() {
    for id in ["123Page", "_Page", "Order-Page", "Order.Page"] {
        let r = validate_view(&ViewDefinition::new(id, "Test"));
        assert!(!r.is_valid && has(&r.errors, "invalid format"), "{id}");
    }
}

// Validate_EmptyOrNullTitle_ShouldReturnError ×3
#[test]
fn view_empty_title() {
    for t in ["", "   ", ""] {
        let r = validate_view(&ViewDefinition::new("TestPage", t));
        assert!(!r.is_valid && has(&r.errors, "requires a Title"), "{t:?}");
    }
}

// Validate_NoViewClass_ShouldReturnWarning
#[test]
fn view_no_class_warns() {
    let v = ViewDefinition {
        view_class_name: None,
        ..view("TestPage", "Test")
    };
    let r = validate_view(&v);
    assert!(r.is_valid && has(&r.warnings, "ViewClass"));
}

// Validate_NoTitleKey_ShouldReturnWarning
#[test]
fn view_no_title_key_warns() {
    let v = ViewDefinition {
        title_key: None,
        ..view("TestPage", "Test")
    };
    let r = validate_view(&v);
    assert!(r.is_valid && has(&r.warnings, "TitleKey"));
}

// Validate_ModuleDefaultTabWithoutModuleId_ShouldReturnWarning
#[test]
fn view_default_tab_without_module_warns() {
    let v = ViewDefinition {
        is_module_default_tab: true,
        ..view("TestPage", "Test")
    };
    let r = validate_view(&v);
    assert!(r.is_valid);
    assert!(
        r.warnings
            .iter()
            .any(|w| w.contains("IsModuleDefaultTab") && w.contains("ModuleId"))
    );
}

// ── CommandValidatorTests ──

// ValidateCommandId_ValidFormats_ShouldReturnSuccess ×7
#[test]
fn command_valid_ids() {
    for id in [
        "file.new",
        "order.list",
        "app.exit",
        "command_test",
        "command-test",
        "CommandTest",
        "command123",
    ] {
        let r = validate_command_id(id);
        assert!(r.is_valid && r.errors.is_empty(), "{id}");
    }
}

// ValidateCommandId_EmptyOrNull_ShouldReturnError ×3
#[test]
fn command_empty() {
    for id in ["", "   ", ""] {
        let r = validate_command_id(id);
        assert!(!r.is_valid && has(&r.errors, "Id is required"), "{id:?}");
    }
}

// ValidateCommandId_InvalidFormat_ShouldReturnError ×4
#[test]
fn command_invalid_format() {
    for id in ["123command", "_command", ".command", "-command"] {
        let r = validate_command_id(id);
        assert!(!r.is_valid && has(&r.errors, "invalid format"), "{id}");
    }
}

// ── ContributionValidationResultTests ──

// Success_ShouldReturnValidResult
#[test]
fn result_success() {
    let r = ContributionValidationResult::success();
    assert!(r.is_valid && r.errors.is_empty() && r.warnings.is_empty());
}

// Failure_ShouldReturnInvalidResultWithErrors
#[test]
fn result_failure() {
    let r = ContributionValidationResult::failure(vec!["Error 1".into(), "Error 2".into()]);
    assert!(!r.is_valid);
    assert_eq!(r.errors, ["Error 1", "Error 2"]);
}

// WithWarnings_ShouldReturnValidResultWithWarnings
#[test]
fn result_with_warnings() {
    let r =
        ContributionValidationResult::with_warnings(vec!["Warning 1".into(), "Warning 2".into()]);
    assert!(r.is_valid);
    assert_eq!(r.warnings, ["Warning 1", "Warning 2"]);
}

// ── ContributionValidationExceptionTests → AppError::Validation ──

// Constructor_WithMessage_ShouldSetMessage
#[test]
fn exception_message() {
    let e = AppError::validation("Test error message", vec![]);
    assert_eq!(e.message(), "Test error message");
}

// ValidationErrors_ShouldBeSettable
#[test]
fn exception_errors_settable() {
    let e = AppError::validation("Test", vec!["Error 1".into(), "Error 2".into()]);
    assert!(
        matches!(e, AppError::Validation { ref errors, .. } if errors == &["Error 1", "Error 2"])
    );
}

// ValidationErrors_DefaultValue_ShouldBeEmpty
#[test]
fn exception_errors_default_empty() {
    let e = AppError::validation("Test", Vec::new());
    assert!(matches!(e, AppError::Validation { ref errors, .. } if errors.is_empty()));
}
