//! The scans for layers around a text as they were before the sibling index: every sibling of
//! every ancestor is looked at, in paint order. They are the reference the indexed scans in the
//! parent module must agree with.

use super::*;

pub fn has_layer_behind(doc: &Document, id: NodeId, occluder: Option<NodeId>, bounds: Option<&SubtreeBounds>) -> Behind {
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
            let painted = painted_box(&world, s, 0.0);
            let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
            let reaches = painted.rect.intersects(&probe.text_box)
                || (!clips
                    && s.children.iter().any(|c| doc.get(*c).is_some_and(|child| probe.paints_over(*c, world * child.transform, None, painted.effects))));
            if !reaches {
                continue;
            }
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
            break;
        }
        cur = parent;
    }
    cover.unwrap_or(Behind::Nothing)
}

pub fn layer_painted_after(doc: &Document, id: NodeId, bounds: Option<&SubtreeBounds>) -> Option<String> {
    let text_box = page_box(doc, id)?;
    let mut probe = Probe { doc, text: id, text_box, bounds, budget: SUBTREE_BUDGET };
    let mut chain = Vec::new();
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        chain.push((cur, parent));
        cur = parent;
    }
    // Check from outermost ancestor to innermost so the top-most painted layer wins.
    for (child, parent) in chain.into_iter().rev() {
        let p = doc.get(parent)?;
        let parent_world = doc.world_transform(parent);
        let paint_kids = p.paint_children();
        let pos = paint_kids.iter().position(|s| *s == child)?;
        for sibling in paint_kids[pos + 1..].iter().rev() {
            let Some(s) = doc.get(*sibling) else { continue };
            if !s.visible || s.opacity <= 0.0 || s.is_mask {
                continue;
            }
            let world = parent_world * s.transform;
            if !probe.can_reach(*sibling, world, 0.0) {
                continue;
            }
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
    for (child, parent) in chain.into_iter().rev() {
        if !opaque_up_to_the_page(doc, parent) {
            continue;
        }
        let p = doc.get(parent)?;
        let parent_world = doc.world_transform(parent);
        let paint_kids = p.paint_children();
        let pos = paint_kids.iter().position(|s| *s == child)?;
        for (offset, sibling) in paint_kids[pos + 1..].iter().enumerate().rev() {
            let Some(s) = doc.get(*sibling) else { continue };
            if !s.visible || s.opacity <= 0.0 || s.is_mask {
                continue;
            }
            let masked = paint_kids[..=pos + 1 + offset].iter().any(|m| doc.get(*m).is_some_and(|m| m.is_mask));
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
    let before = p.children.iter().position(|s| *s == id).unwrap_or(p.children.len());
    let text_world = doc.world_transform(id);
    let Some(text_size) = doc.get(id).map(|n| n.size) else { return false };
    let mut saw_overlay = false;
    for (at, sibling) in p.children[..before].iter().enumerate().rev() {
        let Some(s) = doc.get(*sibling) else { continue };
        let world = parent_world * s.transform;
        if !s.visible || s.opacity <= 0.0 || !probe.can_reach(*sibling, world, 0.0) {
            continue;
        }
        let painted = painted_box(&world, s, 0.0);
        let clips = s.kind.frame_props().is_some_and(|f| f.clips_content);
        let reaches = painted.rect.intersects(&probe.text_box)
            || (!clips && s.children.iter().any(|c| doc.get(*c).is_some_and(|child| probe.paints_over(*c, world * child.transform, None, painted.effects))));
        if !reaches {
            continue;
        }
        if p.children[..=at].iter().any(|m| doc.get(*m).is_some_and(|m| m.is_mask)) {
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
