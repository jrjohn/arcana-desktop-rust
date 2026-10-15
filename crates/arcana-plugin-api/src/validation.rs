//! Contribution validators (C# `MenuItemValidator`, `ViewValidator`, `CommandValidator`,
//! `ContributionValidationResult`). The C# regexes are hand-written matchers here.

use crate::services::{MenuItemDefinition, MenuLocation, ViewDefinition};

/// C# `ContributionValidationResult`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContributionValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ContributionValidationResult {
    #[must_use]
    pub fn success() -> Self {
        Self {
            is_valid: true,
            ..Self::default()
        }
    }
    #[must_use]
    pub fn failure(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
        }
    }
    #[must_use]
    pub fn with_warnings(warnings: Vec<String>) -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings,
        }
    }
    fn from_lists(errors: Vec<String>, warnings: Vec<String>) -> Self {
        Self {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }
}

/// `^[a-zA-Z][a-zA-Z0-9._-]*$` (`extra` = the allowed punctuation).
fn id_matches(s: &str, extra: &[char]) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
}

fn is_command_id(s: &str) -> bool {
    id_matches(s, &['.', '_', '-'])
}

fn blank(s: &str) -> bool {
    s.trim().is_empty()
}

/// C# `MenuItemValidator.Validate`.
#[must_use]
pub fn validate_menu_item(item: &MenuItemDefinition) -> ContributionValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    if blank(&item.id) {
        errors.push("Menu item Id is required".to_string());
    } else if !is_command_id(&item.id) {
        errors.push(format!(
            "Menu item Id '{}' has invalid format. Must start with a letter and contain only letters, numbers, dots, underscores, or hyphens.",
            item.id
        ));
    }
    if !item.is_separator && blank(&item.title) {
        errors.push(format!(
            "Menu item '{}' requires a Title (unless it's a separator)",
            item.id
        ));
    }
    if let Some(cmd) = item.command.as_deref().filter(|c| !blank(c))
        && !is_command_id(cmd)
    {
        warnings.push(format!(
            "Menu item '{}' has command '{cmd}' with non-standard format",
            item.id
        ));
    }
    if item.order < 0 {
        warnings.push(format!(
            "Menu item '{}' has negative order ({}), consider using positive values",
            item.id, item.order
        ));
    }
    if item.location == MenuLocation::ModuleQuickAccess
        && item.module_id.as_deref().is_none_or(blank)
    {
        warnings.push(format!(
            "Menu item '{}' has ModuleQuickAccess location but no ModuleId set",
            item.id
        ));
    }
    ContributionValidationResult::from_lists(errors, warnings)
}

/// C# `ViewValidator.Validate`.
#[must_use]
pub fn validate_view(view: &ViewDefinition) -> ContributionValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    if blank(&view.id) {
        errors.push("View Id is required".to_string());
    } else if !id_matches(&view.id, &['_']) {
        errors.push(format!(
            "View Id '{}' has invalid format. Must start with a letter and contain only letters, numbers, or underscores.",
            view.id
        ));
    }
    if blank(&view.title) {
        errors.push(format!("View '{}' requires a Title", view.id));
    }
    if view.view_class_name.as_deref().is_none_or(str::is_empty) {
        warnings.push(format!(
            "View '{}' has no ViewClass set. Views without ViewClass can only be created via factory.",
            view.id
        ));
    }
    if view.is_module_default_tab && view.module_id.as_deref().is_none_or(blank) {
        warnings.push(format!(
            "View '{}' is marked as IsModuleDefaultTab but has no ModuleId set",
            view.id
        ));
    }
    if view.title_key.as_deref().is_none_or(blank) {
        warnings.push(format!(
            "View '{}' has no TitleKey set. Consider setting TitleKey for dynamic localization.",
            view.id
        ));
    }
    ContributionValidationResult::from_lists(errors, warnings)
}

/// C# `CommandValidator.ValidateCommandId`.
#[must_use]
pub fn validate_command_id(command_id: &str) -> ContributionValidationResult {
    if blank(command_id) {
        ContributionValidationResult::failure(vec!["Command Id is required".to_string()])
    } else if !is_command_id(command_id) {
        ContributionValidationResult::failure(vec![format!(
            "Command Id '{command_id}' has invalid format. Must start with a letter and contain only letters, numbers, dots, underscores, or hyphens."
        )])
    } else {
        ContributionValidationResult::success()
    }
}
