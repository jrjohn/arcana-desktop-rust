//! egui editor page (C# `Views/FlowChartEditorPage.xaml`): toolbar, canvas,
//! properties panel, status bar. Host owns file dialogs/I-O and handles returned effects.

mod canvas;

pub use canvas::{View, show_canvas};

use egui::{Key, Modifiers};

use crate::model::NodeShape;
use crate::vm::{EditorViewModel, Effect, Input};

/// Draws the whole editor into `ui`; returns effects for the host (file dialogs, toasts).
pub fn show_editor(ui: &mut egui::Ui, vm: &mut EditorViewModel) -> Vec<Effect> {
    let mut fx = Vec::new();
    toolbar(ui, vm, &mut fx);
    ui.separator();
    shortcuts(ui, vm, &mut fx);
    let status_h = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
    let avail = ui.available_size();
    let panel_w = 200.0_f32.min(avail.x * 0.3);
    ui.horizontal(|ui| {
        let canvas_size = egui::vec2(
            (avail.x - panel_w - 8.0).max(50.0),
            (avail.y - status_h).max(50.0),
        );
        ui.allocate_ui(canvas_size, |ui| fx.extend(show_canvas(ui, vm)));
        ui.vertical(|ui| {
            ui.set_width(panel_w);
            properties(ui, vm, &mut fx);
        });
    });
    status_bar(ui, vm);
    fx
}

fn toolbar(ui: &mut egui::Ui, vm: &mut EditorViewModel, fx: &mut Vec<Effect>) {
    let mut send = |vm: &mut EditorViewModel, i: Input| fx.extend(vm.on(i));
    ui.horizontal_wrapped(|ui| {
        if ui.button("New").clicked() {
            send(vm, Input::NewDiagram);
        }
        if ui.button("Open").clicked() {
            send(vm, Input::OpenDiagram);
        }
        if ui.button("Save").clicked() {
            send(vm, Input::SaveDiagram);
        }
        if ui.button("Save As").clicked() {
            send(vm, Input::SaveDiagramAs);
        }
        if ui.button("Sample").clicked() {
            send(vm, Input::CreateSampleDiagram);
        }
        ui.separator();
        if ui
            .add_enabled(vm.can_undo(), egui::Button::new("Undo"))
            .clicked()
        {
            send(vm, Input::Undo);
        }
        if ui
            .add_enabled(vm.can_redo(), egui::Button::new("Redo"))
            .clicked()
        {
            send(vm, Input::Redo);
        }
        let has_node = vm.state.selected_node.is_some();
        if ui
            .add_enabled(vm.state.has_selection(), egui::Button::new("Delete"))
            .clicked()
        {
            send(vm, Input::DeleteSelected);
        }
        if ui
            .add_enabled(has_node, egui::Button::new("Duplicate"))
            .clicked()
        {
            send(vm, Input::DuplicateSelected);
        }
        if ui
            .add_enabled(has_node, egui::Button::new("Bring to Front"))
            .clicked()
        {
            send(vm, Input::BringToFront);
        }
        if ui
            .add_enabled(has_node, egui::Button::new("Send to Back"))
            .clicked()
        {
            send(vm, Input::SendToBack);
        }
        ui.separator();
        let mut shape = vm.state.selected_shape;
        egui::ComboBox::from_id_salt("flowchart-shape")
            .selected_text(format!("{shape:?}"))
            .show_ui(ui, |ui| {
                for s in NodeShape::ALL {
                    ui.selectable_value(&mut shape, s, format!("{s:?}"));
                }
            });
        if shape != vm.state.selected_shape {
            send(vm, Input::SetSelectedShape(shape));
        }
        if ui.button("Add Node").clicked() {
            send(vm, Input::AddNode(None));
        }
        ui.separator();
        if ui
            .selectable_label(vm.state.connect_mode, "Connect")
            .clicked()
        {
            send(vm, Input::ToggleConnectMode);
        }
        if ui.selectable_label(vm.state.pan_mode, "Pan").clicked() {
            send(vm, Input::TogglePanMode);
        }
        if ui
            .selectable_label(vm.state.diagram.snap_to_grid, "Snap")
            .clicked()
        {
            send(vm, Input::ToggleSnapToGrid);
        }
        ui.separator();
        if ui.button("−").on_hover_text("Zoom out").clicked() {
            send(vm, Input::ZoomOut);
        }
        ui.label(format!("{:.0}%", vm.state.zoom * 100.0));
        if ui.button("+").on_hover_text("Zoom in").clicked() {
            send(vm, Input::ZoomIn);
        }
        if ui.button("100%").clicked() {
            send(vm, Input::ZoomReset);
        }
    });
}

fn shortcuts(ui: &egui::Ui, vm: &mut EditorViewModel, fx: &mut Vec<Effect>) {
    if ui.memory(|m| m.focused().is_some()) && ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let pressed = |m: Modifiers, k: Key| ui.input_mut(|i| i.consume_key(m, k));
    let mut inputs = Vec::new();
    if pressed(Modifiers::COMMAND, Key::Z) {
        inputs.push(Input::Undo);
    }
    if pressed(Modifiers::COMMAND, Key::Y) {
        inputs.push(Input::Redo);
    }
    if pressed(Modifiers::COMMAND, Key::D) {
        inputs.push(Input::DuplicateSelected);
    }
    if vm.state.has_selection() && pressed(Modifiers::NONE, Key::Delete) {
        inputs.push(Input::DeleteSelected);
    }
    for i in inputs {
        fx.extend(vm.on(i));
    }
}

fn properties(ui: &mut egui::Ui, vm: &mut EditorViewModel, fx: &mut Vec<Effect>) {
    ui.strong("Properties");
    let Some(id) = vm.state.selected_node.clone() else {
        if let Some(e) = vm
            .state
            .selected_edge
            .as_deref()
            .and_then(|e| vm.state.diagram.edge(e))
        {
            ui.label(format!("Connection {}", short(&e.id)));
            ui.label(format!("{:?} → {:?}", e.source_point, e.target_point));
        } else {
            ui.weak("Nothing selected");
        }
        return;
    };
    let Some(node) = vm.state.diagram.node(&id) else {
        return;
    };
    let (shape, nx, ny, nw, nh) = (node.shape, node.x, node.y, node.width, node.height);
    let buf_id = egui::Id::new(("flowchart-text", &id));
    let mut text = ui
        .data(|m| m.get_temp::<String>(buf_id))
        .unwrap_or_else(|| node.text.clone());
    ui.label(format!("{shape:?}"));
    ui.label("Text");
    let r = ui.text_edit_singleline(&mut text);
    if r.changed() {
        ui.data_mut(|m| m.insert_temp(buf_id, text.clone()));
    }
    if r.lost_focus() {
        ui.data_mut(|m| m.remove::<String>(buf_id));
        fx.extend(vm.on(Input::SetNodeText {
            id: id.clone(),
            text,
        }));
    }
    ui.label(format!("X {nx:.0}  Y {ny:.0}"));
    ui.label(format!("W {nw:.0}  H {nh:.0}"));
}

fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn status_bar(ui: &mut egui::Ui, vm: &EditorViewModel) {
    let d = &vm.state.diagram;
    ui.horizontal(|ui| {
        ui.label(&vm.state.status);
        ui.separator();
        ui.label(format!(
            "Nodes: {}  Connections: {}",
            d.nodes.len(),
            d.edges.len()
        ));
        ui.separator();
        ui.label(format!("Zoom: {:.0}%", vm.state.zoom * 100.0));
        ui.separator();
        ui.label(vm.state.title());
    });
}
