//! Editor side: replay a [`LivePatch`] on the full document as ordinary [`Command`]s.

use super::replay_comments::{resolve_comments, CommentRemap};
use super::{LiveOp, LivePatch, SiblingHint, LIVE_PATCH_VERSION, STRUCTURAL_FIELDS};
use crate::{Command, DocError, Document, Node, NodeId};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Fields whose change can affect an auto-layout container's arrangement.
const LAYOUT_FIELDS: [&str; 4] = ["size", "kind", "layout_child", "visible"];

/// Replay state kept across patches of one agent session.
#[derive(Clone, Debug, Default)]
pub struct LivePatchState {
    /// Agent ids that collided with nodes the editor created meanwhile, and their new ids.
    remap: BTreeMap<NodeId, NodeId>,
    /// The same for comment thread and message ids.
    comments: CommentRemap,
    /// [`LivePatch::base`] the remaps above were made under.
    base: Option<u64>,
}

/// What a replay did, for the toast, the post-replay layout pass and tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LivePatchReport {
    /// Roots of inserted subtrees, in insertion order.
    pub inserted_roots: Vec<NodeId>,
    /// Nodes whose change can move or resize siblings in an auto-layout container: new or
    /// removed children, moves, and size, visibility or layout changes.
    pub needs_layout: Vec<NodeId>,
    /// Ops that turned into commands.
    pub applied: usize,
    /// Ops already reflected in the document (a repeated delivery).
    pub duplicates: usize,
    /// Ops that could not be replayed, with the reason.
    pub skipped: Vec<String>,
}

/// Replays `patch` on `doc`, one op at a time so each is checked against the state the previous
/// ones left. Every command goes through `exec`, so the editor can run them in one transaction.
/// Ops whose target is gone are skipped and reported; only an `exec` failure aborts.
pub fn replay_live_patch(
    doc: &mut Document,
    patch: &LivePatch,
    state: &mut LivePatchState,
    exec: &mut dyn FnMut(&mut Document, Command) -> Result<(), DocError>,
) -> Result<LivePatchReport, DocError> {
    let mut report = LivePatchReport::default();
    if patch.v != LIVE_PATCH_VERSION {
        report.skipped.push(format!("unsupported live patch version {}", patch.v));
        return Ok(report);
    }
    if patch.base != state.base {
        // The agent reloaded a canvas the editor wrote with its own ids; earlier remaps would
        // now send its edits to the wrong node or thread.
        state.remap.clear();
        state.comments = CommentRemap::default();
        state.base = patch.base;
    }
    for op in &patch.ops {
        if let Some(cmd) = resolve(doc, op, state, &mut report) {
            exec(doc, cmd)?;
            report.applied += 1;
        }
    }
    Ok(report)
}

fn resolve(doc: &mut Document, op: &LiveOp, state: &mut LivePatchState, report: &mut LivePatchReport) -> Option<Command> {
    let map = |id: NodeId, state: &LivePatchState| state.remap.get(&id).copied().unwrap_or(id);
    match op {
        LiveOp::Insert { parent, at, nodes } => resolve_insert(doc, map(*parent, state), at, nodes, state, report),
        LiveOp::Update { id, set, unset } => {
            let id = map(*id, state);
            let Some(cur) = doc.get(id) else {
                report.skipped.push(format!("update of missing node {}", id.0));
                return None;
            };
            let merged = match merge_fields(cur, set, unset) {
                Ok(n) => n,
                Err(e) => {
                    report.skipped.push(format!("update of node {}: {e}", id.0));
                    return None;
                }
            };
            if merged == *cur {
                report.duplicates += 1;
                return None;
            }
            if set.keys().chain(unset).any(|k| LAYOUT_FIELDS.contains(&k.as_str())) {
                report.needs_layout.push(id);
            }
            Some(Command::Update { node: Box::new(merged) })
        }
        LiveOp::Reparent { id, parent, at } => {
            let (id, parent) = (map(*id, state), map(*parent, state));
            let at = remap_hint(at, state);
            let (Some(node), Some(p)) = (doc.get(id), doc.get(parent)) else {
                report.skipped.push(format!("move of node {} into missing parent {}", id.0, parent.0));
                return None;
            };
            if !p.kind.is_container() || id == parent || doc.is_ancestor(id, parent) || node.parent.is_none() {
                report.skipped.push(format!("move of node {} into {} is not allowed", id.0, parent.0));
                return None;
            }
            let siblings: Vec<_> = p.children.iter().copied().filter(|c| *c != id).collect();
            let index = at.index_in(&siblings);
            if node.parent == Some(parent) && p.children.iter().position(|c| *c == id) == Some(index) {
                report.duplicates += 1;
                return None;
            }
            report.needs_layout.extend(node.parent);
            report.needs_layout.extend([parent, id]);
            Some(Command::Reparent { id, parent, index })
        }
        LiveOp::Remove { id } => {
            let id = map(*id, state);
            let Some(parent) = doc.get(id).map(|n| n.parent) else {
                report.duplicates += 1;
                return None;
            };
            let Some(parent) = parent else {
                report.skipped.push(format!("page {} cannot be removed by an agent edit", id.0));
                return None;
            };
            report.needs_layout.push(parent);
            Some(Command::Remove { id })
        }
        LiveOp::Comments { added, changed, removed } => {
            let node = |id: NodeId| state.remap.get(&id).copied().unwrap_or(id);
            resolve_comments(doc, added, changed, removed, &mut state.comments, &node, report)
        }
    }
}

fn resolve_insert(
    doc: &mut Document,
    parent: NodeId,
    at: &SiblingHint,
    nodes: &[Node],
    state: &mut LivePatchState,
    report: &mut LivePatchReport,
) -> Option<Command> {
    let Some(root) = nodes.first() else { return None };
    if !doc.get(parent).is_some_and(|p| p.kind.is_container()) {
        report.skipped.push(format!("insert of node {} into missing parent {}", root.id.0, parent.0));
        return None;
    }
    let mut nodes: Vec<Node> = nodes.to_vec();
    remap_subtree(&mut nodes, &state.remap);
    if let Some(existing) = doc.get(nodes[0].id) {
        if existing.parent == Some(parent) && same_props(existing, &nodes[0]) {
            report.duplicates += 1;
            return None;
        }
    }
    // Ids the editor already uses (created while the agent worked) get fresh ones.
    let largest = nodes.iter().map(|n| n.id).max().unwrap_or(NodeId(0));
    doc.reserve_ids(largest);
    let mut fresh = BTreeMap::new();
    for n in &nodes {
        if doc.get(n.id).is_some() {
            fresh.insert(n.id, doc.try_alloc_id()?);
        }
    }
    if !fresh.is_empty() {
        remap_subtree(&mut nodes, &fresh);
        state.remap.extend(fresh);
    }
    // Children outside the subtree are existing nodes; their own Reparent ops move them here.
    let ids: BTreeSet<NodeId> = nodes.iter().map(|n| n.id).collect();
    for n in &mut nodes {
        n.children.retain(|c| ids.contains(c));
    }
    nodes[0].parent = Some(parent);
    let siblings = &doc.get(parent)?.children;
    let index = remap_hint(at, state).index_in(siblings);
    report.inserted_roots.push(nodes[0].id);
    report.needs_layout.push(parent);
    Some(Command::Insert { parent, index, nodes })
}

/// `cur` with the changed top-level fields of `set` / `unset` applied; structure is kept.
fn merge_fields(cur: &Node, set: &Map<String, Value>, unset: &[String]) -> Result<Node, String> {
    let Value::Object(mut fields) = serde_json::to_value(cur).map_err(|e| e.to_string())? else {
        return Err("node did not serialize to an object".into());
    };
    for (k, v) in set {
        if !STRUCTURAL_FIELDS.contains(&k.as_str()) {
            fields.insert(k.clone(), v.clone());
        }
    }
    for k in unset {
        if !STRUCTURAL_FIELDS.contains(&k.as_str()) {
            fields.remove(k);
        }
    }
    let mut merged: Node = serde_json::from_value(Value::Object(fields)).map_err(|e| e.to_string())?;
    merged.id = cur.id;
    merged.parent = cur.parent;
    merged.children = cur.children.clone();
    Ok(merged)
}

fn same_props(a: &Node, b: &Node) -> bool {
    let strip = |n: &Node| Node { parent: None, children: Vec::new(), ..n.clone() };
    strip(a) == strip(b)
}

fn remap_subtree(nodes: &mut [Node], map: &BTreeMap<NodeId, NodeId>) {
    if map.is_empty() {
        return;
    }
    let m = |id: NodeId| map.get(&id).copied().unwrap_or(id);
    for n in nodes {
        n.id = m(n.id);
        n.parent = n.parent.map(m);
        for c in &mut n.children {
            *c = m(*c);
        }
    }
}

fn remap_hint(at: &SiblingHint, state: &LivePatchState) -> SiblingHint {
    let m = |id: NodeId| state.remap.get(&id).copied().unwrap_or(id);
    SiblingHint { after: at.after.map(m), before: at.before.map(m) }
}
