//! egui Painter canvas (C# `Controls/FlowChartCanvas.cs`): grid, nodes, edges,
//! hit-testing, drag-move, connect, zoom/pan. All mutations go through `Input`.

use egui::{Align2, Color32, FontId, Mesh, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};

use crate::geometry::{edge_path, hit_edge, hit_node, nearest_anchor, outline, parse_color};
use crate::model::{
    ArrowType, ConnectionPoint, DiagramEdge, DiagramNode, LineStyle, NodeShape, Point,
};
use crate::vm::{EditorViewModel, Effect, Input};

const SELECT: Color32 = Color32::from_rgb(0, 120, 215);
/// Edge pick tolerance in screen points.
const EDGE_PICK: f64 = 6.0;

/// In-progress pointer gesture, kept in egui temp memory between frames.
#[derive(Debug, Clone, PartialEq)]
enum Drag {
    Move {
        id: String,
        grab: (f64, f64),
    },
    Connect {
        source: String,
        point: ConnectionPoint,
        to: Pos2,
    },
    Pan,
}

/// Diagram ↔ screen mapping for one frame.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub origin: Pos2,
    pub zoom: f64,
}

impl View {
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // screen coordinates fit f32
    pub fn to_screen(&self, p: Point) -> Pos2 {
        Pos2::new(
            self.origin.x + (p.x * self.zoom) as f32,
            self.origin.y + (p.y * self.zoom) as f32,
        )
    }

    #[must_use]
    pub fn to_diagram(&self, p: Pos2) -> Point {
        Point::new(
            f64::from(p.x - self.origin.x) / self.zoom,
            f64::from(p.y - self.origin.y) / self.zoom,
        )
    }

    #[allow(clippy::cast_possible_truncation)]
    fn len(&self, v: f64) -> f32 {
        (v * self.zoom) as f32
    }
}

fn color(hex: &str, fallback: Color32) -> Color32 {
    parse_color(hex).map_or(fallback, |[r, g, b, a]| {
        Color32::from_rgba_unmultiplied(r, g, b, a)
    })
}

/// Paints and handles input; returns effects produced by inputs.
pub fn show_canvas(ui: &mut egui::Ui, vm: &mut EditorViewModel) -> Vec<Effect> {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "FlowChart canvas")
    });
    #[allow(clippy::cast_possible_truncation)]
    let view = View {
        origin: rect.min + Vec2::new(vm.state.pan.0 as f32, vm.state.pan.1 as f32),
        zoom: vm.state.zoom,
    };
    let mut fx = Vec::new();
    handle_input(ui, vm, &response, view, &mut fx);
    // Recompute: input may have changed pan/zoom.
    #[allow(clippy::cast_possible_truncation)]
    let view = View {
        origin: rect.min + Vec2::new(vm.state.pan.0 as f32, vm.state.pan.1 as f32),
        zoom: vm.state.zoom,
    };
    let painter = ui.painter_at(rect);
    let bg = color(&vm.state.diagram.background_color, Color32::WHITE);
    painter.rect_filled(rect, 0.0, bg);
    if vm.state.diagram.show_grid {
        draw_grid(&painter, rect, view, vm.state.diagram.grid_size);
    }
    let d = &vm.state.diagram;
    let mut edges: Vec<&DiagramEdge> = d.edges.iter().collect();
    edges.sort_by_key(|e| e.z_index);
    for e in edges {
        if let Some(path) = edge_path(d, e) {
            draw_edge(&painter, view, e, &path);
        }
    }
    let mut nodes: Vec<&DiagramNode> = d.nodes.iter().collect();
    nodes.sort_by_key(|n| n.z_index);
    for n in nodes {
        draw_node(&painter, view, n, vm.state.connect_mode);
    }
    if let Some(Drag::Connect { source, point, to }) = ui.data(|m| m.get_temp::<Drag>(response.id))
        && let Some(n) = d.node(&source)
    {
        let from = view.to_screen(n.anchor(point));
        painter.add(Shape::dashed_line(
            &[from, to],
            Stroke::new(1.5, SELECT),
            6.0,
            4.0,
        ));
    }
    fx
}

#[allow(clippy::too_many_lines)] // one block per gesture phase
fn handle_input(
    ui: &egui::Ui,
    vm: &mut EditorViewModel,
    response: &egui::Response,
    view: View,
    fx: &mut Vec<Effect>,
) {
    let id = response.id;
    if response.hovered() {
        let (zoom_delta, scroll, hover) =
            ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
        if (zoom_delta - 1.0).abs() > f32::EPSILON {
            // Zoom around the cursor: keep the diagram point under it fixed.
            let anchor = hover.map(|p| view.to_diagram(p));
            let old = vm.state.zoom;
            fx.extend(vm.on(Input::SetZoom(old * f64::from(zoom_delta))));
            if let (Some(a), Some(h)) = (anchor, hover) {
                let new = vm.state.zoom;
                let shift = (
                    f64::from(h.x - view.origin.x) - a.x * new,
                    f64::from(h.y - view.origin.y) - a.y * new,
                );
                fx.extend(vm.on(Input::Pan {
                    dx: shift.0,
                    dy: shift.1,
                }));
            }
        } else if scroll != Vec2::ZERO {
            fx.extend(vm.on(Input::Pan {
                dx: f64::from(scroll.x),
                dy: f64::from(scroll.y),
            }));
        }
    }

    if response.drag_started()
        && let Some(pos) = ui
            .input(|i| i.pointer.press_origin())
            .or_else(|| response.interact_pointer_pos())
    {
        let p = view.to_diagram(pos);
        let middle = ui.input(|i| i.pointer.middle_down());
        let hit =
            hit_node(&vm.state.diagram, p).map(|n| (n.id.clone(), n.x, n.y, nearest_anchor(n, p)));
        let drag = match hit {
            _ if middle || vm.state.pan_mode => Drag::Pan,
            Some((nid, _, _, anchor)) if vm.state.connect_mode => Drag::Connect {
                source: nid,
                point: anchor,
                to: pos,
            },
            Some((nid, x, y, _)) => {
                fx.extend(vm.on(Input::SelectNode(Some(nid.clone()))));
                fx.extend(vm.on(Input::BeginMove));
                Drag::Move {
                    id: nid,
                    grab: (p.x - x, p.y - y),
                }
            }
            None => Drag::Pan,
        };
        ui.data_mut(|m| m.insert_temp(id, drag));
    }

    if response.dragged() {
        let drag = ui.data(|m| m.get_temp::<Drag>(id));
        match drag {
            Some(Drag::Move { id: nid, grab }) => {
                if let Some(pos) = response.interact_pointer_pos() {
                    let p = view.to_diagram(pos);
                    fx.extend(vm.on(Input::UpdateNodePosition {
                        id: nid,
                        x: p.x - grab.0,
                        y: p.y - grab.1,
                    }));
                }
            }
            Some(Drag::Pan) => {
                let dlt = response.drag_delta();
                fx.extend(vm.on(Input::Pan {
                    dx: f64::from(dlt.x),
                    dy: f64::from(dlt.y),
                }));
            }
            Some(Drag::Connect { source, point, .. }) => {
                if let Some(to) = response.interact_pointer_pos() {
                    ui.data_mut(|m| m.insert_temp(id, Drag::Connect { source, point, to }));
                }
            }
            None => {}
        }
    }

    if response.drag_stopped() {
        if let Some(Drag::Connect { source, point, to }) = ui.data(|m| m.get_temp::<Drag>(id)) {
            let p = view.to_diagram(to);
            if let Some((target, tp)) =
                hit_node(&vm.state.diagram, p).map(|n| (n.id.clone(), nearest_anchor(n, p)))
            {
                fx.extend(vm.on(Input::AddConnection {
                    source,
                    target,
                    source_point: point,
                    target_point: tp,
                }));
            }
        }
        ui.data_mut(|m| m.remove::<Drag>(id));
    }

    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        response.request_focus();
        let p = view.to_diagram(pos);
        let d = &vm.state.diagram;
        let input = if let Some(n) = hit_node(d, p) {
            Input::SelectNode(Some(n.id.clone()))
        } else if let Some(e) = hit_edge(d, p, EDGE_PICK / vm.state.zoom) {
            Input::SelectEdge(Some(e.id.clone()))
        } else {
            Input::ClearSelection
        };
        fx.extend(vm.on(input));
    }

    if response.double_clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let p = view.to_diagram(pos);
        if hit_node(&vm.state.diagram, p).is_none() {
            let shape = vm.state.selected_shape;
            fx.extend(vm.on(Input::AddNodeAt {
                shape,
                x: p.x,
                y: p.y,
            }));
        }
    }
}

fn draw_grid(painter: &egui::Painter, rect: Rect, view: View, grid: f64) {
    let step = view.len(grid);
    if step < 4.0 {
        return;
    }
    let stroke = Stroke::new(0.5, Color32::from_gray(225));
    let mut x = view.origin.x + ((rect.left() - view.origin.x) / step).floor() * step;
    while x <= rect.right() {
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            stroke,
        );
        x += step;
    }
    let mut y = view.origin.y + ((rect.top() - view.origin.y) / step).floor() * step;
    while y <= rect.bottom() {
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            stroke,
        );
        y += step;
    }
}

/// Fan-triangulated fill around the bounding-box centre (handles the star-shaped outlines).
fn fill(painter: &egui::Painter, pts: &[Pos2], center: Pos2, c: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, c);
    for p in pts {
        mesh.colored_vertex(*p, c);
    }
    #[allow(clippy::cast_possible_truncation)]
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

fn draw_node(painter: &egui::Painter, view: View, n: &DiagramNode, connect_mode: bool) {
    let r = Rect::from_min_max(
        view.to_screen(Point::new(n.x, n.y)),
        view.to_screen(Point::new(n.x + n.width, n.y + n.height)),
    );
    let fill_c = color(&n.fill_color, Color32::WHITE);
    #[allow(clippy::cast_possible_truncation)]
    let stroke = Stroke::new(
        (n.stroke_width * view.zoom).max(0.5) as f32,
        color(&n.stroke_color, Color32::BLACK),
    );
    match n.shape {
        NodeShape::RoundedRectangle => {
            let radius = (r.height() * 0.2).min(12.0);
            painter.rect_filled(r, radius, fill_c);
            painter.rect_stroke(r, radius, stroke, StrokeKind::Middle);
        }
        NodeShape::Cylinder => {
            let ry = (r.height() * 0.12).max(2.0);
            let body = Rect::from_min_max(r.min + Vec2::new(0.0, ry), r.max - Vec2::new(0.0, ry));
            painter.rect_filled(body, 0.0, fill_c);
            let top = egui::epaint::EllipseShape {
                center: Pos2::new(r.center().x, r.top() + ry),
                radius: Vec2::new(r.width() / 2.0, ry),
                fill: fill_c,
                stroke,
                angle: 0.0,
            };
            let bottom = egui::epaint::EllipseShape {
                center: Pos2::new(r.center().x, r.bottom() - ry),
                radius: Vec2::new(r.width() / 2.0, ry),
                fill: fill_c,
                stroke,
                angle: 0.0,
            };
            painter.add(bottom);
            painter.line_segment([body.left_top(), body.left_bottom()], stroke);
            painter.line_segment([body.right_top(), body.right_bottom()], stroke);
            painter.add(top);
        }
        _ => {
            let pts: Vec<Pos2> = outline(n).into_iter().map(|p| view.to_screen(p)).collect();
            fill(painter, &pts, r.center(), fill_c);
            painter.add(Shape::closed_line(pts, stroke));
        }
    }
    #[allow(clippy::cast_possible_truncation)]
    let font = FontId::proportional(view.len(n.font_size).max(4.0));
    painter.text(
        r.center(),
        Align2::CENTER_CENTER,
        &n.text,
        font,
        color(&n.text_color, Color32::BLACK),
    );
    if n.is_selected {
        painter.rect_stroke(
            r.expand(3.0),
            0.0,
            Stroke::new(1.5, SELECT),
            StrokeKind::Outside,
        );
    }
    if connect_mode || n.is_selected {
        for cp in ConnectionPoint::SIDES {
            painter.circle(
                view.to_screen(n.anchor(cp)),
                4.0,
                Color32::WHITE,
                Stroke::new(1.0, SELECT),
            );
        }
    }
}

fn draw_edge(painter: &egui::Painter, view: View, e: &DiagramEdge, path: &[Point]) {
    let pts: Vec<Pos2> = path.iter().map(|p| view.to_screen(*p)).collect();
    let c = if e.is_selected {
        SELECT
    } else {
        color(&e.stroke_color, Color32::BLACK)
    };
    #[allow(clippy::cast_possible_truncation)]
    let stroke = Stroke::new(
        (e.stroke_width * view.zoom).max(0.5) as f32 + if e.is_selected { 1.0 } else { 0.0 },
        c,
    );
    match e.style {
        LineStyle::Solid => {
            painter.add(Shape::line(pts.clone(), stroke));
        }
        LineStyle::Dashed => {
            painter.extend(Shape::dashed_line(&pts, stroke, 8.0, 4.0));
        }
        LineStyle::Dotted => {
            painter.extend(Shape::dotted_line(&pts, c, 4.0, stroke.width / 2.0 + 0.5));
        }
        LineStyle::DashDot => {
            painter.extend(Shape::dashed_line(&pts, stroke, 12.0, 3.0));
        }
    }
    if let [.., a, b] = pts.as_slice() {
        arrow_head(painter, *a, *b, e.target_arrow, c, stroke);
    }
    if let [a, b, ..] = pts.as_slice() {
        arrow_head(painter, *b, *a, e.source_arrow, c, stroke);
    }
    if !e.label.is_empty() {
        let mid = pts.len() / 2;
        let at = if pts.len().is_multiple_of(2) && mid > 0 {
            pts[mid - 1].lerp(pts[mid], 0.5)
        } else {
            pts[mid]
        };
        let galley_font = FontId::proportional(view.len(12.0).max(4.0));
        let rect = painter.text(at, Align2::CENTER_CENTER, &e.label, galley_font.clone(), c);
        painter.rect_filled(rect.expand(2.0), 2.0, Color32::from_white_alpha(230));
        painter.text(at, Align2::CENTER_CENTER, &e.label, galley_font, c);
    }
}

fn arrow_head(
    painter: &egui::Painter,
    from: Pos2,
    tip: Pos2,
    kind: ArrowType,
    c: Color32,
    stroke: Stroke,
) {
    let dir = (tip - from).normalized();
    if kind == ArrowType::None || !dir.x.is_finite() {
        return;
    }
    let size = 10.0;
    let back = tip - dir * size;
    let wing = dir.rot90() * (size * 0.5);
    match kind {
        ArrowType::Arrow => painter.add(Shape::convex_polygon(
            vec![tip, back + wing, back - wing],
            c,
            Stroke::NONE,
        )),
        ArrowType::OpenArrow => {
            painter.add(Shape::line(vec![back + wing, tip, back - wing], stroke))
        }
        ArrowType::Diamond => {
            let far = tip - dir * size * 2.0;
            painter.add(Shape::convex_polygon(
                vec![tip, back + wing, far, back - wing],
                c,
                Stroke::NONE,
            ))
        }
        ArrowType::Circle => painter.add(Shape::circle_filled(tip - dir * 4.0, 4.0, c)),
        ArrowType::Square => painter.add(Shape::rect_filled(
            Rect::from_center_size(tip - dir * 4.0, Vec2::splat(8.0)),
            0.0,
            c,
        )),
        ArrowType::None => return,
    };
}
