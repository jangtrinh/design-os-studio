//! Edit sessions: mutate the document directly (for live previews while
//! dragging) while remembering the original state of every touched node, then
//! turn the net change into one undoable `Command::Batch`.

use designos_doc::{Command, DocError, Document, History, Node, NodeId};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Edit {
    before: BTreeMap<NodeId, Node>,
}

impl Edit {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the original state of `id` (first touch wins).
    pub fn touch(&mut self, doc: &Document, id: NodeId) {
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

    /// Update commands for every touched node whose properties changed.
    fn updates(&self, doc: &Document) -> Vec<Command> {
        let mut out = Vec::new();
        for (id, old) in &self.before {
            if let Some(cur) = doc.get(*id) {
                let mut o = old.clone();
                o.parent = cur.parent;
                o.children = cur.children.clone();
                if *cur != o {
                    out.push(Command::Update { node: Box::new(cur.clone()) });
                }
            }
        }
        out
    }
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

    pub fn commit(mut self, doc: &mut Document, hist: &mut History) -> Result<bool, DocError> {
        if self.poisoned {
            self.cancel(doc);
            return Err(DocError::NotFound(designos_doc::NodeId(0)));
        }
        // connectors follow the nodes they join (and a moved connector its free ends)
        crate::board::connect::reroute_touched(doc, &mut self.ed);
        let mut updates = self.ed.updates(doc);
        crate::comp_ops::record_overrides(doc, &self.ed.before, &mut updates);
        self.ed.revert(doc);
        while let Some(inv) = self.inverses.pop() {
            inv.apply(doc)?;
        }
        let mut all = std::mem::take(&mut self.cmds);
        all.extend(updates);
        if all.is_empty() {
            return Ok(false);
        }
        let cmd = if all.len() == 1 { all.pop().unwrap() } else { Command::Batch(all) };
        // Edits to a main component also update its instances, in the same undo step.
        let cmd = crate::comp_ops::with_sync(doc, cmd);
        hist.exec(doc, cmd)?;
        Ok(true)
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
