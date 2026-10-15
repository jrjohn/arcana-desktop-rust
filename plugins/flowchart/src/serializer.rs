//! JSON / draw.io persistence (C# `Services/DiagramSerializer.cs`).

use std::io::{Read, Write};
use std::path::Path;

use arcana_core::{AppError, AppResult, ErrorCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chrono::{DateTime, Utc};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

use crate::model::{
    ArrowType, ConnectionPoint, Diagram, DiagramEdge, DiagramNode, LineStyle, NodeShape, Point,
    RoutingStyle,
};
use crate::xml::{self, Element};

/// File format (C# `DiagramSerializer.DiagramFormat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramFormat {
    ArcanaJson,
    DrawIo,
    Json,
}

/// Lower-cased extension including the dot, like `Path.GetExtension(...).ToLowerInvariant()`.
fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

/// C# `GetFormatFromExtension`.
#[must_use]
pub fn format_from_extension(path: impl AsRef<Path>) -> DiagramFormat {
    match extension(path.as_ref()).as_str() {
        ".drawio" | ".xml" => DiagramFormat::DrawIo,
        ".json" => DiagramFormat::Json,
        _ => DiagramFormat::ArcanaJson,
    }
}

/// C# `GetFileFilter` as (label, extensions) pairs for a native file dialog.
#[must_use]
pub fn file_filters() -> [(&'static str, &'static [&'static str]); 3] {
    [
        ("Arcana FlowChart", &["afc"]),
        ("Draw.io", &["drawio"]),
        ("JSON", &["json"]),
    ]
}

fn data_err(code: ErrorCode, msg: impl Into<String>) -> AppError {
    AppError::data(code, msg)
}

// ---------------- JSON ----------------

/// Indented camelCase JSON (C# `SerializeToJson`).
///
/// # Errors
/// Only if serde fails, which the model types never do.
pub fn to_json(diagram: &Diagram) -> AppResult<String> {
    serde_json::to_string_pretty(diagram)
        .map_err(|e| data_err(ErrorCode::InvalidFormat, e.to_string()))
}

/// C# `DeserializeFromJson` (which throws `JsonException` on bad input).
///
/// # Errors
/// `InvalidFormat` for malformed JSON.
pub fn from_json(json: &str) -> AppResult<Diagram> {
    serde_json::from_str(json).map_err(|e| data_err(ErrorCode::InvalidFormat, e.to_string()))
}

// ---------------- Files ----------------

/// C# `SaveToFileAsync`.
///
/// # Errors
/// I/O failures as `FileAccessDenied`.
pub fn save_to_file(
    diagram: &Diagram,
    path: impl AsRef<Path>,
    format: DiagramFormat,
) -> AppResult<()> {
    let content = match format {
        DiagramFormat::ArcanaJson | DiagramFormat::Json => to_json(diagram)?,
        DiagramFormat::DrawIo => to_drawio(diagram),
    };
    std::fs::write(path, content).map_err(|e| data_err(ErrorCode::FileAccessDenied, e.to_string()))
}

/// C# `LoadFromFileAsync`: picks the codec from the extension.
///
/// # Errors
/// `FileNotFound` when missing, `InvalidFormat`/`FileCorrupted` when unparsable.
pub fn load_from_file(path: impl AsRef<Path>) -> AppResult<Diagram> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(data_err(
            ErrorCode::FileNotFound,
            format!("Diagram file not found: {}", path.display()),
        ));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| data_err(ErrorCode::FileAccessDenied, e.to_string()))?;
    match extension(path).as_str() {
        ".drawio" | ".xml" => from_drawio(&content)
            .ok_or_else(|| data_err(ErrorCode::FileCorrupted, "Not a draw.io diagram")),
        _ => from_json(&content),
    }
}

// ---------------- draw.io write ----------------

fn shape_style(shape: NodeShape) -> &'static str {
    match shape {
        NodeShape::Diamond => "rhombus",
        NodeShape::Ellipse => "ellipse",
        NodeShape::RoundedRectangle => "rounded=1",
        NodeShape::Parallelogram => "shape=parallelogram;perimeter=parallelogramPerimeter",
        NodeShape::Hexagon => "shape=hexagon;perimeter=hexagonPerimeter2",
        NodeShape::Cylinder => {
            "shape=cylinder3;whiteSpace=wrap;boundedLbl=1;backgroundOutline=1;size=15"
        }
        NodeShape::Document => "shape=document;whiteSpace=wrap;boundedLbl=1;backgroundOutline=1",
        NodeShape::Cloud => "ellipse;shape=cloud",
        NodeShape::Triangle => "triangle;whiteSpace=wrap",
        _ => "",
    }
}

fn node_style(n: &DiagramNode) -> String {
    format!(
        "{};whiteSpace=wrap;html=1;fillColor={};strokeColor={};strokeWidth={};fontColor={};fontSize={}",
        shape_style(n.shape),
        n.fill_color,
        n.stroke_color,
        n.stroke_width,
        n.text_color,
        n.font_size
    )
}

fn arrow_name(a: ArrowType) -> &'static str {
    match a {
        ArrowType::Arrow => "classic",
        ArrowType::OpenArrow => "open",
        ArrowType::Diamond => "diamond",
        ArrowType::Circle => "oval",
        ArrowType::Square => "block",
        ArrowType::None => "none",
    }
}

fn edge_style(e: &DiagramEdge) -> String {
    let dash = match e.style {
        LineStyle::Dashed => "dashed=1;",
        LineStyle::Dotted => "dashed=1;dashPattern=1 2;",
        LineStyle::DashDot => "dashed=1;dashPattern=3 1 1 1;",
        LineStyle::Solid => "",
    };
    let routing = match e.routing {
        RoutingStyle::Orthogonal => "edgeStyle=orthogonalEdgeStyle;",
        RoutingStyle::Curved => "curved=1;",
        RoutingStyle::EntityRelation => "edgeStyle=entityRelationEdgeStyle;",
        RoutingStyle::Direct => "",
    };
    let (ex, ey) = e.source_point.relative();
    let (nx, ny) = e.target_point.relative();
    format!(
        "{routing}{dash}startArrow={};endArrow={};strokeColor={};strokeWidth={};html=1;exitX={ex};exitY={ey};entryX={nx};entryY={ny}",
        arrow_name(e.source_arrow),
        arrow_name(e.target_arrow),
        e.stroke_color,
        e.stroke_width
    )
}

fn graph_model(d: &Diagram) -> Element {
    let mut root = Element::new("root")
        .child(Element::new("mxCell").attr("id", "0"))
        .child(Element::new("mxCell").attr("id", "1").attr("parent", "0"));
    for n in &d.nodes {
        root = root.child(
            Element::new("mxCell")
                .attr("id", &n.id)
                .attr("value", &n.text)
                .attr("style", node_style(n))
                .attr("vertex", "1")
                .attr("parent", "1")
                .child(
                    Element::new("mxGeometry")
                        .attr("x", n.x)
                        .attr("y", n.y)
                        .attr("width", n.width)
                        .attr("height", n.height)
                        .attr("as", "geometry"),
                ),
        );
    }
    for e in &d.edges {
        let mut geo = Element::new("mxGeometry")
            .attr("relative", "1")
            .attr("as", "geometry");
        if !e.waypoints.is_empty() {
            let mut arr = Element::new("Array").attr("as", "points");
            for p in &e.waypoints {
                arr = arr.child(Element::new("mxPoint").attr("x", p.x).attr("y", p.y));
            }
            geo = geo.child(arr);
        }
        root = root.child(
            Element::new("mxCell")
                .attr("id", &e.id)
                .attr("value", &e.label)
                .attr("style", edge_style(e))
                .attr("edge", "1")
                .attr("parent", "1")
                .attr("source", &e.source_node_id)
                .attr("target", &e.target_node_id)
                .child(geo),
        );
    }
    Element::new("mxGraphModel")
        .attr("dx", "0")
        .attr("dy", "0")
        .attr("grid", if d.show_grid { "1" } else { "0" })
        .attr("gridSize", d.grid_size)
        .attr("guides", "1")
        .attr("tooltips", "1")
        .attr("connect", "1")
        .attr("arrows", "1")
        .attr("fold", "1")
        .attr("page", "1")
        .attr("pageScale", "1")
        .attr("pageWidth", d.canvas_width)
        .attr("pageHeight", d.canvas_height)
        .attr("background", &d.background_color)
        .child(root)
}

fn mxfile(d: &Diagram, diagram_el: Element) -> String {
    Element::new("mxfile")
        .attr("host", "Arcana.FlowChart")
        .attr("modified", d.modified_at.to_rfc3339())
        .attr("version", &d.version)
        .child(diagram_el)
        .to_xml()
}

/// Uncompressed draw.io file (C# `SerializeToDrawIO`).
#[must_use]
pub fn to_drawio(d: &Diagram) -> String {
    let el = Element::new("diagram")
        .attr("id", &d.id)
        .attr("name", &d.name)
        .child(graph_model(d));
    mxfile(d, el)
}

/// draw.io file with the model compressed into the `<diagram>` text, as draw.io itself saves.
/// (C# has `CompressDrawIOContent` but never calls it; exposed here so the format round-trips.)
///
/// # Errors
/// Only if the in-memory deflate stream fails.
pub fn to_drawio_compressed(d: &Diagram) -> AppResult<String> {
    let mut el = Element::new("diagram")
        .attr("id", &d.id)
        .attr("name", &d.name);
    el.text = compress(&graph_model(d).to_xml())?;
    Ok(mxfile(d, el))
}

/// encodeURIComponent's unreserved set: letters, digits and `-_.!~*'()`.
const URI_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// URL-encode → raw deflate → base64 (C# `CompressDrawIOContent`).
///
/// # Errors
/// Only if the in-memory deflate stream fails.
pub fn compress(content: &str) -> AppResult<String> {
    let encoded = utf8_percent_encode(content, URI_COMPONENT).to_string();
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
    let bytes = enc
        .write_all(encoded.as_bytes())
        .and_then(|()| enc.finish())
        .map_err(|e| data_err(ErrorCode::FileCorrupted, e.to_string()))?;
    Ok(B64.encode(bytes))
}

/// base64 → inflate → URL-decode (C# `DecompressDrawIOContent`); `None` if not compressed.
#[must_use]
pub fn decompress(compressed: &str) -> Option<String> {
    let raw = B64.decode(compressed.trim()).ok()?;
    let mut out = String::new();
    DeflateDecoder::new(raw.as_slice())
        .read_to_string(&mut out)
        .ok()?;
    percent_decode_str(&out)
        .decode_utf8()
        .ok()
        .map(std::borrow::Cow::into_owned)
}

// ---------------- draw.io read ----------------

fn style_value<'a>(style: &'a str, key: &str) -> Option<&'a str> {
    style.split(';').find_map(|part| {
        let mut kv = part.split('=');
        match (kv.next(), kv.next(), kv.next()) {
            (Some(k), Some(v), None) if k == key => Some(v),
            _ => None,
        }
    })
}

fn num(v: Option<&str>) -> Option<f64> {
    v.and_then(|s| s.trim().parse().ok())
}

fn shape_from_style(style: &str) -> NodeShape {
    const TABLE: [(&str, NodeShape); 9] = [
        ("rhombus", NodeShape::Diamond),
        ("ellipse", NodeShape::Ellipse),
        ("rounded=1", NodeShape::RoundedRectangle),
        ("parallelogram", NodeShape::Parallelogram),
        ("hexagon", NodeShape::Hexagon),
        ("cylinder", NodeShape::Cylinder),
        ("document", NodeShape::Document),
        ("cloud", NodeShape::Cloud),
        ("triangle", NodeShape::Triangle),
    ];
    TABLE
        .iter()
        .find(|(k, _)| style.contains(k))
        .map_or(NodeShape::Rectangle, |(_, s)| *s)
}

fn arrow_from(v: Option<&str>) -> ArrowType {
    match v {
        Some("classic") => ArrowType::Arrow,
        Some("open") => ArrowType::OpenArrow,
        Some("diamond") => ArrowType::Diamond,
        Some("oval") => ArrowType::Circle,
        Some("block") => ArrowType::Square,
        _ => ArrowType::None,
    }
}

fn point_from(x: Option<&str>, y: Option<&str>) -> ConnectionPoint {
    let (Some(x), Some(y)) = (num(x), num(y)) else {
        return ConnectionPoint::Center;
    };
    [
        ConnectionPoint::Left,
        ConnectionPoint::Right,
        ConnectionPoint::Top,
        ConnectionPoint::Bottom,
        ConnectionPoint::TopLeft,
        ConnectionPoint::TopRight,
        ConnectionPoint::BottomLeft,
        ConnectionPoint::BottomRight,
    ]
    .into_iter()
    .find(|p| {
        let (px, py) = p.relative();
        (px - x).abs() < f64::EPSILON && (py - y).abs() < f64::EPSILON
    })
    .unwrap_or(ConnectionPoint::Center)
}

fn parse_node(cell: &Element) -> Option<DiagramNode> {
    let geo = cell.element("mxGeometry")?;
    let mut n = DiagramNode::default();
    if let Some(id) = cell.get("id") {
        n.id = id.into();
    }
    n.text = cell.get("value").unwrap_or_default().into();
    n.x = num(geo.get("x")).unwrap_or(n.x);
    n.y = num(geo.get("y")).unwrap_or(n.y);
    n.width = num(geo.get("width")).unwrap_or(n.width);
    n.height = num(geo.get("height")).unwrap_or(n.height);
    let style = cell.get("style").unwrap_or_default();
    n.shape = shape_from_style(style);
    n.fill_color = style_value(style, "fillColor").unwrap_or("#FFFFFF").into();
    n.stroke_color = style_value(style, "strokeColor")
        .unwrap_or("#000000")
        .into();
    Some(n)
}

fn parse_edge(cell: &Element) -> DiagramEdge {
    let mut e = DiagramEdge::default();
    if let Some(id) = cell.get("id") {
        e.id = id.into();
    }
    e.source_node_id = cell.get("source").unwrap_or_default().into();
    e.target_node_id = cell.get("target").unwrap_or_default().into();
    e.label = cell.get("value").unwrap_or_default().into();
    let style = cell.get("style").unwrap_or_default();
    e.stroke_color = style_value(style, "strokeColor")
        .unwrap_or("#000000")
        .into();
    if style.contains("dashed=1") {
        e.style = LineStyle::Dashed;
    }
    if style.contains("dashPattern=1 2") {
        e.style = LineStyle::Dotted;
    }
    if style.contains("dashPattern=3 1 1 1") {
        e.style = LineStyle::DashDot;
    }
    if style.contains("edgeStyle=orthogonalEdgeStyle") {
        e.routing = RoutingStyle::Orthogonal;
    } else if style.contains("curved=1") {
        e.routing = RoutingStyle::Curved;
    } else if style.contains("edgeStyle=entityRelationEdgeStyle") {
        e.routing = RoutingStyle::EntityRelation;
    }
    e.target_arrow = arrow_from(style_value(style, "endArrow"));
    e.source_arrow = arrow_from(style_value(style, "startArrow"));
    e.source_point = point_from(style_value(style, "exitX"), style_value(style, "exitY"));
    e.target_point = point_from(style_value(style, "entryX"), style_value(style, "entryY"));
    if let Some(arr) = cell
        .element("mxGeometry")
        .and_then(|g| g.element("Array"))
        .filter(|a| a.get("as") == Some("points"))
    {
        e.waypoints = arr
            .children
            .iter()
            .filter(|p| p.name == "mxPoint")
            .filter_map(|p| Some(Point::new(num(p.get("x"))?, num(p.get("y"))?)))
            .collect();
    }
    e
}

/// C# `DeserializeFromDrawIO`: `None` for anything that is not a parsable draw.io document.
/// Accepts a bare `<mxGraphModel>` too, and compressed `<diagram>` bodies.
#[must_use]
pub fn from_drawio(xml_text: &str) -> Option<Diagram> {
    let doc = xml::parse(xml_text)?;
    let mut d = Diagram::default();

    if doc.name == "mxfile"
        && let Some(m) = doc
            .get("modified")
            .and_then(|m| DateTime::parse_from_rfc3339(m).ok())
    {
        d.modified_at = m.with_timezone(&Utc);
    }

    let diagram_el = doc.find_self_or_descendant("diagram");
    if let Some(de) = diagram_el {
        d.id = de
            .get("id")
            .map_or_else(|| uuid::Uuid::new_v4().to_string(), Into::into);
        d.name = de.get("name").unwrap_or("Imported Diagram").into();
    }

    // Compressed body: <diagram> has text instead of an <mxGraphModel> child.
    let inflated = diagram_el
        .filter(|de| de.children.is_empty() && !de.text.trim().is_empty())
        .and_then(|de| decompress(&de.text))
        .and_then(|s| xml::parse(&s));
    let model_owner = inflated.as_ref().unwrap_or(&doc);
    let model = model_owner.find_self_or_descendant("mxGraphModel");

    if let Some(g) = model {
        d.show_grid = g.get("grid") == Some("1");
        d.grid_size = num(g.get("gridSize")).unwrap_or(d.grid_size);
        d.canvas_width = num(g.get("pageWidth")).unwrap_or(d.canvas_width);
        d.canvas_height = num(g.get("pageHeight")).unwrap_or(d.canvas_height);
        d.background_color = g.get("background").unwrap_or("#FFFFFF").into();
    }

    let cells = model.map(|m| m.descendants("mxCell")).unwrap_or_default();
    let mut edge_cells = Vec::new();
    for cell in cells {
        if matches!(cell.get("id"), Some("0" | "1")) {
            continue;
        }
        if cell.get("vertex") == Some("1") {
            if let Some(n) = parse_node(cell) {
                d.nodes.push(n);
            }
        } else if cell.get("edge") == Some("1") {
            edge_cells.push(cell);
        }
    }
    d.edges = edge_cells.into_iter().map(parse_edge).collect();
    Some(d)
}

#[cfg(test)]
#[path = "serializer_tests.rs"]
mod tests;
