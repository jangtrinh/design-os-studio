//! Editor side: replay a [`LivePatch`] on the full document as ordinary [`Command`]s.

use super::replay_comments::{resolve_comments, CommentRemap};
use super::validate::insert_problem;
use super::{LiveOp, LivePatch, SiblingHint, LIVE_PATCH_VERSION, STRUCTURAL_FIELDS};
use crate::{Command, DocError, Document, Node, NodeId, NodeKind};
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
    /// Layers the editor replaced by another one while the agent still knew the old id (the
    /// shape builder's remainder of a layer). Used only while the old id is gone, so undoing the
    /// replacement sends edits back to the restored layer.
    replaced: BTreeMap<NodeId, NodeId>,
}

impl LivePatchState {
    /// The editor removed layer `old` and put `new` in its place; agent edits to `old` that have
    /// not been replayed yet go to `new`.
    pub fn record_replacement(&mut self, old: NodeId, new: NodeId) {
        self.replaced.insert(old, new);
    }

    /// The editor-side id for an id from the agent's document.
    fn target(&self, doc: &Document, id: NodeId) -> NodeId {
        resolve_id(&self.remap, &self.replaced, doc, id).0
    }
}

/// The editor-side id for an id from the agent's document, and whether it is a layer the
/// editor replaced by another one (so the agent's shape numbers describe a different layer).
fn resolve_id(remap: &BTreeMap<NodeId, NodeId>, replaced: &BTreeMap<NodeId, NodeId>, doc: &Document, id: NodeId) -> (NodeId, bool) {
    let id = remap.get(&id).copied().unwrap_or(id);
    if doc.get(id).is_some() {
        return (id, false);
    }
    match replaced.get(&id) {
        Some(new) => (*new, true),
        None => (id, false),
    }
}

/// Fields that describe a layer's shape in its own coordinate frame. The shape builder rebuilds
/// a layer from geometry in the frame of its parent, so the agent's numbers for these do not
/// apply to the replacement.
const SHAPE_FIELDS: [&str; 6] = ["transform", "size", "kind", "fill_geometry", "stroke_geometry", "corner_radii"];

/// How a replayed live-patch op touched a layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchKind {
    Inserted,
    Updated,
    Moved,
    Removed,
}

/// What a replay did, for the toast, the post-replay layout pass and tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LivePatchReport {
    /// Roots of inserted subtrees, in insertion order.
    pub inserted_roots: Vec<NodeId>,
    /// Layers touched by the replayed ops, in op order, in editor ids (after remap).
    pub touched: Vec<(NodeId, TouchKind)>,
    /// Parent ids of removed layers before removal, keyed by the removed layer's editor id.
    pub removed_parents: BTreeMap<NodeId, NodeId>,
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
    // A failed replay is cancelled by the caller, so the remaps it made must go too.
    let saved = state.clone();
    if patch.base != state.base {
        // The agent reloaded a canvas the editor wrote with its own ids; earlier remaps would
        // now send its edits to the wrong node or thread.
        state.remap.clear();
        state.comments = CommentRemap::default();
        // Its replaced layers are gone from that canvas, so their old ids mean nothing there. The
        // first patch of a session has no earlier canvas to be newer than.
        if state.base.is_some() {
            state.replaced.clear();
        }
        state.base = patch.base;
    }
    for op in &patch.ops {
        if let Some(cmd) = resolve(doc, op, state, &mut report) {
            if let Err(e) = exec(doc, cmd) {
                *state = saved;
                return Err(e);
            }
            report.applied += 1;
        }
    }
    Ok(report)
}

fn resolve(doc: &mut Document, op: &LiveOp, state: &mut LivePatchState, report: &mut LivePatchReport) -> Option<Command> {
    match op {
        LiveOp::Insert { parent: agent_parent, at, nodes } => {
            let (parent, redirected) = resolve_id(&state.remap, &state.replaced, doc, *agent_parent);
            if redirected {
                report.skipped.push(format!(
                    "insert into layer {} refused: layer {} was reshaped by the shape builder while this edit waited",
                    agent_parent.0, agent_parent.0
                ));
                return None;
            }
            resolve_insert(doc, parent, at, nodes, state, report)
        }
        LiveOp::Update { id: agent_id, set, unset } => {
            let (id, redirected) = resolve_id(&state.remap, &state.replaced, doc, *agent_id);
            let kept;
            let (set, unset) = if redirected {
                kept = without_shape_fields(*agent_id, set, unset, report);
                if kept.0.is_empty() && kept.1.is_empty() {
                    return None;
                }
                (&kept.0, &kept.1)
            } else {
                (set, unset)
            };
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
            let is_page = |n: &Node| matches!(n.kind, NodeKind::Page { .. });
            if is_page(cur) != is_page(&merged) {
                report.skipped.push(format!("update of node {} would turn a layer into a page or a page into a layer", id.0));
                return None;
            }
            if !cur.children.is_empty() && !merged.kind.is_container() {
                report.skipped.push(format!("update of node {} would turn a layer with children into one that cannot hold them", id.0));
                return None;
            }
            if set.keys().chain(unset).any(|k| LAYOUT_FIELDS.contains(&k.as_str())) {
                report.needs_layout.push(id);
            }
            report.touched.push((id, TouchKind::Updated));
            Some(crate::writer_command(doc, Command::Update { node: Box::new(merged) }).cmd)
        }
        LiveOp::Reparent { id: agent_id, parent: agent_parent, at } => {
            let (id, id_redirected) = resolve_id(&state.remap, &state.replaced, doc, *agent_id);
            let (parent, parent_redirected) = resolve_id(&state.remap, &state.replaced, doc, *agent_parent);
            if id_redirected {
                report
                    .skipped
                    .push(format!("reparent of layer {} refused: layer {} was reshaped by the shape builder while this edit waited", agent_id.0, agent_id.0));
                return None;
            }
            if parent_redirected {
                report.skipped.push(format!(
                    "reparent into layer {} refused: layer {} was reshaped by the shape builder while this edit waited",
                    agent_parent.0, agent_parent.0
                ));
                return None;
            }
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
            report.touched.push((id, TouchKind::Moved));
            Some(Command::Reparent { id, parent, index })
        }
        LiveOp::Remove { id } => {
            let id = state.target(doc, *id);
            let Some(parent) = doc.get(id).map(|n| n.parent) else {
                report.duplicates += 1;
                return None;
            };
            let Some(parent) = parent else {
                report.skipped.push(format!("page {} cannot be removed by an agent edit", id.0));
                return None;
            };
            report.needs_layout.push(parent);
            report.removed_parents.insert(id, parent);
            report.touched.push((id, TouchKind::Removed));
            Some(Command::Remove { id })
        }
        LiveOp::Comments { added, changed, removed } => {
            let (remap, replaced) = (&state.remap, &state.replaced);
            let node = |id: NodeId| resolve_id(remap, replaced, doc, id).0;
            resolve_comments(doc, added, changed, removed, &mut state.comments, &node, report)
        }
        LiveOp::Styles { upserted, upserted_meta, removed, removed_meta } => {
            let mut styles = doc.styles.clone();
            let mut style_meta = doc.style_meta.clone();
            for k in removed {
                styles.remove(k);
            }
            for k in removed_meta {
                style_meta.remove(k);
            }
            for (k, v) in upserted {
                styles.insert(k.clone(), v.clone());
            }
            for (k, v) in upserted_meta {
                style_meta.insert(k.clone(), v.clone());
            }
            if styles == doc.styles && style_meta == doc.style_meta {
                report.duplicates += 1;
                None
            } else {
                Some(Command::SetStyles { styles, style_meta })
            }
        }
        LiveOp::Variables { upserted_collections, removed_collections, upserted_variables, removed_variables } => {
            let mut collections = doc.variable_collections.clone();
            let mut variables = doc.variables.clone();
            for k in removed_collections {
                collections.remove(k);
            }
            for k in removed_variables {
                variables.remove(k);
            }
            for (k, v) in upserted_collections {
                let mut merged = v.clone();
                if let Some(cur) = doc.variable_collections.get(k) {
                    for vid in &cur.variables {
                        if !removed_variables.contains(vid) && !merged.variables.contains(vid) {
                            merged.variables.push(vid.clone());
                        }
                    }
                }
                collections.insert(k.clone(), merged);
            }
            for (k, v) in upserted_variables {
                variables.insert(k.clone(), v.clone());
            }
            if collections == doc.variable_collections && variables == doc.variables {
                report.duplicates += 1;
                None
            } else {
                Some(Command::SetVariables { variables, collections })
            }
        }
        LiveOp::LibraryState { published, upserted_enabled, removed_enabled, upserted_external, removed_external, library_page } => {
            let mut new_state = doc.library.clone();
            let mut new_lp = doc.library_page;
            if let Some(lp_change) = library_page {
                new_lp = match lp_change {
                    Some(agent_lp) => {
                        if let Some(existing_lp) = doc.library_page.filter(|p| doc.nodes.contains_key(p)) {
                            if existing_lp != *agent_lp {
                                state.remap.insert(*agent_lp, existing_lp);
                            }
                            Some(existing_lp)
                        } else if let Some(&mapped) = state.remap.get(agent_lp) {
                            Some(mapped)
                        } else if doc.get(*agent_lp).is_some() {
                            let Some(fresh_lp) = doc.try_alloc_id() else {
                                report.skipped.push("library page allocation failed: no node ids left".into());
                                return None;
                            };
                            state.remap.insert(*agent_lp, fresh_lp);
                            Some(fresh_lp)
                        } else {
                            doc.reserve_ids(*agent_lp);
                            Some(*agent_lp)
                        }
                    }
                    None => {
                        if doc.library_page.is_none_or(|p| doc.get(p).is_none_or(|n| n.children.is_empty())) {
                            None
                        } else {
                            doc.library_page
                        }
                    }
                };
            }
            if let Some(pub_info) = published {
                new_state.published = pub_info.clone();
            }
            for id in removed_enabled {
                new_state.enabled.retain(|r| r.id != *id);
            }
            for lref in upserted_enabled {
                if let Some(existing) = new_state.enabled.iter_mut().find(|r| r.id == lref.id) {
                    *existing = lref.clone();
                } else {
                    new_state.enabled.push(lref.clone());
                }
            }
            for k in removed_external {
                new_state.external.remove(k);
            }
            for (k, ext) in upserted_external {
                let mut ext = ext.clone();
                ext.node = ext.node.map(|id| state.target(doc, id));
                new_state.external.insert(k.clone(), ext);
            }
            if new_state == doc.library && new_lp == doc.library_page && new_lp.is_none_or(|id| doc.nodes.contains_key(&id)) {
                report.duplicates += 1;
                None
            } else {
                Some(Command::SetLibraryState { state: new_state, library_page: new_lp })
            }
        }
        LiveOp::Images { add, remove } => {
            let add: BTreeMap<String, Vec<u8>> = add.iter().filter(|(k, _)| !doc.images.contains_key(*k)).map(|(k, v)| (k.clone(), v.clone())).collect();
            let remove: Vec<String> = remove.iter().filter(|k| doc.images.contains_key(*k)).cloned().collect();
            if add.is_empty() && remove.is_empty() {
                report.duplicates += 1;
                None
            } else {
                Some(Command::SetImages { add, remove })
            }
        }
    }
}

/// `set` and `unset` of an edit to a replaced layer without the shape fields, each of which is
/// reported as refused.
fn without_shape_fields(agent_id: NodeId, set: &Map<String, Value>, unset: &[String], report: &mut LivePatchReport) -> (Map<String, Value>, Vec<String>) {
    let mut refuse = |field: &str| {
        report.skipped.push(format!("{field} change refused: layer {} was reshaped by the shape builder while this edit waited", agent_id.0));
    };
    let mut kept_set = Map::new();
    for (field, value) in set {
        if SHAPE_FIELDS.contains(&field.as_str()) {
            refuse(field);
        } else {
            kept_set.insert(field.clone(), value.clone());
        }
    }
    let mut kept_unset = Vec::new();
    for field in unset {
        if SHAPE_FIELDS.contains(&field.as_str()) {
            refuse(field);
        } else {
            kept_unset.push(field.clone());
        }
    }
    (kept_set, kept_unset)
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
    // Before anything below reserves ids for it.
    if let Some(why) = insert_problem(doc, parent, nodes) {
        report.skipped.push(format!("insert of node {}: {why}", root.id.0));
        return None;
    }
    let mut preview: Vec<Node> = nodes.to_vec();
    remap_subtree(&mut preview, &state.remap);
    if let Some(existing) = doc.get(preview[0].id) {
        if existing.parent == Some(parent) && same_props(existing, &preview[0]) {
            report.duplicates += 1;
            return None;
        }
    }
    // Ids the editor already uses (created while the agent worked) get fresh ones.
    let largest = preview.iter().map(|n| n.id).max().unwrap_or(NodeId(0));
    doc.reserve_ids(largest);
    for orig in nodes {
        let candidate = state.remap.get(&orig.id).copied().unwrap_or(orig.id);
        if doc.get(candidate).is_some() {
            let Some(id) = doc.try_alloc_id() else {
                report.skipped.push(format!("insert of node {}: the document has no node ids left", preview[0].id.0));
                return None;
            };
            state.remap.insert(orig.id, id);
        }
    }
    let mut nodes: Vec<Node> = nodes.to_vec();
    remap_subtree(&mut nodes, &state.remap);
    // Children outside the subtree are existing nodes; their own Reparent ops move them here.
    let ids: BTreeSet<NodeId> = nodes.iter().map(|n| n.id).collect();
    for n in &mut nodes {
        n.children.retain(|c| ids.contains(c));
    }
    nodes[0].parent = Some(parent);
    let siblings = &doc.get(parent)?.children;
    let index = remap_hint(at, state).index_in(siblings);
    report.inserted_roots.push(nodes[0].id);
    report.touched.push((nodes[0].id, TouchKind::Inserted));
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
            if k == "layout_child" {
                if let (Some(Value::Object(cur_lc)), Value::Object(set_lc)) = (fields.get_mut("layout_child"), v) {
                    for (sub_k, sub_v) in set_lc {
                        cur_lc.insert(sub_k.clone(), sub_v.clone());
                    }
                    continue;
                }
            }
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
        crate::renumber_ids::remap_node(n, &m);
    }
}

fn remap_hint(at: &SiblingHint, state: &LivePatchState) -> SiblingHint {
    let m = |id: NodeId| state.remap.get(&id).copied().unwrap_or(id);
    SiblingHint { after: at.after.map(m), before: at.before.map(m) }
}
