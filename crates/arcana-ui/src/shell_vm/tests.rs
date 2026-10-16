//! The shell state machine without egui: inputs in, state + view effects out.

use std::sync::{Arc, Mutex};

use arcana_infra::network::{ProbeNetworkMonitor, StaticProbe};
use arcana_infra::security::TokenServiceOptions;
use arcana_infra::wiring::{ArcanaConfig, ArcanaServices};
use arcana_plugin_api::WindowService as _;

use super::*;

struct Fixture {
    vm: ShellViewModel,
    _rt: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config = ArcanaConfig {
        token_key: b"shell-vm-test-key-0123456789abc!".to_vec(),
        token_options: TokenServiceOptions::default(),
        settings_path: dir.path().join("prefs.json"),
        app_settings_path: dir.path().join("settings.json"),
    };
    let svc = rt.block_on(ArcanaServices::in_memory(config)).unwrap();
    let ui = UiServices::from_arcana(
        &svc,
        Arc::new(ProbeNetworkMonitor::new(StaticProbe(true))),
        dir.path(),
    );
    ui.app_settings.set_language_code("en-US");
    let vm = ShellViewModel::new(ui, rt.handle().clone(), 4);
    Fixture {
        vm,
        _rt: rt,
        _dir: dir,
    }
}

fn open(vm: &mut ShellViewModel, route: Route, origin: Option<TabId>) -> (TabId, Vec<ShellEffect>) {
    let mut fx = Vec::new();
    let req = NavRequest::new_tab(route);
    let req = if origin.is_some() {
        NavRequest {
            mode: NavMode::WithinTab,
            ..req
        }
    } else {
        req
    };
    let tab = vm.navigate(&req, origin, &mut fx);
    (tab, fx)
}

#[test]
fn new_tab_navigation_docks_and_focuses_and_reuses_the_document() {
    let mut f = fixture();
    let (home, fx) = open(&mut f.vm, Route::Home, None);
    assert_eq!(
        fx,
        vec![
            ShellEffect::DockPush(home),
            ShellEffect::DockFocus(home),
            ShellEffect::HomeOpened
        ]
    );
    let (settings, fx) = open(&mut f.vm, Route::Settings, None);
    assert_ne!(settings, home);
    assert_eq!(
        fx[..2],
        [
            ShellEffect::DockPush(settings),
            ShellEffect::DockFocus(settings)
        ]
    );
    // The same document again: focus it, no new tab.
    let (again, fx) = open(&mut f.vm, Route::Settings, None);
    assert_eq!(again, settings);
    assert_eq!(fx, vec![ShellEffect::DockFocus(settings)]);
    assert_eq!(f.vm.tabs.len(), 2);
}

#[test]
fn within_tab_navigation_pushes_onto_the_back_stack() {
    let mut f = fixture();
    let (tab, _) = open(&mut f.vm, Route::PluginManager, None);
    let (same, fx) = open(&mut f.vm, Route::Settings, Some(tab));
    assert_eq!(same, tab);
    assert!(!fx.iter().any(|e| matches!(e, ShellEffect::DockPush(_))));
    assert!(matches!(f.vm.tabs[&tab].page, Page::Settings(_)));
    assert!(f.vm.tabs[&tab].can_go_back());
    f.vm.on(ShellInput::Back(tab));
    assert!(matches!(f.vm.tabs[&tab].page, Page::PluginManager(_)));
}

#[test]
fn close_pop_out_and_dock_back_drive_the_dock() {
    let mut f = fixture();
    let (tab, _) = open(&mut f.vm, Route::Settings, None);
    assert_eq!(
        f.vm.on(ShellInput::PopOut(tab)),
        vec![ShellEffect::DockRemove(tab)]
    );
    assert_eq!(f.vm.floating, vec![tab]);
    assert!(
        f.vm.on(ShellInput::PopOut(tab)).is_empty(),
        "already floating"
    );
    assert_eq!(
        f.vm.on(ShellInput::DockBack(tab)),
        vec![ShellEffect::DockPush(tab), ShellEffect::DockFocus(tab)]
    );
    assert_eq!(
        f.vm.on(ShellInput::Close { tab, force: false }),
        vec![ShellEffect::DockRemove(tab)]
    );
    assert!(f.vm.tabs.is_empty());
    assert!(f.vm.on(ShellInput::Close { tab, force: false }).is_empty());
}

#[test]
fn dialog_answers_route_to_their_target() {
    let mut f = fixture();
    let (tab, _) = open(&mut f.vm, Route::Settings, None);
    // "No" leaves the tab alone.
    f.vm.dialog = Some(Dialog::CloseDirty { tab });
    assert!(f.vm.on(ShellInput::AnswerDialog(false)).is_empty());
    assert!(f.vm.dialog.is_none());
    assert!(f.vm.tabs.contains_key(&tab));
    // "Yes" force-closes it.
    f.vm.dialog = Some(Dialog::CloseDirty { tab });
    assert_eq!(
        f.vm.on(ShellInput::AnswerDialog(true)),
        vec![ShellEffect::DockRemove(tab)]
    );
    assert!(f.vm.tabs.is_empty());
    // No dialog: nothing happens.
    assert!(f.vm.on(ShellInput::AnswerDialog(true)).is_empty());
}

#[test]
fn queued_confirms_wait_behind_the_open_dialog() {
    let mut f = fixture();
    f.vm.on(ShellInput::Execute {
        cmd: CommandId::About,
        focused: None,
    });
    let confirm = Dialog::PluginConfirm {
        title: "T".into(),
        message: "m".into(),
        reply_command: None,
    };
    f.vm.on(ShellInput::Confirm(confirm.clone()));
    assert_eq!(f.vm.dialog, Some(Dialog::About));
    f.vm.on(ShellInput::AnswerDialog(true));
    assert_eq!(f.vm.dialog, Some(confirm));
    f.vm.on(ShellInput::AnswerDialog(false));
    assert!(f.vm.dialog.is_none());
}

#[test]
fn plugin_confirm_replies_through_its_command_then_pumps_plugins() {
    let mut f = fixture();
    let rt = f.vm.services().plugins.clone();
    let answers: Arc<Mutex<Vec<bool>>> = Arc::default();
    let sink = answers.clone();
    let _sub = rt
        .host
        .services
        .commands
        .register_command(
            "vm.reply",
            Arc::new(move |args| {
                let yes = args.first().and_then(|a| a.downcast_ref::<bool>()).copied();
                sink.lock().unwrap().push(yes.unwrap_or_default());
                Ok(())
            }),
        )
        .unwrap();
    for (title, yes) in [("A", false), ("B", true)] {
        f.vm.on(ShellInput::Confirm(Dialog::PluginConfirm {
            title: title.into(),
            message: String::new(),
            reply_command: Some("vm.reply".into()),
        }));
        assert_eq!(
            f.vm.on(ShellInput::AnswerDialog(yes)),
            vec![ShellEffect::PumpPlugins]
        );
    }
    assert_eq!(*answers.lock().unwrap(), vec![false, true]);
    // An unknown reply command is reported, not swallowed.
    f.vm.on(ShellInput::Confirm(Dialog::PluginConfirm {
        title: "C".into(),
        message: String::new(),
        reply_command: Some("vm.missing".into()),
    }));
    f.vm.on(ShellInput::AnswerDialog(true));
    assert_eq!(f.vm.toasts.len(), 1);
    rt.window.set_status_bar_item("unused", None);
}

#[test]
fn commands_toggle_chrome_and_act_on_the_focused_tab() {
    let mut f = fixture();
    let exec = |vm: &mut ShellViewModel, cmd, focused| vm.on(ShellInput::Execute { cmd, focused });
    exec(&mut f.vm, CommandId::ToggleSidebar, None);
    exec(&mut f.vm, CommandId::ToggleStatusBar, None);
    exec(&mut f.vm, CommandId::Fullscreen, None);
    exec(&mut f.vm, CommandId::CommandSearch, None);
    exec(&mut f.vm, CommandId::Exit, None);
    assert!(!f.vm.sidebar_open && !f.vm.statusbar_visible);
    assert!(f.vm.fullscreen && f.vm.focus_search && f.vm.exit_requested);
    // Tab commands without a focused tab do nothing.
    for cmd in [
        CommandId::Save,
        CommandId::Back,
        CommandId::CloseTab,
        CommandId::PopOut,
    ] {
        assert!(exec(&mut f.vm, cmd, None).is_empty());
    }
    // Navigation commands open documents.
    let fx = exec(&mut f.vm, CommandId::Settings, None);
    let tab = f.vm.find_tab(&Route::Settings).expect("settings opened");
    assert_eq!(
        fx[..2],
        [ShellEffect::DockPush(tab), ShellEffect::DockFocus(tab)]
    );
    assert_eq!(
        exec(&mut f.vm, CommandId::PopOut, Some(tab)),
        vec![ShellEffect::DockRemove(tab)]
    );
    exec(&mut f.vm, CommandId::CloseTab, Some(tab));
    assert!(f.vm.tabs.is_empty());
    // Unhandled commands say so.
    exec(&mut f.vm, CommandId::Undo, None);
    assert_eq!(f.vm.toasts.len(), 1);
}

#[test]
fn status_items_are_set_and_removed() {
    let mut f = fixture();
    let set = |text: Option<&str>| ShellInput::Status {
        id: "p".into(),
        text: text.map(str::to_owned),
    };
    f.vm.on(set(Some("busy")));
    assert_eq!(f.vm.status_items.get("p").map(String::as_str), Some("busy"));
    f.vm.on(set(None));
    assert!(f.vm.status_items.is_empty());
}

#[test]
fn tab_back_stack_push_and_pop() {
    let mut t = Tab::new(
        TabId(1),
        Page::PluginManager(crate::pages::plugin_manager::ViewModel::default()),
    );
    assert!(!t.can_go_back());
    assert!(!t.back());
    t.push(Page::Settings(settings::ViewModel::default()));
    assert!(t.can_go_back());
    assert!(t.back());
    assert!(matches!(t.page, Page::PluginManager(_)));
}
