//! `FlowChart` editor tab (C# `FlowChartEditorPage` hosted in a document tab).
//!
//! The editor VM and canvas live in `arcana-plugin-flowchart`; this page wraps them: it turns
//! the editor's file effects into a path prompt (no native file dialog yet) and async load /
//! save tasks, and its toasts into shell notices.

use std::path::PathBuf;

use arcana_plugin_flowchart::Diagram;
use arcana_plugin_flowchart::serializer::{self, DiagramFormat};
use arcana_plugin_flowchart::vm::{EditorViewModel, Effect as EditorEffect, Input as EditorInput};

use crate::effect::{Effect, Notice, NoticeLevel, Text};
use crate::nav::FlowMode;
use crate::services::UiServices;
use crate::widgets::ViewCx;

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
}

/// File I/O done off the UI thread.
#[derive(Debug, Clone, PartialEq)]
pub enum Task {
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
                EditorEffect::RequestFileOpen => {
                    self.prompt = Some((PromptKind::Open, String::new()));
                    vec![]
                }
                EditorEffect::RequestFileSaveAs => {
                    self.prompt = Some((PromptKind::SaveAs, "diagram.afc".to_string()));
                    vec![]
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
    }
}

/// Perform a file task.
pub async fn run(task: Task, _svc: UiServices) -> Input {
    let result = tokio::task::spawn_blocking(move || match task {
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

/// Draw the editor (mutating its VM directly, as the canvas needs) plus the path prompt.
pub fn show(ui: &mut egui::Ui, vm: &mut ViewModel, cx: &ViewCx<'_>) -> Vec<Input> {
    let mut out: Vec<Input> = arcana_plugin_flowchart::ui::show_editor(ui, &mut vm.editor)
        .into_iter()
        .map(Input::Editor)
        .collect();
    if let Some((kind, text)) = &vm.prompt {
        let mut t = text.clone();
        egui::Modal::new(egui::Id::new("flowchart_path_prompt")).show(ui.ctx(), |ui| {
            ui.set_width(420.0);
            ui.heading(match kind {
                PromptKind::Open => cx.t("flowchart.open"),
                PromptKind::SaveAs => cx.t("menu.file.save"),
            });
            if ui
                .add(egui::TextEdit::singleline(&mut t).desired_width(f32::INFINITY))
                .changed()
            {
                out.push(Input::PromptText(t.clone()));
            }
            ui.horizontal(|ui| {
                if ui.button(cx.t("common.ok")).clicked() {
                    out.push(Input::PromptOk);
                }
                if ui.button(cx.t("common.cancel")).clicked() {
                    out.push(Input::PromptCancel);
                }
            });
        });
    }
    out
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
    fn open_dialog_prompts_then_loads() {
        let (mut vm, _) = ViewModel::new(&FlowMode::OpenDialog);
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
        let _ = vm.on(Input::Editor(EditorEffect::RequestFileSaveAs));
        let _ = vm.on(Input::PromptText("a.drawio".into()));
        let fx = vm.on(Input::PromptOk);
        assert!(matches!(
            &fx[0],
            Effect::Run(Task::Save {
                format: DiagramFormat::DrawIo,
                ..
            })
        ));
        let _ = vm.on(Input::Editor(EditorEffect::RequestFileOpen));
        let _ = vm.on(Input::PromptCancel);
        assert!(vm.prompt.is_none());
    }

    #[test]
    fn editor_messages_become_notices() {
        let mut vm = ViewModel::default();
        let fx = vm.on(Input::Editor(EditorEffect::ShowError("bad".into())));
        assert!(matches!(&fx[0], Effect::Notify(n) if n.level == NoticeLevel::Error));
    }
}
