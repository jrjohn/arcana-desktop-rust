//! Ports of C# `DiagramNodeTests` (6) and `DiagramTests` (9).
#![allow(clippy::float_cmp)] // exact values are assigned, not computed

use super::*;

// ---- DiagramNodeTests ----

// C#: Constructor_SetsDefaultValues
#[test]
fn constructor_sets_default_values() {
    let node = DiagramNode::default();
    assert_ne!(node.id.len(), 0);
    assert_eq!(node.shape, NodeShape::Rectangle);
    assert_eq!(node.width, 120.0);
    assert_eq!(node.height, 60.0);
    assert_eq!(node.fill_color, "#FFFFFF");
    assert_eq!(node.stroke_color, "#000000");
    assert_eq!(node.stroke_width, 2.0);
    assert_eq!(node.font_size, 14.0);
    assert_eq!(node.text.len(), 0);
}

// C#: Clone_CreatesDeepCopy
#[test]
fn clone_creates_deep_copy() {
    let mut original = DiagramNode {
        shape: NodeShape::Diamond,
        x: 100.0,
        y: 200.0,
        width: 150.0,
        height: 80.0,
        text: "Original Node".into(),
        fill_color: "#FF0000".into(),
        stroke_color: "#00FF00".into(),
        stroke_width: 3.0,
        font_size: 16.0,
        z_index: 5,
        ..DiagramNode::default()
    };
    original.metadata.insert("key1".into(), "value1".into());
    let clone = original.duplicate();
    assert_ne!(clone.id, original.id);
    assert_eq!(clone.shape, NodeShape::Diamond);
    assert_eq!((clone.x, clone.y), (120.0, 220.0));
    assert_eq!((clone.width, clone.height), (150.0, 80.0));
    assert_eq!(clone.text, "Original Node");
    assert_eq!(clone.fill_color, "#FF0000");
    assert_eq!(clone.stroke_color, "#00FF00");
    assert_eq!(clone.stroke_width, 3.0);
    assert_eq!(clone.font_size, 16.0);
    assert_eq!(clone.z_index, 5);
    assert_eq!(
        clone.metadata.get("key1").map(String::as_str),
        Some("value1")
    );
}

// C#: Clone_MetadataIsIndependent
#[test]
fn clone_metadata_is_independent() {
    let mut original = DiagramNode::default();
    original.metadata.insert("key1".into(), "original".into());
    let mut clone = original.duplicate();
    clone.metadata.insert("key1".into(), "modified".into());
    clone.metadata.insert("key2".into(), "new".into());
    assert_eq!(original.metadata["key1"], "original");
    assert!(!original.metadata.contains_key("key2"));
}

// C#: Shape_CanBeSetToAnyValue (Theory, 8 InlineData)
#[test]
fn shape_can_be_set_to_any_value() {
    for shape in [
        NodeShape::Rectangle,
        NodeShape::Diamond,
        NodeShape::Ellipse,
        NodeShape::Parallelogram,
        NodeShape::Cylinder,
        NodeShape::Cloud,
        NodeShape::Document,
        NodeShape::Hexagon,
    ] {
        let node = DiagramNode {
            shape,
            ..DiagramNode::default()
        };
        assert_eq!(node.shape, shape);
    }
}

// C#: IsSelected_DefaultsFalse
#[test]
fn is_selected_defaults_false() {
    assert!(!DiagramNode::default().is_selected);
}

// C#: Id_IsUniquePerInstance
#[test]
fn id_is_unique_per_instance() {
    assert_ne!(DiagramNode::default().id, DiagramNode::default().id);
}

// ---- DiagramTests ----

// C#: CreateNew_ReturnsEmptyDiagram
#[test]
fn create_new_returns_empty_diagram() {
    let d = Diagram::create_new("Test Diagram");
    assert_eq!(d.name, "Test Diagram");
    assert_eq!(d.nodes.len(), 0);
    assert_eq!(d.edges.len(), 0);
    assert!(d.show_grid);
    assert!(d.snap_to_grid);
}

// C#: CreateSample_ReturnsPopulatedDiagram
#[test]
fn create_sample_returns_populated_diagram() {
    let d = Diagram::create_sample();
    assert_eq!(d.name, "Sample Flowchart");
    assert_eq!(d.nodes.len(), 6);
    assert_eq!(d.edges.len(), 6);
}

// C#: AddNode_AddsNodeToList
#[test]
fn add_node_adds_node_to_list() {
    let mut d = Diagram::create_new("x");
    d.add_node(DiagramNode {
        text: "Test Node".into(),
        ..DiagramNode::default()
    });
    assert_eq!(d.nodes.len(), 1);
    assert_eq!(d.nodes[0].text, "Test Node");
}

fn named(text: &str) -> DiagramNode {
    DiagramNode {
        text: text.into(),
        ..DiagramNode::default()
    }
}

fn link(a: &DiagramNode, b: &DiagramNode) -> DiagramEdge {
    DiagramEdge {
        source_node_id: a.id.clone(),
        target_node_id: b.id.clone(),
        ..DiagramEdge::default()
    }
}

// C#: RemoveNode_RemovesNodeAndConnectedEdges
#[test]
fn remove_node_removes_node_and_connected_edges() {
    let mut d = Diagram::create_new("x");
    let (n1, n2) = (named("Node 1"), named("Node 2"));
    let e = link(&n1, &n2);
    let id1 = n1.id.clone();
    d.add_node(n1);
    d.add_node(n2);
    d.add_edge(e);
    d.remove_node(&id1);
    assert_eq!(d.nodes.len(), 1);
    assert_eq!(d.nodes[0].text, "Node 2");
    assert_eq!(d.edges.len(), 0);
}

// C#: GetNode_ReturnsCorrectNode
#[test]
fn get_node_returns_correct_node() {
    let mut d = Diagram::create_new("x");
    let n = named("Find Me");
    let id = n.id.clone();
    d.add_node(n);
    assert_eq!(d.node(&id).map(|n| n.text.as_str()), Some("Find Me"));
}

// C#: GetNode_ReturnsNullForNonexistent
#[test]
fn get_node_returns_null_for_nonexistent() {
    assert!(Diagram::create_new("x").node("nonexistent-id").is_none());
}

// C#: GetEdgesForNode_ReturnsConnectedEdges
#[test]
fn get_edges_for_node_returns_connected_edges() {
    let mut d = Diagram::create_new("x");
    let (n1, n2, n3) = (named("Node 1"), named("Node 2"), named("Node 3"));
    let (e1, e2, e3) = (link(&n1, &n2), link(&n2, &n3), link(&n1, &n3));
    let (id1, edge1_id, edge3_id) = (n1.id.clone(), e1.id.clone(), e3.id.clone());
    for n in [n1, n2, n3] {
        d.add_node(n);
    }
    for e in [e1, e2, e3] {
        d.add_edge(e);
    }
    let ids: Vec<&str> = d.edges_for_node(&id1).map(|e| e.id.as_str()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&edge1_id.as_str()));
    assert!(ids.contains(&edge3_id.as_str()));
}

// C#: GetNextZIndex_ReturnsIncrementingValue
#[test]
fn get_next_z_index_returns_incrementing_value() {
    let mut d = Diagram::create_new("x");
    d.add_node(DiagramNode {
        z_index: 5,
        ..DiagramNode::default()
    });
    d.add_node(DiagramNode {
        z_index: 3,
        ..DiagramNode::default()
    });
    assert_eq!(d.next_z_index(), 6);
}

// C#: AddNode_UpdatesModifiedAt
#[test]
fn add_node_updates_modified_at() {
    let mut d = Diagram::create_new("x");
    let before = d.modified_at;
    d.add_node(DiagramNode::default());
    assert!(d.modified_at > before);
}

#[test]
fn anchor_and_contains() {
    let n = DiagramNode {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
        ..DiagramNode::default()
    };
    assert_eq!(n.anchor(ConnectionPoint::Bottom), Point::new(60.0, 70.0));
    assert!(n.contains(Point::new(50.0, 40.0)));
    assert!(!n.contains(Point::new(5.0, 40.0)));
}
