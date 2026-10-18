//! What the ancestors of a text layer paint behind it, composited the way the renderer does.

use super::extent::{bounds, box_corners, stroke_under};
use designos_doc::{resolved_node, BlendMode, Color, Document, NodeId, NodeKind, PaintKind, Vec2};

/// One ancestor: what it paints itself (solid fills, bottom to top, as colour and alpha) and the
/// opacity its whole subtree is composited with.
struct Layer {
    owner: NodeId,
    fills: Vec<(Color, f32)>,
    opacity: f32,
}

/// The ancestors of a text layer, innermost first, over the page colour.
pub(super) struct Backdrop {
    layers: Vec<Layer>,
    page: Color,
    /// The page the text is on, when it has one.
    page_node: Option<NodeId>,
    /// The innermost ancestor whose fill is opaque and covers the whole text box. Everything
    /// painted below that fill (its earlier siblings and what they paint, the fills and strokes of
    /// the ancestors above it) cannot show through it.
    pub(super) occluder: Option<NodeId>,
}

/// Premultiplied RGBA.
#[derive(Clone, Copy)]
struct Px {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Px {
    const CLEAR: Px = Px { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    fn of(c: Color, alpha: f32) -> Px {
        let a = alpha.clamp(0.0, 1.0);
        Px { r: c.r * a, g: c.g * a, b: c.b * a, a }
    }

    /// `self` painted over `below`.
    fn over(self, below: Px) -> Px {
        let k = 1.0 - self.a;
        Px { r: self.r + below.r * k, g: self.g + below.g * k, b: self.b + below.b * k, a: self.a + below.a * k }
    }

    fn scaled(self, k: f32) -> Px {
        Px { r: self.r * k, g: self.g * k, b: self.b * k, a: self.a * k }
    }
}

impl Backdrop {
    /// Paints `fills` (colour and alpha, bottom to top) on top of what `parent` paints itself and
    /// under its children, the text included: the backdrop of text that a plain opaque sibling
    /// covers. `false` when `parent` is neither a layer of this backdrop nor its page.
    pub(super) fn covered_by(&mut self, parent: NodeId, fills: Vec<(Color, f32)>) -> bool {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.owner == parent) {
            layer.fills.extend(fills);
        } else if self.page_node == Some(parent) {
            // The page is below every layer.
            self.layers.push(Layer { owner: parent, fills, opacity: 1.0 });
        } else {
            return false;
        }
        true
    }

    /// The colour on screen where `text` (colour and alpha) is drawn over the innermost ancestor, or
    /// the plain backdrop for `None`.
    pub(super) fn pixel(&self, text: Option<(Color, f32)>) -> Color {
        let mut acc = text.map_or(Px::CLEAR, |(c, a)| Px::of(c, a));
        for layer in &self.layers {
            let own = layer.fills.iter().fold(Px::CLEAR, |below, (c, a)| Px::of(*c, *a).over(below));
            acc = acc.over(own).scaled(layer.opacity.clamp(0.0, 1.0));
        }
        let shown = acc.over(Px::of(self.page, 1.0));
        Color { r: shown.r, g: shown.g, b: shown.b, a: 1.0 }
    }
}

/// Whether the renderer blends this layer as a whole group instead of painting it normally.
pub(super) fn blends_as_group(mode: BlendMode) -> bool {
    !matches!(mode, BlendMode::Normal | BlendMode::PassThrough)
}

/// How far a layer's fill reaches under the text.
enum Reach {
    /// The whole text box is over the fill.
    Covers,
    /// No part of the text box is over the fill.
    Misses,
    /// The fill is behind only part of the text, so the backdrop changes under it.
    Partial,
}

/// Where the fill of a layer of `size` is relative to the text box `corners`, given in that
/// layer's own space (so a rotated or scaled ancestor is exact, not approximated by a page box).
fn reach(corners: &[Vec2; 4], size: Vec2) -> Reach {
    // Matrix products leave float noise; text flush with the edge is still on the fill.
    const EDGE: f32 = 0.01;
    if corners.iter().all(|c| c.x >= -EDGE && c.y >= -EDGE && c.x <= size.x + EDGE && c.y <= size.y + EDGE) {
        return Reach::Covers;
    }
    let (x0, x1, y0, y1) = bounds(corners);
    if x1 <= EDGE || y1 <= EDGE || x0 >= size.x - EDGE || y0 >= size.y - EDGE {
        Reach::Misses
    } else {
        Reach::Partial
    }
}

/// The box of `corners` cut down to the layer's own rectangle: where text that a clipping layer
/// cuts off is still painted.
fn inside_layer(corners: &[Vec2; 4], size: Vec2) -> [Vec2; 4] {
    let (x0, x1, y0, y1) = bounds(corners);
    let (x0, x1, y0, y1) = (x0.max(0.0), x1.min(size.x), y0.max(0.0), y1.min(size.y));
    [Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x0, y1), Vec2::new(x1, y1)]
}

/// Whether `corners` lies inside the box of `size` inset by its corner radius `radii`, where the
/// fill is not cut away at the rounded corners.
pub(super) fn inside_inset_box(corners: &[Vec2; 4], size: Vec2, radii: [f32; 4]) -> bool {
    const EDGE: f32 = 0.01;
    let r = radii.into_iter().fold(0.0_f32, f32::max).min(size.x / 2.0).min(size.y / 2.0);
    corners.iter().all(|c| c.x >= r - EDGE && c.y >= r - EDGE && c.x <= size.x - r + EDGE && c.y <= size.y - r + EDGE)
}

/// What the ancestors of `id` paint behind it. `None` when the text is not checkable: a visible
/// ancestor is blended as a group or is not drawn at all (opacity 0), a clipping ancestor cuts the
/// text away, a stroke band is under it, or a fill under the text is not a plain solid or covers
/// only part of it.
pub(super) fn backdrop(doc: &Document, id: NodeId) -> Option<Backdrop> {
    let text = doc.get(id)?;
    // The text box in the space of the layer being looked at; it starts in the parent's space.
    let mut corners = box_corners(text.size).map(|c| text.transform.apply(c));
    let mut out = Backdrop { layers: Vec::new(), page: Color::WHITE, page_node: None, occluder: None };
    // Whether every layer between the text and the one being looked at is drawn at full opacity.
    let mut opaque_chain = true;
    let mut cur = text.parent;
    while let Some(p) = cur {
        let raw = doc.get(p)?;
        let n = resolved_node(doc, p)?;
        cur = raw.parent;
        let mut here = corners;
        let up = |c: &[Vec2; 4]| c.map(|v| raw.transform.apply(v));
        if let NodeKind::Page { background } = &n.kind {
            out.page = Color { a: 1.0, ..*background };
            out.page_node = Some(p);
            continue;
        }
        if !n.visible {
            corners = up(&here);
            continue;
        }
        if blends_as_group(n.blend) || n.opacity <= 0.0 {
            return None;
        }
        opaque_chain &= n.opacity >= 1.0;
        let mut how = reach(&here, n.size);
        // Whether the whole text box is within this layer's box, whatever the layer clips.
        let whole = matches!(how, Reach::Covers);
        if raw.kind.frame_props().is_some_and(|f| f.clips_content) {
            match how {
                // Cut away by this layer: nothing of the text is painted.
                Reach::Misses => return None,
                Reach::Partial => here = inside_layer(&here, n.size),
                Reach::Covers => {}
            }
            how = Reach::Covers;
        }
        corners = up(&here);
        // The renderer paints a layer's stroke over its fill and under its children: a stroke band
        // under the text is a backdrop this chain does not describe, unless an opaque fill below
        // the text already hides the layer's stroke.
        if out.occluder.is_none() && stroke_under(&n, &here) {
            return None;
        }
        let mut fills = Vec::new();
        if n.fills.iter().any(|p| p.visible) {
            match how {
                // Its opacity still scales the text; it just paints nothing under it.
                Reach::Misses => {}
                Reach::Partial => return None,
                Reach::Covers => {
                    for paint in n.fills.iter().filter(|p| p.visible) {
                        match &paint.kind {
                            PaintKind::Solid(c) if paint.blend == BlendMode::Normal => fills.push((*c, c.a * paint.opacity)),
                            _ => return None,
                        }
                    }
                }
            }
        }
        let no_effects = !n.effects.iter().any(|e| e.visible());
        if out.occluder.is_none()
            && opaque_chain
            && whole
            && no_effects
            && inside_inset_box(&here, n.size, n.corner_radii)
            && fills.iter().any(|(_, alpha)| *alpha >= 1.0)
        {
            out.occluder = Some(p);
        }
        out.layers.push(Layer { owner: p, fills, opacity: n.opacity });
    }
    Some(out)
}
