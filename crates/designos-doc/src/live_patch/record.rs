//! Agent side: turn a command about to be executed into the [`LiveOp`]s it performs.

use super::{LiveOp, SiblingHint, ThreadChange, STRUCTURAL_FIELDS};
use crate::{Command, CommentThread, Document, LibraryState, Node, NodeId, Style, StyleMeta, Variable, VariableCollection};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

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
        Command::SetStyles { styles, style_meta } => {
            if let Some(op) = style_changes(&doc.styles, &doc.style_meta, styles, style_meta) {
                out.push(op);
            }
        }
        Command::SetVariables { variables, collections } => {
            if let Some(op) = variable_changes(&doc.variables, &doc.variable_collections, variables, collections) {
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
            set.insert(k.clone(), v.clone());
        }
    }
    let unset = old.keys().filter(|k| !STRUCTURAL_FIELDS.contains(&k.as_str()) && !new.contains_key(*k)).cloned().collect();
    Ok((set, unset))
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

fn style_changes(
    before_styles: &BTreeMap<String, Style>,
    before_meta: &BTreeMap<String, StyleMeta>,
    after_styles: &BTreeMap<String, Style>,
    after_meta: &BTreeMap<String, StyleMeta>,
) -> Option<LiveOp> {
    let upserted: BTreeMap<_, _> = after_styles.iter().filter(|(k, v)| before_styles.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let upserted_meta: BTreeMap<_, _> = after_meta.iter().filter(|(k, v)| before_meta.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let removed: Vec<_> = before_styles.keys().filter(|k| !after_styles.contains_key(*k)).cloned().collect();
    let removed_meta: Vec<_> = before_meta.keys().filter(|k| !after_meta.contains_key(*k)).cloned().collect();
    (!upserted.is_empty() || !upserted_meta.is_empty() || !removed.is_empty() || !removed_meta.is_empty()).then_some(LiveOp::Styles {
        upserted,
        upserted_meta,
        removed,
        removed_meta,
    })
}

fn variable_changes(
    before_vars: &BTreeMap<String, Variable>,
    before_colls: &BTreeMap<String, VariableCollection>,
    after_vars: &BTreeMap<String, Variable>,
    after_colls: &BTreeMap<String, VariableCollection>,
) -> Option<LiveOp> {
    let upserted_collections: BTreeMap<_, _> =
        after_colls.iter().filter(|(k, v)| before_colls.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let removed_collections: Vec<_> = before_colls.keys().filter(|k| !after_colls.contains_key(*k)).cloned().collect();
    let upserted_variables: BTreeMap<_, _> = after_vars.iter().filter(|(k, v)| before_vars.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let removed_variables: Vec<_> = before_vars.keys().filter(|k| !after_vars.contains_key(*k)).cloned().collect();
    (!upserted_collections.is_empty() || !removed_collections.is_empty() || !upserted_variables.is_empty() || !removed_variables.is_empty())
        .then_some(LiveOp::Variables { upserted_collections, removed_collections, upserted_variables, removed_variables })
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
