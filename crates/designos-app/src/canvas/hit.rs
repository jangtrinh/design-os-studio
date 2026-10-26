//! Canvas hit testing and marquee selection.

use designos_doc::{front_to_back, Affine, Document, Node, NodeId, NodeKind, Rect, Vec2};

fn local_hit(kind: &NodeKind, size: Vec2, local: Vec2, tol: f32) -> bool {
    match kind {
        NodeKind::Line => local.x >= -tol && local.x <= size.x + tol && local.y.abs() <= tol.max(size.y / 2.0 + tol),
        NodeKind::Ellipse(_) | NodeKind::Stamp { .. } => {
            let rx = size.x / 2.0 + tol;
            let ry = size.y / 2.0 + tol;
            if rx <= 0.0 || ry <= 0.0 {
                return false;
            }
            let dx = (local.x - size.x / 2.0) / rx;
            let dy = (local.y - size.y / 2.0) / ry;
            dx * dx + dy * dy <= 1.0
        }
        NodeKind::ShapeWithText(s) if s.shape == designos_doc::ShapeKind::Ellipse => local_hit(&NodeKind::Ellipse(Default::default()), size, local, tol),
        _ => local.x >= -tol && local.y >= -tol && local.x <= size.x + tol && local.y <= size.y + tol,
    }
}

/// True if `node` is a [`NodeKind::Component`] whose only child is a leaf shape
/// (rectangle, ellipse, line, polygon, star, vector, boolean op, text, or image)
/// rather than a frame, group, component, instance, or section.
pub fn is_one_shape_component(doc: &Document, node: &Node) -> bool {
    if !matches!(node.kind, NodeKind::Component(_)) {
        return false;
    }
    let [child_id] = node.children.as_slice() else {
        return false;
    };
    let Some(child) = doc.get(*child_id) else {
        return false;
    };
    matches!(
        child.kind,
        NodeKind::Rectangle
            | NodeKind::Ellipse(_)
            | NodeKind::Line
            | NodeKind::Polygon { .. }
            | NodeKind::Star { .. }
            | NodeKind::Vector { .. }
            | NodeKind::BooleanOp(_)
            | NodeKind::Text(_)
    )
}

/// True if the container is "transparent" to clicks: clicking a child
/// selects the child directly (frames, sections, pages, component sets,
/// and multi-child or container-child components). A component made from
/// one leaf shape ([`is_one_shape_component`]) is selected as a whole on
/// single click and entered on double-click.
fn selects_through(doc: &Document, node: &Node) -> bool {
    matches!(node.kind, NodeKind::Page { .. } | NodeKind::Frame(_) | NodeKind::Section | NodeKind::Component(_) | NodeKind::ComponentSet(_))
        && !is_one_shape_component(doc, node)
}

/// True if a marquee that only partly covers a container of this kind selects the layers inside it
/// instead of the container. Main components and component sets are selected whole by a marquee,
/// like instances; their layers stay reachable by click, Cmd+click and double-click.
fn marquee_enters(doc: &Document, node: &Node) -> bool {
    selects_through(doc, node) && !matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_))
}

/// Finds the node under `p` (page space).
///
/// * `deep` selects the deepest node (ctrl/cmd-click).
/// * `entered` lists containers (usually ancestors of the current selection)
///   that groups/instances may be descended into.
#[allow(dead_code)]
pub fn hit_test(doc: &Document, root: NodeId, p: Vec2, tol: f32, deep: bool, entered: &[NodeId]) -> Option<NodeId> {
    hit_test_with_index(doc, root, p, tol, deep, entered, None)
}

/// Like [`hit_test`], culling subtrees via `index` when available.
pub fn hit_test_with_index(
    doc: &Document,
    root: NodeId,
    p: Vec2,
    tol: f32,
    deep: bool,
    entered: &[NodeId],
    index: Option<&designos_render::SceneIndex>,
) -> Option<NodeId> {
    hit_test_excluding_with_index(doc, root, p, tol, deep, entered, &[], index)
}

/// [`hit_test`] that ignores the `exclude`d nodes (and everything inside them).
pub fn hit_test_excluding(doc: &Document, root: NodeId, p: Vec2, tol: f32, deep: bool, entered: &[NodeId], exclude: &[NodeId]) -> Option<NodeId> {
    hit_test_excluding_with_index(doc, root, p, tol, deep, entered, exclude, None)
}

/// [`hit_test_excluding`] that culls subtrees via `index` when available.
#[allow(clippy::too_many_arguments)]
pub fn hit_test_excluding_with_index(
    doc: &Document,
    root: NodeId,
    p: Vec2,
    tol: f32,
    deep: bool,
    entered: &[NodeId],
    exclude: &[NodeId],
    index: Option<&designos_render::SceneIndex>,
) -> Option<NodeId> {
    hit_in(doc, root, Affine::IDENTITY, p, tol, deep, entered, exclude, index)
}

fn indexed_bounds(index: Option<&designos_render::SceneIndex>, id: NodeId) -> Option<Rect> {
    let (x0, y0, x1, y1) = index?.bounds_of(id)?;
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Whether `p` (in the connector's parent space) is on the line of connector `id`.
fn connector_hit(doc: &Document, id: NodeId, parent_local: Vec2, tol: f32) -> bool {
    let Some(n) = doc.get(id) else { return false };
    let Some(route) = designos_doc::connector_route(doc, id) else { return false };
    route.distance_to(parent_local) <= n.stroke.weight / 2.0 + tol * 3.0
}

#[allow(clippy::too_many_arguments)]
fn hit_in(
    doc: &Document,
    parent: NodeId,
    parent_world: Affine,
    p: Vec2,
    tol: f32,
    deep: bool,
    entered: &[NodeId],
    exclude: &[NodeId],
    index: Option<&designos_render::SceneIndex>,
) -> Option<NodeId> {
    let pn = doc.get(parent)?;
    let mut top_cands = Vec::new();
    let iter_slice: &[NodeId] = if let Some(idx) = index.filter(|i| {
        pn.children.len() > 64
            && matches!(pn.kind, NodeKind::Page { .. })
            && pn.children.iter().find(|&&c| doc.get(c).is_some_and(|n| n.visible)).is_some_and(|&c| i.bounds_of(c).is_some())
            && !pn.children.iter().any(|&c| matches!(doc.get(c).map(|n| &n.kind), Some(NodeKind::Connector(_))))
    }) {
        let m = tol + 4.0;
        idx.query_top_level((p.x - m, p.y - m, p.x + m, p.y + m), &mut top_cands);
        &top_cands
    } else {
        &pn.children
    };
    for &cid in front_to_back(iter_slice, pn.paints_children_reversed()) {
        let Some(n) = doc.get(cid) else { continue };
        if !n.visible || n.locked || exclude.contains(&cid) {
            continue;
        }
        if !matches!(n.kind, NodeKind::Connector(_)) {
            if let Some(bb) = indexed_bounds(index, cid) {
                let m = tol + 2.0;
                if !Rect::new(bb.x - m, bb.y - m, bb.w + 2.0 * m, bb.h + 2.0 * m).contains(p) {
                    continue;
                }
            }
        }
        let world = parent_world * n.transform;
        let Some(inv) = world.invert() else { continue };
        let local = inv.apply(p);
        let inside = match &n.kind {
            // a connector is its line, not its bounding box
            NodeKind::Connector(_) => parent_world.invert().is_some_and(|pi| connector_hit(doc, cid, pi.apply(p), tol)),
            k => local_hit(k, n.size, local, tol),
        };
        if n.kind.is_container() {
            let descend = deep || selects_through(doc, n) || entered.contains(&cid);
            let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
            let near_unindexed =
                index.is_some() || inside || n.size.x <= 0.0 || n.size.y <= 0.0 || matches!(n.kind, NodeKind::Group | NodeKind::BooleanOp(_)) || {
                    let pad = if doc.nodes.len() > 5_000 { 512.0 } else { n.size.x.max(n.size.y) + 2000.0 };
                    local.x >= -pad && local.y >= -pad && local.x <= n.size.x + pad && local.y <= n.size.y + pad
                };
            if descend && (!clips || inside) && near_unindexed {
                if let Some(h) = hit_in(doc, cid, world, p, tol, deep, entered, exclude, index) {
                    return Some(h);
                }
            }
            if inside {
                return Some(cid);
            }
        } else if inside {
            return Some(cid);
        }
    }
    None
}

/// Collects all visible, unlocked nodes on `page` under `p` (page space),
/// ordered from top-most leaf down to container/frame (capped at 32 items).
pub fn hit_all_under(doc: &Document, page: NodeId, p: Vec2, index: Option<&designos_render::SceneIndex>) -> Vec<NodeId> {
    let mut out = Vec::new();
    hit_all_in(doc, page, Affine::IDENTITY, p, 2.0, index, &mut out);
    out
}

fn hit_all_in(doc: &Document, parent: NodeId, parent_world: Affine, p: Vec2, tol: f32, index: Option<&designos_render::SceneIndex>, out: &mut Vec<NodeId>) {
    if out.len() >= 32 {
        return;
    }
    let Some(pn) = doc.get(parent) else { return };
    for &cid in pn.children_front_to_back() {
        if out.len() >= 32 {
            return;
        }
        let Some(n) = doc.get(cid) else { continue };
        if !n.visible || n.locked {
            continue;
        }
        if !matches!(n.kind, NodeKind::Connector(_)) {
            if let Some(bb) = indexed_bounds(index, cid) {
                let m = tol + 2.0;
                if !Rect::new(bb.x - m, bb.y - m, bb.w + 2.0 * m, bb.h + 2.0 * m).contains(p) {
                    continue;
                }
            }
        }
        let world = parent_world * n.transform;
        let Some(inv) = world.invert() else { continue };
        let local = inv.apply(p);
        let inside = match &n.kind {
            NodeKind::Connector(_) => parent_world.invert().is_some_and(|pi| connector_hit(doc, cid, pi.apply(p), tol)),
            k => local_hit(k, n.size, local, tol),
        };
        if n.kind.is_container() {
            let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
            if !clips || inside {
                hit_all_in(doc, cid, world, p, tol, index, out);
            }
            if inside && out.len() < 32 {
                out.push(cid);
            }
        } else if inside && out.len() < 32 {
            out.push(cid);
        }
    }
}

/// Nodes selected by a marquee (page space). Fully-enclosed top-level objects
/// are selected as a whole; frames only partly covered are entered. Main
/// components and component sets are always selected as a whole.
#[allow(dead_code)]
pub fn marquee_select(doc: &Document, root: NodeId, r: &Rect) -> Vec<NodeId> {
    marquee_select_with_index(doc, root, r, None)
}

const MAX_MARQUEE_NODES: usize = 512;

/// Like [`marquee_select`], using `index` bounds when available instead of
/// recomputing `world_aabb` recursively for every child.
pub fn marquee_select_with_index(doc: &Document, root: NodeId, r: &Rect, index: Option<&designos_render::SceneIndex>) -> Vec<NodeId> {
    let mut out = Vec::new();
    let large_doc = doc.nodes.len() > 2_000;
    if let (Some(idx), Some(page)) = (index, doc.get(root).filter(|n| matches!(n.kind, NodeKind::Page { .. }))) {
        let mut cands = Vec::new();
        let slice: &[NodeId] = if page.children.len() > 32 {
            idx.query_top_level((r.x, r.y, r.x + r.w, r.y + r.h), &mut cands);
            &cands
        } else {
            &page.children
        };
        for &cid in slice {
            if out.len() >= MAX_MARQUEE_NODES {
                break;
            }
            let Some(n) = doc.get(cid) else { continue };
            if !n.visible || n.locked {
                continue;
            }
            let bb = if let Some((x0, y0, x1, y1)) = idx.bounds_of(cid) { Rect::new(x0, y0, x1 - x0, y1 - y0) } else { super::fast_world_aabb(doc, None, cid) };
            if !r.intersects(&bb) {
                continue;
            }
            let contained = bb.x >= r.x && bb.y >= r.y && bb.x + bb.w <= r.x + r.w && bb.y + bb.h <= r.y + r.h;
            if n.kind.is_container() && marquee_enters(doc, n) && !contained && !n.children.is_empty() {
                marquee_in(doc, cid, r, index, 1, large_doc, &mut out);
            } else {
                out.push(cid);
            }
        }
        return out;
    }
    marquee_in(doc, root, r, index, 0, large_doc, &mut out);
    out
}

fn marquee_in(doc: &Document, parent: NodeId, r: &Rect, index: Option<&designos_render::SceneIndex>, depth: usize, large_doc: bool, out: &mut Vec<NodeId>) {
    let Some(pn) = doc.get(parent) else { return };
    for &cid in &pn.children {
        if out.len() >= MAX_MARQUEE_NODES {
            break;
        }
        let Some(n) = doc.get(cid) else { continue };
        if !n.visible || n.locked {
            continue;
        }
        let bb = if let Some(ibb) = indexed_bounds(index, cid) { ibb } else { super::fast_world_aabb(doc, None, cid) };
        if !r.intersects(&bb) {
            continue;
        }
        let contained = bb.x >= r.x && bb.y >= r.y && bb.x + bb.w <= r.x + r.w && bb.y + bb.h <= r.y + r.h;
        let can_descend = if large_doc {
            matches!(n.kind, NodeKind::Section)
                || (depth < 2 && r.x >= bb.x - 4.0 && r.y >= bb.y - 4.0 && r.x + r.w <= bb.x + bb.w + 4.0 && r.y + r.h <= bb.y + bb.h + 4.0)
        } else {
            true
        };
        if n.kind.is_container() && marquee_enters(doc, n) && !contained && !n.children.is_empty() && can_descend {
            marquee_in(doc, cid, r, index, depth + 1, large_doc, out);
        } else {
            out.push(cid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::v2;
    use designos_doc::{FrameProps, Node};

    fn sample() -> (Document, NodeId, NodeId, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        let gid = d.alloc_id();
        let mut g = Node::new(gid, NodeKind::Group);
        g.size = v2(50.0, 50.0);
        g.transform = Affine::translate(100.0, 100.0);
        let group = d.insert(frame, g).unwrap();
        let rid = d.alloc_id();
        let rect_in_group = d.insert(group, Node::rectangle(rid, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        let r2 = d.alloc_id();
        let rect = d.insert(frame, Node::rectangle(r2, Rect::new(10.0, 10.0, 30.0, 30.0))).unwrap();
        (d, page, frame, group, rect_in_group, rect)
    }

    #[test]
    fn click_selects_child_of_top_frame() {
        let (d, page, _frame, _g, _rg, rect) = sample();
        assert_eq!(hit_test(&d, page, v2(20.0, 20.0), 0.0, false, &[]), Some(rect));
    }

    #[test]
    fn click_group_selects_group_not_child() {
        let (d, page, _f, group, rg, _r) = sample();
        assert_eq!(hit_test(&d, page, v2(120.0, 120.0), 0.0, false, &[]), Some(group));
        assert_eq!(hit_test(&d, page, v2(120.0, 120.0), 0.0, true, &[]), Some(rg));
        assert_eq!(hit_test(&d, page, v2(120.0, 120.0), 0.0, false, &[group]), Some(rg));
    }

    #[test]
    fn click_empty_frame_area_selects_frame() {
        let (d, page, frame, ..) = sample();
        assert_eq!(hit_test(&d, page, v2(180.0, 20.0), 0.0, false, &[]), Some(frame));
        assert_eq!(hit_test(&d, page, v2(500.0, 20.0), 0.0, false, &[]), None);
    }

    #[test]
    fn locked_and_hidden_are_skipped() {
        let (mut d, page, _f, _g, _rg, rect) = sample();
        d.get_mut(rect).unwrap().locked = true;
        let f = hit_test(&d, page, v2(20.0, 20.0), 0.0, false, &[]).unwrap();
        assert_ne!(f, rect);
        d.get_mut(rect).unwrap().locked = false;
        d.get_mut(rect).unwrap().visible = false;
        assert_ne!(hit_test(&d, page, v2(20.0, 20.0), 0.0, false, &[]), Some(rect));
    }

    #[test]
    fn ellipse_corner_misses() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let e = d.insert(page, Node::ellipse(id, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        assert_eq!(hit_test(&d, page, v2(50.0, 50.0), 0.0, false, &[]), Some(e));
        assert_eq!(hit_test(&d, page, v2(2.0, 2.0), 0.0, false, &[]), None);
    }

    #[test]
    fn rotated_node_hit() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 10.0));
        n.transform = Affine::translate(50.0, 50.0) * Affine::rotate(std::f32::consts::FRAC_PI_2);
        let r = d.insert(page, n).unwrap();
        // rotated 90deg clockwise: occupies x in [40,50], y in [50,150]
        assert_eq!(hit_test(&d, page, v2(45.0, 100.0), 0.0, false, &[]), Some(r));
        assert_eq!(hit_test(&d, page, v2(100.0, 55.0), 0.0, false, &[]), None);
    }

    #[test]
    fn marquee_enters_partially_covered_frame() {
        let (d, page, frame, _g, _rg, rect) = sample();
        let partial = marquee_select(&d, page, &Rect::new(0.0, 0.0, 60.0, 60.0));
        assert_eq!(partial, vec![rect]);
        let all = marquee_select(&d, page, &Rect::new(-10.0, -10.0, 400.0, 400.0));
        assert_eq!(all, vec![frame]);
    }

    #[test]
    fn empty_frame_props_default_ok() {
        assert!(!FrameProps::default().clips_content);
    }

    #[test]
    fn hit_and_marquee_with_scene_index_matches() {
        let (d, page, frame, group, rg, rect) = sample();
        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&d, page, 1);
        assert_eq!(hit_test_with_index(&d, page, v2(20.0, 20.0), 0.0, false, &[], Some(&idx)), Some(rect));
        assert_eq!(hit_test_with_index(&d, page, v2(120.0, 120.0), 0.0, false, &[], Some(&idx)), Some(group));
        assert_eq!(hit_test_with_index(&d, page, v2(120.0, 120.0), 0.0, true, &[], Some(&idx)), Some(rg));
        assert_eq!(hit_test_with_index(&d, page, v2(500.0, 20.0), 0.0, false, &[], Some(&idx)), None);
        assert_eq!(hit_test_excluding_with_index(&d, page, v2(20.0, 20.0), 0.0, false, &[], &[rect], Some(&idx)), Some(frame));
        assert_eq!(marquee_select_with_index(&d, page, &Rect::new(0.0, 0.0, 60.0, 60.0), Some(&idx)), vec![rect]);
        assert_eq!(marquee_select_with_index(&d, page, &Rect::new(-10.0, -10.0, 400.0, 400.0), Some(&idx)), vec![frame]);
    }

    #[test]
    fn hit_all_under_returns_overlapping_layers_top_to_bottom() {
        let (mut d, page, frame, group, rg, rect) = sample();
        let top_id = d.alloc_id();
        let top_rect = d.insert(frame, Node::rectangle(top_id, Rect::new(15.0, 15.0, 40.0, 40.0))).unwrap();
        let hits = hit_all_under(&d, page, v2(20.0, 20.0), None);
        assert_eq!(hits, vec![top_rect, rect, frame]);
        let group_hits = hit_all_under(&d, page, v2(120.0, 120.0), None);
        assert_eq!(group_hits, vec![rg, group, frame]);
    }

    /// Horizontal auto-layout row with two overlapping squares (x 0..20 and 8..28).
    fn overlapping_row(reverse_z: bool) -> (Document, NodeId, NodeId, NodeId, NodeId) {
        use designos_doc::{AutoLayout, LayoutDirection};
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let rid = d.alloc_id();
        let mut row = Node::frame(rid, Rect::new(0.0, 0.0, 40.0, 20.0));
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: -12.0, reverse_z, ..Default::default() });
        }
        let row = d.insert(page, row).unwrap();
        let rid = d.alloc_id();
        let red = d.insert(row, Node::rectangle(rid, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
        let bid = d.alloc_id();
        let blue = d.insert(row, Node::rectangle(bid, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
        d.relayout_subtree(row);
        (d, page, row, red, blue)
    }

    #[test]
    fn reverse_z_click_on_overlap_selects_first_child() {
        let (d, page, _row, red, blue) = overlapping_row(true);
        assert_eq!(d.get(blue).unwrap().transform.apply(v2(0.0, 0.0)).x, 8.0, "children must overlap at x 8..20");
        let overlap = v2(14.0, 10.0);
        assert_eq!(hit_test(&d, page, overlap, 0.0, false, &[]), Some(red));
        assert_eq!(hit_test(&d, page, overlap, 0.0, true, &[]), Some(red));
        // outside the overlap each child is still reachable
        assert_eq!(hit_test(&d, page, v2(4.0, 10.0), 0.0, false, &[]), Some(red));
        assert_eq!(hit_test(&d, page, v2(24.0, 10.0), 0.0, false, &[]), Some(blue));
        // excluding the front-most child falls through to the one painted underneath
        assert_eq!(hit_test_excluding(&d, page, overlap, 0.0, false, &[], &[red]), Some(blue));
    }

    #[test]
    fn reverse_z_under_cursor_stack_lists_first_child_first() {
        let (d, page, row, red, blue) = overlapping_row(true);
        assert_eq!(hit_all_under(&d, page, v2(14.0, 10.0), None), vec![red, blue, row]);
    }

    #[test]
    fn reverse_z_hit_matches_scene_index_path() {
        let (d, page, _row, red, blue) = overlapping_row(true);
        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&d, page, 1);
        assert_eq!(hit_test_with_index(&d, page, v2(14.0, 10.0), 0.0, false, &[], Some(&idx)), Some(red));
        assert_eq!(hit_test_with_index(&d, page, v2(24.0, 10.0), 0.0, false, &[], Some(&idx)), Some(blue));
        assert_eq!(hit_all_under(&d, page, v2(14.0, 10.0), Some(&idx))[..2], [red, blue]);
    }

    #[test]
    fn non_reverse_z_overlap_selects_last_child() {
        let (d, page, row, red, blue) = overlapping_row(false);
        let overlap = v2(14.0, 10.0);
        assert_eq!(hit_test(&d, page, overlap, 0.0, false, &[]), Some(blue));
        assert_eq!(hit_all_under(&d, page, overlap, None), vec![blue, red, row]);
    }

    /// A container of `kind` at `r` under `parent`.
    fn container_at(d: &mut Document, parent: NodeId, kind: NodeKind, r: Rect) -> NodeId {
        let id = d.alloc_id();
        let mut n = Node::frame(id, r);
        n.kind = kind;
        d.insert(parent, n).unwrap()
    }

    fn rect_at(d: &mut Document, parent: NodeId, r: Rect) -> NodeId {
        let id = d.alloc_id();
        d.insert(parent, Node::rectangle(id, r)).unwrap()
    }

    /// Page with a 200x200 main component at the origin holding a layer at (10,10) 30x30 and one at (120,120) 60x60.
    fn main_component_sample() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let comp = container_at(&mut d, page, NodeKind::Component(FrameProps::default()), Rect::new(0.0, 0.0, 200.0, 200.0));
        let near = rect_at(&mut d, comp, Rect::new(10.0, 10.0, 30.0, 30.0));
        let far = rect_at(&mut d, comp, Rect::new(120.0, 120.0, 60.0, 60.0));
        (d, page, comp, near, far)
    }

    /// Page with a component set holding two 200x200 variants (x 0 and x 240), each with one layer near its top-left corner.
    fn component_set_sample() -> (Document, NodeId, NodeId, [NodeId; 2], [NodeId; 2]) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let set = container_at(&mut d, page, NodeKind::ComponentSet(FrameProps::default()), Rect::new(0.0, 0.0, 440.0, 200.0));
        let v1 = container_at(&mut d, set, NodeKind::Component(FrameProps::default()), Rect::new(0.0, 0.0, 200.0, 200.0));
        let var2 = container_at(&mut d, set, NodeKind::Component(FrameProps::default()), Rect::new(240.0, 0.0, 200.0, 200.0));
        let l1 = rect_at(&mut d, v1, Rect::new(10.0, 10.0, 30.0, 30.0));
        let l2 = rect_at(&mut d, var2, Rect::new(10.0, 10.0, 30.0, 30.0));
        (d, page, set, [v1, var2], [l1, l2])
    }

    /// Marquee results with and without a scene index, which must agree.
    fn marquee_both_ways(d: &Document, page: NodeId, r: &Rect) -> Vec<NodeId> {
        let plain = marquee_select(d, page, r);
        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(d, page, 1);
        assert_eq!(marquee_select_with_index(d, page, r, Some(&idx)), plain, "indexed and plain marquee must agree");
        plain
    }

    #[test]
    fn partial_marquee_selects_a_main_component_as_a_whole() {
        let (d, page, comp, _near, _far) = main_component_sample();
        // covers only the layer at (10,10)
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(0.0, 0.0, 60.0, 60.0)), vec![comp]);
        // covers only empty component area
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(60.0, 60.0, 30.0, 30.0)), vec![comp]);
        // fully enclosing it selects it as well
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(-10.0, -10.0, 400.0, 400.0)), vec![comp]);
        // missing it selects nothing
        assert!(marquee_both_ways(&d, page, &Rect::new(300.0, 300.0, 40.0, 40.0)).is_empty());
    }

    #[test]
    fn partial_marquee_selects_a_component_set_as_a_whole() {
        let (d, page, set, _variants, _layers) = component_set_sample();
        // crosses both variants and their layers
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(0.0, 0.0, 300.0, 60.0)), vec![set]);
        // touches only the second variant's layer
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(240.0, 0.0, 60.0, 60.0)), vec![set]);
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(-10.0, -10.0, 600.0, 400.0)), vec![set]);
    }

    #[test]
    fn marquee_over_a_frame_and_a_main_component_enters_only_the_frame() {
        let (mut d, page, comp, _near, _far) = main_component_sample();
        let frame = container_at(&mut d, page, NodeKind::Frame(FrameProps::default()), Rect::new(300.0, 0.0, 200.0, 200.0));
        let inner = rect_at(&mut d, frame, Rect::new(10.0, 10.0, 30.0, 30.0));
        // partly covers the component and, by its layer, the frame
        let hits = marquee_both_ways(&d, page, &Rect::new(0.0, 0.0, 330.0, 60.0));
        assert_eq!(hits, vec![comp, inner]);
    }

    #[test]
    fn partial_marquee_selects_a_main_component_inside_a_frame_as_a_whole() {
        let (mut d, page, ..) = main_component_sample();
        let frame = container_at(&mut d, page, NodeKind::Frame(FrameProps::default()), Rect::new(0.0, 400.0, 400.0, 300.0));
        let nested = container_at(&mut d, frame, NodeKind::Component(FrameProps::default()), Rect::new(20.0, 20.0, 200.0, 200.0));
        let _layer = rect_at(&mut d, nested, Rect::new(10.0, 10.0, 30.0, 30.0));
        // enters the partly covered frame, then takes the partly covered component whole
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(0.0, 400.0, 60.0, 60.0)), vec![nested]);
    }

    #[test]
    fn clicks_still_reach_the_layers_of_a_main_component() {
        let (d, page, comp, near, far) = main_component_sample();
        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&d, page, 1);
        for index in [None, Some(&idx)] {
            // plain click, deep (Cmd+click) and a double-click (descend through the entered main) all land on the layer
            assert_eq!(hit_test_with_index(&d, page, v2(20.0, 20.0), 0.0, false, &[], index), Some(near));
            assert_eq!(hit_test_with_index(&d, page, v2(20.0, 20.0), 0.0, true, &[], index), Some(near));
            assert_eq!(hit_test_with_index(&d, page, v2(150.0, 150.0), 0.0, false, &[comp], index), Some(far));
            // empty component area is the component itself
            assert_eq!(hit_test_with_index(&d, page, v2(80.0, 80.0), 0.0, false, &[], index), Some(comp));
        }
    }

    #[test]
    fn one_shape_component_selects_component_on_click_and_shape_when_entered_while_marquee_is_unchanged() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let comp = container_at(&mut d, page, NodeKind::Component(FrameProps::default()), Rect::new(0.0, 0.0, 200.0, 200.0));
        let shape = rect_at(&mut d, comp, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert!(is_one_shape_component(&d, d.get(comp).unwrap()));

        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&d, page, 1);
        for index in [None, Some(&idx)] {
            assert_eq!(hit_test_with_index(&d, page, v2(50.0, 50.0), 0.0, false, &[], index), Some(comp), "single click selects one-shape component");
            assert_eq!(hit_test_with_index(&d, page, v2(50.0, 50.0), 0.0, false, &[comp], index), Some(shape), "entered (double-click) selects shape");
            assert_eq!(hit_test_with_index(&d, page, v2(50.0, 50.0), 0.0, true, &[], index), Some(shape), "deep (Cmd+click) selects shape");
        }
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(0.0, 0.0, 60.0, 60.0)), vec![comp], "partial marquee selects component whole");
        assert_eq!(marquee_both_ways(&d, page, &Rect::new(-10.0, -10.0, 400.0, 400.0)), vec![comp], "enclosing marquee selects component whole");
    }
}
