//! Editor side: replay a [`LivePatch`] on the full document as ordinary [`Command`]s.

use super::replay_comments::{resolve_comments, CommentRemap};
use super::replay_tokens::{filter_missing_token_links_on_insert, filter_missing_token_links_on_update};
use super::validate::insert_problem;
use super::{LiveOp, LivePatch, SiblingHint, LIVE_PATCH_VERSION, STRUCTURAL_FIELDS};
use crate::{Command, DocError, Document, Node, NodeId, NodeKind, Style, StyleMeta, Variable, VariableCollection};
use serde::{Deserialize, Serialize};
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
    /// Variable and collection ids this session's replays added to the editor, mapped to the last
    /// value this session wrote. An agent undo removes only these, and only while the editor still
    /// holds the written value; anything the editor had before, or the user changed since, is left
    /// alone. A later agent edit likewise never overwrites a value the user changed.
    introduced_variables: BTreeMap<String, Variable>,
    introduced_collections: BTreeMap<String, VariableCollection>,
    /// Styles and style metadata this session introduced into the editor document, mapped to the
    /// last value this session wrote. Agent writes go by id and the last one wins; a later agent
    /// edit is refused only when the editor no longer holds the value written here (the user
    /// changed it), and an agent undo removes only ids recorded here whose value is unchanged.
    introduced_styles: BTreeMap<String, Style>,
    introduced_style_meta: BTreeMap<String, StyleMeta>,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Token ops (`Variables` / `Styles`) that turned into commands.
    pub tokens_applied: usize,
    /// Token ops (`Variables` / `Styles`) already reflected in the document.
    pub tokens_duplicates: usize,
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
        // The reloaded canvas already carries what earlier replays added, and the editor's own
        // layers may use it by now, so an undo of an older edit must not remove it.
        state.introduced_variables.clear();
        state.introduced_collections.clear();
        state.introduced_styles.clear();
        state.introduced_style_meta.clear();
        // Its replaced layers are gone from that canvas, so their old ids mean nothing there. The
        // first patch of a session has no earlier canvas to be newer than.
        if state.base.is_some() {
            state.replaced.clear();
        }
        state.base = patch.base;
    }
    let is_spec_undo = patch.ops.iter().any(|o| matches!(o, LiveOp::Remove { .. }));
    for op in &patch.ops {
        if let Some(cmd) = resolve(doc, op, is_spec_undo, state, &mut report) {
            if let Err(e) = exec(doc, cmd) {
                *state = saved;
                return Err(e);
            }
            report.applied += 1;
        }
    }
    doc.prune_in_progress();
    Ok(report)
}

fn resolve(doc: &mut Document, op: &LiveOp, is_spec_undo: bool, state: &mut LivePatchState, report: &mut LivePatchReport) -> Option<Command> {
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
            let mut merged = match merge_fields(cur, set, unset) {
                Ok(n) => n,
                Err(e) => {
                    report.skipped.push(format!("update of node {}: {e}", id.0));
                    return None;
                }
            };
            let skipped_links = filter_missing_token_links_on_update(doc, cur, &mut merged, report);
            if merged == *cur {
                if skipped_links == 0 {
                    report.duplicates += 1;
                }
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
            Some(crate::writer_command(doc, Command::Reparent { id, parent, index }).cmd)
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
            Some(crate::writer_command(doc, Command::Remove { id }).cmd)
        }
        LiveOp::Variables { collections, variables, removed_collections, removed_variables } => {
            resolve_variables(doc, collections, variables, removed_collections, removed_variables, is_spec_undo, state, report)
        }
        LiveOp::Comments { added, changed, removed } => {
            let (remap, replaced) = (&state.remap, &state.replaced);
            let node = |id: NodeId| resolve_id(remap, replaced, doc, id).0;
            resolve_comments(doc, added, changed, removed, &mut state.comments, &node, report)
        }
        LiveOp::SetInProgress { node, state: ip_state } => {
            let id = state.target(doc, *node);
            if doc.get(id).is_none() {
                report.skipped.push(format!("set_in_progress on missing node {}", id.0));
                return None;
            }
            let prev = doc.set_in_progress(id, ip_state.clone());
            if prev.as_ref() == ip_state.as_ref() {
                report.duplicates += 1;
            } else {
                report.applied += 1;
            }
            None
        }
        LiveOp::Styles { styles_set, styles_removed, meta_set, meta_removed } => {
            let mut styles = doc.styles.clone();
            let mut style_meta = doc.style_meta.clone();
            let refused_styles = merge_agent_tokens(&mut styles, &mut state.introduced_styles, styles_set, styles_removed, is_spec_undo, "style", report);
            let refused_meta =
                merge_agent_tokens(&mut style_meta, &mut state.introduced_style_meta, meta_set, meta_removed, is_spec_undo, "style metadata", report);
            if styles == doc.styles && style_meta == doc.style_meta {
                if !(refused_styles || refused_meta) {
                    report.duplicates += 1;
                    report.tokens_duplicates += 1;
                }
                None
            } else {
                report.tokens_applied += 1;
                Some(Command::SetStyles { styles, style_meta })
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

/// The agent's change to one id-keyed token map (styles or style metadata) merged into the
/// editor's copy: writes go by id and the last write wins, the same rule variables follow.
/// An upsert is refused only when this session wrote the id earlier and the editor now holds a
/// different value (the user changed it since). A removal of such an id is refused for the same
/// reason whichever op carries it: an agent undo of `SetStyles` has no `Remove` op, so it cannot
/// be told from a delete. An undo never removes an id this session did not introduce.
/// Every refusal is reported in `skipped`. Returns whether anything was refused.
fn merge_agent_tokens<T: Clone + PartialEq>(
    current: &mut BTreeMap<String, T>,
    introduced: &mut BTreeMap<String, T>,
    upserted: &BTreeMap<String, T>,
    removed: &[String],
    is_spec_undo: bool,
    what: &str,
    report: &mut LivePatchReport,
) -> bool {
    let mut refused = false;
    for k in removed {
        let Some(held) = current.get(k) else {
            introduced.remove(k);
            continue;
        };
        let owned = introduced.get(k);
        if owned.is_some_and(|written| written != held) {
            report.skipped.push(format!("{what} {k} was changed in the editor after the agent wrote it; its removal was not replayed"));
            refused = true;
        } else if owned.is_some() || !is_spec_undo {
            introduced.remove(k);
            current.remove(k);
        }
    }
    for (k, v) in upserted {
        if k.is_empty() {
            report.skipped.push(format!("a {what} without an id was not replayed"));
            refused = true;
            continue;
        }
        if introduced.get(k).is_some_and(|written| current.get(k).is_some_and(|held| held != written)) {
            report.skipped.push(format!("{what} {k} was changed in the editor after the agent wrote it; the agent's edit was not replayed"));
            refused = true;
            continue;
        }
        if !current.contains_key(k) || introduced.contains_key(k) {
            introduced.insert(k.clone(), v.clone());
        }
        current.insert(k.clone(), v.clone());
    }
    refused
}

/// The agent's variable and collection changes merged into the editor's own maps, which can hold
/// more than the agent's copy did. Changed variables and collections are upserted (unioning member
/// lists for existing collections so editor-only members are preserved, and keeping the value of
/// every collection field the user changed since the session wrote it). On an agent spec-undo
/// (`is_spec_undo`), only ids this session introduced are removed so pre-existing editor variables
/// survive; standalone `SetVariables` deletes of ids the editor owns apply directly. An id this
/// session introduced is removed only while the editor still holds the value the session wrote,
/// whichever kind of op removes it (a `SetVariables` undo carries no `Remove` op). Entries without
/// an id are refused.
#[allow(clippy::too_many_arguments)]
fn resolve_variables(
    doc: &Document,
    collections: &[VariableCollection],
    variables: &[Variable],
    removed_collections: &[String],
    removed_variables: &[String],
    is_spec_undo: bool,
    state: &mut LivePatchState,
    report: &mut LivePatchReport,
) -> Option<Command> {
    let mut merged_vars = doc.variables.clone();
    let mut merged_colls = doc.variable_collections.clone();
    let mut refused = false;
    for id in removed_variables {
        let owned = state.introduced_variables.get(id);
        if owned.is_some_and(|written| doc.variables.get(id).is_some_and(|held| held != written)) {
            report.skipped.push(format!("variable {id} was changed in the editor after the agent wrote it; its removal was not replayed"));
            refused = true;
        } else if owned.is_some() || !is_spec_undo {
            state.introduced_variables.remove(id);
            merged_vars.remove(id);
            for coll in merged_colls.values_mut() {
                coll.variables.retain(|vid| vid != id);
            }
        }
    }
    for id in removed_collections {
        let owned = state.introduced_collections.get(id);
        if owned.is_some_and(|written| doc.variable_collections.get(id).is_some_and(|held| held != written)) {
            report.skipped.push(format!("variable collection {id} was changed in the editor after the agent wrote it; its removal was not replayed"));
            refused = true;
        } else if owned.is_some() {
            // Variables that survived (the user changed them) keep their collection.
            if let Some(user_var) = merged_vars.values().find(|v| v.collection == *id) {
                report.skipped.push(format!("variable collection {id} was kept: variable {} still uses it", user_var.id));
                refused = true;
            } else {
                state.introduced_collections.remove(id);
                merged_colls.remove(id);
            }
        } else if !is_spec_undo {
            merged_colls.remove(id);
        }
    }
    for v in variables {
        if v.id.is_empty() {
            report.skipped.push("a variable without an id was not replayed".to_string());
            refused = true;
        } else {
            let user_changed = state.introduced_variables.get(&v.id).is_some_and(|written| merged_vars.get(&v.id).is_some_and(|cur| cur != written));
            if user_changed {
                report.skipped.push(format!("variable {} was changed in the editor after the agent wrote it; the agent's edit was not replayed", v.id));
                refused = true;
                continue;
            }
            if !merged_vars.contains_key(&v.id) || state.introduced_variables.contains_key(&v.id) {
                state.introduced_variables.insert(v.id.clone(), v.clone());
            }
            merged_vars.insert(v.id.clone(), v.clone());
        }
    }
    for c in collections {
        if c.id.is_empty() {
            report.skipped.push("a variable collection without an id was not replayed".to_string());
            continue;
        }
        // Fields the user changed since the agent wrote the collection keep the user's value;
        // the agent's other fields apply. Members always union, so a variable the agent adds
        // stays visible in the collection however the user edited it.
        let (mut merged, mut written) = (c.clone(), c.clone());
        let editor_copy = merged_colls.get(&c.id).cloned();
        if let (Some(held), Some(prior)) = (&editor_copy, state.introduced_collections.get(&c.id)) {
            let kept = keep_user_collection_fields(c, held, prior, &mut merged, &mut written);
            if !kept.is_empty() {
                report.skipped.push(format!(
                    "variable collection {}: {} changed in the editor after the agent wrote it; the agent's value was not replayed",
                    c.id,
                    kept.join(", ")
                ));
                refused = true;
            }
        }
        if let Some(cur) = &editor_copy {
            let mut members: Vec<String> =
                cur.variables.iter().filter(|vid| !removed_variables.contains(vid) || merged_vars.contains_key(*vid)).cloned().collect();
            for vid in &c.variables {
                if !members.contains(vid) {
                    members.push(vid.clone());
                }
            }
            merged.variables = members.clone();
            written.variables = members;
        }
        if !merged_colls.contains_key(&c.id) || state.introduced_collections.contains_key(&c.id) {
            state.introduced_collections.insert(c.id.clone(), written);
        }
        merged_colls.insert(c.id.clone(), merged);
    }
    if merged_vars == doc.variables && merged_colls == doc.variable_collections {
        if !refused {
            report.duplicates += 1;
            report.tokens_duplicates += 1;
        }
        return None;
    }
    report.tokens_applied += 1;
    Some(Command::SetVariables { variables: merged_vars, collections: merged_colls })
}

/// Field-wise merge of an agent upsert `agent` of a collection this session introduced. `merged`
/// (what the editor will hold) and `written` (what is recorded as the session's last write) start
/// as the agent's value. For each scalar field the user changed since the session wrote it (`held`
/// differs from `prior`), `merged` keeps the user's value and `written` keeps the value the
/// session wrote, so the change is still recognised as the user's on a later undo. Returns the
/// names of the fields where the agent's value was refused.
fn keep_user_collection_fields(
    agent: &VariableCollection,
    held: &VariableCollection,
    prior: &VariableCollection,
    merged: &mut VariableCollection,
    written: &mut VariableCollection,
) -> Vec<&'static str> {
    let mut kept = Vec::new();
    macro_rules! field {
        ($f:ident) => {
            if held.$f != prior.$f {
                merged.$f = held.$f.clone();
                if agent.$f == held.$f {
                    written.$f = held.$f.clone();
                } else {
                    written.$f = prior.$f.clone();
                    kept.push(stringify!($f));
                }
            }
        };
    }
    field!(name);
    field!(modes);
    field!(default_mode);
    field!(key);
    field!(source_library_key);
    field!(extensions);
    kept
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
    filter_missing_token_links_on_insert(doc, &mut nodes, report);
    nodes[0].parent = Some(parent);
    let siblings = &doc.get(parent)?.children;
    let index = remap_hint(at, state).index_in(siblings);
    report.inserted_roots.push(nodes[0].id);
    report.touched.push((nodes[0].id, TouchKind::Inserted));
    report.needs_layout.push(parent);
    Some(crate::writer_command(doc, Command::Insert { parent, index, nodes }).cmd)
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
            if k == "component" {
                if let Value::Object(set_comp) = v {
                    let cur_comp = match fields.entry("component".to_string()).or_insert_with(|| Value::Object(Map::new())) {
                        Value::Object(m) => m,
                        other => {
                            *other = Value::Object(Map::new());
                            other.as_object_mut().unwrap()
                        }
                    };
                    for (sub_k, sub_v) in set_comp {
                        if sub_k == "overrides_add" {
                            if let Value::Array(add_ovs) = sub_v {
                                let mut merged_ovs = cur_comp.get("overrides").and_then(Value::as_array).cloned().unwrap_or_default();
                                for ov in add_ovs {
                                    if !merged_ovs.contains(ov) {
                                        merged_ovs.push(ov.clone());
                                    }
                                }
                                cur_comp.insert("overrides".to_string(), Value::Array(merged_ovs));
                            }
                            continue;
                        }
                        if sub_k == "overrides_remove" {
                            if let Value::Array(rem_ovs) = sub_v {
                                let mut merged_ovs = cur_comp.get("overrides").and_then(Value::as_array).cloned().unwrap_or_default();
                                merged_ovs.retain(|ov| !rem_ovs.contains(ov));
                                cur_comp.insert("overrides".to_string(), Value::Array(merged_ovs));
                            }
                            continue;
                        }
                        if sub_k == "overrides" {
                            if let Value::Array(set_ovs) = sub_v {
                                if set_ovs.is_empty() {
                                    cur_comp.insert(sub_k.clone(), Value::Array(Vec::new()));
                                } else {
                                    let mut merged_ovs = cur_comp.get("overrides").and_then(Value::as_array).cloned().unwrap_or_default();
                                    for ov in set_ovs {
                                        if !merged_ovs.contains(ov) {
                                            merged_ovs.push(ov.clone());
                                        }
                                    }
                                    cur_comp.insert(sub_k.clone(), Value::Array(merged_ovs));
                                }
                                continue;
                            }
                        }
                        cur_comp.insert(sub_k.clone(), sub_v.clone());
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
