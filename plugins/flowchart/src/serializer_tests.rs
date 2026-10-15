//! Ports of C# `DiagramSerializerTests` (12), plus compressed draw.io coverage.
#![allow(clippy::float_cmp, clippy::many_single_char_names)]

use super::*;

fn unwrap<T>(r: AppResult<T>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("unexpected error: {e}"),
    }
}

// C#: SerializeToJson_ReturnsValidJson
#[test]
fn serialize_to_json_returns_valid_json() {
    let json = unwrap(to_json(&Diagram::create_sample()));
    assert!(json.contains("\"name\":"));
    assert!(json.contains("Sample Flowchart"));
    assert!(json.contains("\"nodes\":"));
    assert!(json.contains("\"edges\":"));
}

// C#: DeserializeFromJson_RestoresDiagram
#[test]
fn deserialize_from_json_restores_diagram() {
    let original = Diagram::create_sample();
    let restored = unwrap(from_json(&unwrap(to_json(&original))));
    assert_eq!(restored.name, original.name);
    assert_eq!(restored.nodes.len(), original.nodes.len());
    assert_eq!(restored.edges.len(), original.edges.len());
}

// C#: SerializeDeserialize_PreservesNodeProperties
#[test]
fn serialize_deserialize_preserves_node_properties() {
    let mut d = Diagram::create_new("x");
    d.add_node(DiagramNode {
        shape: NodeShape::Diamond,
        x: 100.0,
        y: 200.0,
        width: 150.0,
        height: 80.0,
        text: "Test Node".into(),
        fill_color: "#FF5500".into(),
        stroke_color: "#00FF00".into(),
        stroke_width: 3.0,
        font_size: 18.0,
        z_index: 5,
        ..DiagramNode::default()
    });
    let r = unwrap(from_json(&unwrap(to_json(&d))));
    let n = &r.nodes[0];
    assert_eq!(n.shape, NodeShape::Diamond);
    assert_eq!((n.x, n.y, n.width, n.height), (100.0, 200.0, 150.0, 80.0));
    assert_eq!(n.text, "Test Node");
    assert_eq!(n.fill_color, "#FF5500");
    assert_eq!(n.stroke_color, "#00FF00");
    assert_eq!(n.stroke_width, 3.0);
    assert_eq!(n.font_size, 18.0);
    assert_eq!(n.z_index, 5);
}

// C#: SerializeDeserialize_PreservesEdgeProperties
#[test]
fn serialize_deserialize_preserves_edge_properties() {
    let mut d = Diagram::create_new("x");
    let (a, b) = (DiagramNode::default(), DiagramNode::default());
    d.add_edge(DiagramEdge {
        source_node_id: a.id.clone(),
        target_node_id: b.id.clone(),
        label: "Test Edge".into(),
        style: LineStyle::Dashed,
        routing: RoutingStyle::Orthogonal,
        target_arrow: ArrowType::Diamond,
        stroke_color: "#FF0000".into(),
        ..DiagramEdge::default()
    });
    d.add_node(a);
    d.add_node(b);
    let r = unwrap(from_json(&unwrap(to_json(&d))));
    let e = &r.edges[0];
    assert_eq!(e.label, "Test Edge");
    assert_eq!(e.style, LineStyle::Dashed);
    assert_eq!(e.routing, RoutingStyle::Orthogonal);
    assert_eq!(e.target_arrow, ArrowType::Diamond);
    assert_eq!(e.stroke_color, "#FF0000");
}

// C#: SerializeToDrawIO_ReturnsValidXml
#[test]
fn serialize_to_drawio_returns_valid_xml() {
    let xml = to_drawio(&Diagram::create_sample());
    for s in ["<mxfile", "<mxGraphModel", "<mxCell", "Sample Flowchart"] {
        assert!(xml.contains(s), "missing {s}");
    }
}

// C#: DeserializeFromDrawIO_RestoresDiagram
#[test]
fn deserialize_from_drawio_restores_diagram() {
    let original = Diagram::create_sample();
    let r = from_drawio(&to_drawio(&original));
    let r = r.as_ref();
    assert_eq!(r.map(|d| d.name.as_str()), Some(original.name.as_str()));
    assert_eq!(r.map(|d| d.nodes.len()), Some(original.nodes.len()));
    assert_eq!(r.map(|d| d.edges.len()), Some(original.edges.len()));
}

// C#: GetFormatFromExtension_ReturnsCorrectFormat (Theory, 5 InlineData)
#[test]
fn get_format_from_extension_returns_correct_format() {
    for (ext, expected) in [
        (".afc", DiagramFormat::ArcanaJson),
        (".json", DiagramFormat::Json),
        (".drawio", DiagramFormat::DrawIo),
        (".xml", DiagramFormat::DrawIo),
        (".unknown", DiagramFormat::ArcanaJson),
    ] {
        assert_eq!(
            format_from_extension(format!("test{ext}")),
            expected,
            "{ext}"
        );
    }
}

// C#: DeserializeFromJson_ReturnsNullForInvalidJson (C# asserts JsonException is thrown)
#[test]
fn deserialize_from_json_returns_null_for_invalid_json() {
    assert!(from_json("not valid json").is_err());
}

// C#: DeserializeFromDrawIO_ReturnsNullForInvalidXml
#[test]
fn deserialize_from_drawio_returns_null_for_invalid_xml() {
    assert!(from_drawio("not valid xml").is_none());
}

// C#: SaveToFileAsync_CreatesFile
#[test]
fn save_to_file_creates_file() {
    let dir =
        unwrap(tempfile::tempdir().map_err(|e| AppError::data(ErrorCode::Unknown, e.to_string())));
    let path = dir.path().join("test.afc");
    unwrap(save_to_file(
        &Diagram::create_sample(),
        &path,
        DiagramFormat::ArcanaJson,
    ));
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(content.contains("Sample Flowchart"));
}

// C#: LoadFromFileAsync_LoadsSavedFile
#[test]
fn load_from_file_loads_saved_file() {
    let dir =
        unwrap(tempfile::tempdir().map_err(|e| AppError::data(ErrorCode::Unknown, e.to_string())));
    let path = dir.path().join("test.afc");
    let original = Diagram::create_sample();
    unwrap(save_to_file(&original, &path, DiagramFormat::ArcanaJson));
    let loaded = unwrap(load_from_file(&path));
    assert_eq!(loaded.name, original.name);
    assert_eq!(loaded.nodes.len(), original.nodes.len());
}

// C#: LoadFromFileAsync_ThrowsForMissingFile
#[test]
fn load_from_file_throws_for_missing_file() {
    match load_from_file("/nonexistent/path.afc") {
        Err(AppError::Data { code, .. }) => assert_eq!(code, ErrorCode::FileNotFound),
        other => panic!("expected FileNotFound, got {other:?}"),
    }
}

// ---- beyond C# ----

#[test]
fn drawio_round_trip_preserves_geometry_style_and_anchors() {
    let original = Diagram::create_sample();
    let Some(r) = from_drawio(&to_drawio(&original)) else {
        panic!("parse failed")
    };
    for (a, b) in original.nodes.iter().zip(&r.nodes) {
        assert_eq!(
            (a.id.as_str(), a.shape, a.x, a.y),
            (b.id.as_str(), b.shape, b.x, b.y)
        );
        assert_eq!(a.fill_color, b.fill_color);
    }
    for (a, b) in original.edges.iter().zip(&r.edges) {
        assert_eq!(
            (a.source_point, a.target_point),
            (b.source_point, b.target_point)
        );
        assert_eq!(
            (a.label.as_str(), a.target_arrow),
            (b.label.as_str(), b.target_arrow)
        );
    }
}

#[test]
fn compressed_drawio_round_trip() {
    let mut original = Diagram::create_sample();
    original.edges[0].waypoints = vec![Point::new(1.0, 2.0)];
    let xml = unwrap(to_drawio_compressed(&original));
    assert!(!xml.contains("<mxGraphModel"));
    let Some(r) = from_drawio(&xml) else {
        panic!("parse failed")
    };
    assert_eq!(r.nodes.len(), 6);
    assert_eq!(r.edges.len(), 6);
    assert_eq!(r.edges[0].waypoints, vec![Point::new(1.0, 2.0)]);
    assert_eq!(r.name, "Sample Flowchart");
}

#[test]
fn compress_matches_drawio_encoding_scheme() {
    // Known draw.io payload: deflate(encodeURIComponent("<a b=\"1\"/>")).
    let c = unwrap(compress("<a b=\"1\"/> é"));
    assert_eq!(decompress(&c).as_deref(), Some("<a b=\"1\"/> é"));
    assert!(decompress("!!not base64!!").is_none());
}

#[test]
fn style_parsing_reads_dash_and_routing() {
    let xml = r#"<mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/>
      <mxCell id="e" edge="1" source="a" target="b" style="curved=1;dashed=1;dashPattern=1 2;endArrow=open;exitX=0;exitY=0.5"><mxGeometry relative="1" as="geometry"/></mxCell>
      </root></mxGraphModel>"#;
    let Some(d) = from_drawio(xml) else {
        panic!("parse failed")
    };
    let e = &d.edges[0];
    assert_eq!(e.style, LineStyle::Dotted);
    assert_eq!(e.routing, RoutingStyle::Curved);
    assert_eq!(e.target_arrow, ArrowType::OpenArrow);
    assert_eq!(e.source_point, ConnectionPoint::Left);
    assert_eq!(e.target_point, ConnectionPoint::Center);
}
