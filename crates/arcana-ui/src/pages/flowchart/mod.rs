//! `FlowChart` editor tab (C# `FlowChartEditorPage` hosted in a document tab).
//!
//! The editor VM and canvas live in `arcana-plugin-flowchart`; this page wraps them: it turns
//! the editor's file effects into a native file dialog (`rfd`, run off the UI thread) and
//! async load / save tasks, and its toasts into shell notices.
//!
//! Without a native dialog (Linux without an xdg-desktop-portal, or
//! `ARCANA_NATIVE_DIALOGS=0`) the page falls back to a path-entry prompt. rfd reports a
//! failed portal call the same way as a cancel, so a `None` that came back faster than a
//! human could cancel ([`INSTANT_CANCEL`]) is treated as "no dialog available".

use std::path::PathBuf;
use std::time::{Duration, Instant};

use arcana_plugin_flowchart::Diagram;
use arcana_plugin_flowchart::serializer::{self, DiagramFormat};
use arcana_plugin_flowchart::vm::{EditorViewModel, Effect as EditorEffect, Input as EditorInput};

use crate::effect::{Effect, Notice, NoticeLevel, Text};
use crate::nav::FlowMode;
use crate::services::UiServices;

mod view;

pub use view::show;

/// A dialog that returned nothing faster than this never showed (no portal).
pub const INSTANT_CANCEL: Duration = Duration::from_millis(300);

/// Environment switch: `ARCANA_NATIVE_DIALOGS=0` always uses the path prompt.
pub const NATIVE_DIALOGS_ENV: &str = "ARCANA_NATIVE_DIALOGS";

/// Which file operation the path prompt is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Open,
    SaveAs,
}

/// Page state around the editor VM.
#[derive(Debug, Default)]
pub struct ViewModel {
    pub editor: EditorViewModel,
    /// Open path prompt and its text.
    pub prompt: Option<(PromptKind, String)>,
}

/// Inputs addressed to the page.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// Effect the editor emitted while drawing.
    Editor(EditorEffect),
    /// Forward an input to the editor VM.
    Send(EditorInput),
    PromptText(String),
    PromptOk,
    PromptCancel,
    /// A native dialog finished: `path` is the choice; `native == false` means no dialog
    /// could be shown, so the path prompt opens instead.
    Picked {
        kind: PromptKind,
        path: Option<PathBuf>,
        native: bool,
    },
}

/// File I/O done off the UI thread.
#[derive(Debug, Clone, PartialEq)]
pub enum Task {
    /// Show the native open / save-as dialog.
    Pick(PromptKind),
    Load(PathBuf),
    Save {
        path: PathBuf,
        format: DiagramFormat,
        diagram: Box<Diagram>,
    },
}

impl ViewModel {
    #[must_use]
    pub fn new(mode: &FlowMode) -> (Self, Vec<Effect<Task>>) {
        let mut vm = Self::default();
        let fx = match mode {
            FlowMode::New => vec![],
            FlowMode::Sample => vm.on(Input::Send(EditorInput::CreateSampleDiagram)),
            FlowMode::OpenDialog => vm.on(Input::Editor(EditorEffect::RequestFileOpen)),
            FlowMode::Open(p) => vec![Effect::Run(Task::Load(PathBuf::from(p)))],
        };
        (vm, fx)
    }

    fn editor_effects(&mut self, fx: Vec<EditorEffect>) -> Vec<Effect<Task>> {
        fx.into_iter()
            .flat_map(|e| self.on(Input::Editor(e)))
            .collect()
    }

    pub fn on(&mut self, input: Input) -> Vec<Effect<Task>> {
        match input {
            Input::Send(i) => {
                let fx = self.editor.on(i);
                self.editor_effects(fx)
            }
            Input::Editor(e) => match e {
                EditorEffect::RequestFileOpen => vec![Effect::Run(Task::Pick(PromptKind::Open))],
                EditorEffect::RequestFileSaveAs => {
                    vec![Effect::Run(Task::Pick(PromptKind::SaveAs))]
                }
                EditorEffect::Save {
                    path,
                    format,
                    diagram,
                } => vec![Effect::Run(Task::Save {
                    path,
                    format,
                    diagram,
                })],
                EditorEffect::ShowError(m) => vec![Effect::Notify(Notice::error(m))],
                EditorEffect::ShowSuccess(m) => {
                    vec![Effect::Notify(Notice::new(
                        NoticeLevel::Success,
                        Text::Raw(m),
                    ))]
                }
                EditorEffect::DiagramChanged => vec![],
            },
            Input::Picked { kind, path, native } => match path {
                Some(path) => self.file_task(kind, path),
                None if !native => {
                    let initial = match kind {
                        PromptKind::Open => String::new(),
                        PromptKind::SaveAs => DEFAULT_FILE_NAME.to_string(),
                    };
                    self.prompt = Some((kind, initial));
                    vec![]
                }
                None => vec![],
            },
            Input::PromptText(t) => {
                if let Some((_, text)) = &mut self.prompt {
                    *text = t;
                }
                vec![]
            }
            Input::PromptCancel => {
                self.prompt = None;
                vec![]
            }
            Input::PromptOk => {
                let Some((kind, text)) = self.prompt.take() else {
                    return vec![];
                };
                let path = PathBuf::from(text.trim());
                if path.as_os_str().is_empty() {
                    return vec![];
                }
                self.file_task(kind, path)
            }
        }
    }

    fn file_task(&self, kind: PromptKind, path: PathBuf) -> Vec<Effect<Task>> {
        match kind {
            PromptKind::Open => vec![Effect::Run(Task::Load(path))],
            PromptKind::SaveAs => vec![Effect::Run(Task::Save {
                format: serializer::format_from_extension(&path),
                path,
                diagram: Box::new(self.editor.state.diagram.clone()),
            })],
        }
    }
}

/// Suggested name for "Save As".
pub const DEFAULT_FILE_NAME: &str = "diagram.afc";

pub(crate) fn native_dialogs_enabled() -> bool {
    std::env::var(NATIVE_DIALOGS_ENV).map_or(true, |v| v != "0")
}

/// Show the native dialog; see the module docs for the fallback rule.
async fn pick(kind: PromptKind) -> Input {
    if !native_dialogs_enabled() {
        return Input::Picked {
            kind,
            path: None,
            native: false,
        };
    }
    let started = Instant::now();
    let mut dialog = rfd::AsyncFileDialog::new();
    for (label, ext) in serializer::file_filters() {
        dialog = dialog.add_filter(label, ext);
    }
    let handle = match kind {
        PromptKind::Open => dialog.pick_file().await,
        PromptKind::SaveAs => dialog.set_file_name(DEFAULT_FILE_NAME).save_file().await,
    };
    let path = handle.map(|h| h.path().to_path_buf());
    let native = path.is_some() || started.elapsed() >= INSTANT_CANCEL;
    Input::Picked { kind, path, native }
}

/// Perform a file task.
pub async fn run(task: Task, _svc: UiServices) -> Input {
    if let Task::Pick(kind) = task {
        return pick(kind).await;
    }
    let result = tokio::task::spawn_blocking(move || match task {
        Task::Pick(_) => EditorInput::FileFailed("unreachable".into()),
        Task::Load(path) => match serializer::load_from_file(&path) {
            Ok(d) => EditorInput::FileLoaded {
                path,
                diagram: Box::new(d),
            },
            Err(e) => EditorInput::FileFailed(e.to_user_message()),
        },
        Task::Save {
            path,
            format,
            diagram,
        } => match serializer::save_to_file(&diagram, &path, format) {
            Ok(()) => EditorInput::FileSaved { path },
            Err(e) => EditorInput::FileFailed(e.to_user_message()),
        },
    })
    .await;
    Input::Send(result.unwrap_or_else(|e| EditorInput::FileFailed(e.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_mode_loads_the_sample_diagram() {
        let (vm, fx) = ViewModel::new(&FlowMode::Sample);
        assert!(fx.is_empty());
        assert!(!vm.editor.state.diagram.nodes.is_empty());
    }

    #[test]
    fn open_dialog_without_native_dialog_prompts_then_loads() {
        let (mut vm, fx) = ViewModel::new(&FlowMode::OpenDialog);
        assert_eq!(fx, vec![Effect::Run(Task::Pick(PromptKind::Open))]);
        assert!(vm.prompt.is_none());
        let _ = vm.on(Input::Picked {
            kind: PromptKind::Open,
            path: None,
            native: false,
        });
        assert_eq!(vm.prompt.as_ref().map(|p| p.0), Some(PromptKind::Open));
        let _ = vm.on(Input::PromptText("/tmp/x.drawio".into()));
        let fx = vm.on(Input::PromptOk);
        assert_eq!(
            fx,
            vec![Effect::Run(Task::Load(PathBuf::from("/tmp/x.drawio")))]
        );
        assert!(vm.prompt.is_none());
    }

    #[test]
    fn save_as_uses_extension_format_and_cancel_clears() {
        let (mut vm, _) = ViewModel::new(&FlowMode::New);
        let fx = vm.on(Input::Editor(EditorEffect::RequestFileSaveAs));
        assert_eq!(fx, vec![Effect::Run(Task::Pick(PromptKind::SaveAs))]);
        let _ = vm.on(Input::Picked {
            kind: PromptKind::SaveAs,
            path: None,
            native: false,
        });
        assert_eq!(
            vm.prompt.as_ref().map(|p| p.1.as_str()),
            Some(DEFAULT_FILE_NAME)
        );
        let _ = vm.on(Input::PromptText("a.drawio".into()));
        let fx = vm.on(Input::PromptOk);
        assert!(matches!(
            &fx[0],
            Effect::Run(Task::Save {
                format: DiagramFormat::DrawIo,
                ..
            })
        ));
        let _ = vm.on(Input::Picked {
            kind: PromptKind::Open,
            path: None,
            native: false,
        });
        let _ = vm.on(Input::PromptCancel);
        assert!(vm.prompt.is_none());
    }

    #[test]
    fn native_pick_runs_file_task_and_cancel_does_nothing() {
        let mut vm = ViewModel::default();
        let fx = vm.on(Input::Picked {
            kind: PromptKind::Open,
            path: Some(PathBuf::from("/tmp/a.afc")),
            native: true,
        });
        assert_eq!(
            fx,
            vec![Effect::Run(Task::Load(PathBuf::from("/tmp/a.afc")))]
        );
        let fx = vm.on(Input::Picked {
            kind: PromptKind::SaveAs,
            path: None,
            native: true,
        });
        assert!(fx.is_empty());
        assert!(vm.prompt.is_none(), "user cancelled: no fallback prompt");
    }

    #[test]
    fn editor_messages_become_notices() {
        let mut vm = ViewModel::default();
        let fx = vm.on(Input::Editor(EditorEffect::ShowError("bad".into())));
        assert!(matches!(&fx[0], Effect::Notify(n) if n.level == NoticeLevel::Error));
    }
}
