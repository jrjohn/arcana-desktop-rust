//! Flow chart editor plugin (C# `plugins/FlowChartModule`).
//!
//! - [`model`]: `Diagram` / `DiagramNode` (`NodeShape`) / `DiagramEdge` (`ArrowType`, …)
//! - [`serializer`]: Arcana JSON (`.afc`) and draw.io `mxGraphModel` XML, incl. compressed bodies
//! - [`vm`]: MVVM-UDF editor view model (undo/redo, selection, move, connect, snap, zoom/pan)
//! - [`geometry`]: routing and hit-testing, egui-free
//! - [`ui`]: egui Painter canvas; hosts call [`ui::show_editor`]

pub mod geometry;
pub mod model;
pub mod serializer;
pub mod ui;
pub mod vm;
mod xml;

pub use model::{
    ArrowType, ConnectionPoint, Diagram, DiagramEdge, DiagramNode, LineStyle, NodeShape, Point,
    RoutingStyle,
};
pub use vm::{EditorState, EditorViewModel, Effect, Input};
