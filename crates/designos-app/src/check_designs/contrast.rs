//! Contrast rule: WCAG 2.x AA for text against the backdrop its ancestors paint.
//!
//! Limits, all of which skip the layer instead of guessing: the text must have one visible solid
//! colour (no gradient, image, or per-run colours); no layer from the text up to the page may use a
//! blend mode other than normal or pass-through (the renderer blends such a layer as a whole group,
//! so the colours inside it are not what reaches the screen); every ancestor fill under the text
//! must be solid with normal blending; and no visible layer painted before the text (a sibling of
//! the text or of one of its ancestors) may overlap it, because what that layer paints behind the
//! text is not modelled.
//!
//! An ancestor fill counts only where it is painted: the layer's own box, which nothing clips by
//! default, so a child can sit outside it. A fill that does not reach the text is not its backdrop,
//! and a fill that covers only part of the text leaves the backdrop unknown.
//!
//! Ancestors are composited the way the renderer does: a frame with opacity below 1 and children
//! is one isolated group, so its opacity scales everything inside it, the text included.

use super::{Finding, Fix, Rule};
use designos_doc::{resolved_node, BlendMode, Color, Document, Node, NodeId, NodeKind, Paint, PaintKind, Rect, Vec2};

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

/// Where the fill of a layer of `size` is relative to the text box `corners`, given in that
/// layer's own space (so a rotated or scaled ancestor is exact, not approximated by a page box).
fn reach(corners: &[Vec2; 4], size: Vec2) -> Reach {
    // Matrix products leave float noise; text flush with the edge is still on the fill.
    const EDGE: f32 = 0.01;
    if corners.iter().all(|c| c.x >= -EDGE && c.y >= -EDGE && c.x <= size.x + EDGE && c.y <= size.y + EDGE) {
        return Reach::Covers;
    }
    let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.x), hi.max(c.x)));
    let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.y), hi.max(c.y)));
    if x1 <= EDGE || y1 <= EDGE || x0 >= size.x - EDGE || y0 >= size.y - EDGE {
        Reach::Misses
    } else {
        Reach::Partial
    }
}

/// What the ancestors of `id` paint behind it. `None` when the backdrop is unknown: a visible
/// ancestor is blended as a group, or a fill under the text is not a plain solid or covers only part
/// of it.
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
        let here = corners;
        corners = here.map(|c| raw.transform.apply(c));
        if let NodeKind::Page { background } = &n.kind {
            out.page = Color { a: 1.0, ..*background };
            continue;
        }
        if !n.visible {
            continue;
        }
        if blends_as_group(n.blend) {
            return None;
        }
        let mut fills = Vec::new();
        if n.fills.iter().any(|p| p.visible) {
            match reach(&here, n.size) {
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
    let corners = box_corners(doc.get(id)?.size).map(|c| t.apply(c));
    let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.x), hi.max(c.x)));
    let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(c.y), hi.max(c.y)));
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Whether a visible layer painted before `id` (an earlier sibling of it or of any ancestor) covers
/// part of it. Such a layer is behind the text and the ancestor chain does not describe it.
fn has_layer_behind(doc: &Document, id: NodeId) -> bool {
    let Some(text_box) = page_box(doc, id) else { return true };
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        let Some(p) = doc.get(parent) else { return true };
        for sibling in p.children.iter().take_while(|s| **s != cur) {
            let Some(s) = doc.get(*sibling) else { continue };
            if s.visible && s.opacity > 0.0 && page_box(doc, *sibling).is_some_and(|b| b.intersects(&text_box)) {
                return true;
            }
        }
        cur = parent;
    }
    false
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
