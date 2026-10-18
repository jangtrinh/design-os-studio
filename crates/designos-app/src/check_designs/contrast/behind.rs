//! Layers painted before a text layer (earlier siblings of it or of its ancestors) that may sit
//! under it. The nearest one that reaches the text decides: a plain opaque rectangle or frame that
//! covers the whole text box is its backdrop, and anything else paints something that is not
//! modelled, so the text is not checked.

use super::backdrop::{blends_as_group, inside_inset_box};
use super::extent::{box_corners, clipped, page_box, paint_seen, painted_box, stroke_under};
use super::subtree::{may_reach, SubtreeBounds};
use designos_doc::{resolved_node, Affine, BlendMode, Color, Document, Node, NodeId, NodeKind, PaintKind, Rect, TextProps};

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
    /// The text layer.
    text: NodeId,
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

    /// The solid fills (colour and alpha, bottom to top) that layer `id`, whose transform to the
    /// page is `world`, paints under the text, when it is a plain backdrop for all of it: a rectangle
    /// or frame with an opaque solid fill, opacity 1 and normal blending, no effect, axis
    /// aligned, with the whole text box inside it (and inside its rounded corners), no stroke band
    /// under the text and nothing painted over the fill where the text is.
    fn plain_cover(&mut self, id: NodeId, world: &Affine) -> Option<Vec<(Color, f32)>> {
        // Matrix products leave float noise; a layer turned by less is not turned.
        const AXIS: f32 = 1.0e-4;
        let doc = self.doc;
        let n = resolved_node(doc, id)?;
        let plain = matches!(n.kind, NodeKind::Rectangle | NodeKind::Frame(_)) && n.opacity >= 1.0 && !blends_as_group(n.blend);
        if !plain || n.effects.iter().any(|e| e.visible()) || world.b.abs() > AXIS || world.c.abs() > AXIS {
            return None;
        }
        // The text box in the layer's own space, so a turned text is exact too.
        let to_layer = world.invert()?;
        let text_world = doc.world_transform(self.text);
        let corners = box_corners(doc.get(self.text)?.size).map(|c| to_layer.apply(text_world.apply(c)));
        if !inside_inset_box(&corners, n.size, n.corner_radii) || stroke_under(&n, &corners) {
            return None;
        }
        let mut fills = Vec::new();
        for paint in n.fills.iter().filter(|p| p.visible) {
            match &paint.kind {
                PaintKind::Solid(c) if paint.blend == BlendMode::Normal => fills.push((*c, c.a * paint.opacity)),
                _ => return None,
            }
        }
        if !fills.iter().any(|(_, alpha)| *alpha >= 1.0) {
            return None;
        }
        // Children are painted over the fill, under the text.
        let clip = n.kind.frame_props().is_some_and(|f| f.clips_content).then(|| painted_box(world, &n, 0.0).own);
        let over = n.children.iter().any(|c| doc.get(*c).is_some_and(|child| self.paints_over(*c, *world * child.transform, clip, 0.0)));
        (!over).then_some(fills)
    }
}

/// What is painted behind a text layer.
#[derive(Debug, PartialEq)]
pub(in crate::check_designs) enum Behind {
    /// No layer painted before the text reaches it: its ancestors are its whole backdrop.
    Nothing,
    /// Something reaches it that is not modelled.
    Unknown,
    /// A plain opaque sibling of the text or of one of its ancestors covers it. `parent` is the
    /// layer it is a child of, and `fills` what it paints, bottom to top as colour and alpha.
    Cover { parent: NodeId, fills: Vec<(Color, f32)> },
}

/// Whether every layer from `id` up to the page is drawn at full opacity, so that what lies below
/// it is hidden by what it paints opaquely.
fn opaque_up_to_the_page(doc: &Document, id: NodeId) -> bool {
    let mut cur = Some(id);
    while let Some(c) = cur {
        let Some(n) = resolved_node(doc, c) else { return false };
        if !matches!(n.kind, NodeKind::Page { .. }) && n.opacity < 1.0 {
            return false;
        }
        cur = doc.get(c).and_then(|r| r.parent);
    }
    true
}

/// What the layers painted before `id` (earlier siblings of it or of any ancestor) put behind it,
/// by their own box, by what they paint beyond the box, or by anything in their subtree, scanned in
/// paint order from the nearest. The first layer that reaches the text decides: a plain opaque
/// cover (see `Probe::plain_cover`) is its backdrop and hides everything painted below it in the
/// same parent, and any other layer makes the backdrop unknown. Layers below the cover but outside
/// a parent drawn at less than full opacity still show through it, so the search goes on there.
///
/// `occluder` is an ancestor whose opaque fill covers the whole text box: what is painted below that
/// fill, such as the earlier siblings of the occluder and of its ancestors, is hidden by it and
/// ends the search. Earlier siblings of the text and of the layers between it and the occluder are
/// painted above the fill and still count.
///
/// `bounds` only speeds the search up: it never changes the answer, except that a subtree it rules
/// out no longer uses up the budget.
pub(in crate::check_designs) fn has_layer_behind(doc: &Document, id: NodeId, occluder: Option<NodeId>, bounds: Option<&SubtreeBounds>) -> Behind {
    let Some(text_box) = page_box(doc, id) else { return Behind::Unknown };
    let mut probe = Probe { doc, text: id, text_box, bounds, budget: SUBTREE_BUDGET };
    let mut cover = None;
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        if Some(cur) == occluder {
            break;
        }
        let Some(p) = doc.get(parent) else { return Behind::Unknown };
        let parent_world = doc.world_transform(parent);
        let before = p.children.iter().position(|s| *s == cur).unwrap_or(p.children.len());
        for (at, sibling) in p.children[..before].iter().enumerate().rev() {
            let Some(s) = doc.get(*sibling) else { continue };
            let world = parent_world * s.transform;
            if !s.visible || s.opacity <= 0.0 || !probe.can_reach(*sibling, world, 0.0) {
                continue;
            }
            // The painted box contains the layer's own box. A clipping sibling paints nothing of its
            // content outside its own box, which does not reach the text.
            let painted = painted_box(&world, s, 0.0);
            let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
            let reaches = painted.rect.intersects(&probe.text_box)
                || (!clips
                    && s.children.iter().any(|c| doc.get(*c).is_some_and(|child| probe.paints_over(*c, world * child.transform, None, painted.effects))));
            if !reaches {
                continue;
            }
            // A mask among the layers up to this one cuts it too.
            let masked = p.children[..=at].iter().any(|m| doc.get(*m).is_some_and(|m| m.is_mask));
            let fills = match probe.plain_cover(*sibling, &world) {
                Some(fills) if cover.is_none() && !masked => fills,
                _ => return Behind::Unknown,
            };
            let found = Behind::Cover { parent, fills };
            if opaque_up_to_the_page(doc, parent) {
                return found;
            }
            cover = Some(found);
            // Everything else in this parent is under the cover.
            break;
        }
        cur = parent;
    }
    cover.unwrap_or(Behind::Nothing)
}
