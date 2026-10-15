//! egui editor page (C# `Views/FlowChartEditorPage.xaml`): toolbar, shapes palette, canvas,
//! properties panel, status bar. Host owns file dialogs/I-O and handles returned effects.

mod canvas;

pub use canvas::{View, show_canvas};

use egui::{Key, Modifiers, RichText};
use egui_phosphor::regular as icon;

use crate::model::NodeShape;
use crate::vm::{EditorViewModel, Effect, GRID_MAX, GRID_MIN, Input};

/// Register the Phosphor icon font used by the editor toolbar. The host calls this once
/// while building its `FontDefinitions` (icons fall back to boxes without it).
pub fn add_icon_font(defs: &mut egui::FontDefinitions) {
    egui_phosphor::add_to_fonts(defs, egui_phosphor::Variant::Regular);
}

const PALETTE_W: f32 = 180.0;
const PROPS_W: f32 = 230.0;

/// Draws the whole editor into `ui`; returns effects for the host (file dialogs, toasts).
pub fn show_editor(ui: &mut egui::Ui, vm: &mut EditorViewModel) -> Vec<Effect> {
    let mut fx = Vec::new();
    toolbar(ui, vm, &mut fx);
    ui.separator();
    shortcuts(ui, vm, &mut fx);
    let status_h = ui.text_style_height(&egui::TextStyle::Body) + 10.0;
    let avail = ui.available_size();
    let body_h = (avail.y - status_h).max(50.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(PALETTE_W, body_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(PALETTE_W);
                palette(ui, vm, &mut fx);
            },
        );
        let canvas_w = (ui.available_width() - PROPS_W - 8.0).max(50.0);
        ui.allocate_ui(egui::vec2(canvas_w, body_h), |ui| {
            fx.extend(show_canvas(ui, vm));
        });
        ui.allocate_ui_with_layout(
            egui::vec2(PROPS_W, body_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(PROPS_W);
                egui::ScrollArea::vertical()
                    .id_salt("flowchart-props")
                    .show(ui, |ui| properties(ui, vm, &mut fx));
            },
        );
    });
    status_bar(ui, vm);
    fx
}

fn tool(ui: &mut egui::Ui, enabled: bool, glyph: &str, label: &str) -> bool {
    named_tool(ui, enabled, glyph, label, label)
}

/// Icon + text button whose accessible name is `name` (without the icon glyph).
fn named_tool(ui: &mut egui::Ui, enabled: bool, glyph: &str, label: &str, name: &str) -> bool {
    let r = ui.add_enabled(
        enabled,
        egui::Button::new(format!("{glyph} {label}")).frame(false),
    );
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    r.clicked()
}

fn toolbar(ui: &mut egui::Ui, vm: &mut EditorViewModel, fx: &mut Vec<Effect>) {
    let mut inputs = Vec::new();
    ui.horizontal_wrapped(|ui| {
        for (glyph, label, input) in [
            (icon::FILE_PLUS, "New", Input::NewDiagram),
            (icon::FOLDER_OPEN, "Open", Input::OpenDiagram),
            (icon::FLOPPY_DISK, "Save", Input::SaveDiagram),
            (icon::FLOPPY_DISK_BACK, "Save As", Input::SaveDiagramAs),
        ] {
            if tool(ui, true, glyph, label) {
                inputs.push(input);
            }
        }
        ui.separator();
        if tool(ui, vm.can_undo(), icon::ARROW_COUNTER_CLOCKWISE, "Undo") {
            inputs.push(Input::Undo);
        }
        if tool(ui, vm.can_redo(), icon::ARROW_CLOCKWISE, "Redo") {
            inputs.push(Input::Redo);
        }
        ui.separator();
        let has_node = vm.state.selected_node.is_some();
        if tool(ui, vm.state.has_selection(), icon::TRASH, "Delete") {
            inputs.push(Input::DeleteSelected);
        }
        if tool(ui, has_node, icon::COPY, "Duplicate") {
            inputs.push(Input::DuplicateSelected);
        }
        ui.separator();
        if tool(ui, true, icon::MAGNIFYING_GLASS_PLUS, "Zoom In") {
            inputs.push(Input::ZoomIn);
        }
        if tool(ui, true, icon::MAGNIFYING_GLASS_MINUS, "Zoom Out") {
            inputs.push(Input::ZoomOut);
        }
        let pct = format!("{:.0}%", vm.state.zoom * 100.0);
        if named_tool(ui, true, icon::MAGNIFYING_GLASS, &pct, "Reset Zoom") {
            inputs.push(Input::ZoomReset);
        }
        ui.separator();
        let connect = format!("{} Connect", icon::FLOW_ARROW);
        let r = ui.add(
            egui::Button::selectable(vm.state.connect_mode, connect).frame_when_inactive(false),
        );
        r.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                true,
                vm.state.connect_mode,
                "Connect",
            )
        });
        if r.clicked() {
            inputs.push(Input::ToggleConnectMode);
        }
        ui.menu_button(icon::DOTS_THREE, |ui| {
            let items = [
                (icon::SPARKLE, "Sample", Input::CreateSampleDiagram, true),
                (icon::STACK, "Bring to Front", Input::BringToFront, has_node),
                (
                    icon::STACK_SIMPLE,
                    "Send to Back",
                    Input::SendToBack,
                    has_node,
                ),
            ];
            for (glyph, label, input, enabled) in items {
                if tool(ui, enabled, glyph, label) {
                    inputs.push(input);
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .selectable_label(vm.state.pan_mode, format!("{} Pan", icon::HAND))
                .clicked()
            {
                inputs.push(Input::TogglePanMode);
            }
        });
    });
    for i in inputs {
        fx.extend(vm.on(i));
    }
}

/// Small painted preview of a shape inside `rect`.
fn shape_icon(painter: &egui::Painter, rect: egui::Rect, shape: NodeShape, stroke: egui::Stroke) {
    let n = crate::model::DiagramNode {
        shape,
        x: f64::from(rect.left()),
        y: f64::from(rect.top()),
        width: f64::from(rect.width()),
        height: f64::from(rect.height()),
        ..crate::model::DiagramNode::default()
    };
    #[allow(clippy::cast_possible_truncation)]
    let pts: Vec<egui::Pos2> = crate::geometry::outline(&n)
        .into_iter()
        .map(|p| egui::pos2(p.x as f32, p.y as f32))
        .collect();
    painter.add(egui::Shape::closed_line(pts, stroke));
}

/// C# "Shapes" list: click adds at the default spot, drag drops on the canvas.
fn palette(ui: &mut egui::Ui, vm: &mut EditorViewModel, fx: &mut Vec<Effect>) {
    ui.label(RichText::new("Shapes").size(18.0).strong());
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("flowchart-palette")
        .show(ui, |ui| {
            for shape in NodeShape::ALL {
                let (rect, r) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 28.0),
                    egui::Sense::click_and_drag(),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{shape:?}"))
                });
                let r = r.on_hover_text("Click to add, or drag onto the canvas");
                let visuals = ui.style().interact(&r);
                ui.painter().rect(
                    rect.shrink(1.0),
                    4.0,
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                    egui::StrokeKind::Inside,
                );
                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(rect.left() + 18.0, rect.center().y),
                    egui::vec2(20.0, 14.0),
                );
                shape_icon(
                    ui.painter(),
                    icon_rect,
                    shape,
                    egui::Stroke::new(1.2, visuals.text_color()),
                );
                ui.painter().text(
                    egui::pos2(rect.left() + 36.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("{shape:?}"),
                    egui::FontId::proportional(14.0),
                    visuals.text_color(),
                );
                if r.drag_started() {
                    egui::DragAndDrop::set_payload(ui.ctx(), shape);
                }
                if r.dragged()
                    && let Some(p) = ui.ctx().pointer_interact_pos()
                {
                    egui::Area::new(egui::Id::new("flowchart-dnd-ghost"))
                        .order(egui::Order::Tooltip)
                        .fixed_pos(p + egui::vec2(8.0, 8.0))
                        .interactable(false)
                        .show(ui.ctx(), |ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(48.0, 32.0), egui::Sense::hover());
                            shape_icon(
                                ui.painter(),
                                rect,
                                shape,
                                egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 120, 215)),
                            );
                        });
                }
                if r.clicked() {
                    fx.extend(vm.on(Input::AddNode(Some(shape))));
                }
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

/// Text field that commits to the VM when it loses focus (one undo step per edit).
fn committed_text(
    ui: &mut egui::Ui,
    key: egui::Id,
    current: &str,
    multiline: bool,
) -> Option<String> {
    let buf_id = key;
    let mut text = ui
        .data(|m| m.get_temp::<String>(buf_id))
        .unwrap_or_else(|| current.to_string());
    let edit = if multiline {
        egui::TextEdit::multiline(&mut text).desired_rows(4)
    } else {
        egui::TextEdit::singleline(&mut text)
    };
    let r = ui.add(edit.desired_width(f32::INFINITY));
    if r.changed() {
        ui.data_mut(|m| m.insert_temp(buf_id, text.clone()));
    }
    if r.lost_focus() {
        ui.data_mut(|m| m.remove::<String>(buf_id));
        return Some(text);
    }
    None
}

fn diagram_properties(ui: &mut egui::Ui, vm: &EditorViewModel, out: &mut Vec<Input>) {
    let d = &vm.state.diagram;
    ui.label("Diagram Name");
    if let Some(t) = committed_text(
        ui,
        egui::Id::new(("flowchart-diagram-name", &d.id)),
        &d.name,
        false,
    ) {
        out.push(Input::SetDiagramName(t));
    }
    ui.add_space(4.0);
    ui.label("Description");
    if let Some(t) = committed_text(
        ui,
        egui::Id::new(("flowchart-diagram-desc", &d.id)),
        &d.description,
        true,
    ) {
        out.push(Input::SetDiagramDescription(t));
    }
    ui.add_space(4.0);
    let mut grid = d.show_grid;
    if ui.checkbox(&mut grid, "Show Grid").changed() {
        out.push(Input::SetShowGrid(grid));
    }
    let mut snap = d.snap_to_grid;
    if ui.checkbox(&mut snap, "Snap to Grid").changed() {
        out.push(Input::SetSnapToGrid(snap));
    }
    ui.label("Grid Size");
    let mut size = d.grid_size;
    if ui
        .add(
            egui::DragValue::new(&mut size)
                .range(GRID_MIN..=GRID_MAX)
                .speed(1.0),
        )
        .changed()
    {
        out.push(Input::SetGridSize(size));
    }
}

fn properties(ui: &mut egui::Ui, vm: &mut EditorViewModel, fx: &mut Vec<Effect>) {
    ui.label(RichText::new("Properties").size(18.0).strong());
    ui.add_space(4.0);
    let mut out = Vec::new();
    if let Some(id) = vm.state.selected_node.clone() {
        if let Some(node) = vm.state.diagram.node(&id) {
            ui.label(format!("Shape: {:?}", node.shape));
            ui.label("Text");
            if let Some(text) = committed_text(
                ui,
                egui::Id::new(("flowchart-text", &id)),
                &node.text,
                false,
            ) {
                out.push(Input::SetNodeText {
                    id: id.clone(),
                    text,
                });
            }
            let mut b = [node.x, node.y, node.width, node.height];
            let mut changed = false;
            egui::Grid::new("flowchart-bounds")
                .num_columns(4)
                .show(ui, |ui| {
                    for (row, labels) in [(0, ["X", "Y"]), (2, ["W", "H"])] {
                        for (k, label) in labels.iter().enumerate() {
                            ui.label(*label);
                            changed |= ui
                                .add(egui::DragValue::new(&mut b[row + k]).speed(1.0))
                                .changed();
                        }
                        ui.end_row();
                    }
                });
            if changed {
                out.push(Input::SetNodeBounds {
                    id,
                    x: b[0],
                    y: b[1],
                    width: b[2],
                    height: b[3],
                });
            }
        }
    } else if let Some(e) = vm
        .state
        .selected_edge
        .as_deref()
        .and_then(|e| vm.state.diagram.edge(e))
    {
        ui.label(format!("Connection {}", short(&e.id)));
        ui.label(format!("{:?} → {:?}", e.source_point, e.target_point));
        ui.label(format!("Bend points: {}", e.waypoints.len()));
        ui.weak("Drag a ◆ handle to add a bend, double-click a bend to remove it.");
    } else {
        diagram_properties(ui, vm, &mut out);
    }
    for i in out {
        fx.extend(vm.on(i));
    }
}

fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn status_bar(ui: &mut egui::Ui, vm: &EditorViewModel) {
    let d = &vm.state.diagram;
    ui.horizontal(|ui| {
        ui.label(&vm.state.status);
        ui.separator();
        ui.label(vm.state.title());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{} nodes, {} edges", d.nodes.len(), d.edges.len()));
            ui.label(format!("{:.0}%", vm.state.zoom * 100.0));
        });
    });
}
