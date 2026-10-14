//! Settings (C# `SettingsPage`): theme preset + accent, language (live switch), sync
//! settings, data actions, about.
//!
//! C# leaves "Sync Now", "Export/Import Data" and "Clear Cache" as `// TODO`; here they show
//! the "Under development…" notice instead of silently doing nothing.

mod view;

pub use view::show;

use crate::effect::{Effect, Notice, NoticeLevel, Text};
use crate::services::UiServices;

/// C# `SyncFrequencyComboBox` choices, in minutes.
pub const FREQUENCIES: [(i32, &str); 4] = [
    (5, "settings.every5min"),
    (15, "settings.every15min"),
    (30, "settings.every30min"),
    (60, "settings.everyHour"),
];

/// C# `LanguageComboBox`: zh-TW, English, ja-JP (display names are endonyms).
pub const LANGUAGES: [(&str, &str); 3] = [
    ("zh-TW", "繁體中文"),
    ("en-US", "English"),
    ("ja-JP", "日本語"),
];

/// Current values the page opens with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub theme_id: String,
    pub accent: Option<usize>,
    pub culture: String,
    pub auto_sync: bool,
    pub sync_minutes: i32,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            theme_id: "System".into(),
            accent: None,
            culture: "en-US".into(),
            auto_sync: true,
            sync_minutes: 5,
        }
    }
}

pub type State = Snapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    SelectTheme(String),
    SelectAccent(Option<usize>),
    SelectLanguage(String),
    AutoSyncToggled(bool),
    FrequencyChanged(i32),
    Saved(Result<(), String>),
    SyncNow,
    ExportData,
    ImportData,
    ClearCache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    SaveSync { auto_sync: bool, minutes: i32 },
}

#[derive(Debug, Default)]
pub struct ViewModel {
    pub state: State,
}

impl ViewModel {
    #[must_use]
    pub fn new(snapshot: Snapshot) -> Self {
        Self { state: snapshot }
    }

    fn save_sync(&self) -> Vec<Effect<Task>> {
        vec![Effect::Run(Task::SaveSync {
            auto_sync: self.state.auto_sync,
            minutes: self.state.sync_minutes,
        })]
    }

    pub fn on(&mut self, input: Input) -> Vec<Effect<Task>> {
        let s = &mut self.state;
        match input {
            Input::SelectTheme(id) => {
                if s.theme_id == id {
                    return vec![];
                }
                s.theme_id.clone_from(&id);
                vec![Effect::Theme(id)]
            }
            Input::SelectAccent(a) => {
                s.accent = a;
                vec![Effect::Accent(a)]
            }
            Input::SelectLanguage(code) => {
                if s.culture == code || !LANGUAGES.iter().any(|(c, _)| *c == code) {
                    return vec![];
                }
                s.culture.clone_from(&code);
                vec![Effect::Culture(code)]
            }
            Input::AutoSyncToggled(on) => {
                s.auto_sync = on;
                self.save_sync()
            }
            Input::FrequencyChanged(m) => {
                // C# maps an unknown index to 5.
                s.sync_minutes = if FREQUENCIES.iter().any(|(f, _)| *f == m) {
                    m
                } else {
                    5
                };
                self.save_sync()
            }
            Input::Saved(Ok(())) => vec![],
            Input::Saved(Err(e)) => vec![Effect::Notify(Notice::error(e))],
            Input::SyncNow | Input::ExportData | Input::ImportData | Input::ClearCache => {
                vec![Effect::Notify(Notice::new(
                    NoticeLevel::Info,
                    Text::Key("common.developing"),
                ))]
            }
        }
    }
}

/// Execute a settings task (C# writes `AppSettingsService` properties).
#[allow(clippy::unused_async)]
pub async fn run(task: Task, svc: UiServices) -> Input {
    match task {
        Task::SaveSync { auto_sync, minutes } => {
            svc.app_settings.set_auto_sync_enabled(auto_sync);
            svc.app_settings.set_sync_frequency_minutes(minutes);
            Input::Saved(Ok(()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_accent_and_language_emit_shell_effects() {
        let mut vm = ViewModel::new(Snapshot::default());
        assert_eq!(
            vm.on(Input::SelectTheme("Dark".into())),
            vec![Effect::Theme("Dark".into())]
        );
        assert!(vm.on(Input::SelectTheme("Dark".into())).is_empty());
        assert_eq!(
            vm.on(Input::SelectAccent(Some(2))),
            vec![Effect::Accent(Some(2))]
        );
        assert_eq!(
            vm.on(Input::SelectLanguage("zh-TW".into())),
            vec![Effect::Culture("zh-TW".into())]
        );
        assert!(vm.on(Input::SelectLanguage("zh-TW".into())).is_empty());
        assert!(vm.on(Input::SelectLanguage("fr-FR".into())).is_empty());
        assert_eq!(vm.state.theme_id, "Dark");
        assert_eq!(vm.state.culture, "zh-TW");
    }

    #[test]
    fn sync_settings_persist_and_unknown_frequency_falls_back() {
        let mut vm = ViewModel::default();
        assert_eq!(
            vm.on(Input::AutoSyncToggled(false)),
            vec![Effect::Run(Task::SaveSync {
                auto_sync: false,
                minutes: 5
            })]
        );
        assert_eq!(
            vm.on(Input::FrequencyChanged(30)),
            vec![Effect::Run(Task::SaveSync {
                auto_sync: false,
                minutes: 30
            })]
        );
        let _ = vm.on(Input::FrequencyChanged(7));
        assert_eq!(vm.state.sync_minutes, 5);
        assert!(vm.on(Input::Saved(Ok(()))).is_empty());
        assert_eq!(vm.on(Input::Saved(Err("disk".into()))).len(), 1);
    }

    #[test]
    fn todo_actions_say_under_development() {
        let mut vm = ViewModel::default();
        for i in [
            Input::SyncNow,
            Input::ExportData,
            Input::ImportData,
            Input::ClearCache,
        ] {
            assert!(
                matches!(&vm.on(i)[0], Effect::Notify(n) if n.text == Text::Key("common.developing"))
            );
        }
    }
}
