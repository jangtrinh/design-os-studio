//! Agent side: turn a command about to be executed into the [`LiveOp`]s it performs.

use super::{LiveOp, SiblingHint, ThreadChange, STRUCTURAL_FIELDS};
use crate::{Command, CommentThread, Document, LibraryState, Node, NodeId, Style, StyleMeta, Variable, VariableCollection};
use serde_json::{Map, Value};
use std::cell::Cell;
use std::collections::BTreeMap;

thread_local! {
    static BATCH_DOC_CLONES: Cell<usize> = const { Cell::new(0) };
}

/// Number of scratch [`Document`] clones taken by the [`Command::Batch`] arm of
/// [`live_ops_for_command`] on the current thread (used by performance / zero-clone tests).
pub fn batch_doc_clones() -> usize {
    BATCH_DOC_CLONES.with(Cell::get)
}

/// Whether `cmds` can be recorded against `doc` via [`live_ops_for_independent`] without cloning
/// `doc` (each `Update` targets a distinct node already present in `doc`, and at most one
/// structural child-list edit occurs).
pub fn is_independent_batch(doc: &Document, cmds: &[Command]) -> bool {
    let mut updated_ids = std::collections::HashSet::with_capacity(cmds.len());
    let mut structural_edits = 0usize;
    for cmd in cmds {
        match cmd {
            Command::Update { node } => {
                if doc.get(node.id).is_none() || !updated_ids.insert(node.id) {
                    return false;
                }
            }
            Command::Insert { .. } | Command::Remove { .. } | Command::Reparent { .. } => {
                structural_edits += 1;
                if structural_edits > 1 {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// Records [`LiveOp`]s for a slice of commands that are independent of one another (such as the
/// `parts` produced by [`crate::writer_command`]), inspecting each part against `doc` without
/// cloning the scene graph.
pub fn live_ops_for_independent(doc: &Document, parts: &[Command]) -> Result<Vec<LiveOp>, String> {
    let mut out = Vec::new();
    for part in parts {
        out.extend(live_ops_for_command(doc, part)?);
    }
    Ok(out)
}

/// The [`LiveOp`]s `cmd` performs when applied to `doc`, read from the state before it runs.
///
/// Errors for commands whose effect cannot be replayed on a document the agent only partly
/// holds (page, flow, guide and device edits); the caller should
/// refuse to run them while a live editor is attached instead of letting them vanish.
pub fn live_ops_for_command(doc: &Document, cmd: &Command) -> Result<Vec<LiveOp>, String> {
    let mut out = Vec::new();
    match cmd {
        Command::Insert { parent, index, nodes } => {
            let Some(p) = doc.get(*parent) else { return Ok(out) };
            let mut nodes = nodes.clone();
            if let Some(root) = nodes.first_mut() {
                root.parent = Some(*parent);
                out.push(LiveOp::Insert { parent: *parent, at: SiblingHint::at(&p.children, *index), nodes });
            }
        }
        Command::Remove { id } => out.push(LiveOp::Remove { id: *id }),
        Command::Reparent { id, parent, index } => {
            let Some(p) = doc.get(*parent) else { return Ok(out) };
            let siblings: Vec<_> = p.children.iter().copied().filter(|c| c != id).collect();
            out.push(LiveOp::Reparent { id: *id, parent: *parent, at: SiblingHint::at(&siblings, *index) });
        }
        Command::Update { node } => {
            if let Some(before) = doc.get(node.id) {
                let (set, unset) = field_changes(before, node)?;
                if !set.is_empty() || !unset.is_empty() {
                    out.push(LiveOp::Update { id: node.id, set, unset });
                }
            }
        }
        Command::SetComments { comments } => {
            if let Some(op) = comment_changes(&doc.comments, comments) {
                out.push(op);
            }
        }
        Command::SetVariables { variables, collections } => {
            if let Some(op) = variable_changes(doc, variables, collections) {
                out.push(op);
            }
        }
        Command::SetStyles { styles, style_meta } => {
            if let Some(op) = style_changes(doc, styles, style_meta) {
                out.push(op);
            }
        }
        Command::SetLibraryState { state, library_page } => {
            if let Some(op) = library_state_changes(&doc.library, doc.library_page, state, *library_page) {
                out.push(op);
            }
        }
        Command::SetImages { add, remove } => {
            if !add.is_empty() || !remove.is_empty() {
                out.push(LiveOp::Images { add: add.clone(), remove: remove.clone() });
            }
        }
        Command::Batch(cmds) => {
            // Later commands see the state earlier ones leave behind.
            BATCH_DOC_CLONES.with(|c| c.set(c.get() + 1));
            let mut scratch = doc.clone();
            for c in cmds {
                out.extend(live_ops_for_command(&scratch, c)?);
                c.clone().apply(&mut scratch).map_err(|e| e.to_string())?;
            }
        }
        other => return Err(format!("{} cannot be mirrored to the open editor", command_name(other))),
    }
    Ok(out)
}

/// Serialized top-level fields that differ between `before` and `after`.
fn field_changes(before: &Node, after: &Node) -> Result<(Map<String, Value>, Vec<String>), String> {
    let as_map = |n: &Node| match serde_json::to_value(n) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err("node did not serialize to an object".to_string()),
        Err(e) => Err(format!("cannot serialize node {}: {e}", n.id.0)),
    };
    let (old, new) = (as_map(before)?, as_map(after)?);
    let mut set = Map::new();
    for (k, v) in &new {
        if !STRUCTURAL_FIELDS.contains(&k.as_str()) && old.get(k) != Some(v) {
            if k == "layout_child" {
                if let (Some(Value::Object(old_lc)), Value::Object(new_lc)) = (old.get("layout_child"), v) {
                    let lc_diff = layout_child_changes(old_lc, new_lc, &after.layout_child)?;
                    if !lc_diff.is_empty() {
                        set.insert(k.clone(), Value::Object(lc_diff));
                    }
                    continue;
                }
            }
            if k == "component" {
                if let Value::Object(new_comp) = v {
                    let empty = Map::new();
                    let old_comp = old.get("component").and_then(Value::as_object).unwrap_or(&empty);
                    let comp_diff = component_changes(old_comp, new_comp, &after.component)?;
                    if !comp_diff.is_empty() {
                        set.insert(k.clone(), Value::Object(comp_diff));
                    }
                    continue;
                }
            }
            set.insert(k.clone(), v.clone());
        }
    }
    let unset = old.keys().filter(|k| !STRUCTURAL_FIELDS.contains(&k.as_str()) && !new.contains_key(*k)).cloned().collect();
    Ok((set, unset))
}

fn layout_child_changes(old_lc: &Map<String, Value>, new_lc: &Map<String, Value>, after: &crate::LayoutChild) -> Result<Map<String, Value>, String> {
    let mut diff = Map::new();
    for (k, v) in new_lc {
        if old_lc.get(k) != Some(v) {
            diff.insert(k.clone(), v.clone());
        }
    }
    for k in old_lc.keys() {
        if !new_lc.contains_key(k) {
            let reset_val = match k.as_str() {
                "align_self" => serde_json::to_value(after.align_self).map_err(|e| e.to_string())?,
                "grid_column" => serde_json::to_value(after.grid_column).map_err(|e| e.to_string())?,
                "grid_row" => serde_json::to_value(after.grid_row).map_err(|e| e.to_string())?,
                "col_span" => serde_json::to_value(after.col_span).map_err(|e| e.to_string())?,
                "row_span" => serde_json::to_value(after.row_span).map_err(|e| e.to_string())?,
                "grid_align_h" => serde_json::to_value(after.grid_align_h).map_err(|e| e.to_string())?,
                "grid_align_v" => serde_json::to_value(after.grid_align_v).map_err(|e| e.to_string())?,
                "margin" => serde_json::to_value(after.margin).map_err(|e| e.to_string())?,
                "stretched" => serde_json::to_value(after.stretched).map_err(|e| e.to_string())?,
                _ => Value::Null,
            };
            diff.insert(k.clone(), reset_val);
        }
    }
    Ok(diff)
}

/// What `SetVariables` changes in `doc`'s variables and collections, by id.
fn variable_changes(doc: &Document, variables: &BTreeMap<String, Variable>, collections: &BTreeMap<String, VariableCollection>) -> Option<LiveOp> {
    fn diff<T: Clone + PartialEq>(before: &BTreeMap<String, T>, after: &BTreeMap<String, T>) -> (Vec<T>, Vec<String>) {
        let changed = after.iter().filter(|(id, v)| before.get(*id) != Some(*v)).map(|(_, v)| v.clone()).collect();
        let removed = before.keys().filter(|id| !after.contains_key(*id)).cloned().collect();
        (changed, removed)
    }
    let (variables, removed_variables) = diff(&doc.variables, variables);
    let (collections, removed_collections) = diff(&doc.variable_collections, collections);
    let any = !variables.is_empty() || !collections.is_empty() || !removed_variables.is_empty() || !removed_collections.is_empty();
    any.then_some(LiveOp::Variables { collections, variables, removed_collections, removed_variables })
}

/// What `SetStyles` changes in `doc`'s shared styles and metadata, by style key.
fn style_changes(doc: &Document, styles: &BTreeMap<String, Style>, style_meta: &BTreeMap<String, StyleMeta>) -> Option<LiveOp> {
    fn diff<T: Clone + PartialEq>(before: &BTreeMap<String, T>, after: &BTreeMap<String, T>) -> (BTreeMap<String, T>, Vec<String>) {
        let set = after.iter().filter(|(k, v)| before.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect::<BTreeMap<_, _>>();
        let removed = before.keys().filter(|k| !after.contains_key(*k)).cloned().collect();
        (set, removed)
    }
    let (styles_set, styles_removed) = diff(&doc.styles, styles);
    let (meta_set, meta_removed) = diff(&doc.style_meta, style_meta);
    let any = !styles_set.is_empty() || !styles_removed.is_empty() || !meta_set.is_empty() || !meta_removed.is_empty();
    any.then_some(LiveOp::Styles { styles_set, styles_removed, meta_set, meta_removed })
}

fn component_changes(old_comp: &Map<String, Value>, new_comp: &Map<String, Value>, after: &crate::ComponentLink) -> Result<Map<String, Value>, String> {
    let mut diff = Map::new();
    let old_ovs = old_comp.get("overrides").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let new_ovs = new_comp.get("overrides").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let added: Vec<Value> = new_ovs.iter().filter(|ov| !old_ovs.contains(ov)).cloned().collect();
    let removed: Vec<Value> = old_ovs.iter().filter(|ov| !new_ovs.contains(ov)).cloned().collect();
    if !added.is_empty() {
        diff.insert("overrides_add".to_string(), Value::Array(added));
    }
    if !removed.is_empty() {
        diff.insert("overrides_remove".to_string(), Value::Array(removed));
    }
    for (k, v) in new_comp {
        if k == "overrides" {
            continue;
        }
        if old_comp.get(k) != Some(v) {
            diff.insert(k.clone(), v.clone());
        }
    }
    for k in old_comp.keys() {
        if k == "overrides" {
            continue;
        }
        if !new_comp.contains_key(k) {
            let reset_val = match k.as_str() {
                "source" | "slot_content_id" | "override_level" | "detached_symbol_id" => Value::Null,
                "props" => Value::Array(Vec::new()),
                "values" | "slot_configs" | "slot_content_ids" | "bindings" | "variant_props" => Value::Object(Map::new()),
                "is_slot" | "is_slot_content" | "expose_nested_instances" | "layout_cleared" => Value::Bool(false),
                "description" => Value::String(String::new()),
                "grid_align_overrides" => serde_json::to_value(after.grid_align_overrides).map_err(|e| e.to_string())?,
                _ => Value::Null,
            };
            diff.insert(k.clone(), reset_val);
        }
    }
    Ok(diff)
}

fn comment_changes(before: &[CommentThread], after: &[CommentThread]) -> Option<LiveOp> {
    let find = |list: &[CommentThread], id: &str| list.iter().find(|t| t.id == id).cloned();
    let added: Vec<_> = after.iter().filter(|t| find(before, &t.id).is_none()).cloned().collect();
    let removed: Vec<_> = before.iter().filter(|t| find(after, &t.id).is_none()).map(|t| t.id.clone()).collect();
    let changed: Vec<_> = after.iter().filter_map(|a| find(before, &a.id).filter(|b| b != a).map(|b| thread_change(&b, a))).collect();
    (!added.is_empty() || !changed.is_empty() || !removed.is_empty()).then_some(LiveOp::Comments { added, changed, removed })
}

fn thread_change(before: &CommentThread, after: &CommentThread) -> ThreadChange {
    let old = |id: &str| before.messages.iter().find(|m| m.id == id);
    ThreadChange {
        id: after.id.clone(),
        resolved: (before.resolved != after.resolved).then_some(after.resolved),
        anchor: (before.anchor != after.anchor).then_some(after.anchor),
        added: after.messages.iter().filter(|m| old(&m.id).is_none()).cloned().collect(),
        edited: after.messages.iter().filter(|m| old(&m.id).is_some_and(|o| o != *m)).cloned().collect(),
        removed: before.messages.iter().filter(|m| !after.messages.iter().any(|a| a.id == m.id)).map(|m| m.id.clone()).collect(),
    }
}

fn library_state_changes(before: &LibraryState, before_page: Option<NodeId>, after: &LibraryState, after_page: Option<NodeId>) -> Option<LiveOp> {
    let published = (before.published != after.published).then(|| after.published.clone());
    let upserted_enabled: Vec<_> = after.enabled.iter().filter(|r| !before.enabled.contains(r)).cloned().collect();
    let removed_enabled: Vec<_> = before.enabled.iter().filter(|b| !after.enabled.iter().any(|a| a.id == b.id)).map(|b| b.id.clone()).collect();
    let upserted_external: BTreeMap<_, _> =
        after.external.iter().filter(|(k, v)| before.external.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let removed_external: Vec<_> = before.external.keys().filter(|k| !after.external.contains_key(*k)).cloned().collect();
    let library_page = (before_page != after_page).then_some(after_page);
    (published.is_some()
        || !upserted_enabled.is_empty()
        || !removed_enabled.is_empty()
        || !upserted_external.is_empty()
        || !removed_external.is_empty()
        || library_page.is_some())
    .then_some(LiveOp::LibraryState { published, upserted_enabled, removed_enabled, upserted_external, removed_external, library_page })
}

fn command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::InsertPage { .. } => "Adding a page",
        Command::RemovePage { .. } => "Removing a page",
        Command::SetFlows { .. } => "Editing prototype flows",
        Command::SetGuides { .. } => "Editing guides",
        Command::SetPrototypeDevice { .. } => "Editing the prototype device",
        Command::SetCheckDesignsSettings(_) => "Editing check designs settings",
        _ => "This edit",
    }
}
