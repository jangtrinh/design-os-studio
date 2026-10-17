//! Auto-layout, constraints and group fitting.
//!
//! The solver itself is `designos_doc::layout_solver::compute`, a pure function shared with the
//! headless `Document::relayout_subtree`; the `layout_*`/`relayout_*` functions here apply it to a
//! [`Document`] through an [`Edit`] session so every touched node is recorded for undo.

use crate::edit::Edit;
use crate::util::{aabb_of, rect_corners, v2};
use designos_doc::{resolve_node, Constraint, Document, LayoutDirection, NodeId, NodeKind, Rect, Sizing, TextAutoResize, TextProps, Vec2};

use designos_doc::layout_solver::{compute, ChildIn, SizeLimits};

/// Measured text extent when the text hugs its content (no wrapping).
pub fn estimate_text_size(t: &TextProps) -> Vec2 {
    crate::textmeasure::measure(t, None)
}

/// New node size after a text edit, following the node's auto-resize mode.
pub fn fit_text(t: &TextProps, size: Vec2) -> Vec2 {
    match t.auto_resize {
        TextAutoResize::WidthAndHeight => estimate_text_size(t),
        TextAutoResize::Height => v2(size.x, crate::textmeasure::measure(t, Some(size.x.max(1.0))).y),
        _ => size,
    }
}

/// Lays out `id` and every container below it (children first so hugging
/// sizes bubble up), writing through `ed`.
pub fn layout_node(doc: &mut Document, ed: &mut Edit, id: NodeId) {
    let Some(n) = doc.get(id) else { return };
    let children = n.children.clone();
    let large_doc = doc.nodes.len() > 5_000;
    for c in &children {
        let should_recurse = doc.get(*c).is_some_and(|k| {
            if !k.kind.is_container() {
                return false;
            }
            if !large_doc {
                return true;
            }
            matches!(k.kind, NodeKind::Group)
                || k.layout_child.horizontal == Sizing::Hug
                || k.layout_child.vertical == Sizing::Hug
                || k.kind
                    .frame_props()
                    .and_then(|f| f.auto_layout.as_ref())
                    .is_some_and(|al| al.primary_sizing == Sizing::Hug || al.counter_sizing == Sizing::Hug)
        });
        if should_recurse {
            layout_node(doc, ed, *c);
        }
    }
    if matches!(doc.get(id).map(|n| &n.kind), Some(NodeKind::Group)) {
        fit_group(doc, ed, id);
        return;
    }
    layout_self(doc, ed, id);
}

/// Solves auto-layout for `id` using its direct children's current sizes,
/// recursing into a child container only if `id` changed that child's size (e.g. `Sizing::Fill`).
/// Returns `true` if `id`'s own outer `size` changed.
fn layout_self(doc: &mut Document, ed: &mut Edit, id: NodeId) -> bool {
    let Some(n) = doc.get(id) else { return false };
    let children = n.children.clone();
    // gap / padding bound to variables count with the value of the mode in effect
    let Some(al) = doc.get(id).map(|n| resolve_node(doc, n)).and_then(|n| n.kind.frame_props().and_then(|f| f.auto_layout)) else {
        return false;
    };
    let horiz = al.direction != LayoutDirection::Vertical;
    // the one reading of a flow child (variable-bound sizes, content-sized axes) shared with the headless relayout
    let (ids, kids): (Vec<NodeId>, Vec<ChildIn>) = children.iter().filter_map(|c| doc.flow_child_input(*c, horiz).map(|k| (*c, k))).unzip();
    let frame_size = doc.get(id).unwrap().size;
    let own_limits = SizeLimits::from(&doc.get(id).unwrap().layout_child);
    let out = compute(&al, frame_size, own_limits, &kids);
    let new_frame = out.frame;
    let frame_changed = (new_frame.x - frame_size.x).abs() > 1e-3 || (new_frame.y - frame_size.y).abs() > 1e-3;
    if frame_changed {
        if let Some(n) = ed.node(doc, id) {
            n.size = new_frame;
        }
    }
    for (cid, r) in ids.iter().zip(out.rects.iter()) {
        let n = doc.get(*cid).unwrap();
        let new_size = v2(r.w, r.h);
        let size_changed = (n.size.x - new_size.x).abs() > 1e-3 || (n.size.y - new_size.y).abs() > 1e-3;
        let pos_changed = (n.transform.tx - r.x).abs() > 1e-3 || (n.transform.ty - r.y).abs() > 1e-3;
        if size_changed || pos_changed {
            if let Some(n) = ed.node(doc, *cid) {
                n.transform.tx = r.x;
                n.transform.ty = r.y;
                n.size = new_size;
            }
        }
        if size_changed && doc.get(*cid).map(|n| n.kind.is_container()).unwrap_or(false) {
            layout_node(doc, ed, *cid);
        }
    }
    frame_changed
}

/// Re-fits a group's box to its children (group transform follows the content).
/// Returns `true` if the group's outer transform or size changed.
pub fn fit_group(doc: &mut Document, ed: &mut Edit, id: NodeId) -> bool {
    let Some(g) = doc.get(id) else { return false };
    let mut pts = Vec::new();
    for c in &g.children {
        if let Some(k) = doc.get(*c) {
            let r = Rect::new(0.0, 0.0, k.size.x, k.size.y);
            pts.extend(rect_corners(&r).map(|p| k.transform.apply(p)));
        }
    }
    if pts.is_empty() {
        return false;
    }
    let bb = aabb_of(&pts);
    let size = v2(bb.w, bb.h);
    if bb.x.abs() < 1e-3 && bb.y.abs() < 1e-3 && (g.size.x - size.x).abs() < 1e-3 && (g.size.y - size.y).abs() < 1e-3 {
        return false;
    }
    let children = g.children.clone();
    let shift = designos_doc::Affine::translate(bb.x, bb.y);
    if let Some(g) = ed.node(doc, id) {
        g.transform = g.transform * shift;
        g.size = size;
    }
    for c in children {
        if let Some(k) = ed.node(doc, c) {
            k.transform.tx -= bb.x;
            k.transform.ty -= bb.y;
        }
    }
    true
}

/// After any edit on `start`, re-run layout/fit on its ancestors using Figma's
/// `GroupsAndStacksObserver` (`apply_groups_and_stacks_layout`) bottom-up frontier
/// propagation with Fixed-box boundary cutoff.
pub fn relayout_up(doc: &mut Document, ed: &mut Edit, start: NodeId) {
    // on a board, connectors follow the moved node live (every move, resize and rotate comes through here)
    crate::board::connect::reroute_touched(doc, ed);
    let mut cur = doc.get(start).and_then(|n| n.parent);
    while let Some(p) = cur {
        let is_group = matches!(doc.get(p).map(|n| &n.kind), Some(NodeKind::Group));
        let is_al = doc.get(p).and_then(|n| n.kind.frame_props()).map(|f| f.auto_layout.is_some()).unwrap_or(false);
        let box_changed = if is_group {
            fit_group(doc, ed, p)
        } else if is_al {
            layout_self(doc, ed, p)
        } else {
            false
        };
        if !box_changed {
            break;
        }
        cur = doc.get(p).and_then(|n| n.parent);
    }
}

/// Convenience: lay out `id`'s subtree then fix its ancestors.
pub fn relayout_around(doc: &mut Document, ed: &mut Edit, id: NodeId) {
    layout_node(doc, ed, id);
    relayout_up(doc, ed, id);
}

/// Applies children constraints when a (non auto-layout) frame is resized.
pub fn apply_constraints(doc: &mut Document, ed: &mut Edit, frame: NodeId, old: Vec2, new: Vec2) {
    let Some(f) = doc.get(frame) else { return };
    if f.kind.frame_props().map(|p| p.auto_layout.is_some()).unwrap_or(false) || matches!(f.kind, NodeKind::Group) {
        return;
    }
    let dw = new.x - old.x;
    let dh = new.y - old.y;
    if dw.abs() < 1e-4 && dh.abs() < 1e-4 {
        return;
    }
    for c in f.children.clone() {
        let Some(k) = doc.get(c) else { continue };
        if k.transform.b.abs() > 1e-4 || k.transform.c.abs() > 1e-4 {
            continue;
        }
        let (mut x, mut w) = (k.transform.tx, k.size.x);
        let (mut y, mut h) = (k.transform.ty, k.size.y);
        let old_child = k.size;
        let cons = k.constraints;
        let rx = if old.x > 0.0 { new.x / old.x } else { 1.0 };
        let ry = if old.y > 0.0 { new.y / old.y } else { 1.0 };
        match cons.horizontal {
            Constraint::Min => {}
            Constraint::Max => x += dw,
            Constraint::Center => x += dw / 2.0,
            Constraint::Stretch => w += dw,
            Constraint::Scale => {
                x *= rx;
                w *= rx;
            }
        }
        match cons.vertical {
            Constraint::Min => {}
            Constraint::Max => y += dh,
            Constraint::Center => y += dh / 2.0,
            Constraint::Stretch => h += dh,
            Constraint::Scale => {
                y *= ry;
                h *= ry;
            }
        }
        let w = w.max(1.0);
        let h = h.max(1.0);
        if let Some(k) = ed.node(doc, c) {
            k.transform.tx = x;
            k.transform.ty = y;
            k.size = v2(w, h);
        }
        if (w - old_child.x).abs() > 1e-4 || (h - old_child.y).abs() > 1e-4 {
            apply_constraints(doc, ed, c, old_child, v2(w, h));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{AutoLayout, CounterAlignContent, FrameProps, LayoutAlign, Node};

    fn kid(w: f32, h: f32) -> ChildIn {
        ChildIn { size: v2(w, h), ..Default::default() }
    }
    /// The solver for a frame with no min/max of its own.
    fn compute(al: &AutoLayout, frame: Vec2, kids: &[ChildIn]) -> designos_doc::layout_solver::LayoutOut {
        super::compute(al, frame, SizeLimits::default(), kids)
    }
    fn al() -> AutoLayout {
        AutoLayout { item_spacing: 10.0, padding: [5.0, 5.0, 5.0, 5.0], ..Default::default() }
    }

    #[test]
    fn horizontal_start_fixed() {
        let out = compute(&al(), v2(300.0, 100.0), &[kid(50.0, 20.0), kid(30.0, 40.0)]);
        assert_eq!(out.rects[0], Rect::new(5.0, 5.0, 50.0, 20.0));
        assert_eq!(out.rects[1], Rect::new(65.0, 5.0, 30.0, 40.0));
        assert_eq!(out.frame, v2(300.0, 100.0));
    }

    #[test]
    fn hug_both_axes() {
        let mut a = al();
        a.primary_sizing = Sizing::Hug;
        a.counter_sizing = Sizing::Hug;
        let out = compute(&a, v2(0.0, 0.0), &[kid(50.0, 20.0), kid(30.0, 40.0)]);
        assert_eq!(out.frame, v2(50.0 + 30.0 + 10.0 + 10.0, 40.0 + 10.0));
    }

    #[test]
    fn vertical_center_alignment() {
        let mut a = al();
        a.direction = LayoutDirection::Vertical;
        a.counter_align = LayoutAlign::Center;
        a.primary_align = LayoutAlign::Center;
        let out = compute(&a, v2(100.0, 100.0), &[kid(40.0, 20.0)]);
        assert_eq!(out.rects[0].x, 30.0); // (100-10 - 40)/2 + 5
        assert_eq!(out.rects[0].y, 40.0); // (90 - 20)/2 + 5
    }

    #[test]
    fn fill_children_share_space() {
        let mut k1 = kid(10.0, 10.0);
        k1.main_fill = true;
        let mut k2 = kid(10.0, 10.0);
        k2.main_fill = true;
        let k3 = kid(40.0, 10.0);
        let out = compute(&al(), v2(200.0, 50.0), &[k1, k2, k3]);
        // avail = 190 - 20 (spacing) - 40 = 130 -> 65 each
        assert_eq!(out.rects[0].w, 65.0);
        assert_eq!(out.rects[1].w, 65.0);
        assert_eq!(out.rects[2].x, 5.0 + 65.0 + 10.0 + 65.0 + 10.0);
    }

    #[test]
    fn cross_fill_stretches() {
        let mut k = kid(10.0, 10.0);
        k.cross_fill = true;
        let out = compute(&al(), v2(100.0, 80.0), &[k]);
        assert_eq!(out.rects[0].h, 70.0);
    }

    #[test]
    fn space_between() {
        let mut a = al();
        a.primary_align = LayoutAlign::SpaceBetween;
        a.padding = [0.0; 4];
        let out = compute(&a, v2(100.0, 20.0), &[kid(10.0, 10.0), kid(10.0, 10.0), kid(10.0, 10.0)]);
        assert_eq!(out.rects[0].x, 0.0);
        assert_eq!(out.rects[1].x, 45.0);
        assert_eq!(out.rects[2].x, 90.0);
    }

    #[test]
    fn wrap_breaks_lines() {
        let mut a = al();
        a.wrap = true;
        a.counter_spacing = 4.0;
        a.padding = [0.0; 4];
        let out = compute(&a, v2(75.0, 100.0), &[kid(30.0, 10.0), kid(30.0, 20.0), kid(30.0, 10.0)]);
        assert_eq!(out.rects[1].x, 40.0);
        assert_eq!(out.rects[2].x, 0.0);
        assert_eq!(out.rects[2].y, 24.0); // line 1 cross = 20, + counter spacing 4
    }

    /// Frame 120 wide, four 10-wide items, no padding or gap.
    fn spaced(align: LayoutAlign, horizontal: bool) -> Vec<f32> {
        let mut a = al();
        a.primary_align = align;
        a.padding = [0.0; 4];
        a.item_spacing = 0.0;
        a.direction = if horizontal { LayoutDirection::Horizontal } else { LayoutDirection::Vertical };
        let frame = if horizontal { v2(120.0, 20.0) } else { v2(20.0, 120.0) };
        let out = compute(&a, frame, &[kid(10.0, 10.0), kid(10.0, 10.0), kid(10.0, 10.0), kid(10.0, 10.0)]);
        out.rects.iter().map(|r| if horizontal { r.x } else { r.y }).collect()
    }

    #[test]
    fn space_around_gives_each_item_equal_space_on_both_sides() {
        // free = 120 - 4*10 = 80; per item 80/4 = 20, so 10 before the first item and 10 after the last
        for horizontal in [true, false] {
            assert_eq!(spaced(LayoutAlign::SpaceAround, horizontal), vec![10.0, 40.0, 70.0, 100.0], "horizontal={horizontal}");
        }
    }

    #[test]
    fn space_evenly_makes_every_gap_equal_edges_included() {
        // free = 80 over 5 gaps (4 items + 1) = 16: items at 16, 16+10+16, ...
        for horizontal in [true, false] {
            assert_eq!(spaced(LayoutAlign::SpaceEvenly, horizontal), vec![16.0, 42.0, 68.0, 94.0], "horizontal={horizontal}");
        }
    }

    #[test]
    fn space_around_and_evenly_center_a_single_item() {
        // free = 100 - 20 = 80: around 80/1 -> 40 before; evenly 80/2 = 40 before
        for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly] {
            let mut a = al();
            a.primary_align = align;
            a.padding = [0.0; 4];
            let out = compute(&a, v2(100.0, 20.0), &[kid(20.0, 10.0)]);
            assert_eq!(out.rects[0].x, 40.0, "{align:?}");
        }
    }

    #[test]
    fn space_around_and_evenly_fall_back_to_start_when_items_overflow() {
        // 30 + 30 > 50: no free space, items pack from the start with no gap
        for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly] {
            let mut a = al();
            a.primary_align = align;
            a.padding = [0.0; 4];
            let out = compute(&a, v2(50.0, 20.0), &[kid(30.0, 10.0), kid(30.0, 10.0)]);
            assert_eq!((out.rects[0].x, out.rects[1].x), (0.0, 30.0), "{align:?}");
        }
    }

    #[test]
    fn space_around_respects_padding_and_ignores_the_fixed_gap() {
        let mut a = al();
        a.primary_align = LayoutAlign::SpaceAround;
        a.padding = [0.0, 5.0, 0.0, 5.0];
        a.item_spacing = 99.0;
        // inner width 130 - 10 = 120, same as the unpadded case shifted by the left padding
        let out = compute(&a, v2(130.0, 20.0), &[kid(10.0, 10.0), kid(10.0, 10.0), kid(10.0, 10.0), kid(10.0, 10.0)]);
        assert_eq!(out.rects.iter().map(|r| r.x).collect::<Vec<_>>(), vec![15.0, 45.0, 75.0, 105.0]);
    }

    #[test]
    fn space_around_applies_to_each_wrapped_line() {
        let mut a = al();
        a.primary_align = LayoutAlign::SpaceAround;
        a.padding = [0.0; 4];
        a.item_spacing = 0.0;
        a.wrap = true;
        // 30 + 30 = 60 <= 80 < 90: line 1 holds two items (free 20 -> 10 each: x = 5, 45); line 2 one item (free 50 -> x = 25)
        let out = compute(&a, v2(80.0, 100.0), &[kid(30.0, 10.0), kid(30.0, 10.0), kid(30.0, 10.0)]);
        assert_eq!(out.rects.iter().map(|r| r.x).collect::<Vec<_>>(), vec![5.0, 45.0, 25.0]);
    }

    /// Vertical wrap: A and B fill the first column (40 + 10 + 40 = 90 <= 100), C does not fit under them.
    fn vertical_wrap() -> AutoLayout {
        AutoLayout { direction: LayoutDirection::Vertical, wrap: true, item_spacing: 10.0, counter_spacing: 6.0, padding: [0.0; 4], ..Default::default() }
    }
    fn vertical_kids() -> [ChildIn; 4] {
        [kid(30.0, 40.0), kid(20.0, 40.0), kid(25.0, 40.0), kid(10.0, 30.0)]
    }

    #[test]
    fn vertical_wrap_flows_down_then_into_new_columns() {
        let out = compute(&vertical_wrap(), v2(200.0, 100.0), &vertical_kids());
        // column 1 is 30 wide (A), so column 2 starts at 30 + counter spacing 6 = 36; D sits under C at 40 + 10
        assert_eq!(out.rects[0], Rect::new(0.0, 0.0, 30.0, 40.0));
        assert_eq!(out.rects[1], Rect::new(0.0, 50.0, 20.0, 40.0));
        assert_eq!(out.rects[2], Rect::new(36.0, 0.0, 25.0, 40.0));
        assert_eq!(out.rects[3], Rect::new(36.0, 50.0, 10.0, 30.0));
        assert_eq!(out.frame, v2(200.0, 100.0));
    }

    #[test]
    fn vertical_wrap_hugs_the_columns_across() {
        let mut a = vertical_wrap();
        a.counter_sizing = Sizing::Hug;
        // columns 30 and 25 wide + 6 between
        let out = compute(&a, v2(0.0, 100.0), &vertical_kids());
        assert_eq!(out.frame, v2(61.0, 100.0));
    }

    #[test]
    fn vertical_wrap_with_zero_counter_spacing_packs_columns() {
        let mut a = vertical_wrap();
        a.counter_spacing = 0.0;
        let out = compute(&a, v2(200.0, 100.0), &vertical_kids());
        assert_eq!(out.rects[2].x, 30.0);
    }

    #[test]
    fn vertical_wrap_one_item_taller_than_the_frame_gets_its_own_column() {
        let out = compute(&vertical_wrap(), v2(200.0, 100.0), &[kid(30.0, 150.0), kid(20.0, 20.0)]);
        assert_eq!(out.rects[0], Rect::new(0.0, 0.0, 30.0, 150.0));
        assert_eq!(out.rects[1], Rect::new(36.0, 0.0, 20.0, 20.0));
    }

    #[test]
    fn counter_space_between_spreads_wrapped_horizontal_lines() {
        let mut a = al();
        a.wrap = true;
        a.padding = [0.0; 4];
        a.item_spacing = 0.0;
        a.counter_spacing = 0.0;
        a.counter_align_content = CounterAlignContent::SpaceBetween;
        // 40 + 40 = 80 fits in 100, a third does not: lines of cross 10 and 10; free 100 - 20 = 80 -> line 2 at 10 + 80
        let kids = [kid(40.0, 10.0), kid(40.0, 10.0), kid(40.0, 10.0)];
        assert_eq!(compute(&a, v2(100.0, 100.0), &kids).rects[2].y, 90.0);
        a.counter_align_content = CounterAlignContent::Auto;
        assert_eq!(compute(&a, v2(100.0, 100.0), &kids).rects[2].y, 10.0);
    }

    #[test]
    fn counter_space_between_spreads_wrapped_vertical_columns() {
        let mut a = vertical_wrap();
        a.counter_align_content = CounterAlignContent::SpaceBetween;
        // columns 30 and 25 wide in 200: free 145 over one gap -> column 2 at 30 + 145 = 175
        let out = compute(&a, v2(200.0, 100.0), &vertical_kids());
        assert_eq!((out.rects[2].x, out.rects[3].x), (175.0, 175.0));
        assert_eq!((out.rects[0].x, out.rects[1].x), (0.0, 0.0));
    }

    #[test]
    fn counter_space_between_with_three_unequal_lines() {
        let mut a = al();
        a.wrap = true;
        a.padding = [0.0; 4];
        a.item_spacing = 0.0;
        a.counter_spacing = 0.0;
        a.counter_align_content = CounterAlignContent::SpaceBetween;
        // widths 60 each, frame 60 wide: one item per line; crosses 10, 20, 10 = 40; free 100 - 40 = 60 over 2 gaps = 30
        let out = compute(&a, v2(60.0, 100.0), &[kid(60.0, 10.0), kid(60.0, 20.0), kid(60.0, 10.0)]);
        assert_eq!(out.rects.iter().map(|r| r.y).collect::<Vec<_>>(), vec![0.0, 40.0, 90.0]);
    }

    #[test]
    fn counter_space_between_keeps_the_counter_spacing_as_a_minimum() {
        let mut a = al();
        a.wrap = true;
        a.padding = [0.0; 4];
        a.item_spacing = 0.0;
        a.counter_spacing = 40.0;
        a.counter_align_content = CounterAlignContent::SpaceBetween;
        // free per gap would be (100 - 30) / 2 = 35 < 40: the lines overflow at the minimum gap
        let out = compute(&a, v2(60.0, 100.0), &[kid(60.0, 10.0), kid(60.0, 10.0), kid(60.0, 10.0)]);
        assert_eq!(out.rects.iter().map(|r| r.y).collect::<Vec<_>>(), vec![0.0, 50.0, 100.0]);
    }

    #[test]
    fn counter_space_between_with_a_single_line_stays_at_the_start() {
        let mut a = al();
        a.wrap = true;
        a.padding = [0.0; 4];
        a.counter_align_content = CounterAlignContent::SpaceBetween;
        let out = compute(&a, v2(200.0, 100.0), &[kid(40.0, 10.0)]);
        assert_eq!(out.rects[0].y, 0.0);
    }

    #[test]
    fn empty_hug_collapses_to_padding() {
        let mut a = al();
        a.primary_sizing = Sizing::Hug;
        a.counter_sizing = Sizing::Hug;
        assert_eq!(compute(&a, v2(99.0, 99.0), &[]).frame, v2(10.0, 10.0));
    }

    fn doc_with_frame() -> (Document, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 10.0, 10.0));
        f.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, item_spacing: 8.0, ..Default::default() }),
            ..Default::default()
        });
        let f = d.insert(page, f).unwrap();
        let a = d.alloc_id();
        let a = d.insert(f, Node::rectangle(a, Rect::new(100.0, 100.0, 20.0, 20.0))).unwrap();
        let b = d.alloc_id();
        let b = d.insert(f, Node::rectangle(b, Rect::new(0.0, 0.0, 30.0, 10.0))).unwrap();
        (d, f, a, b)
    }

    #[test]
    fn applies_to_document_and_hugs() {
        let (mut d, f, a, b) = doc_with_frame();
        let mut ed = Edit::new();
        layout_node(&mut d, &mut ed, f);
        assert_eq!(d.get(f).unwrap().size, v2(58.0, 20.0));
        assert_eq!(d.get(a).unwrap().transform.tx, 0.0);
        assert_eq!(d.get(b).unwrap().transform.tx, 28.0);
        // resizing a child re-hugs the parent through relayout_up
        ed.node(&mut d, a).unwrap().size = v2(40.0, 20.0);
        relayout_up(&mut d, &mut ed, a);
        assert_eq!(d.get(f).unwrap().size.x, 78.0);
    }

    #[test]
    fn gap_bound_to_a_variable_follows_the_mode() {
        use designos_doc::{VariableKind, VariableStore, VariableValue};
        let (mut d, f, _, b) = doc_with_frame();
        let mut s = VariableStore::default();
        let c = s.add_collection("Space");
        let light = s.collections[&c].default_mode.clone();
        let dark = s.add_mode(&c, "Wide").unwrap();
        let gap = s.add_variable(&c, "Gap", VariableKind::Number).unwrap();
        s.set_value(&gap, &light, VariableValue::Number(8.0));
        s.set_value(&gap, &dark, VariableValue::Number(20.0));
        d.variables = s.variables;
        d.variable_collections = s.collections;
        d.get_mut(f).unwrap().variables.bindings.insert("itemSpacing".into(), gap);
        let mut ed = Edit::new();
        layout_node(&mut d, &mut ed, f);
        assert_eq!(d.get(b).unwrap().transform.tx, 28.0);
        d.get_mut(f).unwrap().variables.modes.insert(c, dark);
        layout_node(&mut d, &mut ed, f);
        assert_eq!(d.get(b).unwrap().transform.tx, 40.0);
    }

    #[test]
    fn group_fit_follows_children() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let gid = d.alloc_id();
        let mut g = Node::new(gid, NodeKind::Group);
        g.size = v2(1.0, 1.0);
        let g = d.insert(page, g).unwrap();
        let rid = d.alloc_id();
        let r = d.insert(g, Node::rectangle(rid, Rect::new(10.0, 20.0, 30.0, 40.0))).unwrap();
        let mut ed = Edit::new();
        fit_group(&mut d, &mut ed, g);
        assert_eq!(d.get(g).unwrap().size, v2(30.0, 40.0));
        assert_eq!(d.get(g).unwrap().transform.tx, 10.0);
        assert_eq!(d.get(r).unwrap().transform.tx, 0.0);
    }

    #[test]
    fn constraints_on_resize() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let r = d.insert(f, Node::rectangle(rid, Rect::new(80.0, 10.0, 10.0, 10.0))).unwrap();
        {
            let n = d.get_mut(r).unwrap();
            n.constraints.horizontal = Constraint::Max;
            n.constraints.vertical = Constraint::Stretch;
        }
        let mut ed = Edit::new();
        apply_constraints(&mut d, &mut ed, f, v2(100.0, 100.0), v2(150.0, 120.0));
        let n = d.get(r).unwrap();
        assert_eq!(n.transform.tx, 130.0);
        assert_eq!(n.size.y, 30.0);
    }

    #[test]
    fn text_estimate_scales_with_content() {
        let t = |s: &str| TextProps {
            content: s.into(),
            style: Default::default(),
            runs: vec![],
            auto_resize: Default::default(),
            max_lines: None,
            ..Default::default()
        };
        assert!(estimate_text_size(&t("hello world")).x > estimate_text_size(&t("hi")).x);
        assert!(estimate_text_size(&t("a\nb")).y > estimate_text_size(&t("a")).y);
    }
}

#[cfg(test)]
mod solver_parity_tests;
