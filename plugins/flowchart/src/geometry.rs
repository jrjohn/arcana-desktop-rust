//! Pure canvas geometry (C# `FlowChartCanvas` routing / hit-testing), no egui.

use crate::model::{
    ConnectionPoint, Diagram, DiagramEdge, DiagramNode, NodeShape, Point, RoutingStyle,
};

/// C# `CalculateOrthogonalRoute` minimum stub length.
const MIN_OFFSET: f64 = 30.0;

fn is_horizontal(p: ConnectionPoint) -> bool {
    matches!(p, ConnectionPoint::Left | ConnectionPoint::Right)
}

/// Intermediate bend points between two anchors (C# `CalculateOrthogonalRoute`).
#[must_use]
pub fn orthogonal_route(
    start: Point,
    end: Point,
    sp: ConnectionPoint,
    tp: ConnectionPoint,
) -> Vec<Point> {
    let (sh, th) = (is_horizontal(sp), is_horizontal(tp));
    if sh && th {
        let (sr, tr) = (sp == ConnectionPoint::Right, tp == ConnectionPoint::Right);
        let x = if (sr && !tr && end.x > start.x) || (!sr && tr && end.x < start.x) {
            f64::midpoint(start.x, end.x)
        } else if sr {
            start.x.max(end.x) + MIN_OFFSET
        } else {
            start.x.min(end.x) - MIN_OFFSET
        };
        vec![Point::new(x, start.y), Point::new(x, end.y)]
    } else if !sh && !th {
        let (sd, td) = (sp == ConnectionPoint::Bottom, tp == ConnectionPoint::Bottom);
        if (sd && !td && end.y > start.y) || (!sd && td && end.y < start.y) {
            if (start.x - end.x).abs() < 5.0 {
                vec![]
            } else {
                let y = f64::midpoint(start.y, end.y);
                vec![Point::new(start.x, y), Point::new(end.x, y)]
            }
        } else {
            let y = if sd {
                start.y.max(end.y) + MIN_OFFSET
            } else {
                start.y.min(end.y) - MIN_OFFSET
            };
            vec![Point::new(start.x, y), Point::new(end.x, y)]
        }
    } else if sh {
        vec![Point::new(end.x, start.y)]
    } else {
        vec![Point::new(start.x, end.y)]
    }
}

/// Full polyline for an edge: anchors plus waypoints or routing bends. `None` if a node is missing.
#[must_use]
pub fn edge_path(d: &Diagram, e: &DiagramEdge) -> Option<Vec<Point>> {
    let src = d.node(&e.source_node_id)?;
    let tgt = d.node(&e.target_node_id)?;
    let (a, b) = (src.anchor(e.source_point), tgt.anchor(e.target_point));
    let mid = if !e.waypoints.is_empty() {
        e.waypoints.clone()
    } else if e.routing == RoutingStyle::Orthogonal {
        orthogonal_route(a, b, e.source_point, e.target_point)
    } else {
        vec![]
    };
    let mut pts = Vec::with_capacity(mid.len() + 2);
    pts.push(a);
    pts.extend(mid);
    pts.push(b);
    Some(pts)
}

/// Distance from `p` to segment `ab` (C# `DistanceToSegment`).
#[must_use]
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return (p.x - a.x).hypot(p.y - a.y);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

/// Topmost node under a diagram-space point (highest z first, later in list wins ties).
#[must_use]
pub fn hit_node(d: &Diagram, p: Point) -> Option<&DiagramNode> {
    d.nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.contains(p))
        .max_by_key(|(i, n)| (n.z_index, *i))
        .map(|(_, n)| n)
}

/// Nearest edge within `tolerance` diagram units.
#[must_use]
pub fn hit_edge(d: &Diagram, p: Point, tolerance: f64) -> Option<&DiagramEdge> {
    d.edges
        .iter()
        .filter_map(|e| {
            let path = edge_path(d, e)?;
            let dist = path
                .windows(2)
                .map(|w| distance_to_segment(p, w[0], w[1]))
                .fold(f64::INFINITY, f64::min);
            (dist <= tolerance).then_some((e, dist))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
}

/// Side anchor of `node` closest to `p` (the C# canvas only offers the four sides).
#[must_use]
pub fn nearest_anchor(node: &DiagramNode, p: Point) -> ConnectionPoint {
    ConnectionPoint::SIDES
        .into_iter()
        .min_by(|a, b| {
            let da = node.anchor(*a);
            let db = node.anchor(*b);
            (da.x - p.x)
                .hypot(da.y - p.y)
                .total_cmp(&(db.x - p.x).hypot(db.y - p.y))
        })
        .unwrap_or(ConnectionPoint::Right)
}

/// Outline polygon of a node in diagram space (curves approximated).
#[must_use]
#[allow(clippy::many_single_char_names, clippy::too_many_lines)] // one arm per shape
pub fn outline(n: &DiagramNode) -> Vec<Point> {
    let (x, y, w, h) = (n.x, n.y, n.width, n.height);
    let p = Point::new;
    let ellipse = |cx: f64, cy: f64, rx: f64, ry: f64, from: f64, to: f64, steps: u32| {
        (0..=steps)
            .map(|i| {
                let t = from + (to - from) * f64::from(i) / f64::from(steps);
                p(cx + rx * t.cos(), cy + ry * t.sin())
            })
            .collect::<Vec<_>>()
    };
    let regular = |k: u32, inner: Option<f64>| {
        let count = if inner.is_some() { k * 2 } else { k };
        (0..count)
            .map(|i| {
                let r = match inner {
                    Some(f) if i % 2 == 1 => f,
                    _ => 1.0,
                };
                let t = -std::f64::consts::FRAC_PI_2
                    + std::f64::consts::TAU * f64::from(i) / f64::from(count);
                p(
                    x + w / 2.0 * (1.0 + r * t.cos()),
                    y + h / 2.0 * (1.0 + r * t.sin()),
                )
            })
            .collect::<Vec<_>>()
    };
    match n.shape {
        NodeShape::Rectangle | NodeShape::RoundedRectangle | NodeShape::Cylinder => {
            vec![p(x, y), p(x + w, y), p(x + w, y + h), p(x, y + h)]
        }
        NodeShape::Diamond => vec![
            p(x + w / 2.0, y),
            p(x + w, y + h / 2.0),
            p(x + w / 2.0, y + h),
            p(x, y + h / 2.0),
        ],
        NodeShape::Ellipse => ellipse(
            x + w / 2.0,
            y + h / 2.0,
            w / 2.0,
            h / 2.0,
            0.0,
            std::f64::consts::TAU,
            48,
        ),
        NodeShape::Parallelogram => {
            let o = w * 0.2;
            vec![p(x + o, y), p(x + w, y), p(x + w - o, y + h), p(x, y + h)]
        }
        NodeShape::Hexagon => {
            let o = w * 0.2;
            vec![
                p(x + o, y),
                p(x + w - o, y),
                p(x + w, y + h / 2.0),
                p(x + w - o, y + h),
                p(x + o, y + h),
                p(x, y + h / 2.0),
            ]
        }
        NodeShape::Document => {
            let mut pts = vec![p(x, y), p(x + w, y), p(x + w, y + h * 0.85)];
            pts.extend((0..=16).map(|i| {
                let t = f64::from(i) / 16.0;
                p(
                    x + w * (1.0 - t),
                    y + h * 0.85 + h * 0.1 * (t * std::f64::consts::TAU).sin(),
                )
            }));
            pts
        }
        NodeShape::Cloud => ellipse(
            x + w / 2.0,
            y + h / 2.0,
            w / 2.0,
            h / 2.0,
            0.0,
            std::f64::consts::TAU,
            48,
        )
        .into_iter()
        .enumerate()
        .map(|(i, q)| {
            let bump = if i % 8 < 4 { 1.0 } else { 0.9 };
            p(
                x + w / 2.0 + (q.x - x - w / 2.0) * bump,
                y + h / 2.0 + (q.y - y - h / 2.0) * bump,
            )
        })
        .collect(),
        NodeShape::Triangle => vec![p(x + w / 2.0, y), p(x + w, y + h), p(x, y + h)],
        NodeShape::Pentagon => regular(5, None),
        NodeShape::Star => regular(5, Some(0.45)),
        NodeShape::Arrow => vec![
            p(x, y + h * 0.25),
            p(x + w * 0.65, y + h * 0.25),
            p(x + w * 0.65, y),
            p(x + w, y + h / 2.0),
            p(x + w * 0.65, y + h),
            p(x + w * 0.65, y + h * 0.75),
            p(x, y + h * 0.75),
        ],
        NodeShape::Callout => vec![
            p(x, y),
            p(x + w, y),
            p(x + w, y + h * 0.75),
            p(x + w * 0.4, y + h * 0.75),
            p(x + w * 0.2, y + h),
            p(x + w * 0.25, y + h * 0.75),
            p(x, y + h * 0.75),
        ],
    }
}

/// Parses `#RRGGBB` / `#AARRGGBB` into RGBA bytes.
#[must_use]
pub fn parse_color(hex: &str) -> Option<[u8; 4]> {
    let h = hex.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match h.len() {
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(2)?, byte(4)?, byte(6)?, byte(0)?]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]
    use super::*;

    #[test]
    fn orthogonal_routes_match_csharp_cases() {
        use ConnectionPoint::{Bottom, Left, Right, Top};
        let (a, b) = (Point::new(0.0, 0.0), Point::new(100.0, 50.0));
        assert_eq!(
            orthogonal_route(a, b, Right, Left),
            vec![Point::new(50.0, 0.0), Point::new(50.0, 50.0)]
        );
        assert_eq!(
            orthogonal_route(a, b, Right, Right),
            vec![Point::new(130.0, 0.0), Point::new(130.0, 50.0)]
        );
        assert_eq!(
            orthogonal_route(a, Point::new(2.0, 50.0), Bottom, Top),
            vec![]
        );
        assert_eq!(
            orthogonal_route(a, b, Bottom, Bottom),
            vec![Point::new(0.0, 80.0), Point::new(100.0, 80.0)]
        );
        assert_eq!(
            orthogonal_route(a, b, Right, Top),
            vec![Point::new(100.0, 0.0)]
        );
        assert_eq!(
            orthogonal_route(a, b, Bottom, Left),
            vec![Point::new(0.0, 50.0)]
        );
    }

    #[test]
    fn hit_testing() {
        let d = Diagram::create_sample();
        let start = &d.nodes[0];
        assert_eq!(
            hit_node(&d, Point::new(450.0, 75.0)).map(|n| &n.id),
            Some(&start.id)
        );
        assert!(hit_node(&d, Point::new(5.0, 5.0)).is_none());
        // start(bottom 450,100) → p1(top 450,150): vertical edge at x=450.
        assert_eq!(
            hit_edge(&d, Point::new(452.0, 125.0), 5.0).map(|e| &e.id),
            Some(&d.edges[0].id)
        );
        assert!(hit_edge(&d, Point::new(0.0, 0.0), 5.0).is_none());
        assert_eq!(
            nearest_anchor(start, Point::new(399.0, 70.0)),
            ConnectionPoint::Left
        );
    }

    #[test]
    fn colors_and_outlines() {
        assert_eq!(parse_color("#FF8000"), Some([255, 128, 0, 255]));
        assert_eq!(parse_color("#80FF8000"), Some([255, 128, 0, 128]));
        assert_eq!(parse_color("red"), None);
        for shape in NodeShape::ALL {
            let n = DiagramNode {
                shape,
                ..DiagramNode::default()
            };
            assert!(outline(&n).len() >= 3, "{shape:?}");
        }
        assert_eq!(
            distance_to_segment(
                Point::new(0.0, 5.0),
                Point::new(0.0, 0.0),
                Point::new(0.0, 0.0)
            ),
            5.0
        );
    }
}
