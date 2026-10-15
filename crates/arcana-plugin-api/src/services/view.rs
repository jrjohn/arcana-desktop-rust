//! `IViewRegistry`, `ViewDefinition`, `ViewType`.

use std::any::Any;
use std::sync::Arc;

use arcana_core::AppResult;

use crate::services::Subscription;

/// C# `ViewType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ViewType {
    #[default]
    Page,
    Dialog,
    Panel,
    Widget,
    Flyout,
}

impl ViewType {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Page,
            Self::Dialog,
            Self::Panel,
            Self::Widget,
            Self::Flyout,
        ]
        .into_iter()
        .find(|v| format!("{v:?}").eq_ignore_ascii_case(s))
    }
}

/// C# `ViewDefinition`. `Type ViewClass` / `ViewModelType` have no Rust counterpart; the
/// class *name* is kept and views are created through a registered [`ViewFactory`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewDefinition {
    pub id: String,
    pub title: String,
    pub title_key: Option<String>,
    pub icon: Option<String>,
    pub view_type: ViewType,
    pub view_class_name: Option<String>,
    pub can_have_multiple_instances: bool,
    pub category: Option<String>,
    pub order: i32,
    pub module_id: Option<String>,
    pub is_module_default_tab: bool,
    pub module_tab_order: i32,
}

impl ViewDefinition {
    #[must_use]
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            ..Self::default()
        }
    }
}

/// Whatever a factory builds; the UI layer downcasts it to its own view type.
pub type ViewInstance = Box<dyn Any + Send>;
/// C# `Func<object>`.
pub type ViewFactory = Arc<dyn Fn() -> ViewInstance + Send + Sync>;

/// C# `IViewRegistry`. `ViewsChanged` → [`ViewRegistry::revision`].
pub trait ViewRegistry: Send + Sync {
    /// # Errors
    /// Validation failure or duplicate id.
    fn register_view(&self, view: ViewDefinition) -> AppResult<Subscription>;
    fn register_view_factory(&self, view_id: &str, factory: ViewFactory) -> Subscription;
    fn view(&self, view_id: &str) -> Option<ViewDefinition>;
    fn all_views(&self) -> Vec<ViewDefinition>;
    fn module_default_tabs(&self, module_id: &str) -> Vec<ViewDefinition>;
    fn create_view_instance(&self, view_id: &str) -> Option<ViewInstance>;
    fn revision(&self) -> u64;
}
