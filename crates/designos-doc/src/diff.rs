//! Document and page-level structural/geometric diffing (`E15-8` / `#593`).
//!
//! Computes added, removed, modified, and moved nodes between two [`Document`] snapshots
//! along with the tight page-space union bounding box (`changed_bbox`) of affected nodes.

use crate::{resolve_node, Document, Node, NodeId, NodeKind, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Summary of structural and geometric changes between two page or document snapshots.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiffReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<NodeId>,
    #[serde(default)]
    pub added: Vec<NodeId>,
    #[serde(default)]
    pub removed: Vec<NodeId>,
    #[serde(default)]
    pub modified: Vec<NodeId>,
    #[serde(default)]
    pub moved: Vec<NodeId>,
    /// Union of page-space bounding boxes for every added, removed, modified, and moved node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_bbox: Option<Rect>,
}

/// Computes the page-space bounding box of `id` in `doc`.
pub fn node_world_bounds(doc: &Document, id: NodeId) -> Rect {
    let Some(n) = doc.get(id) else {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    };
    if matches!(n.kind, NodeKind::Page { .. }) && n.size.x == 0.0 && n.size.y == 0.0 {
        let mut iter = n.children.iter().copied().filter_map(|cid| doc.get(cid).map(|_| node_world_bounds(doc, cid))).filter(|r| r.w > 0.0 && r.h > 0.0);
        let Some(first) = iter.next() else {
            return Rect::new(0.0, 0.0, 0.0, 0.0);
        };
        return iter.fold(first, |acc, r| acc.union(&r));
    }
    let resolved = resolve_node(doc, n);
    let wt = doc.world_transform(id);
    let corners = [
        wt.apply(Vec2::new(0.0, 0.0)),
        wt.apply(Vec2::new(resolved.size.x, 0.0)),
        wt.apply(Vec2::new(0.0, resolved.size.y)),
        wt.apply(Vec2::new(resolved.size.x, resolved.size.y)),
    ];
    let min_x = corners.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let min_y = corners.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_x = corners.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let max_y = corners.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Rect::new(min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
    } else {
        Rect::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// Compares two resolved nodes ignoring `parent` and `children` (so a container whose only
/// change is a child insertion/deletion does not bloat the changed bounding box to the whole container).
fn node_visual_props_equal(before_doc: &Document, before_node: &Node, after_doc: &Document, after_node: &Node) -> bool {
    let mut a = resolve_node(before_doc, before_node).into_owned();
    let mut b = resolve_node(after_doc, after_node).into_owned();
    a.parent = None;
    b.parent = None;
    a.children.clear();
    b.children.clear();
    a.variables.bindings.clear();
    b.variables.bindings.clear();
    a == b
}

fn union_non_empty(acc: Option<Rect>, r: Rect) -> Option<Rect> {
    if r.w <= 0.0 && r.h <= 0.0 {
        return acc;
    }
    Some(match acc {
        Some(prev) => prev.union(&r),
        None => r,
    })
}

fn collect_page_nodes(doc: &Document, page_id: NodeId) -> BTreeSet<NodeId> {
    let mut ids = BTreeSet::new();
    if doc.get(page_id).is_some() {
        doc.walk(page_id, |n, _| {
            if n.id != page_id {
                ids.insert(n.id);
            }
        });
    }
    ids
}

/// Computes the structural and geometric diff for a single `page_id` between `before` and `after`.
pub fn diff_pages(before: &Document, after: &Document, page_id: NodeId) -> DiffReport {
    let before_ids = collect_page_nodes(before, page_id);
    let after_ids = collect_page_nodes(after, page_id);
    let all_ids: BTreeSet<NodeId> = before_ids.union(&after_ids).copied().collect();

    let mut report = DiffReport { page_id: Some(page_id), ..Default::default() };

    for id in all_ids {
        match (before_ids.contains(&id), after_ids.contains(&id)) {
            (false, true) => {
                report.added.push(id);
                report.changed_bbox = union_non_empty(report.changed_bbox, node_world_bounds(after, id));
            }
            (true, false) => {
                report.removed.push(id);
                report.changed_bbox = union_non_empty(report.changed_bbox, node_world_bounds(before, id));
            }
            (true, true) => {
                let (Some(bn), Some(an)) = (before.get(id), after.get(id)) else { continue };
                let b_wt = before.world_transform(id);
                let a_wt = after.world_transform(id);
                let parent_changed = bn.parent != an.parent;
                let world_moved = b_wt != a_wt;
                let sibling_reordered = !parent_changed
                    && bn.parent.is_some_and(|p| {
                        let b_pos = before.get(p).and_then(|pn| pn.children.iter().position(|&c| c == id));
                        let a_pos = after.get(p).and_then(|pn| pn.children.iter().position(|&c| c == id));
                        b_pos != a_pos && before.get(p).is_some_and(|pn| after.get(p).is_some_and(|apn| pn.children.len() == apn.children.len()))
                    });
                let visual_changed = !node_visual_props_equal(before, bn, after, an);

                if parent_changed || world_moved || sibling_reordered {
                    report.moved.push(id);
                    report.changed_bbox = union_non_empty(report.changed_bbox, node_world_bounds(before, id));
                    report.changed_bbox = union_non_empty(report.changed_bbox, node_world_bounds(after, id));
                }
                if visual_changed {
                    report.modified.push(id);
                    report.changed_bbox = union_non_empty(report.changed_bbox, node_world_bounds(before, id));
                    report.changed_bbox = union_non_empty(report.changed_bbox, node_world_bounds(after, id));
                }
            }
            (false, false) => {}
        }
    }

    report
}

/// Computes the union diff report across all pages in `before` and `after`.
pub fn diff_documents(before: &Document, after: &Document) -> DiffReport {
    let mut pages = Vec::new();
    let mut seen = BTreeSet::new();
    for &p in after.pages.iter().chain(before.pages.iter()) {
        if seen.insert(p) {
            pages.push(p);
        }
    }
    let mut out = DiffReport::default();
    for pid in pages {
        let p_rep = diff_pages(before, after, pid);
        out.added.extend(p_rep.added);
        out.removed.extend(p_rep.removed);
        out.modified.extend(p_rep.modified);
        out.moved.extend(p_rep.moved);
        if let Some(b) = p_rep.changed_bbox {
            out.changed_bbox = union_non_empty(out.changed_bbox, b);
        }
    }
    out
}

/// Grows `r` by `8.0` page units on all 4 sides, clipping to explicit page bounds if `page` has non-zero size.
pub fn grow_and_clip_page_region(doc: &Document, page_id: NodeId, r: Rect) -> Rect {
    let grown = Rect::new(r.x - 8.0, r.y - 8.0, r.w + 16.0, r.h + 16.0);
    if let Some(page_node) = doc.get(page_id) {
        if matches!(page_node.kind, NodeKind::Page { .. }) && page_node.size.x > 0.0 && page_node.size.y > 0.0 {
            let page_rect = Rect::new(0.0, 0.0, page_node.size.x, page_node.size.y);
            let x0 = grown.x.max(page_rect.x);
            let y0 = grown.y.max(page_rect.y);
            let x1 = (grown.x + grown.w).min(page_rect.x + page_rect.w);
            let y1 = (grown.y + grown.h).min(page_rect.y + page_rect.h);
            if x1 > x0 && y1 > y0 {
                return Rect::new(x0, y0, x1 - x0, y1 - y0);
            }
        }
    }
    grown
}

/// Returns `(page_id, page_name, padded_region)` for every page with a non-empty `changed_bbox`,
/// ordered by `after.pages` (followed by any removed pages from `before.pages`).
pub fn diff_bboxes_by_page(before: &Document, after: &Document) -> Vec<(NodeId, String, Rect)> {
    let mut page_order = Vec::new();
    let mut seen = BTreeSet::new();
    for &p in after.pages.iter().chain(before.pages.iter()) {
        if seen.insert(p) {
            page_order.push(p);
        }
    }
    let mut out = Vec::new();
    for pid in page_order {
        let rep = diff_pages(before, after, pid);
        if let Some(bbox) = rep.changed_bbox {
            if bbox.w > 0.0 && bbox.h > 0.0 {
                let page_name = after.get(pid).or_else(|| before.get(pid)).map(|n| n.name.clone()).unwrap_or_else(|| format!("Page {}", pid.0));
                let region = grow_and_clip_page_region(after, pid, bbox);
                out.push((pid, page_name, region));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Paint};

    #[test]
    fn diff_pages_tracks_recolour_move_and_delete_without_parent_bloat() {
        let mut before = Document::new("Test");
        let page = before.add_page("Page 1");
        let board_id = before.alloc_id();
        let mut board = Node::frame(board_id, Rect::new(0.0, 0.0, 2000.0, 2000.0));
        board.fills = vec![Paint::solid(Color::WHITE)];
        before.insert(page, board).unwrap();

        let rect_id = before.alloc_id();
        let mut rect = Node::rectangle(rect_id, Rect::new(100.0, 200.0, 400.0, 120.0));
        rect.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        before.insert(board_id, rect).unwrap();

        // 1. Recolour only -> changed_bbox is (100, 200, 400, 120), padded is (92, 192, 416, 136).
        let mut after_recolour = before.clone();
        after_recolour.get_mut(rect_id).unwrap().fills = vec![Paint::solid(Color::rgb(0, 0, 255))];
        let rep = diff_pages(&before, &after_recolour, page);
        assert_eq!(rep.changed_bbox, Some(Rect::new(100.0, 200.0, 400.0, 120.0)));
        let by_page = diff_bboxes_by_page(&before, &after_recolour);
        assert_eq!(by_page.len(), 1);
        assert_eq!(by_page[0].2, Rect::new(92.0, 192.0, 416.0, 136.0));

        // 2. Delete rect -> changed_bbox is the deleted rect's before bounds (100, 200, 400, 120), NOT 2000x2000 board!
        let mut after_del = before.clone();
        after_del.remove(rect_id).unwrap();
        let del_rep = diff_pages(&before, &after_del, page);
        assert_eq!(del_rep.changed_bbox, Some(Rect::new(100.0, 200.0, 400.0, 120.0)));
    }
}
