//! Edit sessions: mutate the document directly (for live previews while
//! dragging) while remembering the original state of every touched node, then
//! turn the net change into one undoable `Command::Batch`.

use designos_doc::{Command, DocError, Document, History, Node, NodeId};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Edit {
    before: BTreeMap<NodeId, Node>,
    /// [`Document::layout_epoch`] when this edit first touched the document: the baseline for
    /// telling whether layout was written outside every step while the edit stayed open.
    opened_at: Option<u64>,
}

impl Edit {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the document's layout epoch if this edit has not yet (first call wins). Every way an
    /// edit starts to change the document calls it, so the value is the document's state when the
    /// edit began, not when it commits.
    pub fn note_opened(&mut self, doc: &Document) {
        self.opened_at.get_or_insert_with(|| doc.layout_epoch());
    }

    /// [`Document::layout_epoch`] when this edit first touched the document, or `None` for an edit
    /// that has not touched it. Read it before the edit commits and give it to
    /// [`History::mark_last_undo_as_layout_recorded`].
    pub fn opened_at(&self) -> Option<u64> {
        self.opened_at
    }

    /// Takes the document's layout epoch as this edit's baseline again, for an edit that has opened (one that has
    /// not stays unopened). [`Edit::opened_at`] then says the document was at this epoch when the edit's own
    /// work began, so [`History::mark_last_undo_as_layout_recorded`] counts layout written before this call as
    /// the edit's.
    ///
    /// The caller vouches for that: every layout write made since the edit opened landed on a node this edit
    /// holds, with its original state recorded before the write. An operation that records its whole write
    /// scope first and then solves layout on the live document may say so. One that can write outside what it
    /// recorded must not: its step would replay without a solve and leave those nodes where the write put them.
    pub fn rebase_layout_epoch(&mut self, doc: &Document) {
        if let Some(at) = self.opened_at.as_mut() {
            *at = doc.layout_epoch();
        }
    }

    /// Records the original state of `id` (first touch wins).
    pub fn touch(&mut self, doc: &Document, id: NodeId) {
        self.note_opened(doc);
        if let std::collections::btree_map::Entry::Vacant(slot) = self.before.entry(id) {
            if let Some(n) = doc.get(id) {
                slot.insert(n.clone());
            }
        }
    }

    /// Ids of every node touched so far.
    pub fn touched(&self) -> Vec<NodeId> {
        self.before.keys().copied().collect()
    }

    /// Original state of every node touched in this edit session.
    pub fn before_nodes(&self) -> &BTreeMap<NodeId, Node> {
        &self.before
    }

    /// Mutable access to a node, recording its original state.
    pub fn node<'a>(&mut self, doc: &'a mut Document, id: NodeId) -> Option<&'a mut Node> {
        self.touch(doc, id);
        doc.get_mut(id)
    }

    /// Puts every touched node back to its original state.
    pub fn revert(&self, doc: &mut Document) {
        for (id, n) in &self.before {
            if let Some(cur) = doc.get_mut(*id) {
                // Structure (parent/children) is owned by structural commands.
                let (parent, children) = (cur.parent, std::mem::take(&mut cur.children));
                *cur = n.clone();
                cur.parent = parent;
                cur.children = children;
            }
        }
    }

    /// Forgets every touched node whose properties are what they were when it was touched, so the edit holds
    /// only what it changed. For an operation that writes straight into the document and records a whole
    /// scope up front: what it left alone then costs nothing in the undo step or in anything that walks the
    /// edit afterwards. A touched node that is no longer in the document stays, as it does for [`Edit::revert`].
    /// The epoch from [`Edit::note_opened`] is kept.
    pub fn retain_changed(&mut self, doc: &Document) {
        self.before.retain(|id, old| match doc.get(*id) {
            Some(cur) => properties_changed(old, cur),
            None => true,
        });
    }

    /// Update commands for every touched node in an edit already filtered by [`Edit::retain_changed`].
    fn updates_retained(&self, doc: &Document) -> Vec<Command> {
        let mut out = Vec::with_capacity(self.before.len());
        for id in self.before.keys() {
            if let Some(cur) = doc.get(*id) {
                out.push(Command::Update { node: Box::new(cur.clone()) });
            }
        }
        out
    }
}

/// Whether `cur` differs from `old` in anything but its place in the tree: parent and children belong to
/// structural commands, not to an edit's property changes.
pub(crate) fn properties_changed(old: &Node, cur: &Node) -> bool {
    let Node {
        id: _,
        parent: _,
        children: _,
        source_id,
        name,
        kind,
        visible,
        locked,
        opacity,
        blend,
        transform,
        size,
        fills,
        stroke,
        effects,
        corner_radii,
        corner_smoothing,
        constraints,
        layout_child,
        is_mask,
        style_refs,
        fill_geometry,
        stroke_geometry,
        component,
        prototype,
        variables,
        export_settings,
        dev,
        slide,
        lock_aspect,
        hyperlink,
        plugin_data,
        detached_symbol_id,
        fig_metadata,
    } = old;
    *visible != cur.visible
        || *locked != cur.locked
        || *opacity != cur.opacity
        || *blend != cur.blend
        || *transform != cur.transform
        || *size != cur.size
        || *corner_radii != cur.corner_radii
        || *corner_smoothing != cur.corner_smoothing
        || *constraints != cur.constraints
        || *layout_child != cur.layout_child
        || *is_mask != cur.is_mask
        || *lock_aspect != cur.lock_aspect
        || *name != cur.name
        || *fills != cur.fills
        || *stroke != cur.stroke
        || *effects != cur.effects
        || *style_refs != cur.style_refs
        || *component != cur.component
        || *prototype != cur.prototype
        || *variables != cur.variables
        || *export_settings != cur.export_settings
        || *dev != cur.dev
        || *slide != cur.slide
        || *source_id != cur.source_id
        || *hyperlink != cur.hyperlink
        || *detached_symbol_id != cur.detached_symbol_id
        || *plugin_data != cur.plugin_data
        || *fig_metadata != cur.fig_metadata
        || *kind != cur.kind
        || *fill_geometry != cur.fill_geometry
        || *stroke_geometry != cur.stroke_geometry
}

/// A transaction: structural commands applied immediately plus an [`Edit`] for
/// property changes, committed as one undo step.
#[derive(Default)]
pub struct Tx {
    pub ed: Edit,
    cmds: Vec<Command>,
    inverses: Vec<Command>,
    poisoned: bool,
}

impl Tx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Applies a structural command now and remembers it for the commit.
    pub fn cmd(&mut self, doc: &mut Document, c: Command) -> Result<(), DocError> {
        self.ed.note_opened(doc);
        let inv = c.clone().apply(doc)?;
        self.cmds.push(c);
        self.inverses.push(inv);
        Ok(())
    }

    /// Applies `cmds` in order as part of this transaction, all or none. When one fails, the ones
    /// already applied are undone and forgotten, so the transaction (and a "Fix all" built on it)
    /// never commits half of a multi-step edit such as an insert followed by a remove.
    pub fn cmds_all_or_none(&mut self, doc: &mut Document, cmds: Vec<Command>) -> Result<(), DocError> {
        let mark = self.mark();
        for c in cmds {
            if let Err(e) = self.cmd(doc, c) {
                return Err(self.rollback_to(doc, mark).err().unwrap_or(e));
            }
        }
        Ok(())
    }

    /// Position in the structural command log, for [`Tx::rollback_to`].
    pub fn mark(&self) -> usize {
        self.cmds.len()
    }

    /// Undoes and forgets the structural commands applied since `mark`. Property edits in
    /// [`Tx::ed`] are untouched, so callers use this for a multi-step edit that fails before it
    /// writes any property. Returns the first error from undoing a command, after trying them all.
    pub fn rollback_to(&mut self, doc: &mut Document, mark: usize) -> Result<(), DocError> {
        let mut rollback_failed = None;
        while self.inverses.len() > mark {
            if let Some(inv) = self.inverses.pop() {
                if let Err(re) = inv.apply(doc) {
                    rollback_failed.get_or_insert(re);
                }
            }
        }
        self.cmds.truncate(mark);
        match rollback_failed {
            Some(re) => {
                // A command that cannot be undone leaves the document half-edited: the transaction
                // must not commit.
                self.poisoned = true;
                Err(re)
            }
            None => Ok(()),
        }
    }

    /// Restores the document to its state before the transaction began.
    pub fn cancel(mut self, doc: &mut Document) {
        self.ed.revert(doc);
        while let Some(inv) = self.inverses.pop() {
            let _ = inv.apply(doc);
        }
    }

    pub fn commit(self, doc: &mut Document, hist: &mut History) -> Result<bool, DocError> {
        self.commit_labeled(doc, hist, None)
    }

    pub fn commit_labeled(self, doc: &mut Document, hist: &mut History, label: Option<String>) -> Result<bool, DocError> {
        self.commit_with_before(doc, hist, label).map(|(changed, _)| changed)
    }

    /// Commits the transaction and returns `(changed, before_nodes)`, where `before_nodes` holds only the
    /// nodes whose properties changed and that still exist in `doc` after the commit.
    pub fn commit_with_before(mut self, doc: &mut Document, hist: &mut History, label: Option<String>) -> Result<(bool, BTreeMap<NodeId, Node>), DocError> {
        if self.poisoned {
            self.cancel(doc);
            return Err(DocError::NotFound(designos_doc::NodeId(0)));
        }
        // connectors follow the nodes they join (and a moved connector its free ends)
        crate::board::connect::reroute_touched(doc, &mut self.ed);
        self.ed.retain_changed(doc);
        let mut updates = self.ed.updates_retained(doc);
        crate::comp_ops::record_overrides(doc, &self.ed.before, &mut updates);
        self.ed.revert(doc);
        while let Some(inv) = self.inverses.pop() {
            inv.apply(doc)?;
        }
        let mut all = std::mem::take(&mut self.cmds);
        all.extend(updates);
        if all.is_empty() {
            return Ok((false, BTreeMap::new()));
        }
        let cmd = if all.len() == 1 { all.pop().unwrap() } else { Command::Batch(all) };
        // Edits to a main component also update its instances, in the same undo step.
        let cmd = crate::comp_ops::with_sync(doc, cmd);
        hist.exec_labeled(doc, cmd, label)?;
        hist.mark_last_undo_as_user();
        self.ed.before.retain(|id, _| doc.get(*id).is_some());
        Ok((true, self.ed.before))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Rect, Vec2};

    #[test]
    fn tx_structural_plus_props_undo_together() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a = d.alloc_id();
        d.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let mut h = History::default();
        let mut tx = Tx::new();
        let nid = d.alloc_id();
        tx.cmd(&mut d, Command::Insert { parent: page, index: 1, nodes: vec![Node::rectangle(nid, Rect::new(0.0, 0.0, 5.0, 5.0))] }).unwrap();
        tx.ed.node(&mut d, nid).unwrap().size = Vec2::new(50.0, 50.0);
        tx.ed.node(&mut d, a).unwrap().opacity = 0.1;
        assert!(tx.commit(&mut d, &mut h).unwrap());
        assert_eq!(d.get(nid).unwrap().size.x, 50.0);
        assert_eq!(d.get(a).unwrap().opacity, 0.1);
        h.undo(&mut d).unwrap();
        assert!(d.get(nid).is_none());
        assert_eq!(d.get(a).unwrap().opacity, 1.0);
        assert!(!h.can_undo());
        h.redo(&mut d).unwrap();
        assert_eq!(d.get(nid).unwrap().size.x, 50.0);
    }

    #[test]
    fn tx_cancel_restores() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut tx = Tx::new();
        let nid = d.alloc_id();
        tx.cmd(&mut d, Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(nid, Rect::new(0.0, 0.0, 5.0, 5.0))] }).unwrap();
        tx.cancel(&mut d);
        assert!(d.get(nid).is_none());
        assert!(d.get(page).unwrap().children.is_empty());
    }

    #[test]
    fn a_failing_step_rolls_back_the_steps_before_it() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let ghost = d.alloc_id();
        let mk = |d: &mut Document| {
            let id = d.alloc_id();
            Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0))] }
        };
        // Control: applying the steps one by one leaves the first one behind when the second fails.
        let mut plain = Tx::new();
        let first = mk(&mut d);
        assert!(plain.cmd(&mut d, first).is_ok());
        assert!(plain.cmd(&mut d, Command::Remove { id: ghost }).is_err());
        assert_eq!(d.get(page).unwrap().children.len(), 1, "a plain step sequence is not atomic");
        plain.cancel(&mut d);
        assert!(d.get(page).unwrap().children.is_empty());

        let mut tx = Tx::new();
        let kept = mk(&mut d);
        tx.cmd(&mut d, kept).unwrap();
        let before = d.get(page).unwrap().children.clone();
        let (a, b) = (mk(&mut d), mk(&mut d));
        assert!(tx.cmds_all_or_none(&mut d, vec![a, b, Command::Remove { id: ghost }]).is_err());
        assert_eq!(d.get(page).unwrap().children, before, "the two inserts were undone");
        assert_eq!((tx.cmds.len(), tx.inverses.len()), (1, 1), "only the earlier step is left in the transaction");
        let ok = mk(&mut d);
        assert!(tx.cmds_all_or_none(&mut d, vec![ok]).is_ok());
        assert_eq!(d.get(page).unwrap().children.len(), before.len() + 1);
        tx.cancel(&mut d);
        assert!(d.get(page).unwrap().children.is_empty(), "cancel still restores everything");
    }

    #[test]
    fn commit_is_single_undo_step() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a = d.alloc_id();
        d.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let b = d.alloc_id();
        d.insert(page, Node::rectangle(b, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let mut h = History::default();
        let mut e = Tx::new();
        e.ed.node(&mut d, a).unwrap().opacity = 0.5;
        e.ed.node(&mut d, b).unwrap().opacity = 0.25;
        assert!(e.commit(&mut d, &mut h).unwrap());
        assert_eq!(d.get(a).unwrap().opacity, 0.5);
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(a).unwrap().opacity, 1.0);
        assert_eq!(d.get(b).unwrap().opacity, 1.0);
        assert!(!h.can_undo());
    }

    #[test]
    fn retain_changed_keeps_only_what_changed_and_commits_the_same_step() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let ids: Vec<NodeId> = (0..3)
            .map(|i| {
                let id = d.alloc_id();
                d.insert(page, Node::rectangle(id, Rect::new(i as f32 * 20.0, 0.0, 10.0, 10.0))).unwrap();
                id
            })
            .collect();
        let (changed, nudged_back, untouched) = (ids[0], ids[1], ids[2]);
        let mut h = History::default();
        let mut tx = Tx::new();
        for id in &ids {
            tx.ed.touch(&d, *id);
        }
        d.get_mut(changed).unwrap().opacity = 0.5;
        d.get_mut(nudged_back).unwrap().opacity = 0.5;
        d.get_mut(nudged_back).unwrap().opacity = 1.0;
        let opened_at = tx.ed.opened_at();
        assert!(opened_at.is_some(), "touching a node opens the edit");

        tx.ed.retain_changed(&d);

        assert_eq!(tx.ed.touched(), vec![changed], "the node written back to its original and the one never written are forgotten");
        assert_eq!(tx.ed.opened_at(), opened_at, "the epoch the edit opened at is kept");
        assert!(tx.commit(&mut d, &mut h).unwrap());
        assert_eq!(d.get(changed).unwrap().opacity, 0.5);
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(changed).unwrap().opacity, 1.0, "one undo restores the node that changed");
        assert_eq!(d.get(untouched).unwrap().opacity, 1.0);
        assert!(!h.can_undo(), "and it was the only step");
    }

    #[test]
    fn retain_changed_ignores_a_change_of_place_in_the_tree() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let frame = d.alloc_id();
        d.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        let mut ed = Edit::new();
        ed.touch(&d, frame);
        let child = d.alloc_id();
        d.insert(frame, Node::rectangle(child, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();

        ed.retain_changed(&d);

        assert!(ed.touched().is_empty(), "children belong to structural commands, so a frame that only gained one did not change");
    }

    #[test]
    fn rebasing_the_layout_epoch_moves_an_opened_edit_to_now_and_leaves_an_unopened_one_alone() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let frame = d.alloc_id();
        d.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        let mut ed = Edit::new();
        d.note_layout_written_outside_history();
        ed.rebase_layout_epoch(&d);
        assert_eq!(ed.opened_at(), None, "an edit that has not touched the document has no baseline to move");

        ed.touch(&d, frame);
        let opened = d.layout_epoch();
        assert_eq!(ed.opened_at(), Some(opened));
        d.note_layout_written_outside_history();
        d.note_layout_written_outside_history();
        assert_ne!(d.layout_epoch(), opened, "precondition: layout was written after the edit opened");

        ed.rebase_layout_epoch(&d);

        assert_eq!(ed.opened_at(), Some(d.layout_epoch()), "the baseline is now");
        ed.retain_changed(&d);
        assert_eq!(ed.opened_at(), Some(d.layout_epoch()), "forgetting what did not change keeps it");
    }

    #[test]
    fn noop_edit_records_nothing() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a = d.alloc_id();
        d.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let mut h = History::default();
        let mut e = Tx::new();
        e.ed.node(&mut d, a);
        assert!(!e.commit(&mut d, &mut h).unwrap());
        assert!(!h.can_undo());
    }

    #[test]
    fn a_failed_rollback_poisons_the_transaction_and_cancels_commit() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let existing = d.alloc_id();
        d.insert(page, Node::rectangle(existing, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let mut h = History::default();
        let mut tx = Tx::new();
        tx.ed.node(&mut d, existing).unwrap().opacity = 0.25;
        let temp = d.alloc_id();
        let ghost = d.alloc_id();
        let insert_temp = Command::Insert { parent: page, index: 1, nodes: vec![Node::rectangle(temp, Rect::new(0.0, 0.0, 5.0, 5.0))] };
        // The batch removes `temp` and then fails on `ghost`, so rolling back `insert_temp` via `Remove { id: temp }` also fails.
        let bad_batch = Command::Batch(vec![Command::Remove { id: temp }, Command::Remove { id: ghost }]);
        assert!(tx.cmds_all_or_none(&mut d, vec![insert_temp, bad_batch]).is_err());
        assert!(tx.is_poisoned(), "a failed rollback inside cmds_all_or_none poisons the transaction");
        assert!(tx.commit(&mut d, &mut h).is_err(), "a poisoned transaction refuses to commit partial state");
        assert_eq!(d.get(existing).unwrap().opacity, 1.0, "earlier edits in the poisoned transaction are reverted");
        assert!(!h.can_undo());
    }
}
