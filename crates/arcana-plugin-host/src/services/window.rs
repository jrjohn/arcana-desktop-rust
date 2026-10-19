//! Queue-backed `WindowService`: plugins enqueue, the shell drains each frame.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_plugin_api::{MessageLevel, WindowRequest, WindowService};

#[derive(Clone, Default)]
pub struct QueuedWindowService {
    queue: Arc<Mutex<Vec<WindowRequest>>>,
}

impl std::fmt::Debug for QueuedWindowService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QueuedWindowService")
    }
}

impl QueuedWindowService {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> MutexGuard<'_, Vec<WindowRequest>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn message(&self, level: MessageLevel, message: &str, actions: &[&str]) {
        self.lock().push(WindowRequest::Message {
            level,
            message: message.to_string(),
            actions: actions.iter().map(|a| (*a).to_string()).collect(),
        });
    }
}

impl WindowService for QueuedWindowService {
    fn show_info(&self, message: &str, actions: &[&str]) {
        self.message(MessageLevel::Info, message, actions);
    }
    fn show_warning(&self, message: &str, actions: &[&str]) {
        self.message(MessageLevel::Warning, message, actions);
    }
    fn show_error(&self, message: &str, actions: &[&str]) {
        self.message(MessageLevel::Error, message, actions);
    }
    fn show_confirm(&self, title: &str, message: &str, reply_command: Option<&str>) {
        self.lock().push(WindowRequest::Confirm {
            title: title.to_string(),
            message: message.to_string(),
            reply_command: reply_command.map(str::to_string),
        });
    }
    fn set_status_bar_item(&self, id: &str, text: Option<&str>) {
        self.lock().push(WindowRequest::StatusBar {
            id: id.to_string(),
            text: text.map(str::to_string),
        });
    }
    fn drain(&self) -> Vec<WindowRequest> {
        std::mem::take(&mut *self.lock())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_drain_in_order_once() {
        let w = QueuedWindowService::new();
        w.show_info("a", &["OK"]);
        w.show_confirm("t", "m", Some("reply"));
        w.set_status_bar_item("s", Some("x"));
        let d = w.drain();
        assert_eq!(d.len(), 3);
        assert!(
            matches!(&d[0], WindowRequest::Message { level: MessageLevel::Info, actions, .. } if actions == &["OK"])
        );
        assert_eq!(w.drain().len(), 0);
    }
}
