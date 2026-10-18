//! Layers painted before a text layer (earlier siblings of it or of its ancestors) that may sit
//! under it. What they paint is not modelled, so text one of them reaches is not checked.

use super::extent::{page_box, paint_seen, painted_box};
use designos_doc::{Document, Node, NodeId, NodeKind, Rect, TextProps};

/// Nodes one text check may look at in earlier siblings' subtrees. Past it the backdrop is unknown.
const SUBTREE_BUDGET: usize = 4096;

/// Whether the runs of `t` cover every byte of its content.
fn runs_cover_all(t: &TextProps) -> bool {
    let mut spans: Vec<(usize, usize)> = t.runs.iter().map(|r| (r.start, r.end)).collect();
    spans.sort_unstable();
    let mut end = 0;
    for (start, stop) in spans {
        if start > end {
            return false;
        }
        end = end.max(stop);
    }
    end >= t.content.len()
}

/// Whether the glyphs of `t` are drawn with some colour. The renderer paints each glyph with the
/// fills of the run that covers it, and with the base fills where no run does.
fn text_paints(t: &TextProps) -> bool {
    if t.content.trim().is_empty() {
        return false;
    }
    t.runs.iter().any(|r| r.style.fills.iter().any(paint_seen)) || (!runs_cover_all(t) && t.style.fills.iter().any(paint_seen))
}

/// Whether `n` paints anything itself: a visible fill or stroke with some alpha, visible text in
/// the base style or in a run, or a visible effect (which may reach beyond the box).
fn paints_something(n: &Node) -> bool {
    let stroked = (n.stroke.weight > 0.0 || n.stroke.side_weights.is_some()) && n.stroke.paints.iter().any(paint_seen);
    let text = matches!(&n.kind, NodeKind::Text(t) if text_paints(t));
    n.fills.iter().any(paint_seen) || stroked || n.effects.iter().any(|e| e.visible()) || text
}

/// `b` cut down to `clip`; `None` when nothing is left.
fn clipped(b: Rect, clip: Option<Rect>) -> Option<Rect> {
    let Some(c) = clip else { return Some(b) };
    let (x0, y0, x1, y1) = (b.x.max(c.x), b.y.max(c.y), (b.x + b.w).min(c.x + c.w), (b.y + b.h).min(c.y + c.h));
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Whether `id` or a descendant paints where `text_box` is, counting what each layer paints beyond
/// its box: its stroke, shadows and blurs, and the effects of its ancestors (`inherited`, in page
/// units), which act on all of their content. A frame that does not clip draws its children
/// wherever they sit, so a small earlier sibling can still paint under the text; a frame that
/// clips cuts its children to its own box.
fn subtree_paints_over(doc: &Document, id: NodeId, text_box: &Rect, clip: Option<Rect>, inherited: f32, budget: &mut usize) -> bool {
    let Some(n) = doc.get(id) else { return false };
    if !n.visible || n.opacity <= 0.0 {
        return false;
    }
    if *budget == 0 {
        return true;
    }
    *budget -= 1;
    let Some(own) = page_box(doc, id) else { return true };
    let painted = painted_box(doc, id, n, inherited);
    if paints_something(n) && clipped(painted.rect, clip).is_some_and(|b| b.intersects(text_box)) {
        return true;
    }
    let below = if n.kind.frame_props().is_some_and(|f| f.clips_content) {
        match clipped(own, clip) {
            Some(b) => Some(b),
            // Cut away entirely, with everything inside.
            None => return false,
        }
    } else {
        clip
    };
    n.children.iter().any(|c| subtree_paints_over(doc, *c, text_box, below, inherited + painted.effects, budget))
}

/// Whether a visible layer painted before `id` (an earlier sibling of it or of any ancestor) covers
/// part of it, by its own box, by what it paints beyond the box, or by anything in its subtree.
/// Such a layer is behind the text and the ancestor chain does not describe it.
pub(super) fn has_layer_behind(doc: &Document, id: NodeId) -> bool {
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
            // The painted box contains the layer's own box.
            let painted = painted_box(doc, *sibling, s, 0.0);
            if painted.rect.intersects(&text_box) {
                return true;
            }
            // A clipping sibling paints nothing of its content outside its own box, which does not
            // reach the text.
            let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
            if !clips && s.children.iter().any(|c| subtree_paints_over(doc, *c, &text_box, None, painted.effects, &mut budget)) {
                return true;
            }
        }
        cur = parent;
    }
    false
}
