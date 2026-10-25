//! Layers painted before a text layer (earlier siblings of it or of its ancestors) that may sit
//! under it. The nearest one that reaches the text decides: a plain opaque rectangle or frame that
//! covers the whole text box is its backdrop, and anything else paints something that is not
//! modelled, so the text is not checked.

use super::backdrop::{blends_as_group, inside_inset_box};
use super::children::ChildIndex;
use super::extent::{box_corners, clipped, page_box, paint_seen, painted_box, stroke_under};
use super::subtree::{may_reach, SubtreeBounds};
use crate::{resolved_node, Affine, BlendMode, Color, Document, Node, NodeId, NodeKind, PaintKind, Rect, TextProps, Vec2};
use std::sync::Arc;

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

/// The children of a layer, with the index over them when the subtree bounds are known.
struct Kids<'a> {
    layer: &'a Node,
    index: Option<Arc<ChildIndex>>,
}

impl Kids<'_> {
    /// Position of the first occurrence of `id` among the children.
    fn position_of(&self, id: NodeId) -> Option<usize> {
        match &self.index {
            Some(index) => index.position_of(id),
            None => self.layer.children.iter().position(|s| *s == id),
        }
    }

    /// Positions, ascending, of the children that can reach `text_box`; all of them without bounds.
    fn reaching(&self, text_box: &Rect, out: &mut Vec<usize>) {
        match &self.index {
            Some(index) => index.reaching(text_box, out),
            None => {
                out.clear();
                out.extend(0..self.layer.children.len());
            }
        }
    }

    /// Like [`Kids::reaching`], for a test on the resolved layers around a point box.
    fn near_point(&self, sample: &Rect, out: &mut Vec<usize>) {
        match &self.index {
            Some(index) => index.near_point(sample, out),
            None => self.reaching(sample, out),
        }
    }

    /// Whether a child at a position `<= at` masks the siblings after it.
    fn masked_through(&self, doc: &Document, at: usize) -> bool {
        match &self.index {
            Some(index) => index.masked_through(at),
            None => self.layer.children[..=at].iter().any(|m| doc.get(*m).is_some_and(|m| m.is_mask)),
        }
    }

    /// Whether a child at a position `>= at` masks the siblings after it.
    fn masked_from(&self, doc: &Document, at: usize) -> bool {
        match &self.index {
            Some(index) => index.masked_from(at),
            None => self.layer.children[at..].iter().any(|m| doc.get(*m).is_some_and(|m| m.is_mask)),
        }
    }

    /// Whether the layer draws its children in reverse document order.
    fn reversed(&self) -> bool {
        self.layer.paints_children_reversed() && self.layer.children.len() > 1
    }
}

impl Probe<'_> {
    /// The children of `layer`, indexed when the subtree bounds are known.
    fn kids<'n>(&self, parent: NodeId, layer: &'n Node) -> Kids<'n> {
        Kids { layer, index: self.bounds.map(|b| b.children_of(self.doc, parent)) }
    }

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
        let text_world = self.doc.world_transform(self.text);
        let page_corners = box_corners(self.doc.get(self.text)?.size).map(|c| text_world.apply(c));
        self.plain_cover_corners(id, world, page_corners, true)
    }

    fn plain_cover_corners(&mut self, id: NodeId, world: &Affine, page_corners: [Vec2; 4], reject_children_over: bool) -> Option<Vec<(Color, f32)>> {
        // Matrix products leave float noise; a layer turned by less is not turned.
        const AXIS: f32 = 1.0e-4;
        let doc = self.doc;
        let n = resolved_node(doc, id)?;
        let plain = matches!(n.kind, NodeKind::Rectangle | NodeKind::Frame(_)) && n.opacity >= 1.0 && !blends_as_group(n.blend);
        if !plain || n.effects.iter().any(|e| e.visible()) || world.b.abs() > AXIS || world.c.abs() > AXIS {
            return None;
        }
        // Target corners in the layer's own space, so a turned text is exact too.
        let to_layer = world.invert()?;
        let corners = page_corners.map(|c| to_layer.apply(c));
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
        if reject_children_over {
            // Children are painted over the fill, under the text.
            let clip = n.kind.frame_props().is_some_and(|f| f.clips_content).then(|| painted_box(world, &n, 0.0).own);
            let over = n.children.iter().any(|c| doc.get(*c).is_some_and(|child| self.paints_over(*c, *world * child.transform, clip, 0.0)));
            if over {
                return None;
            }
        }
        Some(fills)
    }
}

/// The positions of `near` (ascending, in document order) of the children that paint after the
/// child at `pos`, top-most first. A layer that draws its children in reverse document order paints
/// the earlier ones on top.
fn later_siblings_top_first(near: &[usize], pos: usize, reversed: bool) -> impl Iterator<Item = &usize> {
    let (down, up): (&[usize], &[usize]) =
        if reversed { (&[], &near[..near.partition_point(|a| *a < pos)]) } else { (&near[near.partition_point(|a| *a <= pos)..], &[]) };
    down.iter().rev().chain(up.iter())
}

/// What is painted behind a text layer.
#[derive(Debug, PartialEq)]
pub enum Behind {
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
/// paint order from the nearest.
pub fn has_layer_behind(doc: &Document, id: NodeId, occluder: Option<NodeId>, bounds: Option<&SubtreeBounds>) -> Behind {
    let Some(text_box) = page_box(doc, id) else { return Behind::Unknown };
    let mut probe = Probe { doc, text: id, text_box, bounds, budget: SUBTREE_BUDGET };
    let mut cover = None;
    let mut near = Vec::new();
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        if Some(cur) == occluder {
            break;
        }
        let Some(p) = doc.get(parent) else { return Behind::Unknown };
        let parent_world = doc.world_transform(parent);
        let kids = probe.kids(parent, p);
        let before = kids.position_of(cur).unwrap_or(p.children.len());
        kids.reaching(&probe.text_box, &mut near);
        for &at in near[..near.partition_point(|a| *a < before)].iter().rev() {
            let sibling = &p.children[at];
            let Some(s) = doc.get(*sibling) else { continue };
            let world = parent_world * s.transform;
            if !s.visible || s.opacity <= 0.0 {
                continue;
            }
            let painted = painted_box(&world, s, 0.0);
            let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
            let reaches = painted.rect.intersects(&probe.text_box)
                || (!clips
                    && s.children.iter().any(|c| doc.get(*c).is_some_and(|child| probe.paints_over(*c, world * child.transform, None, painted.effects))));
            if !reaches {
                continue;
            }
            let masked = kids.masked_through(doc, at);
            let fills = match probe.plain_cover(*sibling, &world) {
                Some(fills) if cover.is_none() && !masked => fills,
                _ => return Behind::Unknown,
            };
            let found = Behind::Cover { parent, fills };
            if opaque_up_to_the_page(doc, parent) {
                return found;
            }
            cover = Some(found);
            break;
        }
        cur = parent;
    }
    cover.unwrap_or(Behind::Nothing)
}

/// Returns the name of the top-most visible layer painted AFTER `id` whose painted subtree overlaps
/// `id`'s page box, or `None` if no layer painted after `id` overlaps it.
pub fn layer_painted_after(doc: &Document, id: NodeId, bounds: Option<&SubtreeBounds>) -> Option<String> {
    let text_box = page_box(doc, id)?;
    let mut probe = Probe { doc, text: id, text_box, bounds, budget: SUBTREE_BUDGET };
    let mut chain = Vec::new();
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        chain.push((cur, parent));
        cur = parent;
    }
    let mut near = Vec::new();
    // Check from outermost ancestor to innermost so the top-most painted layer wins.
    for (child, parent) in chain.into_iter().rev() {
        let p = doc.get(parent)?;
        let parent_world = doc.world_transform(parent);
        let kids = probe.kids(parent, p);
        let pos = kids.position_of(child)?;
        kids.reaching(&probe.text_box, &mut near);
        for &at in later_siblings_top_first(&near, pos, kids.reversed()) {
            let sibling = &p.children[at];
            let Some(s) = doc.get(*sibling) else { continue };
            if !s.visible || s.opacity <= 0.0 || s.is_mask {
                continue;
            }
            let world = parent_world * s.transform;
            let painted = painted_box(&world, s, 0.0);
            let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
            let reaches = (paints_something(s) && painted.rect.intersects(&probe.text_box))
                || (!clips && s.children.iter().any(|c| doc.get(*c).is_some_and(|ch| probe.paints_over(*c, world * ch.transform, None, painted.effects))))
                || (clips
                    && painted.own.intersects(&probe.text_box)
                    && s.children
                        .iter()
                        .any(|c| doc.get(*c).is_some_and(|ch| probe.paints_over(*c, world * ch.transform, Some(painted.own), painted.effects))));
            if reaches {
                return Some(s.name.clone());
            }
        }
    }
    None
}

/// Returns the name of the top-most visible opaque layer painted AFTER `id` that covers the glyph
/// sample rect around `sample_pt`, or `None` if no later sibling opaquely covers that sample.
pub fn covering_layer_painted_after(doc: &Document, id: NodeId, sample_pt: crate::lint::SamplePoint, bounds: Option<&SubtreeBounds>) -> Option<String> {
    let sample_box = Rect::new(sample_pt.x - 0.5, sample_pt.y - 0.5, 1.0, 1.0);
    let sample_corners = [
        Vec2::new(sample_box.x, sample_box.y),
        Vec2::new(sample_box.x + sample_box.w, sample_box.y),
        Vec2::new(sample_box.x + sample_box.w, sample_box.y + sample_box.h),
        Vec2::new(sample_box.x, sample_box.y + sample_box.h),
    ];
    let mut probe = Probe { doc, text: id, text_box: sample_box, bounds, budget: SUBTREE_BUDGET };
    let mut chain = Vec::new();
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        chain.push((cur, parent));
        cur = parent;
    }
    let mut near = Vec::new();
    for (child, parent) in chain.into_iter().rev() {
        if !opaque_up_to_the_page(doc, parent) {
            continue;
        }
        let p = doc.get(parent)?;
        let parent_world = doc.world_transform(parent);
        let kids = probe.kids(parent, p);
        let pos = kids.position_of(child)?;
        kids.near_point(&sample_box, &mut near);
        let reversed = kids.reversed();
        for &at in later_siblings_top_first(&near, pos, reversed) {
            let sibling = &p.children[at];
            let Some(s) = doc.get(*sibling) else { continue };
            if !s.visible || s.opacity <= 0.0 || s.is_mask {
                continue;
            }
            // Everything painted up to and including this sibling.
            let masked = if reversed { kids.masked_from(doc, at) } else { kids.masked_through(doc, at) };
            if masked {
                continue;
            }
            let world = parent_world * s.transform;
            if probe.plain_cover_corners(*sibling, &world, sample_corners, false).is_some() {
                return Some(s.name.clone());
            }
        }
    }
    None
}

/// True when `id` sits inside a filled `Frame` (`Some(parent) == occluder`) and every earlier
/// sibling in that frame that reaches `id` is a clean axis-aligned overlay (`Rectangle` or `Frame`
/// with normal blend, no effects, no stroke under the text, no mask, and full coverage over `id`).
pub fn has_plain_overlay_sibling_in_filled_frame(doc: &Document, id: NodeId, occluder: Option<NodeId>, bounds: Option<&SubtreeBounds>) -> bool {
    const AXIS: f32 = 1.0e-4;
    let Some(text_box) = page_box(doc, id) else { return false };
    let Some(parent) = doc.get(id).and_then(|n| n.parent) else { return false };
    if Some(parent) != occluder || !opaque_up_to_the_page(doc, parent) {
        return false;
    }
    let Some(p) = doc.get(parent) else { return false };
    let Some(res_p) = resolved_node(doc, parent) else { return false };
    if !matches!(res_p.kind, NodeKind::Frame(_)) || !res_p.fills.iter().any(|f| f.visible && f.opacity > 0.0) {
        return false;
    }
    let mut probe = Probe { doc, text: id, text_box, bounds, budget: SUBTREE_BUDGET };
    let parent_world = doc.world_transform(parent);
    let kids = probe.kids(parent, p);
    let before = kids.position_of(id).unwrap_or(p.children.len());
    let text_world = doc.world_transform(id);
    let Some(text_size) = doc.get(id).map(|n| n.size) else { return false };
    let mut saw_overlay = false;
    let mut near = Vec::new();
    kids.reaching(&probe.text_box, &mut near);
    for &at in near[..near.partition_point(|a| *a < before)].iter().rev() {
        let sibling = &p.children[at];
        let Some(s) = doc.get(*sibling) else { continue };
        let world = parent_world * s.transform;
        if !s.visible || s.opacity <= 0.0 {
            continue;
        }
        let painted = painted_box(&world, s, 0.0);
        let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
        let reaches = painted.rect.intersects(&probe.text_box)
            || (!clips && s.children.iter().any(|c| doc.get(*c).is_some_and(|child| probe.paints_over(*c, world * child.transform, None, painted.effects))));
        if !reaches {
            continue;
        }
        if kids.masked_through(doc, at) {
            return false;
        }
        let Some(res_s) = resolved_node(doc, *sibling) else { return false };
        if !matches!(res_s.kind, NodeKind::Rectangle | NodeKind::Frame(_))
            || blends_as_group(res_s.blend)
            || res_s.effects.iter().any(|e| e.visible())
            || world.b.abs() > AXIS
            || world.c.abs() > AXIS
        {
            return false;
        }
        let Some(to_layer) = world.invert() else { return false };
        let corners = box_corners(text_size).map(|c| to_layer.apply(text_world.apply(c)));
        if !inside_inset_box(&corners, res_s.size, res_s.corner_radii) || stroke_under(&res_s, &corners) {
            return false;
        }
        if !res_s.fills.iter().any(|f| f.visible && f.opacity > 0.0)
            || res_s.fills.iter().filter(|f| f.visible).any(|f| !matches!(f.blend, BlendMode::Normal | BlendMode::PassThrough))
        {
            return false;
        }
        let clip = res_s.kind.frame_props().is_some_and(|f| f.clips_content).then(|| painted_box(&world, &res_s, 0.0).own);
        let over = res_s.children.iter().any(|c| doc.get(*c).is_some_and(|child| probe.paints_over(*c, world * child.transform, clip, 0.0)));
        if over {
            return false;
        }
        saw_overlay = true;
    }
    saw_overlay
}

#[cfg(test)]
mod equivalence_tests;
#[cfg(test)]
mod reference;
