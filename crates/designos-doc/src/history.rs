//! Undo/redo. Every edit goes through [`Command`]; applying returns its inverse.

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    /// Insert subtree nodes (root first) under `parent` at `index`.
    Insert {
        parent: NodeId,
        index: usize,
        nodes: Vec<Node>,
    },
    Remove {
        id: NodeId,
    },
    Reparent {
        id: NodeId,
        parent: NodeId,
        index: usize,
    },
    /// Replace a node's own properties (children/parent are preserved).
    Update {
        node: Box<Node>,
    },
    /// Insert a page subtree (page node first) at `index` in `Document::pages`.
    InsertPage {
        index: usize,
        nodes: Vec<Node>,
    },
    RemovePage {
        id: NodeId,
    },
    /// Replace the document's prototype flow list (flow starting points).
    SetFlows {
        flows: Vec<Flow>,
    },
    /// Replace shared styles and their metadata (style create / edit / delete).
    SetStyles {
        styles: BTreeMap<String, Style>,
        style_meta: BTreeMap<String, StyleMeta>,
    },
    /// Replace all comment threads.
    SetComments {
        comments: Vec<CommentThread>,
    },
    /// Replace library bookkeeping (publish info, enabled libraries, asset copies).
    SetLibraryState {
        state: LibraryState,
        library_page: Option<NodeId>,
    },
    /// Replace the guides of one page.
    SetGuides {
        page: NodeId,
        guides: Vec<Guide>,
    },
    /// Replace the prototype device settings.
    SetPrototypeDevice {
        device: PrototypeDevice,
    },
    /// Replace Check designs settings (rule toggles, contrast level, ignored findings).
    SetCheckDesignsSettings(CheckDesignsSettings),
    /// Replace all variables and collections (variable edits).
    SetVariables {
        variables: BTreeMap<String, Variable>,
        collections: BTreeMap<String, VariableCollection>,
    },
    /// Adds embedded image bytes (a hash already present keeps its bytes) and drops the `remove` hashes.
    /// Its inverse removes what was newly added and puts the removed bytes back, so undo/redo of a paste
    /// leaves no orphaned images and loses none.
    SetImages {
        add: BTreeMap<String, Vec<u8>>,
        remove: Vec<String>,
    },
    Batch(Vec<Command>),
}

impl Command {
    /// Applies the command and returns the command that undoes it.
    pub fn apply(self, doc: &mut Document) -> Result<Command, DocError> {
        Ok(match self {
            Command::Insert { parent, index, nodes } => {
                let root = nodes.first().map(|n| n.id).ok_or(DocError::NotFound(parent))?;
                let p = doc.nodes.get_mut(&parent).ok_or(DocError::NotFound(parent))?;
                if !p.kind.can_hold_children() {
                    return Err(DocError::NotAContainer(parent));
                }
                let index = index.min(p.children.len());
                p.children.insert(index, root);
                let many = nodes.len() > 64;
                for mut n in nodes {
                    if n.id == root {
                        n.parent = Some(parent);
                    }
                    doc.nodes.insert(n.id, n);
                }
                doc.bump_ids();
                if many {
                    doc.mark_tree_full_dirty();
                } else {
                    doc.mark_node_dirty(parent);
                    doc.mark_node_dirty(root);
                }
                Command::Remove { id: root }
            }
            Command::Remove { id } => {
                let parent = doc.get(id).ok_or(DocError::NotFound(id))?.parent.ok_or(DocError::NotFound(id))?;
                let index = doc.get(parent).map(|p| p.children.iter().position(|c| *c == id).unwrap_or(0)).unwrap_or(0);
                let nodes = doc.remove(id)?;
                Command::Insert { parent, index, nodes }
            }
            Command::Reparent { id, parent, index } => {
                let n = doc.get(id).ok_or(DocError::NotFound(id))?;
                let old_parent = n.parent.ok_or(DocError::NotFound(id))?;
                let old_index = doc.get(old_parent).and_then(|p| p.children.iter().position(|c| *c == id)).unwrap_or(0);
                doc.reparent(id, parent, index)?;
                Command::Reparent { id, parent: old_parent, index: old_index }
            }
            Command::Update { mut node } => {
                let cur = doc.get_mut(node.id).ok_or(DocError::NotFound(node.id))?;
                node.parent = cur.parent;
                node.children = std::mem::take(&mut cur.children);
                let old = std::mem::replace(cur, *node);
                Command::Update { node: Box::new(Node { children: Vec::new(), ..old }) }
            }
            Command::InsertPage { index, nodes } => {
                let root = nodes.first().ok_or(DocError::NotFound(NodeId(0)))?;
                if !matches!(root.kind, NodeKind::Page { .. }) {
                    return Err(DocError::NotAContainer(root.id));
                }
                let root = root.id;
                for mut n in nodes {
                    if n.id == root {
                        n.parent = None;
                    }
                    doc.nodes.insert(n.id, n);
                }
                let index = index.min(doc.pages.len());
                doc.pages.insert(index, root);
                doc.bump_ids();
                doc.mark_tree_full_dirty();
                Command::RemovePage { id: root }
            }
            Command::RemovePage { id } => {
                let index = doc.pages.iter().position(|p| *p == id).ok_or(DocError::NotFound(id))?;
                let nodes = doc.remove(id)?;
                Command::InsertPage { index, nodes }
            }
            Command::SetFlows { flows } => Command::SetFlows { flows: std::mem::replace(&mut doc.flows, flows) },
            Command::SetStyles { styles, style_meta } => {
                doc.mark_full_dirty();
                Command::SetStyles { styles: std::mem::replace(&mut doc.styles, styles), style_meta: std::mem::replace(&mut doc.style_meta, style_meta) }
            }
            Command::SetComments { comments } => {
                doc.bump_mutation_epoch();
                Command::SetComments { comments: std::mem::replace(&mut doc.comments, comments) }
            }
            Command::SetLibraryState { state, library_page } => {
                if let Some(lp) = library_page {
                    if let std::collections::btree_map::Entry::Vacant(e) = doc.nodes.entry(lp) {
                        let mut n = Node::new(lp, NodeKind::Page { background: Color::rgb(0x5A, 0x5A, 0x5A) });
                        n.name = "Library".into();
                        e.insert(n);
                        doc.mark_node_dirty(lp);
                    }
                    doc.bump_ids();
                } else if let Some(old_lp) = doc.library_page {
                    if !doc.pages.contains(&old_lp) && doc.nodes.get(&old_lp).is_some_and(|n| n.children.is_empty()) {
                        doc.nodes.remove(&old_lp);
                        doc.mark_node_dirty(old_lp);
                    }
                }
                Command::SetLibraryState {
                    state: std::mem::replace(&mut doc.library, state),
                    library_page: std::mem::replace(&mut doc.library_page, library_page),
                }
            }
            Command::SetGuides { page, guides } => {
                let old = if guides.is_empty() { doc.guides.remove(&page) } else { doc.guides.insert(page, guides) };
                Command::SetGuides { page, guides: old.unwrap_or_default() }
            }
            Command::SetPrototypeDevice { device } => Command::SetPrototypeDevice { device: std::mem::replace(&mut doc.prototype_device, device) },
            Command::SetCheckDesignsSettings(settings) => {
                doc.bump_mutation_epoch();
                Command::SetCheckDesignsSettings(std::mem::replace(&mut doc.check_designs, settings))
            }
            Command::SetVariables { variables, collections } => {
                doc.mark_full_dirty();
                Command::SetVariables {
                    variables: std::mem::replace(&mut doc.variables, variables),
                    collections: std::mem::replace(&mut doc.variable_collections, collections),
                }
            }
            Command::SetImages { add, remove } => {
                let removed: BTreeMap<String, Vec<u8>> = remove.into_iter().filter_map(|h| doc.images.remove(&h).map(|bytes| (h, bytes))).collect();
                let mut added = Vec::new();
                for (hash, bytes) in add {
                    if let std::collections::btree_map::Entry::Vacant(slot) = doc.images.entry(hash) {
                        added.push(slot.key().clone());
                        slot.insert(bytes);
                    }
                }
                Command::SetImages { add: removed, remove: added }
            }
            Command::Batch(cmds) => {
                let mut inv = Vec::with_capacity(cmds.len());
                for c in cmds {
                    inv.push(c.apply(doc)?);
                }
                inv.reverse();
                Command::Batch(inv)
            }
        })
    }

    /// Returns `true` when this command (or its inverse stored on the undo stack) directly targets `id`,
    /// ignoring passive auto-layout reflow updates on `id` (flow-child translation or Hug/Fill/Stretch size
    /// adjustments) while still treating direct user drags of auto-layout frames and fixed-axis resizes as
    /// targeting `id`.
    pub fn targets_node(&self, doc: &Document, id: NodeId, current: Option<&Node>) -> bool {
        match self {
            Command::Insert { nodes, .. } | Command::InsertPage { nodes, .. } => nodes.iter().any(|n| n.id == id),
            Command::Remove { id: rid } | Command::Reparent { id: rid, .. } | Command::RemovePage { id: rid } => *rid == id,
            Command::Update { node } => {
                if node.id != id {
                    return false;
                }
                let Some(after) = current.or_else(|| doc.get(id)) else {
                    return true;
                };
                if node.as_ref() == after {
                    return false;
                }
                if is_auto_layout_reflow(doc, node, after) {
                    return false;
                }
                true
            }
            Command::Batch(cmds) => cmds.iter().any(|c| c.targets_node(doc, id, current)),
            Command::SetFlows { .. }
            | Command::SetStyles { .. }
            | Command::SetComments { .. }
            | Command::SetLibraryState { .. }
            | Command::SetGuides { .. }
            | Command::SetPrototypeDevice { .. }
            | Command::SetVariables { .. }
            | Command::SetImages { .. }
            | Command::SetCheckDesignsSettings(_) => false,
        }
    }
}

fn is_auto_layout_reflow(doc: &Document, before: &Node, after: &Node) -> bool {
    if !only_layout_fields_differ(before, after) {
        return false;
    }
    let linear_same = (before.transform.a - after.transform.a).abs() <= 1e-5
        && (before.transform.b - after.transform.b).abs() <= 1e-5
        && (before.transform.c - after.transform.c).abs() <= 1e-5
        && (before.transform.d - after.transform.d).abs() <= 1e-5;
    if !linear_same {
        return false;
    }

    let parent_al = after.parent.or(before.parent).and_then(|pid| doc.get(pid)).and_then(|p| p.kind.frame_props()).and_then(|f| f.auto_layout.as_ref());
    let in_auto_layout_flow = parent_al.is_some() && !after.layout_child.absolute;

    let translated = (before.transform.tx - after.transform.tx).abs() > 1e-5 || (before.transform.ty - after.transform.ty).abs() > 1e-5;
    if translated && !in_auto_layout_flow {
        return false;
    }

    let (self_hug_w, self_hug_h) = match &after.kind {
        crate::NodeKind::Group => (true, true),
        crate::NodeKind::Text(tp) => (
            tp.auto_resize == crate::TextAutoResize::WidthAndHeight,
            matches!(tp.auto_resize, crate::TextAutoResize::WidthAndHeight | crate::TextAutoResize::Height),
        ),
        _ => after.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map_or((false, false), crate::layout_solver::hugged_axes),
    };

    let parent_stretches_w = in_auto_layout_flow
        && (matches!(after.layout_child.horizontal, crate::Sizing::Hug | crate::Sizing::Fill)
            || parent_al.is_some_and(|al| al.direction == crate::LayoutDirection::Vertical && after.layout_child.align_self == crate::AlignSelf::Stretch));
    let parent_stretches_h = in_auto_layout_flow
        && (matches!(after.layout_child.vertical, crate::Sizing::Hug | crate::Sizing::Fill)
            || parent_al.is_some_and(|al| al.direction == crate::LayoutDirection::Horizontal && after.layout_child.align_self == crate::AlignSelf::Stretch));

    let dx = (before.size.x - after.size.x).abs() > 1e-5;
    let dy = (before.size.y - after.size.y).abs() > 1e-5;
    if dx && !(self_hug_w || parent_stretches_w) {
        return false;
    }
    if dy && !(self_hug_h || parent_stretches_h) {
        return false;
    }

    in_auto_layout_flow || self_hug_w || self_hug_h
}

fn only_layout_fields_differ(before: &Node, after: &Node) -> bool {
    let mut normalized = before.clone();
    normalized.parent = after.parent;
    normalized.children = after.children.clone();
    normalized.transform = after.transform;
    normalized.size = after.size;
    normalized.layout_child.stretched = after.layout_child.stretched;
    normalized == *after
}

impl Document {
    pub(crate) fn bump_ids(&mut self) {
        if let Some((id, _)) = self.nodes.last_key_value() {
            self.next_id = self.next_id.max(id.0);
        }
    }
}

/// Linear undo stack.
#[derive(Default)]
pub struct History {
    undo: Vec<Command>,
    undo_labels: Vec<Option<String>>,
    redo: Vec<Command>,
    redo_labels: Vec<Option<String>>,
}

impl History {
    pub fn exec(&mut self, doc: &mut Document, cmd: Command) -> Result<(), DocError> {
        self.exec_labeled(doc, cmd, None)
    }
    /// Executes `cmd` and records an optional human-readable label on the resulting undo step.
    pub fn exec_labeled(&mut self, doc: &mut Document, cmd: Command, label: Option<String>) -> Result<(), DocError> {
        let inv = cmd.apply(doc)?;
        self.undo.push(inv);
        self.undo_labels.push(label);
        self.redo.clear();
        self.redo_labels.clear();
        Ok(())
    }
    /// Reverts the latest step. Any deferred auto-layout queue from the superseded edit is dropped
    /// before applying the inverse command, and affected auto-layout containers are re-solved from
    /// their restored state so descendants return to the pre-edit layout.
    pub fn undo(&mut self, doc: &mut Document) -> Result<bool, DocError> {
        let Some(c) = self.undo.pop() else { return Ok(false) };
        let lbl = self.undo_labels.pop().flatten();
        let inv = apply_and_relayout(doc, c)?;
        self.redo.push(inv);
        self.redo_labels.push(lbl);
        Ok(true)
    }
    /// Re-applies the latest undone step, dropping any superseded queue and re-solving affected
    /// auto-layout containers from their restored state, like [`Self::undo`].
    pub fn redo(&mut self, doc: &mut Document) -> Result<bool, DocError> {
        let Some(c) = self.redo.pop() else { return Ok(false) };
        let lbl = self.redo_labels.pop().flatten();
        let inv = apply_and_relayout(doc, c)?;
        self.undo.push(inv);
        self.undo_labels.push(lbl);
        Ok(true)
    }
    /// The command [`Self::undo`] would apply next.
    pub fn peek_undo(&self) -> Option<&Command> {
        self.undo.last()
    }
    /// The undo command stored at 1-based depth `depth` (`self.undo[depth - 1]`).
    pub fn undo_at_depth(&self, depth: usize) -> Option<&Command> {
        depth.checked_sub(1).and_then(|i| self.undo.get(i))
    }
    /// The command [`Self::redo`] would apply next.
    pub fn peek_redo(&self) -> Option<&Command> {
        self.redo.last()
    }
    /// The label of the step [`Self::undo`] would apply next, if any.
    pub fn peek_undo_label(&self) -> Option<&str> {
        self.undo_labels.last().and_then(|l| l.as_deref())
    }
    /// The label of the step [`Self::redo`] would apply next, if any.
    pub fn peek_redo_label(&self) -> Option<&str> {
        self.redo_labels.last().and_then(|l| l.as_deref())
    }
    /// Labels of the undo stack, oldest first (aligned with [`Self::undo_depth`]).
    pub fn undo_labels(&self) -> &[Option<String>] {
        &self.undo_labels
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    /// Number of steps [`Self::undo`] can take.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }
    /// Forgets undo steps past `depth` without applying them, for edits the caller already
    /// reverted another way (by restoring a snapshot of the document).
    pub fn drop_undo_steps_after(&mut self, depth: usize) {
        self.undo.truncate(depth);
        self.undo_labels.truncate(depth);
    }
    /// The steps [`Self::redo`] can take, oldest first, for [`Self::restore_redo`].
    pub fn redo_steps(&self) -> Vec<Command> {
        self.redo.clone()
    }
    /// Puts back the redo steps an edit cleared, for an edit the caller reverted another way.
    pub fn restore_redo(&mut self, steps: Vec<Command>) {
        self.redo_labels.resize(steps.len(), None);
        self.redo = steps;
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Replaces any non-finite numbers stored inside undo and redo commands with their neutral values.
    pub fn neutralize_non_finite(&mut self) -> usize {
        let mut replaced = 0;
        for cmd in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            replaced += cmd.neutralize_non_finite();
        }
        replaced
    }
}

impl Command {
    /// Replaces any non-finite numbers carried by this command with their neutral values.
    pub fn neutralize_non_finite(&mut self) -> usize {
        match self {
            Command::Insert { nodes, .. } | Command::InsertPage { nodes, .. } => nodes.iter_mut().map(finite_json::neutralize_in_place).sum(),
            Command::Update { node } => finite_json::neutralize_in_place(node.as_mut()),
            Command::SetFlows { flows } => flows.iter_mut().map(finite_json::neutralize_in_place).sum(),
            Command::SetStyles { styles, style_meta } => {
                styles.values_mut().map(finite_json::neutralize_in_place).sum::<usize>()
                    + style_meta.values_mut().map(finite_json::neutralize_in_place).sum::<usize>()
            }
            Command::SetComments { comments } => comments.iter_mut().map(finite_json::neutralize_in_place).sum(),
            Command::SetLibraryState { state, .. } => finite_json::neutralize_in_place(state),
            Command::SetGuides { guides, .. } => guides.iter_mut().map(finite_json::neutralize_in_place).sum(),
            Command::SetPrototypeDevice { device } => finite_json::neutralize_in_place(device),
            Command::SetVariables { variables, collections } => {
                variables.values_mut().map(finite_json::neutralize_in_place).sum::<usize>()
                    + collections.values_mut().map(finite_json::neutralize_in_place).sum::<usize>()
            }
            Command::Batch(cmds) => cmds.iter_mut().map(Self::neutralize_non_finite).sum(),
            Command::Remove { .. }
            | Command::Reparent { .. }
            | Command::RemovePage { .. }
            | Command::SetImages { .. }
            | Command::SetCheckDesignsSettings(_) => 0,
        }
    }
}

/// Variable binding keys whose resolved value the layout solver reads: the box, the paddings and
/// spacings, visibility, and the border weights and opacity that decide the border inset.
fn is_layout_binding_key(key: &str) -> bool {
    matches!(
        key,
        "width"
            | "height"
            | "visible"
            | "itemSpacing"
            | "counterAxisSpacing"
            | "paddingTop"
            | "paddingRight"
            | "paddingBottom"
            | "paddingLeft"
            | "strokeWeight"
            | "strokeTopWeight"
            | "strokeRightWeight"
            | "strokeBottomWeight"
            | "strokeLeftWeight"
    ) || (key.starts_with("strokes/") && key.ends_with("/opacity"))
}

/// Whether replacing `old` with `new` can change what the auto-layout solver computes, for the node
/// itself or for the frame that lays it out. Only what the solver reads counts: the size, the
/// child sizing and limits, visibility, the frame's own auto-layout settings, a border that takes
/// space (`bordersTakeSpace`, layout version 0), and variable bindings or modes that feed those.
/// Paint colours, effects and the like never do, and neither does the transform: the solver reads
/// a flow child's size only and writes its position, and absolute children stay where they are.
fn layout_inputs_changed(old: &Node, new: &Node) -> bool {
    let old_al = old.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
    let new_al = new.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
    old.size != new.size
        || old.visible != new.visible
        || old.layout_child != new.layout_child
        || std::mem::discriminant(&old.kind) != std::mem::discriminant(&new.kind)
        || old_al != new_al
        || new_al.is_some_and(|al| al.effective_padding(&old.stroke) != al.effective_padding(&new.stroke))
        || old.variables.modes != new.variables.modes
        || !old.variables.bindings.iter().filter(|(k, _)| is_layout_binding_key(k)).eq(new.variables.bindings.iter().filter(|(k, _)| is_layout_binding_key(k)))
}

fn collect_relayout_seeds(doc: &Document, cmd: &Command, seeds: &mut Vec<NodeId>) {
    match cmd {
        Command::Update { node } => {
            let Some(cur) = doc.get(node.id) else { return };
            if !layout_inputs_changed(cur, node) {
                return;
            }
            let is_auto =
                node.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some() || cur.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some();
            if is_auto {
                seeds.push(node.id);
            }
            if let Some(p) = cur.parent {
                seeds.push(p);
            }
        }
        Command::Insert { parent, nodes, .. } => {
            seeds.push(*parent);
            if let Some(root) = nodes.first() {
                if root.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some() {
                    seeds.push(root.id);
                }
            }
        }
        Command::Remove { id } => {
            if let Some(p) = doc.get(*id).and_then(|n| n.parent) {
                seeds.push(p);
            }
        }
        Command::Reparent { id, parent, .. } => {
            if let Some(old_p) = doc.get(*id).and_then(|n| n.parent) {
                seeds.push(old_p);
            }
            seeds.push(*parent);
        }
        Command::Batch(cmds) => {
            for c in cmds {
                collect_relayout_seeds(doc, c, seeds);
            }
        }
        _ => {}
    }
}

/// Applies `c` and re-solves only the auto-layout frames it changed: each seeded frame is solved on
/// its own and the walk climbs to a parent only while a frame's size changed. Containers below a
/// seed are not walked, so untouched siblings keep their positions. `pending_relayout` is left as
/// it is: a queued subtree is always solved against the tree as it stands when the queue is
/// drained, so it stays valid across the step, and a tail this solve defers joins the queue to be
/// finished a bounded step per frame, like any other edit. A command that fails to apply leaves the
/// document and the queue untouched.
fn apply_and_relayout(doc: &mut Document, c: Command) -> Result<Command, DocError> {
    let mut seeds = Vec::new();
    collect_relayout_seeds(doc, &c, &mut seeds);
    let inv = c.apply(doc)?;
    doc.relayout_seeds_upward(&seeds);
    Ok(inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_update_and_remove() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let mut h = History::default();

        let mut n = d.get(id).unwrap().clone();
        n.name = "Renamed".into();
        h.exec(&mut d, Command::Update { node: Box::new(n) }).unwrap();
        assert_eq!(d.get(id).unwrap().name, "Renamed");
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(id).unwrap().name, "Rectangle");

        h.exec(&mut d, Command::Remove { id }).unwrap();
        assert!(d.get(id).is_none());
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(page).unwrap().children, vec![id]);
        h.redo(&mut d).unwrap();
        assert!(d.get(id).is_none());
    }

    /// 70 nested Fill frames under a page, 200 wide; the outermost is the only one that is not Fill,
    /// and a 10 px leaf hugs the right edge of the innermost frame.
    fn nested_fill_frames() -> (Document, Vec<NodeId>, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let mut frames = Vec::new();
        let mut cur = page;
        for i in 0..70 {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 40.0));
            if let NodeKind::Frame(fp) = &mut f.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    primary_align: LayoutAlign::End,
                    ..AutoLayout::default()
                });
            }
            if i > 0 {
                f.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, ..LayoutChild::default() };
            }
            d.insert(cur, f).unwrap();
            frames.push(id);
            cur = id;
        }
        let leaf = d.alloc_id();
        d.insert(cur, Node::rectangle(leaf, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        d.relayout_subtree(frames[0]);
        d.flush_pending_relayout();
        assert_eq!(d.get(leaf).unwrap().transform.tx, 190.0);
        (d, frames, leaf)
    }

    fn widths(d: &Document, frames: &[NodeId]) -> Vec<f32> {
        frames.iter().map(|f| d.get(*f).unwrap().size.x).collect()
    }

    #[test]
    fn undo_and_redo_never_leave_a_stale_layout_queue_to_write_later() {
        let (mut d, frames, leaf) = nested_fill_frames();
        let mut h = History::default();
        let mut wide = d.get(frames[0]).unwrap().clone();
        wide.size.x = 600.0;
        h.exec(&mut d, Command::Update { node: Box::new(wide) }).unwrap();
        d.relayout_subtree(frames[0]);
        assert!(d.relayout_skipped_count() > 0, "precondition: the nest is deeper than one relayout reaches");

        h.undo(&mut d).unwrap();
        assert_eq!(d.get(frames[0]).unwrap().size.x, 200.0, "the command was undone");
        // The undo re-solves from the restored size within the usual hop budget and queues the
        // rest, like any edit; it does not finish the whole 70-deep chain on the calling thread.
        assert!(d.relayout_skipped_count() > 0, "the tail past the hop budget is queued for later frames, not solved on the undo");
        let hop_budget = crate::layout_apply::MAX_RELAYOUT_RESOLVE_DEPTH;
        assert!(widths(&d, &frames)[..hop_budget].iter().all(|w| *w == 200.0), "the frames within the hop budget are restored at once");
        d.flush_pending_relayout();
        let after_undo = widths(&d, &frames);
        assert!(after_undo.iter().all(|w| *w == 200.0), "draining the queue restores the 200 px layout across all 70 frames: {after_undo:?}");
        assert_eq!(d.get(leaf).unwrap().transform.tx, 190.0, "undo restores the leaf position inside the innermost frame");
        assert_eq!(d.relayout_skipped_count(), 0);

        h.redo(&mut d).unwrap();
        assert!(d.relayout_skipped_count() > 0);
        d.flush_pending_relayout();
        assert!(widths(&d, &frames).iter().all(|w| *w == 600.0), "redo brings back the whole edited layout: {:?}", widths(&d, &frames));
        assert_eq!(d.get(leaf).unwrap().transform.tx, 590.0, "redo places the leaf at the right edge of the 600 px frame");
    }

    #[test]
    fn undo_when_second_step_does_not_resize_queued_chain_still_drains_leaf_to_590() {
        let (mut d, frames, leaf) = nested_fill_frames();
        let mut h = History::default();

        let mut wide = d.get(frames[0]).unwrap().clone();
        wide.size.x = 600.0;
        h.exec(&mut d, Command::Update { node: Box::new(wide) }).unwrap();
        d.relayout_subtree(frames[0]);
        let queued_before = d.pending_relayout().to_vec();
        assert!(!queued_before.is_empty(), "precondition: tail of 70-deep chain is queued");

        assert!(apply_and_relayout(&mut d, Command::Remove { id: NodeId(999_999) }).is_err());
        assert_eq!(d.pending_relayout(), queued_before.as_slice(), "failed apply must not lose pending_relayout");

        let mut painted = d.get(frames[0]).unwrap().clone();
        painted.fills = vec![Paint::solid(Color::WHITE)];
        h.exec(&mut d, Command::Update { node: Box::new(painted) }).unwrap();

        h.undo(&mut d).unwrap();
        assert_eq!(d.pending_relayout(), queued_before.as_slice(), "non-layout undo must preserve pending_relayout");
        d.flush_pending_relayout();
        assert!(widths(&d, &frames).iter().all(|w| *w == 600.0), "undoing the fill step must not drop the queued relayout tail: {:?}", widths(&d, &frames));
        assert_eq!(d.get(leaf).unwrap().transform.tx, 590.0, "leaf reaches 590 once the queue drains");

        h.undo(&mut d).unwrap();
        assert!(d.relayout_skipped_count() > 0, "the undo defers the tail past its hop budget");
        d.flush_pending_relayout();
        assert!(widths(&d, &frames).iter().all(|w| *w == 200.0));
        assert_eq!(d.get(leaf).unwrap().transform.tx, 190.0);

        // Also verify when the second step DOES touch a layout input of the outer frame without
        // resizing the queued Fill chain: undoing that step re-solves the outer frame and keeps the tail.
        let mut wide2 = d.get(frames[0]).unwrap().clone();
        wide2.size.x = 600.0;
        h.exec(&mut d, Command::Update { node: Box::new(wide2) }).unwrap();
        d.relayout_subtree(frames[0]);
        assert!(d.relayout_skipped_count() > 0);

        let mut spaced_outer = d.get(frames[0]).unwrap().clone();
        if let NodeKind::Frame(fp) = &mut spaced_outer.kind {
            fp.auto_layout.as_mut().unwrap().item_spacing = 6.0;
        }
        h.exec(&mut d, Command::Update { node: Box::new(spaced_outer) }).unwrap();

        h.undo(&mut d).unwrap();
        d.flush_pending_relayout();
        assert!(widths(&d, &frames).iter().all(|w| *w == 600.0));
        assert_eq!(d.get(leaf).unwrap().transform.tx, 590.0);
    }

    #[test]
    fn undo_and_redo_of_batch_updating_stack_and_direct_child_resolves_fill_sibling() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let stack = d.alloc_id();
        let mut stack_node = Node::frame(stack, Rect::new(0.0, 0.0, 200.0, 40.0));
        if let NodeKind::Frame(fp) = &mut stack_node.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                ..AutoLayout::default()
            });
        }
        d.insert(page, stack_node).unwrap();

        let child_b = d.alloc_id();
        d.insert(stack, Node::rectangle(child_b, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();

        let child_c = d.alloc_id();
        let mut c_node = Node::rectangle(child_c, Rect::new(0.0, 0.0, 10.0, 40.0));
        c_node.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fixed, ..LayoutChild::default() };
        d.insert(stack, c_node).unwrap();

        d.relayout_subtree(stack);
        assert_eq!(d.get(child_c).unwrap().size.x, 160.0);
        assert_eq!(d.get(child_c).unwrap().transform.tx, 40.0);

        let mut h = History::default();
        let mut stack_wide = d.get(stack).unwrap().clone();
        stack_wide.size.x = 500.0;
        let mut b_wide = d.get(child_b).unwrap().clone();
        b_wide.size.x = 80.0;
        h.exec(&mut d, Command::Batch(vec![Command::Update { node: Box::new(stack_wide) }, Command::Update { node: Box::new(b_wide) }])).unwrap();
        d.relayout_subtree(stack);
        assert_eq!(d.get(child_c).unwrap().size.x, 420.0);
        assert_eq!(d.get(child_c).unwrap().transform.tx, 80.0);

        h.undo(&mut d).unwrap();
        assert_eq!(d.get(stack).unwrap().size.x, 200.0);
        assert_eq!(d.get(child_b).unwrap().size.x, 40.0);
        assert_eq!(d.get(child_c).unwrap().size.x, 160.0, "undo must re-solve stack so Fill sibling shrinks back to 160");
        assert_eq!(d.get(child_c).unwrap().transform.tx, 40.0);

        h.redo(&mut d).unwrap();
        assert_eq!(d.get(stack).unwrap().size.x, 500.0);
        assert_eq!(d.get(child_b).unwrap().size.x, 80.0);
        assert_eq!(d.get(child_c).unwrap().size.x, 420.0, "redo must re-solve stack so Fill sibling expands back to 420");
        assert_eq!(d.get(child_c).unwrap().transform.tx, 80.0);
    }

    fn auto_frame(d: &mut Document, parent: NodeId, size: (f32, f32), sizing: Sizing) -> NodeId {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(0.0, 0.0, size.0, size.1));
        if let NodeKind::Frame(fp) = &mut f.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: sizing,
                counter_sizing: sizing,
                layout_version: 1,
                ..AutoLayout::default()
            });
        }
        d.insert(parent, f).unwrap();
        id
    }

    fn visible_stroke(weight: f32, color: Color) -> Stroke {
        Stroke { paints: vec![Paint::solid(color)], weight, align: StrokeAlign::Inside, ..Stroke::default() }
    }

    #[test]
    fn undo_and_redo_of_a_paint_only_stroke_edit_solve_no_frame() {
        let (mut d, frames, leaf) = nested_fill_frames();
        d.get_mut(leaf).unwrap().stroke = visible_stroke(1.0, Color::BLACK);
        d.get_mut(frames[10]).unwrap().stroke = visible_stroke(1.0, Color::BLACK);
        let mut h = History::default();
        for id in [leaf, frames[10]] {
            let mut recoloured = d.get(id).unwrap().clone();
            recoloured.stroke = visible_stroke(1.0, Color::WHITE);
            h.exec(&mut d, Command::Update { node: Box::new(recoloured) }).unwrap();
        }
        let widths_before = widths(&d, &frames);
        crate::layout_apply::reset_solve_calls();
        h.undo(&mut d).unwrap();
        h.undo(&mut d).unwrap();
        h.redo(&mut d).unwrap();
        h.redo(&mut d).unwrap();
        assert_eq!(crate::layout_apply::solve_calls(), 0, "a stroke colour never reaches the layout solver");
        assert_eq!(widths(&d, &frames), widths_before);
        assert_eq!(d.get(leaf).unwrap().transform.tx, 190.0);
    }

    #[test]
    fn undo_and_redo_of_a_move_or_effect_edit_solve_no_frame() {
        let (mut d, _frames, leaf) = nested_fill_frames();
        let mut h = History::default();
        let mut moved = d.get(leaf).unwrap().clone();
        moved.transform.tx = 3.0;
        moved.effects =
            vec![Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 1.0), radius: 2.0, spread: 0.0, visible: true, show_behind: false }];
        h.exec(&mut d, Command::Update { node: Box::new(moved) }).unwrap();
        crate::layout_apply::reset_solve_calls();
        h.undo(&mut d).unwrap();
        h.redo(&mut d).unwrap();
        assert_eq!(crate::layout_apply::solve_calls(), 0, "the solver reads neither a flow child's transform nor its effects");
    }

    #[test]
    fn undo_of_a_leaf_resize_solves_only_the_chain_up_to_where_the_size_holds() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let top = auto_frame(&mut d, page, (400.0, 400.0), Sizing::Fixed);
        let h0 = auto_frame(&mut d, top, (0.0, 0.0), Sizing::Hug);
        let fixed = auto_frame(&mut d, h0, (100.0, 100.0), Sizing::Fixed);
        let h1 = auto_frame(&mut d, fixed, (0.0, 0.0), Sizing::Hug);
        let h2 = auto_frame(&mut d, h1, (0.0, 0.0), Sizing::Hug);
        let leaf = d.alloc_id();
        d.insert(h2, Node::rectangle(leaf, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let card = auto_frame(&mut d, top, (80.0, 80.0), Sizing::Fixed);
        let card_child = d.alloc_id();
        d.insert(card, Node::rectangle(card_child, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
        d.relayout_subtree(top);
        assert_eq!(d.get(h2).unwrap().size.y, 10.0);
        // A position the solver would overwrite if it ever re-solved the untouched sibling card.
        d.get_mut(card_child).unwrap().transform.tx = 15.0;

        let mut h = History::default();
        let mut tall = d.get(leaf).unwrap().clone();
        tall.size.y = 30.0;
        h.exec(&mut d, Command::Update { node: Box::new(tall) }).unwrap();
        d.relayout_subtree(top);
        assert_eq!((d.get(h2).unwrap().size.y, d.get(h1).unwrap().size.y), (30.0, 30.0));
        d.get_mut(card_child).unwrap().transform.tx = 15.0;

        crate::layout_apply::reset_solve_calls();
        h.undo(&mut d).unwrap();
        assert_eq!(crate::layout_apply::solve_calls(), 3, "h2 and h1 changed size; the fixed frame above them held it, so the climb stops there");
        assert_eq!((d.get(h2).unwrap().size.y, d.get(h1).unwrap().size.y), (10.0, 10.0));
        assert_eq!(d.get(card_child).unwrap().transform.tx, 15.0, "the untouched sibling card is not re-solved");

        crate::layout_apply::reset_solve_calls();
        h.redo(&mut d).unwrap();
        assert_eq!(crate::layout_apply::solve_calls(), 3);
        assert_eq!((d.get(h2).unwrap().size.y, d.get(h1).unwrap().size.y), (30.0, 30.0));
        assert_eq!(d.get(card_child).unwrap().transform.tx, 15.0);
    }

    #[test]
    fn stroke_weight_undo_re_solves_only_when_borders_take_space() {
        for (layout_version, stroke_included, expect_solved) in [(0u8, true, true), (1, true, false), (0, false, false)] {
            let mut d = Document::new("t");
            let page = d.add_page("P");
            let stack = auto_frame(&mut d, page, (100.0, 100.0), Sizing::Fixed);
            if let NodeKind::Frame(fp) = &mut d.get_mut(stack).unwrap().kind {
                let al = fp.auto_layout.as_mut().unwrap();
                al.layout_version = layout_version;
                al.stroke_included = stroke_included;
            }
            d.get_mut(stack).unwrap().stroke = visible_stroke(1.0, Color::BLACK);
            let child = d.alloc_id();
            d.insert(stack, Node::rectangle(child, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
            d.relayout_subtree(stack);
            let at_one = d.get(child).unwrap().transform.tx;

            let mut h = History::default();
            let mut thick = d.get(stack).unwrap().clone();
            thick.stroke = visible_stroke(5.0, Color::BLACK);
            h.exec(&mut d, Command::Update { node: Box::new(thick) }).unwrap();
            d.relayout_subtree(stack);
            let at_five = d.get(child).unwrap().transform.tx;
            assert_eq!(at_five != at_one, expect_solved, "version {layout_version} included {stroke_included}: the solver's reading of the border");

            crate::layout_apply::reset_solve_calls();
            h.undo(&mut d).unwrap();
            assert_eq!(crate::layout_apply::solve_calls() > 0, expect_solved, "version {layout_version} included {stroke_included}");
            if expect_solved {
                assert_eq!(d.get(child).unwrap().transform.tx, at_one, "the child returns to the 1 px border position");
            }
        }
    }

    #[test]
    fn undo_with_a_pending_tail_and_an_unrelated_layout_edit_keeps_the_tail_queued() {
        let (mut d, frames, leaf) = nested_fill_frames();
        let mut h = History::default();
        let mut wide = d.get(frames[0]).unwrap().clone();
        wide.size.x = 600.0;
        h.exec(&mut d, Command::Update { node: Box::new(wide) }).unwrap();
        d.relayout_subtree(frames[0]);
        let queued = d.pending_relayout().to_vec();
        assert!(!queued.is_empty());

        let mut spaced = d.get(frames[1]).unwrap().clone();
        if let NodeKind::Frame(fp) = &mut spaced.kind {
            fp.auto_layout.as_mut().unwrap().item_spacing = 4.0;
        }
        h.exec(&mut d, Command::Update { node: Box::new(spaced) }).unwrap();
        h.undo(&mut d).unwrap();
        assert!(queued.iter().all(|q| d.pending_relayout().contains(q)), "an undo elsewhere must not drop the queued tail");
        d.flush_pending_relayout();
        assert_eq!(d.get(leaf).unwrap().transform.tx, 590.0);
    }

    #[test]
    fn undo_redo_images_add_only_what_was_missing() {
        let mut d = Document::new("t");
        d.images.insert("old".into(), vec![9]);
        let mut h = History::default();
        let add: BTreeMap<String, Vec<u8>> = [("old".to_string(), vec![0]), ("new".to_string(), vec![1, 2])].into();
        h.exec(&mut d, Command::SetImages { add, remove: vec![] }).unwrap();
        assert_eq!(d.images["old"], vec![9], "an existing hash keeps its bytes");
        assert_eq!(d.images["new"], vec![1, 2]);
        h.undo(&mut d).unwrap();
        assert_eq!(d.images.keys().collect::<Vec<_>>(), ["old"], "undo removes only the newly added image");
        assert_eq!(d.images["old"], vec![9]);
        h.redo(&mut d).unwrap();
        assert_eq!(d.images["new"], vec![1, 2]);
        h.exec(&mut d, Command::SetImages { add: BTreeMap::new(), remove: vec!["new".into(), "missing".into()] }).unwrap();
        assert!(!d.images.contains_key("new"));
        h.undo(&mut d).unwrap();
        assert_eq!(d.images["new"], vec![1, 2], "undoing a removal puts the bytes back");
    }

    #[test]
    fn undo_redo_flows() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let fid = d.alloc_id();
        let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let flow = Flow { name: "Flow 1".into(), page, start: f, description: String::new() };
        let mut h = History::default();
        h.exec(&mut d, Command::SetFlows { flows: vec![flow.clone()] }).unwrap();
        assert_eq!(d.flows, vec![flow]);
        h.undo(&mut d).unwrap();
        assert!(d.flows.is_empty());
        h.redo(&mut d).unwrap();
        assert_eq!(d.flows.len(), 1);
    }

    #[test]
    fn undo_redo_pages() {
        let mut d = Document::new("t");
        let p1 = d.add_page("P1");
        let pid = d.alloc_id();
        let mut page = Node::new(pid, NodeKind::Page { background: Color::WHITE });
        page.name = "P2".into();
        let mut h = History::default();
        h.exec(&mut d, Command::InsertPage { index: 0, nodes: vec![page] }).unwrap();
        assert_eq!(d.pages, vec![pid, p1]);
        let rid = d.alloc_id();
        d.insert(pid, Node::rectangle(rid, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();

        h.exec(&mut d, Command::RemovePage { id: pid }).unwrap();
        assert_eq!(d.pages, vec![p1]);
        assert!(d.get(rid).is_none());
        h.undo(&mut d).unwrap();
        assert_eq!(d.pages, vec![pid, p1]);
        assert_eq!(d.get(pid).unwrap().children, vec![rid]);
        h.undo(&mut d).unwrap();
        assert_eq!(d.pages, vec![p1]);
    }
}

#[cfg(test)]
mod style_tests {
    use crate::*;

    #[test]
    fn set_styles_undo() {
        let mut d = Document::new("t");
        d.add_page("P");
        let mut h = History::default();
        let mut styles = d.styles.clone();
        styles.insert("s1".into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
        let mut meta = d.style_meta.clone();
        meta.insert("s1".into(), StyleMeta { name: "Brand/White".into(), ..Default::default() });
        h.exec(&mut d, Command::SetStyles { styles, style_meta: meta }).unwrap();
        assert_eq!(d.style_name("s1"), "Brand/White");
        h.undo(&mut d).unwrap();
        assert!(d.styles.is_empty() && d.style_meta.is_empty());
        h.redo(&mut d).unwrap();
        assert_eq!(d.styles.len(), 1);
    }
}
