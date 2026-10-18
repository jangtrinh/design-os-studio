//! Layers painted before a text layer (earlier siblings of it or of its ancestors) that may sit
//! under it. What they paint is not modelled, so text one of them reaches is not checked.

use super::extent::{clipped, page_box, paint_seen, painted_box};
use super::subtree::{may_reach, SubtreeBounds};
use designos_doc::{Affine, Document, Node, NodeId, NodeKind, Rect, TextProps};

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

/// One search for layers behind the text whose box is `text_box`.
struct Probe<'a> {
    doc: &'a Document,
    text_box: Rect,
    /// Where subtrees paint; without it every subtree is visited.
    bounds: Option<&'a SubtreeBounds>,
    /// Nodes still allowed to be visited.
    budget: usize,
}

impl Probe<'_> {
    /// Whether the subtree of `id` (transform `world`, under ancestors whose effects add `inherited`
    /// page units of growth) can reach the text at all.
    fn can_reach(&self, id: NodeId, world: Affine, inherited: f32) -> bool {
        self.bounds.is_none_or(|b| may_reach(b.of(self.doc, id, world), inherited, &self.text_box))
    }

    /// Whether `id` or a descendant paints where the text is, counting what each layer paints
    /// beyond its box: its stroke, shadows and blurs, and the effects of its ancestors
    /// (`inherited`), which act on all of their content. A frame that does not clip draws its
    /// children wherever they sit, so a small earlier sibling can still paint under the text; a
    /// frame that clips cuts its children to its own box.
    fn paints_over(&mut self, id: NodeId, world: Affine, clip: Option<Rect>, inherited: f32) -> bool {
        let doc = self.doc;
        let Some(n) = doc.get(id) else { return false };
        if !n.visible || n.opacity <= 0.0 || !self.can_reach(id, world, inherited) {
            return false;
        }
        if self.budget == 0 {
            return true;
        }
        self.budget -= 1;
        let painted = painted_box(&world, n, inherited);
        if paints_something(n) && clipped(painted.rect, clip).is_some_and(|b| b.intersects(&self.text_box)) {
            return true;
        }
        let below = if n.kind.frame_props().is_some_and(|f| f.clips_content) {
            match clipped(painted.own, clip) {
                Some(b) => Some(b),
                // Cut away entirely, with everything inside.
                None => return false,
            }
        } else {
            clip
        };
        let inherited = inherited + painted.effects;
        n.children.iter().any(|c| doc.get(*c).is_some_and(|child| self.paints_over(*c, world * child.transform, below, inherited)))
    }
}

/// Whether a visible layer painted before `id` (an earlier sibling of it or of any ancestor) covers
/// part of it, by its own box, by what it paints beyond the box, or by anything in its subtree.
/// Such a layer is behind the text and the ancestor chain does not describe it.
///
/// `occluder` is an ancestor whose opaque fill covers the whole text box: what is painted below that
/// fill, such as the earlier siblings of the occluder and of its ancestors, is hidden by it and
/// ends the search. Earlier siblings of the text and of the layers between it and the occluder are
/// painted above the fill and still count.
///
/// `bounds` only speeds the search up: it never changes the answer, except that a subtree it rules
/// out no longer uses up the budget.
pub(in crate::check_designs) fn has_layer_behind(doc: &Document, id: NodeId, occluder: Option<NodeId>, bounds: Option<&SubtreeBounds>) -> bool {
    let Some(text_box) = page_box(doc, id) else { return true };
    let mut probe = Probe { doc, text_box, bounds, budget: SUBTREE_BUDGET };
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        if Some(cur) == occluder {
            break;
        }
        let Some(p) = doc.get(parent) else { return true };
        let parent_world = doc.world_transform(parent);
        for sibling in p.children.iter().take_while(|s| **s != cur) {
            let Some(s) = doc.get(*sibling) else { continue };
            let world = parent_world * s.transform;
            if !s.visible || s.opacity <= 0.0 || !probe.can_reach(*sibling, world, 0.0) {
                continue;
            }
            // The painted box contains the layer's own box.
            let painted = painted_box(&world, s, 0.0);
            if painted.rect.intersects(&probe.text_box) {
                return true;
            }
            // A clipping sibling paints nothing of its content outside its own box, which does not
            // reach the text.
            let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
            if !clips && s.children.iter().any(|c| doc.get(*c).is_some_and(|child| probe.paints_over(*c, world * child.transform, None, painted.effects))) {
                return true;
            }
        }
        cur = parent;
    }
    false
}
