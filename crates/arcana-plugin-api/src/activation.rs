//! Activation events (C# `ActivationEvents` / `ActivationEventType`).

/// C# `ActivationEventType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivationEventType {
    Unknown,
    OnStartup,
    OnCommand,
    OnView,
    OnLanguage,
    OnFileType,
    OnUri,
    OnWorkspaceContains,
    OnConfiguration,
    OnAuthentication,
    OnMenu,
    Star,
}

/// C# `static class ActivationEvents`.
pub mod activation_events {
    use super::ActivationEventType;

    pub const ON_STARTUP: &str = "onStartup";
    pub const ON_COMMAND_PREFIX: &str = "onCommand:";
    pub const ON_VIEW_PREFIX: &str = "onView:";
    pub const ON_LANGUAGE_PREFIX: &str = "onLanguage:";
    pub const ON_FILE_TYPE_PREFIX: &str = "onFileType:";
    pub const ON_URI_PREFIX: &str = "onUri:";
    pub const ON_WORKSPACE_CONTAINS_PREFIX: &str = "onWorkspaceContains:";
    pub const ON_CONFIGURATION_PREFIX: &str = "onConfiguration:";
    pub const ON_AUTHENTICATION_PREFIX: &str = "onAuthentication:";
    pub const ON_MENU_PREFIX: &str = "onMenu:";
    pub const STAR: &str = "*";

    const PREFIXES: [(&str, ActivationEventType); 9] = [
        (ON_COMMAND_PREFIX, ActivationEventType::OnCommand),
        (ON_VIEW_PREFIX, ActivationEventType::OnView),
        (ON_LANGUAGE_PREFIX, ActivationEventType::OnLanguage),
        (ON_FILE_TYPE_PREFIX, ActivationEventType::OnFileType),
        (ON_URI_PREFIX, ActivationEventType::OnUri),
        (
            ON_WORKSPACE_CONTAINS_PREFIX,
            ActivationEventType::OnWorkspaceContains,
        ),
        (
            ON_CONFIGURATION_PREFIX,
            ActivationEventType::OnConfiguration,
        ),
        (
            ON_AUTHENTICATION_PREFIX,
            ActivationEventType::OnAuthentication,
        ),
        (ON_MENU_PREFIX, ActivationEventType::OnMenu),
    ];

    /// C# `ActivationEvents.Parse`.
    #[must_use]
    pub fn parse(event: &str) -> (ActivationEventType, Option<String>) {
        if event.is_empty() {
            return (ActivationEventType::Unknown, None);
        }
        if event == ON_STARTUP {
            return (ActivationEventType::OnStartup, None);
        }
        if event == STAR {
            return (ActivationEventType::Star, None);
        }
        for (prefix, kind) in PREFIXES {
            if let Some(arg) = event.strip_prefix(prefix) {
                return (kind, Some(arg.to_string()));
            }
        }
        (ActivationEventType::Unknown, Some(event.to_string()))
    }

    /// Inverse of [`parse`] for event types that carry an argument.
    #[must_use]
    pub fn format(kind: ActivationEventType, argument: Option<&str>) -> String {
        match kind {
            ActivationEventType::OnStartup => ON_STARTUP.to_string(),
            ActivationEventType::Star => STAR.to_string(),
            ActivationEventType::Unknown => argument.unwrap_or_default().to_string(),
            other => {
                let prefix = PREFIXES
                    .iter()
                    .find(|(_, k)| *k == other)
                    .map_or("", |(p, _)| *p);
                format!("{prefix}{}", argument.unwrap_or_default())
            }
        }
    }

    #[must_use]
    pub fn for_command(id: &str) -> String {
        format!("{ON_COMMAND_PREFIX}{id}")
    }
    #[must_use]
    pub fn for_view(id: &str) -> String {
        format!("{ON_VIEW_PREFIX}{id}")
    }
    #[must_use]
    pub fn for_language(code: &str) -> String {
        format!("{ON_LANGUAGE_PREFIX}{code}")
    }
    #[must_use]
    pub fn for_file_type(ext: &str) -> String {
        format!("{ON_FILE_TYPE_PREFIX}{ext}")
    }
}
