//! `egui_kittest` flows for the flow chart editor: add node, drag (with snap), connect, undo.

use arcana_plugin_flowchart::ui::show_editor;
use arcana_plugin_flowchart::{Diagram, EditorViewModel, Input};
use egui::{Event, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn harness(vm: EditorViewModel) -> Harness<'static, EditorViewModel> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui_state(
            |ui, vm: &mut EditorViewModel| {
                let _ = show_editor(ui, vm);
            },
            vm,
        );
    h.run();
    h
}

fn press_drag_release(h: &mut Harness<'_, EditorViewModel>, from: Pos2, to: Pos2) {
    let input = |h: &mut Harness<'_, EditorViewModel>, e: Event| h.input_mut().events.push(e);
    input(h, Event::PointerMoved(from));
    h.step();
    input(
        h,
        Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
    );
    h.step();
    let steps = 6;
    for i in 1..=steps {
        #[allow(clippy::cast_precision_loss)]
        let t = i as f32 / steps as f32;
        input(h, Event::PointerMoved(from.lerp(to, t)));
        h.step();
    }
    input(
        h,
        Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    );
    h.step();
    h.run();
}

/// Screen position of diagram (0,0): the labelled canvas rect's top-left (pan is 0).
fn locate(h: &mut Harness<'_, EditorViewModel>) -> Pos2 {
    h.get_by_label("FlowChart canvas").rect().min
}

fn click(h: &mut Harness<'_, EditorViewModel>, at: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.step();
    h.input_mut().events.push(Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.step();
    h.input_mut().events.push(Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.step();
    h.run();
}

#[test]
fn add_node_via_palette_then_undo_redo() {
    let mut h = harness(EditorViewModel::new());
    h.get_by_label("Rectangle").click();
    h.run();
    assert_eq!(h.state().state.diagram.nodes.len(), 1);
    assert!(h.state().state.selected_node.is_some());
    h.get_by_label("Rectangle").click();
    h.run();
    assert_eq!(h.state().state.diagram.nodes.len(), 2);
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(h.state().state.diagram.nodes.len(), 1);
    h.get_by_label("Redo").click();
    h.run();
    assert_eq!(h.state().state.diagram.nodes.len(), 2);
}

#[test]
fn drag_node_snaps_to_grid_and_undoes() {
    let mut vm = EditorViewModel::new();
    vm.on(Input::AddNode(None)); // at diagram (100,100), 120x60
    let mut h = harness(vm);
    let origin = locate(&mut h);
    let grab = origin + egui::vec2(160.0, 130.0);
    press_drag_release(&mut h, grab, grab + egui::vec2(87.0, 43.0));
    let n = &h.state().state.diagram.nodes[0];
    assert_eq!(
        (n.x % 20.0, n.y % 20.0),
        (0.0, 0.0),
        "snapped: {},{}",
        n.x,
        n.y
    );
    assert!(n.x > 150.0 && n.y > 120.0, "moved: {},{}", n.x, n.y);
    h.get_by_label("Undo").click();
    h.run();
    let n = &h.state().state.diagram.nodes[0];
    assert_eq!((n.x, n.y), (100.0, 100.0));
}

#[test]
fn connect_mode_drag_creates_edge() {
    let mut vm = EditorViewModel::new();
    vm.on(Input::AddNode(None)); // (100,100)
    vm.on(Input::AddNodeAt {
        shape: arcana_plugin_flowchart::NodeShape::Diamond,
        x: 400.0,
        y: 100.0,
    });
    let mut h = harness(vm);
    let origin = locate(&mut h);
    h.get_by_label("Connect").click();
    h.run();
    assert!(h.state().state.connect_mode);
    press_drag_release(
        &mut h,
        origin + egui::vec2(215.0, 130.0),
        origin + egui::vec2(410.0, 130.0),
    );
    let d = &h.state().state.diagram;
    assert_eq!(d.edges.len(), 1, "edge created");
    assert_eq!(d.edges[0].source_node_id, d.nodes[0].id);
    assert_eq!(d.edges[0].target_node_id, d.nodes[1].id);
}

#[test]
fn sample_renders_and_zoom_buttons_work() {
    let mut h = harness(EditorViewModel::with_diagram(Diagram::create_sample()));
    h.get_by_label("Zoom In").click();
    h.run();
    assert!((h.state().state.zoom - 1.1).abs() < 1e-9);
    h.get_by_label("Reset Zoom").click();
    h.run();
    assert!((h.state().state.zoom - 1.0).abs() < 1e-9);
    assert!(h.query_by_label_contains("6 nodes, 6 edges").is_some());
}

#[test]
fn click_selects_node_and_edge_and_empty_clears() {
    let mut h = harness(EditorViewModel::with_diagram(Diagram::create_sample()));
    let origin = locate(&mut h);
    // "Start" ellipse at (400,50) 100x50.
    click(&mut h, origin + egui::vec2(450.0, 75.0));
    let start_id = h.state().state.diagram.nodes[0].id.clone();
    assert_eq!(
        h.state().state.selected_node.as_deref(),
        Some(start_id.as_str())
    );
    // Start→Process1 edge runs vertically at x=450 between y=100 and y=150.
    click(&mut h, origin + egui::vec2(451.0, 125.0));
    let edge_id = h.state().state.diagram.edges[0].id.clone();
    assert_eq!(
        h.state().state.selected_edge.as_deref(),
        Some(edge_id.as_str())
    );
    click(&mut h, origin + egui::vec2(30.0, 30.0));
    assert!(!h.state().state.has_selection());
}

#[test]
fn palette_drag_drops_shape_on_canvas() {
    let mut h = harness(EditorViewModel::new());
    let origin = locate(&mut h);
    let from = h.get_by_label("Star").rect().center();
    press_drag_release(&mut h, from, origin + egui::vec2(300.0, 300.0));
    let d = &h.state().state.diagram;
    assert_eq!(d.nodes.len(), 1, "dropped");
    assert_eq!(d.nodes[0].shape, arcana_plugin_flowchart::NodeShape::Star);
    assert!(d.nodes[0].x > 200.0 && d.nodes[0].y > 200.0);
}

#[test]
fn resize_handle_drag_resizes_with_one_undo() {
    let mut vm = EditorViewModel::new();
    vm.on(Input::AddNode(None)); // (100,100) 120x60, selected
    let mut h = harness(vm);
    let origin = locate(&mut h);
    // Bottom-right handle at (220,160).
    press_drag_release(
        &mut h,
        origin + egui::vec2(220.0, 160.0),
        origin + egui::vec2(281.0, 201.0),
    );
    let n = &h.state().state.diagram.nodes[0];
    assert_eq!((n.x, n.y), (100.0, 100.0), "top-left fixed");
    assert!(
        n.width > 170.0 && n.height > 90.0,
        "{}x{}",
        n.width,
        n.height
    );
    h.get_by_label("Undo").click();
    h.run();
    let n = &h.state().state.diagram.nodes[0];
    assert_eq!((n.width, n.height), (120.0, 60.0));
}

#[test]
fn dragging_segment_midpoint_adds_a_bend() {
    let mut vm = EditorViewModel::new();
    vm.on(Input::AddNode(None)); // (100,100)
    vm.on(Input::AddNodeAt {
        shape: arcana_plugin_flowchart::NodeShape::Rectangle,
        x: 400.0,
        y: 100.0,
    });
    let (src, dst) = (
        vm.state.diagram.nodes[0].id.clone(),
        vm.state.diagram.nodes[1].id.clone(),
    );
    vm.on(Input::AddConnection {
        source: src,
        target: dst,
        source_point: arcana_plugin_flowchart::model::ConnectionPoint::Right,
        target_point: arcana_plugin_flowchart::model::ConnectionPoint::Left,
    });
    let mut h = harness(vm);
    let origin = locate(&mut h);
    // Straight edge (220,130)→(400,130): midpoint (310,130).
    press_drag_release(
        &mut h,
        origin + egui::vec2(310.0, 130.0),
        origin + egui::vec2(310.0, 230.0),
    );
    let e = &h.state().state.diagram.edges[0];
    assert!(!e.waypoints.is_empty(), "bend added");
    let w = *e
        .waypoints
        .iter()
        .find(|w| w.y > 200.0)
        .expect("a bend was dragged down");
    assert!(w.y > 200.0, "bend dragged: {w:?}");
}

#[test]
fn diagram_properties_show_when_nothing_selected() {
    let mut h = harness(EditorViewModel::new());
    assert!(h.query_by_label("Diagram Name").is_some());
    eprintln!(
        "{:?} canvas {:?} rect {:?} name {:?}",
        h.get_by_label("Show Grid").rect(),
        h.get_by_label("FlowChart canvas").rect(),
        h.get_by_label("Rectangle").rect(),
        h.get_by_label("Diagram Name").rect()
    );
    h.get_by_label("Show Grid").click();
    h.run();
    assert!(!h.state().state.diagram.show_grid);
    h.get_by_label("Snap to Grid").click();
    h.run();
    assert_eq!(
        h.state().state.diagram.snap_to_grid,
        !EditorViewModel::new().state.diagram.snap_to_grid
    );
}
