//! Writes the shared auto-layout solver ([`crate::layout_solver`]) straight into a [`Document`].

use crate::layout_solver::{compute, ChildIn, SizeLimits};
use crate::{Document, LayoutDirection, NodeId, Vec2};

impl Document {
    /// Re-solves auto layout bottom-up from `root`: every container below it first, so hugging
    /// sizes bubble up, then `root` itself. A child whose size the parent changed (Fill, grid
    /// stretch, min/max) is re-solved top-down within [`MAX_RELAYOUT_RESOLVE_DEPTH`] and
    /// [`MAX_RELAYOUT_RESOLVES`] so its own children follow without unbounded recursion.
    pub fn relayout_subtree(&mut self, root: NodeId) {
        let mut budget = MAX_RELAYOUT_RESOLVES;
        self.relayout_bottom_up(root, 0, &mut budget);
    }

    fn relayout_bottom_up(&mut self, root: NodeId, depth: usize, budget: &mut usize) {
        let Some(n) = self.get(root) else { return };
        if !n.kind.is_container() {
            return;
        }
        let children = n.children.clone();
        for c in &children {
            if self.get(*c).is_some_and(|k| k.kind.is_container()) {
                self.relayout_bottom_up(*c, depth + 1, budget);
            }
        }
        self.solve_auto_layout_node(root, depth, budget);
    }

    fn relayout_top_down(&mut self, root: NodeId, depth: usize, budget: &mut usize) {
        if depth > MAX_RELAYOUT_RESOLVE_DEPTH || *budget == 0 {
            return;
        }
        *budget -= 1;
        #[cfg(test)]
        RESOLVE_CALLS.with(|c| c.set(c.get() + 1));
        self.solve_auto_layout_node(root, depth, budget);
    }

    fn solve_auto_layout_node(&mut self, root: NodeId, depth: usize, budget: &mut usize) {
        let Some(root_node) = self.get(root) else { return };
        let Some(al) = root_node.kind.frame_props().and_then(|f| f.auto_layout) else {
            return;
        };
        let children = root_node.children.clone();
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
            let (size_changed, is_auto_layout) = self
                .get(*cid)
                .map(|n| {
                    let changed = (n.size.x - new_size.x).abs() > 1e-3 || (n.size.y - new_size.y).abs() > 1e-3;
                    let has_al = n.kind.frame_props().and_then(|f| f.auto_layout).is_some();
                    (changed, has_al)
                })
                .unwrap_or((false, false));
            if let Some(cnode) = self.get_mut(*cid) {
                cnode.transform.tx = r.x;
                cnode.transform.ty = r.y;
                cnode.size = new_size;
            }
            if size_changed && is_auto_layout {
                self.relayout_top_down(*cid, depth + 1, budget);
            }
        }
    }
}

/// Maximum nesting depth for top-down child re-solves triggered when a parent changes a child's box.
pub const MAX_RELAYOUT_RESOLVE_DEPTH: usize = 32;

/// Maximum number of top-down child re-solves allowed in a single [`Document::relayout_subtree`] pass.
pub const MAX_RELAYOUT_RESOLVES: usize = 4_096;

#[cfg(test)]
thread_local! {
    static RESOLVE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apply_variable_mode_to_subtree, bind_node_variable, combine_as_variants, create_instance, swap_instance_variant, AutoLayout, BindableProperty,
        FrameProps, LayoutChild, Node, NodeKind, Rect, Sizing, Variable, VariableCollection, VariableKind, VariableMode, VariableValue,
    };
    use std::time::Instant;

    fn reset_resolve_calls() {
        RESOLVE_CALLS.with(|c| c.set(0));
    }

    fn resolve_calls() -> usize {
        RESOLVE_CALLS.with(std::cell::Cell::get)
    }

    fn fill_chain(doc: &mut Document, parent: NodeId, depth: usize) -> Vec<NodeId> {
        let mut ids = Vec::with_capacity(depth);
        let mut cur = parent;
        for _ in 0..depth {
            let id = doc.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 0.0, 50.0, 40.0));
            if let NodeKind::Frame(fp) = &mut f.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    ..AutoLayout::default()
                });
            }
            f.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, ..LayoutChild::default() };
            doc.insert(cur, f).unwrap();
            ids.push(id);
            cur = id;
        }
        ids
    }

    #[test]
    fn changelog_callers_on_50k_nodes_respect_recursive_resolve_budget() {
        let mut doc = Document::new("50k-relayout-budget");
        let page = doc.add_page("Page 1");

        // Variable collection with two modes so `apply_variable_mode_to_subtree` resizes `mode_root`.
        doc.variable_collections.insert(
            "col-layout".into(),
            VariableCollection {
                id: "col-layout".into(),
                name: "Layout".into(),
                modes: vec![VariableMode { id: "compact".into(), name: "Compact".into() }, VariableMode { id: "wide".into(), name: "Wide".into() }],
                default_mode: "compact".into(),
                variables: vec!["var-root-w".into()],
                key: None,
                source_library_key: None,
            },
        );
        doc.variables.insert(
            "var-root-w".into(),
            Variable {
                id: "var-root-w".into(),
                name: "root/width".into(),
                collection: "col-layout".into(),
                kind: VariableKind::Number,
                values: [("compact".to_string(), VariableValue::Number(320.0)), ("wide".to_string(), VariableValue::Number(640.0))].into_iter().collect(),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );

        let mode_root = doc.alloc_id();
        let mut root_frame = Node::frame(mode_root, Rect::new(0.0, 0.0, 320.0, 400.0));
        if let NodeKind::Frame(fp) = &mut root_frame.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                item_spacing: 4.0,
                ..AutoLayout::default()
            });
        }
        doc.insert(page, root_frame).unwrap();
        bind_node_variable(&mut doc, mode_root, BindableProperty::Width, "var-root-w").unwrap();

        // Depth-40 nested Fill chain under `mode_root` (exceeds `MAX_RELAYOUT_RESOLVE_DEPTH = 32`).
        let mode_chain = fill_chain(&mut doc, mode_root, 40);

        // ComponentSet with two differently sized variants + instance inside an auto-layout parent with a depth-40 Fill chain.
        let comp_sm = doc.alloc_id();
        let mut n_sm = Node::new(comp_sm, NodeKind::Component(FrameProps { auto_layout: Some(AutoLayout::default()), ..FrameProps::default() }));
        n_sm.name = "Size=Sm".into();
        n_sm.size = Vec2::new(120.0, 48.0);
        doc.insert(page, n_sm).unwrap();

        let comp_lg = doc.alloc_id();
        let mut n_lg = Node::new(comp_lg, NodeKind::Component(FrameProps { auto_layout: Some(AutoLayout::default()), ..FrameProps::default() }));
        n_lg.name = "Size=Lg".into();
        n_lg.size = Vec2::new(280.0, 64.0);
        doc.insert(page, n_lg).unwrap();
        combine_as_variants(&mut doc, &[comp_sm, comp_lg]).apply(&mut doc).unwrap();

        let swap_host = doc.alloc_id();
        let mut swap_frame = Node::frame(swap_host, Rect::new(0.0, 500.0, 200.0, 100.0));
        if let NodeKind::Frame(fp) = &mut swap_frame.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Hug,
                item_spacing: 8.0,
                ..AutoLayout::default()
            });
        }
        doc.insert(page, swap_frame).unwrap();
        create_instance(&mut doc, comp_sm, swap_host, Vec2::ZERO).apply(&mut doc).unwrap();
        let inst_id = *doc.get(swap_host).unwrap().children.last().unwrap();
        let swap_chain = fill_chain(&mut doc, swap_host, 40);

        // Populate remaining Fill auto-layout rows and leaf nodes under `mode_root` to reach 50,000 nodes.
        while doc.nodes.len() < 50_000 {
            let row_id = doc.alloc_id();
            let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 300.0, 24.0));
            if let NodeKind::Frame(fp) = &mut row.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    item_spacing: 4.0,
                    ..AutoLayout::default()
                });
            }
            row.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fixed, ..LayoutChild::default() };
            doc.insert(mode_root, row).unwrap();
            for i in 0..9 {
                if doc.nodes.len() >= 50_000 {
                    break;
                }
                let leaf_id = doc.alloc_id();
                doc.insert(row_id, Node::rectangle(leaf_id, Rect::new((i * 6) as f32, 0.0, 5.0, 16.0))).unwrap();
            }
        }
        assert_eq!(doc.nodes.len(), 50_000);

        // Make initial layout consistent before timing the two callers.
        doc.relayout_subtree(mode_root);
        doc.relayout_subtree(swap_host);

        // 1. Mode switch caller (`apply_variable_mode_to_subtree`) on the 50k-node document.
        reset_resolve_calls();
        let t0 = Instant::now();
        apply_variable_mode_to_subtree(&mut doc, mode_root, "col-layout", "wide").unwrap();
        let mode_switch_elapsed = t0.elapsed();
        let mode_resolves = resolve_calls();
        eprintln!("50k-node bench: apply_variable_mode_to_subtree = {:.2} ms (re-solves = {mode_resolves})", mode_switch_elapsed.as_secs_f64() * 1000.0);
        assert!(mode_resolves <= MAX_RELAYOUT_RESOLVES, "mode switch re-solves ({mode_resolves}) exceeded budget ({MAX_RELAYOUT_RESOLVES})");
        assert_eq!(doc.get(mode_chain[MAX_RELAYOUT_RESOLVE_DEPTH]).unwrap().size.x, 640.0);
        assert_eq!(doc.get(*mode_chain.last().unwrap()).unwrap().size.x, 50.0, "nodes deeper than MAX_RELAYOUT_RESOLVE_DEPTH (32) must not be re-solved");

        // 2. Variant swap caller (`swap_instance_variant`) on the 50k-node document.
        reset_resolve_calls();
        let t1 = Instant::now();
        swap_instance_variant(&mut doc, inst_id, "Size", "Lg").unwrap();
        let variant_swap_elapsed = t1.elapsed();
        let swap_resolves = resolve_calls();
        eprintln!("50k-node bench: swap_instance_variant = {:.2} ms (re-solves = {swap_resolves})", variant_swap_elapsed.as_secs_f64() * 1000.0);
        assert!(
            swap_resolves <= MAX_RELAYOUT_RESOLVE_DEPTH,
            "variant swap chain re-solves ({swap_resolves}) exceeded depth budget ({MAX_RELAYOUT_RESOLVE_DEPTH})"
        );
        assert_eq!(doc.get(swap_chain[MAX_RELAYOUT_RESOLVE_DEPTH]).unwrap().size.y, 64.0);
        assert_eq!(doc.get(*swap_chain.last().unwrap()).unwrap().size.y, 40.0, "nodes deeper than MAX_RELAYOUT_RESOLVE_DEPTH (32) must not be re-solved");
    }
}
