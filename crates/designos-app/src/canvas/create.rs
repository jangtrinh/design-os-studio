//! Creating nodes from tools: shapes, text, pen paths.

use super::tools::Tool;
use crate::app::App;
use crate::edit::Tx;
use crate::layout;
use crate::theme;
use crate::util::*;
use designos_doc::*;

/// Where a drag-drawn shape ends up and how it is shaped, in page space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawGeom {
    pub rect: Rect,
    /// For lines: start point, length and angle (radians).
    pub line: Option<(Vec2, f32, f32)>,
}

/// Computes the drawn geometry for a drag from `start` to `cur`.
/// Shift constrains (square / 15 degree steps), alt draws from the center.
pub fn draw_geom(tool: Tool, start: Vec2, cur: Vec2, shift: bool, alt: bool) -> DrawGeom {
    let mut dx = cur.x - start.x;
    let mut dy = cur.y - start.y;
    if matches!(tool, Tool::Line | Tool::Arrow) {
        let mut len = (dx * dx + dy * dy).sqrt();
        let mut ang = dy.atan2(dx);
        if shift {
            ang = crate::canvas::handles::snap_angle(ang, 15.0);
        }
        let origin = if alt { sub(start, v2(ang.cos() * len, ang.sin() * len)) } else { start };
        if alt {
            len *= 2.0;
        }
        dx = ang.cos() * len;
        dy = ang.sin() * len;
        let end = add(origin, v2(dx, dy));
        return DrawGeom { rect: rect_from_points(origin, end), line: Some((origin, len, ang)) };
    }
    if shift {
        let s = dx.abs().max(dy.abs());
        dx = s * if dx < 0.0 { -1.0 } else { 1.0 };
        dy = s * if dy < 0.0 { -1.0 } else { 1.0 };
    }
    let rect =
        if alt { Rect::new(start.x - dx.abs(), start.y - dy.abs(), dx.abs() * 2.0, dy.abs() * 2.0) } else { rect_from_points(start, add(start, v2(dx, dy))) };
    DrawGeom { rect, line: None }
}

/// Deepest frame-like container containing `p` (page space), skipping `exclude`.
pub fn container_at(doc: &Document, page: NodeId, p: Vec2, exclude: &[NodeId]) -> NodeId {
    container_at_with_index(doc, None, page, p, exclude)
}

/// Deepest frame-like container containing `p` (page space), skipping `exclude`,
/// accelerated with `SceneIndex` bounds pruning when available.
pub fn container_at_with_index(doc: &Document, index: Option<&designos_render::SceneIndex>, page: NodeId, p: Vec2, exclude: &[NodeId]) -> NodeId {
    fn rec(doc: &Document, index: Option<&designos_render::SceneIndex>, parent: NodeId, parent_world: Affine, p: Vec2, exclude: &[NodeId]) -> Option<NodeId> {
        let pn = doc.get(parent)?;
        for &c in pn.children_front_to_back() {
            if exclude.contains(&c) {
                continue;
            }
            let Some(n) = doc.get(c) else { continue };
            if !n.visible || n.locked || !n.kind.is_container() {
                continue;
            }
            if let Some(idx) = index {
                if let Some((bx0, by0, bx1, by1)) = idx.bounds_of(c) {
                    if p.x < bx0 || p.x > bx1 || p.y < by0 || p.y > by1 {
                        continue;
                    }
                }
            }
            let frame_like = matches!(n.kind, NodeKind::Frame(_) | NodeKind::Section | NodeKind::Component(_) | NodeKind::ComponentSet(_));
            if !frame_like && n.children.is_empty() {
                continue;
            }
            let world = parent_world * n.transform;
            let Some(inv) = world.invert() else { continue };
            let l = inv.apply(p);
            let inside = l.x >= 0.0 && l.y >= 0.0 && l.x <= n.size.x && l.y <= n.size.y;
            if frame_like && inside {
                return rec(doc, index, c, world, p, exclude).or(Some(c));
            }
            if !frame_like {
                if index.is_none() && n.size.x > 0.0 && n.size.y > 0.0 && !inside {
                    continue;
                }
                if let Some(h) = rec(doc, index, c, world, p, exclude) {
                    return Some(h);
                }
            }
        }
        None
    }
    rec(doc, index, page, Affine::IDENTITY, p, exclude).unwrap_or(page)
}

fn grey() -> Paint {
    Paint::solid(Color::from_hex(&format!("{:06X}", theme::DEFAULT_SHAPE_FILL)).unwrap_or(Color::WHITE))
}

/// A fresh node for a drawing tool (size/transform filled in by the caller).
pub fn new_shape(app: &mut App, tool: Tool) -> Node {
    if let Some(n) = crate::board::create::item(app, tool) {
        return n;
    }
    let id = app.doc.alloc_id();
    let r = Rect::new(0.0, 0.0, 0.0, 0.0);
    let mut n = match tool {
        Tool::Frame => {
            let mut f = Node::frame(id, r);
            f.fills = vec![Paint::solid(Color::from_hex(&format!("{:06X}", theme::DEFAULT_FRAME_FILL)).unwrap_or(Color::WHITE))];
            f.name = app.unique_name("Frame");
            f
        }
        Tool::Section => {
            let mut s = Node::new(id, NodeKind::Section);
            s.fills = vec![Paint::solid(Color::rgb(0xF5, 0xF5, 0xF5))];
            if app.is_board() {
                // a board section is a white panel with a light outline
                s.fills = vec![Paint::solid(Color::WHITE)];
                s.stroke.paints = vec![Paint::solid(Color::rgb(0xCF, 0xCF, 0xCF))];
                s.stroke.weight = 1.0;
                s.stroke.align = StrokeAlign::Inside;
            }
            s.name = app.unique_name("Section");
            s
        }
        Tool::Slice => {
            let mut s = Node::rectangle(id, r);
            s.fills.clear();
            s.name = app.unique_name("Slice");
            s
        }
        Tool::Ellipse => {
            let mut e = Node::ellipse(id, r);
            e.name = app.unique_name("Ellipse");
            e
        }
        Tool::Polygon => {
            let mut s = Node::new(id, NodeKind::Polygon { count: 3 });
            s.fills = vec![grey()];
            s.name = app.unique_name("Polygon");
            s
        }
        Tool::Star => {
            let mut s = Node::new(id, NodeKind::Star { count: 5, inner_ratio: 0.382 });
            s.fills = vec![grey()];
            s.name = app.unique_name("Star");
            s
        }
        Tool::Line | Tool::Arrow => {
            let mut s = Node::new(id, NodeKind::Line);
            s.stroke.paints = vec![Paint::solid(Color::from_hex(&format!("{:06X}", theme::DEFAULT_STROKE)).unwrap_or(Color::BLACK))];
            s.stroke.weight = 1.0;
            s.stroke.align = StrokeAlign::Center;
            if tool == Tool::Arrow {
                s.stroke.cap = StrokeCap::Round;
            }
            s.name = app.unique_name(if tool == Tool::Arrow { "Arrow" } else { "Line" });
            s
        }
        _ => {
            let mut s = Node::rectangle(id, r);
            s.name = app.unique_name("Rectangle");
            s
        }
    };
    n.size = Vec2::ZERO;
    n
}

/// Applies draw geometry to `node` (id already inserted in `parent`).
pub fn apply_geom(doc: &mut Document, tx: &mut Tx, id: NodeId, parent: NodeId, g: &DrawGeom) {
    let Some(inv) = doc.world_transform(parent).invert() else { return };
    let Some(n) = tx.ed.node(doc, id) else { return };
    match g.line {
        Some((origin, len, ang)) => {
            let o = inv.apply(origin);
            n.transform = Affine::translate(o.x, o.y) * Affine::rotate(ang);
            n.size = v2(len.max(0.0), 0.0);
        }
        None => {
            let o = inv.apply(v2(g.rect.x, g.rect.y));
            n.transform = Affine::translate(o.x, o.y);
            n.size = v2(g.rect.w.max(1.0), g.rect.h.max(1.0));
        }
    }
    layout::relayout_up(doc, &mut tx.ed, id);
}

/// A new empty text node placed at `at` (page space) inside `parent`.
pub fn new_text(app: &mut App, parent: NodeId, at: Vec2) -> Option<(Node, Vec2)> {
    let inv = app.doc.world_transform(parent).invert()?;
    let o = inv.apply(at);
    let id = app.doc.alloc_id();
    let size = if app.is_board() { 24.0 } else { theme::DEFAULT_TEXT_SIZE };
    let style = TextStyle { font_size: size, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() };
    let mut n = Node::text(id, Rect::new(o.x, o.y, 10.0, 10.0), "", style);
    if let NodeKind::Text(t) = &n.kind {
        n.size = layout::estimate_text_size(t);
    }
    n.name = "Text".into();
    Some((n, o))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenPoint {
    pub pos: Vec2,
    pub hin: Option<Vec2>,
    pub hout: Option<Vec2>,
}

impl PenPoint {
    pub fn new(pos: Vec2) -> Self {
        Self { pos, hin: None, hout: None }
    }
}

/// Builds path commands for `pts` (page space) shifted by `-origin`.
pub fn pen_path(pts: &[PenPoint], closed: bool, origin: Vec2) -> Path {
    let o = |p: Vec2| sub(p, origin);
    let mut cmds = Vec::new();
    let Some(first) = pts.first() else { return Path::default() };
    cmds.push(PathCmd::MoveTo(o(first.pos)));
    let seg = |cmds: &mut Vec<PathCmd>, a: &PenPoint, b: &PenPoint| {
        if a.hout.is_some() || b.hin.is_some() {
            cmds.push(PathCmd::CubicTo(o(a.hout.unwrap_or(a.pos)), o(b.hin.unwrap_or(b.pos)), o(b.pos)));
        } else {
            cmds.push(PathCmd::LineTo(o(b.pos)));
        }
    };
    for w in pts.windows(2) {
        seg(&mut cmds, &w[0], &w[1]);
    }
    if closed && pts.len() > 2 {
        seg(&mut cmds, pts.last().unwrap(), first);
        cmds.push(PathCmd::Close);
    }
    Path { cmds, fill_rule: FillRule::NonZero }
}

/// Bounding box of the pen path including sampled curves.
pub fn pen_bounds(pts: &[PenPoint], closed: bool) -> Rect {
    let mut samples = Vec::new();
    let n = pts.len();
    let segs = if closed && n > 2 { n } else { n.saturating_sub(1) };
    for i in 0..segs {
        let a = &pts[i];
        let b = &pts[(i + 1) % n];
        let (c1, c2) = (a.hout.unwrap_or(a.pos), b.hin.unwrap_or(b.pos));
        for k in 0..=16 {
            let t = k as f32 / 16.0;
            let u = 1.0 - t;
            let x = u * u * u * a.pos.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.pos.x;
            let y = u * u * u * a.pos.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.pos.y;
            samples.push(v2(x, y));
        }
    }
    if samples.is_empty() {
        samples.extend(pts.iter().map(|p| p.pos));
    }
    aabb_of(&samples)
}

/// Creates a Vector node from pen points in `tx`; returns its id.
pub fn commit_pen(app: &mut App, tx: &mut Tx, pts: &[PenPoint], closed: bool) -> Option<NodeId> {
    if pts.len() < 2 {
        return None;
    }
    let parent = container_at_with_index(&app.doc, super::interact::active_index(app), app.page, pts[0].pos, &[]);
    let inv = app.doc.world_transform(parent).invert()?;
    let local_pts: Vec<PenPoint> =
        pts.iter().map(|p| PenPoint { pos: inv.apply(p.pos), hin: p.hin.map(|h| inv.apply(h)), hout: p.hout.map(|h| inv.apply(h)) }).collect();
    let bb = pen_bounds(&local_pts, closed);
    let origin = v2(bb.x, bb.y);
    let id = app.doc.alloc_id();
    let net = VectorNetwork::from_paths(&[pen_path(&local_pts, closed, origin)]);
    let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
    n.name = app.unique_name("Vector");
    n.transform = Affine::translate(bb.x, bb.y);
    n.size = v2(bb.w.max(1.0), bb.h.max(1.0));
    n.stroke.paints = vec![Paint::solid(Color::BLACK)];
    n.stroke.weight = 1.0;
    n.stroke.align = StrokeAlign::Center;
    if !app.slot_insert_allowed(parent, &[&n]) {
        return None;
    }
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![n] }).ok()?;
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    Some(id)
}

/// Constrains `p` to a multiple of 45 degrees around `prev` (Shift while placing a pen point).
pub fn snap_to_45(prev: Vec2, p: Vec2) -> Vec2 {
    add(prev, crate::canvas::vector::snap45(sub(p, prev)))
}

/// Turns a freehand stroke into smooth bezier anchors (pencil tool). `zoom` makes the tolerance
/// a screen distance. Returns the anchors and whether the stroke closes on its start.
pub fn smooth_stroke(points: &[Vec2], zoom: f32) -> (Vec<PenPoint>, bool) {
    let zoom = zoom.max(1e-3);
    let dense = decimate(points, 0.75 / zoom);
    let mut pts: Vec<Vec2> = dense.iter().map(|p| p.pos).collect();
    if pts.len() < 2 {
        return (dense, false);
    }
    let path_len: f32 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let gap = dist(pts[0], pts[pts.len() - 1]);
    // ends meeting after a long stroke close the shape
    let closed = pts.len() > 8 && gap * zoom < 10.0 && path_len > gap * 4.0;
    if closed {
        // the loop's seam is one point, not two
        pts.pop();
    }
    // low-pass the hand's jitter before fitting (the ends of an open stroke stay put)
    for _ in 0..3 {
        let prev = pts.clone();
        let n = prev.len();
        for i in 0..n {
            if !closed && (i == 0 || i + 1 == n) {
                continue;
            }
            let (a, b, c) = (prev[(i + n - 1) % n], prev[i], prev[(i + 1) % n]);
            pts[i] = v2((a.x + 2.0 * b.x + c.x) / 4.0, (a.y + 2.0 * b.y + c.y) / 4.0);
        }
    }
    let anchors = fit_polyline(&pts, closed, 1.0 / zoom, 45.0);
    (anchors.into_iter().map(|a| PenPoint { pos: a.pos, hin: a.hin, hout: a.hout }).collect(), closed)
}

/// Freehand polyline from raw points (pencil tool), lightly decimated.
pub fn decimate(points: &[Vec2], min_dist: f32) -> Vec<PenPoint> {
    let mut out: Vec<PenPoint> = Vec::new();
    for p in points {
        if out.last().map(|l| dist(l.pos, *p) >= min_dist).unwrap_or(true) {
            out.push(PenPoint::new(*p));
        }
    }
    if let (Some(last), Some(raw)) = (out.last().copied(), points.last()) {
        if last.pos != *raw {
            out.push(PenPoint::new(*raw));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_drag_and_modifiers() {
        let g = draw_geom(Tool::Rectangle, v2(10.0, 10.0), v2(50.0, 30.0), false, false);
        assert_eq!(g.rect, Rect::new(10.0, 10.0, 40.0, 20.0));
        let g = draw_geom(Tool::Rectangle, v2(10.0, 10.0), v2(50.0, 30.0), true, false);
        assert_eq!((g.rect.w, g.rect.h), (40.0, 40.0));
        let g = draw_geom(Tool::Rectangle, v2(10.0, 10.0), v2(0.0, 0.0), false, false);
        assert_eq!(g.rect, Rect::new(0.0, 0.0, 10.0, 10.0));
        let g = draw_geom(Tool::Ellipse, v2(100.0, 100.0), v2(120.0, 110.0), false, true);
        assert_eq!(g.rect, Rect::new(80.0, 90.0, 40.0, 20.0));
    }

    #[test]
    fn line_drag_snaps_with_shift() {
        let g = draw_geom(Tool::Line, v2(0.0, 0.0), v2(100.0, 10.0), true, false);
        let (_, len, ang) = g.line.unwrap();
        assert!(ang.abs() < 1e-5, "snaps to 0 degrees");
        assert!((len - (100.0f32 * 100.0 + 100.0).sqrt()).abs() < 1e-3);
        let g = draw_geom(Tool::Line, v2(0.0, 0.0), v2(30.0, 40.0), false, false);
        assert!((g.line.unwrap().1 - 50.0).abs() < 1e-4);
    }

    #[test]
    fn container_lookup_prefers_deepest_frame() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a = d.alloc_id();
        let outer = d.insert(page, Node::frame(a, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        let b = d.alloc_id();
        let inner = d.insert(outer, Node::frame(b, Rect::new(50.0, 50.0, 100.0, 100.0))).unwrap();
        assert_eq!(container_at(&d, page, v2(60.0, 60.0), &[]), inner);
        assert_eq!(container_at(&d, page, v2(10.0, 10.0), &[]), outer);
        assert_eq!(container_at(&d, page, v2(500.0, 10.0), &[]), page);
        assert_eq!(container_at(&d, page, v2(60.0, 60.0), &[inner]), outer);
    }

    #[test]
    fn container_lookup_follows_paint_order_in_reverse_z_frames() {
        use designos_doc::AutoLayout;
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a = d.alloc_id();
        let outer = d.insert(page, Node::frame(a, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        let first_id = d.alloc_id();
        let first = d.insert(outer, Node::frame(first_id, Rect::new(10.0, 10.0, 100.0, 100.0))).unwrap();
        let second_id = d.alloc_id();
        let second = d.insert(outer, Node::frame(second_id, Rect::new(50.0, 50.0, 100.0, 100.0))).unwrap();
        let overlap = v2(60.0, 60.0);
        assert_eq!(container_at(&d, page, overlap, &[]), second, "last child paints on top by default");
        if let NodeKind::Frame(fp) = &mut d.get_mut(outer).unwrap().kind {
            fp.auto_layout = Some(AutoLayout { reverse_z: true, ..Default::default() });
        }
        assert_eq!(container_at(&d, page, overlap, &[]), first, "first child paints on top with reverse_z");
        assert_eq!(container_at(&d, page, overlap, &[first]), second);
    }

    #[test]
    fn container_at_with_index_and_unique_name_fast_on_large_page() {
        let mut doc = Document::new("t");
        let _ = doc.add_page("p");
        let mut app = App::new(doc);
        let page = app.page;
        let mut target_frame = page;
        // Build 2,600+ nodes: 52 top-level groups each containing a frame and 50 child rectangles
        for g_idx in 0..52 {
            let gid = app.doc.alloc_id();
            let gx = (g_idx as f32) * 300.0;
            let mut g = Node::new(gid, NodeKind::Group);
            g.transform = Affine::translate(gx, 0.0);
            g.size = v2(200.0, 200.0);
            app.doc.insert(page, g).unwrap();

            let fid = app.doc.alloc_id();
            let mut f = Node::frame(fid, Rect::new(10.0, 10.0, 180.0, 180.0));
            f.name = format!("Frame {}", g_idx + 1);
            app.doc.insert(gid, f).unwrap();
            if g_idx == 25 {
                target_frame = fid;
            }
            for r_idx in 0..50 {
                let rid = app.doc.alloc_id();
                let mut r = Node::rectangle(rid, Rect::new((r_idx % 10) as f32 * 15.0, (r_idx / 10) as f32 * 15.0, 10.0, 10.0));
                r.name = format!("Rectangle {}", g_idx * 50 + r_idx + 1);
                app.doc.insert(fid, r).unwrap();
            }
        }
        assert!(app.doc.nodes.len() > 2_500);
        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&app.doc, page, 1);

        let budget = std::time::Duration::from_millis(1);
        let patience = std::time::Duration::from_secs(2);
        let (container_elapsed, found) =
            crate::test_timing::fastest_within(budget, patience, || container_at_with_index(&app.doc, Some(&idx), page, v2(25.0 * 300.0 + 50.0, 50.0), &[]));
        assert_eq!(found, target_frame);
        assert!(container_elapsed < budget, "container_at_with_index took {:?} at best (>= 1ms)", container_elapsed);

        app.selection = vec![target_frame];
        let (name_elapsed, name) = crate::test_timing::fastest_within(budget, patience, || app.unique_name("Rectangle"));
        assert_eq!(name, "Rectangle 51");
        assert!(name_elapsed < budget, "unique_name took {:?} at best (>= 1ms)", name_elapsed);
    }

    #[test]
    fn pen_path_lines_and_curves() {
        let pts = [PenPoint::new(v2(10.0, 10.0)), PenPoint { pos: v2(50.0, 10.0), hin: Some(v2(40.0, 0.0)), hout: None }, PenPoint::new(v2(50.0, 50.0))];
        let p = pen_path(&pts, false, v2(10.0, 10.0));
        assert_eq!(p.cmds.len(), 3);
        assert!(matches!(p.cmds[0], PathCmd::MoveTo(v) if v == v2(0.0, 0.0)));
        assert!(matches!(p.cmds[1], PathCmd::CubicTo(..)));
        assert!(matches!(p.cmds[2], PathCmd::LineTo(_)));
        let closed = pen_path(&pts, true, v2(0.0, 0.0));
        assert!(matches!(closed.cmds.last(), Some(PathCmd::Close)));
        let bb = pen_bounds(&pts, false);
        assert!(bb.y < 10.0, "curve bulges above the straight segment");
    }

    #[test]
    fn shift_snaps_pen_points_to_45_degrees() {
        let prev = v2(10.0, 10.0);
        assert_eq!(snap_to_45(prev, v2(110.0, 14.0)), v2(110.0, 10.0));
        let p = snap_to_45(prev, v2(60.0, 53.0));
        assert!((p.x - 10.0 - (p.y - 10.0)).abs() < 1e-3, "diagonal: {p:?}");
        let p = snap_to_45(prev, v2(12.0, 90.0));
        assert!((p.x - 10.0).abs() < 1e-3 && (p.y - 90.0).abs() < 0.1, "vertical: {p:?}");
        assert_eq!(snap_to_45(prev, prev), prev);
    }

    fn noisy_circle(n: usize, r: f32, wobble: f32) -> Vec<Vec2> {
        (0..=n)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / n as f32;
                let k = if i % 2 == 0 { wobble } else { -wobble };
                v2(200.0 + (r + k) * a.cos(), 200.0 + (r + k) * a.sin())
            })
            .collect()
    }

    #[test]
    fn pencil_turns_a_wobbly_circle_into_a_few_smooth_closed_anchors() {
        let raw = noisy_circle(180, 60.0, 0.4);
        let (pts, closed) = smooth_stroke(&raw, 1.0);
        // hand jitter of a pixel or two, at any zoom, is smoothed away too
        for (w, z) in [(1.0f32, 1.0f32), (1.0, 0.4), (2.0, 0.4)] {
            let (p2, c2) = smooth_stroke(&noisy_circle(120, 70.0 / z, w / z), z);
            assert!(c2 && p2.len() <= 12, "wobble {w} at zoom {z}: {} anchors", p2.len());
        }
        assert!(closed, "the end meets the start");
        assert!(pts.len() <= 10, "{} anchors for a circle", pts.len());
        assert!(pts.iter().all(|p| p.hin.is_some() && p.hout.is_some()), "smooth curves, not a polyline");
    }

    #[test]
    fn pencil_keeps_an_open_stroke_open_and_simplifies_a_straight_one() {
        let line: Vec<Vec2> = (0..60).map(|i| v2(i as f32 * 2.0, 50.0 + if i % 2 == 0 { 0.3 } else { -0.3 })).collect();
        let (pts, closed) = smooth_stroke(&line, 1.0);
        assert!(!closed);
        assert_eq!(pts.len(), 2, "jitter within the tolerance disappears");
        let arc: Vec<Vec2> = (0..=40)
            .map(|i| {
                let a = std::f32::consts::PI * i as f32 / 40.0;
                v2(100.0 + 80.0 * a.cos(), 100.0 + 80.0 * a.sin())
            })
            .collect();
        let (pts, closed) = smooth_stroke(&arc, 1.0);
        assert!(!closed && pts.len() <= 4 && pts.len() >= 2);
        // zooming in tightens the tolerance, keeping more of the hand's wobble
        let wobbly = noisy_circle(180, 60.0, 0.4);
        let (loose, _) = smooth_stroke(&wobbly, 1.0);
        let (tight, _) = smooth_stroke(&wobbly, 8.0);
        assert!(tight.len() >= loose.len());
    }

    #[test]
    fn decimate_drops_close_points_keeps_last() {
        let raw: Vec<Vec2> = (0..20).map(|i| v2(i as f32 * 0.5, 0.0)).collect();
        let d = decimate(&raw, 3.0);
        assert!(d.len() < raw.len());
        assert_eq!(d.last().unwrap().pos, *raw.last().unwrap());
    }
}
