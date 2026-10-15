//! Editor VM tests (C# has no `ViewModel` tests; these cover the behaviour of
//! `FlowChartEditorViewModel` that the canvas relies on).
#![allow(clippy::float_cmp)]

use super::*;

fn vm_with_two() -> (EditorViewModel, String, String) {
    let mut vm = EditorViewModel::new();
    vm.on(Input::AddNode(Some(NodeShape::Rectangle)));
    vm.on(Input::AddNode(Some(NodeShape::Diamond)));
    let a = vm.state.diagram.nodes[0].id.clone();
    let b = vm.state.diagram.nodes[1].id.clone();
    (vm, a, b)
}

#[test]
fn add_node_uses_default_text_offset_and_selects() {
    let (vm, _, b) = vm_with_two();
    let n = &vm.state.diagram.nodes[1];
    assert_eq!(n.text, "Decision");
    assert_eq!((n.x, n.y), (120.0, 120.0));
    assert_eq!(vm.state.selected_node.as_deref(), Some(b.as_str()));
    assert!(n.is_selected);
    assert!(vm.state.modified);
    assert_eq!(vm.state.title(), "Untitled * - FlowChart Editor");
}

#[test]
fn undo_redo_restores_snapshots() {
    let (mut vm, _, _) = vm_with_two();
    assert!(vm.can_undo());
    vm.on(Input::Undo);
    assert_eq!(vm.state.diagram.nodes.len(), 1);
    vm.on(Input::Undo);
    assert!(vm.state.diagram.nodes.is_empty());
    assert!(!vm.can_undo());
    assert_eq!(vm.on(Input::Undo), vec![]);
    vm.on(Input::Redo);
    vm.on(Input::Redo);
    assert_eq!(vm.state.diagram.nodes.len(), 2);
    assert!(!vm.can_redo());
}

#[test]
fn new_action_clears_redo() {
    let (mut vm, _, _) = vm_with_two();
    vm.on(Input::Undo);
    assert!(vm.can_redo());
    vm.on(Input::AddNode(None));
    assert!(!vm.can_redo());
}

#[test]
fn undo_stack_is_capped() {
    let mut vm = EditorViewModel::new();
    for _ in 0..(UNDO_LIMIT + 10) {
        vm.on(Input::AddNode(None));
    }
    let mut n = 0;
    while vm.can_undo() {
        vm.on(Input::Undo);
        n += 1;
    }
    assert_eq!(n, UNDO_LIMIT);
    assert_eq!(vm.state.diagram.nodes.len(), 10);
}

#[test]
fn move_snaps_to_grid_and_one_undo_per_gesture() {
    let (mut vm, a, _) = vm_with_two();
    vm.on(Input::BeginMove);
    vm.on(Input::UpdateNodePosition {
        id: a.clone(),
        x: 131.0,
        y: 49.0,
    });
    vm.on(Input::UpdateNodePosition {
        id: a.clone(),
        x: 209.0,
        y: 251.0,
    });
    let n = vm.state.diagram.node(&a).map(|n| (n.x, n.y));
    assert_eq!(n, Some((200.0, 260.0)));
    vm.on(Input::Undo);
    assert_eq!(
        vm.state.diagram.node(&a).map(|n| (n.x, n.y)),
        Some((100.0, 100.0))
    );
}

#[test]
fn move_without_snap_keeps_exact_position() {
    let (mut vm, a, _) = vm_with_two();
    vm.on(Input::ToggleSnapToGrid);
    vm.on(Input::UpdateNodePosition {
        id: a.clone(),
        x: 131.5,
        y: 49.0,
    });
    assert_eq!(vm.state.diagram.node(&a).map(|n| n.x), Some(131.5));
}

#[test]
fn connect_adds_edge_and_rejects_self_loops() {
    let (mut vm, a, b) = vm_with_two();
    let conn = |s: &str, t: &str| Input::AddConnection {
        source: s.into(),
        target: t.into(),
        source_point: ConnectionPoint::Right,
        target_point: ConnectionPoint::Left,
    };
    assert_eq!(vm.on(conn(&a, &a)), vec![]);
    assert_eq!(vm.on(conn(&a, "missing")), vec![]);
    assert_eq!(vm.on(conn(&a, &b)), vec![Effect::DiagramChanged]);
    assert_eq!(vm.state.diagram.edges.len(), 1);
    assert!(vm.state.selected_edge.is_some());
    assert!(vm.state.selected_node.is_none());
    // Deleting the node removes the edge too.
    vm.on(Input::SelectNode(Some(a)));
    vm.on(Input::DeleteSelected);
    assert!(vm.state.diagram.edges.is_empty());
}

#[test]
fn duplicate_and_z_order() {
    let (mut vm, a, _) = vm_with_two();
    vm.on(Input::SelectNode(Some(a.clone())));
    vm.on(Input::DuplicateSelected);
    assert_eq!(vm.state.diagram.nodes.len(), 3);
    let dup = &vm.state.diagram.nodes[2];
    assert_eq!((dup.x, dup.y), (120.0, 120.0));
    assert_eq!(dup.z_index, 3);
    vm.on(Input::SelectNode(Some(a.clone())));
    vm.on(Input::BringToFront);
    assert_eq!(vm.state.diagram.node(&a).map(|n| n.z_index), Some(4));
    vm.on(Input::SendToBack);
    assert_eq!(vm.state.diagram.node(&a).map(|n| n.z_index), Some(1));
}

#[test]
fn zoom_is_clamped() {
    let mut vm = EditorViewModel::new();
    for _ in 0..40 {
        vm.on(Input::ZoomIn);
    }
    assert!((vm.state.zoom - ZOOM_MAX).abs() < 1e-9);
    vm.on(Input::SetZoom(0.0));
    assert_eq!(vm.state.zoom, ZOOM_MIN);
    vm.on(Input::Pan { dx: 5.0, dy: -3.0 });
    vm.on(Input::ZoomReset);
    assert_eq!((vm.state.zoom, vm.state.pan), (1.0, (0.0, 0.0)));
}

#[test]
fn resize_has_minimums() {
    let (mut vm, a, _) = vm_with_two();
    vm.on(Input::UpdateNodeSize {
        id: a.clone(),
        width: 5.0,
        height: 5.0,
    });
    assert_eq!(
        vm.state.diagram.node(&a).map(|n| (n.width, n.height)),
        Some((40.0, 30.0))
    );
}

#[test]
fn save_goes_through_effects() {
    let mut vm = EditorViewModel::new();
    assert_eq!(vm.on(Input::SaveDiagram), vec![Effect::RequestFileSaveAs]);
    vm.on(Input::FileSaved {
        path: "/tmp/x.drawio".into(),
    });
    assert!(!vm.state.modified);
    assert_eq!(vm.state.title(), "x.drawio - FlowChart Editor");
    match vm.on(Input::SaveDiagram).as_slice() {
        [Effect::Save { format, .. }] => assert_eq!(*format, DiagramFormat::DrawIo),
        other => panic!("unexpected {other:?}"),
    }
    let loaded = Diagram::create_sample();
    vm.on(Input::FileLoaded {
        path: "/tmp/s.afc".into(),
        diagram: Box::new(loaded),
    });
    assert_eq!(vm.state.diagram.nodes.len(), 6);
    assert_eq!(vm.state.status, "Loaded s.afc");
    assert!(vm.can_undo());
}

#[test]
fn set_node_text_is_undoable() {
    let (mut vm, a, _) = vm_with_two();
    vm.on(Input::SetNodeText {
        id: a.clone(),
        text: "Hello".into(),
    });
    assert_eq!(
        vm.state.diagram.node(&a).map(|n| n.text.as_str()),
        Some("Hello")
    );
    vm.on(Input::Undo);
    assert_eq!(
        vm.state.diagram.node(&a).map(|n| n.text.as_str()),
        Some("Process")
    );
}
