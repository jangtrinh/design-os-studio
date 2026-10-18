//! Document-level editing operations (selection commands, z-order, grouping,
//! alignment, clipboard, pages, file IO). Everything goes through `Tx`.

use crate::app::{App, CopiedStyle, View};
use crate::canvas::tools::Tool;
use crate::clipboard;
use crate::edit::{Edit, Tx};
use crate::io;
use crate::layout;
use crate::theme;
use crate::util::*;
use designos_doc::*;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    HCenter,
    Right,
    Top,
    VCenter,
    Bottom,
}

/// Sets size + transform of a node, applying constraints/auto-layout to its
/// content and fixing ancestors. Shared by canvas resizing and the inspector.
pub fn apply_resize(doc: &mut Document, ed: &mut Edit, id: NodeId, size: Vec2, transform: Affine, relayout: bool) {
    let Some(old) = doc.get(id).map(|n| n.size) else { return };
    let counter_horiz = doc.get(id).and_then(|n| n.parent).and_then(|p| doc.get(p)).and_then(LayoutChild::counter_axis_is_horizontal);
    let w_changed = (size.x - old.x).abs() > 1e-4;
    let h_changed = (size.y - old.y).abs() > 1e-4;
    if let Some(n) = ed.node(doc, id) {
        n.size = size;
        n.transform = transform;
        if w_changed {
            n.layout_child.set_axis_sizing(true, Sizing::Fixed, counter_horiz);
            if let Some(al) = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                let target = if al.primary_is_horizontal() { &mut al.primary_sizing } else { &mut al.counter_sizing };
                if *target == Sizing::Hug {
                    *target = Sizing::Fixed;
                }
            }
        }
        if h_changed {
            n.layout_child.set_axis_sizing(false, Sizing::Fixed, counter_horiz);
            if let Some(al) = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                let target = if al.primary_is_horizontal() { &mut al.counter_sizing } else { &mut al.primary_sizing };
                if *target == Sizing::Hug {
                    *target = Sizing::Fixed;
                }
            }
        }
        if let NodeKind::Text(t) = &mut n.kind {
            if t.auto_resize == TextAutoResize::WidthAndHeight || (h_changed && t.auto_resize == TextAutoResize::Height) {
                t.auto_resize = TextAutoResize::None;
            }
        }
    }
    if doc.get(id).map(|n| n.kind.is_container()).unwrap_or(false) {
        layout::apply_constraints(doc, ed, id, old, size);
        layout::layout_node(doc, ed, id);
    }
    if relayout {
        layout::relayout_up(doc, ed, id);
    }
}

/// Rotates every `(node, world transform at start)` by `delta` radians about
/// the page-space point `center`, keeping each node's parent untouched.
pub fn rotate_nodes_about(doc: &mut Document, ed: &mut Edit, nodes: &[(NodeId, Affine)], center: Vec2, delta: f32) {
    let rot = Affine::translate(center.x, center.y) * Affine::rotate(delta) * Affine::translate(-center.x, -center.y);
    transform_nodes(doc, ed, nodes, rot);
}

/// Mirrors nodes across the vertical (`horizontal`) or horizontal line through `center`.
pub fn flip_nodes_about(doc: &mut Document, ed: &mut Edit, nodes: &[(NodeId, Affine)], center: Vec2, horizontal: bool) {
    let s = if horizontal { Affine::scale(-1.0, 1.0) } else { Affine::scale(1.0, -1.0) };
    let m = Affine::translate(center.x, center.y) * s * Affine::translate(-center.x, -center.y);
    transform_nodes(doc, ed, nodes, m);
}

/// Applies the page-space transform `rot` to every `(node, world transform at start)`.
fn transform_nodes(doc: &mut Document, ed: &mut Edit, nodes: &[(NodeId, Affine)], rot: Affine) {
    // Compute everything first: relayout of a shared group parent moves its transform.
    let new: Vec<(NodeId, Affine)> = nodes
        .iter()
        .filter_map(|(id, world0)| {
            let pinv = parent_world(doc, *id).invert()?;
            Some((*id, pinv * rot * *world0))
        })
        .collect();
    for (id, t) in &new {
        if let Some(n) = ed.node(doc, *id) {
            n.transform = *t;
        }
    }
    for (id, _) in &new {
        layout::relayout_up(doc, ed, *id);
    }
}

/// The first slot limit broken by taking `leaving` (slot, count) layers out of their slots and
/// putting `entering` layers into one. Parents that are not slots are not limited.
pub(crate) fn slot_edit_error(doc: &Document, leaving: &[(NodeId, usize)], entering: Option<(NodeId, &[&Node])>) -> Option<SlotConstraintError> {
    let into = entering.map(|(slot, _)| slot);
    for &(slot, count) in leaving {
        // A slot that is also entered is judged once, on the count it ends with.
        if Some(slot) != into && is_slot_container(doc, slot) {
            if let Err(err) = check_slot_remove(doc, slot, count) {
                return Some(err);
            }
        }
    }
    let (slot, roots) = entering?;
    if roots.is_empty() || !is_slot_container(doc, slot) {
        return None;
    }
    let removing = leaving.iter().find(|(s, _)| *s == slot).map_or(0, |(_, n)| *n);
    check_slot_replace(doc, slot, removing, roots).err()
}

impl App {
    /// Mirrors the selection across the centre of its bounding box.
    pub fn flip_selection(&mut self, horizontal: bool) {
        let tops = self.top_selection();
        let idx = crate::canvas::interact::active_index(self);
        let Some(bb) = union_all(tops.iter().map(|i| crate::canvas::fast_world_aabb(&self.doc, idx, *i))) else { return };
        let nodes: Vec<(NodeId, Affine)> = tops.iter().map(|i| (*i, self.doc.world_transform(*i))).collect();
        let mut tx = Tx::new();
        flip_nodes_about(&mut self.doc, &mut tx.ed, &nodes, rect_center(&bb), horizontal);
        self.run_tx(tx);
    }

    /// Spaces three or more selected layers evenly along their dominant axis.
    pub fn distribute_selection(&mut self) -> bool {
        let tops = self.top_selection();
        if tops.len() < 3 {
            self.show_toast("Select three or more layers to distribute", false);
            return false;
        }
        let idx = crate::canvas::interact::active_index(self);
        let mut items: Vec<(NodeId, Rect)> = tops.iter().map(|i| (*i, crate::canvas::fast_world_aabb(&self.doc, idx, *i))).collect();
        let bb = union_all(items.iter().map(|i| i.1)).unwrap_or_default();
        let horizontal = bb.w >= bb.h;
        #[allow(clippy::type_complexity)] // pair of fn-pointer axis accessors
        let (start, len): (fn(&Rect) -> f32, fn(&Rect) -> f32) = if horizontal { (|r| r.x, |r| r.w) } else { (|r| r.y, |r| r.h) };
        items.sort_by(|a, b| start(&a.1).partial_cmp(&start(&b.1)).unwrap_or(std::cmp::Ordering::Equal));
        let total: f32 = items.iter().map(|i| len(&i.1)).sum();
        let first = start(&items[0].1);
        let last = start(&items[items.len() - 1].1) + len(&items[items.len() - 1].1);
        let gap = (last - first - total) / (items.len() as f32 - 1.0);
        let mut tx = Tx::new();
        let mut cursor = first;
        for (id, r) in &items {
            let d = cursor - start(r);
            let page_delta = if horizontal { v2(d, 0.0) } else { v2(0.0, d) };
            let dp = delta_to_parent(&self.doc, *id, page_delta);
            if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                n.transform.tx += dp.x;
                n.transform.ty += dp.y;
            }
            cursor += len(r) + gap;
        }
        for (id, _) in &items {
            layout::relayout_up(&mut self.doc, &mut tx.ed, *id);
        }
        self.run_tx(tx);
        true
    }

    /// Rotates the selection by `delta` radians around the centre of its bounding box.
    pub fn rotate_selection(&mut self, delta: f32) {
        let tops = self.top_selection();
        let idx = crate::canvas::interact::active_index(self);
        let Some(bb) = union_all(tops.iter().map(|i| crate::canvas::fast_world_aabb(&self.doc, idx, *i))) else { return };
        let nodes: Vec<(NodeId, Affine)> = tops.iter().map(|i| (*i, self.doc.world_transform(*i))).collect();
        let mut tx = Tx::new();
        rotate_nodes_about(&mut self.doc, &mut tx.ed, &nodes, rect_center(&bb), delta);
        self.run_tx(tx);
    }

    // ------------------------------------------------------------ plumbing
    pub fn run_tx(&mut self, tx: Tx) -> bool {
        if self.history.viewing.is_some() {
            // an old version is shown read-only
            tx.cancel(&mut self.doc);
            self.changed();
            self.show_toast("Viewing an old version is read-only", true);
            return false;
        }
        if self.scrub.is_some() {
            // Scrubbing: edits stay live in the document; `scrub_end` records them as one step.
            self.changed();
            return true;
        }
        match tx.commit(&mut self.doc, &mut self.hist) {
            Ok(changed) => {
                self.changed();
                self.sanitize();
                changed
            }
            Err(e) => {
                self.show_toast(format!("Edit failed: {e}"), true);
                false
            }
        }
    }

    /// Applies `f` to each node and commits one undo step.
    pub fn edit_nodes(&mut self, ids: &[NodeId], mut f: impl FnMut(&mut Node)) {
        let mut tx = Tx::new();
        let mut relayout_roots = Vec::new();
        let mut seen_parents = Vec::new();
        for id in ids {
            let old_horiz = self.doc.get(*id).and_then(|n| n.kind.frame_props()).and_then(|fp| fp.auto_layout.as_ref()).map(AutoLayout::primary_is_horizontal);
            if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                f(n);
            }
            let new_horiz = self.doc.get(*id).and_then(|n| n.kind.frame_props()).and_then(|fp| fp.auto_layout.as_ref()).map(AutoLayout::primary_is_horizontal);
            if let (Some(old_h), Some(new_h)) = (old_horiz, new_horiz) {
                if old_h != new_h {
                    let children = self.doc.get(*id).map(|n| n.children.clone()).unwrap_or_default();
                    for cid in children {
                        if let Some(cn) = tx.ed.node(&mut self.doc, cid) {
                            cn.layout_child.apply_parent_direction_change(old_h, new_h);
                        }
                    }
                }
            }
            if ids.len() <= 4 {
                layout::relayout_around(&mut self.doc, &mut tx.ed, *id);
            } else {
                layout::layout_node(&mut self.doc, &mut tx.ed, *id);
                if let Some(p) = self.doc.get(*id).and_then(|n| n.parent) {
                    if !seen_parents.contains(&p) {
                        seen_parents.push(p);
                        relayout_roots.push(*id);
                    }
                }
            }
        }
        for id in relayout_roots {
            layout::relayout_up(&mut self.doc, &mut tx.ed, id);
        }
        self.run_tx(tx);
    }

    /// Starts/ends a scrub session following the flag set by the numeric fields.
    pub fn scrub_sync(&mut self, ctx: &egui::Context) {
        let id = egui::Id::new(crate::widgets::field::SCRUB_FLAG);
        let mut active = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        if active && !ctx.input(|i| i.pointer.any_down()) {
            active = false;
            ctx.data_mut(|d| d.insert_temp(id, false));
        }
        match (active, self.scrub.is_some()) {
            (true, false) => {
                if self.doc.nodes.len() <= 5_000 {
                    self.scrub = Some(self.doc.nodes.clone());
                } else {
                    let mut snap = std::collections::BTreeMap::new();
                    if let Some(pn) = self.doc.get(self.page) {
                        snap.insert(self.page, pn.clone());
                    }
                    let mut stack: Vec<NodeId> = Vec::new();
                    for &sid in &self.selection {
                        stack.push(sid);
                        let mut cur = self.doc.get(sid).and_then(|n| n.parent);
                        while let Some(p) = cur {
                            if p == self.page {
                                break;
                            }
                            if let Some(pn) = self.doc.get(p) {
                                snap.entry(p).or_insert_with(|| pn.clone());
                                for &sib in pn.children.iter().take(256) {
                                    if let Some(sn) = self.doc.get(sib) {
                                        snap.entry(sib).or_insert_with(|| sn.clone());
                                    }
                                }
                                cur = pn.parent;
                            } else {
                                break;
                            }
                        }
                    }
                    while let Some(nid) = stack.pop() {
                        if snap.len() >= 4_096 {
                            break;
                        }
                        if let Some(n) = self.doc.get(nid) {
                            snap.entry(nid).or_insert_with(|| n.clone());
                            stack.extend(n.children.iter().copied());
                        }
                    }
                    self.scrub = Some(snap);
                }
            }
            (false, true) => self.scrub_end(),
            _ => {}
        }
    }

    /// Folds everything changed since the scrub began into one undo step.
    pub fn scrub_end(&mut self) {
        let Some(base) = self.scrub.take() else { return };
        let changed: Vec<NodeId> = base.iter().filter(|(id, old)| self.doc.nodes.get(id).is_some_and(|cur| cur != *old)).map(|(id, _)| *id).collect();
        let mut updates: Vec<Command> = changed.iter().filter_map(|id| self.doc.get(*id)).map(|n| Command::Update { node: Box::new(n.clone()) }).collect();
        crate::comp_ops::record_overrides(&self.doc, &base, &mut updates);
        for id in &changed {
            if let (Some(old), Some(cur)) = (base.get(id), self.doc.get_mut(*id)) {
                *cur = old.clone();
            }
        }
        if updates.is_empty() {
            return;
        }
        let cmd = if updates.len() == 1 { updates.into_iter().next().unwrap() } else { Command::Batch(updates) };
        let cmd = crate::comp_ops::with_sync(&mut self.doc, cmd);
        if let Err(e) = self.hist.exec(&mut self.doc, cmd) {
            self.show_toast(format!("Edit failed: {e}"), true);
        }
        self.changed();
        self.sanitize();
    }

    pub fn undo(&mut self) {
        if self.try_undo_restore() {
            return;
        }
        match self.hist.undo(&mut self.doc) {
            Ok(true) => {
                self.relayout_moved_geometry();
                self.sanitize();
            }
            Ok(false) => {}
            Err(e) => self.show_toast(format!("Undo failed: {e}"), true),
        }
    }

    pub fn redo(&mut self) {
        if self.try_redo_restore() {
            return;
        }
        match self.hist.redo(&mut self.doc) {
            Ok(true) => {
                self.relayout_moved_geometry();
                self.sanitize();
            }
            Ok(false) => {}
            Err(e) => self.show_toast(format!("Redo failed: {e}"), true),
        }
    }

    /// Selected nodes that have no selected ancestor, in z/document order.
    pub fn top_selection(&self) -> Vec<NodeId> {
        if self.selection.len() <= 1 {
            return self.selection.clone();
        }
        if self.selection.len() <= 8 {
            return self.selection.iter().copied().filter(|id| !self.selection.iter().any(|o| o != id && self.doc.is_ancestor(*o, *id))).collect();
        }
        let set: std::collections::HashSet<NodeId> = self.selection.iter().copied().collect();
        self.selection
            .iter()
            .copied()
            .filter(|&id| {
                let mut cur = self.doc.get(id).and_then(|n| n.parent);
                while let Some(p) = cur {
                    if set.contains(&p) {
                        return false;
                    }
                    cur = self.doc.get(p).and_then(|n| n.parent);
                }
                true
            })
            .collect()
    }

    pub fn selection_bounds(&self) -> Option<Rect> {
        let idx = crate::canvas::interact::active_index(self);
        union_all(self.selection.iter().take(512).map(|id| crate::canvas::fast_world_aabb(&self.doc, idx, *id)))
    }

    pub fn unique_name(&self, base: &str) -> String {
        if self.doc.nodes.len() <= 2_048 {
            let n = self.doc.nodes.values().filter(|n| n.name.starts_with(base)).count();
            return format!("{base} {}", n + 1);
        }
        let container = self.container_for_insert();
        let mut count = 0usize;
        if let Some(cn) = self.doc.get(container) {
            for &cid in cn.children.iter().rev().take(512) {
                if self.doc.get(cid).is_some_and(|c| c.name.starts_with(base)) {
                    count += 1;
                }
            }
        }
        if container != self.page {
            if let Some(pn) = self.doc.get(self.page) {
                for &cid in pn.children.iter().rev().take(512) {
                    if self.doc.get(cid).is_some_and(|c| c.name.starts_with(base)) {
                        count += 1;
                    }
                }
            }
        }
        format!("{base} {}", count + 1)
    }

    pub(crate) fn container_for_insert(&self) -> NodeId {
        if let [only] = self.selection.as_slice() {
            if let Some(n) = self.doc.get(*only) {
                if n.kind.is_container() && !matches!(n.kind, NodeKind::Instance { .. }) {
                    return *only;
                }
                if let Some(p) = n.parent {
                    return p;
                }
            }
        }
        self.page
    }

    /// Records on an instance slot that its content now differs from the main component, so a
    /// later main-component sync leaves the content alone. Call it only after the edit happened.
    pub(crate) fn mark_slot_content_override(doc: &mut Document, ed: &mut Edit, parent: NodeId) {
        let is_inst_slot = is_slot_container(doc, parent) && (doc.get(parent).is_some_and(is_linked) || instance_root_of(doc, parent).is_some());
        if is_inst_slot {
            if let Some(sn) = ed.node(doc, parent) {
                sn.component.overrides.insert(OverrideField::SlotContent);
            }
        }
    }

    /// Existing layers `ids` are about to be parented under `to`: refuses (with a toast) when that
    /// breaks the max, preferred-only or min limits of a slot they enter or leave. Layers that
    /// already sit in `to` only reorder and are not counted.
    pub(crate) fn slot_move_allowed(&mut self, ids: &[NodeId], to: NodeId) -> bool {
        let moving: Vec<&Node> = ids.iter().filter_map(|id| self.doc.get(*id)).filter(|n| n.parent != Some(to)).collect();
        let mut leaving: Vec<(NodeId, usize)> = Vec::new();
        for parent in moving.iter().filter_map(|n| n.parent) {
            match leaving.iter_mut().find(|(slot, _)| *slot == parent) {
                Some((_, count)) => *count += 1,
                None => leaving.push((parent, 1)),
            }
        }
        let err = slot_edit_error(&self.doc, &leaving, Some((to, &moving)));
        self.report_slot_error(err)
    }

    /// New layers (roots of freshly built subtrees) are about to be inserted under `into`:
    /// refuses (with a toast) when that breaks the slot's max or preferred-only limit.
    pub(crate) fn slot_insert_allowed(&mut self, into: NodeId, roots: &[&Node]) -> bool {
        let err = slot_edit_error(&self.doc, &[], Some((into, roots)));
        self.report_slot_error(err)
    }

    /// `removing` of `slot`'s own children are swapped for the new `roots` in one edit (ungroup,
    /// wrap in a group): refuses (with a toast) when the slot would end outside its limits.
    pub(crate) fn slot_swap_allowed(&mut self, slot: NodeId, removing: usize, roots: &[&Node]) -> bool {
        let err = slot_edit_error(&self.doc, &[(slot, removing)], Some((slot, roots)));
        self.report_slot_error(err)
    }

    fn report_slot_error(&mut self, err: Option<SlotConstraintError>) -> bool {
        match err {
            Some(err) => {
                self.show_toast(err.to_string(), true);
                false
            }
            None => true,
        }
    }

    // ------------------------------------------------------------ delete / duplicate
    pub fn delete_selection(&mut self) {
        let tops = self.top_selection();
        if tops.is_empty() {
            return;
        }
        let mut tx = Tx::new();
        let mut parents = Vec::new();
        for id in &tops {
            let parent = self.doc.get(*id).and_then(|n| n.parent);
            let in_slot = parent.is_some_and(|p| is_slot_container(&self.doc, p));
            // Layers inside an instance cannot be removed, only hidden (like Figma) — except direct children of a slot!
            if !in_slot && self.doc.get(*id).is_some_and(|n| is_linked(n) && !is_top_instance(n)) {
                if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                    n.visible = false;
                }
                continue;
            }
            if let Some(p) = parent {
                if in_slot {
                    if let Err(err) = check_slot_remove(&self.doc, p, 1) {
                        self.show_toast(err.to_string(), true);
                        continue;
                    }
                    Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, p);
                }
                if !parents.contains(&p) {
                    parents.push(p);
                }
            }
            if let Err(e) = tx.cmd(&mut self.doc, Command::Remove { id: *id }) {
                self.show_toast(format!("Delete failed: {e}"), true);
            }
        }
        for p in parents {
            if self.doc.get(p).is_some() {
                layout::relayout_around(&mut self.doc, &mut tx.ed, p);
            }
        }
        self.run_tx(tx);
        self.selection.clear();
    }

    pub fn duplicate_selection(&mut self) {
        let tops = self.top_selection();
        if tops.is_empty() {
            return;
        }
        let mut tx = Tx::new();
        let mut new_sel = Vec::new();
        // one clip for the whole selection, so connectors between duplicated nodes join the copies
        let copies = clipboard::capture(&self.doc, &tops).map(|c| clipboard::instantiate(&mut self.doc, &c)).unwrap_or_default();
        for (id, mut nodes) in tops.into_iter().zip(copies) {
            let Some(parent) = self.doc.get(id).and_then(|n| n.parent) else { continue };
            if let Some(root) = nodes.first() {
                if !self.slot_insert_allowed(parent, &[root]) {
                    continue;
                }
            }
            let index = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)).map(|i| i + 1).unwrap_or(usize::MAX);
            let in_al = self.doc.get(parent).and_then(|p| p.kind.frame_props()).map(|f| f.auto_layout.is_some()).unwrap_or(false);
            if let Some(root) = nodes.first_mut() {
                if !in_al {
                    root.transform.tx += theme::PASTE_OFFSET;
                    root.transform.ty += theme::PASTE_OFFSET;
                }
                root.name = self.doc.get(id).map(|n| n.name.clone()).unwrap_or_default();
                apply_slot_stretch_on_insert(&self.doc, parent, root);
                new_sel.push(root.id);
            }
            if tx.cmd(&mut self.doc, Command::Insert { parent, index, nodes }).is_ok() {
                Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, parent);
                layout::relayout_around(&mut self.doc, &mut tx.ed, parent);
            }
        }
        self.run_tx(tx);
        if !new_sel.is_empty() {
            self.selection = new_sel;
        }
    }

    // ------------------------------------------------------------ clipboard
    pub fn copy_selection(&mut self) -> Option<String> {
        let tops = self.top_selection();
        let clip = clipboard::capture(&self.doc, &tops)?;
        let json = clipboard::to_json(&clip);
        self.clip = Some(clip);
        self.paste_count = 0;
        json
    }

    pub fn cut_selection(&mut self) -> Option<String> {
        let s = self.copy_selection();
        self.delete_selection();
        s
    }

    /// Paste with the system clipboard's HTML flavour read through [`App::system_html`].
    pub fn paste_from_system(&mut self, system_text: Option<&str>) {
        let html = (self.system_html)();
        self.paste_with_html(system_text, html.as_deref());
    }

    /// Paste with the system clipboard's HTML flavour at hand: a Figma payload wins over SVG and text.
    pub fn paste_with_html(&mut self, system_text: Option<&str>, system_html: Option<&str>) {
        if system_html.is_some_and(|h| clipboard::paste_figma(self, h)) {
            return;
        }
        self.paste(system_text);
    }

    pub fn paste(&mut self, system_text: Option<&str>) {
        // SVG markup on the system clipboard becomes layers.
        if system_text.is_some_and(|t| clipboard::paste_svg(self, t)) {
            return;
        }
        let clip = system_text.and_then(clipboard::from_json).or_else(|| self.clip.clone());
        let Some(clip) = clip else { return };
        let parent = self.container_for_insert();
        let lists = clipboard::instantiate(&mut self.doc, &clip);
        let roots: Vec<&Node> = lists.iter().filter_map(|l| l.first()).collect();
        if !self.slot_insert_allowed(parent, &roots) {
            return;
        }
        let mut tx = Tx::new();
        let mut new_sel = Vec::new();
        let off = theme::PASTE_OFFSET * self.paste_count as f32;
        for mut nodes in lists {
            if let Some(root) = nodes.first_mut() {
                root.transform.tx += off;
                root.transform.ty += off;
                apply_slot_stretch_on_insert(&self.doc, parent, root);
                new_sel.push(root.id);
            }
            let index = self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
            let _ = tx.cmd(&mut self.doc, Command::Insert { parent, index, nodes });
        }
        Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, parent);
        layout::relayout_around(&mut self.doc, &mut tx.ed, parent);
        self.paste_count += 1;
        self.run_tx(tx);
        self.selection = new_sel;
    }

    /// `Cmd+Shift+V` (`PasteOverSelection`): pastes clipboard nodes directly over the current
    /// selection (or inside the selected container at its top-left `(0, 0)`).
    pub fn paste_over_selection(&mut self) {
        let Some(clip) = self.clip.clone() else { return };
        let parent = self.container_for_insert();
        let (target_local, base_idx) = if let Some(&sel_id) = self.selection.first() {
            if sel_id == parent {
                (v2(0.0, 0.0), self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0))
            } else if let Some(n) = self.doc.get(sel_id) {
                let idx = self
                    .doc
                    .get(parent)
                    .and_then(|p| p.children.iter().position(|&c| c == sel_id).map(|i| i + 1))
                    .unwrap_or_else(|| self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0));
                (v2(n.transform.tx, n.transform.ty), idx)
            } else {
                (v2(0.0, 0.0), self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0))
            }
        } else {
            (v2(0.0, 0.0), self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0))
        };
        let lists = clipboard::instantiate(&mut self.doc, &clip);
        let roots: Vec<&Node> = lists.iter().filter_map(|l| l.first()).collect();
        if !self.slot_insert_allowed(parent, &roots) {
            return;
        }
        let first_orig = lists.first().and_then(|l| l.first()).map(|r| v2(r.transform.tx, r.transform.ty)).unwrap_or(v2(0.0, 0.0));
        let delta = v2(target_local.x - first_orig.x, target_local.y - first_orig.y);
        let mut tx = Tx::new();
        let mut new_sel = Vec::new();
        for (k, mut nodes) in lists.into_iter().enumerate() {
            if let Some(root) = nodes.first_mut() {
                root.transform.tx += delta.x;
                root.transform.ty += delta.y;
                apply_slot_stretch_on_insert(&self.doc, parent, root);
                new_sel.push(root.id);
            }
            let index = (base_idx + k).min(self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0));
            let _ = tx.cmd(&mut self.doc, Command::Insert { parent, index, nodes });
        }
        Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, parent);
        layout::relayout_around(&mut self.doc, &mut tx.ed, parent);
        self.run_tx(tx);
        self.selection = new_sel;
        self.show_toast("Pasted over selection", false);
    }

    /// `Cmd+Shift+R` / `Cmd+Alt+Shift+R` (`PasteToReplace`): replaces each selected node in-place with the
    /// clipboard contents, preserving the target's parent, child index, and `(tx, ty)` position.
    pub fn paste_to_replace(&mut self) {
        let Some(clip) = self.clip.clone() else { return };
        let tops = self.top_selection();
        if tops.is_empty() {
            return;
        }
        let mut tx = Tx::new();
        let mut new_sel = Vec::new();
        let mut affected_parents = Vec::new();
        for target_id in tops {
            let Some((parent, index, tx0, ty0)) = self.doc.get(target_id).and_then(|n| {
                let p = n.parent?;
                let idx = self.doc.get(p)?.children.iter().position(|&c| c == target_id).unwrap_or(0);
                Some((p, idx, n.transform.tx, n.transform.ty))
            }) else {
                continue;
            };
            if tx.cmd(&mut self.doc, Command::Remove { id: target_id }).is_err() {
                continue;
            }
            let lists = clipboard::instantiate(&mut self.doc, &clip);
            // The target is already out of the slot, so the check sees the net count after the swap.
            let roots: Vec<&Node> = lists.iter().filter_map(|l| l.first()).collect();
            if !self.slot_insert_allowed(parent, &roots) {
                tx.cancel(&mut self.doc);
                return;
            }
            let first_orig = lists.first().and_then(|l| l.first()).map(|r| v2(r.transform.tx, r.transform.ty)).unwrap_or(v2(0.0, 0.0));
            let delta = v2(tx0 - first_orig.x, ty0 - first_orig.y);
            for (k, mut nodes) in lists.into_iter().enumerate() {
                if let Some(root) = nodes.first_mut() {
                    root.transform.tx += delta.x;
                    root.transform.ty += delta.y;
                    new_sel.push(root.id);
                }
                let ins_idx = (index + k).min(self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0));
                let _ = tx.cmd(&mut self.doc, Command::Insert { parent, index: ins_idx, nodes });
            }
            if !affected_parents.contains(&parent) {
                affected_parents.push(parent);
            }
        }
        for p in affected_parents {
            Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, p);
            layout::relayout_around(&mut self.doc, &mut tx.ed, p);
        }
        self.run_tx(tx);
        if !new_sel.is_empty() {
            self.selection = new_sel;
            self.show_toast("Pasted to replace", false);
        }
    }

    /// `Cmd+Alt+C` (`CopyProperties`): copies visual properties (fills, strokes, effects,
    /// opacity, blend mode, corner radii, and text style) from the first selected layer.
    pub fn copy_properties(&mut self) {
        let Some(&id) = self.selection.first() else { return };
        let Some(n) = self.doc.get(id) else { return };
        self.copied_style = Some(CopiedStyle {
            fills: n.fills.clone(),
            stroke: n.stroke.clone(),
            effects: n.effects.clone(),
            opacity: n.opacity,
            blend_mode: n.blend,
            corner_radii: n.corner_radii,
            corner_smoothing: n.corner_smoothing,
            text_style: n.kind.any_text().map(|t| t.style.clone()),
        });
        self.show_toast("Copied properties", false);
    }

    /// `Cmd+Alt+V` (`PasteProperties`): pastes previously copied visual properties onto all
    /// currently selected layers in a single undoable transaction.
    pub fn paste_properties(&mut self) {
        let Some(snap) = self.copied_style.clone() else { return };
        if self.selection.is_empty() {
            return;
        }
        let ids = self.selection.clone();
        let mut tx = Tx::new();
        for id in ids {
            let mut relayout_text = false;
            if let Some(n) = tx.ed.node(&mut self.doc, id) {
                n.fills = snap.fills.clone();
                n.stroke = snap.stroke.clone();
                n.effects = snap.effects.clone();
                n.opacity = snap.opacity;
                n.blend = snap.blend_mode;
                n.corner_radii = snap.corner_radii;
                n.corner_smoothing = snap.corner_smoothing;
                let is_text = matches!(n.kind, NodeKind::Text(_));
                if let (Some(style), Some(tp)) = (&snap.text_style, n.kind.any_text_mut()) {
                    crate::richtext::apply_all(tp, |st| *st = style.clone());
                    if is_text {
                        n.size = layout::fit_text(tp, n.size);
                        relayout_text = true;
                    }
                }
            }
            if relayout_text {
                layout::relayout_around(&mut self.doc, &mut tx.ed, id);
            }
        }
        self.run_tx(tx);
        self.show_toast("Pasted properties", false);
    }

    /// `Q` or `Cmd/Ctrl + Alt + A`: Multi-Edit / Select Matching Layers across top-level frames
    /// and component variants on the active page (matching `name` and `NodeKind` variant).
    pub fn select_matching_layers(&mut self) -> usize {
        if self.selection.is_empty() {
            return 0;
        }
        let targets: Vec<(String, std::mem::Discriminant<NodeKind>)> =
            self.selection.iter().filter_map(|&id| self.doc.get(id).map(|n| (n.name.clone(), std::mem::discriminant(&n.kind)))).collect();
        if targets.is_empty() {
            return 0;
        }
        let mut matched: Vec<NodeId> = Vec::new();
        let doc = &self.doc;
        doc.walk(self.page, |n, depth| {
            if depth == 0 || !n.visible {
                return;
            }
            let disc = std::mem::discriminant(&n.kind);
            if targets.iter().any(|(name, d)| *name == n.name && *d == disc) {
                let mut cur = n.parent;
                let mut all_vis = true;
                while let Some(p) = cur {
                    if p == self.page {
                        break;
                    }
                    if !doc.get(p).is_some_and(|pn| pn.visible) {
                        all_vis = false;
                        break;
                    }
                    cur = doc.get(p).and_then(|pn| pn.parent);
                }
                if all_vis && !matched.contains(&n.id) {
                    matched.push(n.id);
                }
            }
        });
        if !matched.is_empty() {
            self.selection = matched;
            self.show_toast(format!("Selected {} matching layers", self.selection.len()), false);
        }
        self.selection.len()
    }

    fn resolve_selected_grid_frame(&self) -> Option<NodeId> {
        for &id in &self.selection {
            if let Some(n) = self.doc.get(id) {
                if n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).is_some_and(|al| al.direction == LayoutDirection::Grid) {
                    return Some(id);
                }
                if let Some(p) = n.parent {
                    if self
                        .doc
                        .get(p)
                        .and_then(|pn| pn.kind.frame_props())
                        .and_then(|fp| fp.auto_layout.as_ref())
                        .is_some_and(|al| al.direction == LayoutDirection::Grid)
                    {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    /// Inserts a row into the selected Table / CSS Grid frame (`InsertTableRow`).
    pub fn insert_table_row(&mut self) {
        let Some(fid) = self.resolve_selected_grid_frame() else { return };
        let mut tx = Tx::new();
        if let Some(n) = tx.ed.node(&mut self.doc, fid) {
            if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                al.grid_rows = (al.grid_rows.max(1) + 1).min(designos_doc::MAX_GRID_TRACKS as u32);
            }
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, fid);
        self.run_tx(tx);
    }

    /// Deletes a row from the selected Table / CSS Grid frame (`DeleteTableRow`).
    pub fn delete_table_row(&mut self) {
        let Some(fid) = self.resolve_selected_grid_frame() else { return };
        let mut tx = Tx::new();
        if let Some(n) = tx.ed.node(&mut self.doc, fid) {
            if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                al.grid_rows = al.grid_rows.saturating_sub(1).max(1);
            }
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, fid);
        self.run_tx(tx);
    }

    /// Inserts a column into the selected Table / CSS Grid frame (`InsertTableColumn`).
    pub fn insert_table_column(&mut self) {
        let Some(fid) = self.resolve_selected_grid_frame() else { return };
        let mut tx = Tx::new();
        if let Some(n) = tx.ed.node(&mut self.doc, fid) {
            if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                al.grid_columns = (al.grid_columns.max(1) + 1).min(designos_doc::MAX_GRID_TRACKS as u32);
            }
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, fid);
        self.run_tx(tx);
    }

    /// Deletes a column from the selected Table / CSS Grid frame (`DeleteTableColumn`).
    pub fn delete_table_column(&mut self) {
        let Some(fid) = self.resolve_selected_grid_frame() else { return };
        let mut tx = Tx::new();
        if let Some(n) = tx.ed.node(&mut self.doc, fid) {
            if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                al.grid_columns = al.grid_columns.saturating_sub(1).max(1);
            }
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, fid);
        self.run_tx(tx);
    }

    // ------------------------------------------------------------ grouping
    /// Wraps `ids` (all siblings, in desired child order) into a new group or
    /// frame; returns the container id.
    fn wrap_in_container(&mut self, tx: &mut Tx, ids: &[NodeId], frame: bool) -> Option<NodeId> {
        let kind = if frame { NodeKind::Frame(FrameProps::default()) } else { NodeKind::Group };
        self.wrap_in_kind(tx, ids, kind)
    }

    /// Wraps `ids` into a new container of `kind` (group, frame or boolean group).
    pub(crate) fn wrap_in_kind(&mut self, tx: &mut Tx, ids: &[NodeId], kind: NodeKind) -> Option<NodeId> {
        let frame = matches!(kind, NodeKind::Frame(_));
        let first = *ids.first()?;
        let parent = self.doc.get(first)?.parent?;
        let siblings = &self.doc.get(parent)?.children;
        let max_index = ids.iter().filter_map(|i| siblings.iter().position(|c| c == i)).max()?;
        let pinv = self.doc.world_transform(parent).invert()?;
        let mut pts = Vec::new();
        let mut rel: Vec<(NodeId, Affine)> = Vec::new();
        for id in ids {
            let w = self.doc.world_transform(*id);
            let size = self.doc.get(*id)?.size;
            pts.extend(rect_corners(&Rect::new(0.0, 0.0, size.x, size.y)).map(|p| pinv.apply(w.apply(p))));
            rel.push((*id, pinv * w));
        }
        let bb = aabb_of(&pts);
        let gid = self.doc.alloc_id();
        let name = kind.default_name();
        let mut g = if frame { Node::frame(gid, bb) } else { Node::new(gid, kind) };
        if frame {
            g.fills.clear();
        }
        g.name = self.unique_name(name);
        g.transform = Affine::translate(bb.x, bb.y);
        g.size = v2(bb.w, bb.h);
        // The wrapped layers leave the parent and one group takes their place.
        if !self.slot_swap_allowed(parent, ids.len(), &[&g]) {
            return None;
        }
        tx.cmd(&mut self.doc, Command::Insert { parent, index: max_index + 1, nodes: vec![g] }).ok()?;
        Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, parent);
        for (i, (id, in_parent)) in rel.into_iter().enumerate() {
            tx.cmd(&mut self.doc, Command::Reparent { id, parent: gid, index: i }).ok()?;
            if let Some(n) = tx.ed.node(&mut self.doc, id) {
                n.transform = Affine::translate(-bb.x, -bb.y) * in_parent;
            }
        }
        layout::relayout_up(&mut self.doc, &mut tx.ed, gid);
        Some(gid)
    }

    pub(crate) fn same_parent_tops(&self) -> Vec<NodeId> {
        let mut tops = self.top_selection();
        let Some(first) = tops.first().copied() else { return tops };
        let parent = self.doc.get(first).and_then(|n| n.parent);
        tops.retain(|t| self.doc.get(*t).and_then(|n| n.parent) == parent);
        if let Some(p) = parent.and_then(|p| self.doc.get(p)) {
            tops.sort_by_key(|t| p.children.iter().position(|c| c == t).unwrap_or(0));
        }
        tops
    }

    pub fn group_selection(&mut self, frame: bool) {
        let ids = self.same_parent_tops();
        if ids.is_empty() {
            return;
        }
        let mut tx = Tx::new();
        let g = self.wrap_in_container(&mut tx, &ids, frame);
        self.run_tx(tx);
        if let Some(g) = g {
            self.selection = vec![g];
        }
    }

    pub fn ungroup_selection(&mut self) {
        let tops = self.top_selection();
        let mut tx = Tx::new();
        let mut new_sel = Vec::new();
        for id in tops {
            let Some(g) = self.doc.get(id).cloned() else { continue };
            let is_group = matches!(g.kind, NodeKind::Group) || (matches!(g.kind, NodeKind::Frame(_)) && g.parent.is_some());
            let Some(parent) = g.parent else { continue };
            if !is_group || g.children.is_empty() {
                continue;
            }
            // The group leaves its parent and its children take its place: judge the slot on the net count.
            let kids: Vec<Node> = g.children.iter().filter_map(|c| self.doc.get(*c).cloned()).collect();
            let kid_refs: Vec<&Node> = kids.iter().collect();
            if !self.slot_swap_allowed(parent, 1, &kid_refs) {
                continue;
            }
            let mut index = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)).unwrap_or(0);
            for c in g.children.clone() {
                let t = self.doc.get(c).map(|n| n.transform).unwrap_or_default();
                if tx.cmd(&mut self.doc, Command::Reparent { id: c, parent, index }).is_ok() {
                    if let Some(n) = tx.ed.node(&mut self.doc, c) {
                        n.transform = g.transform * t;
                    }
                    new_sel.push(c);
                    index += 1;
                }
            }
            let _ = tx.cmd(&mut self.doc, Command::Remove { id });
            Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, parent);
            layout::relayout_around(&mut self.doc, &mut tx.ed, parent);
        }
        self.run_tx(tx);
        if !new_sel.is_empty() {
            self.selection = new_sel;
        }
    }

    /// Shift+A: wraps the selection in an auto-layout frame, or toggles
    /// auto-layout on a selected frame.
    pub fn toggle_auto_layout(&mut self) {
        let tops = self.same_parent_tops();
        if tops.is_empty() {
            return;
        }
        let mut tx = Tx::new();
        let single_frame = tops.len() == 1 && self.doc.get(tops[0]).map(|n| n.kind.frame_props().is_some() && n.parent.is_some()).unwrap_or(false);
        let target = if single_frame {
            tops[0]
        } else {
            // Order children by position along the dominant axis.
            let rects: Vec<(NodeId, Rect)> = tops.iter().map(|t| (*t, crate::canvas::fast_world_aabb(&self.doc, None, *t))).collect();
            let bb = union_all(rects.iter().map(|r| r.1)).unwrap_or_default();
            let horizontal = {
                let xs = rects.iter().map(|r| r.1.x + r.1.w / 2.0);
                let ys = rects.iter().map(|r| r.1.y + r.1.h / 2.0);
                let spread = |it: &mut dyn Iterator<Item = f32>| {
                    let v: Vec<f32> = it.collect();
                    v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
                };
                spread(&mut xs.clone()) >= spread(&mut ys.clone()) || bb.w >= bb.h
            };
            let mut ordered = rects.clone();
            ordered.sort_by(|a, b| {
                let (ka, kb) = if horizontal { (a.1.x, b.1.x) } else { (a.1.y, b.1.y) };
                ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
            });
            let ids: Vec<NodeId> = ordered.iter().map(|r| r.0).collect();
            let Some(g) = self.wrap_in_container(&mut tx, &ids, true) else { return };
            if let Some(n) = tx.ed.node(&mut self.doc, g) {
                if let Some(fp) = n.kind.frame_props_mut() {
                    fp.auto_layout = Some(AutoLayout {
                        direction: if horizontal { LayoutDirection::Horizontal } else { LayoutDirection::Vertical },
                        item_spacing: theme::AUTO_LAYOUT_SPACING,
                        padding: [theme::AUTO_LAYOUT_PADDING; 4],
                        primary_sizing: Sizing::Hug,
                        counter_sizing: Sizing::Hug,
                        ..AutoLayout::v1()
                    });
                }
            }
            layout::relayout_around(&mut self.doc, &mut tx.ed, g);
            self.run_tx(tx);
            self.selection = vec![g];
            return;
        };
        if let Some(n) = tx.ed.node(&mut self.doc, target) {
            if let Some(fp) = n.kind.frame_props_mut() {
                fp.auto_layout = match fp.auto_layout {
                    Some(_) => None,
                    None => Some(AutoLayout {
                        item_spacing: theme::AUTO_LAYOUT_SPACING,
                        padding: [theme::AUTO_LAYOUT_PADDING; 4],
                        primary_sizing: Sizing::Hug,
                        counter_sizing: Sizing::Hug,
                        ..AutoLayout::v1()
                    }),
                };
            }
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, target);
        self.run_tx(tx);
    }

    /// Alt+Shift+A: removes auto-layout from selected frames while preserving children positions.
    pub fn remove_auto_layout(&mut self) {
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        let mut tx = Tx::new();
        for id in ids {
            let has_al = self.doc.get(id).and_then(|n| n.kind.frame_props()).is_some_and(|fp| fp.auto_layout.is_some());
            if has_al {
                if let Some(n) = tx.ed.node(&mut self.doc, id) {
                    if let Some(fp) = n.kind.frame_props_mut() {
                        fp.auto_layout = None;
                    }
                }
                layout::relayout_around(&mut self.doc, &mut tx.ed, id);
            }
        }
        self.run_tx(tx);
    }

    /// Whether the current selection (or its subtree) has any legacy version-0 auto-layout frame.
    pub fn has_upgradable_auto_layout(&self) -> bool {
        let mut found = false;
        for &root in &self.selection {
            self.doc.walk(root, |n, _| {
                if n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).is_some_and(|al| al.layout_version < 1) {
                    found = true;
                }
            });
            if found {
                return true;
            }
        }
        false
    }

    /// Upgrades selected auto-layout frames (and any legacy v0 auto-layout frames inside the
    /// selection subtree) to `layout_version = 1` and re-lays out as a single undo step,
    /// mirroring Figma's `upgradeStackLayoutVersionForSelection`.
    pub fn upgrade_auto_layout_version(&mut self) -> usize {
        let roots = self.selection.clone();
        if roots.is_empty() {
            return 0;
        }
        let mut to_upgrade = Vec::new();
        for &root in &roots {
            self.doc.walk(root, |n, _| {
                if n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).is_some_and(|al| al.layout_version < 1) && !to_upgrade.contains(&n.id) {
                    to_upgrade.push(n.id);
                }
            });
        }
        if to_upgrade.is_empty() {
            return 0;
        }
        let mut tx = Tx::new();
        for &id in &to_upgrade {
            if let Some(n) = tx.ed.node(&mut self.doc, id) {
                if let Some(fp) = n.kind.frame_props_mut() {
                    if let Some(al) = fp.auto_layout.as_mut() {
                        al.layout_version = 1;
                    }
                }
            }
        }
        for &id in &to_upgrade {
            layout::relayout_around(&mut self.doc, &mut tx.ed, id);
        }
        self.run_tx(tx);
        self.show_toast("Updated auto layout to version 1", false);
        to_upgrade.len()
    }

    // ------------------------------------------------------------ transforms
    pub fn nudge(&mut self, dx: f32, dy: f32) {
        let tops = self.top_selection();
        let mut tx = Tx::new();
        let mut relayout_roots = Vec::new();
        let mut seen_parents = Vec::new();
        let mut al_parents = Vec::new();
        let forward_dir = dx > 0.0 || dy > 0.0;
        let ordered_tops: Vec<NodeId> = if forward_dir { tops.iter().rev().copied().collect() } else { tops };
        for id in &ordered_tops {
            let flow_al = self.doc.get(*id).and_then(|n| {
                if n.layout_child.absolute {
                    return None;
                }
                let pid = n.parent?;
                let al = self.doc.get(pid)?.kind.frame_props()?.auto_layout.as_ref()?;
                Some((pid, al.direction))
            });
            if let Some((parent_id, dir)) = flow_al {
                let delta_idx: isize = match dir {
                    LayoutDirection::Horizontal | LayoutDirection::Grid if dx != 0.0 => {
                        if dx > 0.0 {
                            1
                        } else {
                            -1
                        }
                    }
                    LayoutDirection::Vertical | LayoutDirection::Grid if dy != 0.0 => {
                        if dy > 0.0 {
                            1
                        } else {
                            -1
                        }
                    }
                    _ => 0,
                };
                if delta_idx != 0 {
                    let len = self.doc.get(parent_id).map(|p| p.children.len()).unwrap_or(0);
                    if let Some(cur) = self.doc.get(parent_id).and_then(|p| p.children.iter().position(|c| c == id)) {
                        let target = (cur as isize + delta_idx).clamp(0, len.saturating_sub(1) as isize) as usize;
                        if target != cur {
                            let _ = tx.cmd(&mut self.doc, Command::Reparent { id: *id, parent: parent_id, index: target });
                            if !al_parents.contains(&parent_id) {
                                al_parents.push(parent_id);
                            }
                        }
                    }
                }
                continue;
            }
            let d = delta_to_parent(&self.doc, *id, v2(dx, dy));
            if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                n.transform.tx += d.x;
                n.transform.ty += d.y;
                if let Some(p) = n.parent {
                    if !seen_parents.contains(&p) {
                        seen_parents.push(p);
                        relayout_roots.push(*id);
                    }
                }
            }
        }
        for pid in al_parents {
            layout::relayout_around(&mut self.doc, &mut tx.ed, pid);
        }
        for id in relayout_roots {
            layout::relayout_up(&mut self.doc, &mut tx.ed, id);
        }
        self.run_tx(tx);
    }

    /// Cmd/Ctrl + Arrow (`±1px`) or Cmd/Ctrl + Shift + Arrow (`±10px`) resizes the selection.
    pub fn resize_selection_by(&mut self, dw: f32, dh: f32) {
        let tops = self.top_selection();
        if tops.is_empty() {
            return;
        }
        let mut tx = Tx::new();
        for id in tops {
            let Some((old_size, transform, locked)) = self.doc.get(id).map(|n| (n.size, n.transform, n.locked)) else {
                continue;
            };
            if locked {
                continue;
            }
            let new_size = Vec2::new((old_size.x + dw).max(1.0), (old_size.y + dh).max(1.0));
            let counter = self.doc.get(id).and_then(|n| n.parent).and_then(|p| self.doc.get(p)).and_then(designos_doc::LayoutChild::counter_axis_is_horizontal);
            if let Some(n) = tx.ed.node(&mut self.doc, id) {
                if dw.abs() > 1e-4 {
                    n.layout_child.set_axis_sizing(true, Sizing::Fixed, counter);
                    if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                        if matches!(al.direction, LayoutDirection::Horizontal | LayoutDirection::Grid) {
                            if al.primary_sizing == Sizing::Hug {
                                al.primary_sizing = Sizing::Fixed;
                            }
                        } else if al.counter_sizing == Sizing::Hug {
                            al.counter_sizing = Sizing::Fixed;
                        }
                    }
                }
                if dh.abs() > 1e-4 {
                    n.layout_child.set_axis_sizing(false, Sizing::Fixed, counter);
                    if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                        if al.direction == LayoutDirection::Vertical {
                            if al.primary_sizing == Sizing::Hug {
                                al.primary_sizing = Sizing::Fixed;
                            }
                        } else if al.counter_sizing == Sizing::Hug {
                            al.counter_sizing = Sizing::Fixed;
                        }
                    }
                }
            }
            apply_resize(&mut self.doc, &mut tx.ed, id, new_size, transform, true);
        }
        self.run_tx(tx);
    }

    /// Rapid `0`..`9` opacity shortcut (`5` -> 50%, `4` then `5` within 400ms -> 45%, `0` -> 100%, `00` -> 0%).
    pub fn apply_opacity_digit(&mut self, digit: u8) {
        if digit > 9 || self.selection.is_empty() {
            return;
        }
        let opacity = match self.opacity_buffer {
            Some((prev, prev_time)) if (self.now - prev_time) >= 0.0 && (self.now - prev_time) <= 0.4 => {
                self.opacity_buffer = None;
                if prev == 0 && digit == 0 {
                    0.0
                } else {
                    ((prev as u16 * 10 + digit as u16) as f32 / 100.0).clamp(0.0, 1.0)
                }
            }
            _ => {
                self.opacity_buffer = Some((digit, self.now));
                if digit == 0 {
                    1.0
                } else {
                    (digit as f32) / 10.0
                }
            }
        };
        let ids = self.selection.clone();
        self.edit_nodes(&ids, |n| n.opacity = opacity);
    }

    pub fn align(&mut self, a: Align) {
        let tops = self.top_selection();
        if tops.is_empty() {
            return;
        }
        let idx = crate::canvas::interact::active_index(self);
        let top_aabbs: Vec<(NodeId, Rect)> = tops.iter().map(|&t| (t, crate::canvas::fast_world_aabb(&self.doc, idx, t))).collect();
        let target = if top_aabbs.len() == 1 {
            match self.doc.get(top_aabbs[0].0).and_then(|n| n.parent) {
                Some(p) if !matches!(self.doc.get(p).map(|n| &n.kind), Some(NodeKind::Page { .. })) => crate::canvas::fast_world_aabb(&self.doc, idx, p),
                _ => top_aabbs[0].1,
            }
        } else {
            union_all(top_aabbs.iter().map(|(_, r)| *r)).unwrap_or_default()
        };
        let mut tx = Tx::new();
        let mut relayout_roots = Vec::new();
        let mut seen_parents = Vec::new();
        for (id, r) in top_aabbs {
            let d = match a {
                Align::Left => v2(target.x - r.x, 0.0),
                Align::HCenter => v2(target.x + target.w / 2.0 - (r.x + r.w / 2.0), 0.0),
                Align::Right => v2(target.x + target.w - (r.x + r.w), 0.0),
                Align::Top => v2(0.0, target.y - r.y),
                Align::VCenter => v2(0.0, target.y + target.h / 2.0 - (r.y + r.h / 2.0)),
                Align::Bottom => v2(0.0, target.y + target.h - (r.y + r.h)),
            };
            let d = delta_to_parent(&self.doc, id, d);
            if let Some(n) = tx.ed.node(&mut self.doc, id) {
                n.transform.tx += d.x;
                n.transform.ty += d.y;
                if let Some(p) = n.parent {
                    if !seen_parents.contains(&p) {
                        seen_parents.push(p);
                        relayout_roots.push(id);
                    }
                }
            }
        }
        for id in relayout_roots {
            layout::relayout_up(&mut self.doc, &mut tx.ed, id);
        }
        self.run_tx(tx);
    }

    /// `Shift + Click` on any alignment button (`#81` / `REA 4I`): aligns the entire multi-selection
    /// as a single rigid group relative to its shared parent container (or page viewport if top-level),
    /// preserving the relative offsets between selected layers.
    pub fn align_selection_as_group(&mut self, a: Align) {
        let tops = self.top_selection();
        if tops.is_empty() {
            return;
        }
        if tops.len() == 1 {
            self.align(a);
            return;
        }
        let idx = crate::canvas::interact::active_index(self);
        let top_aabbs: Vec<(NodeId, Rect)> = tops.iter().map(|&t| (t, crate::canvas::fast_world_aabb(&self.doc, idx, t))).collect();
        let Some(group_bb) = union_all(top_aabbs.iter().map(|(_, r)| *r)) else {
            return;
        };
        let parent_target = self
            .doc
            .get(tops[0])
            .and_then(|n| n.parent)
            .filter(|&p| !matches!(self.doc.get(p).map(|n| &n.kind), Some(NodeKind::Page { .. })))
            .map(|p| crate::canvas::fast_world_aabb(&self.doc, idx, p))
            .unwrap_or(group_bb);
        let world_delta = match a {
            Align::Left => v2(parent_target.x - group_bb.x, 0.0),
            Align::HCenter => v2(parent_target.x + parent_target.w * 0.5 - (group_bb.x + group_bb.w * 0.5), 0.0),
            Align::Right => v2(parent_target.x + parent_target.w - (group_bb.x + group_bb.w), 0.0),
            Align::Top => v2(0.0, parent_target.y - group_bb.y),
            Align::VCenter => v2(0.0, parent_target.y + parent_target.h * 0.5 - (group_bb.y + group_bb.h * 0.5)),
            Align::Bottom => v2(0.0, parent_target.y + parent_target.h - (group_bb.y + group_bb.h)),
        };
        if world_delta.x.abs() < 1e-4 && world_delta.y.abs() < 1e-4 {
            return;
        }
        let mut tx = Tx::new();
        let mut relayout_roots = Vec::new();
        let mut seen_parents = Vec::new();
        for (id, _) in top_aabbs {
            let d = delta_to_parent(&self.doc, id, world_delta);
            if let Some(n) = tx.ed.node(&mut self.doc, id) {
                n.transform.tx += d.x;
                n.transform.ty += d.y;
                if let Some(p) = n.parent {
                    if !seen_parents.contains(&p) {
                        seen_parents.push(p);
                        relayout_roots.push(id);
                    }
                }
            }
        }
        for id in relayout_roots {
            layout::relayout_up(&mut self.doc, &mut tx.ed, id);
        }
        self.run_tx(tx);
    }

    /// Equalizes spacing and aligns cross-axis for 2+ selected sibling layers (`#16` Tidy up).
    pub fn tidy_up(&mut self) {
        let Some((_, mut sorted)) = crate::canvas::interact::smart_selection_items(self) else {
            return;
        };
        if sorted.len() < 2 {
            return;
        }
        let min_x = sorted.iter().map(|(_, r)| r.x).fold(f32::MAX, f32::min);
        let max_x = sorted.iter().map(|(_, r)| r.x + r.w).fold(f32::MIN, f32::max);
        let min_y = sorted.iter().map(|(_, r)| r.y).fold(f32::MAX, f32::min);
        let max_y = sorted.iter().map(|(_, r)| r.y + r.h).fold(f32::MIN, f32::max);
        let horiz = (max_x - min_x) >= (max_y - min_y);
        sorted.sort_by(|a, b| {
            let ka = if horiz { a.1.x } else { a.1.y };
            let kb = if horiz { b.1.x } else { b.1.y };
            ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let n_items = sorted.len();
        let total_span = if horiz {
            (sorted[n_items - 1].1.x + sorted[n_items - 1].1.w) - sorted[0].1.x
        } else {
            (sorted[n_items - 1].1.y + sorted[n_items - 1].1.h) - sorted[0].1.y
        };
        let sum_sizes: f32 = sorted.iter().map(|(_, r)| if horiz { r.w } else { r.h }).sum();
        let gap = ((total_span - sum_sizes) / ((n_items - 1) as f32)).max(0.0).round();
        let mut cursor = if horiz { sorted[0].1.x } else { sorted[0].1.y };
        let cross_origin = if horiz { sorted[0].1.y } else { sorted[0].1.x };
        let mut tx = Tx::new();
        for (id, bb) in sorted {
            let world_delta = if horiz { v2(cursor - bb.x, cross_origin - bb.y) } else { v2(cross_origin - bb.x, cursor - bb.y) };
            cursor += (if horiz { bb.w } else { bb.h }) + gap;
            let d = delta_to_parent(&self.doc, id, world_delta);
            if let Some(n) = tx.ed.node(&mut self.doc, id) {
                n.transform.tx += d.x;
                n.transform.ty += d.y;
            }
        }
        if self.run_tx(tx) {
            self.show_toast(format!("Tidied up (gap {gap}px)"), false);
        }
    }

    /// Moves the selection one step (or to the extreme) in z-order.
    pub fn reorder(&mut self, forward: bool, extreme: bool) {
        let tops = self.same_parent_tops();
        let Some(parent) = tops.first().and_then(|t| self.doc.get(*t)).and_then(|n| n.parent) else { return };
        let len = self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
        let mut tx = Tx::new();
        let order: Vec<NodeId> = if forward { tops.iter().rev().copied().collect() } else { tops.clone() };
        let mut moved = false;
        for (k, id) in order.iter().enumerate() {
            let cur = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| c == id)).unwrap_or(0);
            let target = match (forward, extreme) {
                (true, true) => len.saturating_sub(1),
                (false, true) => k,
                (true, false) => (cur + 1).min(len.saturating_sub(1)),
                (false, false) => cur.saturating_sub(1),
            };
            if target != cur {
                let _ = tx.cmd(&mut self.doc, Command::Reparent { id: *id, parent, index: target });
                moved = true;
            }
        }
        if moved {
            Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, parent);
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, parent);
        self.run_tx(tx);
    }

    /// Reparents `ids` into `new_parent` at `index` (index counted after the
    /// dragged nodes are removed), preserving their page position.
    pub fn move_to_parent(&mut self, ids: &[NodeId], new_parent: NodeId, index: usize) {
        let Some(npw) = self.doc.world_transform(new_parent).invert() else { return };
        let mut tx = Tx::new();
        let mut old_parents = Vec::new();
        let mut moved = false;
        for (i, id) in ids.iter().enumerate() {
            if *id == new_parent || self.doc.is_ancestor(*id, new_parent) {
                continue;
            }
            let old_p = self.doc.get(*id).and_then(|n| n.parent);
            if !self.slot_move_allowed(&[*id], new_parent) {
                continue;
            }
            let world = self.doc.world_transform(*id);
            if let Err(e) = tx.cmd(&mut self.doc, Command::Reparent { id: *id, parent: new_parent, index: index + i }) {
                self.show_toast(format!("Cannot move layer: {e}"), true);
                continue;
            }
            moved = true;
            if let Some(p) = old_p {
                old_parents.push(p);
            }
            if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                n.transform = npw * world;
            }
            if old_p != Some(new_parent) && is_slot_container(&self.doc, new_parent) {
                if let Some(mut cloned) = self.doc.get(*id).cloned() {
                    apply_slot_stretch_on_insert(&self.doc, new_parent, &mut cloned);
                    if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                        *n = cloned;
                    }
                }
            }
        }
        for p in old_parents {
            if self.doc.get(p).is_some() {
                Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, p);
                layout::relayout_around(&mut self.doc, &mut tx.ed, p);
            }
        }
        if moved {
            Self::mark_slot_content_override(&mut self.doc, &mut tx.ed, new_parent);
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, new_parent);
        self.run_tx(tx);
    }

    // ------------------------------------------------------------ naming / flags
    pub fn rename(&mut self, id: NodeId, name: &str) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        self.edit_nodes(&[id], |n| n.name = name.clone());
    }

    /// Cmd+R: opens the left Layers panel, expands ancestors of `selection[0]`, and begins inline rename.
    pub fn rename_selection(&mut self) {
        let Some(&id) = self.selection.first() else { return };
        self.ui.left_collapsed = false;
        let mut cur = self.doc.get(id).and_then(|n| n.parent);
        let mut expanded_any = false;
        while let Some(p) = cur {
            if p == self.page {
                break;
            }
            if self.layers.expanded.insert(p) {
                expanded_any = true;
            }
            cur = self.doc.get(p).and_then(|n| n.parent);
        }
        if expanded_any {
            self.layers.expanded_gen = self.layers.expanded_gen.wrapping_add(1);
        }
        self.layers.renaming = Some(id);
        self.layers.rename_started = false;
        self.layers.rename_seen = false;
    }

    pub fn toggle_visible(&mut self, id: NodeId) {
        self.edit_nodes(&[id], |n| n.visible = !n.visible);
    }

    /// Cmd+Shift+H: toggles visibility across the current selection.
    pub fn toggle_selection_visible(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let ids = self.selection.clone();
        let any_visible = ids.iter().any(|&id| self.doc.get(id).is_some_and(|n| n.visible));
        let new_vis = !any_visible;
        self.edit_nodes(&ids, |n| n.visible = new_vis);
    }

    pub fn toggle_lock(&mut self, id: NodeId) {
        self.edit_nodes(&[id], |n| n.locked = !n.locked);
    }

    /// Cmd+Shift+L: toggles lock state across the current selection.
    pub fn toggle_selection_locked(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let ids = self.selection.clone();
        let any_locked = ids.iter().any(|&id| self.doc.get(id).is_some_and(|n| n.locked));
        let new_locked = !any_locked;
        self.edit_nodes(&ids, |n| n.locked = new_locked);
    }

    // ------------------------------------------------------------ pages
    /// Pre-order copy of a subtree (root first), as `InsertPage` expects.
    fn subtree_nodes(&self, id: NodeId) -> Vec<Node> {
        let mut out = Vec::new();
        self.doc.walk(id, |n, _| out.push(n.clone()));
        out
    }

    fn unique_page_name(&self, base: &str) -> String {
        let names: Vec<&str> = self.doc.pages.iter().filter_map(|p| self.doc.get(*p)).map(|n| n.name.as_str()).collect();
        if !names.contains(&base) {
            return base.to_string();
        }
        (2..).map(|i| format!("{base} {i}")).find(|c| !names.contains(&c.as_str())).unwrap_or_else(|| base.to_string())
    }

    /// Appends a new empty page (undoable) and switches to it.
    pub fn add_page(&mut self) -> Option<NodeId> {
        crate::decks::ui::flush_notes(self);
        let id = self.doc.alloc_id();
        let mut page = Node::new(id, NodeKind::Page { background: Color::from_hex(&format!("{:06X}", theme::DEFAULT_PAGE_BG)).unwrap_or(Color::WHITE) });
        page.name = format!("Page {}", self.doc.pages.len() + 1);
        let index = self.doc.pages.len();
        let mut tx = Tx::new();
        tx.cmd(&mut self.doc, Command::InsertPage { index, nodes: vec![page] }).ok()?;
        self.run_tx(tx);
        self.switch_page(id);
        Some(id)
    }

    /// Duplicates a page with all its layers right after it; returns the copy.
    pub fn duplicate_page(&mut self, id: NodeId) -> Option<NodeId> {
        let index = self.doc.pages.iter().position(|p| *p == id)? + 1;
        crate::decks::ui::flush_notes(self);
        let mut nodes = clipboard::clone_subtree(&mut self.doc, id);
        let root = nodes.first_mut()?;
        root.name = self.unique_page_name(&format!("{} copy", self.doc.get(id)?.name));
        let new_id = root.id;
        let mut tx = Tx::new();
        tx.cmd(&mut self.doc, Command::InsertPage { index, nodes }).ok()?;
        self.run_tx(tx);
        self.switch_page(new_id);
        Some(new_id)
    }

    /// Deletes a page; the last remaining page can never be deleted.
    pub fn delete_page(&mut self, id: NodeId) -> bool {
        if self.doc.pages.len() <= 1 || !self.doc.pages.contains(&id) {
            return false;
        }
        crate::decks::ui::flush_notes(self);
        let pos = self.doc.pages.iter().position(|p| *p == id).unwrap_or(0);
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, Command::RemovePage { id }).is_err() {
            return false;
        }
        self.run_tx(tx);
        if self.page == id || !self.doc.nodes.contains_key(&self.page) {
            let next = self.doc.pages[pos.min(self.doc.pages.len() - 1)];
            self.switch_page(next);
        }
        true
    }

    /// Moves a page to `index` (position after removal) in the page list.
    pub fn move_page(&mut self, id: NodeId, index: usize) {
        let Some(from) = self.doc.pages.iter().position(|p| *p == id) else { return };
        let index = index.min(self.doc.pages.len().saturating_sub(1));
        if from == index {
            return;
        }
        crate::decks::ui::flush_notes(self);
        let nodes = self.subtree_nodes(id);
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, Command::RemovePage { id }).is_ok() {
            let _ = tx.cmd(&mut self.doc, Command::InsertPage { index, nodes });
        }
        self.run_tx(tx);
    }

    pub fn switch_page(&mut self, id: NodeId) {
        if self.doc.pages.contains(&id) {
            crate::decks::ui::flush_notes(self);
            self.doc.ensure_page_hydrated(id);
            self.page = id;
            self.selection.clear();
            self.canvas.reset_interaction();
            self.changed();
            self.fit_all();
        }
    }

    /// PageUp (`forward = false`) / PageDown (`forward = true`): steps to the previous or next page.
    pub fn step_page(&mut self, forward: bool) {
        let Some(idx) = self.doc.pages.iter().position(|&p| p == self.page) else {
            return;
        };
        let next = if forward { idx.checked_add(1).filter(|&i| i < self.doc.pages.len()) } else { idx.checked_sub(1) };
        if let Some(i) = next {
            let target = self.doc.pages[i];
            self.switch_page(target);
        }
    }

    /// `N` / `Shift+N` (`fit_zoom = true`) and `Home` / `End` (`fit_zoom = false`):
    /// steps to the next or previous top-level frame on the current page.
    pub fn step_frame(&mut self, forward: bool, fit_zoom: bool) {
        let Some(page_node) = self.doc.get(self.page) else {
            return;
        };
        let is_frame_like = |n: &Node| n.visible && matches!(n.kind, NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_));
        let mut frames = Vec::new();
        for &cid in &page_node.children {
            let Some(cn) = self.doc.get(cid) else { continue };
            if !cn.visible {
                continue;
            }
            if is_frame_like(cn) {
                frames.push(cid);
            } else if matches!(cn.kind, NodeKind::Section) {
                for &scid in &cn.children {
                    if self.doc.get(scid).is_some_and(is_frame_like) {
                        frames.push(scid);
                    }
                }
            }
        }
        if frames.is_empty() {
            return;
        }
        let idx = crate::canvas::interact::active_index(self);
        let mut items: Vec<(NodeId, Rect)> = frames.into_iter().map(|id| (id, crate::canvas::fast_world_aabb(&self.doc, idx, id))).collect();
        items.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then_with(|| a.1.y.total_cmp(&b.1.y)));
        let len = items.len();
        let cur_idx = self.selection.first().and_then(|&sel| items.iter().position(|&(fid, _)| fid == sel || self.doc.is_ancestor(fid, sel)));
        let target_idx = match cur_idx {
            Some(i) => {
                if forward {
                    (i + 1) % len
                } else {
                    (i + len - 1) % len
                }
            }
            None => {
                if forward {
                    0
                } else {
                    len - 1
                }
            }
        };
        let (target, b) = items[target_idx];
        self.selection = vec![target];
        if fit_zoom {
            self.zoom_to_selection();
        } else {
            let z = self.view.zoom.clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
            let size = self.canvas_rect.size();
            self.view = View { zoom: z, offset: Vec2::new(b.x + b.w / 2.0 - size.x / 2.0 / z, b.y + b.h / 2.0 - size.y / 2.0 / z) };
        }
    }

    // ------------------------------------------------------------ view
    pub fn page_bounds(&self) -> Option<Rect> {
        let kids = &self.doc.get(self.page)?.children;
        let idx = crate::canvas::interact::active_index(self);
        union_all(kids.iter().filter(|c| self.doc.get(**c).map(|n| n.visible).unwrap_or(false)).map(|c| crate::canvas::fast_world_aabb(&self.doc, idx, *c)))
    }

    pub fn fit_all(&mut self) {
        let b = self.page_bounds().unwrap_or(Rect::new(0.0, 0.0, 800.0, 600.0));
        self.view = View::fit(b, self.canvas_rect.size());
    }

    pub fn fit_selection(&mut self) {
        match self.selection_bounds() {
            Some(b) => {
                self.view = View::fit(b, self.canvas_rect.size());
            }
            None => self.fit_all(),
        }
    }

    pub fn zoom_to_selection(&mut self) {
        self.fit_selection();
    }

    /// Zooms to `zoom` (a factor) centred on the selection, else on the page content.
    pub fn zoom_on_content(&mut self, zoom: f32) {
        let b = self.selection_bounds().or_else(|| self.page_bounds()).unwrap_or(Rect::new(0.0, 0.0, 800.0, 600.0));
        let z = zoom.clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
        let size = self.canvas_rect.size();
        self.view = View { zoom: z, offset: Vec2::new(b.x + b.w / 2.0 - size.x / 2.0 / z, b.y + b.h / 2.0 - size.y / 2.0 / z) };
    }

    pub fn zoom_to(&mut self, zoom: f32) {
        let c = self.canvas_rect.center();
        self.view.zoom_at(self.canvas_rect.min, c, zoom);
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom_to(self.view.zoom * factor);
    }

    // ------------------------------------------------------------ files
    /// Drops the cached pre-repair bytes of `old` once the active document no longer lives there,
    /// unless it moves to the same file (`now_at`) or another tab still holds `old`.
    pub(crate) fn release_repaired_original(&self, old: &std::path::Path, now_at: Option<&std::path::Path>) {
        let same_as_new = now_at.is_some_and(|np| io::same_repair_key(old, np));
        let skip = (!self.tabs.parked_in_home).then_some(self.tabs.active_idx);
        let held_by_other_tab = self.tabs.has_open_repair_key(self, old, skip);
        if !same_as_new && !held_by_other_tab {
            io::drop_repaired_original(old);
        }
    }

    /// Releases the active document's cached pre-repair bytes (from either `file_path` or
    /// `lib_file`) when replacing it in-place with `now_at`.
    pub(crate) fn release_active_repaired_original(&mut self, now_at: Option<&std::path::Path>) {
        if !self.tabs.parked_in_home {
            if let Some(old_path) = self.file_path.take() {
                self.release_repaired_original(&old_path, now_at);
            }
            if let Some(old_lf) = self.lib_file.take() {
                if let Some(old_lib_path) = self.lib_file_disk_path(&old_lf) {
                    self.release_repaired_original(&old_lib_path, now_at);
                }
            }
        } else {
            self.file_path = None;
            self.lib_file = None;
        }
    }

    pub fn replace_document(&mut self, doc: Document, path: Option<PathBuf>) {
        crate::decks::ui::flush_notes(self);
        self.stop_plugin();
        // Pending edits of the library file being left are saved first (from the live state, not an old version).
        self.exit_version_view();
        self.flush_autosave();
        self.release_active_repaired_original(path.as_deref());
        self.renaming_file = false;
        let mut doc = doc;
        io::ensure_page(&mut doc);
        let first_page = doc.pages[0];
        doc.ensure_page_hydrated(first_page);
        self.page = first_page;
        self.doc = doc;
        self.decks = Default::default();
        self.hist = History::default();
        self.history.reset();
        self.comments.close_popovers();
        self.selection.clear();
        self.hover = None;
        self.file_path = path;
        self.canvas.reset_interaction();
        // a design tool means nothing on a board and the other way round
        self.tool = Tool::Move;
        self.board.dots = None;
        self.board.target = None;
        self.board.tool_menu = None;
        self.board.bar_menu = None;
        self.layers.reset();
        self.fonts.load_for(&self.doc);
        self.changed();
        self.fit_all();
    }

    pub fn open_dialog(&mut self) {
        if let Some(p) = io::pick_open() {
            self.open_path(p);
        }
    }

    pub fn open_path(&mut self, p: PathBuf) {
        // A variables-only JSON opened alongside a design merges into it.
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            if let Ok(text) = std::fs::read_to_string(&p) {
                // Figma REST comments (`/v1/files/:key/comments`) are merged the same way.
                if crate::comments::import::is_comments_payload(&text) {
                    self.import_comments_file(&p);
                    return;
                }
                if crate::variables::is_variables_payload(&text) {
                    self.import_variables_file(&p);
                    return;
                }
            }
        }
        match io::load_path_with_warning(&p) {
            Ok((doc, warning)) => {
                let name = doc.name.clone();
                // An SVG becomes a new unsaved document, never overwritten by Save.
                let svg = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
                self.replace_document(doc, (!svg).then_some(p));
                self.show_toast(io::opened_toast_text(&name, warning.as_deref()), warning.is_some());
            }
            Err(e) => self.show_toast(e, true),
        }
    }

    pub fn save(&mut self, as_new: bool) {
        crate::decks::ui::flush_notes(self);
        // Library files save back to the library; "Save as…" writes an external copy.
        if !as_new && self.lib_file.is_some() {
            self.save_library_file_with_toast();
            return;
        }
        let target = match (&self.file_path, as_new) {
            (Some(p), false) if p.extension().map(|e| e == io::NATIVE_EXT).unwrap_or(false) => Some(p.clone()),
            _ => io::pick_save(&self.doc.name),
        };
        let Some(path) = target else { return };
        self.settle_pending_relayout();
        match io::save_path_counted(&self.doc, &path) {
            Ok((p, replaced)) => {
                if replaced > 0 {
                    self.doc.neutralize_non_finite();
                    self.hist.neutralize_non_finite();
                    self.changed();
                }
                let notice = io::non_finite_notice_once(&p, replaced);
                match notice {
                    Some(w) => self.show_toast(format!("Saved {} ({w})", p.display()), true),
                    None => self.show_toast(format!("Saved {}", p.display()), false),
                }
                if self.lib_file.is_none() {
                    if let Some(old) = self.file_path.replace(p.clone()) {
                        self.release_repaired_original(&old, Some(&p));
                    }
                    self.autosave.saved_hash = crate::home::doc_hash(&self.doc);
                    self.autosave.seen_rev = self.canvas.tiles.rev();
                    self.autosave.disk_mtime = std::fs::metadata(&p).ok().and_then(|m| m.modified().ok());
                }
            }
            Err(e) => self.show_toast(e, true),
        }
    }

    pub fn new_document(&mut self) {
        let mut d = Document::new("Untitled");
        d.add_page("Page 1");
        self.replace_document(d, None);
    }

    /// A new, empty board (whiteboard) document.
    pub fn new_board_document(&mut self) {
        self.replace_document(crate::board::create::new_document("Untitled"), None);
    }

    pub fn export_selection(&mut self) {
        let Some(id) = self.selection.first().copied() else {
            self.show_toast("Select a layer to export", true);
            return;
        };
        let name = self.doc.get(id).map(|n| n.name.clone()).unwrap_or_default();
        let Some(path) = io::pick_export(&name) else { return };
        let png = designos_export::export_png(&self.export_doc(), id, 1.0);
        if png.is_empty() {
            self.show_toast("Nothing to export", true);
            return;
        }
        match std::fs::write(&path, png) {
            Ok(()) => self.show_toast(format!("Exported {}", path.display()), false),
            Err(e) => self.show_toast(format!("Export failed: {e}"), true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_rects() -> (App, Vec<NodeId>) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut ids = vec![];
        for i in 0..3 {
            let id = d.alloc_id();
            let r = d.insert(page, Node::rectangle(id, Rect::new(i as f32 * 100.0, 20.0 * i as f32, 50.0, 50.0))).unwrap();
            ids.push(r);
        }
        (App::new(d), ids)
    }

    #[test]
    fn group_and_undo() {
        let (mut app, ids) = app_with_rects();
        app.selection = vec![ids[0], ids[1]];
        app.group_selection(false);
        let g = app.selection[0];
        let gn = app.doc.get(g).unwrap();
        assert!(matches!(gn.kind, NodeKind::Group));
        assert_eq!(gn.children.len(), 2);
        assert_eq!(gn.size, v2(150.0, 70.0));
        // world position preserved
        assert_eq!(world_aabb(&app.doc, ids[1]).x, 100.0);
        app.undo();
        assert_eq!(app.doc.get(ids[0]).unwrap().parent, Some(app.page));
        assert_eq!(app.doc.get(app.page).unwrap().children.len(), 3);
    }

    #[test]
    fn delete_duplicate_paste() {
        let (mut app, ids) = app_with_rects();
        app.selection = vec![ids[0]];
        app.duplicate_selection();
        assert_eq!(app.doc.get(app.page).unwrap().children.len(), 4);
        assert_ne!(app.selection[0], ids[0]);
        app.delete_selection();
        assert_eq!(app.doc.get(app.page).unwrap().children.len(), 3);
        app.selection = vec![ids[2]];
        let json = app.copy_selection().unwrap();
        app.paste(Some(&json));
        assert_eq!(app.doc.get(app.page).unwrap().children.len(), 4);
        app.undo();
        assert_eq!(app.doc.get(app.page).unwrap().children.len(), 3);
    }

    /// An instance (slot id "9:9") holding one child (slot id "9:10") on the page.
    fn app_with_slot_instance() -> (App, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let iid = d.alloc_id();
        let mut inst = Node::frame(iid, Rect::new(0.0, 0.0, 100.0, 100.0));
        let frame = inst.kind.frame_props().cloned().unwrap();
        inst.kind = NodeKind::Instance { frame, component: None, component_key: None };
        inst.component.slot_content_id = Some("9:9".into());
        let inst = d.insert(page, inst).unwrap();
        let cid = d.alloc_id();
        let mut child = Node::rectangle(cid, Rect::new(5.0, 5.0, 10.0, 10.0));
        child.component.slot_content_id = Some("9:10".into());
        let child = d.insert(inst, child).unwrap();
        (App::new(d), inst, child)
    }

    fn slot_ids(app: &App, id: NodeId) -> Vec<Option<String>> {
        let n = app.doc.get(id).unwrap();
        std::iter::once(id).chain(n.children.iter().copied()).map(|i| app.doc.get(i).unwrap().component.slot_content_id.clone()).collect()
    }

    #[test]
    fn duplicate_gives_the_copy_its_own_slot_content_id() {
        let (mut app, inst, child) = app_with_slot_instance();
        app.selection = vec![inst];
        app.duplicate_selection();
        let copy = app.selection[0];
        assert_ne!(copy, inst);
        assert_eq!(slot_ids(&app, inst), vec![Some("9:9".into()), Some("9:10".into())], "original keeps its ids");
        assert_eq!(slot_ids(&app, copy), vec![None, None], "copy must not share the original's slot content ids");
        assert_eq!(app.doc.get(child).unwrap().component.slot_content_id.as_deref(), Some("9:10"));
    }

    #[test]
    fn pasting_twice_never_shares_a_slot_content_id() {
        let (mut app, inst, _) = app_with_slot_instance();
        app.selection = vec![inst];
        let json = app.copy_selection().unwrap();
        for text in [Some(json.as_str()), None] {
            app.paste(text);
            let pasted = app.selection[0];
            assert_ne!(pasted, inst);
            assert_eq!(slot_ids(&app, pasted), vec![None, None]);
        }
        assert_eq!(slot_ids(&app, inst), vec![Some("9:9".into()), Some("9:10".into())]);
    }

    #[test]
    fn align_left_multi() {
        let (mut app, ids) = app_with_rects();
        app.selection = ids.clone();
        app.align(Align::Left);
        for id in ids {
            assert_eq!(world_aabb(&app.doc, id).x, 0.0);
        }
    }

    #[test]
    fn nudge_and_reorder() {
        let (mut app, ids) = app_with_rects();
        app.selection = vec![ids[0]];
        app.nudge(10.0, 0.0);
        assert_eq!(app.doc.get(ids[0]).unwrap().transform.tx, 10.0);
        app.reorder(true, true);
        assert_eq!(app.doc.get(app.page).unwrap().children.last(), Some(&ids[0]));
        app.reorder(false, true);
        assert_eq!(app.doc.get(app.page).unwrap().children.first(), Some(&ids[0]));
    }

    #[test]
    fn auto_layout_wrap_orders_by_x() {
        let (mut app, ids) = app_with_rects();
        app.selection = vec![ids[2], ids[0]];
        app.toggle_auto_layout();
        let g = app.selection[0];
        let gn = app.doc.get(g).unwrap();
        assert!(gn.kind.frame_props().unwrap().auto_layout.is_some());
        assert_eq!(gn.children, vec![ids[0], ids[2]]);
        // 10 padding + 50 + 10 spacing + 50 + 10 padding
        assert_eq!(gn.size.x, 130.0);
    }

    #[test]
    fn move_to_parent_keeps_world_position() {
        let (mut app, ids) = app_with_rects();
        let fid = app.doc.alloc_id();
        let frame = app.doc.insert(app.page, Node::frame(fid, Rect::new(500.0, 500.0, 300.0, 300.0))).unwrap();
        let before = world_aabb(&app.doc, ids[1]);
        app.move_to_parent(&[ids[1]], frame, 0);
        assert_eq!(app.doc.get(ids[1]).unwrap().parent, Some(frame));
        let after = world_aabb(&app.doc, ids[1]);
        assert_eq!((before.x, before.y), (after.x, after.y));
        app.undo();
        assert_eq!(app.doc.get(ids[1]).unwrap().parent, Some(app.page));
    }

    #[test]
    fn ungroup_restores_children() {
        let (mut app, ids) = app_with_rects();
        app.selection = vec![ids[0], ids[1]];
        app.group_selection(false);
        app.ungroup_selection();
        assert_eq!(app.doc.get(ids[0]).unwrap().parent, Some(app.page));
        assert_eq!(world_aabb(&app.doc, ids[1]).x, 100.0);
    }

    #[test]
    fn pages_add_rename_duplicate_delete_are_undoable() {
        let (mut app, ids) = app_with_rects();
        let p1 = app.page;
        let p2 = app.add_page().unwrap();
        assert_eq!(app.doc.pages, vec![p1, p2]);
        assert_eq!(app.page, p2);
        app.rename(p2, "Second");
        assert_eq!(app.doc.get(p2).unwrap().name, "Second");
        app.undo();
        assert_eq!(app.doc.get(p2).unwrap().name, "Page 2");
        app.undo();
        assert_eq!(app.doc.pages, vec![p1]);
        app.redo();
        assert_eq!(app.doc.pages, vec![p1, p2]);

        let copy = app.duplicate_page(p1).unwrap();
        assert_eq!(app.doc.pages, vec![p1, copy, p2]);
        assert_eq!(app.doc.get(copy).unwrap().children.len(), ids.len());
        assert!(app.doc.get(copy).unwrap().name.ends_with("copy"));
        assert!(app.doc.get(copy).unwrap().children.iter().all(|c| !ids.contains(c)), "fresh ids");

        assert!(app.delete_page(copy));
        assert_eq!(app.doc.pages, vec![p1, p2]);
        assert!(app.doc.nodes.contains_key(&app.page));
        app.undo();
        assert_eq!(app.doc.pages, vec![p1, copy, p2]);
        assert_eq!(app.doc.get(copy).unwrap().children.len(), ids.len());
    }

    #[test]
    fn last_page_cannot_be_deleted_and_pages_reorder() {
        let (mut app, _ids) = app_with_rects();
        let p1 = app.page;
        assert!(!app.delete_page(p1));
        assert_eq!(app.doc.pages.len(), 1);
        let p2 = app.add_page().unwrap();
        let p3 = app.add_page().unwrap();
        app.move_page(p3, 0);
        assert_eq!(app.doc.pages, vec![p3, p1, p2]);
        app.undo();
        assert_eq!(app.doc.pages, vec![p1, p2, p3]);
        assert!(app.delete_page(p1));
        assert!(!app.delete_page(p2) || app.doc.pages.len() == 1);
        assert!(!app.doc.pages.is_empty());
        assert!(app.doc.nodes.contains_key(&app.page));
    }

    #[test]
    fn multi_rotate_pivots_about_bbox_center() {
        let (mut app, ids) = app_with_rects();
        app.selection = vec![ids[0], ids[2]];
        let bb = app.selection_bounds().unwrap();
        let c0 = rect_center(&bb);
        app.rotate_selection(std::f32::consts::FRAC_PI_2);
        for id in [ids[0], ids[2]] {
            let n = app.doc.get(id).unwrap();
            assert!((n.transform.rotation_rad() - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        }
        // the union box (now rotated) is centred on the same point and swapped in shape
        let bb1 = app.selection_bounds().unwrap();
        let c1 = rect_center(&bb1);
        assert!((c0.x - c1.x).abs() < 1e-2 && (c0.y - c1.y).abs() < 1e-2, "{c0:?} vs {c1:?}");
        assert!(dist(c0, c1) < 1e-2);
        assert!((bb1.w - bb.h).abs() < 1e-2 && (bb1.h - bb.w).abs() < 1e-2);
        // untouched node did not move
        assert_eq!(app.doc.get(ids[1]).unwrap().transform.tx, 100.0);
        app.undo();
        assert_eq!(app.doc.get(ids[0]).unwrap().transform.rotation_rad(), 0.0);
        assert!(!app.hist.can_undo());
    }

    #[test]
    fn flip_mirrors_about_selection_center() {
        let (mut app, ids) = app_with_rects();
        app.selection = vec![ids[0], ids[2]];
        let bb = app.selection_bounds().unwrap();
        let before: Vec<Rect> = ids.iter().map(|i| world_aabb(&app.doc, *i)).collect();
        app.flip_selection(true);
        let after: Vec<Rect> = ids.iter().map(|i| world_aabb(&app.doc, *i)).collect();
        // x mirrored inside the union box, y untouched, size kept
        assert!((after[0].x - (bb.x + bb.w - (before[0].x - bb.x) - before[0].w)).abs() < 1e-3);
        assert!((after[2].x - (bb.x + bb.w - (before[2].x - bb.x) - before[2].w)).abs() < 1e-3);
        assert_eq!((after[0].y, after[0].w), (before[0].y, before[0].w));
        assert_eq!(after[1], before[1], "unselected node untouched");
        app.flip_selection(true);
        let again = world_aabb(&app.doc, ids[0]);
        assert!((again.x - before[0].x).abs() < 1e-3, "flipping twice restores");
        app.undo();
        app.undo();
        assert_eq!(world_aabb(&app.doc, ids[0]), before[0]);
        app.flip_selection(false);
        let v = world_aabb(&app.doc, ids[0]);
        assert!((v.y - (bb.y + bb.h - (before[0].y - bb.y) - before[0].h)).abs() < 1e-3);
    }

    #[test]
    fn distribute_evens_out_gaps() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut ids = vec![];
        for x in [0.0, 60.0, 300.0] {
            let id = d.alloc_id();
            ids.push(d.insert(page, Node::rectangle(id, Rect::new(x, 0.0, 50.0, 50.0))).unwrap());
        }
        let mut app = App::new(d);
        app.selection = ids.clone();
        assert!(app.distribute_selection());
        let xs: Vec<f32> = ids.iter().map(|i| world_aabb(&app.doc, *i).x).collect();
        assert_eq!(xs, vec![0.0, 150.0, 300.0]);
        app.undo();
        assert_eq!(world_aabb(&app.doc, ids[1]).x, 60.0);
        app.selection = ids[..2].to_vec();
        assert!(!app.distribute_selection(), "needs three layers");
    }

    #[test]
    fn paste_over_selection_paste_to_replace_and_copy_paste_properties() {
        let (mut app, ids) = app_with_rects();
        if let Some(n0) = app.doc.get_mut(ids[0]) {
            n0.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
            n0.corner_radii = [12.0; 4];
            n0.opacity = 0.75;
        }
        // 1. Copy properties from ids[0] and paste onto ids[1]
        app.selection = vec![ids[0]];
        app.copy_properties();
        app.selection = vec![ids[1]];
        app.paste_properties();
        assert_eq!(app.doc.get(ids[1]).unwrap().corner_radii, [12.0; 4]);
        assert!((app.doc.get(ids[1]).unwrap().opacity - 0.75).abs() < 1e-4);
        app.undo();
        assert_eq!(app.doc.get(ids[1]).unwrap().corner_radii, [0.0; 4]);

        // 2. Copy ids[0], select ids[2] (at x=200, y=40), and paste_over_selection
        app.selection = vec![ids[0]];
        app.copy_selection();
        app.selection = vec![ids[2]];
        app.paste_over_selection();
        let pasted_id = app.selection[0];
        assert_ne!(pasted_id, ids[0]);
        assert_eq!(app.doc.get(pasted_id).unwrap().transform.tx, 200.0);
        assert_eq!(app.doc.get(pasted_id).unwrap().transform.ty, 40.0);
        app.undo();

        // 3. Paste to replace ids[1] (at x=100, y=20) preserves child index in parent
        let orig_idx = app.doc.get(app.page).unwrap().children.iter().position(|&c| c == ids[1]).unwrap();
        app.selection = vec![ids[1]];
        app.paste_to_replace();
        let replaced_id = app.selection[0];
        assert!(app.doc.get(ids[1]).is_none());
        assert_eq!(app.doc.get(replaced_id).unwrap().transform.tx, 100.0);
        assert_eq!(app.doc.get(replaced_id).unwrap().transform.ty, 20.0);
        assert_eq!(app.doc.get(app.page).unwrap().children[orig_idx], replaced_id);
    }

    #[test]
    fn select_matching_layers_across_sibling_frames() {
        let mut d = Document::new("multi-edit-q");
        let page = d.add_page("p");
        let f1 = d.alloc_id();
        let f2 = d.alloc_id();
        d.insert(page, Node::frame(f1, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        d.insert(page, Node::frame(f2, Rect::new(240.0, 0.0, 200.0, 200.0))).unwrap();

        let r1 = d.alloc_id();
        let mut nr1 = Node::rectangle(r1, Rect::new(10.0, 10.0, 80.0, 40.0));
        nr1.name = "CTA Button".into();
        d.insert(f1, nr1).unwrap();

        let r2 = d.alloc_id();
        let mut nr2 = Node::rectangle(r2, Rect::new(10.0, 10.0, 80.0, 40.0));
        nr2.name = "CTA Button".into();
        d.insert(f2, nr2).unwrap();

        let other = d.alloc_id();
        let mut nother = Node::rectangle(other, Rect::new(10.0, 60.0, 80.0, 40.0));
        nother.name = "Other".into();
        d.insert(f2, nother).unwrap();

        let mut app = App::new(d);
        app.selection = vec![r1];
        let matched = app.select_matching_layers();
        assert_eq!(matched, 2);
        assert!(app.selection.contains(&r1));
        assert!(app.selection.contains(&r2));
        assert!(!app.selection.contains(&other));
    }

    #[test]
    fn table_grid_insert_and_delete_row_and_column_are_undoable() {
        let mut d = Document::new("grid-test");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 300.0));
        if let Some(fp) = f.kind.frame_props_mut() {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Grid, grid_columns: 2, grid_rows: 2, ..Default::default() });
        }
        d.insert(page, f).unwrap();
        let mut app = App::new(d);
        app.selection = vec![fid];
        app.insert_table_row();
        app.insert_table_column();
        let al = app.doc.get(fid).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al.grid_rows, 3);
        assert_eq!(al.grid_columns, 3);
        app.delete_table_row();
        app.delete_table_column();
        let al2 = app.doc.get(fid).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al2.grid_rows, 2);
        assert_eq!(al2.grid_columns, 2);
    }

    #[test]
    fn deleting_a_grid_column_moves_children_pinned_to_it_into_a_free_cell() {
        let mut d = Document::new("grid-pins");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 300.0, 200.0));
        if let Some(fp) = f.kind.frame_props_mut() {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Grid, grid_columns: 3, grid_rows: 2, ..Default::default() });
        }
        d.insert(page, f).unwrap();
        let mut ids = Vec::new();
        for column in 0..3u16 {
            let id = d.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
            n.layout_child.grid_row = Some(0);
            n.layout_child.grid_column = Some(column);
            ids.push(d.insert(fid, n).unwrap());
        }
        let mut app = App::new(d);
        app.selection = vec![fid];
        app.delete_table_column();
        let origin = |i: usize| {
            let t = app.doc.get(ids[i]).unwrap().transform;
            (t.tx, t.ty)
        };
        // two 150 px columns remain; the child that lost column 2 takes the first free cell instead of stacking on column 1
        assert_eq!([origin(0), origin(1), origin(2)], [(0.0, 0.0), (150.0, 0.0), (0.0, 10.0)]);
    }

    #[test]
    fn new_auto_layout_frames_created_in_studio_default_to_version_1() {
        let mut d = Document::new("v1-default");
        let page = d.add_page("p");
        let r1 = d.alloc_id();
        let r2 = d.alloc_id();
        d.insert(page, Node::rectangle(r1, Rect::new(0.0, 0.0, 40.0, 20.0))).unwrap();
        d.insert(page, Node::rectangle(r2, Rect::new(50.0, 0.0, 40.0, 20.0))).unwrap();

        let mut app = App::new(d);
        app.selection = vec![r1, r2];
        app.toggle_auto_layout();
        let wrapped = app.selection[0];
        let al = app.doc.get(wrapped).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al.layout_version, 1);

        // Toggling auto-layout onto an existing plain frame in Studio also sets version 1.
        let fid = app.doc.alloc_id();
        app.doc.insert(page, Node::frame(fid, Rect::new(0.0, 100.0, 120.0, 80.0))).unwrap();
        app.selection = vec![fid];
        app.toggle_auto_layout();
        let al2 = app.doc.get(fid).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al2.layout_version, 1);
    }

    #[test]
    fn imported_v0_frame_stays_v0_until_update_auto_layout_upgrades_and_relayouts_in_one_undo_step() {
        let rest_json = r#"{
            "name": "Legacy v0 Stack",
            "document": {
                "id": "0:0", "name": "Document", "type": "DOCUMENT",
                "children": [{
                    "id": "0:1", "name": "Page 1", "type": "CANVAS",
                    "children": [{
                        "id": "1:2", "name": "LegacyFrame", "type": "FRAME",
                        "layoutMode": "HORIZONTAL",
                        "primaryAxisSizingMode": "AUTO",
                        "counterAxisSizingMode": "AUTO",
                        "paddingTop": 10.0, "paddingRight": 10.0, "paddingBottom": 10.0, "paddingLeft": 10.0,
                        "itemSpacing": 8.0,
                        "strokesIncludedInLayout": true,
                        "strokes": [{"type": "SOLID", "visible": true, "opacity": 1.0, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}}],
                        "strokeWeight": 4.0,
                        "strokeAlign": "INSIDE",
                        "absoluteBoundingBox": {"x": 0.0, "y": 0.0, "width": 100.0, "height": 50.0},
                        "children": [
                            {"id": "1:3", "name": "ChildA", "type": "RECTANGLE", "absoluteBoundingBox": {"x": 14.0, "y": 14.0, "width": 50.0, "height": 20.0}},
                            {"id": "1:4", "name": "ChildB", "type": "RECTANGLE", "absoluteBoundingBox": {"x": 72.0, "y": 14.0, "width": 30.0, "height": 20.0}}
                        ]
                    }]
                }]
            }
        }"#;
        let mut doc = designos_fig::import_rest_json(rest_json).unwrap();
        let fid = doc.nodes.values().find(|n| n.name == "LegacyFrame").unwrap().id;
        let cid_a = doc.nodes.values().find(|n| n.name == "ChildA").unwrap().id;
        let cid_b = doc.nodes.values().find(|n| n.name == "ChildB").unwrap().id;
        doc.relayout_subtree(fid);

        let mut app = App::new(doc);
        // Imported v0 frame keeps version 0 and includes 4px inside stroke in layout inset.
        let al0 = app.doc.get(fid).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al0.layout_version, 0);
        assert!(al0.stroke_included);
        assert_eq!(app.doc.get(fid).unwrap().size, Vec2::new(116.0, 48.0));
        assert_eq!((app.doc.get(cid_a).unwrap().transform.tx, app.doc.get(cid_a).unwrap().transform.ty), (14.0, 14.0));
        assert_eq!((app.doc.get(cid_b).unwrap().transform.tx, app.doc.get(cid_b).unwrap().transform.ty), (72.0, 14.0));

        // Trigger "Update auto layout" on selection: upgrades to v1 and re-lays out as one undo step.
        app.selection = vec![fid];
        assert_eq!(app.upgrade_auto_layout_version(), 1);
        let al1 = app.doc.get(fid).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al1.layout_version, 1);
        assert_eq!(app.doc.get(fid).unwrap().size, Vec2::new(108.0, 40.0));
        assert_eq!((app.doc.get(cid_a).unwrap().transform.tx, app.doc.get(cid_a).unwrap().transform.ty), (10.0, 10.0));
        assert_eq!((app.doc.get(cid_b).unwrap().transform.tx, app.doc.get(cid_b).unwrap().transform.ty), (68.0, 10.0));

        // Single undo restores both version 0 and the version-0 child positions/frame size.
        app.undo();
        let al_undo = app.doc.get(fid).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al_undo.layout_version, 0);
        assert_eq!(app.doc.get(fid).unwrap().size, Vec2::new(116.0, 48.0));
        assert_eq!((app.doc.get(cid_a).unwrap().transform.tx, app.doc.get(cid_a).unwrap().transform.ty), (14.0, 14.0));
        assert_eq!((app.doc.get(cid_b).unwrap().transform.tx, app.doc.get(cid_b).unwrap().transform.ty), (72.0, 14.0));
    }

    #[test]
    fn keyboard_resize_of_an_imported_stretch_child_ends_the_stretch_on_the_counter_axis() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0));
        let al = AutoLayout { direction: LayoutDirection::Horizontal, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        frame.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
        let fid = d.insert(page, frame).unwrap();
        let cid = d.alloc_id();
        let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 30.0, 20.0));
        child.layout_child.vertical = Sizing::Fill;
        child.layout_child.align_self = AlignSelf::Stretch;
        let cid = d.insert(fid, child).unwrap();
        let mut app = App::new(d);
        app.selection = vec![cid];
        app.resize_selection_by(0.0, 10.0);
        let n = app.doc.get(cid).unwrap();
        assert_eq!(n.layout_child.align_self, AlignSelf::Auto);
        assert_eq!(n.size, v2(30.0, 30.0), "the typed height sticks instead of snapping back to the stretched 100");
    }
}
