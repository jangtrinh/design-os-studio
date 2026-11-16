//! Selection handles: positions, hit testing and resize/rotate math.

use crate::util::{add, dist, scale, sub, v2};
use designos_doc::{Affine, AutoLayout, LayoutDirection, NodeKind, Rect, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoLayoutHandleKind {
    /// Padding bar on side: 0 = Top, 1 = Right, 2 = Bottom, 3 = Left.
    Padding(usize),
    /// Gap bar between flow child `i` and `i + 1`.
    Gap(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoLayoutHandle {
    pub kind: AutoLayoutHandleKind,
    /// Center position in frame-local coordinates.
    pub local_center: Vec2,
    /// Pill dimensions in screen pixels (`12×3` or `3×12`).
    pub pill_size: Vec2,
    pub value: f32,
}

/// Whether `kind` supports on-canvas corner radius handles when selected.
pub fn supports_corner_radius_handles(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Rectangle
            | NodeKind::Frame(_)
            | NodeKind::Component(_)
            | NodeKind::ComponentSet(_)
            | NodeKind::Instance { .. }
            | NodeKind::Polygon { .. }
            | NodeKind::Star { .. }
    )
}

/// Screen positions of the 4 circular Corner Radius Handles (`TL`, `TR`, `BR`, `BL`)
/// when `zoom >= 0.5` and screen size is `>= 48px`.
/// Each handle is inset from its corner along both adjacent edges by `(corner_radius * zoom).clamp(10.0, 28.0)`.
pub fn corner_radius_handle_positions(corners: &[Vec2; 4], radii: [f32; 4], zoom: f32) -> Option<[Vec2; 4]> {
    if zoom < 0.5 {
        return None;
    }
    let w = dist(corners[0], corners[1]);
    let h = dist(corners[0], corners[3]);
    if w < 48.0 || h < 48.0 {
        return None;
    }
    let ux = scale(sub(corners[1], corners[0]), 1.0 / w);
    let uy = scale(sub(corners[3], corners[0]), 1.0 / h);
    let max_inset = (w.min(h) * 0.42).max(10.0);
    let inset_for = |r: f32| (r.max(0.0) * zoom).clamp(10.0, 28.0).min(max_inset);
    let d0 = inset_for(radii[0]);
    let d1 = inset_for(radii[1]);
    let d2 = inset_for(radii[2]);
    let d3 = inset_for(radii[3]);
    Some([
        add(corners[0], add(scale(ux, d0), scale(uy, d0))),
        add(corners[1], add(scale(ux, -d1), scale(uy, d1))),
        add(corners[2], add(scale(ux, -d2), scale(uy, -d2))),
        add(corners[3], add(scale(ux, d3), scale(uy, -d3))),
    ])
}

/// Hit-tests the 4 circular Corner Radius Handles in screen space.
pub fn hit_corner_radius_handle(corners: &[Vec2; 4], radii: [f32; 4], zoom: f32, p: Vec2, hit_radius: f32) -> Option<usize> {
    let pts = corner_radius_handle_positions(corners, radii, zoom)?;
    pts.iter().position(|&c| dist(c, p) <= hit_radius)
}

/// Computes the on-canvas pink Auto-Layout padding & gap handles (`12px × 3px` or `3px × 12px` pills)
/// for a `Horizontal` or `Vertical` auto-layout container when `zoom >= 0.5`.
pub fn auto_layout_handles(al: &AutoLayout, frame_size: Vec2, child_rects: &[Rect], zoom: f32) -> Vec<AutoLayoutHandle> {
    if zoom < 0.5
        || !matches!(al.direction, LayoutDirection::Horizontal | LayoutDirection::Vertical)
        || frame_size.x * zoom < 32.0
        || frame_size.y * zoom < 32.0
    {
        return Vec::new();
    }
    let [pt, pr, pb, pl] = al.padding;
    let min_off = 4.0 / zoom;
    let mut out = Vec::with_capacity(4 + child_rects.len().saturating_sub(1));
    // Top, Right, Bottom, Left padding handles
    out.push(AutoLayoutHandle {
        kind: AutoLayoutHandleKind::Padding(0),
        local_center: v2(frame_size.x * 0.5, (pt * 0.5).max(min_off)),
        pill_size: v2(12.0, 3.0),
        value: pt,
    });
    out.push(AutoLayoutHandle {
        kind: AutoLayoutHandleKind::Padding(1),
        local_center: v2((frame_size.x - (pr * 0.5).max(min_off)).max(0.0), frame_size.y * 0.5),
        pill_size: v2(3.0, 12.0),
        value: pr,
    });
    out.push(AutoLayoutHandle {
        kind: AutoLayoutHandleKind::Padding(2),
        local_center: v2(frame_size.x * 0.5, (frame_size.y - (pb * 0.5).max(min_off)).max(0.0)),
        pill_size: v2(12.0, 3.0),
        value: pb,
    });
    out.push(AutoLayoutHandle {
        kind: AutoLayoutHandleKind::Padding(3),
        local_center: v2((pl * 0.5).max(min_off), frame_size.y * 0.5),
        pill_size: v2(3.0, 12.0),
        value: pl,
    });

    // Gap handles between adjacent children
    for i in 0..child_rects.len().saturating_sub(1) {
        let a = &child_rects[i];
        let b = &child_rects[i + 1];
        let (local_center, pill_size) = if al.direction == LayoutDirection::Horizontal {
            let gx = (a.x + a.w + b.x) * 0.5;
            let gy = (a.y + a.h * 0.5 + b.y + b.h * 0.5) * 0.5;
            (v2(gx, gy), v2(3.0, 12.0))
        } else {
            let gx = (a.x + a.w * 0.5 + b.x + b.w * 0.5) * 0.5;
            let gy = (a.y + a.h + b.y) * 0.5;
            (v2(gx, gy), v2(12.0, 3.0))
        };
        out.push(AutoLayoutHandle { kind: AutoLayoutHandleKind::Gap(i), local_center, pill_size, value: al.item_spacing });
    }
    out
}

/// Hit-tests the on-canvas Auto-Layout padding and gap pill bars in screen space.
pub fn hit_auto_layout_handle(handles: &[AutoLayoutHandle], to_screen: impl Fn(Vec2) -> Vec2, p: Vec2, hit_pad: f32) -> Option<AutoLayoutHandleKind> {
    for h in handles {
        let c = to_screen(h.local_center);
        let hx = h.pill_size.x * 0.5 + hit_pad;
        let hy = h.pill_size.y * 0.5 + hit_pad;
        if (p.x - c.x).abs() <= hx && (p.y - c.y).abs() <= hy {
            return Some(h.kind);
        }
    }
    None
}

/// Screen positions of on-canvas shape control handles for `Ellipse` (Arc handle),
/// `Polygon` (Vertex / Count handle), and `Star` (Vertex & Inner-Radius handles).
pub fn shape_control_handles(kind: &NodeKind, corners: &[Vec2; 4], zoom: f32) -> Vec<Vec2> {
    if zoom < 0.5 {
        return Vec::new();
    }
    let w = dist(corners[0], corners[1]);
    let h = dist(corners[0], corners[3]);
    if w < 32.0 || h < 32.0 {
        return Vec::new();
    }
    let ex = sub(corners[1], corners[0]);
    let ey = sub(corners[3], corners[0]);
    let at_norm = |nx: f32, ny: f32| add(corners[0], add(scale(ex, nx), scale(ey, ny)));

    match kind {
        NodeKind::Ellipse(arc) => {
            let r_outer = 0.34;
            let mut pts = vec![at_norm(0.5 + r_outer * arc.end.cos(), 0.5 + r_outer * arc.end.sin())];
            let is_open_arc = (arc.end - arc.start - std::f32::consts::TAU).abs() > 1e-2;
            if arc.inner_radius > 0.0 || is_open_arc {
                let mid_ang = (arc.start + arc.end) * 0.5;
                let r_norm = arc.inner_radius.clamp(0.15, 0.85) * 0.5;
                pts.push(at_norm(0.5 + r_norm * mid_ang.cos(), 0.5 + r_norm * mid_ang.sin()));
                let r_start = if is_open_arc { r_outer } else { 0.44 };
                pts.push(at_norm(0.5 + r_start * arc.start.cos(), 0.5 + r_start * arc.start.sin()));
            }
            pts
        }
        NodeKind::Polygon { count } => {
            let n = (*count).max(3) as f32;
            let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU / n;
            vec![at_norm(0.5, 0.14), at_norm(0.5 + 0.38 * angle.cos(), 0.5 + 0.38 * angle.sin())]
        }
        NodeKind::Star { count, inner_ratio } => {
            let n = (*count).max(3) as f32;
            let valley_angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI / n;
            let r = 0.45 * inner_ratio.clamp(0.15, 0.85);
            vec![at_norm(0.5, 0.12), at_norm(0.5 + r * valley_angle.cos(), 0.5 + r * valley_angle.sin())]
        }
        _ => Vec::new(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeControlKind {
    EllipseSweep,
    EllipseInnerRadius,
    EllipseStartAngle,
    PolygonCount,
    StarCount,
    StarInnerRatio,
}

/// Hit-tests on-canvas shape control handles (`Ellipse` sweep/inner-radius/start-angle,
/// `Polygon` count, `Star` count/inner-ratio) in screen space.
pub fn hit_shape_control_handle(kind: &NodeKind, corners: &[Vec2; 4], zoom: f32, p: Vec2, hit_radius: f32) -> Option<ShapeControlKind> {
    let pts = shape_control_handles(kind, corners, zoom);
    for (idx, &pt) in pts.iter().enumerate() {
        if dist(p, pt) <= hit_radius {
            return match (kind, idx) {
                (NodeKind::Ellipse(_), 0) => Some(ShapeControlKind::EllipseSweep),
                (NodeKind::Ellipse(_), 1) => Some(ShapeControlKind::EllipseInnerRadius),
                (NodeKind::Ellipse(_), 2) => Some(ShapeControlKind::EllipseStartAngle),
                (NodeKind::Polygon { .. }, 0 | 1) => Some(ShapeControlKind::PolygonCount),
                (NodeKind::Star { .. }, 0) => Some(ShapeControlKind::StarCount),
                (NodeKind::Star { .. }, 1) => Some(ShapeControlKind::StarInnerRatio),
                _ => None,
            };
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TL,
    T,
    TR,
    R,
    BR,
    B,
    BL,
    L,
}

impl Handle {
    pub const ALL: [Handle; 8] = [Handle::TL, Handle::T, Handle::TR, Handle::R, Handle::BR, Handle::B, Handle::BL, Handle::L];

    /// Normalized position on the box (0..1 on each axis).
    pub fn norm(self) -> (f32, f32) {
        match self {
            Handle::TL => (0.0, 0.0),
            Handle::T => (0.5, 0.0),
            Handle::TR => (1.0, 0.0),
            Handle::R => (1.0, 0.5),
            Handle::BR => (1.0, 1.0),
            Handle::B => (0.5, 1.0),
            Handle::BL => (0.0, 1.0),
            Handle::L => (0.0, 0.5),
        }
    }
    pub fn affects_x(self) -> bool {
        self.norm().0 != 0.5
    }
    pub fn affects_y(self) -> bool {
        self.norm().1 != 0.5
    }
    pub fn is_corner(self) -> bool {
        self.affects_x() && self.affects_y()
    }
    pub fn cursor(self) -> egui::CursorIcon {
        match self {
            Handle::TL | Handle::BR => egui::CursorIcon::ResizeNwSe,
            Handle::TR | Handle::BL => egui::CursorIcon::ResizeNeSw,
            Handle::T | Handle::B => egui::CursorIcon::ResizeVertical,
            Handle::L | Handle::R => egui::CursorIcon::ResizeHorizontal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleHit {
    Resize(Handle),
    /// Rotation zone just outside the given corner (0=TL, 1=TR, 2=BR, 3=BL).
    Rotate(usize),
}

/// Position of a handle on a box given its (tl, tr, br, bl) corners.
pub fn handle_pos(corners: &[Vec2; 4], h: Handle) -> Vec2 {
    let (nx, ny) = h.norm();
    let ex = sub(corners[1], corners[0]);
    let ey = sub(corners[3], corners[0]);
    add(corners[0], add(scale(ex, nx), scale(ey, ny)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmartSelectionHandleKind {
    /// Drag to adjust uniform gap along `horiz` axis (`true` = X, `false` = Y).
    Gap { horiz: bool, index: usize },
    /// Drag center ring of `NodeId` onto another selected node to swap positions.
    Swap(designos_doc::NodeId),
}

/// Computes Figma Smart Selection handles (`Swap` center rings and `Gap` spacing bars)
/// when 2 or more nodes are selected.
/// Returns `(kind, center_in_rect_space, screen_size_px)`.
pub fn smart_selection_handles(items: &[(designos_doc::NodeId, Rect)]) -> Vec<(SmartSelectionHandleKind, Vec2, Vec2)> {
    if items.len() < 2 {
        return Vec::new();
    }
    let mut min_cx = f32::MAX;
    let mut max_cx = f32::MIN;
    let mut min_cy = f32::MAX;
    let mut max_cy = f32::MIN;
    for (_, r) in items {
        let cx = r.x + r.w * 0.5;
        let cy = r.y + r.h * 0.5;
        min_cx = min_cx.min(cx);
        max_cx = max_cx.max(cx);
        min_cy = min_cy.min(cy);
        max_cy = max_cy.max(cy);
    }
    let horiz = (max_cx - min_cx) >= (max_cy - min_cy);
    let mut sorted = items.to_vec();
    if horiz {
        sorted.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then_with(|| a.1.y.total_cmp(&b.1.y)));
    } else {
        sorted.sort_by(|a, b| a.1.y.total_cmp(&b.1.y).then_with(|| a.1.x.total_cmp(&b.1.x)));
    }

    let mut out = Vec::with_capacity(sorted.len() * 2 - 1);
    for &(id, ref r) in &sorted {
        out.push((SmartSelectionHandleKind::Swap(id), v2(r.x + r.w * 0.5, r.y + r.h * 0.5), v2(10.0, 10.0)));
    }
    for i in 0..(sorted.len() - 1) {
        let a = &sorted[i].1;
        let b = &sorted[i + 1].1;
        let (center, size) = if horiz {
            let gx = (a.x + a.w + b.x) * 0.5;
            let gy = (a.y + a.h * 0.5 + b.y + b.h * 0.5) * 0.5;
            (v2(gx, gy), v2(4.0, 14.0))
        } else {
            let gx = (a.x + a.w * 0.5 + b.x + b.w * 0.5) * 0.5;
            let gy = (a.y + a.h + b.y) * 0.5;
            (v2(gx, gy), v2(14.0, 4.0))
        };
        out.push((SmartSelectionHandleKind::Gap { horiz, index: i }, center, size));
    }
    out
}

/// Hit-tests Smart Selection `Gap` bars and `Swap` rings in screen space.
pub fn hit_smart_selection_handle(
    handles: &[(SmartSelectionHandleKind, Vec2, Vec2)],
    to_screen: impl Fn(Vec2) -> Vec2,
    p: Vec2,
    hit_pad: f32,
) -> Option<SmartSelectionHandleKind> {
    // Check Gap bars first so tight gaps between small items are easy to grab, then Swap rings.
    for &(kind, center, size) in handles {
        if matches!(kind, SmartSelectionHandleKind::Gap { .. }) {
            let sc = to_screen(center);
            let hx = size.x * 0.5 + hit_pad;
            let hy = size.y * 0.5 + hit_pad;
            if (p.x - sc.x).abs() <= hx && (p.y - sc.y).abs() <= hy {
                return Some(kind);
            }
        }
    }
    for &(kind, center, size) in handles {
        if matches!(kind, SmartSelectionHandleKind::Swap(_)) {
            let sc = to_screen(center);
            let r = size.x.max(size.y) * 0.5 + hit_pad;
            if dist(p, sc) <= r {
                return Some(kind);
            }
        }
    }
    None
}

/// Minimum Euclidean distance from point `p` to line segment `a..=b`.
pub fn point_to_segment_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = sub(b, a);
    let len_sq = ab.x * ab.x + ab.y * ab.y;
    if len_sq <= 1e-6 {
        return dist(p, a);
    }
    let ap = sub(p, a);
    let t = ((ap.x * ab.x + ap.y * ab.y) / len_sq).clamp(0.0, 1.0);
    let proj = add(a, scale(ab, t));
    dist(p, proj)
}

/// Hit-tests handles (corner resize first, then midpoint/edge-segment resize when `allow_mid`,
/// then rotation zones just outside corners).
/// All values are in the same space as `corners` (screen pixels).
pub fn hit_handle(corners: &[Vec2; 4], p: Vec2, hit_radius: f32, rotate_zone: f32, allow_mid: bool) -> Option<HandleHit> {
    for h in [Handle::TL, Handle::TR, Handle::BR, Handle::BL] {
        if dist(handle_pos(corners, h), p) <= hit_radius {
            return Some(HandleHit::Resize(h));
        }
    }
    if allow_mid {
        for h in [Handle::T, Handle::R, Handle::B, Handle::L] {
            if dist(handle_pos(corners, h), p) <= hit_radius {
                return Some(HandleHit::Resize(h));
            }
        }
        let edges = [
            (corners[0], corners[1], Handle::T),
            (corners[1], corners[2], Handle::R),
            (corners[2], corners[3], Handle::B),
            (corners[3], corners[0], Handle::L),
        ];
        for (a, b, h) in edges {
            if point_to_segment_dist(p, a, b) <= hit_radius {
                return Some(HandleHit::Resize(h));
            }
        }
    }
    for (i, c) in corners.iter().enumerate() {
        let d = dist(*c, p);
        if d > hit_radius && d <= rotate_zone && !inside_quad(corners, p) {
            return Some(HandleHit::Rotate(i));
        }
    }
    None
}

/// Point-in-convex-quad test (corners in order).
pub fn inside_quad(c: &[Vec2; 4], p: Vec2) -> bool {
    let mut sign = 0.0f32;
    for i in 0..4 {
        let a = c[i];
        let b = c[(i + 1) % 4];
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cross.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if sign != cross.signum() {
            return false;
        }
    }
    true
}

/// Resize math in the node's local space.
///
/// `mouse_l` is the pointer in the *start-of-drag* local frame. Returns the new
/// size and the local offset `d` to apply to the node transform
/// (`T' = T0 * translate(d)`) so the anchor stays put in the parent space.
pub fn resize_local(size0: Vec2, h: Handle, mouse_l: Vec2, keep_ratio: bool, from_center: bool) -> (Vec2, Vec2) {
    let (hx, hy) = h.norm();
    let (w0, h0) = (size0.x.max(1e-3), size0.y.max(1e-3));
    let (ax, ay) = if from_center { (0.5, 0.5) } else { (if h.affects_x() { 1.0 - hx } else { 0.5 }, if h.affects_y() { 1.0 - hy } else { 0.5 }) };
    let anchor = v2(ax * w0, ay * h0);
    let mut nw = w0;
    let mut nh = h0;
    if h.affects_x() {
        nw = if hx == 1.0 { mouse_l.x - anchor.x } else { anchor.x - mouse_l.x };
        if from_center {
            nw = if hx == 1.0 { 2.0 * (mouse_l.x - anchor.x) } else { 2.0 * (anchor.x - mouse_l.x) };
        }
    }
    if h.affects_y() {
        nh = if hy == 1.0 { mouse_l.y - anchor.y } else { anchor.y - mouse_l.y };
        if from_center {
            nh = if hy == 1.0 { 2.0 * (mouse_l.y - anchor.y) } else { 2.0 * (anchor.y - mouse_l.y) };
        }
    }
    nw = nw.max(1.0);
    nh = nh.max(1.0);
    if keep_ratio {
        let ratio = w0 / h0;
        if h.affects_x() && h.affects_y() {
            let s = (nw / w0).max(nh / h0);
            nw = w0 * s;
            nh = h0 * s;
        } else if h.affects_x() {
            nh = nw / ratio;
        } else {
            nw = nh * ratio;
        }
    }
    let d = v2(ax * w0 - ax * nw, ay * h0 - ay * nh);
    (v2(nw, nh), d)
}

/// New parent-space transform after rotating about the node's center.
pub fn rotate_about_center(t0: &Affine, size: Vec2, angle: f32) -> Affine {
    let c = t0.apply(v2(size.x / 2.0, size.y / 2.0));
    Affine::translate(c.x, c.y) * Affine::rotate(angle) * Affine::translate(-size.x / 2.0, -size.y / 2.0)
}

pub fn angle_of(center: Vec2, p: Vec2) -> f32 {
    (p.y - center.y).atan2(p.x - center.x)
}

/// Snaps an angle (radians) to multiples of `step_deg`.
pub fn snap_angle(rad: f32, step_deg: f32) -> f32 {
    let step = step_deg.to_radians();
    (rad / step).round() * step
}

/// Computes interactive on-canvas Table / CSS Grid column and row track resize divider handles
/// (`(start_world, end_world, is_column_divider)`) for a Grid/Table frame (`LayoutDirection::Grid` / `NodeKind::Table`).
#[allow(dead_code)]
pub fn table_col_resize_handle(doc: &designos_doc::Document, id: designos_doc::NodeId) -> Vec<(egui::Pos2, egui::Pos2, bool)> {
    let Some(node) = doc.get(id) else {
        return Vec::new();
    };
    let (cols, rows, pad, col_gap, row_gap) = match node.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) {
        Some(al) if al.direction == LayoutDirection::Grid => {
            (al.grid_columns.max(1), al.grid_rows.max(1), al.padding, al.item_spacing.max(0.0), al.line_gap().max(0.0))
        }
        _ if node.name.to_ascii_lowercase().contains("table") => (2, 2, [0.0; 4], 0.0, 0.0),
        _ => return Vec::new(),
    };
    let [pt, pr, pb, pl] = pad;
    let inner_w = (node.size.x - pl - pr - col_gap * (cols.saturating_sub(1) as f32)).max(1.0);
    let inner_h = (node.size.y - pt - pb - row_gap * (rows.saturating_sub(1) as f32)).max(1.0);
    let col_w = inner_w / (cols as f32);
    let row_h = inner_h / (rows as f32);
    let world = doc.world_transform(id);
    let to_pos2 = |p: Vec2| {
        let wp = world.apply(p);
        egui::Pos2::new(wp.x, wp.y)
    };
    let mut out = Vec::new();
    for c in 1..cols {
        let x = pl + (c as f32) * col_w + ((c - 1) as f32 + 0.5) * col_gap;
        out.push((to_pos2(v2(x, pt)), to_pos2(v2(x, node.size.y - pb)), true));
    }
    for r in 1..rows {
        let y = pt + (r as f32) * row_h + ((r - 1) as f32 + 0.5) * row_gap;
        out.push((to_pos2(v2(pl, y)), to_pos2(v2(node.size.x - pr, y)), false));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_right_handle() {
        let (s, d) = resize_local(v2(100.0, 50.0), Handle::R, v2(150.0, 20.0), false, false);
        assert_eq!((s.x, s.y), (150.0, 50.0));
        assert_eq!((d.x, d.y), (0.0, 0.0));
    }

    #[test]
    fn resize_left_handle_moves_origin() {
        let (s, d) = resize_local(v2(100.0, 50.0), Handle::L, v2(-20.0, 20.0), false, false);
        assert_eq!(s.x, 120.0);
        assert_eq!(d.x, -20.0);
    }

    #[test]
    fn resize_keeps_ratio_on_corner() {
        let (s, _) = resize_local(v2(100.0, 50.0), Handle::BR, v2(200.0, 60.0), true, false);
        assert!((s.x / s.y - 2.0).abs() < 1e-4);
        assert_eq!(s.x, 200.0);
    }

    #[test]
    fn resize_from_center_is_symmetric() {
        let (s, d) = resize_local(v2(100.0, 100.0), Handle::BR, v2(150.0, 150.0), false, true);
        assert_eq!(s.x, 200.0);
        assert_eq!(d.x, -50.0);
    }

    #[test]
    fn resize_clamps_to_min() {
        let (s, _) = resize_local(v2(100.0, 50.0), Handle::R, v2(-400.0, 0.0), false, false);
        assert_eq!(s.x, 1.0);
    }

    #[test]
    fn rotation_preserves_center() {
        let t0 = Affine::translate(10.0, 20.0);
        let size = v2(40.0, 20.0);
        let t = rotate_about_center(&t0, size, 0.8);
        let c0 = t0.apply(v2(20.0, 10.0));
        let c1 = t.apply(v2(20.0, 10.0));
        assert!((c0.x - c1.x).abs() < 1e-4 && (c0.y - c1.y).abs() < 1e-4);
        assert!((t.rotation_rad() - 0.8).abs() < 1e-4);
    }

    #[test]
    fn snap_angle_15() {
        let a = snap_angle(16.0f32.to_radians(), 15.0);
        assert!((a - 15.0f32.to_radians()).abs() < 1e-5);
    }

    #[test]
    fn handle_hit_priorities() {
        let c = [v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 100.0), v2(0.0, 100.0)];
        assert_eq!(hit_handle(&c, v2(1.0, 1.0), 6.0, 20.0, true), Some(HandleHit::Resize(Handle::TL)));
        assert_eq!(hit_handle(&c, v2(50.0, 2.0), 6.0, 20.0, true), Some(HandleHit::Resize(Handle::T)));
        assert_eq!(hit_handle(&c, v2(110.0, 110.0), 6.0, 20.0, true), Some(HandleHit::Rotate(2)));
        assert_eq!(hit_handle(&c, v2(50.0, 50.0), 6.0, 20.0, true), None);
        assert_eq!(hit_handle(&c, v2(50.0, 2.0), 6.0, 20.0, false), None);
    }

    #[test]
    fn quad_contains() {
        let c = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 10.0)];
        assert!(inside_quad(&c, v2(5.0, 5.0)));
        assert!(!inside_quad(&c, v2(15.0, 5.0)));
    }

    #[test]
    fn corner_radius_handle_positions_and_hit_testing() {
        let c = [v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 80.0), v2(0.0, 80.0)];
        assert!(corner_radius_handle_positions(&c, [0.0; 4], 0.4).is_none(), "hidden below 0.5x zoom");
        let pts = corner_radius_handle_positions(&c, [16.0, 0.0, 40.0, 8.0], 1.0).unwrap();
        assert_eq!(pts[0], v2(16.0, 16.0));
        assert_eq!(pts[1], v2(90.0, 10.0)); // clamped to min 10px
        assert_eq!(pts[2], v2(72.0, 52.0)); // clamped to max 28px
        assert_eq!(pts[3], v2(10.0, 70.0));
        assert_eq!(hit_corner_radius_handle(&c, [16.0, 0.0, 40.0, 8.0], 1.0, v2(16.5, 15.5), 5.0), Some(0));
        assert_eq!(hit_corner_radius_handle(&c, [16.0, 0.0, 40.0, 8.0], 1.0, v2(50.0, 40.0), 5.0), None);
    }

    #[test]
    fn auto_layout_padding_and_gap_handles_calculation_and_hit() {
        let al = AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 16.0, padding: [12.0, 20.0, 12.0, 20.0], ..AutoLayout::default() };
        let kids = [Rect::new(20.0, 12.0, 60.0, 40.0), Rect::new(96.0, 12.0, 60.0, 40.0)];
        let hnds = auto_layout_handles(&al, v2(176.0, 64.0), &kids, 1.0);
        assert_eq!(hnds.len(), 5, "4 padding handles + 1 gap handle");
        assert_eq!(hnds[0].kind, AutoLayoutHandleKind::Padding(0));
        assert_eq!(hnds[0].local_center, v2(88.0, 6.0));
        assert_eq!(hnds[4].kind, AutoLayoutHandleKind::Gap(0));
        assert_eq!(hnds[4].local_center, v2(88.0, 32.0));
        assert_eq!(hit_auto_layout_handle(&hnds, |v| v, v2(88.0, 32.0), 3.0), Some(AutoLayoutHandleKind::Gap(0)));
        assert_eq!(hit_auto_layout_handle(&hnds, |v| v, v2(10.0, 32.0), 3.0), Some(AutoLayoutHandleKind::Padding(3)));
    }

    #[test]
    fn table_col_resize_handle_computes_grid_column_and_row_dividers() {
        let mut doc = designos_doc::Document::new("Table Test");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let mut f = designos_doc::Node::frame(tid, Rect::new(0.0, 0.0, 300.0, 200.0));
        if let Some(fp) = f.kind.frame_props_mut() {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Grid, grid_columns: 3, grid_rows: 2, ..AutoLayout::default() });
        }
        doc.insert(page, f).unwrap();
        let dividers = table_col_resize_handle(&doc, tid);
        // 2 internal column dividers + 1 internal row divider = 3 dividers
        assert_eq!(dividers.len(), 3);
        assert_eq!(dividers.iter().filter(|d| d.2).count(), 2);
        assert_eq!(dividers.iter().filter(|d| !d.2).count(), 1);
    }
}
