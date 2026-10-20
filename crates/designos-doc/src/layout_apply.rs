//! Writes the shared auto-layout solver ([`crate::layout_solver`]) straight into a [`Document`].

use crate::layout_solver::{compute_with_stroke, hugged_axes, ChildIn, SizeLimits};
use crate::{Document, LayoutDirection, Node, NodeId, NodeKind, Vec2};

impl Document {
    /// Re-solves auto layout bottom-up from `root`: every container below it first, so hugging
    /// sizes bubble up, then `root` itself. A child whose size the parent changed (Fill, grid
    /// stretch, min/max) is re-solved top-down within [`MAX_RELAYOUT_RESOLVE_DEPTH`] hops from the
    /// changed node and [`MAX_RELAYOUT_RESOLVES`] total re-solves; any subtree past either limit
    /// is queued in `pending_relayout` so it can be reported and finished lazily.
    pub fn relayout_subtree(&mut self, root: NodeId) {
        let mut budget = MAX_RELAYOUT_RESOLVES;
        self.relayout_bottom_up(root, &mut budget);
    }

    /// Re-solves only the auto-layout frames in `seeds`, each once, deepest first, and then climbs:
    /// after a frame is solved, its auto-layout parent is solved too only when the frame's own size
    /// changed (a hugging frame bubbling a new size up). The climb stops at the first frame whose
    /// size held, or at the first ancestor that is not auto layout. Unlike [`Self::relayout_subtree`]
    /// this never walks the containers below a seed, so untouched siblings keep the positions they
    /// have. One [`MAX_RELAYOUT_RESOLVES`] budget covers the whole call; a subtree past it is
    /// queued in `pending_relayout` exactly as for [`Self::relayout_subtree`].
    pub(crate) fn relayout_seeds_upward(&mut self, seeds: &[NodeId]) {
        let depth = |doc: &Document, id: NodeId| {
            let mut d = 0usize;
            let mut cur = doc.get(id).and_then(|n| n.parent);
            while let Some(p) = cur {
                d += 1;
                cur = doc.get(p).and_then(|n| n.parent);
            }
            d
        };
        let mut order: Vec<(usize, NodeId)> = Vec::with_capacity(seeds.len());
        for &id in seeds {
            if self.is_auto_layout_frame(id) && !order.iter().any(|(_, o)| *o == id) {
                order.push((depth(self, id), id));
            }
        }
        // Deepest first, so a container is solved after the children whose new sizes it reads.
        order.sort_by_key(|(d, _)| std::cmp::Reverse(*d));

        let mut budget = MAX_RELAYOUT_RESOLVES;
        for (_, seed) in order {
            let mut cur = seed;
            loop {
                let before = self.get(cur).map(|n| n.size);
                self.solve_auto_layout_node(cur, 0, &mut budget);
                let moved = match (before, self.get(cur).map(|n| n.size)) {
                    (Some(b), Some(a)) => (b.x - a.x).abs() > 1e-3 || (b.y - a.y).abs() > 1e-3,
                    _ => false,
                };
                let parent = self.get(cur).and_then(|n| n.parent);
                match parent.filter(|p| moved && self.is_auto_layout_frame(*p)) {
                    Some(p) => cur = p,
                    None => break,
                }
            }
        }
    }

    fn is_auto_layout_frame(&self, id: NodeId) -> bool {
        self.get(id).and_then(|n| n.kind.frame_props()).is_some_and(|f| f.auto_layout.is_some())
    }

    /// What a flow frame's solver reads of child `id`: `None` when the child sits outside the flow
    /// (hidden by its visibility or variable, or absolutely positioned), otherwise its box with
    /// variable bindings applied for the mode in effect. The editor and the headless relayout both
    /// read children through this, so a bound size means the same everywhere.
    ///
    /// A `width` / `height` binding does not apply on an axis the node sizes from its own content:
    /// a group always fits its children, and an auto-layout frame hugging that axis takes its
    /// content's extent. The stored size there is already the fitted one, so the parent places the
    /// next sibling behind it instead of behind a bound size the node would shed again.
    pub fn flow_child_input(&self, id: NodeId, horiz: bool) -> Option<ChildIn> {
        let raw = self.get(id)?;
        let bound = crate::resolve_node(self, raw);
        if !bound.visible || raw.layout_child.absolute {
            return None;
        }
        let (fit_w, fit_h) = content_sized_axes(raw);
        let natural = raw.layout_child.natural_size(raw.size, horiz);
        let has_w_bind = raw.variables.bindings.contains_key("width");
        let has_h_bind = raw.variables.bindings.contains_key("height");
        let size = Vec2::new(
            if fit_w {
                raw.size.x
            } else if has_w_bind {
                bound.size.x
            } else {
                natural.x
            },
            if fit_h {
                raw.size.y
            } else if has_h_bind {
                bound.size.y
            } else {
                natural.y
            },
        );
        Some(ChildIn::from_layout_child(size, &bound.layout_child, horiz))
    }

    fn relayout_bottom_up(&mut self, root: NodeId, budget: &mut usize) {
        let Some(n) = self.get(root) else { return };
        if !n.kind.is_container() {
            return;
        }
        let children = n.children.clone();
        for c in &children {
            if self.get(*c).is_some_and(|k| k.kind.is_container()) {
                self.relayout_bottom_up(*c, budget);
            }
        }
        // A container queued by an earlier pass is solved here like any other, at the size its
        // parent already wrote: skipping it would leave it queued forever, because a repeat pass
        // finds its size unchanged and never takes the top-down path to it again.
        self.solve_auto_layout_node(root, 0, budget);
    }

    fn relayout_top_down(&mut self, root: NodeId, hops: usize, budget: &mut usize) {
        if hops > MAX_RELAYOUT_RESOLVE_DEPTH || *budget == 0 {
            if !self.pending_relayout.contains(&root) {
                self.pending_relayout.push(root);
            }
            return;
        }
        *budget -= 1;
        #[cfg(test)]
        RESOLVE_CALLS.with(|c| c.set(c.get() + 1));
        self.solve_auto_layout_node(root, hops, budget);
    }

    fn solve_auto_layout_node(&mut self, root: NodeId, hops: usize, budget: &mut usize) {
        #[cfg(test)]
        SOLVE_CALLS.with(|c| c.set(c.get() + 1));
        if !self.pending_relayout.is_empty() {
            self.pending_relayout.retain(|&id| id != root);
        }
        let Some(root_node) = self.get(root) else { return };
        let resolved = crate::resolve_node(self, root_node);
        let Some(al) = resolved.kind.frame_props().and_then(|f| f.auto_layout.clone()) else {
            return;
        };
        let children = root_node.children.clone();
        let frame = root_node.size;
        let own_limits = SizeLimits::from(&root_node.layout_child);
        let stroke = resolved.stroke.clone();
        let horiz = al.direction != LayoutDirection::Vertical;
        for cid in &children {
            if let Some(cnode) = self.get_mut(*cid) {
                if let Some(old_horiz) = cnode.layout_child.stretched.last_parent_horiz {
                    cnode.layout_child.apply_parent_direction_change(old_horiz, horiz);
                }
            }
        }
        let (flow_ids, kids): (Vec<NodeId>, Vec<ChildIn>) = children.iter().filter_map(|cid| self.flow_child_input(*cid, horiz).map(|k| (*cid, k))).unzip();

        let out = compute_with_stroke(&al, frame, own_limits, &stroke, &kids);
        if let Some(fnode) = self.get_mut(root) {
            fnode.size = out.frame;
        }
        for ((cid, kid), r) in flow_ids.iter().zip(&kids).zip(&out.rects) {
            let new_size = Vec2::new(r.w, r.h);
            let (size_changed, is_auto_layout) = self
                .get(*cid)
                .map(|n| {
                    let changed = (n.size.x - new_size.x).abs() > 1e-3 || (n.size.y - new_size.y).abs() > 1e-3;
                    let has_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some();
                    (changed, has_al)
                })
                .unwrap_or((false, false));
            if let Some(cnode) = self.get_mut(*cid) {
                cnode.transform.tx = r.x;
                cnode.transform.ty = r.y;
                cnode.size = new_size;
                cnode.layout_child.record_stretched(kid.size, new_size, horiz, kid.stretches_cross());
            }
            if size_changed && is_auto_layout {
                self.relayout_top_down(*cid, hops + 1, budget);
            }
        }
    }

    /// Number of auto-layout subtrees whose box changed during [`Self::relayout_subtree`] but
    /// whose own children were deferred past [`MAX_RELAYOUT_RESOLVES`] or
    /// [`MAX_RELAYOUT_RESOLVE_DEPTH`].
    pub fn relayout_skipped_count(&self) -> usize {
        self.pending_relayout.len()
    }

    /// Queued subtree roots waiting for top-down re-layout.
    pub fn pending_relayout(&self) -> &[NodeId] {
        &self.pending_relayout
    }

    /// User-facing status/toast summary when [`Self::relayout_skipped_count`] is non-zero; it
    /// always starts with [`RELAYOUT_NOTICE_PREFIX`].
    pub fn relayout_notice(&self) -> Option<String> {
        let n = self.pending_relayout.len();
        if n == 0 {
            None
        } else if n == 1 {
            Some(format!("{RELAYOUT_NOTICE_PREFIX}: 1 subtree skipped past budget"))
        } else {
            Some(format!("{RELAYOUT_NOTICE_PREFIX}: {n} subtrees skipped past budget"))
        }
    }

    /// Runs one bounded top-down pass (`MAX_RELAYOUT_RESOLVES` re-solves, up to
    /// `MAX_RELAYOUT_RESOLVE_DEPTH` hops per queued root) over queued subtrees and returns the
    /// number of subtrees still remaining in the queue.
    pub fn flush_pending_relayout_step(&mut self) -> usize {
        if self.pending_relayout.is_empty() {
            return 0;
        }
        let queued = std::mem::take(&mut self.pending_relayout);
        let mut budget = MAX_RELAYOUT_RESOLVES;
        for id in queued {
            if budget == 0 {
                if !self.pending_relayout.contains(&id) {
                    self.pending_relayout.push(id);
                }
            } else {
                self.relayout_top_down(id, 1, &mut budget);
            }
        }
        self.pending_relayout.len()
    }

    /// Finishes every queued subtree in bounded passes and returns the number of steps executed.
    ///
    /// A step advances a chain of nested frames by at most [`MAX_RELAYOUT_RESOLVE_DEPTH`] hops, so
    /// how many steps a nest needs grows with its depth: the loop runs until the queue is empty
    /// instead of for a fixed number of steps. Every step solves at least the first queued root, so
    /// the queue keeps changing; a step that leaves it untouched would spin forever, so that
    /// stops the loop rather than hang the caller. The subtrees stay queued and counted by
    /// [`Self::relayout_skipped_count`], which a caller that must not go on with a stale layout
    /// checks after the flush; debug builds treat the stalled step as a bug.
    pub fn flush_pending_relayout(&mut self) -> usize {
        let mut steps = 0;
        while !self.pending_relayout.is_empty() {
            let before = self.pending_relayout.clone();
            self.flush_pending_relayout_step();
            steps += 1;
            if self.pending_relayout == before {
                debug_assert!(false, "a relayout step left the queued subtrees untouched");
                break;
            }
        }
        steps
    }
}

/// The `(width, height)` axes along which layout derives `node`'s size from its content.
fn content_sized_axes(node: &Node) -> (bool, bool) {
    if matches!(node.kind, NodeKind::Group) {
        return (true, true);
    }
    node.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map_or((false, false), hugged_axes)
}

/// What every [`Document::relayout_notice`] starts with, so a caller can recognise (and retire) a
/// toast that reported the notice.
pub const RELAYOUT_NOTICE_PREFIX: &str = "Layout incomplete";

/// Maximum number of top-down hops from the node whose size changed before deferring deeper
/// child re-solves to [`Document::pending_relayout`].
pub const MAX_RELAYOUT_RESOLVE_DEPTH: usize = 32;

/// Maximum number of top-down child re-solves allowed in a single [`Document::relayout_subtree`] pass.
pub const MAX_RELAYOUT_RESOLVES: usize = 4_096;

#[cfg(test)]
thread_local! {
    static RESOLVE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Every `solve_auto_layout_node` entry: bottom-up, top-down and the undo climb alike.
    static SOLVE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Resets the per-thread count of auto-layout frames solved.
#[cfg(test)]
pub(crate) fn reset_solve_calls() {
    SOLVE_CALLS.with(|c| c.set(0));
}

/// Auto-layout frames solved on this thread since [`reset_solve_calls`].
#[cfg(test)]
pub(crate) fn solve_calls() -> usize {
    SOLVE_CALLS.with(std::cell::Cell::get)
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
                extensions: Default::default(),
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
                extensions: Default::default(),
                mode_extensions: Default::default(),
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

        // Make initial layout consistent before timing the two callers: a pass that stops at the
        // depth budget leaves the rest queued, and the next pass would finish it inside the
        // timed call.
        doc.relayout_subtree(mode_root);
        doc.relayout_subtree(swap_host);
        doc.flush_pending_relayout();
        assert_eq!(doc.relayout_skipped_count(), 0, "precondition: the baseline layout is complete");
        let mode_chain_end_before = doc.get(*mode_chain.last().unwrap()).unwrap().size.x;
        assert_ne!(mode_chain_end_before, 640.0, "precondition: the end of the chain still has the compact mode's width");
        let swap_chain_end_before = doc.get(*swap_chain.last().unwrap()).unwrap().size.y;
        assert_ne!(swap_chain_end_before, 64.0, "precondition: the end of the chain still has the small variant's height");

        // 1. Mode switch caller (`apply_variable_mode_to_subtree`) on the 50k-node document.
        reset_resolve_calls();
        let t0 = Instant::now();
        apply_variable_mode_to_subtree(&mut doc, mode_root, "col-layout", "wide").unwrap();
        let mode_switch_elapsed = t0.elapsed();
        let mode_resolves = resolve_calls();
        eprintln!("50k-node bench: apply_variable_mode_to_subtree = {:.2} ms (re-solves = {mode_resolves})", mode_switch_elapsed.as_secs_f64() * 1000.0);
        assert!(mode_resolves <= MAX_RELAYOUT_RESOLVES, "mode switch re-solves ({mode_resolves}) exceeded budget ({MAX_RELAYOUT_RESOLVES})");
        assert_eq!(doc.get(mode_chain[MAX_RELAYOUT_RESOLVE_DEPTH]).unwrap().size.x, 640.0);
        assert_eq!(
            doc.get(*mode_chain.last().unwrap()).unwrap().size.x,
            mode_chain_end_before,
            "nodes deeper than MAX_RELAYOUT_RESOLVE_DEPTH (32) must not be re-solved"
        );

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
        assert_eq!(
            doc.get(*swap_chain.last().unwrap()).unwrap().size.y,
            swap_chain_end_before,
            "nodes deeper than MAX_RELAYOUT_RESOLVE_DEPTH (32) must not be re-solved"
        );
        assert!(doc.relayout_skipped_count() > 0, "skipped subtrees past budget/depth must be counted and not dropped silently");
        doc.flush_pending_relayout();
        assert_eq!(doc.get(*mode_chain.last().unwrap()).unwrap().size.x, 640.0);
        assert_eq!(doc.get(*swap_chain.last().unwrap()).unwrap().size.y, 64.0);
        assert_eq!(doc.relayout_skipped_count(), 0);
    }

    /// A vertical frame of 4,097 centred rows, each holding one 20 px rectangle at x = 40.
    /// Widening the frame to 500 moves every rectangle to x = 240; the 4,096 re-solve budget
    /// leaves the last row for the lazy pass.
    fn rows_doc() -> (Document, NodeId, Vec<NodeId>) {
        let mut doc = Document::new("4097-rows");
        let page = doc.add_page("Page 1");
        let parent = doc.alloc_id();
        let mut pf = Node::frame(parent, Rect::new(0.0, 0.0, 100.0, 2000.0));
        if let NodeKind::Frame(fp) = &mut pf.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                ..AutoLayout::default()
            });
        }
        doc.insert(page, pf).unwrap();

        let mut rects = Vec::with_capacity(4_097);
        for _ in 0..4_097 {
            let row = doc.alloc_id();
            let mut rf = Node::frame(row, Rect::new(0.0, 0.0, 100.0, 20.0));
            if let NodeKind::Frame(fp) = &mut rf.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    primary_align: crate::LayoutAlign::Center,
                    ..AutoLayout::default()
                });
            }
            rf.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fixed, ..LayoutChild::default() };
            doc.insert(parent, rf).unwrap();

            let r = doc.alloc_id();
            doc.insert(row, Node::rectangle(r, Rect::new(40.0, 0.0, 20.0, 20.0))).unwrap();
            rects.push(r);
        }
        (doc, parent, rects)
    }

    #[test]
    fn repro_4097_rows_reports_skipped_and_flushes_remaining_row() {
        let (mut doc, parent, rects) = rows_doc();

        doc.get_mut(parent).unwrap().size.x = 500.0;
        doc.relayout_subtree(parent);

        for &r in &rects[..4_096] {
            assert_eq!(doc.get(r).unwrap().transform.tx, 240.0);
        }
        assert_eq!(doc.relayout_skipped_count(), 1, "row 4096 must be reported as skipped past MAX_RELAYOUT_RESOLVES");
        assert!(doc.relayout_notice().is_some(), "relayout_notice must surface the skipped subtree count");

        doc.flush_pending_relayout();
        assert_eq!(doc.get(rects[4_096]).unwrap().transform.tx, 240.0, "flushing pending relayout must finish row 4096");
        assert_eq!(doc.relayout_skipped_count(), 0);
    }

    #[test]
    fn a_second_relayout_repairs_a_row_the_first_one_left_queued() {
        let (mut doc, parent, rects) = rows_doc();
        doc.get_mut(parent).unwrap().size.x = 500.0;
        doc.relayout_subtree(parent);
        let last = rects[4_096];
        assert_eq!(doc.relayout_skipped_count(), 1, "precondition: the last row is queued");
        assert_eq!(doc.get(last).unwrap().transform.tx, 40.0, "precondition: its rectangle is not centred yet");

        // The parent already wrote the row's new size, so the second pass sees no size change and
        // must repair the row in its own bottom-up solve.
        doc.relayout_subtree(parent);
        assert_eq!(doc.get(last).unwrap().transform.tx, 240.0, "a queued row must be solved by the next relayout, not skipped again");
        assert_eq!(doc.relayout_skipped_count(), 0, "a solved row leaves the queue");
    }

    #[test]
    fn repro_33_nested_frames_under_page_counts_hops_from_changed_node() {
        let mut doc = Document::new("33-nested-frames");
        let page = doc.add_page("Page 1");

        // 33 nested horizontal auto-layout frames under `page` (frames 0..=32):
        // outer frame 0 is fixed 400x40 with `primary_align: End`, frames 1..=32 are `Fill`.
        let mut cur = page;
        let mut frames = Vec::with_capacity(33);
        for i in 0..33 {
            let fid = doc.alloc_id();
            let init_w = if i == 0 { 400.0 } else { 50.0 };
            let mut f = Node::frame(fid, Rect::new(0.0, 0.0, init_w, 40.0));
            if let NodeKind::Frame(fp) = &mut f.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    primary_align: crate::LayoutAlign::End,
                    ..AutoLayout::default()
                });
            }
            if i > 0 {
                f.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, ..LayoutChild::default() };
            }
            doc.insert(cur, f).unwrap();
            frames.push(fid);
            cur = fid;
        }
        let leaf = doc.alloc_id();
        doc.insert(frames[32], Node::rectangle(leaf, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();

        doc.relayout_subtree(page);
        assert_eq!(doc.get(frames[32]).unwrap().size, Vec2::new(400.0, 40.0));
        assert_eq!(doc.get(leaf).unwrap().transform.tx, 390.0, "frame 32 is 32 hops from frame 0 (not 33 from page) so its 10x10 child must sit at x = 390");
        assert_eq!(doc.relayout_skipped_count(), 0);

        // Now add a 34th frame (frame 33, 33 hops from frame 0) and resize frame 0 to 500:
        // hop 33 exceeds MAX_RELAYOUT_RESOLVE_DEPTH (32), so it is counted as skipped and finished on flush.
        doc.remove(leaf).unwrap();
        let frame_33 = doc.alloc_id();
        let mut f33 = Node::frame(frame_33, Rect::new(0.0, 0.0, 50.0, 40.0));
        if let NodeKind::Frame(fp) = &mut f33.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                primary_align: crate::LayoutAlign::End,
                ..AutoLayout::default()
            });
        }
        f33.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, ..LayoutChild::default() };
        doc.insert(frames[32], f33).unwrap();
        let deep_leaf = doc.alloc_id();
        doc.insert(frame_33, Node::rectangle(deep_leaf, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();

        doc.get_mut(frames[0]).unwrap().size.x = 500.0;
        doc.relayout_subtree(page);
        assert_eq!(doc.relayout_skipped_count(), 1, "hop 33 must be counted as skipped");
        doc.flush_pending_relayout();
        assert_eq!(doc.get(deep_leaf).unwrap().transform.tx, 490.0);
        assert_eq!(doc.relayout_skipped_count(), 0);
    }

    #[test]
    fn flush_pending_relayout_finishes_a_nest_deeper_than_sixty_four_steps() {
        // Every lazy step advances a chain by at most MAX_RELAYOUT_RESOLVE_DEPTH hops, so 2,200
        // nested Fill frames need 69 steps: more than the 64 the old flush allowed.
        let depth = 2_200;
        let mut doc = Document::new("deep-nest");
        let page = doc.add_page("Page 1");
        let mut frames = Vec::with_capacity(depth);
        let mut cur = page;
        for i in 0..depth {
            let id = doc.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 40.0));
            if let NodeKind::Frame(fp) = &mut f.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    ..AutoLayout::default()
                });
            }
            if i > 0 {
                f.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, ..LayoutChild::default() };
            }
            doc.insert(cur, f).unwrap();
            frames.push(id);
            cur = id;
        }
        doc.relayout_subtree(frames[0]);
        assert_eq!(doc.relayout_skipped_count(), 0, "a consistent nest has nothing to defer");

        doc.get_mut(frames[0]).unwrap().size.x = 600.0;
        doc.relayout_subtree(frames[0]);
        assert!(doc.relayout_skipped_count() > 0, "the nest is deeper than one relayout reaches");

        doc.flush_pending_relayout();
        assert_eq!(doc.relayout_skipped_count(), 0, "a flush that returns must leave nothing queued");
        assert!(doc.relayout_notice().is_none());
        assert_eq!(doc.get(*frames.last().unwrap()).unwrap().size.x, 600.0, "the innermost frame follows the outermost");
    }

    #[test]
    fn relayout_subtree_uses_resolved_bound_stroke_weight_and_side_weights_in_version_0() {
        use crate::{Color, Paint, Stroke, StrokeAlign};

        let mut doc = Document::new("bound-stroke-relayout");
        let page = doc.add_page("Page 1");
        doc.variable_collections.insert(
            "col".into(),
            VariableCollection {
                id: "col".into(),
                name: "Border".into(),
                modes: vec![VariableMode { id: "compact".into(), name: "Compact".into() }, VariableMode { id: "spacious".into(), name: "Spacious".into() }],
                default_mode: "compact".into(),
                variables: vec!["v-stroke".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        doc.variables.insert(
            "v-stroke".into(),
            Variable {
                id: "v-stroke".into(),
                name: "border/weight".into(),
                collection: "col".into(),
                kind: VariableKind::Number,
                values: [("compact".to_string(), VariableValue::Number(1.0)), ("spacious".to_string(), VariableValue::Number(20.0))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                code_syntax: Default::default(),
                hidden_from_publishing: false,
                key: None,
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        let fid = doc.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
        f.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                item_spacing: 8.0,
                padding: [10.0, 12.0, 14.0, 16.0],
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Hug,
                stroke_included: true,
                layout_version: 0,
                ..AutoLayout::default()
            }),
            ..FrameProps::default()
        });
        f.stroke = Stroke { paints: vec![Paint::solid(Color::BLACK)], weight: 1.0, align: StrokeAlign::Inside, ..Stroke::default() };
        f.variables.bindings.insert("strokeWeight".into(), "v-stroke".into());
        f.variables.modes.insert("col".into(), "spacious".into());
        let f = doc.insert(page, f).unwrap();

        let c0 = doc.alloc_id();
        let c1 = doc.alloc_id();
        let c0 = doc.insert(f, Node::rectangle(c0, Rect::new(0.0, 0.0, 50.0, 20.0))).unwrap();
        let c1 = doc.insert(f, Node::rectangle(c1, Rect::new(0.0, 0.0, 30.0, 20.0))).unwrap();

        // Stored stroke weight is 1, but the active mode resolves `strokeWeight` to 20:
        // padding [10, 12, 14, 16] + 20 on every side = [30, 32, 34, 36].
        doc.relayout_subtree(f);
        assert_eq!(doc.get(f).unwrap().size, Vec2::new(156.0, 84.0));
        assert_eq!((doc.get(c0).unwrap().transform.tx, doc.get(c0).unwrap().transform.ty), (36.0, 30.0));
        assert_eq!((doc.get(c1).unwrap().transform.tx, doc.get(c1).unwrap().transform.ty), (94.0, 30.0));
    }
}
