//! Contrast rule: WCAG 2.x AA for text against the backdrop its ancestors paint.
//!
//! Limits, all of which skip the layer instead of guessing: the text must have one visible solid
//! colour (no gradient, image, or per-run colours) painted source-over, and be drawn at all
//! (opacity above 0, here and on every ancestor); no layer from the text up to the page may use a
//! blend mode other than normal or pass-through (the renderer blends such a layer as a whole group,
//! so the colours inside it are not what reaches the screen); every ancestor fill under the text
//! must be solid with normal blending; the nearest visible layer painted before the text (a sibling
//! of the text or of one of its ancestors) that reaches it, by its own box, by what it paints beyond
//! the box (an outside stroke, a shadow, a blur) or by anything painted in its subtree (a frame that
//! does not clip draws its children wherever they sit, one that clips cuts them to its box), must be
//! a plain opaque rectangle or frame that covers the whole text box (see `behind`), which then is the
//! backdrop; and no stroke band of an ancestor may lie under it. What any other layer paints behind
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

mod backdrop;
mod behind;
mod extent;
mod subtree;

use super::{Finding, Fix, Rule};
use backdrop::{backdrop, blends_as_group, Backdrop};
pub(super) use behind::{has_layer_behind, Behind};
use designos_doc::{resolved_node, BlendMode, Color, Document, Node, NodeId, NodeKind, Paint, PaintKind};
pub(in crate::check_designs) use subtree::SubtreeBounds;

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

/// The text's single visible solid paint: its index and colour. `None` for anything mixed, and for a
/// paint the renderer does not draw source-over (the colour on screen is then not the paint's).
fn text_paint(n: &Node) -> Option<(usize, Color)> {
    let NodeKind::Text(t) = &n.kind else { return None };
    let visible = |ps: &[Paint]| -> Vec<(usize, Paint)> { ps.iter().cloned().enumerate().filter(|(_, p)| p.visible).collect() };
    let base = visible(&t.style.fills);
    if t.runs.iter().any(|r| visible(&r.style.fills) != base) {
        return None;
    }
    match base.as_slice() {
        [(i, Paint { kind: PaintKind::Solid(c), blend: BlendMode::Normal | BlendMode::PassThrough, .. })] => Some((*i, *c)),
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

fn failure(doc: &Document, id: NodeId, bounds: &SubtreeBounds) -> Option<Failure> {
    let n = resolved_node(doc, id)?;
    let NodeKind::Text(t) = &n.kind else { return None };
    // A layer with opacity 0 is not drawn, and neither is anything inside it.
    if t.content.trim().is_empty() || blends_as_group(n.blend) || n.opacity <= 0.0 {
        return None;
    }
    let (paint, color) = text_paint(&n)?;
    let alpha = color.a * t.style.fills[paint].opacity * n.opacity;
    if alpha <= 0.0 {
        return None;
    }
    let mut backdrop = backdrop(doc, id)?;
    // A plain opaque sibling that covers the text is the backdrop, so it is looked for before the
    // ratio is known: text that passes against its ancestors can fail against the sibling.
    match has_layer_behind(doc, id, backdrop.occluder, Some(bounds)) {
        Behind::Nothing => {}
        Behind::Unknown => return None,
        Behind::Cover { parent, fills } => {
            if !backdrop.covered_by(parent, fills) {
                return None;
            }
        }
    }
    let bg = backdrop.pixel(None);
    let ratio = contrast_ratio(backdrop.pixel(Some((color, alpha))), bg);
    let required = required_ratio(t.style.font_size, t.style.font_weight);
    (ratio < required).then_some(Failure { paint, color, alpha, backdrop, bg, ratio, required })
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
    let f = failure(doc, id, &SubtreeBounds::default())?;
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

/// Checks text layer `n`. `bounds` is shared by all the checks of one audit.
pub(super) fn check(doc: &Document, bounds: &SubtreeBounds, n: &Node, out: &mut Vec<Finding>) {
    if !matches!(n.kind, NodeKind::Text(_)) {
        return;
    }
    let Some(f) = failure(doc, n.id, bounds) else { return };
    if let Some(src_id) = n.component.source {
        let same_own = match (resolved_node(doc, n.id).as_deref(), resolved_node(doc, src_id).as_deref()) {
            (Some(a), Some(b)) => match (&a.kind, &b.kind) {
                (NodeKind::Text(ta), NodeKind::Text(tb)) => {
                    a.opacity == b.opacity
                        && a.blend == b.blend
                        && ta.style.fills == tb.style.fills
                        && ta.runs == tb.runs
                        && required_ratio(ta.style.font_size, ta.style.font_weight) == required_ratio(tb.style.font_size, tb.style.font_weight)
                }
                _ => false,
            },
            _ => false,
        };
        if same_own && failure(doc, src_id, bounds).is_some_and(|sf| sf.bg == f.bg) {
            return;
        }
    }
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
