//! Contrast rule: WCAG 2.x AA for text against the backdrop its ancestors paint.
//!
//! Limits, all of which skip the layer instead of guessing: the text must have one visible solid
//! colour (no gradient, image, or per-run colours); no layer from the text up to the page may use a
//! blend mode other than normal or pass-through (the renderer blends such a layer as a whole group,
//! so the colours inside it are not what reaches the screen); every ancestor fill under the text
//! must be solid with normal blending; no visible layer painted before the text (a sibling of the
//! text or of one of its ancestors) may overlap it, by its own box or by anything painted in its
//! subtree (a frame that does not clip draws its children wherever they sit, one that clips cuts
//! them to its box); and no stroke band of an ancestor may lie under it. What those paint behind
//! the text is not modelled, so such text is neither reported nor fixed.
//!
//! An ancestor fill counts only where it is painted: the layer's own box. A frame that does not
//! clip lets a child sit outside it, so a fill that does not reach the text is not its backdrop and
//! one that covers only part of the text leaves the backdrop unknown. A frame that clips paints
//! only the part of the text inside it: text wholly outside is not painted at all, and for text
//! half outside, only the part inside is looked at.
//!
//! Ancestors are composited the way the renderer does: a frame with opacity below 1 and children
//! is one isolated group, so its opacity scales everything inside it, the text included.

use super::{Finding, Fix, Rule};
use designos_doc::{resolved_node, BlendMode, Color, Document, Node, NodeId, NodeKind, Paint, PaintKind, Rect, StrokeAlign, Vec2};

fn linear(v: f32) -> f64 {
    let v = f64::from(v.clamp(0.0, 1.0));
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of an opaque sRGB colour.
fn luminance(c: Color) -> f64 {
    0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
}

/// WCAG contrast ratio (1..=21) between two opaque colours; the order does not matter.
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// AA ratio for text: 3:1 for large text (24 px, or 18.66 px and bold from weight 700), else 4.5:1.
pub fn required_ratio(font_size: f32, weight: u16) -> f64 {
    if font_size >= 24.0 || (font_size >= 18.66 && weight >= 700) {
        3.0
    } else {
        4.5
    }
}

/// One ancestor: what it paints itself (solid fills, bottom to top, as colour and alpha) and the
/// opacity its whole subtree is composited with.
struct Layer {
    fills: Vec<(Color, f32)>,
    opacity: f32,
}

/// The ancestors of a text layer, innermost first, over the page colour.
struct Backdrop {
    layers: Vec<Layer>,
    page: Color,
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
    /// The colour on screen where `text` (colour and alpha) is drawn over the innermost ancestor, or
    /// the plain backdrop for `None`.
    fn pixel(&self, text: Option<(Color, f32)>) -> Color {
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
fn blends_as_group(mode: BlendMode) -> bool {
    !matches!(mode, BlendMode::Normal | BlendMode::PassThrough)
}

/// The four corners of a box of `size` at the origin of its layer.
fn box_corners(size: Vec2) -> [Vec2; 4] {
    [Vec2::new(0.0, 0.0), Vec2::new(size.x, 0.0), Vec2::new(0.0, size.y), Vec2::new(size.x, size.y)]
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

/// The extent of `corners` as `(x0, x1, y0, y1)`.
fn bounds(corners: &[Vec2; 4]) -> (f32, f32, f32, f32) {
    let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.x), hi.max(c.x)));
    let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.y), hi.max(c.y)));
    (x0, x1, y0, y1)
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

/// What the ancestors of `id` paint behind it. `None` when the text is not checkable: a visible
/// ancestor is blended as a group, a clipping ancestor cuts the text away, or a fill under the text
/// is not a plain solid or covers only part of it.
fn backdrop(doc: &Document, id: NodeId) -> Option<Backdrop> {
    let text = doc.get(id)?;
    // The text box in the space of the layer being looked at; it starts in the parent's space.
    let mut corners = box_corners(text.size).map(|c| text.transform.apply(c));
    let mut out = Backdrop { layers: Vec::new(), page: Color::WHITE };
    let mut cur = text.parent;
    while let Some(p) = cur {
        let raw = doc.get(p)?;
        let n = resolved_node(doc, p)?;
        cur = raw.parent;
        let mut here = corners;
        let up = |c: &[Vec2; 4]| c.map(|v| raw.transform.apply(v));
        if let NodeKind::Page { background } = &n.kind {
            out.page = Color { a: 1.0, ..*background };
            continue;
        }
        if !n.visible {
            corners = up(&here);
            continue;
        }
        if blends_as_group(n.blend) {
            return None;
        }
        let mut how = reach(&here, n.size);
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
        // under the text is a backdrop this chain does not describe.
        if stroke_under(&n, &here) {
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
        out.layers.push(Layer { fills, opacity: n.opacity });
    }
    Some(out)
}

/// The layer's bounds in page space (the box of its transformed corners).
fn page_box(doc: &Document, id: NodeId) -> Option<Rect> {
    let t = doc.world_transform(id);
    let (x0, x1, y0, y1) = bounds(&box_corners(doc.get(id)?.size).map(|c| t.apply(c)));
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Nodes one text check may look at in earlier siblings' subtrees. Past it the backdrop is unknown.
const SUBTREE_BUDGET: usize = 4096;

/// Whether `n` paints anything itself: a visible fill or stroke with some alpha, visible text, or a
/// visible effect (which may reach beyond the box).
fn paints_something(n: &Node) -> bool {
    let seen = |p: &Paint| p.visible && p.opacity > 0.0 && !matches!(&p.kind, PaintKind::Solid(c) if c.a <= 0.0);
    let text = matches!(&n.kind, NodeKind::Text(t) if !t.content.trim().is_empty() && t.style.fills.iter().any(seen));
    n.fills.iter().any(seen) || (n.stroke.weight > 0.0 && n.stroke.paints.iter().any(seen)) || n.effects.iter().any(|e| e.visible()) || text
}

/// `b` cut down to `clip`; `None` when nothing is left.
fn clipped(b: Rect, clip: Option<Rect>) -> Option<Rect> {
    let Some(c) = clip else { return Some(b) };
    let (x0, y0, x1, y1) = (b.x.max(c.x), b.y.max(c.y), (b.x + b.w).min(c.x + c.w), (b.y + b.h).min(c.y + c.h));
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Whether `id` or a descendant paints where `text_box` is. A frame that does not clip draws its
/// children wherever they sit, so a small earlier sibling can still paint under the text; a frame
/// that clips cuts its children to its own box.
fn subtree_paints_over(doc: &Document, id: NodeId, text_box: &Rect, clip: Option<Rect>, budget: &mut usize) -> bool {
    let Some(n) = doc.get(id) else { return false };
    if !n.visible || n.opacity <= 0.0 {
        return false;
    }
    if *budget == 0 {
        return true;
    }
    *budget -= 1;
    let Some(own) = page_box(doc, id) else { return true };
    let Some(shown) = clipped(own, clip) else { return false };
    if paints_something(n) && shown.intersects(text_box) {
        return true;
    }
    let clips = n.kind.frame_props().is_some_and(|f| f.clips_content);
    let below = if clips { Some(shown) } else { clip };
    n.children.iter().any(|c| subtree_paints_over(doc, *c, text_box, below, budget))
}

/// Whether a visible layer painted before `id` (an earlier sibling of it or of any ancestor) covers
/// part of it, by its own box or by anything in its subtree. Such a layer is behind the text and the
/// ancestor chain does not describe it.
fn has_layer_behind(doc: &Document, id: NodeId) -> bool {
    let Some(text_box) = page_box(doc, id) else { return true };
    let mut budget = SUBTREE_BUDGET;
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        let Some(p) = doc.get(parent) else { return true };
        for sibling in p.children.iter().take_while(|s| **s != cur) {
            let Some(s) = doc.get(*sibling) else { continue };
            if !s.visible || s.opacity <= 0.0 {
                continue;
            }
            if page_box(doc, *sibling).is_some_and(|b| b.intersects(&text_box)) {
                return true;
            }
            // A clipping sibling paints nothing outside its own box, which does not reach the text.
            let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
            if !clips && s.children.iter().any(|c| subtree_paints_over(doc, *c, &text_box, None, &mut budget)) {
                return true;
            }
        }
        cur = parent;
    }
    false
}

/// Whether a painted stroke of layer `n` lies under `corners`, the text box in the layer's own
/// space. The stroke is a band along the layer's edge: inside, centred on it, or outside it. Text
/// is under the band when it reaches the band's outer edge without lying wholly within its inner
/// edge, so text nowhere near the stroke is not.
fn stroke_under(n: &Node, corners: &[Vec2; 4]) -> bool {
    let painted = n.stroke.paints.iter().any(|p| p.visible && p.opacity > 0.0 && !matches!(&p.kind, PaintKind::Solid(c) if c.a <= 0.0));
    if n.stroke.weight <= 0.0 || !painted {
        return false;
    }
    let weight = n.stroke.side_weights.map_or(n.stroke.weight, |s| s.into_iter().fold(0.0, f32::max));
    // How far the band reaches into the layer and beyond its edge.
    let (inset, reach) = match n.stroke.align {
        StrokeAlign::Inside => (weight, 0.0),
        StrokeAlign::Center => (weight / 2.0, weight / 2.0),
        StrokeAlign::Outside => (0.0, weight),
    };
    let (x0, x1, y0, y1) = bounds(corners);
    let reaches_band = x1 > -reach && y1 > -reach && x0 < n.size.x + reach && y0 < n.size.y + reach;
    let inside_band = x0 >= inset && y0 >= inset && x1 <= n.size.x - inset && y1 <= n.size.y - inset;
    reaches_band && !inside_band
}

/// The text's single visible solid paint: its index and colour. `None` for anything mixed.
fn text_paint(n: &Node) -> Option<(usize, Color)> {
    let NodeKind::Text(t) = &n.kind else { return None };
    let visible = |ps: &[Paint]| -> Vec<(usize, Paint)> { ps.iter().cloned().enumerate().filter(|(_, p)| p.visible).collect() };
    let base = visible(&t.style.fills);
    if t.runs.iter().any(|r| visible(&r.style.fills) != base) {
        return None;
    }
    match base.as_slice() {
        [(i, Paint { kind: PaintKind::Solid(c), .. })] => Some((*i, *c)),
        _ => None,
    }
}

/// Everything the rule needs about a text layer that currently fails AA.
struct Failure {
    paint: usize,
    color: Color,
    /// The text's own alpha: colour alpha x paint opacity x layer opacity.
    alpha: f32,
    backdrop: Backdrop,
    /// The plain backdrop colour on screen.
    bg: Color,
    ratio: f64,
    required: f64,
}

impl Failure {
    /// The contrast the text would have if it were painted in `color`.
    fn ratio_with(&self, color: Color) -> f64 {
        contrast_ratio(self.backdrop.pixel(Some((color, self.alpha))), self.bg)
    }
}

fn failure(doc: &Document, id: NodeId) -> Option<Failure> {
    let n = resolved_node(doc, id)?;
    let NodeKind::Text(t) = &n.kind else { return None };
    if t.content.trim().is_empty() || blends_as_group(n.blend) {
        return None;
    }
    let (paint, color) = text_paint(&n)?;
    let backdrop = backdrop(doc, id)?;
    let alpha = color.a * t.style.fills[paint].opacity * n.opacity;
    let bg = backdrop.pixel(None);
    let ratio = contrast_ratio(backdrop.pixel(Some((color, alpha))), bg);
    let required = required_ratio(t.style.font_size, t.style.font_weight);
    // The layer-behind scan only runs for text that already fails, so passing text stays cheap.
    (ratio < required && !has_layer_behind(doc, id)).then_some(Failure { paint, color, alpha, backdrop, bg, ratio, required })
}

/// The mildest recolouring toward black or white that reaches `required`, quantised to 8 bits.
fn suggest(f: &Failure) -> Option<Color> {
    let mut targets = [Color::BLACK, Color::WHITE];
    targets.sort_by(|a, b| contrast_ratio(*b, f.bg).total_cmp(&contrast_ratio(*a, f.bg)));
    let q = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    targets.iter().find_map(|target| {
        (0..=255u16).find_map(|step| {
            let t = f32::from(step) / 255.0;
            let mix = |a: f32, b: f32| a + (b - a) * t;
            let c = Color { a: f.color.a, ..Color::rgb(q(mix(f.color.r, target.r)), q(mix(f.color.g, target.g)), q(mix(f.color.b, target.b))) };
            (f.ratio_with(c) >= f.required).then_some(c)
        })
    })
}

/// Paint index and replacement colour for `id` as it is right now, or `None` when it passes, is
/// not checkable, or no recolouring can reach AA. Fixes use this rather than a stored suggestion.
pub(super) fn fresh_fix(doc: &Document, id: NodeId) -> Option<(usize, Color)> {
    let f = failure(doc, id)?;
    if owner(doc, doc.get(id)?, f.paint).is_some() {
        return None;
    }
    suggest(&f).map(|c| (f.paint, c))
}

/// What owns the colour of text paint `paint`: a variable or a style, which a local edit would fight.
fn owner(doc: &Document, n: &Node, paint: usize) -> Option<String> {
    let bound = n.variables.bindings.get(&format!("fills/{paint}")).and_then(|v| doc.variables.get(v)).map(|v| format!("variable {}", v.name));
    bound.or_else(|| n.style_refs.get("fill").map(|k| format!("style {}", doc.style_name(k))))
}

fn floor2(v: f64) -> f64 {
    (v * 100.0).floor() / 100.0
}

fn ratio_label(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v}")
    }
}

pub(super) fn check(doc: &Document, n: &Node, out: &mut Vec<Finding>) {
    if !matches!(n.kind, NodeKind::Text(_)) {
        return;
    }
    let Some(f) = failure(doc, n.id) else { return };
    let mut message = format!("Contrast {:.2}:1 is below {}:1 (WCAG AA)", floor2(f.ratio), ratio_label(f.required));
    let (fix, fix_label) = match (owner(doc, n, f.paint), suggest(&f)) {
        (Some(owner), _) => {
            message.push_str(&format!(" · colour comes from {owner}"));
            (None, String::new())
        }
        (None, Some(color)) => {
            let new_ratio = f.ratio_with(color);
            (Some(Fix::SetTextColor { color }), format!("Change text to #{} ({:.2}:1)", color.to_hex(), floor2(new_ratio)))
        }
        (None, None) => (None, String::new()),
    };
    out.push(Finding { node: n.id, node_name: n.name.clone(), rule: Rule::Contrast, message, fix, fix_label });
}
