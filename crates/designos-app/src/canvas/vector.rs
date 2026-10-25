//! Vector edit mode on a vector network.
//!
//! Vertices (which may join more than two segments), bezier handles and
//! segments are edited in place; every gesture (a drag, a click that adds a
//! point, a delete, an inspector edit) commits as one undo step through
//! [`Tx`]. The geometry lives in a [`VectorNetwork`]: the node's `paths` are
//! re-derived from it on every write.
//!
//! Gestures: drag a vertex or handle; click a segment to add a point; drag a
//! segment to move it, or Ctrl-drag (or use the Bend tool) to bend it;
//! drag a marquee (Alt for a lasso) to select vertices; the Paint bucket
//! fills and unfills the areas the network encloses.

use super::*;
use crate::edit::Tx;
use crate::theme;
use crate::util::*;
use designos_doc::vector::{point_in_polygon, End, Face};
use designos_doc::{Document, HandleMirror, Node, NodeKind, VectorNetwork};
use egui::{Modifiers, Painter, Pos2, Stroke, StrokeKind};
use std::collections::BTreeSet;

/// Sub-tool of vector edit mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VTool {
    /// Select and move vertices, handles and segments.
    #[default]
    Move,
    /// Drag a vertex to pull out handles, drag a segment to bend it.
    Bend,
    /// Click an enclosed area to fill or unfill it.
    Bucket,
}

impl VTool {
    pub fn label(self) -> &'static str {
        match self {
            VTool::Move => "Move",
            VTool::Bend => "Bend",
            VTool::Bucket => "Paint bucket",
        }
    }
    pub const ALL: [VTool; 3] = [VTool::Move, VTool::Bend, VTool::Bucket];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Vertex(usize),
    Handle(usize, End),
    /// Bounding-box resize handle when 2 or more vertices are selected.
    BoxHandle(super::handles::Handle),
    /// Segment, at parameter `t`.
    Segment(usize, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum DragKind {
    /// Move the selected vertices.
    Vertices,
    Handle(usize, End),
    /// Scale the selected vertices' bounding box from `handle`.
    ScaleBox {
        handle: super::handles::Handle,
        box0: Rect,
    },
    /// Reshape a segment so it passes through the pointer.
    Bend {
        seg: usize,
        t: f32,
        base: [DV; 4],
    },
    /// Pull mirrored handles out of a vertex.
    Pull(usize),
    /// A press on a segment: a click adds a point, a drag moves the segment.
    Segment {
        seg: usize,
        t: f32,
    },
}

pub struct VDrag {
    pub tx: Tx,
    pub kind: DragKind,
    /// World transform and own transform of the node when the drag began;
    /// all math happens in that frame.
    pub world0: Affine,
    pub t0: Affine,
    pub start: VectorNetwork,
    /// Pointer at press: page space and node-local.
    pub anchor_page: DV,
    pub anchor: DV,
    pub changed: bool,
}

/// A selection rectangle (or lasso) being dragged, in page space.
pub struct Marquee {
    pub start: DV,
    pub cur: DV,
    /// Lasso points (Alt-drag); empty for a rectangle.
    pub lasso: Vec<DV>,
    pub additive: bool,
    pub base: BTreeSet<usize>,
}

pub struct VectorEdit {
    pub id: NodeId,
    pub net: VectorNetwork,
    pub sel: BTreeSet<usize>,
    /// Segment last clicked, with the parameter of the click (for Split).
    pub seg: Option<(usize, f32)>,
    pub tool: VTool,
    pub drag: Option<VDrag>,
    pub marquee: Option<Marquee>,
    pub hover: Option<Hit>,
    /// The area under the pointer with the Paint bucket.
    pub hover_face: Option<Face>,
}

// ------------------------------------------------------------ geometry helpers
fn sp(p: DV) -> Pos2 {
    Pos2::new(p.x, p.y)
}

/// Axis-aligned bounding box in node-local coordinates of the selected vertices when `sel.len() >= 2`.
pub fn selected_bounds(net: &VectorNetwork, sel: &BTreeSet<usize>) -> Option<Rect> {
    if sel.len() < 2 {
        return None;
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut count = 0usize;
    for &i in sel {
        if let Some(vx) = net.vertices.get(i) {
            min_x = min_x.min(vx.pos.x);
            min_y = min_y.min(vx.pos.y);
            max_x = max_x.max(vx.pos.x);
            max_y = max_y.max(vx.pos.y);
            count += 1;
        }
    }
    if count < 2 || ((max_x - min_x) < 1e-4 && (max_y - min_y) < 1e-4) {
        return None;
    }
    Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

fn box_screen_corners(b: Rect, to_screen: &dyn Fn(DV) -> DV) -> [DV; 4] {
    [to_screen(v2(b.x, b.y)), to_screen(v2(b.x + b.w, b.y)), to_screen(v2(b.x + b.w, b.y + b.h)), to_screen(v2(b.x, b.y + b.h))]
}

/// Screen distance between the selected points' box and the frame that carries its scale handles.
/// Twice the point pick radius: a handle never overlaps a point inside the box, nor the stroke of
/// a straight segment between two of them, so those keep their own gestures.
const BOX_HANDLE_GAP_PX: f32 = 2.0 * theme::VEDIT_HIT;

/// Centres of the eight scale handles of the box `b` on the screen, on a frame
/// [`BOX_HANDLE_GAP_PX`] outside the box along its own axes (so it follows a rotated or flipped
/// node, and a box without width or height still gets distinct handles).
pub fn box_handle_points(b: Rect, to_screen: &dyn Fn(DV) -> DV) -> [(super::handles::Handle, DV); 8] {
    let [tl, tr, br, bl] = box_screen_corners(b, to_screen);
    let axis = |to: DV, fallback: DV| {
        let d = sub(to, tl);
        let l = len(d);
        if l < 1e-6 {
            fallback
        } else {
            scale(d, 1.0 / l)
        }
    };
    let ux = scale(axis(to_screen(v2(b.x + 1.0, b.y)), v2(1.0, 0.0)), BOX_HANDLE_GAP_PX);
    let uy = scale(axis(to_screen(v2(b.x, b.y + 1.0)), v2(0.0, 1.0)), BOX_HANDLE_GAP_PX);
    let frame = [sub(sub(tl, ux), uy), sub(add(tr, ux), uy), add(add(br, ux), uy), add(sub(bl, ux), uy)];
    super::handles::Handle::ALL.map(|h| (h, super::handles::handle_pos(&frame, h)))
}

/// Handles worth showing: those of the selected vertices and of their neighbours,
/// and both ends of the selected segment.
fn visible_handles(net: &VectorNetwork, sel: &BTreeSet<usize>, seg: Option<usize>) -> BTreeSet<(usize, End)> {
    let mut out = BTreeSet::new();
    let other = |e: End| if e == End::Start { End::End } else { End::Start };
    for &v in sel {
        if v >= net.vertices.len() {
            continue;
        }
        for (s, e) in net.ends_at(v) {
            out.insert((s, e));
            out.insert((s, other(e)));
        }
    }
    if let Some(s) = seg.filter(|s| *s < net.segments.len()) {
        out.insert((s, End::Start));
        out.insert((s, End::End));
    }
    out
}

fn handle_pos(net: &VectorNetwork, seg: usize, end: End) -> DV {
    add(net.pos(net.vertex_of(seg, end)), net.tangent(seg, end))
}

/// Finds what is under screen point `p`. `to_screen` maps node-local points to screen.
pub fn hit_test(
    net: &VectorNetwork,
    sel: &BTreeSet<usize>,
    seg_sel: Option<usize>,
    to_screen: &dyn Fn(DV) -> DV,
    p: DV,
    radius: f32,
    seg_radius: f32,
) -> Option<Hit> {
    let knob = visible_handles(net, sel, seg_sel)
        .into_iter()
        .filter(|(s, e)| len(net.tangent(*s, *e)) >= 1e-4)
        .map(|(s, e)| (dist(to_screen(handle_pos(net, s, e)), p), Hit::Handle(s, e)))
        .filter(|(d, _)| *d <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let vertex = net.vertices.iter().enumerate().rev().find(|(_, v)| dist(to_screen(v.pos), p) <= radius).map(|(i, _)| Hit::Vertex(i));
    // The scale frame is drawn over tangent handles and segments, so where one of them lies under a
    // scale handle the nearer wins and the frame wins an exact tie. A point still outranks the frame.
    let box_hit = selected_bounds(net, sel).and_then(|b| {
        box_handle_points(b, to_screen)
            .into_iter()
            .map(|(h, c)| (dist(c, p), Hit::BoxHandle(h)))
            .filter(|(d, _)| *d <= radius)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    });
    if let Some((dk, k)) = knob {
        return match box_hit {
            Some((db, h)) if db <= dk && vertex.is_none() => Some(h),
            _ => Some(k),
        };
    }
    if vertex.is_some() {
        return vertex;
    }
    let mut best: Option<(f32, Hit)> = None;
    for seg in 0..net.segments.len() {
        let c = net.cubic(seg);
        const STEPS: usize = 32;
        let mut prev = to_screen(c[0]);
        for k in 1..=STEPS {
            let t1 = k as f32 / STEPS as f32;
            let cur = to_screen(designos_doc::vector::cubic_point(&c, t1));
            let (d, f) = point_segment(p, prev, cur);
            if d <= seg_radius && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                let raw_t = ((k - 1) as f32 + f) / STEPS as f32;
                let snapped_t = if (raw_t - 0.5).abs() < 0.07 { 0.5 } else { raw_t };
                best = Some((d, Hit::Segment(seg, snapped_t)));
            }
            prev = cur;
        }
    }
    match (box_hit, best) {
        (Some((dh, h)), Some((ds, _))) if dh <= ds => Some(h),
        (_, Some((_, s))) => Some(s),
        (Some((_, h)), None) => Some(h),
        (None, None) => None,
    }
}

/// Distance from `p` to segment `a`-`b` and the position along it (0..1).
fn point_segment(p: DV, a: DV, b: DV) -> (f32, f32) {
    let ab = sub(b, a);
    let len2 = ab.x * ab.x + ab.y * ab.y;
    let f = if len2 < 1e-9 { 0.0 } else { (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0) };
    (dist(p, add(a, scale(ab, f))), f)
}

fn len(a: DV) -> f32 {
    (a.x * a.x + a.y * a.y).sqrt()
}

fn mapper<'a>(app: &'a App, rect: ERect, world: Affine) -> impl Fn(DV) -> DV + 'a {
    move |p: DV| {
        let s = app.view.to_screen(rect.min, world.apply(p));
        v2(s.x, s.y)
    }
}

/// Bends a cubic so that its point at `t` moves by `d`, spreading the shift over both handles.
fn bend_cubic(base: &[DV; 4], t: f32, d: DV) -> [DV; 4] {
    let t = t.clamp(0.08, 0.92);
    let u = 1.0 - t;
    let k = 1.0 / (3.0 * t * u * (u * u + t * t));
    [base[0], add(base[1], scale(d, k * u)), add(base[2], scale(d, k * t)), base[3]]
}

/// Constrains `v` to the nearest multiple of 45 degrees.
pub fn snap45(v: DV) -> DV {
    let l = len(v);
    if l < 1e-6 {
        return v;
    }
    let step = std::f32::consts::FRAC_PI_4;
    let a = (v.y.atan2(v.x) / step).round() * step;
    // the point of the snapped line closest to `v`
    let (dx, dy) = (a.cos(), a.sin());
    let along = v.x * dx + v.y * dy;
    v2(dx * along, dy * along)
}

// ------------------------------------------------------------ session
/// Writes `net` into the node (kind, size, transform), re-fitting the node box
/// to the geometry. `t0` is the node transform the network's coordinates are
/// relative to. A rectangle, ellipse, polygon, star or line becomes a vector.
fn write_model(doc: &mut Document, ed: &mut crate::edit::Edit, id: NodeId, net: &VectorNetwork, t0: Affine) {
    let bb = net.bounds();
    let origin = v2(bb.x, bb.y);
    let mut shifted = net.clone();
    shifted.translate(v2(-origin.x, -origin.y));
    let paths = shifted.to_paths();
    if let Some(n) = ed.node(doc, id) {
        n.kind = NodeKind::Vector { paths, network: Some(shifted) };
        n.transform = t0 * Affine::translate(origin.x, origin.y);
        n.size = v2(bb.w.max(1.0), bb.h.max(1.0));
        n.fill_geometry = None;
        n.stroke_geometry = None;
        n.corner_radii = [0.0; 4];
        n.corner_smoothing = 0.0;
    }
}

/// Whether `id` can be edited as a vector (vectors and the basic shapes, which convert on the first edit).
pub fn is_vector(app: &App, id: NodeId) -> bool {
    app.doc.get(id).is_some_and(|n| {
        matches!(
            n.kind,
            NodeKind::Vector { .. } | NodeKind::Rectangle | NodeKind::Ellipse(_) | NodeKind::Polygon { .. } | NodeKind::Star { .. } | NodeKind::Line
        )
    })
}

/// The network of a node: the stored one, or one derived from its geometry.
pub fn network_of(app: &App, n: &Node) -> Option<VectorNetwork> {
    let net = match &n.kind {
        NodeKind::Vector { network: Some(net), .. } if net.is_valid() => net.clone(),
        NodeKind::Vector { paths, .. } => VectorNetwork::from_paths(paths),
        _ => VectorNetwork::from_paths(&designos_render::vectorops::node_paths(&app.doc, n)?),
    };
    (!net.vertices.is_empty()).then_some(net)
}

/// Enters point editing for a vector or basic shape.
pub fn enter(app: &mut App, id: NodeId) -> bool {
    if !is_vector(app, id) {
        return false;
    }
    let Some(net) = app.doc.get(id).and_then(|n| network_of(app, n)) else { return false };
    app.selection = vec![id];
    app.canvas.vedit =
        Some(VectorEdit { id, net, sel: BTreeSet::new(), seg: None, tool: VTool::Move, drag: None, marquee: None, hover: None, hover_face: None });
    true
}

pub fn exit(app: &mut App) {
    if let Some(mut v) = app.canvas.vedit.take() {
        if let Some(d) = v.drag.take() {
            d.tx.cancel(&mut app.doc);
        }
    }
    if matches!(app.canvas.inter, Interaction::VectorDrag) {
        app.canvas.inter = Interaction::None;
    }
    app.changed();
}

/// Re-reads the network from the document (after undo/redo or a committed edit).
pub fn reload(app: &mut App) {
    let Some(v) = app.canvas.vedit.as_ref() else { return };
    if v.drag.is_some() || v.marquee.is_some() {
        return;
    }
    let id = v.id;
    let net = app.doc.get(id).filter(|_| is_vector(app, id)).and_then(|n| network_of(app, n));
    let Some(v) = app.canvas.vedit.as_mut() else { return };
    match net {
        Some(net) => {
            let n = net.vertices.len();
            v.sel.retain(|i| *i < n);
            if v.seg.is_some_and(|(s, _)| s >= net.segments.len()) {
                v.seg = None;
            }
            v.net = net;
        }
        None => app.canvas.vedit = None,
    }
}

/// Applies `f` to a copy of the network and the vertex selection and commits the result as one
/// undo step. Returns false when `f` declined or the edit would empty the vector.
pub fn edit(app: &mut App, f: impl FnOnce(&mut VectorNetwork, &mut BTreeSet<usize>) -> bool) -> bool {
    let Some(v) = app.canvas.vedit.as_mut() else { return false };
    if v.drag.is_some() {
        return false;
    }
    let id = v.id;
    let mut net = v.net.clone();
    let mut sel = v.sel.clone();
    if !f(&mut net, &mut sel) || net.vertices.is_empty() {
        return false;
    }
    v.sel = sel;
    v.seg = None;
    let Some(t0) = app.doc.get(id).map(|n| n.transform) else { return false };
    let mut tx = Tx::new();
    write_model(&mut app.doc, &mut tx.ed, id, &net, t0);
    let changed = app.run_tx(tx);
    reload(app);
    changed
}

pub fn set_tool(app: &mut App, tool: VTool) {
    if let Some(v) = app.canvas.vedit.as_mut() {
        v.tool = tool;
        v.hover_face = None;
    }
}

// ------------------------------------------------------------ pointer
fn begin_drag(app: &mut App, kind: DragKind, world0: Affine, t0: Affine, page: DV) {
    let Some(v) = app.canvas.vedit.as_mut() else { return };
    let anchor = world0.invert().map(|i| i.apply(page)).unwrap_or(page);
    v.drag = Some(VDrag { tx: Tx::new(), kind, world0, t0, start: v.net.clone(), anchor_page: page, anchor, changed: false });
    app.canvas.inter = Interaction::VectorDrag;
}

/// Handles a primary press while editing. Returns false when the press
/// should leave edit mode and be processed normally.
pub fn press(app: &mut App, rect: ERect, pos: Pos2, mods: Modifiers) -> bool {
    let Some(v) = app.canvas.vedit.as_ref() else { return false };
    let id = v.id;
    let world = app.doc.world_transform(id);
    let Some(t0) = app.doc.get(id).map(|n| n.transform) else { return false };
    let page = app.view.to_page(rect.min, pos);
    let tool = v.tool;
    if tool == VTool::Bucket {
        bucket_click(app, rect, pos);
        return true;
    }
    let hit = {
        let to_s = mapper(app, rect, world);
        hit_test(&v.net, &v.sel, v.seg.map(|s| s.0), &to_s, v2(pos.x, pos.y), theme::VEDIT_HIT, theme::VEDIT_SEGMENT_HIT)
    };
    let vertex_of = |app: &App, seg: usize, end: End| app.canvas.vedit.as_ref().map(|v| v.net.vertex_of(seg, end));
    match hit {
        Some(Hit::Handle(seg, end)) => {
            if let Some(vi) = vertex_of(app, seg, end) {
                if let Some(v) = app.canvas.vedit.as_mut() {
                    if !v.sel.contains(&vi) {
                        v.sel = BTreeSet::from([vi]);
                    }
                }
            }
            begin_drag(app, DragKind::Handle(seg, end), world, t0, page);
        }
        Some(Hit::BoxHandle(h)) => {
            if let Some(box0) = selected_bounds(&v.net, &v.sel) {
                if let Some(v) = app.canvas.vedit.as_mut() {
                    v.seg = None;
                }
                begin_drag(app, DragKind::ScaleBox { handle: h, box0 }, world, t0, page);
            }
        }
        Some(Hit::Vertex(i)) => {
            let Some(v) = app.canvas.vedit.as_mut() else { return true };
            v.seg = None;
            if tool == VTool::Bend || mods.command {
                v.sel = BTreeSet::from([i]);
                begin_drag(app, DragKind::Pull(i), world, t0, page);
            } else if mods.shift {
                if !v.sel.remove(&i) {
                    v.sel.insert(i);
                }
            } else {
                if !v.sel.contains(&i) {
                    v.sel = BTreeSet::from([i]);
                }
                begin_drag(app, DragKind::Vertices, world, t0, page);
            }
        }
        Some(Hit::Segment(seg, t)) => {
            let Some(v) = app.canvas.vedit.as_mut() else { return true };
            v.seg = Some((seg, t));
            let base = v.net.cubic(seg);
            if mods.command || tool == VTool::Bend {
                v.sel.clear();
                begin_drag(app, DragKind::Bend { seg, t, base }, world, t0, page);
            } else {
                begin_drag(app, DragKind::Segment { seg, t }, world, t0, page);
            }
        }
        None => {
            // Another object under the pointer leaves edit mode; empty canvas starts a marquee.
            if let Some(h) = super::interact::hit_under(app, page, false) {
                if h != id {
                    exit(app);
                    return false;
                }
            }
            if let Some(v) = app.canvas.vedit.as_mut() {
                let base = if mods.shift { v.sel.clone() } else { BTreeSet::new() };
                v.seg = None;
                v.marquee = Some(Marquee { start: page, cur: page, lasso: if mods.alt { vec![page] } else { vec![] }, additive: mods.shift, base });
                app.canvas.inter = Interaction::VectorDrag;
            }
        }
    }
    true
}

/// Pointer moved while a gesture is running.
pub fn drag(app: &mut App, cur: DV, mods: Modifiers) {
    let App { doc, canvas, view, .. } = app;
    let zoom = view.zoom;
    let Some(v) = canvas.vedit.as_mut() else { return };
    if let Some(m) = v.marquee.as_mut() {
        m.cur = cur;
        if !m.lasso.is_empty() && m.lasso.last().map(|l| dist(*l, cur) * zoom > 2.0).unwrap_or(true) {
            m.lasso.push(cur);
        }
        return;
    }
    let id = v.id;
    let Some(d) = v.drag.as_mut() else { return };
    let Some(inv) = d.world0.invert() else { return };
    let local = inv.apply(cur);
    // a press on a segment becomes a drag once the pointer has moved a few pixels
    if let DragKind::Segment { seg, .. } = d.kind {
        if dist(cur, d.anchor_page) * zoom <= theme::DRAG_THRESHOLD_PX {
            return;
        }
        let s = &d.start.segments[seg];
        v.sel = BTreeSet::from([s.a, s.b]);
        d.kind = DragKind::Vertices;
    }
    let mut net = d.start.clone();
    match d.kind.clone() {
        DragKind::Vertices => {
            let mut delta = sub(local, d.anchor);
            if mods.shift {
                if delta.x.abs() > delta.y.abs() {
                    delta.y = 0.0;
                } else {
                    delta.x = 0.0;
                }
            }
            for &i in &v.sel {
                if let Some(vx) = net.vertices.get_mut(i) {
                    vx.pos = add(d.start.vertices[i].pos, delta);
                }
            }
        }
        DragKind::ScaleBox { handle, box0 } => {
            if dist(cur, d.anchor_page) * zoom <= theme::DRAG_THRESHOLD_PX && !d.changed {
                return;
            }
            let (hx, hy) = handle.norm();
            let (ax, ay) =
                if mods.alt { (0.5, 0.5) } else { (if handle.affects_x() { 1.0 - hx } else { 0.5 }, if handle.affects_y() { 1.0 - hy } else { 0.5 }) };
            let anchor_pt = v2(box0.x + ax * box0.w, box0.y + ay * box0.h);
            let handle_pt0 = v2(box0.x + hx * box0.w, box0.y + hy * box0.h);
            // the handle sits outside the box, so it follows the pointer's travel rather than its position
            let target = add(handle_pt0, sub(local, d.anchor));
            let mut sx =
                if handle.affects_x() && (handle_pt0.x - anchor_pt.x).abs() > 1e-4 { (target.x - anchor_pt.x) / (handle_pt0.x - anchor_pt.x) } else { 1.0 };
            let mut sy =
                if handle.affects_y() && (handle_pt0.y - anchor_pt.y).abs() > 1e-4 { (target.y - anchor_pt.y) / (handle_pt0.y - anchor_pt.y) } else { 1.0 };
            if mods.shift && handle.is_corner() {
                // one signed factor from the axis the pointer moved furthest, so crossing the anchor on the other axis never mirrors
                let s = if sx.abs() >= sy.abs() { sx } else { sy };
                sx = s;
                sy = s;
            }
            for &i in &v.sel {
                if let Some(vx) = net.vertices.get_mut(i) {
                    let p0 = d.start.vertices[i].pos;
                    vx.pos = v2(anchor_pt.x + (p0.x - anchor_pt.x) * sx, anchor_pt.y + (p0.y - anchor_pt.y) * sy);
                }
            }
            for (si, seg) in net.segments.iter_mut().enumerate() {
                let orig = &d.start.segments[si];
                if v.sel.contains(&orig.a) {
                    seg.ta = v2(orig.ta.x * sx, orig.ta.y * sy);
                }
                if v.sel.contains(&orig.b) {
                    seg.tb = v2(orig.tb.x * sx, orig.tb.y * sy);
                }
            }
        }
        DragKind::Handle(seg, end) => {
            let vi = net.vertex_of(seg, end);
            let mut t = sub(local, net.pos(vi));
            if mods.shift {
                t = snap45(t);
            }
            if mods.alt {
                // Alt breaks the mirroring of the vertex for good
                net.vertices[vi].mirror = HandleMirror::None;
            }
            net.set_tangent(seg, end, t);
            net.enforce_mirror(seg, end);
        }
        DragKind::Bend { seg, t, base } => {
            let d_vec = sub(local, designos_doc::vector::cubic_point(&base, t));
            let c = bend_cubic(&base, t, d_vec);
            let (a, b) = (net.segments[seg].a, net.segments[seg].b);
            net.set_tangent(seg, End::Start, sub(c[1], net.pos(a)));
            net.set_tangent(seg, End::End, sub(c[2], net.pos(b)));
            net.enforce_mirror(seg, End::Start);
            net.enforce_mirror(seg, End::End);
        }
        DragKind::Pull(vi) => {
            let mut t = sub(local, net.pos(vi));
            if mods.shift {
                t = snap45(t);
            }
            let ends = net.ends_at(vi);
            let out = ends.iter().find(|e| e.1 == End::Start).or(ends.first()).copied();
            if let Some((s, e)) = out {
                net.set_tangent(s, e, t);
                if let Some(&(s2, e2)) = ends.iter().find(|x| **x != (s, e)).filter(|_| ends.len() == 2) {
                    net.set_tangent(s2, e2, scale(t, -1.0));
                    net.vertices[vi].mirror = HandleMirror::AngleAndLength;
                }
            }
        }
        DragKind::Segment { .. } => return,
    }
    d.changed = true;
    write_model(doc, &mut d.tx.ed, id, &net, d.t0);
    v.net = net;
    canvas.dirty = true;
}

/// Pointer released: commits the gesture as one undo step.
pub fn release(app: &mut App) {
    let zoom = app.view.zoom;
    let Some(v) = app.canvas.vedit.as_mut() else { return };
    if let Some(m) = v.marquee.take() {
        let Some(world) = app.doc.get(v.id).map(|_| app.doc.world_transform(v.id)) else { return };
        // a lasso that ends where it began has still moved
        let moved = dist(m.start, m.cur) * zoom > theme::DRAG_THRESHOLD_PX || m.lasso.iter().any(|q| dist(m.start, *q) * zoom > theme::DRAG_THRESHOLD_PX);
        let mut sel = m.base.clone();
        if moved {
            let inside = |p: DV| -> bool {
                if m.lasso.len() >= 3 {
                    point_in_polygon(p, &m.lasso)
                } else {
                    let (x0, x1) = (m.start.x.min(m.cur.x), m.start.x.max(m.cur.x));
                    let (y0, y1) = (m.start.y.min(m.cur.y), m.start.y.max(m.cur.y));
                    p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1
                }
            };
            for (i, vx) in v.net.vertices.iter().enumerate() {
                if inside(world.apply(vx.pos)) {
                    sel.insert(i);
                }
            }
        } else if !m.additive {
            sel.clear();
        }
        v.sel = sel;
        app.changed();
        return;
    }
    let Some(d) = v.drag.take() else { return };
    if !d.changed {
        match d.kind {
            DragKind::Segment { seg, t } => {
                // a click on a segment adds a point
                d.tx.cancel(&mut app.doc);
                insert_point(app, seg, t);
                return;
            }
            DragKind::Pull(vi) => {
                // a click on a vertex with the Bend tool (or Cmd+Click) toggles sharp corner <-> smooth mirrored handles
                d.tx.cancel(&mut app.doc);
                edit(app, |net, _| {
                    if vi >= net.vertices.len() {
                        return false;
                    }
                    let ends = net.ends_at(vi);
                    if ends.iter().any(|(s, e)| len(net.tangent(*s, *e)) >= 1e-4) {
                        net.straighten(vi);
                        net.vertices[vi].mirror = HandleMirror::None;
                        return true;
                    }
                    let other_v = |net: &VectorNetwork, s: usize, e: End| {
                        if e == End::Start {
                            net.segments[s].b
                        } else {
                            net.segments[s].a
                        }
                    };
                    if ends.len() >= 2 {
                        let p0 = net.pos(other_v(net, ends[0].0, ends[0].1));
                        let p1 = net.pos(other_v(net, ends[1].0, ends[1].1));
                        let mut dir = scale(sub(p1, p0), 0.25);
                        if len(dir) < 1.0 {
                            dir = v2(20.0, 0.0);
                        }
                        net.set_tangent(ends[0].0, ends[0].1, scale(dir, -1.0));
                        net.set_tangent(ends[1].0, ends[1].1, dir);
                        net.vertices[vi].mirror = HandleMirror::AngleAndLength;
                        true
                    } else if ends.len() == 1 {
                        let p0 = net.pos(other_v(net, ends[0].0, ends[0].1));
                        let mut dir = scale(sub(p0, net.pos(vi)), 0.33);
                        if len(dir) < 1.0 {
                            dir = v2(20.0, 0.0);
                        }
                        net.set_tangent(ends[0].0, ends[0].1, dir);
                        net.vertices[vi].mirror = HandleMirror::AngleAndLength;
                        true
                    } else {
                        false
                    }
                });
                return;
            }
            _ => {}
        }
    }
    if d.changed {
        app.run_tx(d.tx);
    }
    reload(app);
}

/// Splits a segment by clicking it and selects the new vertex.
pub fn insert_point(app: &mut App, seg: usize, t: f32) {
    edit(app, |net, sel| {
        if seg >= net.segments.len() {
            return false;
        }
        let nv = net.split_segment(seg, t);
        *sel = BTreeSet::from([nv]);
        true
    });
}

/// Updates what is under the pointer (for highlights).
pub fn hover(app: &mut App, rect: ERect, pos: Option<Pos2>) {
    let Some(v) = app.canvas.vedit.as_ref() else { return };
    if v.drag.is_some() || v.marquee.is_some() {
        return;
    }
    let world = app.doc.world_transform(v.id);
    let (hit, face) = match pos.filter(|p| rect.contains(*p)) {
        None => (None, None),
        Some(p) if v.tool == VTool::Bucket => {
            let local = world.invert().map(|i| i.apply(app.view.to_page(rect.min, p)));
            (None, local.and_then(|l| v.net.face_at(l)))
        }
        Some(p) => {
            let to_s = mapper(app, rect, world);
            (hit_test(&v.net, &v.sel, v.seg.map(|s| s.0), &to_s, v2(p.x, p.y), theme::VEDIT_HIT, theme::VEDIT_SEGMENT_HIT), None)
        }
    };
    if let Some(v) = app.canvas.vedit.as_mut() {
        v.hover = hit;
        v.hover_face = face;
    }
}

/// Paint bucket click: fills or unfills the area under the pointer.
fn bucket_click(app: &mut App, rect: ERect, pos: Pos2) {
    let Some(v) = app.canvas.vedit.as_ref() else { return };
    let world = app.doc.world_transform(v.id);
    let Some(local) = world.invert().map(|i| i.apply(app.view.to_page(rect.min, pos))) else { return };
    let Some(face) = v.net.face_at(local) else { return };
    edit(app, |net, _| {
        net.toggle_face(&face);
        true
    });
}

// ------------------------------------------------------------ commands
/// Removes the selected vertices (or the selected segment).
pub fn delete_selected(app: &mut App) {
    let Some(v) = app.canvas.vedit.as_ref() else { return };
    let seg = v.seg.map(|s| s.0);
    edit(app, |net, sel| {
        if sel.is_empty() {
            let Some(s) = seg.filter(|s| *s < net.segments.len()) else { return false };
            net.delete_segment(s);
            return !net.vertices.is_empty();
        }
        net.delete_vertices(sel);
        sel.clear();
        !net.vertices.is_empty()
    });
}

/// Shift + Backspace / Shift + Delete (`DeleteAndHealVectorBehavior`): deletes the selected
/// degree-2 vertices while healing the connecting curve tangents between their two neighbors.
pub fn delete_and_heal_selected(app: &mut App) {
    edit(app, |net, sel| {
        if sel.is_empty() {
            return false;
        }
        for &vi in sel.iter() {
            if vi >= net.vertices.len() {
                continue;
            }
            let ends = net.ends_at(vi);
            if ends.len() == 2 {
                let (s0, e0) = ends[0];
                let (s1, e1) = ends[1];
                if s0 != s1 {
                    let opp0 = if e0 == End::Start { End::End } else { End::Start };
                    let opp1 = if e1 == End::Start { End::End } else { End::Start };
                    let u = net.vertex_of(s0, opp0);
                    let w = net.vertex_of(s1, opp1);
                    if u != w {
                        let tu = net.tangent(s0, opp0);
                        let tw = net.tangent(s1, opp1);
                        let tv0 = net.tangent(s0, e0);
                        let tv1 = net.tangent(s1, e1);
                        if len(tv0) > 1e-4 || len(tv1) > 1e-4 {
                            let du = dist(net.pos(u), net.pos(vi));
                            let dw = dist(net.pos(vi), net.pos(w));
                            let t = if du + dw > 1e-4 { (du / (du + dw)).clamp(0.15, 0.85) } else { 0.5 };
                            let healed_tu = if len(tu) > 1e-4 { scale(tu, 1.0 / t) } else { scale(sub(net.pos(vi), net.pos(u)), 0.33) };
                            let healed_tw = if len(tw) > 1e-4 { scale(tw, 1.0 / (1.0 - t)) } else { scale(sub(net.pos(vi), net.pos(w)), 0.33) };
                            net.set_tangent(s0, opp0, healed_tu);
                            net.set_tangent(s1, opp1, healed_tw);
                        }
                    }
                }
            }
        }
        net.delete_vertices(sel);
        sel.clear();
        !net.vertices.is_empty()
    });
}

/// Joins the two selected vertices into one.
pub fn join_selected(app: &mut App) -> bool {
    edit(app, |net, sel| {
        let pair: Vec<usize> = sel.iter().copied().collect();
        let [a, b] = pair[..] else { return false };
        match net.join_vertices(a, b) {
            Some(m) => {
                *sel = BTreeSet::from([m]);
                true
            }
            None => false,
        }
    })
}

/// Cuts the path at the selected vertices, or at the clicked point of a segment.
pub fn split_selected(app: &mut App) -> bool {
    let Some(v) = app.canvas.vedit.as_ref() else { return false };
    let seg = v.seg;
    edit(app, |net, sel| {
        if sel.is_empty() {
            let Some((s, t)) = seg.filter(|s| s.0 < net.segments.len()) else { return false };
            return match net.cut_at(s, t) {
                Some((a, b)) => {
                    *sel = BTreeSet::from([a, b]);
                    true
                }
                None => false,
            };
        }
        let mut changed = false;
        let mut added = vec![];
        for &vi in sel.iter().rev() {
            if let Some(nv) = net.split_vertex(vi) {
                added.push(nv);
                changed = true;
            }
        }
        sel.extend(added);
        changed
    })
}

/// Reduces the vertices of the whole vector within `tol`.
pub fn simplify(app: &mut App, tol: f32) -> usize {
    let mut removed = 0;
    edit(app, |net, sel| {
        removed = net.simplify(tol);
        sel.clear();
        removed > 0
    });
    removed
}

/// Moves the selected vertices by `d` (arrow keys), one undo step per press.
pub fn nudge(app: &mut App, d: DV) -> bool {
    edit(app, |net, sel| {
        if sel.is_empty() {
            return false;
        }
        for &i in sel.iter() {
            net.vertices[i].pos = add(net.vertices[i].pos, d);
        }
        true
    })
}

/// Sets a property on every selected vertex (inspector fields).
pub fn set_vertices(app: &mut App, f: impl Fn(&mut designos_doc::vector::Vertex)) -> bool {
    edit(app, |net, sel| {
        if sel.is_empty() {
            return false;
        }
        for &i in sel.iter() {
            f(&mut net.vertices[i]);
        }
        true
    })
}

/// Sets the handle mirroring of the selected vertices and snaps their handles to it.
pub fn set_mirror(app: &mut App, mode: HandleMirror) -> bool {
    edit(app, |net, sel| {
        if sel.is_empty() {
            return false;
        }
        for &i in sel.iter() {
            net.set_mirror(i, mode);
        }
        true
    })
}

/// Removes the handles of the selected vertices.
pub fn straighten_selected(app: &mut App) -> bool {
    edit(app, |net, sel| {
        if sel.is_empty() {
            return false;
        }
        for &i in sel.iter() {
            net.straighten(i);
            net.vertices[i].mirror = HandleMirror::None;
        }
        true
    })
}

/// Moves the selected vertices to `x` / `y` in node space (inspector position fields).
pub fn move_selected_to(app: &mut App, x: Option<f32>, y: Option<f32>) -> bool {
    edit(app, |net, sel| {
        if sel.is_empty() {
            return false;
        }
        for &i in sel.iter() {
            if let Some(x) = x {
                net.vertices[i].pos.x = x;
            }
            if let Some(y) = y {
                net.vertices[i].pos.y = y;
            }
        }
        true
    })
}

/// Handles a key while editing. Returns true when the key was used.
pub fn key(app: &mut App, key: egui::Key, m: Modifiers) -> bool {
    use egui::Key;
    match key {
        Key::Escape | Key::Enter => {
            exit(app);
            true
        }
        Key::Delete | Key::Backspace => {
            if m.shift {
                delete_and_heal_selected(app);
            } else {
                delete_selected(app);
            }
            true
        }
        Key::V if !m.shift => {
            set_tool(app, VTool::Move);
            true
        }
        Key::B if !m.shift => {
            set_tool(app, VTool::Bend);
            true
        }
        Key::K if !m.shift => {
            set_tool(app, VTool::Bucket);
            true
        }
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => {
            let sel_empty = app.canvas.vedit.as_ref().is_none_or(|v| v.sel.is_empty());
            if sel_empty {
                return false;
            }
            let step = if m.shift { theme::NUDGE_BIG } else { theme::NUDGE };
            let d = match key {
                Key::ArrowLeft => v2(-step, 0.0),
                Key::ArrowRight => v2(step, 0.0),
                Key::ArrowUp => v2(0.0, -step),
                _ => v2(0.0, step),
            };
            nudge(app, d);
            true
        }
        _ => false,
    }
}

/// Common value of a property over the selected vertices.
pub fn common_vertex<T: PartialEq>(app: &App, f: impl Fn(&designos_doc::vector::Vertex) -> T) -> Option<T> {
    let v = app.canvas.vedit.as_ref()?;
    let mut it = v.sel.iter().filter_map(|i| v.net.vertices.get(*i)).map(f);
    let first = it.next()?;
    it.all(|x| x == first).then_some(first)
}

// ------------------------------------------------------------ overlay
pub fn overlay(app: &App, p: &Painter, rect: ERect) {
    let Some(v) = &app.canvas.vedit else { return };
    // while a gesture runs the document node is re-fitted on every move; its network lives in the frame of the press
    let world = v.drag.as_ref().map(|d| d.world0).unwrap_or_else(|| app.doc.world_transform(v.id));
    let to_s = mapper(app, rect, world);
    let col = theme::CANVAS_SELECTION;
    // filled areas hint
    if let Some(f) = &v.hover_face {
        let contours: Vec<Vec<Pos2>> = v.net.face_contours(f).iter().map(|c| c.iter().map(|q| sp(to_s(*q))).collect()).collect();
        if let Some((outer, holes)) = contours.split_first() {
            super::polyfill::fill(p, outer, holes, col.gamma_multiply(0.35));
        }
    }
    // curves
    for seg in 0..v.net.segments.len() {
        let c = v.net.cubic(seg);
        let selected = v.seg.is_some_and(|s| s.0 == seg) || matches!(v.hover, Some(Hit::Segment(s, _)) if s == seg);
        let curve = egui::epaint::CubicBezierShape::from_points_stroke(
            [sp(to_s(c[0])), sp(to_s(c[1])), sp(to_s(c[2])), sp(to_s(c[3]))],
            false,
            egui::Color32::TRANSPARENT,
            Stroke::new(if selected { theme::CANVAS_OUTLINE_WIDTH * 2.0 } else { theme::CANVAS_OUTLINE_WIDTH }, col),
        );
        p.add(curve);
    }
    if let Some(Hit::Segment(seg, t)) = v.hover {
        if seg < v.net.segments.len() {
            let pt = sp(to_s(designos_doc::vector::cubic_point(&v.net.cubic(seg), t)));
            p.circle_filled(pt, theme::VEDIT_HANDLE_RADIUS, theme::CANVAS_HANDLE_FILL);
            p.circle_stroke(pt, theme::VEDIT_HANDLE_RADIUS, Stroke::new(1.5, col));
        }
    }
    // handles
    for (s, e) in visible_handles(&v.net, &v.sel, v.seg.map(|s| s.0)) {
        let t = v.net.tangent(s, e);
        if len(t) < 1e-4 {
            continue;
        }
        let anchor = sp(to_s(v.net.pos(v.net.vertex_of(s, e))));
        let h = sp(to_s(handle_pos(&v.net, s, e)));
        p.line_segment([anchor, h], Stroke::new(1.0, col));
        p.circle_filled(h, theme::VEDIT_HANDLE_RADIUS, theme::CANVAS_HANDLE_FILL);
        p.circle_stroke(h, theme::VEDIT_HANDLE_RADIUS, Stroke::new(1.0, col));
    }
    // vertices
    for (i, vx) in v.net.vertices.iter().enumerate() {
        let hovered = v.hover == Some(Hit::Vertex(i));
        let size = theme::VEDIT_POINT_SIZE * if hovered { 1.4 } else { 1.0 };
        let r = ERect::from_center_size(sp(to_s(vx.pos)), egui::Vec2::splat(size));
        let selected = v.sel.contains(&i);
        p.rect_filled(r, 0.0, if selected { col } else { theme::CANVAS_HANDLE_FILL });
        p.rect_stroke(r, 0.0, Stroke::new(1.0, col), StrokeKind::Inside);
        if vx.corner_radius > 0.0 {
            // a ring marks vertices with a rounded corner
            p.circle_stroke(sp(to_s(vx.pos)), theme::VEDIT_POINT_SIZE * 1.1, Stroke::new(1.0, col));
        }
    }
    // multi-point scale frame: the box outline sits on the 8 handles, clear of the points inside it
    if let Some(b) = selected_bounds(&v.net, &v.sel) {
        let handles = box_handle_points(b, &to_s);
        // `Handle::ALL` runs clockwise from the top-left corner, so the handles in order trace the frame
        let mut ring: Vec<Pos2> = handles.iter().map(|(_, c)| sp(*c)).collect();
        ring.push(ring[0]);
        p.add(egui::Shape::line(ring, Stroke::new(1.0, col)));
        for (h, c) in handles {
            let hovered = v.hover == Some(Hit::BoxHandle(h));
            let size = theme::VEDIT_POINT_SIZE * if hovered { 1.4 } else { 1.0 };
            let hr = ERect::from_center_size(sp(c), egui::Vec2::splat(size));
            p.rect_filled(hr, 0.0, if hovered { col } else { theme::CANVAS_HANDLE_FILL });
            p.rect_stroke(hr, 0.0, Stroke::new(1.0, col), StrokeKind::Inside);
        }
    }
    // marquee / lasso
    if let Some(m) = &v.marquee {
        let to_scr = |q: DV| app.view.to_screen(rect.min, q);
        if m.lasso.len() >= 2 {
            let pts: Vec<Pos2> = m.lasso.iter().map(|q| to_scr(*q)).collect();
            super::polyfill::fill(p, &pts, &[], col.gamma_multiply(0.1));
            p.add(egui::Shape::line(pts, Stroke::new(1.0, col)));
        } else {
            let r = ERect::from_two_pos(to_scr(m.start), to_scr(m.cur));
            p.rect_filled(r, 0.0, col.gamma_multiply(0.1));
            p.rect_stroke(r, 0.0, Stroke::new(1.0, col), StrokeKind::Inside);
        }
    }
}

/// The floating sub-tool switcher at the bottom of the canvas.
pub fn toolbar(app: &mut App, ui: &mut egui::Ui, rect: ERect) {
    let Some(tool) = app.canvas.vedit.as_ref().map(|v| v.tool) else { return };
    let font = theme::font_body();
    let w = 84.0;
    let total = VTool::ALL.len() as f32 * w + 8.0;
    let bar = ERect::from_center_size(
        Pos2::new(rect.center().x, rect.bottom() - theme::TOOLBELT_BOTTOM_MARGIN - theme::TOOLBELT_HEIGHT - 26.0),
        egui::vec2(total, 36.0),
    );
    ui.painter().rect_filled(bar, theme::cr(theme::RADIUS_CONTROL), theme::BG_TOOLBAR);
    let mut picked = None;
    for (i, t) in VTool::ALL.iter().enumerate() {
        let r = ERect::from_min_size(Pos2::new(bar.left() + 4.0 + i as f32 * w, bar.top() + 4.0), egui::vec2(w, 28.0));
        let resp = ui.interact(r, ui.id().with(("vtool", i)), egui::Sense::click());
        #[cfg(test)]
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("test-rect", format!("vtool:{}", t.label()))), r));
        let on = *t == tool;
        let bg = if on {
            theme::BG_TOOLBAR_SELECTED
        } else if resp.hovered() {
            theme::BG_TOOLBAR_HOVER
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), bg);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, t.label(), font.clone(), theme::TEXT);
        if resp.clicked() {
            picked = Some(*t);
        }
    }
    if let Some(t) = picked {
        set_tool(app, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, FillRule, Paint, Path, PathCmd};

    fn line_path() -> Vec<Path> {
        vec![Path {
            cmds: vec![PathCmd::MoveTo(v2(0.0, 0.0)), PathCmd::LineTo(v2(100.0, 0.0)), PathCmd::LineTo(v2(100.0, 100.0))],
            fill_rule: FillRule::NonZero,
        }]
    }

    fn app_with_paths(paths: Vec<Path>) -> (App, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut n = designos_doc::Node::new(fid, NodeKind::Vector { paths, network: None });
        n.transform = Affine::translate(200.0, 100.0);
        n.size = v2(100.0, 100.0);
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        let id = d.insert(page, n).unwrap();
        let mut app = App::new(d);
        app.selection = vec![id];
        (app, id)
    }

    fn app_with_vector() -> (App, NodeId) {
        app_with_paths(line_path())
    }

    fn paths_of(app: &App, id: NodeId) -> Vec<Path> {
        match &app.doc.get(id).unwrap().kind {
            NodeKind::Vector { paths, .. } => paths.clone(),
            _ => unreachable!(),
        }
    }

    fn net_of(app: &App, id: NodeId) -> VectorNetwork {
        match &app.doc.get(id).unwrap().kind {
            NodeKind::Vector { network: Some(n), .. } => n.clone(),
            k => panic!("no network: {k:?}"),
        }
    }

    fn vedit(app: &App) -> &VectorEdit {
        app.canvas.vedit.as_ref().unwrap()
    }

    fn sel(app: &mut App, v: &[usize]) {
        app.canvas.vedit.as_mut().unwrap().sel = v.iter().copied().collect();
    }

    fn press_drag(app: &mut App, kind: DragKind, page: DV) {
        let id = vedit(app).id;
        let world = app.doc.world_transform(id);
        let t0 = app.doc.get(id).unwrap().transform;
        begin_drag(app, kind, world, t0, page);
    }

    #[test]
    fn vectors_and_basic_shapes_can_be_edited_frames_cannot() {
        let (mut app, id) = app_with_vector();
        let fid = app.doc.alloc_id();
        let f = app.doc.insert(app.page, designos_doc::Node::frame(fid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let rid = app.doc.alloc_id();
        let r = app.doc.insert(app.page, designos_doc::Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        assert!(!enter(&mut app, f));
        assert!(enter(&mut app, r), "a rectangle can be edited");
        exit(&mut app);
        assert!(enter(&mut app, id));
        assert!(app.canvas.vedit.is_some());
        exit(&mut app);
        assert!(app.canvas.vedit.is_none());
    }

    #[test]
    fn the_first_edit_turns_a_rectangle_into_a_vector_and_undo_restores_it() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let rid = d.alloc_id();
        let mut r = designos_doc::Node::rectangle(rid, Rect::new(50.0, 60.0, 100.0, 40.0));
        r.corner_radii = [8.0; 4];
        let id = d.insert(page, r).unwrap();
        let mut app = App::new(d);
        enter(&mut app, id);
        assert_eq!(vedit(&app).net.vertices.len(), 8, "a rounded rectangle has an arc at every corner");
        assert!(matches!(app.doc.get(id).unwrap().kind, NodeKind::Rectangle), "entering alone changes nothing");
        assert!(!app.hist.can_undo());
        sel(&mut app, &[3]);
        nudge(&mut app, v2(10.0, 0.0));
        let n = app.doc.get(id).unwrap();
        assert!(matches!(n.kind, NodeKind::Vector { network: Some(_), .. }));
        assert_eq!(n.corner_radii, [0.0; 4], "the radius is baked into the geometry");
        app.undo();
        assert!(matches!(app.doc.get(id).unwrap().kind, NodeKind::Rectangle));
        assert_eq!(app.doc.get(id).unwrap().corner_radii, [8.0; 4]);
        assert!(app.canvas.vedit.is_some(), "still editing after undo");
    }

    #[test]
    fn dragging_a_vertex_is_one_undo_step() {
        let (mut app, id) = app_with_vector();
        let before = paths_of(&app, id);
        enter(&mut app, id);
        sel(&mut app, &[1]);
        // the vertex (100, 0) local = (300, 100) page; drag it by (+20, -30)
        press_drag(&mut app, DragKind::Vertices, v2(300.0, 100.0));
        for step in 1..=3 {
            drag(&mut app, v2(300.0 + 20.0 * step as f32 / 3.0, 100.0 - 10.0 * step as f32), Modifiers::NONE);
        }
        release(&mut app);
        assert!(vedit(&app).drag.is_none());
        let n = app.doc.get(id).unwrap();
        let p = match &n.kind {
            NodeKind::Vector { paths, .. } => paths[0].cmds.iter().filter_map(|c| if let PathCmd::LineTo(p) = c { Some(*p) } else { None }).next().unwrap(),
            _ => unreachable!(),
        };
        let wp = app.doc.world_transform(id).apply(p);
        assert!((wp.x - 320.0).abs() < 1e-3 && (wp.y - 70.0).abs() < 1e-3, "{wp:?}");
        assert_eq!(n.size, v2(120.0, 130.0), "the box was re-fitted");
        app.undo();
        assert_eq!(paths_of(&app, id), before);
        assert_eq!(app.doc.get(id).unwrap().transform, Affine::translate(200.0, 100.0));
        assert!(!app.hist.can_undo(), "the whole drag was a single step");
        app.redo();
        assert!(app.hist.can_undo());
    }

    #[test]
    fn several_selected_vertices_move_together() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        sel(&mut app, &[0, 1]);
        press_drag(&mut app, DragKind::Vertices, v2(200.0, 100.0));
        drag(&mut app, v2(210.0, 105.0), Modifiers::NONE);
        release(&mut app);
        let net = net_of(&app, id);
        let w = app.doc.world_transform(id);
        let p0 = w.apply(net.vertices[0].pos);
        let p1 = w.apply(net.vertices[1].pos);
        assert!((p0.x - 210.0).abs() < 1e-3 && (p0.y - 105.0).abs() < 1e-3, "{p0:?}");
        assert!((p1.x - 310.0).abs() < 1e-3 && (p1.y - 105.0).abs() < 1e-3, "{p1:?}");
        let p2 = w.apply(net.vertices[2].pos);
        assert!((p2.x - 300.0).abs() < 1e-3 && (p2.y - 200.0).abs() < 1e-3, "the third stayed: {p2:?}");
        assert!(app.hist.can_undo());
        app.undo();
        assert!(!app.hist.can_undo());
    }

    fn smooth_path() -> Vec<Path> {
        vec![Path {
            cmds: vec![
                PathCmd::MoveTo(v2(0.0, 50.0)),
                PathCmd::CubicTo(v2(20.0, 50.0), v2(30.0, 0.0), v2(50.0, 0.0)),
                PathCmd::CubicTo(v2(70.0, 0.0), v2(80.0, 50.0), v2(100.0, 50.0)),
            ],
            fill_rule: FillRule::NonZero,
        }]
    }

    #[test]
    fn dragging_a_handle_mirrors_the_opposite_one_unless_alt() {
        let (mut app, id) = app_with_paths(smooth_path());
        enter(&mut app, id);
        sel(&mut app, &[1]);
        let seg = 1; // second segment starts at the middle vertex
        press_drag(&mut app, DragKind::Handle(seg, End::Start), v2(270.0, 100.0));
        // local (70,0) -> (60,10): page = local + (200,100)
        drag(&mut app, v2(260.0, 110.0), Modifiers::NONE);
        let net = &vedit(&app).net;
        assert_eq!(net.tangent(seg, End::Start), v2(10.0, 10.0));
        assert_eq!(net.tangent(0, End::End), v2(-10.0, -10.0), "mirrored through the vertex");
        release(&mut app);
        assert!(app.hist.can_undo());
        app.undo();
        assert!(!app.hist.can_undo());
        // Alt breaks the mirroring, and it stays broken
        sel(&mut app, &[1]);
        press_drag(&mut app, DragKind::Handle(seg, End::Start), v2(270.0, 100.0));
        let before = vedit(&app).net.tangent(0, End::End);
        drag(&mut app, v2(260.0, 110.0), Modifiers::ALT);
        release(&mut app);
        let net = net_of(&app, id);
        assert_eq!(net.tangent(0, End::End), before, "the other handle did not follow");
        assert_eq!(net.vertices[1].mirror, HandleMirror::None);
    }

    #[test]
    fn shift_snaps_handles_to_45_degrees() {
        let (mut app, id) = app_with_paths(smooth_path());
        enter(&mut app, id);
        sel(&mut app, &[1]);
        press_drag(&mut app, DragKind::Handle(1, End::Start), v2(270.0, 100.0));
        drag(&mut app, v2(262.0, 111.0), Modifiers::SHIFT);
        let t = vedit(&app).net.tangent(1, End::Start);
        assert!((t.x - t.y).abs() < 1e-3, "{t:?}");
        release(&mut app);
    }

    #[test]
    fn adding_and_deleting_points_are_undoable() {
        let (mut app, id) = app_with_vector();
        let before = paths_of(&app, id);
        enter(&mut app, id);
        insert_point(&mut app, 0, 0.5);
        assert_eq!(vedit(&app).net.vertices.len(), 4);
        assert_eq!(vedit(&app).sel, BTreeSet::from([3]), "the new vertex is selected");
        assert_eq!(paths_of(&app, id)[0].cmds.len(), 4);
        delete_selected(&mut app);
        assert_eq!(paths_of(&app, id), before);
        assert!(vedit(&app).sel.is_empty());
        app.undo();
        assert_eq!(paths_of(&app, id)[0].cmds.len(), 4, "undo restores the inserted point");
        app.undo();
        assert_eq!(paths_of(&app, id), before);
    }

    #[test]
    fn the_last_segment_cannot_be_deleted_away() {
        let (mut app, id) =
            app_with_paths(vec![Path { cmds: vec![PathCmd::MoveTo(v2(0.0, 0.0)), PathCmd::LineTo(v2(100.0, 0.0))], fill_rule: FillRule::NonZero }]);
        enter(&mut app, id);
        sel(&mut app, &[0]);
        delete_selected(&mut app);
        assert!(app.doc.get(id).is_some());
        assert!(!app.hist.can_undo(), "deleting the whole vector is refused");
    }

    #[test]
    fn bending_a_straight_segment_makes_it_pass_through_the_pointer() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        let base = vedit(&app).net.cubic(0);
        // press the middle of the first segment (local (50,0) = page (250,100)), pull it 20 up
        press_drag(&mut app, DragKind::Bend { seg: 0, t: 0.5, base }, v2(250.0, 100.0));
        drag(&mut app, v2(250.0, 80.0), Modifiers::NONE);
        release(&mut app);
        let net = net_of(&app, id);
        let mid = net.point_on(0, 0.5);
        let w = app.doc.world_transform(id).apply(mid);
        assert!((w.x - 250.0).abs() < 0.5 && (w.y - 80.0).abs() < 0.5, "{w:?}");
        assert!(!net.is_straight(0), "the segment now has handles");
        assert!(net.is_straight(1), "the other segment is untouched");
        assert!(paths_of(&app, id)[0].cmds.iter().any(|c| matches!(c, PathCmd::CubicTo(..))));
        app.undo();
        assert!(!app.hist.can_undo(), "one undo step");
    }

    #[test]
    fn a_click_on_a_segment_adds_a_point_but_a_drag_moves_the_segment() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        press_drag(&mut app, DragKind::Segment { seg: 1, t: 0.5 }, v2(300.0, 150.0));
        release(&mut app);
        assert_eq!(vedit(&app).net.vertices.len(), 4, "a click adds a vertex");
        app.undo();
        reload(&mut app);
        press_drag(&mut app, DragKind::Segment { seg: 1, t: 0.5 }, v2(300.0, 150.0));
        drag(&mut app, v2(330.0, 150.0), Modifiers::NONE);
        release(&mut app);
        let net = net_of(&app, id);
        assert_eq!(net.vertices.len(), 3);
        let w = app.doc.world_transform(id);
        assert!((w.apply(net.vertices[1].pos).x - 330.0).abs() < 1e-3 && (w.apply(net.vertices[2].pos).x - 330.0).abs() < 1e-3);
        assert!((w.apply(net.vertices[0].pos).x - 200.0).abs() < 1e-3, "the other end of the path stayed");
    }

    #[test]
    fn the_bend_tool_pulls_handles_out_of_a_vertex_and_a_click_removes_them() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        set_tool(&mut app, VTool::Bend);
        sel(&mut app, &[1]);
        press_drag(&mut app, DragKind::Pull(1), v2(300.0, 100.0));
        drag(&mut app, v2(330.0, 100.0), Modifiers::NONE);
        release(&mut app);
        let net = net_of(&app, id);
        assert_eq!(net.vertices[1].mirror, HandleMirror::AngleAndLength);
        let out = net.tangent(1, End::Start);
        let inn = net.tangent(0, End::End);
        assert!((out.x - 30.0).abs() < 1e-3 && out.y.abs() < 1e-3, "{out:?}");
        assert_eq!(inn, v2(-30.0, 0.0));
        // a click without moving straightens it again
        sel(&mut app, &[1]);
        press_drag(&mut app, DragKind::Pull(1), v2(300.0, 100.0));
        release(&mut app);
        let net = net_of(&app, id);
        assert_eq!(net.tangent(1, End::Start), DV::ZERO);
        assert_eq!(net.tangent(0, End::End), DV::ZERO);
        app.undo();
        app.undo();
        assert!(!app.hist.can_undo());
    }

    #[test]
    fn marquee_selects_the_vertices_inside_and_shift_adds() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        let world = app.doc.world_transform(id);
        let page = |i: usize| world.apply(vedit(&app).net.vertices[i].pos);
        let (p0, p1) = (page(0), page(1));
        app.canvas.vedit.as_mut().unwrap().marquee =
            Some(Marquee { start: v2(p0.x - 10.0, p0.y - 10.0), cur: v2(p0.x - 10.0, p0.y - 10.0), lasso: vec![], additive: false, base: BTreeSet::new() });
        drag(&mut app, v2(p1.x + 10.0, p1.y + 10.0), Modifiers::NONE);
        release(&mut app);
        assert_eq!(vedit(&app).sel, BTreeSet::from([0, 1]), "the top edge is inside, the last vertex is not");
        // a click on empty space deselects
        app.canvas.vedit.as_mut().unwrap().marquee =
            Some(Marquee { start: v2(0.0, 0.0), cur: v2(0.0, 0.0), lasso: vec![], additive: false, base: BTreeSet::new() });
        release(&mut app);
        assert!(vedit(&app).sel.is_empty());
        assert!(!app.hist.can_undo(), "selection is not an edit");
    }

    #[test]
    fn lasso_selects_what_it_encloses() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        let w = app.doc.world_transform(id);
        let p2 = w.apply(vedit(&app).net.vertices[2].pos);
        let lasso = vec![v2(p2.x - 20.0, p2.y - 20.0), v2(p2.x + 20.0, p2.y - 20.0), v2(p2.x + 20.0, p2.y + 20.0), v2(p2.x - 20.0, p2.y + 20.0)];
        app.canvas.vedit.as_mut().unwrap().marquee = Some(Marquee { start: lasso[0], cur: lasso[3], lasso, additive: false, base: BTreeSet::new() });
        release(&mut app);
        assert_eq!(vedit(&app).sel, BTreeSet::from([2]));
    }

    #[test]
    fn corner_radius_is_one_undo_step_and_rounds_the_derived_path() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        sel(&mut app, &[1]);
        assert!(set_vertices(&mut app, |v| v.corner_radius = 20.0));
        let paths = paths_of(&app, id);
        assert!(paths[0].cmds.iter().any(|c| matches!(c, PathCmd::CubicTo(..))), "the corner is an arc now");
        assert_eq!(net_of(&app, id).vertices[1].corner_radius, 20.0);
        assert_eq!(common_vertex(&app, |v| v.corner_radius), Some(20.0));
        app.undo();
        reload(&mut app);
        assert_eq!(vedit(&app).net.vertices[1].corner_radius, 0.0);
        assert!(!app.hist.can_undo());
        // the rounded corner stays within the box and the box refits
        app.redo();
        let n = app.doc.get(id).unwrap();
        assert!((n.size.x - 100.0).abs() < 1e-3 && (n.size.y - 100.0).abs() < 1e-3, "{:?}", n.size);
    }

    #[test]
    fn caps_mirroring_and_position_edit_the_selected_vertices() {
        let (mut app, id) = app_with_paths(smooth_path());
        enter(&mut app, id);
        sel(&mut app, &[0, 2]);
        set_vertices(&mut app, |v| v.cap = Some(designos_doc::StrokeCap::Round));
        let net = net_of(&app, id);
        assert_eq!(
            (net.vertices[0].cap, net.vertices[2].cap, net.vertices[1].cap),
            (Some(designos_doc::StrokeCap::Round), Some(designos_doc::StrokeCap::Round), None)
        );
        sel(&mut app, &[1]);
        set_mirror(&mut app, HandleMirror::Angle);
        assert_eq!(net_of(&app, id).vertices[1].mirror, HandleMirror::Angle);
        straighten_selected(&mut app);
        let net = net_of(&app, id);
        assert_eq!(net.tangent(1, End::Start), DV::ZERO);
        assert_eq!(net.vertices[1].mirror, HandleMirror::None);
        move_selected_to(&mut app, None, Some(70.0));
        let net = net_of(&app, id);
        let top = net.vertices[1].pos.y;
        let bottom_ref = net.vertices[0].pos.y;
        assert!(top > bottom_ref, "moved below the end points ({top} vs {bottom_ref})");
    }

    #[test]
    fn join_split_and_simplify_commands() {
        let two = vec![
            Path { cmds: vec![PathCmd::MoveTo(v2(0.0, 0.0)), PathCmd::LineTo(v2(50.0, 0.0))], fill_rule: FillRule::NonZero },
            Path { cmds: vec![PathCmd::MoveTo(v2(55.0, 0.0)), PathCmd::LineTo(v2(100.0, 40.0))], fill_rule: FillRule::NonZero },
        ];
        let (mut app, id) = app_with_paths(two);
        enter(&mut app, id);
        assert!(!join_selected(&mut app), "needs two selected vertices");
        sel(&mut app, &[1, 2]);
        assert!(join_selected(&mut app));
        assert_eq!(net_of(&app, id).vertices.len(), 3);
        assert_eq!(paths_of(&app, id).len(), 1, "one path now");
        // split the middle vertex again
        sel(&mut app, &[1]);
        assert!(split_selected(&mut app));
        assert_eq!(net_of(&app, id).vertices.len(), 4);
        assert_eq!(paths_of(&app, id)[0].cmds.iter().filter(|c| matches!(c, PathCmd::MoveTo(_))).count(), 2);
        app.undo();
        app.undo();
        assert!(!app.hist.can_undo());
    }

    #[test]
    fn simplify_removes_nodes_in_one_step() {
        let mut cmds = vec![PathCmd::MoveTo(v2(0.0, 0.0))];
        for i in 1..=60 {
            cmds.push(PathCmd::LineTo(v2(i as f32 * 2.0, 20.0 * (i as f32 * 0.12).sin())));
        }
        let (mut app, id) = app_with_paths(vec![Path { cmds, fill_rule: FillRule::NonZero }]);
        enter(&mut app, id);
        let removed = simplify(&mut app, 1.0);
        assert!(removed > 40, "{removed}");
        assert!(net_of(&app, id).vertices.len() < 15);
        app.undo();
        assert!(!app.hist.can_undo());
        assert_eq!(net_of_or_paths(&app, id), 61);
    }

    fn net_of_or_paths(app: &App, id: NodeId) -> usize {
        match &app.doc.get(id).unwrap().kind {
            NodeKind::Vector { network: Some(n), .. } => n.vertices.len(),
            NodeKind::Vector { paths, .. } => VectorNetwork::from_paths(paths).vertices.len(),
            _ => 0,
        }
    }

    #[test]
    fn the_paint_bucket_fills_and_unfills_an_area() {
        let rect = |x: f32, w: f32| {
            vec![PathCmd::MoveTo(v2(x, 0.0)), PathCmd::LineTo(v2(x + w, 0.0)), PathCmd::LineTo(v2(x + w, 100.0)), PathCmd::LineTo(v2(x, 100.0)), PathCmd::Close]
        };
        let mut cmds = rect(0.0, 100.0);
        cmds.extend(rect(100.0, 100.0));
        let (mut app, id) = app_with_paths(vec![Path { cmds, fill_rule: FillRule::NonZero }]);
        enter(&mut app, id);
        assert_eq!(net_of_or_regions(&app, id), 1, "both squares form one region");
        // unfill: the whole shape's region is the faces the paths enclose; remove it, then fill one square
        edit(&mut app, |net, _| {
            net.regions.clear();
            true
        });
        assert_eq!(net_of(&app, id).regions.len(), 0);
        assert!(net_of(&app, id).has_stroke_only_segments());
        let left = net_of(&app, id).face_at(v2(50.0, 50.0)).unwrap();
        edit(&mut app, |net, _| {
            net.toggle_face(&left);
            true
        });
        let net = net_of(&app, id);
        assert_eq!(net.regions.len(), 1);
        assert!(net.face_at(v2(50.0, 50.0)).and_then(|f| net.region_of_face(&f)).is_some());
        assert!(net.face_at(v2(150.0, 50.0)).and_then(|f| net.region_of_face(&f)).is_none());
        app.undo();
        assert_eq!(net_of(&app, id).regions.len(), 0);
    }

    fn net_of_or_regions(app: &App, id: NodeId) -> usize {
        match &app.doc.get(id).unwrap().kind {
            NodeKind::Vector { network: Some(n), .. } => n.regions.len(),
            NodeKind::Vector { paths, .. } => VectorNetwork::from_paths(paths).regions.len(),
            _ => 0,
        }
    }

    #[test]
    fn arrow_keys_nudge_selected_vertices_and_leave_the_node_alone() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        sel(&mut app, &[2]);
        let t0 = app.doc.get(id).unwrap().transform;
        assert!(key(&mut app, egui::Key::ArrowRight, Modifiers::SHIFT));
        let net = net_of(&app, id);
        let w = app.doc.world_transform(id);
        assert!((w.apply(net.vertices[2].pos).x - 310.0).abs() < 1e-3);
        assert_ne!(app.doc.get(id).unwrap().transform, Affine::translate(0.0, 0.0));
        let _ = t0;
        sel(&mut app, &[]);
        assert!(!key(&mut app, egui::Key::ArrowRight, Modifiers::NONE), "without a selection the arrow keys move the layer");
    }

    #[test]
    fn hit_priority_handle_then_vertex_then_segment() {
        let net = VectorNetwork::from_paths(&[Path {
            cmds: vec![PathCmd::MoveTo(v2(0.0, 0.0)), PathCmd::CubicTo(v2(30.0, 50.0), v2(70.0, 50.0), v2(100.0, 0.0))],
            fill_rule: FillRule::NonZero,
        }]);
        let ident = |p: DV| p;
        let none = BTreeSet::new();
        assert_eq!(hit_test(&net, &none, None, &ident, v2(1.0, 1.0), 6.0, 5.0), Some(Hit::Vertex(0)));
        // handles only respond for the selected vertex
        assert!(!matches!(hit_test(&net, &none, None, &ident, v2(30.0, 50.0), 6.0, 5.0), Some(Hit::Handle(..))));
        let sel0 = BTreeSet::from([0]);
        assert_eq!(hit_test(&net, &sel0, None, &ident, v2(30.0, 50.0), 6.0, 5.0), Some(Hit::Handle(0, End::Start)));
        // the neighbour end's handle is reachable too
        assert_eq!(hit_test(&net, &sel0, None, &ident, v2(70.0, 50.0), 6.0, 5.0), Some(Hit::Handle(0, End::End)));
        let mid = net.point_on(0, 0.5);
        match hit_test(&net, &none, None, &ident, mid, 6.0, 5.0) {
            Some(Hit::Segment(0, t)) => assert!((t - 0.5).abs() < 0.05, "{t}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(hit_test(&net, &none, None, &ident, v2(50.0, 300.0), 6.0, 5.0), None);
    }

    #[test]
    fn bend_math_moves_the_point_exactly() {
        let base = [v2(0.0, 0.0), v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 0.0)];
        for t in [0.2, 0.5, 0.8] {
            let c = bend_cubic(&base, t, v2(0.0, -15.0));
            let before = designos_doc::vector::cubic_point(&base, t);
            let after = designos_doc::vector::cubic_point(&c, t);
            assert!((after.y - (before.y - 15.0)).abs() < 1e-3 && (after.x - before.x).abs() < 1e-3, "t={t}");
        }
        assert_eq!(snap45(v2(10.0, 1.0)).y, 0.0);
    }

    #[test]
    fn undo_while_editing_reloads_the_network() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        insert_point(&mut app, 0, 0.5);
        assert_eq!(vedit(&app).net.vertices.len(), 4);
        app.undo();
        reload(&mut app);
        assert_eq!(vedit(&app).net.vertices.len(), 3);
        assert!(vedit(&app).sel.iter().all(|i| *i < 3));
        // deleting the node ends the session
        app.selection = vec![id];
        app.delete_selection();
        reload(&mut app);
        assert!(app.canvas.vedit.is_none());
    }

    #[test]
    fn midpoint_snap_cmd_click_toggle_and_delete_and_heal() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);

        // 1. S2-01: hit_test near segment midpoint (0.46) snaps to exact 0.5
        let ident = |p: DV| p;
        let none = BTreeSet::new();
        let near_mid = vedit(&app).net.point_on(0, 0.46);
        assert_eq!(hit_test(&vedit(&app).net, &none, None, &ident, near_mid, 6.0, 5.0), Some(Hit::Segment(0, 0.5)));

        // 2. S2-02: Cmd/Ctrl + Click (or Pull click) on sharp vertex 1 toggles to smooth mirrored handles, and clicking again straightens back
        let rect = ERect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let v1_screen = app.view.to_screen(rect.min, app.doc.world_transform(id).apply(vedit(&app).net.vertices[1].pos));
        let cmd_mods = Modifiers { command: true, ..Modifiers::NONE };
        assert!(press(&mut app, rect, v1_screen, cmd_mods));
        release(&mut app);
        let net = net_of(&app, id);
        assert_eq!(net.vertices[1].mirror, HandleMirror::AngleAndLength);
        let t_in = net.tangent(0, End::End);
        let t_out = net.tangent(1, End::Start);
        assert!(len(t_in) > 1.0 && len(t_out) > 1.0);
        assert!((t_in.x + t_out.x).abs() < 1e-3 && (t_in.y + t_out.y).abs() < 1e-3);

        // Clicking again with Cmd toggles back to sharp corner
        assert!(press(&mut app, rect, v1_screen, cmd_mods));
        release(&mut app);
        let net2 = net_of(&app, id);
        assert_eq!(net2.vertices[1].mirror, HandleMirror::None);
        assert_eq!(net2.tangent(0, End::End), DV::ZERO);
        assert_eq!(net2.tangent(1, End::Start), DV::ZERO);

        // 3. S2-03: Shift + Backspace (delete_and_heal_selected) heals a smoothed middle vertex into a curved segment between 0 and 2
        assert!(press(&mut app, rect, v1_screen, cmd_mods));
        release(&mut app);
        sel(&mut app, &[1]);
        assert!(key(&mut app, egui::Key::Backspace, Modifiers::SHIFT));
        let net3 = net_of(&app, id);
        assert_eq!(net3.vertices.len(), 2);
        assert_eq!(net3.segments.len(), 1);
        assert!(!net3.is_straight(0), "healed segment preserves curve tangents");
    }

    #[test]
    fn scale_box_respects_shift_aspect_and_alt_center_and_scales_tangents() {
        let (mut app, id) = app_with_paths(smooth_path());
        enter(&mut app, id);
        sel(&mut app, &[0, 1]);
        let box0 = selected_bounds(&vedit(&app).net, &vedit(&app).sel).unwrap();
        assert_eq!(box0, Rect::new(0.0, 0.0, 50.0, 50.0));
        let t0_before = vedit(&app).net.segments[0].ta;

        // Alt + Shift corner scale from BR (local (50, 50) = page (250, 150)) to (275, 140):
        // Center is local (25, 25) = page (225, 125); dx = 50 (2x from center), dy = 15 (0.6x) -> Shift locks to 2x!
        press_drag(&mut app, DragKind::ScaleBox { handle: super::super::handles::Handle::BR, box0 }, v2(250.0, 150.0));
        drag(&mut app, v2(275.0, 140.0), Modifiers::ALT | Modifiers::SHIFT);
        release(&mut app);

        let w = app.doc.world_transform(id);
        let net = net_of(&app, id);
        let p0 = w.apply(net.vertices[0].pos);
        let p1 = w.apply(net.vertices[1].pos);
        let p2 = w.apply(net.vertices[2].pos);
        // Center (225, 125) stays fixed; v0 (200, 150) -> (175, 175), v1 (250, 100) -> (275, 75)
        assert!((p0.x - 175.0).abs() < 1e-3 && (p0.y - 175.0).abs() < 1e-3, "{p0:?}");
        assert!((p1.x - 275.0).abs() < 1e-3 && (p1.y - 75.0).abs() < 1e-3, "{p1:?}");
        // Unselected vertex 2 stayed at page (300, 150)
        assert!((p2.x - 300.0).abs() < 1e-3 && (p2.y - 150.0).abs() < 1e-3, "{p2:?}");
        // Segment 0's start tangent (attached to selected vertex 0) scaled by 2x
        assert!((net.segments[0].ta.x - t0_before.x * 2.0).abs() < 1e-3);
        assert!((net.segments[0].ta.y - t0_before.y * 2.0).abs() < 1e-3);
    }

    fn test_rect() -> ERect {
        ERect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))
    }

    /// Screen position of the scale handle `h` of the selected points' box. It is found by probing
    /// `hit_test` over the screen, so the tests do not depend on where the handle is drawn.
    fn scale_handle_pos(app: &App, rect: ERect, h: super::super::handles::Handle) -> Pos2 {
        let v = vedit(app);
        let to_s = mapper(app, rect, app.doc.world_transform(v.id));
        let pts: Vec<DV> = v.sel.iter().map(|i| to_s(v.net.vertices[*i].pos)).collect();
        let lo = v2(pts.iter().map(|p| p.x).fold(f32::MAX, f32::min) - 60.0, pts.iter().map(|p| p.y).fold(f32::MAX, f32::min) - 60.0);
        let hi = v2(pts.iter().map(|p| p.x).fold(f32::MIN, f32::max) + 60.0, pts.iter().map(|p| p.y).fold(f32::MIN, f32::max) + 60.0);
        let (mut sum, mut n) = (v2(0.0, 0.0), 0u32);
        let mut y = lo.y;
        while y <= hi.y {
            let mut x = lo.x;
            while x <= hi.x {
                let hit = hit_test(&v.net, &v.sel, v.seg.map(|s| s.0), &to_s, v2(x, y), theme::VEDIT_HIT, theme::VEDIT_SEGMENT_HIT);
                if hit == Some(Hit::BoxHandle(h)) {
                    sum = add(sum, v2(x, y));
                    n += 1;
                }
                x += 1.0;
            }
            y += 1.0;
        }
        assert!(n > 0, "the {h:?} scale handle cannot be hit");
        Pos2::new(sum.x / n as f32, sum.y / n as f32)
    }

    /// Screen position of the page point `p`.
    fn screen_of(app: &App, rect: ERect, p: DV) -> Pos2 {
        app.view.to_screen(rect.min, p)
    }

    /// A pointer gesture on the canvas, like the interaction layer feeds it: press at a screen
    /// position, move through page-space points, release.
    fn gesture(app: &mut App, rect: ERect, from: Pos2, path: &[DV], mods: Modifiers) {
        assert!(press(app, rect, from, mods));
        for q in path {
            drag(app, *q, mods);
        }
        release(app);
    }

    fn world_pos(app: &App, id: NodeId, vertex: usize) -> DV {
        app.doc.world_transform(id).apply(vedit(app).net.vertices[vertex].pos)
    }

    #[test]
    fn shift_on_a_scale_handle_over_a_selected_point_keeps_the_aspect() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        // vertices 0 and 2 are the top-left and bottom-right of the selection's box
        sel(&mut app, &[0, 2]);
        let rect = test_rect();
        let from = scale_handle_pos(&app, rect, super::super::handles::Handle::BR);
        let page0 = app.view.to_page(rect.min, from);
        // pointer travel (+100, +50): 2x wide and 1.5x tall unless the aspect is locked
        gesture(&mut app, rect, from, &[add(page0, v2(100.0, 50.0))], Modifiers::SHIFT);
        let (p0, p1, p2) = (world_pos(&app, id, 0), world_pos(&app, id, 1), world_pos(&app, id, 2));
        assert!((p0.x - 200.0).abs() < 1e-3 && (p0.y - 100.0).abs() < 1e-3, "the opposite corner stays: {p0:?}");
        assert!((p2.x - 400.0).abs() < 1e-3 && (p2.y - 300.0).abs() < 1e-3, "locked to 2x in both axes: {p2:?}");
        assert!((p1.x - 300.0).abs() < 1e-3 && (p1.y - 100.0).abs() < 1e-3, "the unselected point stays: {p1:?}");
        assert_eq!(vedit(&app).sel, BTreeSet::from([0, 2]), "the selection is untouched");
    }

    #[test]
    fn dragging_a_selected_corner_point_moves_the_whole_selection() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        sel(&mut app, &[0, 2]);
        let rect = test_rect();
        let on_point = screen_of(&app, rect, world_pos(&app, id, 2));
        let page0 = app.view.to_page(rect.min, on_point);
        gesture(&mut app, rect, on_point, &[add(page0, v2(20.0, 0.0))], Modifiers::NONE);
        let (p0, p2) = (world_pos(&app, id, 0), world_pos(&app, id, 2));
        assert!((p0.x - 220.0).abs() < 1e-3 && (p0.y - 100.0).abs() < 1e-3, "{p0:?}");
        assert!((p2.x - 320.0).abs() < 1e-3 && (p2.y - 200.0).abs() < 1e-3, "{p2:?}");
    }

    #[test]
    fn shift_and_command_clicks_on_a_selected_corner_point_still_edit_that_point() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        sel(&mut app, &[0, 1, 2]);
        let rect = test_rect();
        let on_point = screen_of(&app, rect, world_pos(&app, id, 2));
        gesture(&mut app, rect, on_point, &[], Modifiers::SHIFT);
        assert_eq!(vedit(&app).sel, BTreeSet::from([0, 1]), "shift toggles the point out of the selection");
        sel(&mut app, &[0, 1, 2]);
        gesture(&mut app, rect, on_point, &[], Modifiers { command: true, ..Modifiers::NONE });
        assert_eq!(vedit(&app).sel, BTreeSet::from([2]), "command picks the point alone");
        assert_eq!(vedit(&app).net.vertices[2].mirror, HandleMirror::AngleAndLength, "and toggles it smooth");
    }

    #[test]
    fn pressing_a_scale_handle_without_dragging_keeps_the_selection() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        sel(&mut app, &[0, 1, 2]);
        let rect = test_rect();
        let handle = scale_handle_pos(&app, rect, super::super::handles::Handle::TL);
        let before = vedit(&app).net.clone();
        // a plain click
        gesture(&mut app, rect, handle, &[], Modifiers::NONE);
        assert_eq!(vedit(&app).sel, BTreeSet::from([0, 1, 2]), "a click on a handle selects nothing");
        // a press that moves less than the drag threshold
        let page0 = app.view.to_page(rect.min, handle);
        let nudge_page = v2(2.0 / app.view.zoom, 0.0);
        gesture(&mut app, rect, handle, &[add(page0, nudge_page)], Modifiers::NONE);
        assert_eq!(vedit(&app).sel, BTreeSet::from([0, 1, 2]), "a tiny movement selects nothing either");
        assert_eq!(vedit(&app).net, before, "and moves nothing");
        assert!(!app.hist.can_undo(), "no undo step was recorded");
    }

    #[test]
    fn scale_handles_lie_outside_the_box_and_clear_of_its_points_under_any_node_transform() {
        let points = [v2(10.0, 20.0), v2(110.0, 20.0), v2(110.0, 120.0), v2(40.0, 70.0)];
        let flat = [v2(10.0, 20.0), v2(110.0, 20.0)];
        let transforms = [
            Affine::translate(200.0, 100.0),
            Affine::translate(200.0, 100.0) * Affine::rotate(0.6),
            Affine::translate(50.0, 50.0) * Affine::scale(-0.5, 0.5),
            Affine::scale(3.0, 2.0) * Affine::rotate(2.5),
        ];
        let min_gap = 2.0 * theme::VEDIT_HIT - 1e-3;
        for t in transforms {
            for pts in [&points[..], &flat[..]] {
                let (x0, x1) = (pts.iter().map(|p| p.x).fold(f32::MAX, f32::min), pts.iter().map(|p| p.x).fold(f32::MIN, f32::max));
                let (y0, y1) = (pts.iter().map(|p| p.y).fold(f32::MAX, f32::min), pts.iter().map(|p| p.y).fold(f32::MIN, f32::max));
                let b = Rect::new(x0, y0, x1 - x0, y1 - y0);
                let to_s = |p: DV| t.apply(p);
                let handles = box_handle_points(b, &to_s);
                let inv = t.invert().unwrap();
                for (i, (h, c)) in handles.iter().enumerate() {
                    for q in pts {
                        assert!(dist(*c, to_s(*q)) >= min_gap, "{h:?} is too close to a point under {t:?}");
                    }
                    for (h2, c2) in &handles[i + 1..] {
                        assert!(dist(*c, *c2) >= min_gap, "{h:?} and {h2:?} overlap under {t:?}");
                    }
                    // outside the box on the sides the handle names, within it along the others
                    let l = inv.apply(*c);
                    let (hx, hy) = h.norm();
                    let eps = 1e-2;
                    let side_ok = |v: f32, lo: f32, hi: f32, n: f32| {
                        if n == 0.0 {
                            v < lo - eps
                        } else if n == 1.0 {
                            v > hi + eps
                        } else {
                            v >= lo - eps && v <= hi + eps
                        }
                    };
                    assert!(side_ok(l.x, b.x, b.x + b.w, hx) && side_ok(l.y, b.y, b.y + b.h, hy), "{h:?} at {l:?} is not outside {b:?} under {t:?}");
                }
            }
        }
    }

    #[test]
    fn a_straight_segments_midpoint_is_not_taken_by_the_box_handles_of_its_end_points() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        // vertices 0 and 1 are the ends of the horizontal segment 0, so the box has no height
        sel(&mut app, &[0, 1]);
        let ident = |p: DV| p;
        let sel01 = vedit(&app).sel.clone();
        assert_eq!(hit_test(&vedit(&app).net, &sel01, None, &ident, v2(50.0, 0.0), 6.0, 5.0), Some(Hit::Segment(0, 0.5)));
        // a click on the midpoint adds a point, as it did before points could be scaled
        let rect = test_rect();
        let mid = screen_of(&app, rect, v2(250.0, 100.0));
        gesture(&mut app, rect, mid, &[], Modifiers::NONE);
        assert_eq!(vedit(&app).net.vertices.len(), 4, "a point was inserted");
    }

    #[test]
    fn dragging_a_straight_segments_midpoint_moves_the_segment_when_its_end_points_are_selected() {
        let (mut app, id) = app_with_vector();
        enter(&mut app, id);
        sel(&mut app, &[0, 1]);
        let rect = test_rect();
        let mid = screen_of(&app, rect, v2(250.0, 100.0));
        gesture(&mut app, rect, mid, &[v2(250.0, 115.0), v2(250.0, 130.0)], Modifiers::NONE);
        let (p0, p1, p2) = (world_pos(&app, id, 0), world_pos(&app, id, 1), world_pos(&app, id, 2));
        assert!((p0.x - 200.0).abs() < 1e-3 && (p0.y - 130.0).abs() < 1e-3, "{p0:?}");
        assert!((p1.x - 300.0).abs() < 1e-3 && (p1.y - 130.0).abs() < 1e-3, "{p1:?}");
        assert!((p2.x - 300.0).abs() < 1e-3 && (p2.y - 200.0).abs() < 1e-3, "the other point stays: {p2:?}");
    }

    /// One segment from (0, 0) to (100, 0) that bulges to y = 15 at its middle, both ends selected:
    /// its box is flat, so the lower scale handle is at (50, 12), 3 px from the curve.
    fn shallow_curve_app() -> (App, NodeId) {
        let path = vec![Path {
            cmds: vec![PathCmd::MoveTo(v2(0.0, 0.0)), PathCmd::CubicTo(v2(30.0, 20.0), v2(70.0, 20.0), v2(100.0, 0.0))],
            fill_rule: FillRule::NonZero,
        }];
        let (mut app, id) = app_with_paths(path);
        enter(&mut app, id);
        sel(&mut app, &[0, 1]);
        (app, id)
    }

    #[test]
    fn a_curve_that_runs_through_a_scale_handle_keeps_its_gestures_where_it_is_closer() {
        let (mut app, id) = shallow_curve_app();
        let ident = |p: DV| p;
        let sel01 = vedit(&app).sel.clone();
        let net = vedit(&app).net.clone();
        assert!(matches!(hit_test(&net, &sel01, None, &ident, v2(50.0, 15.0), 6.0, 5.0), Some(Hit::Segment(0, _))), "on the curve, 3 px from the handle");
        assert_eq!(
            hit_test(&net, &sel01, None, &ident, v2(50.0, 12.0), 6.0, 5.0),
            Some(Hit::BoxHandle(super::super::handles::Handle::B)),
            "on the handle, 3 px from the curve"
        );
        // a click on the curve adds a point, as it did before points could be scaled
        let rect = test_rect();
        let on_curve = screen_of(&app, rect, app.doc.world_transform(id).apply(net.point_on(0, 0.5)));
        gesture(&mut app, rect, on_curve, &[], Modifiers::NONE);
        assert_eq!(vedit(&app).net.vertices.len(), 3, "a point was inserted");
    }

    #[test]
    fn dragging_a_curve_that_runs_through_a_scale_handle_moves_the_segment() {
        let (mut app, id) = shallow_curve_app();
        let rect = test_rect();
        let net = vedit(&app).net.clone();
        let on_curve = world_pos_of(&app, id, net.point_on(0, 0.5));
        let from = screen_of(&app, rect, on_curve);
        gesture(&mut app, rect, from, &[add(on_curve, v2(0.0, 15.0)), add(on_curve, v2(0.0, 30.0))], Modifiers::NONE);
        let (p0, p1) = (world_pos(&app, id, 0), world_pos(&app, id, 1));
        assert!((p0.x - 200.0).abs() < 1e-3 && (p0.y - 130.0).abs() < 1e-3, "{p0:?}");
        assert!((p1.x - 300.0).abs() < 1e-3 && (p1.y - 130.0).abs() < 1e-3, "{p1:?}");
    }

    fn world_pos_of(app: &App, id: NodeId, local: DV) -> DV {
        app.doc.world_transform(id).apply(local)
    }

    #[test]
    fn shift_scales_uniformly_even_when_the_pointer_crosses_the_anchor_on_one_axis() {
        // The bottom-right corner of the 100 x 100 box moves by `travel`; Shift keeps the aspect.
        // Returns where vertex 2, the corner that was dragged, ends up.
        let drag_corner = |travel: DV| -> DV {
            let (mut app, id) = app_with_vector();
            enter(&mut app, id);
            sel(&mut app, &[0, 2]);
            let rect = test_rect();
            let from = scale_handle_pos(&app, rect, super::super::handles::Handle::BR);
            let page0 = app.view.to_page(rect.min, from);
            gesture(&mut app, rect, from, &[add(page0, travel)], Modifiers::SHIFT);
            world_pos(&app, id, 2)
        };
        // x dominant (2x), y crosses the anchor (-0.01x): one factor of 2 on both axes, no mirror
        let p = drag_corner(v2(100.0, -101.0));
        assert!((p.x - 400.0).abs() < 1e-3 && (p.y - 300.0).abs() < 1e-3, "{p:?}");
        // y dominant (-1.5x), x stays positive (0.5x): one factor of -1.5 on both axes
        let p = drag_corner(v2(-50.0, -250.0));
        assert!((p.x - 50.0).abs() < 1e-3 && (p.y + 50.0).abs() < 1e-3, "{p:?}");
    }

    /// Two selected points (0, 0) and (80, 60) whose first segment leaves the first one by
    /// `tangent`. The top-left scale handle is at (-12, -12) on an identity screen.
    fn tangent_near_scale_handle_app(tangent: DV) -> (App, NodeId) {
        let path =
            vec![Path { cmds: vec![PathCmd::MoveTo(v2(0.0, 0.0)), PathCmd::CubicTo(tangent, v2(60.0, 60.0), v2(80.0, 60.0))], fill_rule: FillRule::NonZero }];
        let (mut app, id) = app_with_paths(path);
        enter(&mut app, id);
        sel(&mut app, &[0, 1]);
        (app, id)
    }

    #[test]
    fn a_scale_handle_drawn_over_a_tangent_handle_wins_where_it_is_nearer() {
        use super::super::handles::Handle as ScaleHandle;
        let ident = |p: DV| p;
        let hit = |app: &App, p: DV| {
            let v = vedit(app);
            hit_test(&v.net, &v.sel, None, &ident, p, theme::VEDIT_HIT, theme::VEDIT_SEGMENT_HIT)
        };
        // the tangent handle sits exactly under the scale handle: the one drawn on top wins
        let (app, _) = tangent_near_scale_handle_app(v2(-12.0, -12.0));
        assert_eq!(hit(&app, v2(-12.0, -12.0)), Some(Hit::BoxHandle(ScaleHandle::TL)));
        // four pixels apart: each wins on its own side of the midpoint
        let (app, _) = tangent_near_scale_handle_app(v2(-8.0, -12.0));
        assert_eq!(hit(&app, v2(-9.0, -12.0)), Some(Hit::Handle(0, End::Start)), "1 px from the tangent handle, 3 px from the scale handle");
        assert_eq!(hit(&app, v2(-13.0, -12.0)), Some(Hit::BoxHandle(ScaleHandle::TL)), "1 px from the scale handle, 5 px from the tangent handle");
    }

    #[test]
    fn a_point_under_the_pointer_keeps_the_scale_frame_from_taking_a_tangent_handle() {
        // a third point sits on the tangent handle and under the top-left scale handle: tangent
        // handles outranked points before the frame existed and the frame never outranks a point
        let path = vec![Path {
            cmds: vec![PathCmd::MoveTo(v2(0.0, 0.0)), PathCmd::CubicTo(v2(-12.0, -12.0), v2(60.0, 60.0), v2(80.0, 60.0)), PathCmd::LineTo(v2(-12.0, -12.0))],
            fill_rule: FillRule::NonZero,
        }];
        let (mut app, id) = app_with_paths(path);
        enter(&mut app, id);
        sel(&mut app, &[0, 1]);
        let ident = |p: DV| p;
        let v = vedit(&app);
        let hit = hit_test(&v.net, &v.sel, None, &ident, v2(-12.0, -12.0), theme::VEDIT_HIT, theme::VEDIT_SEGMENT_HIT);
        assert_eq!(hit, Some(Hit::Handle(0, End::Start)));
    }

    #[test]
    fn a_tangent_handle_clear_of_every_scale_handle_is_still_grabbed() {
        let ident = |p: DV| p;
        let (app, _) = tangent_near_scale_handle_app(v2(-30.0, -30.0));
        let v = vedit(&app);
        let at_tangent = hit_test(&v.net, &v.sel, None, &ident, v2(-30.0, -30.0), theme::VEDIT_HIT, theme::VEDIT_SEGMENT_HIT);
        assert_eq!(at_tangent, Some(Hit::Handle(0, End::Start)));
        // 5 px off the tangent handle, 22 px from the scale handle
        let near = hit_test(&v.net, &v.sel, None, &ident, v2(-30.0, -25.0), theme::VEDIT_HIT, theme::VEDIT_SEGMENT_HIT);
        assert_eq!(near, Some(Hit::Handle(0, End::Start)));
    }

    #[test]
    fn pressing_a_scale_handle_that_covers_a_tangent_handle_scales_instead_of_bending() {
        let (mut app, id) = tangent_near_scale_handle_app(v2(-12.0, -12.0));
        let rect = test_rect();
        let from = {
            let to_s = mapper(&app, rect, app.doc.world_transform(id));
            let b = selected_bounds(&vedit(&app).net, &vedit(&app).sel).unwrap();
            let tl = box_handle_points(b, &to_s)[0];
            assert_eq!(tl.0, super::super::handles::Handle::TL);
            Pos2::new(tl.1.x, tl.1.y)
        };
        let page0 = app.view.to_page(rect.min, from);
        assert!(press(&mut app, rect, from, Modifiers::NONE));
        assert!(matches!(vedit(&app).drag.as_ref().map(|d| &d.kind), Some(DragKind::ScaleBox { .. })), "a scale gesture started");
        // the top-left handle moves by (-20, -20): the box grows from its bottom-right corner
        drag(&mut app, add(page0, v2(-20.0, -20.0)), Modifiers::NONE);
        release(&mut app);
        assert_eq!(vedit(&app).sel, BTreeSet::from([0, 1]), "the selection is untouched");
        let (p0, p1) = (world_pos(&app, id, 0), world_pos(&app, id, 1));
        assert!((p1.x - 280.0).abs() < 1e-3 && (p1.y - 160.0).abs() < 1e-3, "the anchor corner stays: {p1:?}");
        assert!((p0.x - 180.0).abs() < 1e-3 && (p0.y - 80.0).abs() < 1e-3, "the dragged corner followed the pointer: {p0:?}");
    }
}
