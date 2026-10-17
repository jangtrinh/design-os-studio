//! Writes the shared auto-layout solver ([`crate::layout_solver`]) straight into a [`Document`].

use crate::layout_solver::{compute, ChildIn, SizeLimits};
use crate::{Document, LayoutDirection, NodeId, Vec2};

impl Document {
    /// Re-solves auto layout bottom-up from `root`: every container below it first, so hugging
    /// sizes bubble up, then `root` itself. A child whose size the parent changed (Fill, grid
    /// stretch, min/max) is re-solved once more so its own children follow.
    pub fn relayout_subtree(&mut self, root: NodeId) {
        let Some(n) = self.get(root) else { return };
        let children = n.children.clone();
        for c in &children {
            self.relayout_subtree(*c);
        }
        let Some(root_node) = self.get(root) else { return };
        let Some(al) = root_node.kind.frame_props().and_then(|f| f.auto_layout) else {
            return;
        };
        let frame = root_node.size;
        let own_limits = SizeLimits::from(&root_node.layout_child);
        let horiz = al.direction != LayoutDirection::Vertical;
        let (flow_ids, kids): (Vec<NodeId>, Vec<ChildIn>) = children
            .iter()
            .filter_map(|cid| {
                let k = self.get(*cid)?;
                (k.visible && !k.layout_child.absolute).then(|| (*cid, ChildIn::from_layout_child(k.size, &k.layout_child, horiz)))
            })
            .unzip();

        let out = compute(&al, frame, own_limits, &kids);
        if let Some(fnode) = self.get_mut(root) {
            fnode.size = out.frame;
        }
        for (cid, r) in flow_ids.iter().zip(&out.rects) {
            let new_size = Vec2::new(r.w, r.h);
            let size_changed = self.get(*cid).is_some_and(|n| (n.size.x - new_size.x).abs() > 1e-3 || (n.size.y - new_size.y).abs() > 1e-3);
            if let Some(cnode) = self.get_mut(*cid) {
                cnode.transform.tx = r.x;
                cnode.transform.ty = r.y;
                cnode.size = new_size;
            }
            if size_changed {
                self.relayout_subtree(*cid);
            }
        }
    }
}
