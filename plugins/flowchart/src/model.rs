//! Diagram model (C# `Models/Diagram.cs`, `DiagramNode.cs`, `DiagramEdge.cs`).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Node shape (C# `NodeShape`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeShape {
    #[default]
    Rectangle,
    RoundedRectangle,
    Diamond,
    Ellipse,
    Parallelogram,
    Hexagon,
    Cylinder,
    Document,
    Cloud,
    Triangle,
    Pentagon,
    Star,
    Arrow,
    Callout,
}

impl NodeShape {
    /// Every shape in declaration order (C# `Enum.GetValues<NodeShape>()`).
    pub const ALL: [Self; 14] = [
        Self::Rectangle,
        Self::RoundedRectangle,
        Self::Diamond,
        Self::Ellipse,
        Self::Parallelogram,
        Self::Hexagon,
        Self::Cylinder,
        Self::Document,
        Self::Cloud,
        Self::Triangle,
        Self::Pentagon,
        Self::Star,
        Self::Arrow,
        Self::Callout,
    ];

    /// Default label for a freshly added node (C# `GetDefaultTextForShape`).
    #[must_use]
    pub fn default_text(self) -> &'static str {
        match self {
            Self::Ellipse => "Start/End",
            Self::Diamond => "Decision",
            Self::Parallelogram => "Input/Output",
            Self::Document => "Document",
            Self::Cylinder => "Database",
            Self::Cloud => "Cloud",
            _ => "Process",
        }
    }
}

/// Anchor on a node's bounding box (C# `ConnectionPoint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionPoint {
    Top,
    #[default]
    Right,
    Bottom,
    Left,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl ConnectionPoint {
    /// Relative (x, y) in the unit box; also draw.io's `exitX/exitY` (C# `GetExitX/GetExitY`).
    #[must_use]
    pub fn relative(self) -> (f64, f64) {
        match self {
            Self::Left => (0.0, 0.5),
            Self::Right => (1.0, 0.5),
            Self::Top => (0.5, 0.0),
            Self::Bottom => (0.5, 1.0),
            Self::TopLeft => (0.0, 0.0),
            Self::TopRight => (1.0, 0.0),
            Self::BottomLeft => (0.0, 1.0),
            Self::BottomRight => (1.0, 1.0),
            Self::Center => (0.5, 0.5),
        }
    }

    /// The four side anchors the canvas offers for connecting (C# canvas `ConnectionPoints`).
    pub const SIDES: [Self; 4] = [Self::Top, Self::Right, Self::Bottom, Self::Left];
}

/// Edge line style (C# `LineStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    DashDot,
}

/// Arrow head (C# `ArrowType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArrowType {
    #[default]
    None,
    Arrow,
    OpenArrow,
    Diamond,
    Circle,
    Square,
}

/// Edge routing (C# `RoutingStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoutingStyle {
    Direct,
    #[default]
    Orthogonal,
    Curved,
    EntityRelation,
}

/// 2-D point in diagram coordinates (C# `Point`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    #[must_use]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A shape on the canvas (C# `DiagramNode`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagramNode {
    pub id: String,
    pub shape: NodeShape,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: String,
    pub fill_color: String,
    pub stroke_color: String,
    pub stroke_width: f64,
    pub text_color: String,
    pub font_size: f64,
    pub z_index: i32,
    /// UI-only (C# `[JsonIgnore]`).
    #[serde(skip)]
    pub is_selected: bool,
    pub metadata: BTreeMap<String, String>,
}

impl Default for DiagramNode {
    fn default() -> Self {
        Self {
            id: new_id(),
            shape: NodeShape::Rectangle,
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 60.0,
            text: String::new(),
            fill_color: "#FFFFFF".into(),
            stroke_color: "#000000".into(),
            stroke_width: 2.0,
            text_color: "#000000".into(),
            font_size: 14.0,
            z_index: 0,
            is_selected: false,
            metadata: BTreeMap::new(),
        }
    }
}

impl DiagramNode {
    /// Copy with a new id, offset by (20, 20), not selected (C# `Clone`).
    #[must_use]
    pub fn duplicate(&self) -> Self {
        Self {
            id: new_id(),
            x: self.x + 20.0,
            y: self.y + 20.0,
            is_selected: false,
            ..self.clone()
        }
    }

    /// Whether a diagram-space point lies inside the bounding box.
    #[must_use]
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.x + self.width && p.y >= self.y && p.y <= self.y + self.height
    }

    /// Absolute position of an anchor (C# canvas `GetConnectionPoint`).
    #[must_use]
    pub fn anchor(&self, point: ConnectionPoint) -> Point {
        let (rx, ry) = point.relative();
        Point::new(self.x + self.width * rx, self.y + self.height * ry)
    }
}

/// A connection between two nodes (C# `DiagramEdge`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagramEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub source_point: ConnectionPoint,
    pub target_point: ConnectionPoint,
    pub style: LineStyle,
    pub source_arrow: ArrowType,
    pub target_arrow: ArrowType,
    pub routing: RoutingStyle,
    pub stroke_color: String,
    pub stroke_width: f64,
    pub label: String,
    pub label_position: f64,
    pub waypoints: Vec<Point>,
    pub z_index: i32,
    #[serde(skip)]
    pub is_selected: bool,
    pub metadata: BTreeMap<String, String>,
}

impl Default for DiagramEdge {
    fn default() -> Self {
        Self {
            id: new_id(),
            source_node_id: String::new(),
            target_node_id: String::new(),
            source_point: ConnectionPoint::Right,
            target_point: ConnectionPoint::Left,
            style: LineStyle::Solid,
            source_arrow: ArrowType::None,
            target_arrow: ArrowType::Arrow,
            routing: RoutingStyle::Orthogonal,
            stroke_color: "#000000".into(),
            stroke_width: 2.0,
            label: String::new(),
            label_position: 0.5,
            waypoints: Vec::new(),
            z_index: 0,
            is_selected: false,
            metadata: BTreeMap::new(),
        }
    }
}

impl DiagramEdge {
    /// Edge between two nodes with given anchors, other fields default.
    #[must_use]
    pub fn between(
        source: &str,
        target: &str,
        source_point: ConnectionPoint,
        target_point: ConnectionPoint,
    ) -> Self {
        Self {
            source_node_id: source.into(),
            target_node_id: target.into(),
            source_point,
            target_point,
            ..Self::default()
        }
    }
}

/// A whole diagram (C# `Diagram`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Diagram {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub author: String,
    pub canvas_width: f64,
    pub canvas_height: f64,
    pub background_color: String,
    pub grid_size: f64,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for Diagram {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: new_id(),
            name: "Untitled Diagram".into(),
            description: String::new(),
            version: "1.0".into(),
            created_at: now,
            modified_at: now,
            author: String::new(),
            canvas_width: 2000.0,
            canvas_height: 2000.0,
            background_color: "#FFFFFF".into(),
            grid_size: 20.0,
            show_grid: true,
            snap_to_grid: true,
            nodes: Vec::new(),
            edges: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

impl Diagram {
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&DiagramNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut DiagramNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    #[must_use]
    pub fn edge(&self, id: &str) -> Option<&DiagramEdge> {
        self.edges.iter().find(|e| e.id == id)
    }

    /// Edges touching a node (C# `GetEdgesForNode`).
    pub fn edges_for_node<'a>(&'a self, node_id: &'a str) -> impl Iterator<Item = &'a DiagramEdge> {
        self.edges
            .iter()
            .filter(move |e| e.source_node_id == node_id || e.target_node_id == node_id)
    }

    /// Bumps `modified_at`, guaranteeing it strictly increases even within one clock tick.
    pub fn touch(&mut self) {
        let now = Utc::now();
        self.modified_at = if now > self.modified_at {
            now
        } else {
            self.modified_at + chrono::Duration::microseconds(1)
        };
    }

    pub fn add_node(&mut self, node: DiagramNode) {
        self.nodes.push(node);
        self.touch();
    }

    /// Removes a node and every edge attached to it.
    pub fn remove_node(&mut self, node_id: &str) {
        self.nodes.retain(|n| n.id != node_id);
        self.edges
            .retain(|e| e.source_node_id != node_id && e.target_node_id != node_id);
        self.touch();
    }

    pub fn add_edge(&mut self, edge: DiagramEdge) {
        self.edges.push(edge);
        self.touch();
    }

    pub fn remove_edge(&mut self, edge_id: &str) {
        self.edges.retain(|e| e.id != edge_id);
        self.touch();
    }

    /// One above the highest node/edge z-index (C# `GetNextZIndex`).
    #[must_use]
    pub fn next_z_index(&self) -> i32 {
        let n = self.nodes.iter().map(|n| n.z_index).max().unwrap_or(0);
        let e = self.edges.iter().map(|e| e.z_index).max().unwrap_or(0);
        n.max(e) + 1
    }

    /// Empty diagram with a name (C# `CreateNew`).
    #[must_use]
    pub fn create_new(name: &str) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Six-node sample flowchart (C# `CreateSample`).
    #[must_use]
    pub fn create_sample() -> Self {
        use ConnectionPoint::{Bottom, Left, Right, Top};
        let mut d = Self {
            name: "Sample Flowchart".into(),
            description: "A sample flowchart demonstrating various shapes and connections".into(),
            ..Self::default()
        };
        let mk = |shape, x, y, w, h, text: &str, fill: &str, z| DiagramNode {
            shape,
            x,
            y,
            width: w,
            height: h,
            text: text.into(),
            fill_color: fill.into(),
            z_index: z,
            ..DiagramNode::default()
        };
        let start = mk(
            NodeShape::Ellipse,
            400.0,
            50.0,
            100.0,
            50.0,
            "Start",
            "#90EE90",
            1,
        );
        let p1 = mk(
            NodeShape::Rectangle,
            375.0,
            150.0,
            150.0,
            60.0,
            "Process Step 1",
            "#87CEEB",
            2,
        );
        let dec = mk(
            NodeShape::Diamond,
            375.0,
            260.0,
            150.0,
            100.0,
            "Decision?",
            "#FFD700",
            3,
        );
        let p2 = mk(
            NodeShape::Rectangle,
            200.0,
            410.0,
            150.0,
            60.0,
            "Process Step 2",
            "#87CEEB",
            4,
        );
        let p3 = mk(
            NodeShape::Rectangle,
            550.0,
            410.0,
            150.0,
            60.0,
            "Process Step 3",
            "#87CEEB",
            5,
        );
        let end = mk(
            NodeShape::Ellipse,
            400.0,
            520.0,
            100.0,
            50.0,
            "End",
            "#FFB6C1",
            6,
        );
        let edges = [
            (&start.id, &p1.id, Bottom, Top, ""),
            (&p1.id, &dec.id, Bottom, Top, ""),
            (&dec.id, &p2.id, Left, Top, "Yes"),
            (&dec.id, &p3.id, Right, Top, "No"),
            (&p2.id, &end.id, Bottom, Left, ""),
            (&p3.id, &end.id, Bottom, Right, ""),
        ]
        .map(|(s, t, sp, tp, label)| DiagramEdge {
            label: label.into(),
            ..DiagramEdge::between(s, t, sp, tp)
        });
        for n in [start, p1, dec, p2, p3, end] {
            d.add_node(n);
        }
        for e in edges {
            d.add_edge(e);
        }
        d
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
