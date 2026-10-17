//! Agent side: turn a command about to be executed into the [`LiveOp`]s it performs.

use super::{LiveOp, SiblingHint, ThreadChange, STRUCTURAL_FIELDS};
use crate::{Command, CommentThread, Document, Node};
use serde_json::{Map, Value};

/// The [`LiveOp`]s `cmd` performs when applied to `doc`, read from the state before it runs.
///
/// Errors for commands whose effect cannot be replayed on a document the agent only partly
/// holds (page, style, variable, flow, library, guide and device edits); the caller should
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

fn command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::InsertPage { .. } => "Adding a page",
        Command::RemovePage { .. } => "Removing a page",
        Command::SetFlows { .. } => "Editing prototype flows",
        Command::SetStyles { .. } => "Editing shared styles",
        Command::SetLibraryState { .. } => "Editing library state",
        Command::SetGuides { .. } => "Editing guides",
        Command::SetPrototypeDevice { .. } => "Editing the prototype device",
        Command::SetVariables { .. } => "Editing variables",
        _ => "This edit",
    }
}
