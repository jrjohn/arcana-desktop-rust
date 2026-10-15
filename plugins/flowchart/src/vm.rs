//! Editor view model (C# `ViewModels/FlowChartEditorViewModel.cs`) as MVVM-UDF:
//! `on(Input) -> Vec<Effect>`. No I/O and no egui here; the host performs file effects
//! and feeds results back as inputs.

use std::path::{Path, PathBuf};

use crate::model::{ConnectionPoint, Diagram, DiagramEdge, DiagramNode, NodeShape, Point};
use crate::serializer::{DiagramFormat, format_from_extension};

/// Undo depth (C# trims its stack to 50).
pub const UNDO_LIMIT: usize = 50;
pub const ZOOM_MIN: f64 = 0.1;
pub const ZOOM_MAX: f64 = 3.0;
pub const ZOOM_STEP: f64 = 0.1;
/// C# `UpdateNodeSize` minimums.
pub const MIN_NODE_WIDTH: f64 = 40.0;
pub const MIN_NODE_HEIGHT: f64 = 30.0;
/// Grid size range offered by the properties panel.
pub const GRID_MIN: f64 = 5.0;
pub const GRID_MAX: f64 = 100.0;

/// Everything the user can ask the editor to do (C# nested `Input` class).
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    NewDiagram,
    OpenDiagram,
    SaveDiagram,
    SaveDiagramAs,
    /// The host finished reading a file.
    FileLoaded {
        path: PathBuf,
        diagram: Box<Diagram>,
    },
    /// The host finished writing a file.
    FileSaved {
        path: PathBuf,
    },
    /// A host file operation failed.
    FileFailed(String),
    Undo,
    Redo,
    DeleteSelected,
    DuplicateSelected,
    AddNode(Option<NodeShape>),
    /// Add a node with its top-left at a diagram position (palette drop / double-click).
    AddNodeAt {
        shape: NodeShape,
        x: f64,
        y: f64,
    },
    BringToFront,
    SendToBack,
    /// Start of a drag gesture: snapshot for undo once per gesture.
    BeginMove,
    UpdateNodePosition {
        id: String,
        x: f64,
        y: f64,
    },
    UpdateNodeSize {
        id: String,
        width: f64,
        height: f64,
    },
    SetNodeText {
        id: String,
        text: String,
    },
    AddConnection {
        source: String,
        target: String,
        source_point: ConnectionPoint,
        target_point: ConnectionPoint,
    },
    SelectNode(Option<String>),
    SelectEdge(Option<String>),
    ClearSelection,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    SetZoom(f64),
    /// Pan by a screen-space delta.
    Pan {
        dx: f64,
        dy: f64,
    },
    ToggleConnectMode,
    TogglePanMode,
    SetConnectMode(bool),
    SetPanMode(bool),
    ToggleSnapToGrid,
    SetSelectedShape(NodeShape),
    CreateSampleDiagram,
    /// Diagram properties (shown when nothing is selected).
    SetDiagramName(String),
    SetDiagramDescription(String),
    SetShowGrid(bool),
    SetSnapToGrid(bool),
    SetGridSize(f64),
    /// Properties-panel edit of a node's bounds (one undo step).
    SetNodeBounds {
        id: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    /// Resize-handle drag (no undo; the gesture starts with `BeginMove`).
    ResizeNode {
        id: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    /// Insert a bend point at `index` of the edge's waypoints. An edge drawn by automatic
    /// routing first gets its current bends as explicit waypoints. One undo step.
    InsertWaypoint {
        edge: String,
        index: usize,
        x: f64,
        y: f64,
    },
    /// Drag a bend point (no undo; the gesture starts with `BeginMove`/`InsertWaypoint`).
    MoveWaypoint {
        edge: String,
        index: usize,
        x: f64,
        y: f64,
    },
    RemoveWaypoint {
        edge: String,
        index: usize,
    },
}

/// Side effects for the host (C# nested `Effect` class; file effects carry the payload).
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    RequestFileOpen,
    RequestFileSaveAs,
    Save {
        path: PathBuf,
        format: DiagramFormat,
        diagram: Box<Diagram>,
    },
    ShowError(String),
    ShowSuccess(String),
    DiagramChanged,
}

/// Observable state (C# nested `Output` class).
#[derive(Debug, Clone, PartialEq)]
pub struct EditorState {
    pub diagram: Diagram,
    pub selected_node: Option<String>,
    pub selected_edge: Option<String>,
    pub selected_shape: NodeShape,
    pub zoom: f64,
    /// Screen-space pan offset in points.
    pub pan: (f64, f64),
    pub connect_mode: bool,
    pub pan_mode: bool,
    pub status: String,
    pub current_file: Option<PathBuf>,
    pub modified: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            diagram: Diagram::create_new("Untitled Diagram"),
            selected_node: None,
            selected_edge: None,
            selected_shape: NodeShape::Rectangle,
            zoom: 1.0,
            pan: (0.0, 0.0),
            connect_mode: false,
            pan_mode: false,
            status: "Ready".into(),
            current_file: None,
            modified: false,
        }
    }
}

impl EditorState {
    /// C# `UpdateTitle`: `"{file}{ *} - FlowChart Editor"`.
    #[must_use]
    pub fn title(&self) -> String {
        let name = self
            .current_file
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(
                || "Untitled".to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
        let star = if self.modified { " *" } else { "" };
        format!("{name}{star} - FlowChart Editor")
    }

    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selected_node.is_some() || self.selected_edge.is_some()
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The editor view model.
#[derive(Debug, Clone, Default)]
pub struct EditorViewModel {
    pub state: EditorState,
    undo: Vec<Diagram>,
    redo: Vec<Diagram>,
}

impl EditorViewModel {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Editor opened on an existing diagram.
    #[must_use]
    pub fn with_diagram(diagram: Diagram) -> Self {
        let mut vm = Self::default();
        vm.state.diagram = diagram;
        vm
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn save_undo(&mut self) {
        self.undo.push(self.state.diagram.clone());
        self.redo.clear();
        if self.undo.len() > UNDO_LIMIT {
            let excess = self.undo.len() - UNDO_LIMIT;
            self.undo.drain(..excess);
        }
    }

    fn sync_selection_flags(&mut self) {
        let (sn, se) = (
            self.state.selected_node.clone(),
            self.state.selected_edge.clone(),
        );
        for n in &mut self.state.diagram.nodes {
            n.is_selected = sn.as_deref() == Some(n.id.as_str());
        }
        for e in &mut self.state.diagram.edges {
            e.is_selected = se.as_deref() == Some(e.id.as_str());
        }
    }

    fn select_node(&mut self, id: Option<String>) {
        self.state.selected_edge = None;
        self.state.selected_node = id.filter(|i| self.state.diagram.node(i).is_some());
        self.sync_selection_flags();
    }

    fn select_edge(&mut self, id: Option<String>) {
        self.state.selected_node = None;
        self.state.selected_edge = id.filter(|i| self.state.diagram.edge(i).is_some());
        self.sync_selection_flags();
    }

    fn clear_selection(&mut self) {
        self.state.selected_node = None;
        self.state.selected_edge = None;
        self.sync_selection_flags();
    }

    fn replace_diagram(&mut self, d: Diagram, status: &str) {
        self.state.diagram = d;
        self.clear_selection();
        self.state.status = status.into();
    }

    fn changed(&mut self, status: impl Into<String>) -> Vec<Effect> {
        self.state.modified = true;
        self.state.status = status.into();
        vec![Effect::DiagramChanged]
    }

    /// Handles one input. Pure state transition; returned effects are for the host.
    #[allow(clippy::too_many_lines, clippy::many_single_char_names)] // one flat match over the input enum
    pub fn on(&mut self, input: Input) -> Vec<Effect> {
        let s = &mut self.state;
        match input {
            Input::NewDiagram => {
                self.save_undo();
                self.replace_diagram(
                    Diagram::create_new("Untitled Diagram"),
                    "New diagram created",
                );
                self.state.current_file = None;
                self.state.modified = false;
                vec![Effect::DiagramChanged]
            }
            Input::OpenDiagram => {
                s.status = "Opening diagram...".into();
                vec![Effect::RequestFileOpen]
            }
            Input::SaveDiagram => match s.current_file.clone() {
                None => vec![Effect::RequestFileSaveAs],
                Some(path) => vec![Effect::Save {
                    format: format_from_extension(&path),
                    path,
                    diagram: Box::new(s.diagram.clone()),
                }],
            },
            Input::SaveDiagramAs => vec![Effect::RequestFileSaveAs],
            Input::FileLoaded { path, diagram } => {
                self.save_undo();
                let status = format!("Loaded {}", file_name(&path));
                self.replace_diagram(*diagram, &status);
                self.state.current_file = Some(path);
                self.state.modified = false;
                vec![Effect::DiagramChanged]
            }
            Input::FileSaved { path } => {
                let msg = format!("Saved to {}", file_name(&path));
                s.current_file = Some(path);
                s.modified = false;
                s.status.clone_from(&msg);
                vec![Effect::ShowSuccess(msg)]
            }
            Input::FileFailed(msg) => {
                s.status = format!("Error: {msg}");
                vec![Effect::ShowError(msg)]
            }
            Input::Undo => {
                let Some(prev) = self.undo.pop() else {
                    return vec![];
                };
                self.redo
                    .push(std::mem::replace(&mut self.state.diagram, prev));
                self.clear_selection();
                self.state.status = "Undo".into();
                vec![Effect::DiagramChanged]
            }
            Input::Redo => {
                let Some(next) = self.redo.pop() else {
                    return vec![];
                };
                self.undo
                    .push(std::mem::replace(&mut self.state.diagram, next));
                self.clear_selection();
                self.state.status = "Redo".into();
                vec![Effect::DiagramChanged]
            }
            Input::DeleteSelected => {
                // C# snapshots and marks modified even when nothing is selected; keep that.
                self.save_undo();
                let s = &mut self.state;
                if let Some(id) = s.selected_node.take() {
                    s.diagram.remove_node(&id);
                    s.status = "Node deleted".into();
                } else if let Some(id) = s.selected_edge.take() {
                    s.diagram.remove_edge(&id);
                    s.status = "Connection deleted".into();
                }
                s.modified = true;
                vec![Effect::DiagramChanged]
            }
            Input::DuplicateSelected => {
                let Some(src) = s.selected_node.as_deref().and_then(|id| s.diagram.node(id)) else {
                    return vec![];
                };
                let mut clone = src.duplicate();
                clone.z_index = s.diagram.next_z_index();
                let id = clone.id.clone();
                self.save_undo();
                self.state.diagram.add_node(clone);
                self.select_node(Some(id));
                self.changed("Node duplicated")
            }
            Input::AddNode(shape) => {
                let shape = shape.unwrap_or(s.selected_shape);
                #[allow(clippy::cast_precision_loss)] // node counts are tiny
                let k = s.diagram.nodes.len() as f64 * 20.0;
                self.add_node_at(shape, 100.0 + k % 400.0, 100.0 + k % 300.0)
            }
            Input::AddNodeAt { shape, x, y } => {
                let (x, y) = self.snap(x, y);
                self.add_node_at(shape, x, y)
            }
            Input::BringToFront => {
                let Some(id) = s.selected_node.clone() else {
                    return vec![];
                };
                self.save_undo();
                let z = self.state.diagram.next_z_index();
                if let Some(n) = self.state.diagram.node_mut(&id) {
                    n.z_index = z;
                }
                self.changed("Brought to front")
            }
            Input::SendToBack => {
                let Some(id) = s.selected_node.clone() else {
                    return vec![];
                };
                self.save_undo();
                let min = self
                    .state
                    .diagram
                    .nodes
                    .iter()
                    .map(|n| n.z_index)
                    .min()
                    .unwrap_or(0);
                if let Some(n) = self.state.diagram.node_mut(&id) {
                    n.z_index = min - 1;
                }
                self.changed("Sent to back")
            }
            Input::BeginMove => {
                self.save_undo();
                vec![]
            }
            Input::UpdateNodePosition { id, x, y } => {
                let (x, y) = self.snap(x, y);
                let d = &mut self.state.diagram;
                let Some(n) = d.node_mut(&id) else {
                    return vec![];
                };
                n.x = x;
                n.y = y;
                d.touch();
                self.state.modified = true;
                vec![]
            }
            Input::UpdateNodeSize { id, width, height } => {
                let d = &mut self.state.diagram;
                let Some(n) = d.node_mut(&id) else {
                    return vec![];
                };
                n.width = width.max(MIN_NODE_WIDTH);
                n.height = height.max(MIN_NODE_HEIGHT);
                d.touch();
                self.state.modified = true;
                vec![]
            }
            Input::SetNodeText { id, text } => {
                if s.diagram.node(&id).is_none_or(|n| n.text == text) {
                    return vec![];
                }
                self.save_undo();
                if let Some(n) = self.state.diagram.node_mut(&id) {
                    n.text = text;
                }
                self.state.diagram.touch();
                self.changed("Text changed")
            }
            Input::AddConnection {
                source,
                target,
                source_point,
                target_point,
            } => {
                if source == target
                    || s.diagram.node(&source).is_none()
                    || s.diagram.node(&target).is_none()
                {
                    return vec![];
                }
                self.save_undo();
                let edge = DiagramEdge {
                    z_index: self.state.diagram.next_z_index(),
                    ..DiagramEdge::between(&source, &target, source_point, target_point)
                };
                let id = edge.id.clone();
                self.state.diagram.add_edge(edge);
                self.select_edge(Some(id));
                self.changed("Connection added")
            }
            Input::SelectNode(id) => {
                self.select_node(id);
                vec![]
            }
            Input::SelectEdge(id) => {
                self.select_edge(id);
                vec![]
            }
            Input::ClearSelection => {
                self.clear_selection();
                vec![]
            }
            Input::ZoomIn => {
                s.zoom = (s.zoom + ZOOM_STEP).min(ZOOM_MAX);
                vec![]
            }
            Input::ZoomOut => {
                s.zoom = (s.zoom - ZOOM_STEP).max(ZOOM_MIN);
                vec![]
            }
            Input::ZoomReset => {
                s.zoom = 1.0;
                s.pan = (0.0, 0.0);
                vec![]
            }
            Input::SetZoom(z) => {
                s.zoom = z.clamp(ZOOM_MIN, ZOOM_MAX);
                vec![]
            }
            Input::Pan { dx, dy } => {
                s.pan = (s.pan.0 + dx, s.pan.1 + dy);
                vec![]
            }
            Input::ToggleConnectMode => {
                s.connect_mode = !s.connect_mode;
                vec![]
            }
            Input::TogglePanMode => {
                s.pan_mode = !s.pan_mode;
                vec![]
            }
            Input::SetConnectMode(b) => {
                s.connect_mode = b;
                vec![]
            }
            Input::SetPanMode(b) => {
                s.pan_mode = b;
                vec![]
            }
            Input::ToggleSnapToGrid => {
                s.diagram.snap_to_grid = !s.diagram.snap_to_grid;
                vec![]
            }
            Input::SetSelectedShape(shape) => {
                s.selected_shape = shape;
                vec![]
            }
            Input::CreateSampleDiagram => {
                self.save_undo();
                self.replace_diagram(Diagram::create_sample(), "Sample diagram created");
                self.state.current_file = None;
                self.state.modified = true;
                vec![Effect::DiagramChanged]
            }
            Input::SetDiagramName(name) => {
                if s.diagram.name == name {
                    return vec![];
                }
                self.save_undo();
                self.state.diagram.name = name;
                self.state.diagram.touch();
                self.changed("Diagram renamed")
            }
            Input::SetDiagramDescription(text) => {
                if s.diagram.description == text {
                    return vec![];
                }
                self.save_undo();
                self.state.diagram.description = text;
                self.state.diagram.touch();
                self.changed("Description changed")
            }
            Input::SetShowGrid(b) => {
                s.diagram.show_grid = b;
                vec![]
            }
            Input::SetSnapToGrid(b) => {
                s.diagram.snap_to_grid = b;
                vec![]
            }
            Input::SetGridSize(g) => {
                s.diagram.grid_size = g.clamp(GRID_MIN, GRID_MAX);
                vec![]
            }
            Input::SetNodeBounds {
                id,
                x,
                y,
                width,
                height,
            } => {
                if s.diagram.node(&id).is_none() {
                    return vec![];
                }
                self.save_undo();
                self.set_bounds(&id, x, y, width, height);
                self.changed("Node resized")
            }
            Input::ResizeNode {
                id,
                x,
                y,
                width,
                height,
            } => {
                let (x, y) = self.snap(x, y);
                self.set_bounds(&id, x, y, width, height);
                vec![]
            }
            Input::InsertWaypoint { edge, index, x, y } => {
                let Some(e) = s.diagram.edges.iter().find(|e| e.id == edge) else {
                    return vec![];
                };
                let route = crate::geometry::edge_path(&s.diagram, e).unwrap_or_default();
                let materialize = e.waypoints.is_empty() && route.len() > 2;
                self.save_undo();
                let (x, y) = self.snap(x, y);
                let Some(e) = self.state.diagram.edges.iter_mut().find(|e| e.id == edge) else {
                    return vec![];
                };
                if materialize {
                    e.waypoints = route[1..route.len() - 1].to_vec();
                    e.waypoints.dedup();
                }
                let at = index.min(e.waypoints.len());
                e.waypoints.insert(at, Point::new(x, y));
                self.state.diagram.touch();
                self.changed("Bend point added")
            }
            Input::MoveWaypoint { edge, index, x, y } => {
                let (x, y) = self.snap(x, y);
                let d = &mut self.state.diagram;
                let Some(w) = d
                    .edges
                    .iter_mut()
                    .find(|e| e.id == edge)
                    .and_then(|e| e.waypoints.get_mut(index))
                else {
                    return vec![];
                };
                *w = Point::new(x, y);
                d.touch();
                self.state.modified = true;
                vec![]
            }
            Input::RemoveWaypoint { edge, index } => {
                let ok = s
                    .diagram
                    .edges
                    .iter()
                    .any(|e| e.id == edge && index < e.waypoints.len());
                if !ok {
                    return vec![];
                }
                self.save_undo();
                if let Some(e) = self.state.diagram.edges.iter_mut().find(|e| e.id == edge) {
                    e.waypoints.remove(index);
                }
                self.state.diagram.touch();
                self.changed("Bend point removed")
            }
        }
    }

    fn set_bounds(&mut self, id: &str, x: f64, y: f64, width: f64, height: f64) {
        let d = &mut self.state.diagram;
        if let Some(n) = d.node_mut(id) {
            n.x = x;
            n.y = y;
            n.width = width.max(MIN_NODE_WIDTH);
            n.height = height.max(MIN_NODE_HEIGHT);
            d.touch();
            self.state.modified = true;
        }
    }

    /// C# snap: `Math.Round(v / grid) * grid` when snapping is on.
    #[must_use]
    pub fn snap(&self, x: f64, y: f64) -> (f64, f64) {
        let d = &self.state.diagram;
        if d.snap_to_grid && d.grid_size > 0.0 {
            let g = d.grid_size;
            ((x / g).round() * g, (y / g).round() * g)
        } else {
            (x, y)
        }
    }

    fn add_node_at(&mut self, shape: NodeShape, x: f64, y: f64) -> Vec<Effect> {
        self.save_undo();
        let node = DiagramNode {
            shape,
            x,
            y,
            text: shape.default_text().into(),
            z_index: self.state.diagram.next_z_index(),
            ..DiagramNode::default()
        };
        let id = node.id.clone();
        self.state.diagram.add_node(node);
        self.select_node(Some(id));
        self.changed(format!("Added {shape:?} node"))
    }
}

#[cfg(test)]
#[path = "vm_tests.rs"]
mod tests;
