//! Applies Check designs fixes through the editor's transaction, so one fix (or "Fix all") is one
//! undo step. Every fix re-validates the layer as it is NOW and refuses when it drifted since the
//! audit, so a stale finding can never overwrite an edit made in between.

use super::{contrast, detached, hardcoded, Finding, Fix};
use crate::app::App;
use crate::edit::Tx;
use crate::layout;
use crate::libraries::is_noop;
use designos_doc::{import_asset, local_key, set_bound_property, set_text_fills_everywhere, Command, NodeId, NodeKind, OverrideField, PaintKind, Vec2};

/// Order fixes run in within "Fix all": recolouring first, then links to variables and styles, and
/// the swap of a detached frame for an instance last. The swap only proceeds while the frame still
/// equals its main component, so any fix made to the frame or to a layer inside it first makes the
/// swap refuse, instead of the swap deleting the layer a fix was just applied to (or a later fix
/// finding it gone). A bind that a recolour made stale is likewise refused rather than undoing it.
fn priority(fix: &Fix) -> u8 {
    match fix {
        Fix::SetTextColor { .. } => 0,
        Fix::BindVariable { .. } | Fix::BindVariables { .. } | Fix::ApplyStyle { .. } => 1,
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
        let mut applied = 0;
        for f in todo {
            if self.apply_check_fix(&mut tx, f) {
                applied += 1;
            } else if tx.is_poisoned() {
                tx.cancel(&mut self.doc);
                return 0;
            }
        }
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
            // A library asset is imported and then bound. When the bind is refused, the import goes
            // with it, so "Fix all" never commits an imported asset that nothing uses.
            Fix::BindVariable { .. } | Fix::BindVariables { .. } | Fix::ApplyStyle { .. } => {
                let mark = tx.mark();
                if self.import_and_link(tx, id, fix) {
                    return true;
                }
                if tx.rollback_to(&mut self.doc, mark).is_err() {
                    self.show_toast("Couldn't undo a partial import".to_string(), true);
                }
                false
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

    /// Imports the library asset a variable or style fix needs (when it is not in the document yet),
    /// then links the layer. False when either step is refused; the caller rolls back the import.
    fn import_and_link(&mut self, tx: &mut Tx, id: NodeId, fix: &Fix) -> bool {
        match fix {
            Fix::BindVariable { key, variable } => {
                if !self.doc.variables.contains_key(variable) && !self.import_library_variable_for_fix(tx, id, std::slice::from_ref(key), variable) {
                    return false;
                }
                let keys = hardcoded::keys_to_bind(&self.doc, id, key, variable);
                self.bind_keys(tx, id, keys.into_iter().map(str::to_string).collect(), variable)
            }
            Fix::BindVariables { keys, variable } => {
                if !self.doc.variables.contains_key(variable) && !self.import_library_variable_for_fix(tx, id, keys, variable) {
                    return false;
                }
                if !keys.iter().all(|k| hardcoded::bind_is_current(&self.doc, id, k, variable)) {
                    return false;
                }
                self.bind_keys(tx, id, keys.clone(), variable)
            }
            Fix::ApplyStyle { style } => {
                if !self.doc.styles.contains_key(style) && !self.import_library_style_for_fix(tx, id, style) {
                    return false;
                }
                if !hardcoded::style_is_current(&self.doc, id, style) {
                    return false;
                }
                self.apply_style_in(tx, &[id], style);
                true
            }
            _ => false,
        }
    }

    fn bind_keys(&mut self, tx: &mut Tx, id: NodeId, keys: Vec<String>, variable: &str) -> bool {
        if keys.is_empty() {
            return false;
        }
        let Some(value) = self.doc.resolve_variable(id, variable) else { return false };
        let Some(n) = tx.ed.node(&mut self.doc, id) else { return false };
        for k in &keys {
            n.variables.bindings.insert(k.clone(), variable.to_string());
            set_bound_property(n, k, &value);
        }
        true
    }

    fn import_pinned_asset_in(&mut self, tx: &mut Tx, library_id: &str, version: u32, key: &str) -> Option<String> {
        let ekey = local_key(library_id, key);
        if let Some(l) = self.doc.library.external.get(&ekey).and_then(|e| e.local_key.clone()) {
            return Some(l);
        }
        let snap = self.snapshot_of(library_id, version)?;
        let cmd = import_asset(&mut self.doc, &snap, key, None);
        if !is_noop(&cmd) && tx.cmd(&mut self.doc, cmd).is_err() {
            return None;
        }
        self.doc.library.external.get(&ekey).and_then(|e| e.local_key.clone())
    }

    fn import_library_variable_for_fix(&mut self, tx: &mut Tx, id: NodeId, keys: &[String], variable: &str) -> bool {
        let enabled = self.doc.library.enabled.clone();
        for r in enabled {
            let prefix = format!("{}/", r.id);
            let Some(raw) = variable.strip_prefix(&prefix) else { continue };
            let Some(snap) = self.snapshot_of(&r.id, r.version) else {
                self.show_toast(format!("Couldn't import \"{raw}\" from {}", r.name), true);
                return false;
            };
            if keys.is_empty() || !keys.iter().all(|k| hardcoded::library_bind_is_current(&self.doc, &snap, id, k, raw)) {
                return false;
            }
            let asset_name = snap.variables.get(raw).map(|v| v.name.clone()).unwrap_or_else(|| raw.to_string());
            let imported = self.import_pinned_asset_in(tx, &r.id, r.version, &format!("var:{raw}")).is_some();
            if imported {
                self.libs.invalidate();
            } else {
                self.show_toast(format!("Couldn't import \"{asset_name}\" from {}", r.name), true);
            }
            return imported;
        }
        false
    }

    fn import_library_style_for_fix(&mut self, tx: &mut Tx, id: NodeId, style: &str) -> bool {
        let enabled = self.doc.library.enabled.clone();
        for r in enabled {
            let prefix = format!("{}/", r.id);
            let Some(raw) = style.strip_prefix(&prefix) else { continue };
            let Some(snap) = self.snapshot_of(&r.id, r.version) else {
                self.show_toast(format!("Couldn't import \"{raw}\" from {}", r.name), true);
                return false;
            };
            if !hardcoded::library_style_is_current(&self.doc, &snap, id, raw) {
                return false;
            }
            let asset_name = snap.style_meta.get(raw).map(|m| m.name.clone()).unwrap_or_else(|| raw.to_string());
            let imported = self.import_pinned_asset_in(tx, &r.id, r.version, &format!("style:{raw}")).is_some();
            if imported {
                self.libs.invalidate();
            } else {
                self.show_toast(format!("Couldn't import \"{asset_name}\" from {}", r.name), true);
            }
            return imported;
        }
        false
    }

    /// Replaces the detached frame `id` with an instance of `component` in the same slot, keeping
    /// its name, size, transform, constraints and auto-layout child settings. Refuses unless the
    /// frame still equals the component apart from name and placement, so no edit is lost.
    fn reconnect_detached(&mut self, tx: &mut Tx, id: NodeId, component: NodeId) -> bool {
        let Some(old) = self.doc.get(id).cloned() else { return false };
        let still_detached = detached::symbol_of(&old).is_some_and(|s| detached::is_main_component(&self.doc, s, component));
        // A mirror inside an instance would be removed again by the next sync from its main.
        let Some(parent) = old.parent.filter(|_| still_detached && old.component.source.is_none() && detached::matches_component(&self.doc, id, component))
        else {
            return false;
        };
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
