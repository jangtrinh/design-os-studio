//! Drag-to-reorder inside auto-layout frames: where a dragged child would be
//! inserted and the line Figma draws between siblings.

use crate::util::*;
use designos_doc::{Document, LayoutDirection, NodeId, Vec2 as DV};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Insertion {
    pub parent: NodeId,
    /// Index in the parent's children with the dragged node removed (what `Command::Reparent` expects).
    pub index: usize,
    /// The indicator line in page space.
    pub line: [DV; 2],
}

/// Computes the insertion slot of `dragged` in auto-layout frame `parent` for a
/// pointer at `page_pt` (page space). `None` if `parent` has no auto-layout.
pub fn insertion_at(doc: &Document, parent: NodeId, dragged: NodeId, page_pt: DV) -> Option<Insertion> {
    let pn = doc.get(parent)?;
    let al = pn.kind.frame_props()?.auto_layout.as_ref()?;
    let horiz = al.direction == LayoutDirection::Horizontal;
    let world = doc.world_transform(parent);
    let p = world.invert()?.apply(page_pt);
    let main_of = |v: DV| if horiz { v.x } else { v.y };

    let others: Vec<NodeId> = pn.children.iter().copied().filter(|c| *c != dragged).collect();
    // (id, start edge, end edge) along the main axis in parent space.
    let sibs: Vec<(NodeId, f32, f32)> = others
        .iter()
        .filter_map(|c| doc.get(*c).filter(|n| n.visible && !n.layout_child.absolute).map(|n| (*c, n)))
        .map(|(c, n)| {
            let pts = rect_corners(&designos_doc::Rect::new(0.0, 0.0, n.size.x, n.size.y)).map(|q| n.transform.apply(q));
            let bb = aabb_of(&pts);
            let (a, b) = if horiz { (bb.x, bb.x + bb.w) } else { (bb.y, bb.y + bb.h) };
            (c, a, b)
        })
        .collect();

    let pm = main_of(v2(p.x, p.y));
    let k = sibs.iter().filter(|(_, a, b)| (a + b) / 2.0 < pm).count();
    let index = if k < sibs.len() {
        others.iter().position(|c| *c == sibs[k].0)?
    } else if let Some((last, _, _)) = sibs.last() {
        others.iter().position(|c| c == last)? + 1
    } else {
        others.len()
    };

    let [pt, pr, pb, pl] = al.padding;
    let (pad_start, pad_end, cross_start, cross_end, main_len) =
        if horiz { (pl, pr, pt, pn.size.y - pb, pn.size.x) } else { (pt, pb, pl, pn.size.x - pr, pn.size.y) };
    let half_gap = al.item_spacing.max(0.0) / 2.0;
    let at = match (k.checked_sub(1).map(|i| sibs[i]), sibs.get(k)) {
        (Some(prev), Some(next)) => (prev.2 + next.1) / 2.0,
        (None, Some(next)) => next.1 - half_gap,
        (Some(prev), None) => prev.2 + half_gap,
        (None, None) => pad_start.min(main_len - pad_end),
    };
    let (a, b) = if horiz { (v2(at, cross_start), v2(at, cross_end)) } else { (v2(cross_start, at), v2(cross_end, at)) };
    Some(Insertion { parent, index, line: [world.apply(a), world.apply(b)] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout;
    use designos_doc::*;

    /// Horizontal auto-layout frame at (100, 50) with three 40x40 children, spacing 10, padding 10.
    fn row() -> (Document, NodeId, [NodeId; 3]) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(100.0, 50.0, 10.0, 10.0));
        f.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                item_spacing: 10.0,
                padding: [10.0; 4],
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Hug,
                ..Default::default()
            }),
            ..Default::default()
        });
        let f = d.insert(page, f).unwrap();
        let mut ids = [NodeId(0); 3];
        for slot in &mut ids {
            let id = d.alloc_id();
            *slot = d.insert(f, Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        }
        let mut ed = crate::edit::Edit::new();
        layout::layout_node(&mut d, &mut ed, f);
        (d, f, ids)
    }

    #[test]
    fn index_follows_pointer_along_main_axis() {
        let (d, f, [a, b, c]) = row();
        // children sit at x = 10, 60, 110 (frame-local) -> page x = 110, 160, 210; centres 130, 180, 230
        let at = |x: f32| insertion_at(&d, f, a, v2(x, 80.0)).unwrap();
        assert_eq!(at(100.0).index, 0, "before b");
        assert_eq!(at(150.0).index, 0);
        assert_eq!(at(200.0).index, 1, "between b and c");
        assert_eq!(at(400.0).index, 2, "after the last sibling");
        // dragging the middle one: its own slot is ignored
        let i = insertion_at(&d, f, b, v2(400.0, 80.0)).unwrap();
        assert_eq!(i.index, 2);
        let _ = c;
    }

    #[test]
    fn indicator_is_a_vertical_line_in_the_gap() {
        let (d, f, [a, _b, _c]) = row();
        // between b (page x 160..200) and c (210..250)
        let i = insertion_at(&d, f, a, v2(205.0, 80.0)).unwrap();
        assert_eq!(i.line[0].x, 205.0);
        assert_eq!(i.line[1].x, 205.0);
        assert_eq!(i.line[0].y, 50.0 + 10.0, "starts at the top padding");
        assert_eq!(i.line[1].y, 50.0 + 60.0 - 10.0, "ends at the bottom padding");
    }

    #[test]
    fn vertical_frames_use_the_y_axis() {
        let (mut d, f, [a, _, _]) = row();
        if let Some(fp) = d.get_mut(f).unwrap().kind.frame_props_mut() {
            fp.auto_layout.as_mut().unwrap().direction = LayoutDirection::Vertical;
        }
        let mut ed = crate::edit::Edit::new();
        layout::layout_node(&mut d, &mut ed, f);
        let i = insertion_at(&d, f, a, v2(120.0, 1000.0)).unwrap();
        assert_eq!(i.index, 2);
        assert_eq!(i.line[0].y, i.line[1].y);
    }

    #[test]
    fn non_auto_layout_parents_have_no_insertion() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let r = d.insert(f, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        assert!(insertion_at(&d, f, r, v2(5.0, 5.0)).is_none());
    }
}
