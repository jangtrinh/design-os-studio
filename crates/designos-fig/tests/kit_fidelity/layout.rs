//! Part 2, auto layout: every auto-layout frame is solved again against the imported state, and each flow child must
//! land within 0.5 px of the transform and size the file stored. Also the census of `kit_auto_layout_census.rs`
//! (page-space movers after every frame is solved at once), run unchanged for the reconciliation.

use super::board::{broken_comparator, Part};
use super::keys::NodeKeys;
use designos_doc::*;
use std::collections::HashMap;

pub const TOL: f32 = 0.5;

pub type Solved = (Vec2, Vec<(NodeId, Rect)>);

pub fn bounds(wt: Affine, size: Vec2) -> [f32; 4] {
    let c = [wt.apply(Vec2::new(0.0, 0.0)), wt.apply(Vec2::new(size.x, 0.0)), wt.apply(Vec2::new(0.0, size.y)), wt.apply(Vec2::new(size.x, size.y))];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in c {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    [x0, y0, x1, y1]
}

pub fn edge_diff(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs()).max((a[3] - b[3]).abs())
}

fn page_bounds(doc: &Document) -> HashMap<NodeId, [f32; 4]> {
    let mut out = HashMap::new();
    let mut stack: Vec<(NodeId, Affine, bool)> = doc.pages.iter().map(|&p| (p, Affine::IDENTITY, true)).collect();
    while let Some((id, pwt, is_page)) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        let wt = if is_page { Affine::IDENTITY } else { pwt * n.transform };
        if !is_page {
            out.insert(id, bounds(wt, n.size));
        }
        stack.extend(n.children.iter().map(|&c| (c, wt, false)));
    }
    out
}

pub fn auto_layout_of(doc: &Document, id: NodeId) -> Option<AutoLayout> {
    let n = doc.get(id)?;
    resolve_node(doc, n).kind.frame_props()?.auto_layout.clone()
}

/// The census's solve of one frame against the current document state.
pub fn solve(doc: &Document, fid: NodeId) -> Option<Solved> {
    let root = doc.get(fid)?;
    let resolved = resolve_node(doc, root);
    let al = resolved.kind.frame_props()?.auto_layout.clone()?;
    if root.children.is_empty() {
        return None;
    }
    let own = layout_solver::SizeLimits::from(&root.layout_child);
    let stroke = resolved.stroke.clone();
    let horiz = al.direction != LayoutDirection::Vertical;
    let (ids, mut kids): (Vec<NodeId>, Vec<layout_solver::ChildIn>) =
        root.children.iter().filter_map(|c| doc.flow_child_input(*c, horiz).map(|k| (*c, k))).unzip();
    let measure = designos_doc::text_measure::layout_measurer();
    let out = layout_solver::compute_then_refit_wrapped(&al, root.size, own, &stroke, &ids, &mut kids, |id, w| {
        let m = measure.as_deref()?;
        designos_doc::text_measure::height_resize_text_height(doc.get(id)?, w, m)
    });
    Some((out.frame, ids.into_iter().zip(out.rects).collect()))
}

fn auto_frames(doc: &Document) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> = doc.nodes.keys().copied().filter(|&id| auto_layout_of(doc, id).is_some()).collect();
    ids.sort_by_key(|i| i.0);
    ids
}

fn in_hidden_subtree(doc: &Document, mut id: NodeId) -> bool {
    loop {
        let Some(n) = doc.get(id) else { return false };
        if matches!(n.kind, NodeKind::Page { .. }) {
            return false;
        }
        if !n.visible {
            return true;
        }
        match n.parent {
            Some(p) => id = p,
            None => return false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Census {
    pub page_layers: usize,
    pub auto_frames_on_pages: usize,
    pub moved: usize,
    pub moved_hidden: usize,
}

/// The census algorithm: solve every auto-layout frame, write all results at once in id order, and count the
/// page-space layers whose bounds move by more than 0.5 px. The document is restored afterwards.
pub fn census(doc: &mut Document) -> Census {
    let before = page_bounds(doc);
    let frames = auto_frames(doc);
    let auto_frames_on_pages = frames.iter().filter(|f| before.contains_key(f)).count();
    let solved: Vec<(NodeId, Solved)> = frames.iter().filter_map(|&f| solve(doc, f).map(|s| (f, s))).collect();
    let mut saved: HashMap<NodeId, (Affine, Vec2)> = HashMap::new();
    for (fid, (frame, rects)) in solved {
        if let Some(f) = doc.get_mut(fid) {
            saved.entry(fid).or_insert((f.transform, f.size));
            f.size = frame;
        }
        for (cid, r) in rects {
            if let Some(c) = doc.get_mut(cid) {
                saved.entry(cid).or_insert((c.transform, c.size));
                let w = designos_doc::slot_write(&c.transform, c.size, r);
                c.transform = w.transform;
                c.size = w.size;
            }
        }
    }
    let after = page_bounds(doc);
    let moved: Vec<NodeId> = before.iter().filter(|(id, b0)| after.get(id).is_some_and(|b1| edge_diff(b0, b1) > TOL)).map(|(id, _)| *id).collect();
    for (id, (t, s)) in saved {
        if let Some(n) = doc.get_mut(id) {
            n.transform = t;
            n.size = s;
        }
    }
    let moved_hidden = moved.iter().filter(|id| in_hidden_subtree(doc, **id)).count();
    Census { page_layers: before.len(), auto_frames_on_pages, moved: moved.len(), moved_hidden }
}

fn sizing(s: Sizing) -> &'static str {
    match s {
        Sizing::Fixed => "fixed",
        Sizing::Hug => "hug",
        Sizing::Fill => "fill",
    }
}

fn child_buckets(al: &AutoLayout, c: &Node) -> Vec<String> {
    let mut b = vec!["total".to_string()];
    b.push(match al.direction {
        LayoutDirection::Horizontal => "direction: horizontal".into(),
        LayoutDirection::Vertical => "direction: vertical".into(),
        LayoutDirection::Grid => "grid".into(),
    });
    if al.direction != LayoutDirection::Grid {
        b.push(if al.primary_align == LayoutAlign::SpaceBetween { "spacing: space-between".into() } else { "spacing: gap".into() });
        b.push(if al.wrap { "wrap: yes".into() } else { "wrap: no".into() });
    }
    b.push(if al.padding.iter().any(|p| *p != 0.0) { "padding: set".into() } else { "padding: none".into() });
    let lc = &c.layout_child;
    let fill_w = lc.horizontal == Sizing::Fill || (al.direction == LayoutDirection::Horizontal && lc.grow > 0.0);
    let fill_h = lc.vertical == Sizing::Fill || (al.direction == LayoutDirection::Vertical && lc.grow > 0.0);
    b.push(format!("child width: {}", if fill_w { "fill" } else { sizing(lc.horizontal) }));
    b.push(format!("child height: {}", if fill_h { "fill" } else { sizing(lc.vertical) }));
    if lc.min_w.is_some() || lc.max_w.is_some() || lc.min_h.is_some() || lc.max_h.is_some() {
        b.push("min/max: set".into());
    }
    if al.counter_align == LayoutAlign::Baseline || lc.align_self == AlignSelf::Baseline {
        b.push("baseline".into());
    }
    if matches!(c.kind, NodeKind::Text(_)) {
        b.push("text child".into());
    }
    let t = c.transform;
    if t.b.abs() > 1e-3 || t.c.abs() > 1e-3 || t.a < 0.0 || t.d < 0.0 {
        b.push("rotated child".into());
    }
    b
}

/// Scores each flow child of each auto-layout frame on the document (solved one frame at a time).
pub fn part(doc: &Document, keys: &NodeKeys) -> Part {
    let mut p = Part::new("auto_layout", "Auto layout", "each flow child within 0.5 px of its stored transform and size after its frame is solved again");
    let tol = if broken_comparator() { f32::INFINITY } else { TOL };
    for fid in auto_frames(doc) {
        if !keys.key.contains_key(&fid) {
            continue;
        }
        let Some(al) = auto_layout_of(doc, fid) else { continue };
        let Some(n) = doc.get(fid) else { continue };
        for &cid in &n.children {
            if doc.get(cid).is_some_and(|c| c.layout_child.absolute) {
                p.unscored("absolute child", "not placed by the flow solve, so a solve can never move it");
            }
        }
        let Some((_, rects)) = solve(doc, fid) else { continue };
        for (cid, r) in rects {
            let Some(c) = doc.get(cid) else { continue };
            let w = designos_doc::slot_write(&c.transform, c.size, r);
            let d = edge_diff(&bounds(c.transform, c.size), &bounds(w.transform, w.size));
            p.record(keys.key_of(cid), keys.is_hidden(cid), d <= tol, &child_buckets(&al, c));
        }
    }
    p
}
