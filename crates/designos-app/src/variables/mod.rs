//! Variables: the local-variables view (full view or floating panel), the inspector
//! binding picker and the App operations behind them. Every change goes through `Command::SetVariables`
//! (or ordinary node updates for bindings) so it is one undo step.

pub mod library;
pub mod popover;
pub mod view;
pub mod widgets;

use crate::app::App;
use crate::edit::Tx;
use crate::layout;
use designos_doc::*;
use egui::Rect;
use std::collections::HashSet;
use std::path::Path;

/// Which popup menu of the variables view is open.
#[derive(Clone, Debug, PartialEq)]
pub enum VarMenuKind {
    /// "+ Create variable" type dropdown.
    Create,
    Row(String),
    Mode(String, String),
    Collection(String),
    Folder(String, String),
    /// Collection settings (sliders button): import / export.
    Settings,
    /// Filter button: variable types.
    Filter,
}

#[derive(Clone, Debug)]
pub struct VarMenu {
    pub kind: VarMenuKind,
    pub pos: egui::Pos2,
    pub anchor: Rect,
}

/// Colour picker editing one value cell; edits are live and committed on close.
pub struct CellPicker {
    pub var: String,
    pub mode: String,
    pub color: egui::Color32,
    pub anchor: Rect,
    pub before: VariableStore,
}

/// Alias picker for one value cell.
pub struct AliasPicker {
    pub var: String,
    pub mode: String,
    pub anchor: Rect,
    pub query: String,
}

/// Binding picker opened from the inspector.
pub struct BindPicker {
    pub keys: Vec<String>,
    pub kind: VariableKind,
    pub nodes: Vec<NodeId>,
    pub anchor: Rect,
    pub query: String,
}

pub struct RowDrag {
    pub var: String,
}

/// "Edit variable" popover.
#[derive(Clone, Debug)]
pub struct EditPopover {
    pub var: String,
    pub anchor: Rect,
    pub scroll: f32,
}

pub enum VarsIoResult {
    Imported {
        collections: std::collections::BTreeMap<String, VariableCollection>,
        variables: std::collections::BTreeMap<String, Variable>,
        report: designos_fig::ImportReport,
    },
    Exported {
        var_count: usize,
        format_label: &'static str,
    },
    Error(String),
}

pub struct VarsState {
    pub open: bool,
    /// Full view (true) or floating panel (false); remembered between openings.
    pub full: bool,
    /// Top-left of the floating panel; `None` = default position.
    pub float_pos: Option<egui::Pos2>,
    /// Sidebar hidden by its toggle.
    pub sidebar_hidden: bool,
    /// Selected collection id.
    pub collection: Option<String>,
    /// Selected group path (`None` = "All").
    pub group: Option<String>,
    /// Selected variable rows (ids) and the range anchor.
    pub selected: Vec<String>,
    pub anchor: Option<String>,
    pub scroll: f32,
    pub scroll_x: f32,
    pub search: String,
    /// Variable types hidden by the filter button.
    pub hidden_kinds: Vec<VariableKind>,
    pub folded: HashSet<String>,
    pub menu: Option<VarMenu>,
    pub cell_picker: Option<CellPicker>,
    pub alias_picker: Option<AliasPicker>,
    pub bind_picker: Option<BindPicker>,
    pub edit: Option<EditPopover>,
    pub drag: Option<RowDrag>,
    /// Variable whose name should start editing on the next frame.
    pub focus_name: Option<String>,
    /// Thing being renamed inline: ('v', var) / ('c', coll) / ('f', "coll\0path") / ('t', coll) / ('m', mode).
    pub renaming: Option<(char, String)>,
    pub rename_frames: u32,
    /// Last import report (for the skipped-tokens details popover, `E1-9` §7).
    pub last_import_report: Option<designos_fig::ImportReport>,
    /// Whether the skipped-tokens details modal/popover is visible (`E1-9` §7).
    pub show_skipped_details: bool,
    /// Background variable import/export worker result receiver.
    pub pending_io: Option<std::sync::mpsc::Receiver<VarsIoResult>>,
}

impl Default for VarsState {
    fn default() -> Self {
        Self {
            open: false,
            full: false,
            float_pos: None,
            sidebar_hidden: false,
            collection: None,
            group: None,
            selected: vec![],
            anchor: None,
            scroll: 0.0,
            scroll_x: 0.0,
            search: String::new(),
            hidden_kinds: vec![],
            folded: HashSet::new(),
            menu: None,
            cell_picker: None,
            alias_picker: None,
            bind_picker: None,
            edit: None,
            drag: None,
            focus_name: None,
            renaming: None,
            rename_frames: 0,
            last_import_report: None,
            show_skipped_details: false,
            pending_io: None,
        }
    }
}

impl App {
    /// The document with variable bindings applied, for exporters that read raw
    /// node properties (borrowed when nothing is bound).
    ///
    /// Auto-layout subtrees still queued for a lazy re-layout are finished on a copy, so an export
    /// never shows them stale (the live document keeps its own queue for the window to drain).
    pub fn export_doc(&self) -> std::borrow::Cow<'_, Document> {
        let bound = self.doc.nodes.values().any(|n| !n.variables.bindings.is_empty());
        if !self.doc.pending_relayout().is_empty() {
            let mut settled = self.doc.clone();
            settled.flush_pending_relayout();
            return std::borrow::Cow::Owned(if bound { apply_bound_variables(&settled) } else { settled });
        }
        if bound {
            std::borrow::Cow::Owned(apply_bound_variables(&self.doc))
        } else {
            std::borrow::Cow::Borrowed(&self.doc)
        }
    }

    pub fn open_variables(&mut self) {
        self.vars.open = true;
        let known = self.vars.collection.as_ref().is_some_and(|c| self.doc.variable_collections.contains_key(c));
        if !known {
            self.vars.collection = library::first_collection_of(&self.doc.variable_collections);
        }
        self.ui.menu = None;
    }

    /// Rail "Variables" item: opens the view in its last form, or closes it.
    pub fn toggle_variables(&mut self) {
        if self.vars.open {
            self.close_variables();
        } else {
            self.open_variables();
        }
    }

    /// Runs `f` on a working copy of the variable store and installs the result as one undo step.
    /// `f` returns the ids of variables and collections it removed.
    pub fn edit_variables(&mut self, f: impl FnOnce(&mut VariableStore) -> (Vec<String>, Vec<String>)) -> bool {
        let mut store = VariableStore::from_doc(&self.doc);
        let (rv, rc) = f(&mut store);
        self.commit_store(store, &rv, &rc)
    }

    pub fn close_variables(&mut self) {
        self.vars.open = false;
        self.vars.menu = None;
        self.vars.alias_picker = None;
        self.vars.renaming = None;
        self.vars.drag = None;
        self.vars.edit = None;
        if let Some(p) = self.vars.cell_picker.take() {
            self.finish_cell_picker(p);
        }
    }

    /// Installs an edited variable store as one undo step. `removed_*` list
    /// variables / collections that disappeared so their bindings are dropped
    /// from nodes in the same step.
    pub fn commit_store(&mut self, mut store: VariableStore, removed_vars: &[String], removed_colls: &[String]) -> bool {
        // A collection subscribed from a library is edited in its library file, whichever path got here.
        store.enforce_library_read_only(&VariableStore::from_doc(&self.doc));
        if store.matches_doc(&self.doc) {
            return false;
        }
        // a removal the guard refused leaves the variable in place, so its bindings stay too
        let removed_vars: Vec<String> = removed_vars.iter().filter(|v| !store.variables.contains_key(*v)).cloned().collect();
        let removed_colls: Vec<String> = removed_colls.iter().filter(|c| !store.collections.contains_key(*c)).cloned().collect();
        let (removed_vars, removed_colls) = (&removed_vars[..], &removed_colls[..]);
        let cleanup = dangling_binding_cleanup(&self.doc, removed_vars, removed_colls);
        let mut cleaned = Vec::new();
        for mut n in cleanup {
            let keys: Vec<String> = self
                .doc
                .get(n.id)
                .map(|o| o.variables.bindings.iter().filter(|(_, v)| removed_vars.contains(v)).map(|(k, _)| k.clone()).collect())
                .unwrap_or_default();
            bake_bindings(&self.doc, &mut n, &keys);
            cleaned.push(n);
        }
        let changed = changed_variables(&self.doc, &store, removed_vars);
        let affected = bound_nodes(&self.doc, &changed);
        let mut cmds = vec![];
        if affected.is_empty() {
            cmds.push(store.command());
            cmds.extend(cleaned.into_iter().map(|n| Command::Update { node: Box::new(n) }));
        } else {
            // Nodes bound to a changed variable (directly or through aliases) reflow in the
            // same undo step: auto layout and text auto-resize run on the new values.
            let mut trial = self.doc.clone();
            let _ = store.clone().command().apply(&mut trial);
            for n in cleaned {
                let _ = Command::Update { node: Box::new(n) }.apply(&mut trial);
            }
            reflow_bound(&mut trial, &changed, &affected);
            cmds.push(store.command());
            for (id, n) in &trial.nodes {
                if self.doc.nodes.get(id) != Some(n) {
                    cmds.push(Command::Update { node: Box::new(n.clone()) });
                }
            }
        }
        let mut cmd = if cmds.len() == 1 { cmds.pop().unwrap() } else { Command::Batch(cmds) };
        if !affected.is_empty() {
            // Edits to a main component's layers carry over to its instances.
            cmd = crate::comp_ops::with_sync(&mut self.doc, cmd);
        }
        match self.hist.exec(&mut self.doc, cmd) {
            Ok(()) => {
                self.changed();
                self.sanitize();
                true
            }
            Err(e) => {
                self.show_toast(format!("Variable edit failed: {e}"), true);
                false
            }
        }
    }

    /// Ends a colour-picker session: the live edits become one undo step.
    pub fn finish_cell_picker(&mut self, p: CellPicker) {
        let after = VariableStore::from_doc(&self.doc);
        self.doc.variables = p.before.variables.clone();
        self.doc.variable_collections = p.before.collections.clone();
        self.commit_store(after, &[], &[]);
        self.changed();
    }

    /// Binds `keys` of every node in `nodes` to variable `var` (one undo step).
    /// The raw property takes the variable's current value so detaching keeps the look.
    pub fn bind_variable(&mut self, nodes: &[NodeId], keys: &[&str], var: &str) {
        let mut tx = Tx::new();
        self.bind_variable_in(&mut tx, nodes, keys, var);
        self.run_tx(tx);
    }

    /// [`App::bind_variable`] inside a caller-owned transaction.
    pub fn bind_variable_in(&mut self, tx: &mut Tx, nodes: &[NodeId], keys: &[&str], var: &str) {
        let mut pre = Vec::new();
        for id in nodes {
            for k in keys {
                if let Some(v) = self.doc.resolve_variable(*id, var) {
                    pre.push((*id, k.to_string(), v));
                }
            }
        }
        for (id, key, val) in &pre {
            if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                n.variables.bindings.insert(key.clone(), var.to_string());
                set_bound_property(n, key, val);
            }
            layout::relayout_around(&mut self.doc, &mut tx.ed, *id);
        }
    }

    /// Removes the bindings of `keys`, keeping the values they currently resolve to.
    pub fn detach_variable(&mut self, nodes: &[NodeId], keys: &[&str]) {
        let mut pre = Vec::new();
        for id in nodes {
            for k in keys {
                if let Some(v) = self.doc.bound_value(*id, k) {
                    pre.push((*id, k.to_string(), v));
                }
            }
        }
        let mut tx = Tx::new();
        for id in nodes {
            if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                for k in keys {
                    n.variables.bindings.remove(*k);
                }
            }
        }
        for (id, key, val) in &pre {
            if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                set_bound_property(n, key, val);
            }
        }
        for id in nodes {
            layout::relayout_around(&mut self.doc, &mut tx.ed, *id);
        }
        self.run_tx(tx);
    }

    /// Sets (or clears, with `None`) the explicit mode of `collection` on nodes.
    pub fn set_node_mode(&mut self, nodes: &[NodeId], collection: &str, mode: Option<&str>) {
        let c = collection.to_string();
        let m = mode.map(String::from);
        self.edit_nodes(nodes, |n| match &m {
            Some(m) => {
                n.variables.modes.insert(c.clone(), m.clone());
            }
            None => {
                n.variables.modes.remove(&c);
            }
        });
    }

    /// Merges a `variables/local`, `figma-native`, or `dtcg` JSON payload into the document (one undo step).
    /// Records that would change a library collection, or that would take the id of a library one, are left out
    /// and counted as skipped (see [`library::drop_refused_records`]).
    #[allow(dead_code)]
    pub fn import_variables_text(&mut self, text: &str) -> Result<library::VariablesImport, String> {
        let mut tmp = Document::new("import");
        let rep = designos_fig::import_variables(
            &mut tmp,
            text,
            designos_fig::VariablesFormat::Auto,
            designos_fig::ImportVariablesOptions { replace: true, ..Default::default() },
        )
        .map_err(|e| e.to_string())?;
        let total = tmp.variables.len();
        let mut store = VariableStore::from_doc(&self.doc);
        let skipped = library::drop_refused_records(&store, &mut tmp.variable_collections, &mut tmp.variables);
        store.collections.extend(tmp.variable_collections);
        store.variables.extend(tmp.variables);
        self.commit_store(store, &[], &[]);
        if self.vars.collection.is_none() {
            self.vars.collection = library::first_collection_of(&self.doc.variable_collections);
        }
        self.vars.last_import_report = Some(rep);
        Ok(library::VariablesImport { imported: total - skipped, skipped_library: skipped })
    }

    fn variables_only_doc(&self) -> Document {
        let mut working = Document::new("variables_work");
        working.variable_collections = self.doc.variable_collections.clone();
        working.variables = self.doc.variables.clone();
        working
    }

    fn apply_imported_variables(
        &mut self,
        mut collections: std::collections::BTreeMap<String, VariableCollection>,
        mut variables: std::collections::BTreeMap<String, Variable>,
        mut rep: designos_fig::ImportReport,
    ) {
        let mut store = VariableStore::from_doc(&self.doc);
        let skipped_lib = library::drop_refused_records(&store, &mut collections, &mut variables);
        for _ in 0..skipped_lib {
            rep.skipped.push(designos_fig::SkippedVariable { path: "library".into(), reason: "subscribed library collection is read-only".into() });
        }
        store.collections = collections;
        store.variables = variables;
        self.commit_store(store, &[], &[]);
        if self.vars.collection.is_none() {
            self.vars.collection = library::first_collection_of(&self.doc.variable_collections);
        }
        let toast = rep.toast_message();
        self.vars.show_skipped_details = !rep.skipped.is_empty();
        self.vars.last_import_report = Some(rep);
        self.show_toast(toast, false);
    }

    /// Polls any in-flight background variable import/export worker and commits results on the UI thread.
    pub fn poll_variables_io(&mut self) {
        let Some(rx) = &self.vars.pending_io else { return };
        match rx.try_recv() {
            Ok(res) => {
                self.vars.pending_io = None;
                match res {
                    VarsIoResult::Imported { collections, variables, report } => {
                        self.apply_imported_variables(collections, variables, report);
                    }
                    VarsIoResult::Exported { var_count, format_label } => {
                        self.show_toast(format!("Exported {var_count} variables ({format_label})"), false);
                    }
                    VarsIoResult::Error(msg) => {
                        self.show_toast(msg, true);
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.vars.pending_io = None;
            }
        }
    }

    /// Spawns a background worker to read and parse variable files off the UI thread.
    pub fn spawn_import_variables_files_with_format(&mut self, paths: Vec<std::path::PathBuf>, format: designos_fig::VariablesFormat) {
        if paths.is_empty() {
            return;
        }
        let mut working = self.variables_only_doc();
        let (tx, rx) = std::sync::mpsc::channel();
        self.vars.pending_io = Some(rx);
        std::thread::spawn(move || {
            let res = run_import_variables_worker(&mut working, &paths, format);
            let _ = tx.send(match res {
                Ok(report) => VarsIoResult::Imported { collections: working.variable_collections, variables: working.variables, report },
                Err(e) => VarsIoResult::Error(e),
            });
        });
    }

    /// Spawns a background worker to serialize and write exported variables off the UI thread.
    pub fn spawn_export_variables_with_format(&mut self, path: std::path::PathBuf, format: designos_fig::VariablesFormat) {
        let working = self.variables_only_doc();
        let var_count = working.variables.len();
        let (tx, rx) = std::sync::mpsc::channel();
        self.vars.pending_io = Some(rx);
        std::thread::spawn(move || {
            let res = run_export_variables_worker(&working, &path, format);
            let _ = tx.send(match res {
                Ok(format_label) => VarsIoResult::Exported { var_count, format_label },
                Err(e) => VarsIoResult::Error(e),
            });
        });
    }

    /// Merges one or more variable files (`figma-rest`, `figma-native`, `dtcg`, or `.zip`) into the document (`E1-9` §3.3, §7).
    pub fn import_variables_files_with_format(&mut self, paths: &[std::path::PathBuf], format: designos_fig::VariablesFormat) {
        if paths.is_empty() {
            return;
        }
        let mut working = self.variables_only_doc();
        match run_import_variables_worker(&mut working, paths, format) {
            Ok(rep) => self.apply_imported_variables(working.variable_collections, working.variables, rep),
            Err(e) => self.show_toast(e, true),
        }
    }

    #[allow(dead_code)]
    pub fn import_variables_file(&mut self, path: &Path) {
        self.import_variables_files_with_format(&[path.to_path_buf()], designos_fig::VariablesFormat::Auto);
    }

    pub fn export_variables_with_format(&mut self, path: &Path, format: designos_fig::VariablesFormat) {
        let working = self.variables_only_doc();
        match run_export_variables_worker(&working, path, format) {
            Ok(label) => self.show_toast(format!("Exported {} variables ({label})", self.doc.variables.len()), false),
            Err(e) => self.show_toast(e, true),
        }
    }

    #[allow(dead_code)]
    pub fn export_variables_file(&mut self, path: &Path) {
        self.export_variables_with_format(path, designos_fig::VariablesFormat::FigmaRest);
    }
}

fn run_import_variables_worker(
    working: &mut Document,
    paths: &[std::path::PathBuf],
    format: designos_fig::VariablesFormat,
) -> Result<designos_fig::ImportReport, String> {
    let Some((first_path, rest_paths)) = paths.split_first() else {
        return Err("No variable files selected".into());
    };
    let bytes = std::fs::read(first_path).map_err(|e| format!("Cannot read {}: {e}", first_path.display()))?;
    let mut opts = designos_fig::ImportVariablesOptions::default();
    for p in rest_paths {
        if let (Some(fname), Ok(fbytes)) = (p.file_name().and_then(|s| s.to_str()), std::fs::read(p)) {
            opts.additional_files.push((fname.to_string(), fbytes));
        }
    }
    if first_path.file_name().and_then(|s| s.to_str()) == Some("tokens.manifest.json") {
        if let Some(parent_dir) = first_path.parent() {
            if let Ok(entries) = std::fs::read_dir(parent_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p != *first_path && p.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let (Some(fname), Ok(fbytes)) = (p.file_name().and_then(|s| s.to_str()), std::fs::read(&p)) {
                            if !opts.additional_files.iter().any(|(n, _)| n == fname) {
                                opts.additional_files.push((fname.to_string(), fbytes));
                            }
                        }
                    }
                }
            }
        }
    }
    designos_fig::import_variables_bytes(working, &bytes, first_path.file_name().and_then(|s| s.to_str()), format, opts)
        .map_err(|e| format!("Cannot import variables: {e}"))
}

fn run_export_variables_worker(working: &Document, path: &Path, format: designos_fig::VariablesFormat) -> Result<&'static str, String> {
    let bundle = designos_fig::export_variables(working, format).map_err(|e| format!("Cannot export variables: {e}"))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let res = if ext.eq_ignore_ascii_case("zip") || (bundle.files.len() > 1 && !path.is_dir()) {
        let zip_path = if ext.eq_ignore_ascii_case("zip") { path.to_path_buf() } else { path.with_extension("zip") };
        bundle.to_zip_bytes().map_err(|e| e.to_string()).and_then(|bytes| std::fs::write(&zip_path, bytes).map_err(|e| e.to_string()))
    } else if let Some((_, first_bytes)) = bundle.files.first() {
        std::fs::write(path, first_bytes).map_err(|e| e.to_string())
    } else {
        Ok(())
    };
    res.map(|()| bundle.format.label()).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}

/// Variables whose resolved value may differ after installing `store`: those added or edited,
/// those of collections whose modes changed, and every variable aliasing one of them.
/// Removed variables are left out (their bindings are baked, nothing reflows).
pub fn changed_variables(doc: &Document, store: &VariableStore, removed: &[String]) -> HashSet<String> {
    let mut changed: HashSet<String> = store.variables.iter().filter(|(id, v)| doc.variables.get(*id) != Some(v)).map(|(id, _)| id.clone()).collect();
    for (cid, c) in &store.collections {
        if doc.variable_collections.get(cid).is_some_and(|old| old.modes != c.modes || old.default_mode != c.default_mode) {
            changed.extend(c.variables.iter().cloned());
        }
    }
    loop {
        let more: Vec<String> = store
            .variables
            .iter()
            .filter(|(id, v)| !changed.contains(*id) && v.values.values().any(|x| matches!(x, VariableValue::Alias(a) if changed.contains(a))))
            .map(|(id, _)| id.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        changed.extend(more);
    }
    changed.retain(|id| !removed.contains(id));
    changed
}

/// Nodes with a binding to one of `vars`.
pub fn bound_nodes(doc: &Document, vars: &HashSet<String>) -> Vec<NodeId> {
    if vars.is_empty() {
        return vec![];
    }
    doc.nodes.values().filter(|n| n.variables.bindings.values().any(|v| vars.contains(v))).map(|n| n.id).collect()
}

/// Writes the new values of `vars` into the bound properties of `nodes` (which are already
/// resolved against the new store in `doc`), re-fits auto-sized text and re-runs auto layout.
pub fn reflow_bound(doc: &mut Document, vars: &HashSet<String>, nodes: &[NodeId]) {
    for id in nodes {
        let binds: Vec<(String, String)> = match doc.get(*id) {
            Some(n) => n.variables.bindings.iter().filter(|(_, v)| vars.contains(*v)).map(|(k, v)| (k.clone(), v.clone())).collect(),
            None => continue,
        };
        for (key, var) in binds {
            if let Some(val) = doc.resolve_variable(*id, &var) {
                if let Some(n) = doc.get_mut(*id) {
                    set_bound_property(n, &key, &val);
                }
            }
        }
        let fitted = doc.get(*id).and_then(|n| match &resolve_node(doc, n).kind {
            NodeKind::Text(t) => Some(layout::fit_text(t, n.size)),
            _ => None,
        });
        if let (Some(size), Some(n)) = (fitted, doc.get_mut(*id)) {
            n.size = size;
        }
    }
    let mut ed = crate::edit::Edit::new();
    for id in nodes {
        layout::relayout_around(doc, &mut ed, *id);
    }
}

/// True when `text` is a variables-only payload (no design document in it).
pub fn is_variables_payload(text: &str) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return false };
    if v.get("document").is_some() || v.get("nodes").is_some() || v.get("pages").is_some() {
        return false;
    }
    designos_fig::detect_variables_format_value(&v).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_vars() -> App {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let rid = d.alloc_id();
        d.insert(page, Node::rectangle(rid, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let mut app = App::new(d);
        let mut s = VariableStore::default();
        let c = s.add_collection("Theme");
        let a = s.add_variable(&c, "Brand", VariableKind::Color).unwrap();
        let mode = s.collections[&c].default_mode.clone();
        s.set_value(&a, &mode, VariableValue::Color(Color::rgb(255, 0, 0)));
        let g = s.add_variable(&c, "Gap", VariableKind::Number).unwrap();
        s.set_value(&g, &mode, VariableValue::Number(12.0));
        assert!(app.commit_store(s, &[], &[]));
        app
    }

    #[test]
    fn bind_detach_and_delete_are_single_undo_steps() {
        let mut app = app_with_vars();
        let rect = *app.doc.nodes.keys().last().unwrap();
        let brand = app.doc.variables.values().find(|v| v.name == "Brand").unwrap().id.clone();
        app.bind_variable(&[rect], &["fills/0"], &brand);
        let n = app.doc.get(rect).unwrap();
        assert_eq!(n.variables.bindings["fills/0"], brand);
        assert_eq!(n.fills[0].kind, PaintKind::Solid(Color::rgb(255, 0, 0)));
        app.undo();
        assert!(app.doc.get(rect).unwrap().variables.is_empty());
        app.redo();
        app.detach_variable(&[rect], &["fills/0"]);
        assert!(app.doc.get(rect).unwrap().variables.bindings.is_empty());
        assert_eq!(app.doc.get(rect).unwrap().fills[0].kind, PaintKind::Solid(Color::rgb(255, 0, 0)));

        // deleting a bound variable in the store drops the binding in the same step
        app.bind_variable(&[rect], &["fills/0"], &brand);
        let mut store = VariableStore::from_doc(&app.doc);
        store.delete_variable(&brand);
        assert!(app.commit_store(store, std::slice::from_ref(&brand), &[]));
        assert!(app.doc.get(rect).unwrap().variables.bindings.is_empty());
        app.undo();
        assert_eq!(app.doc.get(rect).unwrap().variables.bindings["fills/0"], brand);
        assert!(app.doc.variables.contains_key(&brand));
    }

    /// A horizontal hug-all frame with two 20x20 rects and a text whose `itemSpacing`
    /// binds to `gap_var`; returns (frame, first, second, text).
    fn spaced_row(app: &mut App, gap_var: &str) -> (NodeId, NodeId, NodeId, NodeId) {
        let page = app.page;
        let fid = app.doc.alloc_id();
        let mut f = Node::frame(fid, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
        f.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                item_spacing: 12.0,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Hug,
                ..Default::default()
            }),
            ..Default::default()
        });
        let frame = app.doc.insert(page, f).unwrap();
        let mut kids = vec![];
        for _ in 0..2 {
            let id = app.doc.alloc_id();
            kids.push(app.doc.insert(frame, Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap());
        }
        let tid = app.doc.alloc_id();
        let style = TextStyle { font_size: 12.0, ..Default::default() };
        let text = app.doc.insert(frame, Node::text(tid, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0), "Size me", style)).unwrap();
        app.bind_variable(&[frame], &["itemSpacing"], gap_var);
        (frame, kids[0], kids[1], text)
    }

    fn set_number(app: &mut App, name: &str, v: f32) {
        let mut store = VariableStore::from_doc(&app.doc);
        let id = store.variables.values().find(|x| x.name == name).unwrap().id.clone();
        let mode = store.collections.values().next().unwrap().default_mode.clone();
        store.set_value(&id, &mode, VariableValue::Number(v));
        assert!(app.commit_store(store, &[], &[]));
    }

    #[test]
    fn changing_a_variable_reflows_auto_layout_in_the_same_undo_step() {
        let mut app = app_with_vars();
        let gap = app.doc.variables.values().find(|v| v.name == "Gap").unwrap().id.clone();
        let (frame, _a, b, _t) = spaced_row(&mut app, &gap);
        assert_eq!(app.doc.get(b).unwrap().transform.tx, 32.0);
        let w0 = app.doc.get(frame).unwrap().size.x;
        set_number(&mut app, "Gap", 30.0);
        assert_eq!(app.doc.get(b).unwrap().transform.tx, 50.0, "the second child moved by the gap change");
        assert_eq!(app.doc.get(frame).unwrap().size.x, w0 + 18.0 * 2.0, "a hugging frame grows with two more gaps");
        // layout and the variable edit undo together
        app.undo();
        assert_eq!(app.doc.get(b).unwrap().transform.tx, 32.0);
        assert_eq!(app.doc.get(frame).unwrap().size.x, w0);
        assert_eq!(app.doc.variables[&gap].values.values().next(), Some(&VariableValue::Number(12.0)));
        app.redo();
        assert_eq!(app.doc.get(b).unwrap().transform.tx, 50.0);
    }

    #[test]
    fn bound_fill_opacity_follows_each_edit_without_compounding() {
        let mut app = app_with_vars();
        let rect = *app.doc.nodes.keys().last().unwrap();
        let mut store = VariableStore::from_doc(&app.doc);
        let coll = store.collections.keys().next().unwrap().clone();
        let mode = store.collections[&coll].default_mode.clone();
        let fade = store.add_variable(&coll, "Fade", VariableKind::Number).unwrap();
        store.set_value(&fade, &mode, VariableValue::Number(60.0));
        assert!(app.commit_store(store, &[], &[]));
        assert_eq!(app.doc.get(rect).unwrap().fills[0].opacity, 1.0);
        let rendered = |app: &App| designos_doc::resolved_node(&app.doc, rect).unwrap().fills[0].opacity;

        app.bind_variable(&[rect], &["fills/0/opacity"], &fade);
        assert!((app.doc.get(rect).unwrap().fills[0].opacity - 0.6).abs() < 1e-6, "stored after bind");
        assert!((rendered(&app) - 0.6).abs() < 1e-6, "rendered after bind");

        set_number(&mut app, "Fade", 70.0);
        assert!((app.doc.get(rect).unwrap().fills[0].opacity - 0.7).abs() < 1e-6, "stored after edit 1");
        assert!((rendered(&app) - 0.7).abs() < 1e-6, "rendered after edit 1");

        set_number(&mut app, "Fade", 40.0);
        assert!((app.doc.get(rect).unwrap().fills[0].opacity - 0.4).abs() < 1e-6, "stored after edit 2");
        assert!((rendered(&app) - 0.4).abs() < 1e-6, "rendered after edit 2");
    }

    #[test]
    fn aliased_variables_reflow_their_bound_nodes() {
        let mut app = app_with_vars();
        let gap = app.doc.variables.values().find(|v| v.name == "Gap").unwrap().id.clone();
        let mut store = VariableStore::from_doc(&app.doc);
        let coll = store.collections.keys().next().unwrap().clone();
        let mode = store.collections[&coll].default_mode.clone();
        let alias = store.add_variable(&coll, "Gap/Alias", VariableKind::Number).unwrap();
        assert!(store.set_alias(&alias, &mode, &gap));
        assert!(app.commit_store(store, &[], &[]));
        let (frame, _a, b, _t) = spaced_row(&mut app, &alias);
        assert_eq!(app.doc.get(b).unwrap().transform.tx, 32.0);
        set_number(&mut app, "Gap", 4.0);
        assert_eq!(app.doc.get(b).unwrap().transform.tx, 24.0, "bound through an alias, still follows");
        assert!(app.doc.get(frame).unwrap().size.x < 100.0);
        app.undo();
        assert_eq!(app.doc.get(b).unwrap().transform.tx, 32.0);
    }

    #[test]
    fn variable_driven_text_size_refits_auto_resize_text_and_the_frame_around_it() {
        let mut app = app_with_vars();
        let gap = app.doc.variables.values().find(|v| v.name == "Gap").unwrap().id.clone();
        let (frame, _a, _b, text) = spaced_row(&mut app, &gap);
        app.bind_variable(&[text], &["fontSize"], &gap);
        let (tw, fw) = (app.doc.get(text).unwrap().size, app.doc.get(frame).unwrap().size.x);
        set_number(&mut app, "Gap", 48.0);
        let t = app.doc.get(text).unwrap();
        assert!(t.size.x > tw.x && t.size.y > tw.y, "text grew: {:?} -> {:?}", tw, t.size);
        assert!(app.doc.get(frame).unwrap().size.x > fw, "and the hugging frame followed");
        app.undo();
        assert_eq!(app.doc.get(text).unwrap().size, tw);
    }

    #[test]
    fn node_modes_and_import_merge() {
        let mut app = app_with_vars();
        let rect = *app.doc.nodes.keys().last().unwrap();
        let coll = app.doc.variable_collections.keys().next().unwrap().clone();
        app.set_node_mode(&[rect], &coll, Some("m"));
        assert_eq!(app.doc.get(rect).unwrap().variables.modes[&coll], "m");
        app.set_node_mode(&[rect], &coll, None);
        assert!(app.doc.get(rect).unwrap().variables.is_empty());

        let before = app.doc.variables.len();
        let json = designos_fig::export_variables_json(&app.doc).to_string();
        assert!(is_variables_payload(&json));
        assert!(!is_variables_payload("{\"document\": {}}"));
        assert!(!is_variables_payload("nope"));
        let mut other = App::new(Document::new("o"));
        assert_eq!(other.import_variables_text(&json).unwrap(), library::VariablesImport { imported: before, skipped_library: 0 });
        assert_eq!(other.doc.variables, app.doc.variables);
        other.undo();
        assert!(other.doc.variables.is_empty());
        assert!(other.import_variables_text("{}").is_err());
    }
}
