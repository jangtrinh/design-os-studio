//! The census of `kit_auto_layout_census.rs`, run unchanged for the reconciliation: every auto-layout frame solved at
//! once, and the page-space layers that move by more than 0.5 px counted.

use super::layout::{auto_frames, bounds, edge_diff, solve, Solved, TOL};
use designos_doc::*;
use std::collections::HashMap;

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
