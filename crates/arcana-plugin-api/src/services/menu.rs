//! `IMenuRegistry`, `MenuLocation`, `MenuItemDefinition`.

use arcana_core::AppResult;

use crate::services::Subscription;

/// C# `MenuLocation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MenuLocation {
    MainMenu,
    FileMenu,
    EditMenu,
    ViewMenu,
    ToolsMenu,
    HelpMenu,
    ContextMenu,
    Toolbar,
    StatusBar,
    FunctionTree,
    QuickAccess,
    ModuleQuickAccess,
}

impl MenuLocation {
    pub const ALL: [Self; 12] = [
        Self::MainMenu,
        Self::FileMenu,
        Self::EditMenu,
        Self::ViewMenu,
        Self::ToolsMenu,
        Self::HelpMenu,
        Self::ContextMenu,
        Self::Toolbar,
        Self::StatusBar,
        Self::FunctionTree,
        Self::QuickAccess,
        Self::ModuleQuickAccess,
    ];
    /// `Enum.TryParse(..., ignoreCase: true)`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|l| format!("{l:?}").eq_ignore_ascii_case(s))
    }
}

/// C# `MenuItemDefinition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemDefinition {
    pub id: String,
    pub title: String,
    pub location: MenuLocation,
    pub parent_id: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub shortcut: Option<String>,
    pub command: Option<String>,
    pub order: i32,
    pub group: Option<String>,
    pub when: Option<String>,
    pub is_separator: bool,
    pub children: Vec<MenuItemDefinition>,
    pub module_id: Option<String>,
}

impl MenuItemDefinition {
    #[must_use]
    pub fn new(id: &str, title: &str, location: MenuLocation) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            location,
            parent_id: None,
            icon: None,
            tooltip: None,
            shortcut: None,
            command: None,
            order: 0,
            group: None,
            when: None,
            is_separator: false,
            children: Vec::new(),
            module_id: None,
        }
    }
    #[must_use]
    pub fn with_command(mut self, command: &str) -> Self {
        self.command = Some(command.to_string());
        self
    }
    #[must_use]
    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }
    #[must_use]
    pub fn with_parent(mut self, parent: &str) -> Self {
        self.parent_id = Some(parent.to_string());
        self
    }
    #[must_use]
    pub fn with_module(mut self, module: &str) -> Self {
        self.module_id = Some(module.to_string());
        self
    }
}

/// C# `IMenuRegistry`. `MenusChanged` becomes a monotonically increasing
/// [`MenuRegistry::revision`] the UI polls each frame.
pub trait MenuRegistry: Send + Sync {
    /// # Errors
    /// Validation failure or duplicate id (C# `ContributionValidationException` /
    /// `InvalidOperationException`).
    fn register_menu_item(&self, item: MenuItemDefinition) -> AppResult<Subscription>;
    /// # Errors
    /// Validation failure of any item (nothing is registered then).
    fn register_menu_items(&self, items: Vec<MenuItemDefinition>) -> AppResult<Subscription>;
    fn menu_items(&self, location: MenuLocation) -> Vec<MenuItemDefinition>;
    fn menu_items_for_module(
        &self,
        location: MenuLocation,
        module_id: &str,
    ) -> Vec<MenuItemDefinition>;
    fn all_menu_items(&self) -> Vec<MenuItemDefinition>;
    fn revision(&self) -> u64;
}
