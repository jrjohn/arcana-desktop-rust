//! Translates what plugins queued (navigation, shell commands, window requests) into
//! [`ShellAction`]s. This is the decision logic; the shell view only applies the actions.

use arcana_infra::plugin_runtime::PluginRuntime;
use arcana_plugin_api::{MessageLevel, WindowRequest, WindowService as _};
use arcana_plugin_flowchart::plugin::EditorArgs;

use crate::commands::{CommandId, CommandRegistry};
use crate::effect::{Notice, NoticeLevel, Text};
use crate::nav::{FlowMode, NavRequest, PluginNavRequest as PluginNav, Route, routes};

/// One thing the shell must do in response to a plugin request.
#[derive(Debug)]
pub enum ShellAction {
    Navigate(NavRequest),
    Notify(Notice),
    Back,
    Close,
    Execute(CommandId),
    Confirm(ConfirmRequest),
    Status { id: String, text: Option<String> },
}

/// A plugin's yes/no question for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmRequest {
    pub title: String,
    pub message: String,
    /// Command executed with `true`/`false` when the user answers.
    pub reply_command: Option<String>,
}

type Param = Option<Box<dyn std::any::Any + Send>>;

fn flow_mode(parameter: Option<&Box<dyn std::any::Any + Send>>) -> FlowMode {
    match parameter.and_then(|p| p.downcast_ref::<EditorArgs>()) {
        Some(EditorArgs::Sample) => FlowMode::Sample,
        Some(EditorArgs::OpenDialog) => FlowMode::OpenDialog,
        Some(EditorArgs::Open(p)) => FlowMode::Open(p.clone()),
        Some(EditorArgs::New) | None => FlowMode::New,
    }
}

/// Resolve a plugin route id (plus its parameter) to a shell route.
#[must_use]
pub fn route_for(
    route_id: &str,
    parameter: Option<&Box<dyn std::any::Any + Send>>,
) -> Option<Route> {
    if route_id == routes::FLOWCHART {
        return Some(Route::FlowChart {
            mode: flow_mode(parameter),
        });
    }
    Route::from_id(route_id)
}

fn developing() -> ShellAction {
    ShellAction::Notify(Notice::new(
        NoticeLevel::Info,
        Text::Key("common.developing"),
    ))
}

fn open(route_id: &str, parameter: &Param) -> ShellAction {
    route_for(route_id, parameter.as_ref()).map_or_else(developing, |r| {
        ShellAction::Navigate(NavRequest::new_tab(r))
    })
}

fn within_tab(route_id: &str, parameter: &Param) -> Option<ShellAction> {
    route_for(route_id, parameter.as_ref())
        .map(|r| ShellAction::Navigate(NavRequest::within_tab(r)))
}

/// The action for one plugin navigation request (`None` = nothing to do).
#[must_use]
pub fn nav_action(req: PluginNav) -> Option<ShellAction> {
    match req {
        PluginNav::Open {
            route_id,
            parameter,
        } => Some(open(&route_id, &parameter)),
        PluginNav::WithinTab {
            route_id,
            parameter,
        } => within_tab(&route_id, &parameter),
        PluginNav::Back => Some(ShellAction::Back),
        PluginNav::Forward => None,
        PluginNav::Close => Some(ShellAction::Close),
    }
}

fn notice_level(level: MessageLevel) -> NoticeLevel {
    match level {
        MessageLevel::Info => NoticeLevel::Info,
        MessageLevel::Warning => NoticeLevel::Warning,
        MessageLevel::Error => NoticeLevel::Error,
    }
}

/// The action for one plugin window request.
#[must_use]
pub fn window_action(req: WindowRequest) -> ShellAction {
    match req {
        WindowRequest::Confirm {
            title,
            message,
            reply_command,
        } => ShellAction::Confirm(ConfirmRequest {
            title,
            message,
            reply_command,
        }),
        WindowRequest::StatusBar { id, text } => ShellAction::Status { id, text },
        WindowRequest::Message { level, message, .. } => {
            ShellAction::Notify(Notice::new(notice_level(level), Text::Raw(message)))
        }
    }
}

/// Drain every plugin queue, in order: navigation, shell commands, window requests.
pub fn drain(rt: &PluginRuntime) -> Vec<ShellAction> {
    let mut out: Vec<ShellAction> = rt
        .nav
        .drain()
        .into_iter()
        .filter_map(|r| nav_action(PluginNav::from(r)))
        .collect();
    out.extend(
        rt.take_shell_commands()
            .iter()
            .filter_map(|n| CommandRegistry::by_name(n))
            .map(ShellAction::Execute),
    );
    out.extend(rt.window.drain().into_iter().map(window_action));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flowchart_route_carries_editor_args() {
        let p: Box<dyn std::any::Any + Send> = Box::new(EditorArgs::Sample);
        assert_eq!(
            route_for(routes::FLOWCHART, Some(&p)),
            Some(Route::FlowChart {
                mode: FlowMode::Sample
            })
        );
        assert_eq!(route_for("SettingsPage", None), Some(Route::Settings));
        assert_eq!(route_for("ProductCategoriesPage", None), None);
    }

    #[test]
    fn unknown_route_notifies_and_forward_is_ignored() {
        let open = PluginNav::Open {
            route_id: "Nope".into(),
            parameter: None,
        };
        assert!(matches!(nav_action(open), Some(ShellAction::Notify(_))));
        assert!(nav_action(PluginNav::Forward).is_none());
        let within = PluginNav::WithinTab {
            route_id: "Nope".into(),
            parameter: None,
        };
        assert!(nav_action(within).is_none());
    }

    #[test]
    fn window_requests_map_to_actions() {
        let s = window_action(WindowRequest::StatusBar {
            id: "x".into(),
            text: None,
        });
        assert!(matches!(s, ShellAction::Status { text: None, .. }));
    }
}
