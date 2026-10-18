//! Applies Check designs fixes through the editor's transaction, so one fix (or "Fix all") is one
//! undo step. Every fix re-validates the layer as it is NOW and refuses when it drifted since the
//! audit, so a stale finding can never overwrite an edit made in between.

use super::{contrast, detached, hardcoded, Finding, Fix};
use crate::app::App;
use crate::edit::Tx;
use crate::layout;
use designos_doc::{set_bound_property, set_text_fills_everywhere, Command, NodeId, NodeKind, OverrideField, PaintKind, Vec2};

/// Order fixes run in within "Fix all": recolouring first, then links to variables and styles, and
/// the swap of a detached frame for an instance last. The swap only proceeds while the frame still
/// equals its main component, so any fix made to the frame or to a layer inside it first makes the
/// swap refuse, instead of the swap deleting the layer a fix was just applied to (or a later fix
/// finding it gone). A bind that a recolour made stale is likewise refused rather than undoing it.
fn priority(fix: &Fix) -> u8 {
    match fix {
        Fix::SetTextColor { .. } => 0,
        Fix::BindVariable { .. } | Fix::ApplyStyle { .. } => 1,
        Fix::Reconnect { .. } => 2,
    }
}

impl App {
    /// Applies one finding's fix as a single undo step. False when nothing changed.
    pub fn check_designs_fix(&mut self, finding: &Finding) -> bool {
        let mut tx = Tx::new();
        if !self.apply_check_fix(&mut tx, finding) {
            tx.cancel(&mut self.doc);
            return false;
        }
        self.run_tx(tx)
    }

    /// Applies every fixable finding as ONE undo step; returns how many fixes landed.
    pub fn check_designs_fix_all(&mut self, findings: &[Finding]) -> usize {
        let mut todo: Vec<&Finding> = findings.iter().filter(|f| f.fix.is_some()).collect();
        todo.sort_by_key(|f| f.fix.as_ref().map_or(u8::MAX, priority));
        let mut tx = Tx::new();
        let applied = todo.into_iter().filter(|f| self.apply_check_fix(&mut tx, f)).count();
        if applied == 0 {
            tx.cancel(&mut self.doc);
            return 0;
        }
        if self.run_tx(tx) {
            applied
        } else {
            0
        }
    }

    fn apply_check_fix(&mut self, tx: &mut Tx, finding: &Finding) -> bool {
        let Some(fix) = &finding.fix else { return false };
        let id = finding.node;
        match fix {
            Fix::BindVariable { key, variable } => {
                if !hardcoded::bind_is_current(&self.doc, id, key, variable) {
                    return false;
                }
                let Some(value) = self.doc.resolve_variable(id, variable) else { return false };
                let Some(n) = tx.ed.node(&mut self.doc, id) else { return false };
                n.variables.bindings.insert(key.clone(), variable.clone());
                set_bound_property(n, key, &value);
                true
            }
            Fix::ApplyStyle { style } => {
                if !hardcoded::style_is_current(&self.doc, id, style) {
                    return false;
                }
                self.apply_style_in(tx, &[id], style);
                true
            }
            Fix::SetTextColor { color } => {
                // The stored colour is what the user was shown; apply it only while the audit would
                // still suggest exactly that for this layer.
                let Some((paint, fresh)) = contrast::fresh_fix(&self.doc, id).filter(|(_, fresh)| fresh == color) else { return false };
                let Some(n) = tx.ed.node(&mut self.doc, id) else { return false };
                let NodeKind::Text(t) = &mut n.kind else { return false };
                set_text_fills_everywhere(t, paint, PaintKind::Solid(fresh))
            }
            Fix::Reconnect { component } => self.reconnect_detached(tx, id, *component),
        }
    }

    /// Replaces the detached frame `id` with an instance of `component` in the same slot, keeping
    /// its name, size, transform, constraints and auto-layout child settings. Refuses unless the
    /// frame still equals the component apart from name and placement, so no edit is lost.
    fn reconnect_detached(&mut self, tx: &mut Tx, id: NodeId, component: NodeId) -> bool {
        let Some(old) = self.doc.get(id).cloned() else { return false };
        let still_detached = detached::symbol_of(&old).is_some_and(|s| detached::is_main_component(&self.doc, s, component));
        let Some(parent) = old.parent.filter(|_| still_detached && detached::matches_component(&self.doc, id, component)) else { return false };
        let Some(index) = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)) else { return false };
        let cmd = designos_doc::create_instance(&mut self.doc, component, parent, Vec2::new(old.transform.tx, old.transform.ty));
        let Command::Insert { mut nodes, .. } = cmd else { return false };
        let Some(root) = nodes.first_mut() else { return false };
        root.name = old.name.clone();
        // The name is the layer's own. Without the override, a sync from the component (it runs
        // when the same step also edits the component) copies the component's name over it.
        root.component.overrides.insert(OverrideField::Name);
        root.size = old.size;
        root.transform = old.transform;
        root.constraints = old.constraints;
        root.layout_child = old.layout_child;
        let new_id = root.id;
        // Both steps or neither: a half-applied swap would leave a duplicate in a "Fix all".
        if let Err(e) = tx.cmds_all_or_none(&mut self.doc, vec![Command::Insert { parent, index, nodes }, Command::Remove { id }]) {
            self.show_toast(format!("Edit failed: {e}"), true);
            return false;
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, new_id);
        true
    }
}
