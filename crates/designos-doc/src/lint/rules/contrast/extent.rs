//! Where a layer paints: its box in page space, the band its stroke covers along the edge, and how
//! far its stroke, shadows and blurs reach beyond the box. Reaches are over-estimates on purpose:
//! the rule skips text whose backdrop is unknown, so a reach that is too long only costs a finding.

use crate::{Affine, Document, Effect, Node, NodeId, NodeKind, Paint, PaintKind, Rect, StrokeAlign, StrokeJoin, Vec2};

/// The four corners of a box of `size` at the origin of its layer.
pub fn box_corners(size: Vec2) -> [Vec2; 4] {
    [Vec2::new(0.0, 0.0), Vec2::new(size.x, 0.0), Vec2::new(0.0, size.y), Vec2::new(size.x, size.y)]
}

/// The extent of `corners` as `(x0, x1, y0, y1)`.
pub fn bounds(corners: &[Vec2; 4]) -> (f32, f32, f32, f32) {
    let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.x), hi.max(c.x)));
    let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.y), hi.max(c.y)));
    (x0, x1, y0, y1)
}

/// The layer's bounds in page space (the box of its transformed corners).
pub fn page_box(doc: &Document, id: NodeId) -> Option<Rect> {
    let t = doc.world_transform(id);
    let (x0, x1, y0, y1) = bounds(&box_corners(doc.get(id)?.size).map(|c| t.apply(c)));
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Whether a paint draws anything: it is visible, has some opacity, and is not a fully transparent
/// solid.
pub fn paint_seen(p: &Paint) -> bool {
    p.visible && p.opacity > 0.0 && !matches!(&p.kind, PaintKind::Solid(c) if c.a <= 0.0)
}

/// The band a painted stroke of `n` covers along its layer's edge, as `(inset, reach)`: how far it
/// runs into the layer and how far beyond the edge. `None` when nothing is stroked.
fn stroke_band(n: &Node) -> Option<(f32, f32)> {
    let weight = n.stroke.side_weights.map_or(n.stroke.weight, |s| s.into_iter().fold(n.stroke.weight, f32::max));
    if weight <= 0.0 || !n.stroke.paints.iter().any(paint_seen) {
        return None;
    }
    Some(match n.stroke.align {
        StrokeAlign::Inside => (weight, 0.0),
        StrokeAlign::Center => (weight / 2.0, weight / 2.0),
        StrokeAlign::Outside => (0.0, weight),
    })
}

/// Whether a painted stroke of layer `n` lies under `corners`, the text box in the layer's own
/// space.
pub fn stroke_under(n: &Node, corners: &[Vec2; 4]) -> bool {
    let Some((inset, reach)) = stroke_band(n) else { return false };
    let (x0, x1, y0, y1) = bounds(corners);
    let reaches_band = x1 > -reach && y1 > -reach && x0 < n.size.x + reach && y0 < n.size.y + reach;
    let inside_band = x0 >= inset && y0 >= inset && x1 <= n.size.x - inset && y1 <= n.size.y - inset;
    reaches_band && !inside_band
}

/// How far the stroke of `n` is painted beyond its box, in the layer's units.
fn stroke_outset(n: &Node) -> f32 {
    let Some((_, mut reach)) = stroke_band(n) else { return 0.0 };
    if matches!(n.kind, NodeKind::Line | NodeKind::Vector { .. }) {
        let weight = n.stroke.side_weights.map_or(n.stroke.weight, |s| s.into_iter().fold(n.stroke.weight, f32::max));
        reach = reach.max(weight / 2.0);
    }
    if n.stroke.join == StrokeJoin::Miter
        && matches!(n.kind, NodeKind::Polygon { .. } | NodeKind::Star { .. } | NodeKind::Vector { .. } | NodeKind::BooleanOp(_))
    {
        reach *= n.stroke.miter_limit.max(1.0);
    }
    reach
}

/// How far the shadows and blurs of `n` reach beyond its box, in the layer's units.
fn effect_outset(n: &Node) -> f32 {
    n.effects.iter().fold(0.0, |m, e| {
        m.max(match e {
            Effect::DropShadow { offset, radius, spread, visible: true, .. } => {
                offset.x.abs().max(offset.y.abs()) + spread.max(0.0) + radius.max(0.0) * 1.6 + 2.0
            }
            Effect::LayerBlur { radius, visible: true } if *radius > 0.0 => radius * 1.6 + 2.0,
            Effect::Texture(t) if t.visible => t.radius.max(0.0) + 2.0,
            _ => 0.0,
        })
    })
}

/// `b` cut down to `clip`; `None` when nothing is left.
pub fn clipped(b: Rect, clip: Option<Rect>) -> Option<Rect> {
    let Some(c) = clip else { return Some(b) };
    let (x0, y0, x1, y1) = (b.x.max(c.x), b.y.max(c.y), (b.x + b.w).min(c.x + c.w), (b.y + b.h).min(c.y + c.h));
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// `r` grown by `by` on every side.
pub fn grown(r: &Rect, by: f32) -> Rect {
    Rect::new(r.x - by, r.y - by, r.w + 2.0 * by, r.h + 2.0 * by)
}

/// A layer's box in page space, and that box grown by everything the layer paints beyond it.
pub struct PaintedBox {
    /// The layer's own box.
    pub own: Rect,
    /// The own box grown by the layer's stroke and effects and by the `inherited` growth.
    pub rect: Rect,
    /// The growth, in page units, that this layer's own effects add to everything inside it.
    pub effects: f32,
}

/// The painted box of layer `n`, whose transform to the page is `world`.
pub fn painted_box(world: &Affine, n: &Node, inherited: f32) -> PaintedBox {
    let (x0, x1, y0, y1) = bounds(&box_corners(n.size).map(|c| world.apply(c)));
    let scale = world.a.hypot(world.b).max(world.c.hypot(world.d));
    let effects = effect_outset(n) * scale;
    let own = Rect::new(x0, y0, x1 - x0, y1 - y0);
    PaintedBox { rect: grown(&own, stroke_outset(n) * scale + effects + inherited), own, effects }
}
