//! What a view model asks the outside world to do (the C# `Effect` subjects, as data).
//!
//! A view model never performs I/O: `on(Input)` mutates its state and returns effects. The
//! shell interprets them — `Run` goes to the async executor, `Navigate` to the nav graph,
//! `Notify` to the toast list — and feeds results back as inputs.

use crate::nav::NavRequest;

/// Severity of a [`Notice`] (C# `ShowError` / `ShowWarning` / `ShowSuccess` / `ShowInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Message text: a localization key (resolved at draw time, so it follows language switches)
/// or literal text (error messages coming from services/validators).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Text {
    Key(&'static str),
    /// Key with `{0}`… arguments.
    KeyWith(&'static str, Vec<String>),
    Raw(String),
}

/// A toast for the status area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub level: NoticeLevel,
    pub text: Text,
}

impl Notice {
    #[must_use]
    pub fn new(level: NoticeLevel, text: Text) -> Self {
        Self { level, text }
    }

    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(NoticeLevel::Error, Text::Raw(message.into()))
    }
}

/// One side effect. `T` is the page's own async task type.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect<T> {
    /// Run an async task off the UI thread; its result comes back as an input.
    Run(T),
    /// Open / focus a route.
    Navigate(NavRequest),
    /// Close the tab hosting this page (C# `NavGraph.Close`).
    CloseSelf,
    Notify(Notice),
    /// Ask the user to confirm discarding unsaved changes; `Confirm` input follows on yes.
    ConfirmDiscard,
    /// Ask the user to confirm a delete; the page receives its own confirm input on yes.
    ConfirmDelete {
        id: i32,
        label: String,
    },
    /// Something other pages may care about changed (C# `OrderSaved` / `OrderDeleted`).
    DataChanged(DataKind),
    /// Apply and persist a theme preset id (C# `ThemeService.ApplyTheme`).
    Theme(String),
    /// Apply and persist an accent override (`None` = the theme's own accent).
    Accent(Option<usize>),
    /// Switch and persist the UI culture (C# `LocalizationService.SetCulture`).
    Culture(String),
    /// Run a command by name through the plugin `CommandService` (contributed quick actions).
    Command(String),
}

/// Which data set changed, so open list pages can refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataKind {
    Orders,
    Customers,
    Products,
}

impl<T> Effect<T> {
    /// Re-type a non-`Run` effect (used when a page forwards effects of a sub-model).
    ///
    /// # Panics
    /// Never: `Run` is mapped through `f`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Effect<U> {
        match self {
            Self::Run(t) => Effect::Run(f(t)),
            Self::Navigate(n) => Effect::Navigate(n),
            Self::CloseSelf => Effect::CloseSelf,
            Self::Notify(n) => Effect::Notify(n),
            Self::ConfirmDiscard => Effect::ConfirmDiscard,
            Self::ConfirmDelete { id, label } => Effect::ConfirmDelete { id, label },
            Self::DataChanged(k) => Effect::DataChanged(k),
            Self::Theme(t) => Effect::Theme(t),
            Self::Accent(a) => Effect::Accent(a),
            Self::Culture(c) => Effect::Culture(c),
            Self::Command(c) => Effect::Command(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::Route;

    #[test]
    fn map_preserves_every_variant() {
        let all: Vec<Effect<u8>> = vec![
            Effect::Run(1),
            Effect::Navigate(NavRequest::new_tab(Route::Home)),
            Effect::CloseSelf,
            Effect::Notify(Notice::error("x")),
            Effect::ConfirmDiscard,
            Effect::ConfirmDelete {
                id: 1,
                label: "a".into(),
            },
            Effect::DataChanged(DataKind::Orders),
            Effect::Theme("Dark".into()),
            Effect::Accent(Some(1)),
            Effect::Culture("zh-TW".into()),
            Effect::Command("order.new".into()),
        ];
        let mapped: Vec<Effect<u16>> = all.into_iter().map(|e| e.map(u16::from)).collect();
        assert_eq!(mapped[0], Effect::Run(1u16));
        assert_eq!(mapped[2], Effect::CloseSelf);
        assert_eq!(mapped[6], Effect::DataChanged(DataKind::Orders));
        assert_eq!(mapped[9], Effect::Culture("zh-TW".into()));
        assert_eq!(mapped[10], Effect::Command("order.new".into()));
        assert_eq!(mapped.len(), 11);
    }
}
