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

/// Signed area (shoelace); positive = counter-clockwise in a y-up frame.
fn signed_area(pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// Ear-clipping triangulation of a simple polygon (convex or concave, either winding).
/// Returns index triples into `pts`; duplicate closing points are tolerated.
#[must_use]
pub fn triangulate(pts: &[Point]) -> Vec<[usize; 3]> {
    const EPS: f64 = 1e-9;
    let mut idx: Vec<usize> = Vec::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        let dup = idx
            .last()
            .is_some_and(|&j: &usize| (pts[j].x - p.x).abs() < EPS && (pts[j].y - p.y).abs() < EPS);
        if !dup {
            idx.push(i);
        }
    }
    while idx.len() > 3 {
        let (f, l) = (pts[idx[0]], pts[idx[idx.len() - 1]]);
        if (f.x - l.x).abs() < EPS && (f.y - l.y).abs() < EPS {
            idx.pop();
        } else {
            break;
        }
    }
    let poly: Vec<Point> = idx.iter().map(|&i| pts[i]).collect();
    let ccw = signed_area(&poly) > 0.0;
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < pts.len() * pts.len() + 8 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (ip, ic, inx) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (pts[ip], pts[ic], pts[inx]);
            let cr = cross(a, b, c);
            let convex = if ccw { cr > EPS } else { cr < -EPS };
            if cr.abs() <= EPS {
                // Collinear vertex: drop it without emitting a triangle.
                idx.remove(i);
                clipped = true;
                break;
            }
            if !convex {
                continue;
            }
            let blocked = idx
                .iter()
                .any(|&j| j != ip && j != ic && j != inx && in_triangle(pts[j], a, b, c));
            if blocked {
                continue;
            }
            out.push([ip, ic, inx]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
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
    fn tri_area(pts: &[Point], t: [usize; 3]) -> f64 {
        signed_area(&[pts[t[0]], pts[t[1]], pts[t[2]]]).abs()
    }

    #[test]
    fn ear_clipping_covers_concave_outlines_exactly() {
        for shape in [
            NodeShape::Star,
            NodeShape::Arrow,
            NodeShape::Callout,
            NodeShape::Cloud,
            NodeShape::Document,
            NodeShape::Ellipse,
            NodeShape::Diamond,
        ] {
            let n = DiagramNode {
                shape,
                ..DiagramNode::default()
            };
            let pts = outline(&n);
            let tris = triangulate(&pts);
            assert!(!tris.is_empty(), "{shape:?}");
            let sum: f64 = tris.iter().map(|t| tri_area(&pts, *t)).sum();
            let mut poly = pts.clone();
            if poly.len() > 1 && poly[0] == poly[poly.len() - 1] {
                poly.pop();
            }
            let area = signed_area(&poly).abs();
            assert!(
                (sum - area).abs() < area * 1e-6,
                "{shape:?}: {sum} vs {area}"
            );
        }
    }

    #[test]
    fn ear_clipping_handles_both_windings_and_degenerate_input() {
        let sq = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        assert_eq!(triangulate(&sq).len(), 2);
        let mut rev = sq;
        rev.reverse();
        assert_eq!(triangulate(&rev).len(), 2);
        assert!(triangulate(&sq[..2]).is_empty());
        // L-shape: the reflex corner must not get an ear across it.
        let l = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 1.0),
            Point::new(1.0, 1.0),
            Point::new(1.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        let sum: f64 = triangulate(&l).iter().map(|t| tri_area(&l, *t)).sum();
        assert!((sum - 3.0).abs() < 1e-9);
    }
}

/// Node bounds `(x, y, width, height)`.
pub type Bounds = (f64, f64, f64, f64);

/// The eight resize handles of a box, clockwise from top-left:
/// TL, T, TR, R, BR, B, BL, L.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn resize_handles((x, y, w, h): Bounds) -> [Point; 8] {
    let (cx, cy, r, b) = (x + w / 2.0, y + h / 2.0, x + w, y + h);
    [
        Point::new(x, y),
        Point::new(cx, y),
        Point::new(r, y),
        Point::new(r, cy),
        Point::new(r, b),
        Point::new(cx, b),
        Point::new(x, b),
        Point::new(x, cy),
    ]
}

/// New bounds after dragging handle `handle` (index into [`resize_handles`]) by `(dx, dy)`.
/// The opposite side stays fixed and the size never drops below `min`.
#[must_use]
pub fn resize_bounds(orig: Bounds, handle: usize, dx: f64, dy: f64, min: (f64, f64)) -> Bounds {
    let (x, y, w, h) = orig;
    let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
    let moves_left = matches!(handle, 0 | 6 | 7);
    let moves_right = matches!(handle, 2..=4);
    let moves_top = matches!(handle, 0..=2);
    let moves_bottom = matches!(handle, 4..=6);
    if moves_left {
        left = (left + dx).min(right - min.0);
    }
    if moves_right {
        right = (right + dx).max(left + min.0);
    }
    if moves_top {
        top = (top + dy).min(bottom - min.1);
    }
    if moves_bottom {
        bottom = (bottom + dy).max(top + min.1);
    }
    (left, top, right - left, bottom - top)
}

#[cfg(test)]
mod handle_tests {
    #![allow(clippy::float_cmp)]
    use super::*;

    #[test]
    fn handles_and_resize_keep_the_opposite_side() {
        let b = (10.0, 20.0, 100.0, 50.0);
        let hs = resize_handles(b);
        assert_eq!(hs[0], Point::new(10.0, 20.0));
        assert_eq!(hs[4], Point::new(110.0, 70.0));
        assert_eq!(hs[7], Point::new(10.0, 45.0));
        let min = (40.0, 30.0);
        assert_eq!(
            resize_bounds(b, 4, 20.0, 10.0, min),
            (10.0, 20.0, 120.0, 60.0)
        );
        assert_eq!(
            resize_bounds(b, 0, -10.0, -5.0, min),
            (0.0, 15.0, 110.0, 55.0)
        );
        // Too small: clamps against the fixed side.
        assert_eq!(
            resize_bounds(b, 0, 500.0, 500.0, min),
            (70.0, 40.0, 40.0, 30.0)
        );
        // Edge handles move one axis only.
        assert_eq!(
            resize_bounds(b, 1, 99.0, -10.0, min),
            (10.0, 10.0, 100.0, 60.0)
        );
        assert_eq!(
            resize_bounds(b, 3, 5.0, 99.0, min),
            (10.0, 20.0, 105.0, 50.0)
        );
    }
}
