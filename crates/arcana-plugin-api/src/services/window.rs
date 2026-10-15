//! `WindowService` — dialogs, toasts and status-bar text requested by plugins.

/// Severity of a message box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
}

/// One queued UI request (the shell drains them every frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowRequest {
    Message {
        level: MessageLevel,
        message: String,
        actions: Vec<String>,
    },
    Confirm {
        title: String,
        message: String,
        /// Command executed with `true`/`false` when the user answers.
        reply_command: Option<String>,
    },
    StatusBar {
        id: String,
        text: Option<String>,
    },
}

/// C# `WindowService`. C# awaits the user's answer; an egui frame cannot block, so answers
/// come back through a command (`reply_command`) instead of a `Task<T>`. File / folder pickers
/// and progress dialogs are not part of the contract (no C# plugin calls them).
pub trait WindowService: Send + Sync {
    fn show_info(&self, message: &str, actions: &[&str]);
    fn show_warning(&self, message: &str, actions: &[&str]);
    fn show_error(&self, message: &str, actions: &[&str]);
    fn show_confirm(&self, title: &str, message: &str, reply_command: Option<&str>);
    fn set_status_bar_item(&self, id: &str, text: Option<&str>);
    fn drain(&self) -> Vec<WindowRequest>;
}
