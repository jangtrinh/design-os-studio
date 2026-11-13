//! T10.3 (`#345`, `R5.18`): Merge a branch into `main` (or pull `main` into a
//! branch) with deterministic ID remapping, per-layer conflict resolution,
//! reference validation, and single-`Command::Batch` undo.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::branch_diff::{diff3, flow_key, Conflict, ConflictKind, Diff3, EntityChange, EntityKey, Group, NodeChange, SideChanges};
use crate::{
    Action, Command, Comment, CommentAnchor, CommentThread, ComponentLink, Document, ExprValue, Expression, Flow, Guide, History, LibraryAssetKind,
    LibraryState, Node, NodeId, NodeKind, PropKind, PropValue, Style, StyleMeta, Variable, VariableCollection, VariableValue,
};

/// Which side wins a conflict during a 3-way merge (`R5.18`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Side {
    /// Keep `main` ("ours").
    Main,
    /// Take `branch` ("theirs").
    Branch,
}

/// Alias used by callers that refer to `ConflictSide`.
pub type ConflictSide = Side;

/// Caller-supplied conflict resolution choices for a 3-way merge (`T10.3 §4`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub per_layer: BTreeMap<NodeId, Side>,
    pub per_entity: BTreeMap<EntityKey, Side>,
}

impl Resolution {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_layer(mut self, id: NodeId, side: Side) -> Self {
        self.per_layer.insert(id, side);
        self
    }

    pub fn with_entity(mut self, key: EntityKey, side: Side) -> Self {
        self.per_entity.insert(key, side);
        self
    }
}

/// An unresolved conflict reported by [`MergeError::Unresolved`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum UnresolvedItem {
    Layer(NodeId),
    Entity(EntityKey),
}

impl std::fmt::Display for UnresolvedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layer(id) => write!(f, "layer {}", id.0),
            Self::Entity(key) => write!(f, "entity {key:?}"),
        }
    }
}

/// A reference in the merged document whose target no longer resolves (`T10.3 §2`, `§3.7`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct DanglingReference {
    pub owner: String,
    pub field: &'static str,
    pub target: String,
}

impl std::fmt::Display for DanglingReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {} references missing {}", self.owner, self.field, self.target)
    }
}

/// Failure modes for [`merge`] and library branch merge (`T10.3 §4`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum MergeError {
    /// One or more conflicts in `Diff3` lacked a choice in `Resolution`.
    Unresolved(Vec<UnresolvedItem>),
    /// The merged document would contain dangling references (`T10.3 §8.1`, `§8.2`).
    DanglingReference(Vec<DanglingReference>),
    /// A generated merge command failed to apply against `main`.
    Apply { step: usize, reason: String },
    /// The merged working tree would make a component hold an instance of itself.
    InstanceCycle(String),
    /// The merged variable store would close a variable alias cycle.
    AliasCycle(String),
    /// Attempted to merge an archived or already-merged branch (`T10.3 §8.6`).
    BranchArchived(String),
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unresolved(items) => {
                let list: Vec<String> = items.iter().map(ToString::to_string).collect();
                write!(f, "unresolved merge conflicts: {}", list.join(", "))
            }
            Self::DanglingReference(refs) => {
                let list: Vec<String> = refs.iter().map(ToString::to_string).collect();
                write!(f, "dangling references after merge: {}", list.join("; "))
            }
            Self::Apply { step, reason } => {
                write!(f, "merge command failed at step {step}: {reason}")
            }
            Self::InstanceCycle(message) | Self::AliasCycle(message) => write!(f, "{message}"),
            Self::BranchArchived(name) => {
                write!(f, "branch '{name}' is archived or already merged")
            }
        }
    }
}

impl std::error::Error for MergeError {}

/// Summary of ID remaps and applied changes produced by [`merge`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeReport {
    pub remapped_nodes: BTreeMap<NodeId, NodeId>,
    pub remapped_styles: BTreeMap<String, String>,
    pub remapped_variables: BTreeMap<String, String>,
    pub remapped_collections: BTreeMap<String, String>,
    /// Variables added on both `main` and `branch` with different IDs but the
    /// same display name (`T10.3 §8.4`), renamed to `<name> (branch)`.
    pub renamed_variables: BTreeMap<String, String>,
    pub resolved_layers: usize,
    pub resolved_entities: usize,
    pub command_steps: usize,
}

/// Output of [`merge`]: a single undoable [`Command`] (`Command::Batch`),
/// the remapped IDs, and the report (`T10.3 §4`).
#[derive(Clone, Debug)]
pub struct MergePlan {
    pub command: Command,
    pub id_remap: BTreeMap<NodeId, NodeId>,
    pub report: MergeReport,
    /// Final merged document state, already verified to apply cleanly and pass
    /// reference validation.
    pub merged_document: Document,
}

/// Convenience helper: compute `diff3(base, main, branch)`, run [`merge`], and
/// return `(merged_document, plan)`.
pub fn merge_branch(base: &Document, main: &Document, branch: &Document, resolution: &Resolution) -> Result<(Document, MergePlan), MergeError> {
    let diff = diff3(base, main, branch);
    let plan = merge(base, main, branch, &diff, resolution)?;
    Ok((plan.merged_document.clone(), plan))
}

/// Compute a 3-way merge plan from `base`, `main` ("ours"), `branch` ("theirs"),
/// a precomputed `Diff3`, and caller `Resolution` (`T10.3 §4`).
pub fn merge(base: &Document, main: &Document, branch: &Document, diff: &Diff3, resolution: &Resolution) -> Result<MergePlan, MergeError> {
    // 1. Identify which entity conflicts are branch-added ID collisions that
    // `T10.3 §3.3` auto-remaps to `<key>-branch` when no manual choice is given.
    let mut auto_remap_styles = BTreeSet::new();
    let mut auto_remap_variables = BTreeSet::new();
    let mut auto_remap_collections = BTreeSet::new();

    for k in branch.styles.keys() {
        if !base.styles.contains_key(k) && main.styles.contains_key(k) {
            auto_remap_styles.insert(k.clone());
        }
    }
    for k in branch.variables.keys() {
        if !base.variables.contains_key(k) && main.variables.contains_key(k) {
            auto_remap_variables.insert(k.clone());
        }
    }
    for k in branch.variable_collections.keys() {
        if !base.variable_collections.contains_key(k) && main.variable_collections.contains_key(k) {
            auto_remap_collections.insert(k.clone());
        }
    }

    // 2. Verify every remaining conflict in `diff` has a caller resolution (`T10.3 §3.2`).
    let mut unresolved = Vec::new();
    for conflict in &diff.conflicts {
        if !resolution.per_layer.contains_key(&conflict.node) {
            unresolved.push(UnresolvedItem::Layer(conflict.node));
        }
    }
    for conflict in &diff.entity_conflicts {
        let is_resolved = match &conflict.key {
            EntityKey::Style(k) => auto_remap_styles.contains(k) || resolution.per_entity.contains_key(&conflict.key),
            EntityKey::Variable(k) => auto_remap_variables.contains(k) || resolution.per_entity.contains_key(&conflict.key),
            EntityKey::Collection(k) => auto_remap_collections.contains(k) || resolution.per_entity.contains_key(&conflict.key),
            EntityKey::Guides(page_id) => resolution.per_entity.contains_key(&conflict.key) || resolution.per_layer.contains_key(page_id),
            EntityKey::Flow(_) | EntityKey::Comment(_) | EntityKey::Image(_) => resolution.per_entity.contains_key(&conflict.key),
        };
        if !is_resolved {
            unresolved.push(UnresolvedItem::Entity(conflict.key.clone()));
        }
    }
    unresolved.sort();
    unresolved.dedup();
    if !unresolved.is_empty() {
        return Err(MergeError::Unresolved(unresolved));
    }

    // 3. Build deterministic ID remaps for branch-added nodes and entities (`T10.3 §3.3`).
    let id_remap = build_branch_node_remap(base, main, branch);
    let style_remap = build_style_remap(base, main, branch, &resolution.per_entity);
    let collection_remap = build_collection_remap(base, main, branch, &resolution.per_entity);
    let variable_remap = build_variable_remap(base, main, branch, &resolution.per_entity);

    // Clone and rewrite `branch` so all branch-added colliding IDs and references
    // already use their remapped IDs.
    let mut remapped_branch = branch.clone();
    rewrite_document_ids(&mut remapped_branch, &id_remap, &style_remap, &collection_remap, &variable_remap);

    // 4. Build the ordered `Command` list (`T10.3 §3.4`).
    let mut report = MergeReport {
        remapped_nodes: id_remap.clone(),
        remapped_styles: style_remap.clone(),
        remapped_variables: variable_remap.clone(),
        remapped_collections: collection_remap.clone(),
        renamed_variables: BTreeMap::new(),
        resolved_layers: diff.conflicts.len(),
        resolved_entities: diff.entity_conflicts.len(),
        command_steps: 0,
    };

    let commands =
        build_merge_commands(base, main, branch, &remapped_branch, diff, resolution, &id_remap, &style_remap, &collection_remap, &variable_remap, &mut report);

    // 5. Execute each command step-by-step against a working clone of `main` so
    // any invalid intermediate operation is caught with its step index (`T10.3 §4`).
    let mut working = main.clone();
    for (step, cmd) in commands.iter().enumerate() {
        let mut step_history = History::default();
        step_history.exec(&mut working, cmd.clone()).map_err(|reason| MergeError::Apply { step, reason: reason.to_string() })?;
    }
    if let Some((component, cycle_path)) = crate::components::first_new_self_holding_component_cycle(main, &remapped_branch, &working, &commands) {
        let comp_name = crate::cycle_node_label(&working, component);
        return Err(MergeError::InstanceCycle(crate::cycle_refusal_message(crate::CycleAction::Merge, &comp_name, &cycle_path)));
    }

    // 6. Validate all references in the merged document (`T10.3 §2`, `§3.7`, `§8.1`, `§8.2`).
    let dangling = validate_merged_references(&working);
    if !dangling.is_empty() {
        return Err(MergeError::DanglingReference(dangling));
    }

    if let Some(cycle_msg) = first_new_variable_alias_cycle(main, &remapped_branch, &working) {
        return Err(MergeError::AliasCycle(cycle_msg));
    }

    report.command_steps = commands.len();
    let batch = Command::Batch(commands);

    // Verify the batch executes cleanly in one shot via `History::exec`.
    let mut verify_doc = main.clone();
    let mut verify_history = History::default();
    verify_history.exec(&mut verify_doc, batch.clone()).map_err(|reason| MergeError::Apply { step: 0, reason: reason.to_string() })?;

    Ok(MergePlan { command: batch, id_remap, report, merged_document: verify_doc })
}

fn build_branch_node_remap(base: &Document, main: &Document, branch: &Document) -> BTreeMap<NodeId, NodeId> {
    let max_existing = base.nodes.keys().chain(main.nodes.keys()).chain(branch.nodes.keys()).map(|id| id.0).max().unwrap_or(0);

    let mut next_fresh = max_existing + 1;
    let mut remap = BTreeMap::new();

    // Pre-order traversal of `branch` pages + library_page + any remaining nodes
    // for deterministic fresh ID allocation (`T10.3 §3.3`).
    let mut visited = BTreeSet::new();
    for &page_id in &branch.pages {
        collect_colliding_branch_nodes(page_id, base, main, branch, &mut visited, &mut next_fresh, &mut remap);
    }
    if let Some(lib_page) = branch.library_page {
        collect_colliding_branch_nodes(lib_page, base, main, branch, &mut visited, &mut next_fresh, &mut remap);
    }
    for &id in branch.nodes.keys() {
        collect_colliding_branch_nodes(id, base, main, branch, &mut visited, &mut next_fresh, &mut remap);
    }

    remap
}

fn collect_colliding_branch_nodes(
    id: NodeId,
    base: &Document,
    main: &Document,
    branch: &Document,
    visited: &mut BTreeSet<NodeId>,
    next_fresh: &mut u64,
    remap: &mut BTreeMap<NodeId, NodeId>,
) {
    if !visited.insert(id) {
        return;
    }
    let is_branch_added = !base.nodes.contains_key(&id);
    if is_branch_added && main.nodes.contains_key(&id) {
        let fresh = NodeId(*next_fresh);
        *next_fresh += 1;
        remap.insert(id, fresh);
    }
    if let Some(node) = branch.nodes.get(&id) {
        for &child in &node.children {
            collect_colliding_branch_nodes(child, base, main, branch, visited, next_fresh, remap);
        }
    }
}

fn allocate_branch_key(raw: &str, used: &mut BTreeSet<String>) -> String {
    let base_candidate = format!("{raw}-branch");
    if used.insert(base_candidate.clone()) {
        return base_candidate;
    }
    let mut idx = 2usize;
    loop {
        let candidate = format!("{raw}-branch-{idx}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        idx += 1;
    }
}

fn build_style_remap(base: &Document, main: &Document, branch: &Document, per_entity: &BTreeMap<EntityKey, Side>) -> BTreeMap<String, String> {
    let mut used: BTreeSet<String> = base.styles.keys().chain(main.styles.keys()).chain(branch.styles.keys()).cloned().collect();
    let mut remap = BTreeMap::new();
    for k in branch.styles.keys() {
        let key = EntityKey::Style(k.clone());
        if !base.styles.contains_key(k) && main.styles.contains_key(k) && !per_entity.contains_key(&key) {
            let fresh = allocate_branch_key(k, &mut used);
            remap.insert(k.clone(), fresh);
        }
    }
    remap
}

fn build_collection_remap(base: &Document, main: &Document, branch: &Document, per_entity: &BTreeMap<EntityKey, Side>) -> BTreeMap<String, String> {
    let mut used: BTreeSet<String> =
        base.variable_collections.keys().chain(main.variable_collections.keys()).chain(branch.variable_collections.keys()).cloned().collect();
    let mut remap = BTreeMap::new();
    for k in branch.variable_collections.keys() {
        let key = EntityKey::Collection(k.clone());
        if !base.variable_collections.contains_key(k) && main.variable_collections.contains_key(k) && !per_entity.contains_key(&key) {
            let fresh = allocate_branch_key(k, &mut used);
            remap.insert(k.clone(), fresh);
        }
    }
    remap
}

fn build_variable_remap(base: &Document, main: &Document, branch: &Document, per_entity: &BTreeMap<EntityKey, Side>) -> BTreeMap<String, String> {
    let mut used: BTreeSet<String> = base.variables.keys().chain(main.variables.keys()).chain(branch.variables.keys()).cloned().collect();
    let mut remap = BTreeMap::new();
    for k in branch.variables.keys() {
        let key = EntityKey::Variable(k.clone());
        if !base.variables.contains_key(k) && main.variables.contains_key(k) && !per_entity.contains_key(&key) {
            let fresh = allocate_branch_key(k, &mut used);
            remap.insert(k.clone(), fresh);
        }
    }
    remap
}

fn remap_node_id(id: NodeId, map: &BTreeMap<NodeId, NodeId>) -> NodeId {
    map.get(&id).copied().unwrap_or(id)
}

fn remap_str(key: &str, map: &BTreeMap<String, String>) -> String {
    map.get(key).cloned().unwrap_or_else(|| key.to_owned())
}

fn col_needs_mode_remap(collections: &BTreeMap<String, VariableCollection>, col_id: &str, collection_map: &BTreeMap<String, String>) -> bool {
    let mut cur = col_id;
    for _ in 0..8 {
        if collection_map.contains_key(cur) {
            return true;
        }
        let Some(parent) = collections.get(cur).and_then(VariableCollection::extension_parent_id) else {
            break;
        };
        cur = parent;
    }
    false
}

fn remap_mode(old_col_id: &str, mode_id: &str, mode_map: &BTreeMap<(String, String), String>, flat_mode_map: &BTreeMap<String, String>) -> String {
    mode_map.get(&(old_col_id.to_owned(), mode_id.to_owned())).or_else(|| flat_mode_map.get(mode_id)).cloned().unwrap_or_else(|| mode_id.to_owned())
}

fn remap_mode_key(old_col_id: &str, mode_key: &str, mode_map: &BTreeMap<(String, String), String>, flat_mode_map: &BTreeMap<String, String>) -> String {
    if mode_key.contains('+') {
        mode_key.split('+').map(|part| remap_mode(old_col_id, part, mode_map, flat_mode_map)).collect::<Vec<_>>().join("+")
    } else {
        remap_mode(old_col_id, mode_key, mode_map, flat_mode_map)
    }
}

fn first_new_variable_alias_cycle(main: &Document, remapped_branch: &Document, merged: &Document) -> Option<String> {
    let main_store = crate::VariableStore::from_doc(main);
    let branch_store = crate::VariableStore::from_doc(remapped_branch);
    let merged_store = crate::VariableStore::from_doc(merged);

    let mut before = main_store.unresolved_pairs();
    before.extend(branch_store.unresolved_pairs());
    let after = merged_store.unresolved_pairs();
    if let Some((var_id, mode_id)) = after.difference(&before).next() {
        let var_name = merged.variables.get(var_id).map(|v| v.name.as_str()).unwrap_or(var_id.as_str());
        return Some(format!("ALIAS_CYCLE: cannot merge branch because variable '{var_name}' ({var_id}) in mode '{mode_id}' closes an alias cycle"));
    }

    for (var_id, var) in &merged_store.variables {
        for val in var.values.values() {
            let VariableValue::Alias(target) = val else { continue };
            if !merged_store.variables.contains_key(target) {
                continue;
            }
            let pre_existing_in = |s: &crate::VariableStore| {
                s.variables
                    .get(var_id)
                    .is_some_and(|v| v.values.values().any(|x| matches!(x, VariableValue::Alias(t) if t == target)) && !s.can_alias(var_id, target))
            };
            if !merged_store.can_alias(var_id, target) && !pre_existing_in(&main_store) && !pre_existing_in(&branch_store) {
                return Some(format!("ALIAS_CYCLE: cannot merge branch because variable '{}' ({var_id}) closes an alias cycle with '{target}'", var.name));
            }
        }
    }
    None
}

/// Rewrite all node, style, collection, and variable IDs and references inside a document (`T10.3 §2`, `§3.3`).
fn rewrite_document_ids(
    doc: &mut Document,
    node_map: &BTreeMap<NodeId, NodeId>,
    style_map: &BTreeMap<String, String>,
    collection_map: &BTreeMap<String, String>,
    variable_map: &BTreeMap<String, String>,
) {
    if node_map.is_empty() && style_map.is_empty() && collection_map.is_empty() && variable_map.is_empty() {
        return;
    }

    let mut used_modes: BTreeSet<String> = doc.variable_collections.values().flat_map(|col| col.modes.iter().map(|m| m.id.clone())).collect();
    let mut mode_map: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut flat_mode_map: BTreeMap<String, String> = BTreeMap::new();
    for (old_col_id, col) in &doc.variable_collections {
        if col_needs_mode_remap(&doc.variable_collections, old_col_id, collection_map) {
            for mode in &col.modes {
                let fresh_mode = allocate_branch_key(&mode.id, &mut used_modes);
                mode_map.insert((old_col_id.clone(), mode.id.clone()), fresh_mode.clone());
                flat_mode_map.insert(mode.id.clone(), fresh_mode);
            }
        }
    }

    doc.pages = doc.pages.iter().map(|&id| remap_node_id(id, node_map)).collect();
    doc.library_page = doc.library_page.map(|p| remap_node_id(p, node_map));

    let old_nodes = std::mem::take(&mut doc.nodes);
    for (old_id, mut node) in old_nodes {
        let new_id = remap_node_id(old_id, node_map);
        rewrite_node_references(&mut node, node_map, style_map, collection_map, variable_map, &mode_map, &flat_mode_map);
        node.id = new_id;
        doc.nodes.insert(new_id, node);
    }

    let old_styles = std::mem::take(&mut doc.styles);
    for (old_key, style) in old_styles {
        let new_key = remap_str(&old_key, style_map);
        doc.styles.insert(new_key, style);
    }
    let old_style_meta = std::mem::take(&mut doc.style_meta);
    for (old_key, mut meta) in old_style_meta {
        let new_key = remap_str(&old_key, style_map);
        for var_id in meta.bindings.values_mut() {
            *var_id = remap_str(var_id, variable_map);
        }
        doc.style_meta.insert(new_key, meta);
    }

    let old_collections = std::mem::take(&mut doc.variable_collections);
    for (old_id, mut col) in old_collections {
        let new_id = remap_str(&old_id, collection_map);
        col.id = new_id.clone();
        col.variables = col.variables.into_iter().map(|v| remap_str(&v, variable_map)).collect();
        for mode in &mut col.modes {
            mode.id = remap_mode(&old_id, &mode.id, &mode_map, &flat_mode_map);
        }
        col.default_mode = remap_mode(&old_id, &col.default_mode, &mode_map, &flat_mode_map);
        if let Some(mut ext) = col.extension() {
            let old_parent_id = ext.parent.collection.clone();
            if !ext.parent.collection.is_empty() {
                ext.parent.collection = remap_str(&ext.parent.collection, collection_map);
            }
            ext.mode_map = ext
                .mode_map
                .into_iter()
                .map(|(own_m, parent_m)| {
                    (remap_mode(&old_id, &own_m, &mode_map, &flat_mode_map), remap_mode(&old_parent_id, &parent_m, &mode_map, &flat_mode_map))
                })
                .collect();
            ext.overrides = ext
                .overrides
                .into_iter()
                .map(|(var_id, by_mode)| {
                    let new_var_id = remap_str(&var_id, variable_map);
                    let new_by_mode = by_mode
                        .into_iter()
                        .map(|(m, mut val)| {
                            if let VariableValue::Alias(target) = &mut val {
                                *target = remap_str(target, variable_map);
                            }
                            (remap_mode(&old_id, &m, &mode_map, &flat_mode_map), val)
                        })
                        .collect();
                    (new_var_id, new_by_mode)
                })
                .collect();
            col.set_extension(&ext);
        }
        doc.variable_collections.insert(new_id, col);
    }

    let old_variables = std::mem::take(&mut doc.variables);
    for (old_id, mut var) in old_variables {
        let old_col_id = var.collection.clone();
        let new_id = remap_str(&old_id, variable_map);
        var.id = new_id.clone();
        var.collection = remap_str(&old_col_id, collection_map);
        let old_values = std::mem::take(&mut var.values);
        for (mode_key, mut val) in old_values {
            if let VariableValue::Alias(target) = &mut val {
                *target = remap_str(target, variable_map);
            }
            let new_mode_key = remap_mode_key(&old_col_id, &mode_key, &mode_map, &flat_mode_map);
            var.values.insert(new_mode_key, val);
        }
        let old_mode_ext = std::mem::take(&mut var.mode_extensions);
        for (mode_key, ext_val) in old_mode_ext {
            let new_mode_key = remap_mode_key(&old_col_id, &mode_key, &mode_map, &flat_mode_map);
            var.mode_extensions.insert(new_mode_key, ext_val);
        }
        doc.variables.insert(new_id, var);
    }

    for flow in &mut doc.flows {
        flow.page = remap_node_id(flow.page, node_map);
        flow.start = remap_node_id(flow.start, node_map);
    }

    let old_guides = std::mem::take(&mut doc.guides);
    for (old_page, mut guides) in old_guides {
        let new_page = remap_node_id(old_page, node_map);
        for guide in &mut guides {
            guide.frame = guide.frame.map(|f| remap_node_id(f, node_map));
        }
        doc.guides.insert(new_page, guides);
    }

    for thread in &mut doc.comments {
        thread.anchor = CommentAnchor {
            page: remap_node_id(thread.anchor.page, node_map),
            node: thread.anchor.node.map(|n| remap_node_id(n, node_map)),
            offset: thread.anchor.offset,
        };
    }

    for asset in doc.library.external.values_mut() {
        asset.node = asset.node.map(|n| remap_node_id(n, node_map));
        if let Some(local_key) = &mut asset.local_key {
            match asset.kind {
                LibraryAssetKind::Style => *local_key = remap_str(local_key, style_map),
                LibraryAssetKind::Variable => *local_key = remap_str(local_key, variable_map),
                LibraryAssetKind::VariableCollection => *local_key = remap_str(local_key, collection_map),
                LibraryAssetKind::Component | LibraryAssetKind::ComponentSet => {}
            }
        }
    }

    doc.bump_ids();
}

fn rewrite_node_references(
    node: &mut Node,
    node_map: &BTreeMap<NodeId, NodeId>,
    style_map: &BTreeMap<String, String>,
    collection_map: &BTreeMap<String, String>,
    variable_map: &BTreeMap<String, String>,
    mode_map: &BTreeMap<(String, String), String>,
    flat_mode_map: &BTreeMap<String, String>,
) {
    node.id = remap_node_id(node.id, node_map);
    node.parent = node.parent.map(|p| remap_node_id(p, node_map));
    for child in &mut node.children {
        *child = remap_node_id(*child, node_map);
    }

    match &mut node.kind {
        NodeKind::Instance { component, .. } => {
            *component = component.map(|c| remap_node_id(c, node_map));
        }
        NodeKind::Connector(conn) => {
            conn.start.node = conn.start.node.map(|n| remap_node_id(n, node_map));
            conn.end.node = conn.end.node.map(|n| remap_node_id(n, node_map));
        }
        _ => {}
    }

    rewrite_component_link(&mut node.component, node_map);

    // Rewrite `node.variables.bindings` and `node.variables.modes`.
    let old_bindings = std::mem::take(&mut node.variables.bindings);
    for (prop, var_id) in old_bindings {
        node.variables.bindings.insert(prop, remap_str(&var_id, variable_map));
    }
    let old_prop_defaults = std::mem::take(&mut node.component.prop_default_variables);
    for (prop, var_id) in old_prop_defaults {
        node.component.prop_default_variables.insert(prop, remap_str(&var_id, variable_map));
    }
    let old_modes = std::mem::take(&mut node.variables.modes);
    for (col_id, mode_id) in old_modes {
        let new_mode_id = remap_mode(&col_id, &mode_id, mode_map, flat_mode_map);
        node.variables.modes.insert(remap_str(&col_id, collection_map), new_mode_id);
    }

    // Rewrite `node.style_refs`.
    for style_key in node.style_refs.values_mut() {
        *style_key = remap_str(style_key, style_map);
    }

    // Rewrite `node.prototype`.
    for fixed in &mut node.prototype.fixed_children {
        *fixed = remap_node_id(*fixed, node_map);
    }
    for reaction in &mut node.prototype.reactions {
        for action in &mut reaction.actions {
            rewrite_action(action, node_map, collection_map, variable_map, mode_map, flat_mode_map);
        }
    }
}

fn rewrite_component_link(link: &mut ComponentLink, node_map: &BTreeMap<NodeId, NodeId>) {
    link.source = link.source.map(|s| remap_node_id(s, node_map));
    for prop in &mut link.props {
        rewrite_prop_value(&mut prop.default, node_map);
        if let PropKind::InstanceSwap { preferred } = &mut prop.kind {
            for p in preferred {
                *p = remap_node_id(*p, node_map);
            }
        }
    }
    for val in link.values.values_mut() {
        rewrite_prop_value(val, node_map);
    }
    for cfg in link.slot_configs.values_mut() {
        for p in &mut cfg.preferred {
            *p = remap_node_id(*p, node_map);
        }
    }
}

fn rewrite_prop_value(val: &mut PropValue, node_map: &BTreeMap<NodeId, NodeId>) {
    match val {
        PropValue::Instance(id) => *id = remap_node_id(*id, node_map),
        PropValue::Slot(ids) => {
            for id in ids {
                *id = remap_node_id(*id, node_map);
            }
        }
        PropValue::Bool(_) | PropValue::Text(_) | PropValue::Variant(_) => {}
    }
}

fn rewrite_action(
    action: &mut Action,
    node_map: &BTreeMap<NodeId, NodeId>,
    collection_map: &BTreeMap<String, String>,
    variable_map: &BTreeMap<String, String>,
    mode_map: &BTreeMap<(String, String), String>,
    flat_mode_map: &BTreeMap<String, String>,
) {
    match action {
        Action::Navigate { dest, .. }
        | Action::OpenOverlay { dest, .. }
        | Action::SwapOverlay { dest }
        | Action::ScrollTo { dest, .. }
        | Action::ChangeTo { dest }
        | Action::MediaControl { dest, .. } => {
            *dest = remap_node_id(*dest, node_map);
        }
        Action::SetVariable { variable, value } => {
            *variable = remap_str(variable, variable_map);
            rewrite_expression(value, variable_map);
        }
        Action::SetVariableMode { collection, mode } => {
            *mode = remap_mode(collection, mode, mode_map, flat_mode_map);
            *collection = remap_str(collection, collection_map);
        }
        Action::Conditional { branches, otherwise } => {
            for (cond, actions) in branches {
                rewrite_expression(cond, variable_map);
                for a in actions {
                    rewrite_action(a, node_map, collection_map, variable_map, mode_map, flat_mode_map);
                }
            }
            for a in otherwise {
                rewrite_action(a, node_map, collection_map, variable_map, mode_map, flat_mode_map);
            }
        }
        Action::CloseOverlay | Action::Back | Action::OpenUrl { .. } => {}
    }
}

fn rewrite_expression(expr: &mut Expression, variable_map: &BTreeMap<String, String>) {
    match expr {
        Expression::Literal(_) => {}
        Expression::Variable(var_id) => {
            *var_id = remap_str(var_id, variable_map);
        }
        Expression::Unary { arg, .. } => rewrite_expression(arg, variable_map),
        Expression::Binary { lhs, rhs, .. } => {
            rewrite_expression(lhs, variable_map);
            rewrite_expression(rhs, variable_map);
        }
        Expression::Ternary { cond, then, otherwise } => {
            rewrite_expression(cond, variable_map);
            rewrite_expression(then, variable_map);
            rewrite_expression(otherwise, variable_map);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_merge_commands(
    base: &Document,
    main: &Document,
    raw_branch: &Document,
    remapped_branch: &Document,
    diff: &Diff3,
    resolution: &Resolution,
    id_remap: &BTreeMap<NodeId, NodeId>,
    style_remap: &BTreeMap<String, String>,
    collection_remap: &BTreeMap<String, String>,
    variable_remap: &BTreeMap<String, String>,
    report: &mut MergeReport,
) -> Vec<Command> {
    let mut entity_cmds = Vec::new();
    let mut remove_cmds = Vec::new();
    let mut add_cmds = Vec::new();
    let mut move_cmds = Vec::new();
    let mut update_cmds = Vec::new();
    let mut flow_guide_comment_cmds = Vec::new();

    // --- Step 1: Entity changes (collections, variables, styles, library state, images) ---
    let (merged_variables, merged_collections) = merge_variables_and_collections(
        base,
        main,
        raw_branch,
        remapped_branch,
        diff,
        resolution,
        collection_remap,
        variable_remap,
        &mut report.renamed_variables,
    );
    if merged_variables != main.variables || merged_collections != main.variable_collections {
        entity_cmds.push(Command::SetVariables { variables: merged_variables, collections: merged_collections });
    }

    let (merged_styles, merged_style_meta) = merge_styles(base, main, raw_branch, remapped_branch, diff, resolution, style_remap);
    if merged_styles != main.styles || merged_style_meta != main.style_meta {
        entity_cmds.push(Command::SetStyles { styles: merged_styles, style_meta: merged_style_meta });
    }

    // Merge embedded images (`Command::SetImages`).
    let mut add_images = BTreeMap::new();
    for (hash, bytes) in &remapped_branch.images {
        if !main.images.contains_key(hash) {
            let key = EntityKey::Image(hash.clone());
            let is_conflicting = diff.entity_conflicts.iter().any(|c| c.key == key);
            let take_branch = if is_conflicting { resolution.per_entity.get(&key) == Some(&Side::Branch) } else { !base.images.contains_key(hash) };
            if take_branch {
                add_images.insert(hash.clone(), bytes.clone());
            }
        }
    }
    let mut remove_images = Vec::new();
    for hash in base.images.keys() {
        if !remapped_branch.images.contains_key(hash) && main.images.contains_key(hash) {
            let key = EntityKey::Image(hash.clone());
            let is_conflicting = diff.entity_conflicts.iter().any(|c| c.key == key);
            if !is_conflicting || resolution.per_entity.get(&key) == Some(&Side::Branch) {
                remove_images.push(hash.clone());
            }
        }
    }
    if !add_images.is_empty() || !remove_images.is_empty() {
        entity_cmds.push(Command::SetImages { add: add_images, remove: remove_images });
    }

    // --- Determine winning node survival, additions, moves, and group updates ---
    let conflict_nodes: BTreeMap<NodeId, &Conflict> = diff.conflicts.iter().map(|c| (c.node, c)).collect();

    // Nodes to remove from `main`:
    // 1. Removed on branch without conflict
    // 2. Involved in a `DeleteModify` conflict where `Side::Branch` (which removed it) was chosen
    // 3. Involved in a `DependsOnRemoved` conflict on a main-added node where `Side::Branch` was chosen
    let mut nodes_to_remove = BTreeSet::new();
    for (&id, change) in &diff.theirs.nodes {
        if change.is_removed() {
            if let Some(conflict) = conflict_nodes.get(&id) {
                if matches!(conflict.kind, ConflictKind::DeleteModify) && resolution.per_layer.get(&id) == Some(&Side::Branch) {
                    nodes_to_remove.insert(id);
                }
            } else if main.nodes.contains_key(&id) {
                nodes_to_remove.insert(id);
            }
        }
    }
    for (&id, conflict) in &conflict_nodes {
        if matches!(conflict.kind, ConflictKind::DependsOnRemoved)
            && main.nodes.contains_key(&id)
            && !base.nodes.contains_key(&id)
            && resolution.per_layer.get(&id) == Some(&Side::Branch)
        {
            nodes_to_remove.insert(id);
        }
    }

    // Nodes that `main` removed, `branch` modified (`DeleteModify`), and caller chose `Side::Branch`:
    // these must be re-added from `remapped_branch`!
    let mut branch_resurrected_nodes = BTreeSet::new();
    for (&id, conflict) in &conflict_nodes {
        if matches!(conflict.kind, ConflictKind::DeleteModify)
            && !main.nodes.contains_key(&id)
            && remapped_branch.nodes.contains_key(&id)
            && resolution.per_layer.get(&id) == Some(&Side::Branch)
        {
            collect_branch_subtree(id, remapped_branch, &mut branch_resurrected_nodes);
        }
    }

    // Expand `nodes_to_remove` to include descendants in `main` so we know the full removed set,
    // while only emitting top-level `RemovePage` / `Remove` commands (`Command::Remove` removes descendants).
    let mut all_removed_in_main = BTreeSet::new();
    for &id in &nodes_to_remove {
        collect_main_subtree(id, main, &mut all_removed_in_main);
    }

    for &id in &nodes_to_remove {
        if let Some(node) = main.nodes.get(&id) {
            if let Some(parent) = node.parent {
                if all_removed_in_main.contains(&parent) {
                    continue;
                }
            }
            if matches!(node.kind, NodeKind::Page { .. }) && main.pages.contains(&id) {
                remove_cmds.push(Command::RemovePage { id });
            } else if node.parent.is_some() {
                remove_cmds.push(Command::Remove { id });
            }
        }
    }

    // --- Step 3: Node adds, parents before children ---
    let mut nodes_to_add = BTreeSet::new();
    let mut skipped_branch_subtrees = BTreeSet::new();
    for (&raw_id, conflict) in &conflict_nodes {
        if matches!(conflict.kind, ConflictKind::DependsOnRemoved)
            && !base.nodes.contains_key(&raw_id)
            && raw_branch.nodes.contains_key(&raw_id)
            && resolution.per_layer.get(&raw_id) == Some(&Side::Main)
        {
            let mapped = remap_node_id(raw_id, id_remap);
            collect_branch_subtree(mapped, remapped_branch, &mut skipped_branch_subtrees);
        }
    }

    for (&raw_id, change) in &diff.theirs.nodes {
        if change.is_added() {
            let mapped_id = remap_node_id(raw_id, id_remap);
            if !skipped_branch_subtrees.contains(&mapped_id) && !main.nodes.contains_key(&mapped_id) {
                nodes_to_add.insert(mapped_id);
            }
        }
    }
    for id in branch_resurrected_nodes {
        if !main.nodes.contains_key(&id) {
            nodes_to_add.insert(id);
        }
    }

    // Order `nodes_to_add` in pre-order (parents before children) following `remapped_branch`.
    let ordered_adds = order_branch_nodes_topologically(remapped_branch, &nodes_to_add);
    for id in ordered_adds {
        let Some(branch_node) = remapped_branch.nodes.get(&id) else {
            continue;
        };
        if matches!(branch_node.kind, NodeKind::Page { .. }) && remapped_branch.pages.contains(&id) {
            let page_index = remapped_branch.pages.iter().position(|&p| p == id).unwrap_or(main.pages.len());
            let mut page_node = branch_node.clone();
            page_node.parent = None;
            page_node.children.clear();
            add_cmds.push(Command::InsertPage { index: page_index, nodes: vec![page_node] });
        } else if let Some(parent_id) = branch_node.parent {
            let parent_exists = (main.nodes.contains_key(&parent_id) && !all_removed_in_main.contains(&parent_id)) || nodes_to_add.contains(&parent_id);
            if !parent_exists {
                continue;
            }
            let desired_index = remapped_branch.nodes.get(&parent_id).and_then(|p| p.children.iter().position(|&c| c == id)).unwrap_or(usize::MAX);
            let mut single_node = branch_node.clone();
            single_node.children.clear();
            add_cmds.push(Command::Insert { parent: parent_id, index: desired_index, nodes: vec![single_node] });
        }
    }

    // --- Step 4: Node moves (`Command::Reparent`) ---
    for &id in base.nodes.keys() {
        if all_removed_in_main.contains(&id) || !main.nodes.contains_key(&id) {
            continue;
        }
        let Some(main_node) = main.nodes.get(&id) else {
            continue;
        };
        let Some(branch_node) = remapped_branch.nodes.get(&id) else {
            continue;
        };
        if matches!(main_node.kind, NodeKind::Page { .. }) {
            continue;
        }

        let main_moved = diff.ours.nodes.get(&id).is_some_and(NodeChange::is_moved);
        let branch_moved = diff.theirs.nodes.get(&id).is_some_and(NodeChange::is_moved);
        let pos_conflict = conflict_nodes.get(&id).is_some_and(|c| c.groups.contains(&Group::Position));

        let take_branch_move = if pos_conflict { resolution.per_layer.get(&id) == Some(&Side::Branch) } else { branch_moved && !main_moved };

        if take_branch_move {
            if let Some(new_parent) = branch_node.parent {
                let parent_valid = (main.nodes.contains_key(&new_parent) && !all_removed_in_main.contains(&new_parent)) || nodes_to_add.contains(&new_parent);
                if parent_valid {
                    let new_index = remapped_branch.nodes.get(&new_parent).and_then(|p| p.children.iter().position(|&c| c == id)).unwrap_or(0);
                    move_cmds.push(Command::Reparent { id, parent: new_parent, index: new_index });
                }
            }
        }
    }

    // --- Step 5: Node property changes (`Command::Update`) ---
    // For every surviving node in `main`, merge non-conflicting groups from both sides
    // and conflicting groups from the chosen side for that layer (`T10.3 §3.2`).
    for (&id, main_node) in &main.nodes {
        if all_removed_in_main.contains(&id) {
            continue;
        }
        let Some(branch_node) = remapped_branch.nodes.get(&id) else {
            continue;
        };
        if !base.nodes.contains_key(&id) {
            continue;
        }

        let main_groups = modified_groups(&diff.ours, id);
        let branch_groups = modified_groups(&diff.theirs, id);
        if branch_groups.is_empty() {
            continue;
        }

        let layer_side = resolution.per_layer.get(&id).copied();
        let mut merged_node = main_node.clone();

        for &group in &branch_groups {
            if group == Group::Position {
                continue;
            }
            let in_main = main_groups.contains(&group);
            let apply_branch = if in_main {
                // If both sides changed `group` to the exact same value, `branch` is identical to `main`.
                // If they differed, `diff.conflicts` recorded the conflict and `layer_side` decides.
                layer_side == Some(Side::Branch)
            } else {
                true
            };
            if apply_branch {
                copy_node_group(&mut merged_node, branch_node, group);
            }
        }

        merged_node.parent = main_node.parent;
        merged_node.children = main_node.children.clone();

        if &merged_node != main_node {
            update_cmds.push(Command::Update { node: Box::new(merged_node) });
        }
    }

    // Also merge `Document.library` and `Document.library_page` (`Command::SetLibraryState`).
    let (merged_library, merged_lib_page) = merge_library_state(base, main, remapped_branch, &all_removed_in_main, &nodes_to_add);
    if merged_library != main.library || merged_lib_page != main.library_page {
        entity_cmds.push(Command::SetLibraryState { state: merged_library, library_page: merged_lib_page });
    }

    // --- Step 6: Flows, guides, and comments ---
    let merged_flows = merge_flows(base, main, raw_branch, remapped_branch, diff, resolution);
    if merged_flows != main.flows {
        flow_guide_comment_cmds.push(Command::SetFlows { flows: merged_flows });
    }

    let merged_guides = merge_guides(base, main, remapped_branch, diff, id_remap, &all_removed_in_main, &nodes_to_add, resolution);
    if merged_guides != main.guides {
        let all_guide_pages: BTreeSet<NodeId> = main.guides.keys().chain(merged_guides.keys()).copied().collect();
        for page_id in all_guide_pages {
            let before_g = main.guides.get(&page_id);
            let after_g = merged_guides.get(&page_id);
            if before_g != after_g {
                flow_guide_comment_cmds.push(Command::SetGuides { page: page_id, guides: after_g.cloned().unwrap_or_default() });
            }
        }
    }

    let merged_comments = merge_comments(base, main, raw_branch, remapped_branch, diff, resolution);
    if merged_comments != main.comments {
        flow_guide_comment_cmds.push(Command::SetComments { comments: merged_comments });
    }

    let mut all_cmds =
        Vec::with_capacity(entity_cmds.len() + remove_cmds.len() + add_cmds.len() + move_cmds.len() + update_cmds.len() + flow_guide_comment_cmds.len());
    all_cmds.extend(entity_cmds);
    all_cmds.extend(remove_cmds);
    all_cmds.extend(add_cmds);
    all_cmds.extend(move_cmds);
    all_cmds.extend(update_cmds);
    all_cmds.extend(flow_guide_comment_cmds);
    all_cmds
}

fn collect_main_subtree(id: NodeId, main: &Document, out: &mut BTreeSet<NodeId>) {
    if !out.insert(id) {
        return;
    }
    if let Some(node) = main.nodes.get(&id) {
        for &child in &node.children {
            collect_main_subtree(child, main, out);
        }
    }
}

fn collect_branch_subtree(id: NodeId, branch: &Document, out: &mut BTreeSet<NodeId>) {
    if !out.insert(id) {
        return;
    }
    if let Some(node) = branch.nodes.get(&id) {
        for &child in &node.children {
            collect_branch_subtree(child, branch, out);
        }
    }
}

fn order_branch_nodes_topologically(branch: &Document, nodes_to_add: &BTreeSet<NodeId>) -> Vec<NodeId> {
    let mut ordered = Vec::with_capacity(nodes_to_add.len());
    let mut visited = BTreeSet::new();

    for &page_id in &branch.pages {
        visit_branch_add_preorder(page_id, branch, nodes_to_add, &mut visited, &mut ordered);
    }
    if let Some(lib_page) = branch.library_page {
        visit_branch_add_preorder(lib_page, branch, nodes_to_add, &mut visited, &mut ordered);
    }
    for &id in nodes_to_add {
        visit_branch_add_preorder(id, branch, nodes_to_add, &mut visited, &mut ordered);
    }
    ordered
}

fn visit_branch_add_preorder(id: NodeId, branch: &Document, nodes_to_add: &BTreeSet<NodeId>, visited: &mut BTreeSet<NodeId>, ordered: &mut Vec<NodeId>) {
    if !visited.insert(id) {
        return;
    }
    if let Some(node) = branch.nodes.get(&id) {
        if let Some(parent) = node.parent {
            if nodes_to_add.contains(&parent) && !visited.contains(&parent) {
                visit_branch_add_preorder(parent, branch, nodes_to_add, visited, ordered);
            }
        }
        if nodes_to_add.contains(&id) {
            ordered.push(id);
        }
        for &child in &node.children {
            visit_branch_add_preorder(child, branch, nodes_to_add, visited, ordered);
        }
    }
}

fn modified_groups(side: &SideChanges, id: NodeId) -> BTreeSet<Group> {
    side.nodes.get(&id).and_then(NodeChange::groups).cloned().unwrap_or_default()
}

fn copy_node_group(dst: &mut Node, src: &Node, group: Group) {
    match group {
        Group::Name => dst.name = src.name.clone(),
        Group::Visible => dst.visible = src.visible,
        Group::Locked => dst.locked = src.locked,
        Group::Opacity => dst.opacity = src.opacity,
        Group::Blend => dst.blend = src.blend,
        Group::Transform => dst.transform = src.transform,
        Group::Size => dst.size = src.size,
        Group::Fills => dst.fills = src.fills.clone(),
        Group::Stroke => dst.stroke = src.stroke.clone(),
        Group::Effects => dst.effects = src.effects.clone(),
        Group::CornerRadii => {
            dst.corner_radii = src.corner_radii;
            dst.corner_smoothing = src.corner_smoothing;
        }
        Group::Constraints => dst.constraints = src.constraints,
        Group::LayoutChild => dst.layout_child = src.layout_child,
        Group::Mask => {
            dst.is_mask = src.is_mask;
            dst.mask_type = src.mask_type;
        }
        Group::StyleRefs => {
            dst.style_refs = src.style_refs.clone();
            dst.text_style_overrides = src.text_style_overrides;
        }
        Group::Variables => dst.variables = src.variables.clone(),
        Group::Component => {
            dst.component = src.component.clone();
            if let (NodeKind::Instance { component: dc, component_key: dk, .. }, NodeKind::Instance { component: sc, component_key: sk, .. }) =
                (&mut dst.kind, &src.kind)
            {
                *dc = *sc;
                *dk = sk.clone();
            }
        }
        Group::Prototype => dst.prototype = src.prototype.clone(),
        Group::Dev => dst.dev = src.dev.clone(),
        Group::ExportSettings => dst.export_settings = src.export_settings.clone(),
        Group::Kind => copy_kind_group(&mut dst.kind, &src.kind),
        Group::TextContent => {
            if let (Some(dt), Some(st)) = (dst.kind.any_text_mut(), src.kind.any_text()) {
                dt.content = st.content.clone();
            }
        }
        Group::TextStyle => {
            if let (Some(dt), Some(st)) = (dst.kind.any_text_mut(), src.kind.any_text()) {
                let preserved_content = dt.content.clone();
                *dt = st.clone();
                dt.content = preserved_content;
            }
        }
        Group::Frame => {
            if let (Some(df), Some(sf)) = (dst.kind.frame_props_mut(), src.kind.frame_props()) {
                *df = sf.clone();
            }
        }
        Group::Vector => {
            dst.fill_geometry = src.fill_geometry.clone();
            dst.stroke_geometry = src.stroke_geometry.clone();
            if let (NodeKind::Vector { paths: dp, network: dn }, NodeKind::Vector { paths: sp, network: sn }) = (&mut dst.kind, &src.kind) {
                *dp = sp.clone();
                *dn = sn.clone();
            }
        }
        Group::PluginData => dst.plugin_data = src.plugin_data.clone(),
        Group::Other => {
            dst.source_id = src.source_id.clone();
            dst.slide = src.slide.clone();
            dst.lock_aspect = src.lock_aspect;
            dst.hyperlink = src.hyperlink.clone();
            dst.detached_symbol_id = src.detached_symbol_id.clone();
            dst.fig_metadata = src.fig_metadata.clone();
            dst.motion = src.motion.clone();
        }
        Group::Position => {}
    }
}

fn copy_kind_group(dst: &mut NodeKind, src: &NodeKind) {
    // Preserve existing FrameProps when converting between Frame/Component/ComponentSet/Instance
    // so `Group::Kind` and `Group::Frame` stay orthogonal (`T10.2 §8.1`).
    let existing_frame = dst.frame_props().cloned();
    let existing_text = dst.any_text().cloned();

    let mut next = src.clone();
    if let (Some(ef), Some(nf)) = (existing_frame, next.frame_props_mut()) {
        *nf = ef;
    }
    if let (Some(et), Some(nt)) = (existing_text, next.any_text_mut()) {
        *nt = et;
    }
    *dst = next;
}

#[allow(clippy::too_many_arguments)]
fn merge_variables_and_collections(
    base: &Document,
    main: &Document,
    raw_branch: &Document,
    remapped_branch: &Document,
    diff: &Diff3,
    resolution: &Resolution,
    collection_remap: &BTreeMap<String, String>,
    variable_remap: &BTreeMap<String, String>,
    renamed_variables: &mut BTreeMap<String, String>,
) -> (BTreeMap<String, Variable>, BTreeMap<String, VariableCollection>) {
    let mut out_vars = main.variables.clone();
    let mut out_cols = main.variable_collections.clone();

    let conflict_keys: BTreeSet<&EntityKey> = diff.entity_conflicts.iter().map(|c| &c.key).collect();

    // 1. Merge collections.
    for (raw_col_id, change) in &diff.theirs.collections {
        let entity_key = EntityKey::Collection(raw_col_id.clone());
        let mapped_col_id = remap_str(raw_col_id, collection_remap);
        if mapped_col_id != *raw_col_id {
            if let Some(col) = remapped_branch.variable_collections.get(&mapped_col_id) {
                out_cols.insert(mapped_col_id, col.clone());
            }
            continue;
        }

        let take_branch = if conflict_keys.contains(&entity_key) { resolution.per_entity.get(&entity_key) == Some(&Side::Branch) } else { true };
        if !take_branch {
            continue;
        }

        match change {
            EntityChange::Added | EntityChange::Changed => {
                if let Some(col) = remapped_branch.variable_collections.get(&mapped_col_id) {
                    out_cols.insert(mapped_col_id, col.clone());
                }
            }
            EntityChange::Removed => {
                out_cols.remove(&mapped_col_id);
            }
        }
    }

    // 2. Merge variables (including non-conflicting per-mode edits from both sides, `T10.2 §8.6`).
    for (raw_var_id, change) in &diff.theirs.variables {
        let entity_key = EntityKey::Variable(raw_var_id.clone());
        let mapped_var_id = remap_str(raw_var_id, variable_remap);
        if mapped_var_id != *raw_var_id {
            if let Some(var) = remapped_branch.variables.get(&mapped_var_id) {
                out_vars.insert(mapped_var_id.clone(), var.clone());
                if let Some(col) = out_cols.get_mut(&var.collection) {
                    if !col.variables.contains(&mapped_var_id) {
                        col.variables.push(mapped_var_id);
                    }
                }
            }
            continue;
        }

        let is_conflicting = conflict_keys.contains(&entity_key);
        let side_choice = resolution.per_entity.get(&entity_key).copied();

        match change {
            EntityChange::Added => {
                if !is_conflicting || side_choice == Some(Side::Branch) {
                    if let Some(var) = remapped_branch.variables.get(&mapped_var_id) {
                        out_vars.insert(mapped_var_id.clone(), var.clone());
                        if let Some(col) = out_cols.get_mut(&var.collection) {
                            if !col.variables.contains(&mapped_var_id) {
                                col.variables.push(mapped_var_id);
                            }
                        }
                    }
                }
            }
            EntityChange::Removed => {
                if !is_conflicting || side_choice == Some(Side::Branch) {
                    out_vars.remove(&mapped_var_id);
                    for col in out_cols.values_mut() {
                        col.variables.retain(|v| v != &mapped_var_id);
                    }
                }
            }
            EntityChange::Changed => {
                if let (Some(base_v), Some(main_v), Some(branch_v)) =
                    (base.variables.get(raw_var_id), main.variables.get(&mapped_var_id), remapped_branch.variables.get(&mapped_var_id))
                {
                    let prefer_branch = !is_conflicting || side_choice == Some(Side::Branch);
                    let merged_v = merge_single_variable(base_v, main_v, branch_v, prefer_branch);
                    out_vars.insert(mapped_var_id, merged_v);
                } else if !is_conflicting || side_choice == Some(Side::Branch) {
                    if let Some(branch_v) = remapped_branch.variables.get(&mapped_var_id) {
                        out_vars.insert(mapped_var_id, branch_v.clone());
                    }
                }
            }
        }
    }

    // 3. `T10.3 §8.4`: Both sides add a variable with the same name (and different IDs).
    // Keep both, rename the branch's variable to `<name> (branch)` and record in report.
    let main_added_names: BTreeMap<(String, String), String> =
        main.variables.iter().filter(|(id, _)| !base.variables.contains_key(*id)).map(|(id, v)| ((v.collection.clone(), v.name.clone()), id.clone())).collect();

    for (raw_id, _) in raw_branch.variables.iter().filter(|(id, _)| !base.variables.contains_key(*id)) {
        let mapped_id = remap_str(raw_id, variable_remap);
        let Some(var) = out_vars.get(&mapped_id).cloned() else {
            continue;
        };
        if let Some(main_var_id) = main_added_names.get(&(var.collection.clone(), var.name.clone())) {
            if main_var_id != &mapped_id {
                let new_name = format!("{} (branch)", var.name);
                if let Some(entry) = out_vars.get_mut(&mapped_id) {
                    entry.name = new_name.clone();
                }
                renamed_variables.insert(mapped_id, new_name);
            }
        }
    }

    let extended_parents: BTreeSet<String> = out_cols.values().filter_map(|c| c.extension_parent_id().map(str::to_owned)).collect();
    for parent_id in extended_parents {
        if let Some(parent_col) = out_cols.get_mut(&parent_id) {
            if !parent_col.is_library() {
                parent_col.set_extendable(true);
            }
        }
    }

    (out_vars, out_cols)
}

fn merge_single_variable(base: &Variable, main: &Variable, branch: &Variable, prefer_branch_on_conflict: bool) -> Variable {
    let pick = |b_eq_m: bool, b_eq_t: bool| -> Side {
        match (b_eq_m, b_eq_t) {
            (_, true) => Side::Main,
            (true, false) => Side::Branch,
            (false, false) => {
                if prefer_branch_on_conflict {
                    Side::Branch
                } else {
                    Side::Main
                }
            }
        }
    };

    let mut out = main.clone();
    if pick(base.name == main.name, base.name == branch.name) == Side::Branch {
        out.name = branch.name.clone();
    }
    if pick(base.collection == main.collection, base.collection == branch.collection) == Side::Branch {
        out.collection = branch.collection.clone();
    }
    if pick(base.kind == main.kind, base.kind == branch.kind) == Side::Branch {
        out.kind = branch.kind;
    }
    if pick(base.description == main.description, base.description == branch.description) == Side::Branch {
        out.description = branch.description.clone();
    }
    if pick(base.scopes == main.scopes, base.scopes == branch.scopes) == Side::Branch {
        out.scopes = branch.scopes.clone();
    }
    if pick(base.hidden_from_publishing == main.hidden_from_publishing, base.hidden_from_publishing == branch.hidden_from_publishing) == Side::Branch {
        out.hidden_from_publishing = branch.hidden_from_publishing;
    }
    if pick(base.key == main.key, base.key == branch.key) == Side::Branch {
        out.key = branch.key.clone();
    }
    if pick(base.code_syntax == main.code_syntax, base.code_syntax == branch.code_syntax) == Side::Branch {
        out.code_syntax = branch.code_syntax.clone();
    }

    let all_modes: BTreeSet<&String> = base.values.keys().chain(main.values.keys()).chain(branch.values.keys()).collect();
    for mode in all_modes {
        let bv = base.values.get(mode);
        let mv = main.values.get(mode);
        let tv = branch.values.get(mode);
        let side = pick(bv == mv, bv == tv);
        if side == Side::Branch {
            match tv {
                Some(v) => {
                    out.values.insert(mode.clone(), v.clone());
                }
                None => {
                    out.values.remove(mode);
                }
            }
        }
    }

    out
}

fn merge_styles(
    _base: &Document,
    main: &Document,
    _raw_branch: &Document,
    remapped_branch: &Document,
    diff: &Diff3,
    resolution: &Resolution,
    style_remap: &BTreeMap<String, String>,
) -> (BTreeMap<String, Style>, BTreeMap<String, StyleMeta>) {
    let mut out_styles = main.styles.clone();
    let mut out_meta = main.style_meta.clone();
    let conflict_keys: BTreeSet<&EntityKey> = diff.entity_conflicts.iter().map(|c| &c.key).collect();

    for (raw_key, change) in &diff.theirs.styles {
        let entity_key = EntityKey::Style(raw_key.clone());
        let mapped_key = remap_str(raw_key, style_remap);
        if mapped_key != *raw_key {
            if let Some(style) = remapped_branch.styles.get(&mapped_key) {
                out_styles.insert(mapped_key.clone(), style.clone());
            }
            if let Some(meta) = remapped_branch.style_meta.get(&mapped_key) {
                out_meta.insert(mapped_key, meta.clone());
            }
            continue;
        }

        let take_branch = if conflict_keys.contains(&entity_key) { resolution.per_entity.get(&entity_key) == Some(&Side::Branch) } else { true };
        if !take_branch {
            continue;
        }

        match change {
            EntityChange::Added | EntityChange::Changed => {
                if let Some(style) = remapped_branch.styles.get(&mapped_key) {
                    out_styles.insert(mapped_key.clone(), style.clone());
                }
                if let Some(meta) = remapped_branch.style_meta.get(&mapped_key) {
                    out_meta.insert(mapped_key, meta.clone());
                } else {
                    out_meta.remove(&mapped_key);
                }
            }
            EntityChange::Removed => {
                out_styles.remove(&mapped_key);
                out_meta.remove(&mapped_key);
            }
        }
    }

    (out_styles, out_meta)
}

fn merge_library_state(
    base: &Document,
    main: &Document,
    remapped_branch: &Document,
    removed_in_main: &BTreeSet<NodeId>,
    added_from_branch: &BTreeSet<NodeId>,
) -> (LibraryState, Option<NodeId>) {
    let mut out = main.library.clone();
    out.external.retain(|_, asset| asset.node.is_none_or(|n| !removed_in_main.contains(&n)));

    for (k, asset) in &remapped_branch.library.external {
        if asset.node.is_some_and(|n| removed_in_main.contains(&n)) {
            continue;
        }
        if !base.library.external.contains_key(k) || base.library.external.get(k) == main.library.external.get(k) {
            out.external.insert(k.clone(), asset.clone());
        }
    }

    for lib_ref in &remapped_branch.library.enabled {
        if !out.enabled.iter().any(|e| e.id == lib_ref.id) {
            out.enabled.push(lib_ref.clone());
        }
    }

    let mut lib_page = main.library_page;
    if lib_page.is_none() {
        if let Some(branch_lp) = remapped_branch.library_page {
            if added_from_branch.contains(&branch_lp) || (main.nodes.contains_key(&branch_lp) && !removed_in_main.contains(&branch_lp)) {
                lib_page = Some(branch_lp);
            }
        }
    }
    if let Some(lp) = lib_page {
        if removed_in_main.contains(&lp) {
            lib_page = None;
        }
    }

    (out, lib_page)
}

fn merge_flows(base: &Document, main: &Document, raw_branch: &Document, remapped_branch: &Document, diff: &Diff3, resolution: &Resolution) -> Vec<Flow> {
    let base_flows: BTreeMap<String, &Flow> = base.flows.iter().map(|f| (flow_key(f), f)).collect();
    let main_flows: BTreeMap<String, &Flow> = main.flows.iter().map(|f| (flow_key(f), f)).collect();
    let raw_branch_flows: BTreeMap<String, usize> = raw_branch.flows.iter().enumerate().map(|(idx, f)| (flow_key(f), idx)).collect();

    let conflict_keys: BTreeSet<&EntityKey> = diff.entity_conflicts.iter().map(|c| &c.key).collect();

    let mut merged_by_key: BTreeMap<String, Flow> = main.flows.iter().map(|f| (flow_key(f), f.clone())).collect();

    for (raw_key, change) in &diff.theirs.flows {
        let entity_key = EntityKey::Flow(raw_key.clone());
        let take_branch = if conflict_keys.contains(&entity_key) { resolution.per_entity.get(&entity_key) == Some(&Side::Branch) } else { true };
        if !take_branch {
            continue;
        }

        match change {
            EntityChange::Added | EntityChange::Changed => {
                if let Some(&idx) = raw_branch_flows.get(raw_key) {
                    if let Some(remapped_flow) = remapped_branch.flows.get(idx) {
                        merged_by_key.insert(flow_key(remapped_flow), remapped_flow.clone());
                    }
                }
            }
            EntityChange::Removed => {
                if base_flows.contains_key(raw_key) && main_flows.contains_key(raw_key) {
                    merged_by_key.remove(raw_key);
                }
            }
        }
    }

    merged_by_key.into_values().collect()
}

#[allow(clippy::too_many_arguments)]
fn merge_guides(
    _base: &Document,
    main: &Document,
    remapped_branch: &Document,
    diff: &Diff3,
    id_remap: &BTreeMap<NodeId, NodeId>,
    removed_in_main: &BTreeSet<NodeId>,
    added_from_branch: &BTreeSet<NodeId>,
    resolution: &Resolution,
) -> BTreeMap<NodeId, Vec<Guide>> {
    let mut out = main.guides.clone();
    out.retain(|page_id, _| !removed_in_main.contains(page_id));

    for (&page_id, branch_guides) in &remapped_branch.guides {
        if added_from_branch.contains(&page_id) && !removed_in_main.contains(&page_id) {
            out.insert(page_id, branch_guides.clone());
        }
    }

    let conflict_keys: BTreeSet<&EntityKey> = diff.entity_conflicts.iter().map(|c| &c.key).collect();
    for (&raw_page_id, &change) in &diff.theirs.guides {
        let page_id = id_remap.get(&raw_page_id).copied().unwrap_or(raw_page_id);
        if removed_in_main.contains(&page_id) || added_from_branch.contains(&page_id) || page_id != raw_page_id {
            continue;
        }
        let entity_key = EntityKey::Guides(raw_page_id);
        let is_conflicting = conflict_keys.contains(&entity_key);
        let side_choice = resolution.per_entity.get(&entity_key).copied().or_else(|| resolution.per_layer.get(&raw_page_id).copied());
        let take_branch = if is_conflicting { side_choice == Some(Side::Branch) } else { true };
        if !take_branch {
            continue;
        }
        match change {
            EntityChange::Added | EntityChange::Changed => {
                if let Some(branch_guides) = remapped_branch.guides.get(&page_id) {
                    out.insert(page_id, branch_guides.clone());
                }
            }
            EntityChange::Removed => {
                out.remove(&page_id);
            }
        }
    }

    out
}

fn merge_comments(
    base: &Document,
    main: &Document,
    raw_branch: &Document,
    remapped_branch: &Document,
    diff: &Diff3,
    resolution: &Resolution,
) -> Vec<CommentThread> {
    let mut out = main.comments.clone();
    let conflict_keys: BTreeSet<&EntityKey> = diff.entity_conflicts.iter().map(|c| &c.key).collect();

    let base_by_id: BTreeMap<&str, &CommentThread> = base.comments.iter().map(|t| (t.id.as_str(), t)).collect();
    let raw_index: BTreeMap<&str, usize> = raw_branch.comments.iter().enumerate().map(|(i, t)| (t.id.as_str(), i)).collect();

    for (thread_id, change) in &diff.theirs.comments {
        let entity_key = EntityKey::Comment(thread_id.clone());
        let is_conflicting = conflict_keys.contains(&entity_key);
        let side_choice = resolution.per_entity.get(&entity_key).copied();

        match change {
            EntityChange::Added => {
                if !is_conflicting || side_choice == Some(Side::Branch) {
                    if let Some(&idx) = raw_index.get(thread_id.as_str()) {
                        if let Some(remapped_thread) = remapped_branch.comments.get(idx) {
                            if let Some(existing) = out.iter_mut().find(|t| t.id == remapped_thread.id) {
                                *existing = remapped_thread.clone();
                            } else {
                                out.push(remapped_thread.clone());
                            }
                        }
                    }
                }
            }
            EntityChange::Removed => {
                if !is_conflicting || side_choice == Some(Side::Branch) {
                    out.retain(|t| &t.id != thread_id);
                }
            }
            EntityChange::Changed => {
                let Some(&idx) = raw_index.get(thread_id.as_str()) else {
                    continue;
                };
                let Some(remapped_thread) = remapped_branch.comments.get(idx) else {
                    continue;
                };
                if let Some(existing) = out.iter_mut().find(|t| &t.id == thread_id) {
                    if let Some(&base_thread) = base_by_id.get(thread_id.as_str()) {
                        let prefer_branch = !is_conflicting || side_choice == Some(Side::Branch);
                        *existing = merge_single_comment_thread(base_thread, existing, remapped_thread, prefer_branch);
                    } else if !is_conflicting || side_choice == Some(Side::Branch) {
                        *existing = remapped_thread.clone();
                    }
                }
            }
        }
    }

    out
}

fn merge_single_comment_thread(base: &CommentThread, main: &CommentThread, branch: &CommentThread, prefer_branch_on_conflict: bool) -> CommentThread {
    let mut out = main.clone();
    if base.anchor != branch.anchor && (base.anchor == main.anchor || prefer_branch_on_conflict) {
        out.anchor = branch.anchor;
    }
    if base.resolved != branch.resolved && (base.resolved == main.resolved || prefer_branch_on_conflict) {
        out.resolved = branch.resolved;
    }

    let b_msgs: BTreeMap<&str, &Comment> = base.messages.iter().map(|m| (m.id.as_str(), m)).collect();
    let m_msgs: BTreeMap<&str, &Comment> = main.messages.iter().map(|m| (m.id.as_str(), m)).collect();
    let t_msgs: BTreeMap<&str, &Comment> = branch.messages.iter().map(|m| (m.id.as_str(), m)).collect();

    let mut merged_msgs = Vec::new();
    let mut seen = BTreeSet::new();
    for m in &main.messages {
        seen.insert(m.id.as_str());
        match (b_msgs.get(m.id.as_str()), t_msgs.get(m.id.as_str())) {
            (Some(_), None) => {
                if prefer_branch_on_conflict && b_msgs.get(m.id.as_str()) == Some(&m) {
                    // Removed on branch and unchanged on main
                } else if b_msgs.get(m.id.as_str()) != Some(&m) && !prefer_branch_on_conflict {
                    merged_msgs.push(m.clone());
                }
            }
            (Some(bm), Some(tm)) => {
                if *bm == m && *bm != *tm || (*bm != m && *bm != *tm && prefer_branch_on_conflict) {
                    merged_msgs.push((*tm).clone());
                } else {
                    merged_msgs.push(m.clone());
                }
            }
            (None, Some(tm)) => {
                if prefer_branch_on_conflict {
                    merged_msgs.push((*tm).clone());
                } else {
                    merged_msgs.push(m.clone());
                }
            }
            (None, None) => merged_msgs.push(m.clone()),
        }
    }
    for tm in &branch.messages {
        if !seen.contains(tm.id.as_str()) && !b_msgs.contains_key(tm.id.as_str()) {
            let _ = m_msgs;
            merged_msgs.push(tm.clone());
        }
    }
    out.messages = merged_msgs;
    out
}

/// Validate that every reference in `T10.3 §2` resolves in `doc` (`T10.3 §3.7`).
pub fn validate_merged_references(doc: &Document) -> Vec<DanglingReference> {
    let mut dangling = Vec::new();

    for (&id, node) in &doc.nodes {
        let owner = format!("node {}", id.0);
        if let Some(parent) = node.parent {
            if !doc.nodes.contains_key(&parent) {
                dangling.push(DanglingReference { owner: owner.clone(), field: "Node.parent", target: format!("node {}", parent.0) });
            }
        }
        for &child in &node.children {
            if !doc.nodes.contains_key(&child) {
                dangling.push(DanglingReference { owner: owner.clone(), field: "Node.children", target: format!("node {}", child.0) });
            }
        }

        match &node.kind {
            NodeKind::Instance { component: Some(comp_id), .. } => match doc.nodes.get(comp_id) {
                Some(target) if matches!(target.kind, NodeKind::Component(_)) => {}
                _ => dangling.push(DanglingReference {
                    owner: owner.clone(),
                    field: "NodeKind::Instance.component",
                    target: format!("component node {}", comp_id.0),
                }),
            },
            NodeKind::Connector(conn) => {
                if let Some(start_node) = conn.start.node {
                    if !doc.nodes.contains_key(&start_node) {
                        dangling.push(DanglingReference { owner: owner.clone(), field: "Connector.start.node", target: format!("node {}", start_node.0) });
                    }
                }
                if let Some(end_node) = conn.end.node {
                    if !doc.nodes.contains_key(&end_node) {
                        dangling.push(DanglingReference { owner: owner.clone(), field: "Connector.end.node", target: format!("node {}", end_node.0) });
                    }
                }
            }
            _ => {}
        }

        // `node.component` (`ComponentLink`) references (`T10.3 §2`).
        if let Some(src) = node.component.source {
            if !doc.nodes.contains_key(&src) {
                dangling.push(DanglingReference { owner: owner.clone(), field: "ComponentLink.source", target: format!("node {}", src.0) });
            }
        }
        for prop in &node.component.props {
            check_prop_value_reference(doc, &owner, "ComponentPropDef.default", &prop.default, &mut dangling);
            if let PropKind::InstanceSwap { preferred } = &prop.kind {
                for &pref in preferred {
                    if !doc.nodes.contains_key(&pref) {
                        dangling.push(DanglingReference {
                            owner: owner.clone(),
                            field: "PropKind::InstanceSwap.preferred",
                            target: format!("node {}", pref.0),
                        });
                    }
                }
            }
        }
        for val in node.component.values.values() {
            check_prop_value_reference(doc, &owner, "ComponentLink.values", val, &mut dangling);
        }
        for cfg in node.component.slot_configs.values() {
            for &pref in &cfg.preferred {
                if !doc.nodes.contains_key(&pref) {
                    dangling.push(DanglingReference { owner: owner.clone(), field: "SlotPropConfig.preferred", target: format!("node {}", pref.0) });
                }
            }
        }

        // Variable bindings & modes on node (`T10.3 §2`).
        for var_id in node.variables.bindings.values() {
            if !doc.variables.contains_key(var_id) {
                dangling.push(DanglingReference { owner: owner.clone(), field: "NodeVariables.bindings", target: format!("variable {var_id}") });
            }
        }
        for (col_id, mode_id) in &node.variables.modes {
            match doc.variable_collections.get(col_id) {
                None => dangling.push(DanglingReference { owner: owner.clone(), field: "NodeVariables.modes", target: format!("collection {col_id}") }),
                Some(col) if !col.modes.iter().any(|m| &m.id == mode_id) => {
                    dangling.push(DanglingReference {
                        owner: owner.clone(),
                        field: "NodeVariables.modes",
                        target: format!("collection {col_id} mode {mode_id}"),
                    });
                }
                Some(_) => {}
            }
        }

        // Style refs on node (`T10.3 §2`).
        for style_key in node.style_refs.values() {
            if !doc.styles.contains_key(style_key) {
                dangling.push(DanglingReference { owner: owner.clone(), field: "Node.style_refs", target: format!("style {style_key}") });
            }
        }

        // Image hash references in node fills, stroke paints, and text style fills (`T10.3 §2`).
        for paint in &node.fills {
            if let crate::PaintKind::Image { image_ref, .. } = &paint.kind {
                if !doc.images.contains_key(image_ref) {
                    dangling.push(DanglingReference { owner: owner.clone(), field: "Node.fills", target: format!("image {image_ref}") });
                }
            }
        }
        for paint in &node.stroke.paints {
            if let crate::PaintKind::Image { image_ref, .. } = &paint.kind {
                if !doc.images.contains_key(image_ref) {
                    dangling.push(DanglingReference { owner: owner.clone(), field: "Node.stroke.paints", target: format!("image {image_ref}") });
                }
            }
        }
        if let Some(tp) = node.kind.any_text() {
            for paint in &tp.style.fills {
                if let crate::PaintKind::Image { image_ref, .. } = &paint.kind {
                    if !doc.images.contains_key(image_ref) {
                        dangling.push(DanglingReference { owner: owner.clone(), field: "TextStyle.fills", target: format!("image {image_ref}") });
                    }
                }
            }
            for run in &tp.runs {
                for paint in &run.style.fills {
                    if let crate::PaintKind::Image { image_ref, .. } = &paint.kind {
                        if !doc.images.contains_key(image_ref) {
                            dangling.push(DanglingReference { owner: owner.clone(), field: "TextRunStyle.fills", target: format!("image {image_ref}") });
                        }
                    }
                }
            }
        }

        // Prototype references on node (`T10.3 §2`).
        for &fixed in &node.prototype.fixed_children {
            if !doc.nodes.contains_key(&fixed) {
                dangling.push(DanglingReference { owner: owner.clone(), field: "PrototypeProps.fixed_children", target: format!("node {}", fixed.0) });
            }
        }
        for reaction in &node.prototype.reactions {
            for action in &reaction.actions {
                check_action_references(doc, &owner, action, &mut dangling);
            }
        }
    }

    // Variable & collection references (`Variable.collection` and `VariableValue::Alias`).
    for (var_id, var) in &doc.variables {
        let owner = format!("variable {var_id}");
        if !doc.variable_collections.contains_key(&var.collection) {
            dangling.push(DanglingReference { owner: owner.clone(), field: "Variable.collection", target: format!("collection {}", var.collection) });
        }
        for val in var.values.values() {
            if let VariableValue::Alias(target_var) = val {
                if !doc.variables.contains_key(target_var) {
                    dangling.push(DanglingReference { owner: owner.clone(), field: "VariableValue::Alias", target: format!("variable {target_var}") });
                }
            }
        }
    }

    // Flows (`page`, `start`).
    for flow in &doc.flows {
        let owner = format!("flow '{}'", flow.name);
        if !doc.nodes.contains_key(&flow.page) {
            dangling.push(DanglingReference { owner: owner.clone(), field: "Flow.page", target: format!("node {}", flow.page.0) });
        }
        if !doc.nodes.contains_key(&flow.start) {
            dangling.push(DanglingReference { owner, field: "Flow.start", target: format!("node {}", flow.start.0) });
        }
    }

    // Guides (`page` key and `guide.frame`).
    for (&page_id, guides) in &doc.guides {
        let owner = format!("guides for page {}", page_id.0);
        if !doc.nodes.contains_key(&page_id) {
            dangling.push(DanglingReference { owner: owner.clone(), field: "Document.guides.page", target: format!("node {}", page_id.0) });
        }
        for guide in guides {
            if let Some(frame_id) = guide.frame {
                if !doc.nodes.contains_key(&frame_id) {
                    dangling.push(DanglingReference { owner: owner.clone(), field: "Guide.frame", target: format!("node {}", frame_id.0) });
                }
            }
        }
    }

    // Comments (`CommentAnchor { page, node, .. }`).
    for thread in &doc.comments {
        let owner = format!("comment thread {}", thread.id);
        if !doc.nodes.contains_key(&thread.anchor.page) {
            dangling.push(DanglingReference { owner: owner.clone(), field: "CommentAnchor.page", target: format!("node {}", thread.anchor.page.0) });
        }
        if let Some(anchor_node) = thread.anchor.node {
            if !doc.nodes.contains_key(&anchor_node) {
                dangling.push(DanglingReference { owner, field: "CommentAnchor.node", target: format!("node {}", anchor_node.0) });
            }
        }
    }

    // Library state (`external[*].node` and `library_page`).
    for (ext_key, asset) in &doc.library.external {
        if let Some(node_id) = asset.node {
            if !doc.nodes.contains_key(&node_id) {
                dangling.push(DanglingReference {
                    owner: format!("library.external {ext_key}"),
                    field: "ExternalAsset.node",
                    target: format!("node {}", node_id.0),
                });
            }
        }
    }
    if let Some(lib_page) = doc.library_page {
        if !doc.nodes.contains_key(&lib_page) {
            dangling.push(DanglingReference { owner: "Document.library_page".into(), field: "Document.library_page", target: format!("node {}", lib_page.0) });
        }
    }

    dangling
}

fn check_prop_value_reference(doc: &Document, owner: &str, field: &'static str, val: &PropValue, dangling: &mut Vec<DanglingReference>) {
    match val {
        PropValue::Instance(target) => {
            if !doc.nodes.contains_key(target) {
                dangling.push(DanglingReference { owner: owner.to_owned(), field, target: format!("node {}", target.0) });
            }
        }
        PropValue::Slot(targets) => {
            for target in targets {
                if !doc.nodes.contains_key(target) {
                    dangling.push(DanglingReference { owner: owner.to_owned(), field, target: format!("node {}", target.0) });
                }
            }
        }
        PropValue::Bool(_) | PropValue::Text(_) | PropValue::Variant(_) => {}
    }
}

fn check_action_references(doc: &Document, owner: &str, action: &Action, dangling: &mut Vec<DanglingReference>) {
    match action {
        Action::Navigate { dest, .. }
        | Action::OpenOverlay { dest, .. }
        | Action::SwapOverlay { dest }
        | Action::ScrollTo { dest, .. }
        | Action::ChangeTo { dest }
        | Action::MediaControl { dest, .. } => {
            if !doc.nodes.contains_key(dest) {
                dangling.push(DanglingReference { owner: owner.to_owned(), field: "Action.dest", target: format!("node {}", dest.0) });
            }
        }
        Action::SetVariable { variable, value } => {
            if !doc.variables.contains_key(variable) {
                dangling.push(DanglingReference { owner: owner.to_owned(), field: "Action::SetVariable.variable", target: format!("variable {variable}") });
            }
            check_expression_references(doc, owner, value, dangling);
        }
        Action::SetVariableMode { collection, mode } => match doc.variable_collections.get(collection) {
            None => dangling.push(DanglingReference {
                owner: owner.to_owned(),
                field: "Action::SetVariableMode.collection",
                target: format!("collection {collection}"),
            }),
            Some(col) if !col.modes.iter().any(|m| &m.id == mode) => {
                dangling.push(DanglingReference {
                    owner: owner.to_owned(),
                    field: "Action::SetVariableMode.mode",
                    target: format!("collection {collection} mode {mode}"),
                });
            }
            Some(_) => {}
        },
        Action::Conditional { branches, otherwise } => {
            for (cond, actions) in branches {
                check_expression_references(doc, owner, cond, dangling);
                for a in actions {
                    check_action_references(doc, owner, a, dangling);
                }
            }
            for a in otherwise {
                check_action_references(doc, owner, a, dangling);
            }
        }
        Action::CloseOverlay | Action::Back | Action::OpenUrl { .. } => {}
    }
}

fn check_expression_references(doc: &Document, owner: &str, expr: &Expression, dangling: &mut Vec<DanglingReference>) {
    match expr {
        Expression::Literal(ExprValue::Number(_) | ExprValue::String(_) | ExprValue::Boolean(_) | ExprValue::Color(_)) => {}
        Expression::Variable(var_id) => {
            if !doc.variables.contains_key(var_id) {
                dangling.push(DanglingReference { owner: owner.to_owned(), field: "Expression::Variable", target: format!("variable {var_id}") });
            }
        }
        Expression::Unary { arg, .. } => {
            check_expression_references(doc, owner, arg, dangling);
        }
        Expression::Binary { lhs, rhs, .. } => {
            check_expression_references(doc, owner, lhs, dangling);
            check_expression_references(doc, owner, rhs, dangling);
        }
        Expression::Ternary { cond, then, otherwise } => {
            check_expression_references(doc, owner, cond, dangling);
            check_expression_references(doc, owner, then, dangling);
            check_expression_references(doc, owner, otherwise, dangling);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Color, ComponentPropDef, ConnectorCap, ConnectorEnd, ConnectorLine, ConnectorProps, FrameProps, Guide, GuideAxis, Magnet, Paint, Reaction, Rect,
        SlotPropConfig, Style, StyleMeta, StyleProperty, Trigger, Variable, VariableCollection, VariableKind, VariableMode, VariableValue, Vec2,
    };

    fn new_doc_with_page() -> (Document, NodeId) {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        (doc, page)
    }

    fn add_rect(doc: &mut Document, page: NodeId, name: &str, x: f32, y: f32) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(x, y, 100.0, 60.0));
        n.name = name.into();
        doc.insert(page, n).unwrap();
        id
    }

    #[test]
    fn every_reference_resolves_after_remap() {
        let (mut base, page) = new_doc_with_page();
        let _seed = add_rect(&mut base, page, "Seed", 0.0, 0.0);

        let mut main = base.clone();
        let mut branch = base.clone();

        // Main allocates several nodes, styles, collections, and variables so
        // their IDs collide with the branch's newly allocated items.
        let main_card = add_rect(&mut main, page, "Main Card", 20.0, 20.0);
        let main_extra = add_rect(&mut main, page, "Main Extra", 40.0, 40.0);
        assert_ne!(main_card, main_extra);
        main.styles.insert("brand/accent".into(), Style::Fill(vec![Paint::solid(Color::rgb(10, 40, 220))]));
        main.style_meta.insert(
            "brand/accent".into(),
            StyleMeta {
                name: "Main Accent".into(),
                key: None,
                description: String::new(),
                property: StyleProperty::Fill,
                bindings: Default::default(),
                ..Default::default()
            },
        );
        let col_id = "col-tokens".to_string();
        let mode_id = "mode-light".to_string();
        let var_id = "var-primary".to_string();
        main.variable_collections.insert(
            col_id.clone(),
            VariableCollection {
                id: col_id.clone(),
                name: "Main Tokens".into(),
                modes: vec![VariableMode { id: mode_id.clone(), name: "Light".into() }],
                default_mode: mode_id.clone(),
                variables: vec![var_id.clone()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        main.variables.insert(
            var_id.clone(),
            Variable {
                id: var_id.clone(),
                name: "MainOnlyPrimary".into(),
                collection: col_id.clone(),
                kind: VariableKind::Color,
                values: BTreeMap::from([(mode_id.clone(), VariableValue::Color(Color::rgb(10, 40, 220)))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        // Branch allocates colliding NodeIds, style key, collection id, and variable id,
        // and wires every reference family in `T10.3 §2` to those branch-added items.
        let branch_comp = branch.alloc_id(); // collides with `main_card`
        let branch_slot = branch.alloc_id(); // collides with `main_extra`
        let branch_inst = branch.alloc_id();
        let branch_conn = branch.alloc_id();

        let mut comp_node = Node::new(branch_comp, NodeKind::Component(FrameProps::default()));
        comp_node.name = "Branch Button".into();
        comp_node.component.props.push(ComponentPropDef {
            name: "Swap".into(),
            kind: PropKind::InstanceSwap { preferred: vec![branch_comp] },
            default: PropValue::Instance(branch_comp),
        });
        comp_node.component.slot_configs.insert("IconSlot".into(), SlotPropConfig { preferred: vec![branch_comp], ..Default::default() });
        comp_node.style_refs.insert("fill".into(), "brand/accent".into());
        comp_node.variables.bindings.insert("opacity".into(), var_id.clone());
        comp_node.variables.modes.insert(col_id.clone(), mode_id.clone());
        comp_node.prototype.fixed_children.push(branch_slot);
        branch.insert(page, comp_node).unwrap();

        let mut slot_node = Node::frame(branch_slot, Rect::new(0.0, 0.0, 24.0, 24.0));
        slot_node.name = "Icon Slot".into();
        slot_node.component.is_slot = true;
        slot_node.component.source = Some(branch_comp);
        branch.insert(branch_comp, slot_node).unwrap();

        let mut inst_node = Node::new(branch_inst, NodeKind::Instance { frame: FrameProps::default(), component: Some(branch_comp), component_key: None });
        inst_node.name = "Branch Button Instance".into();
        inst_node.component.values.insert("Swap".into(), PropValue::Instance(branch_comp));
        inst_node.component.values.insert("IconSlot".into(), PropValue::Slot(vec![branch_slot]));
        inst_node.prototype.reactions.push(Reaction {
            trigger: Trigger::OnClick,
            actions: vec![
                Action::Navigate { dest: branch_comp, preserve_scroll: false },
                Action::SetVariable { variable: var_id.clone(), value: Expression::Variable(var_id.clone()) },
                Action::SetVariableMode { collection: col_id.clone(), mode: mode_id.clone() },
            ],
            transition: None,
        });
        branch.insert(page, inst_node).unwrap();

        let mut conn_node = Node::new(
            branch_conn,
            NodeKind::Connector(ConnectorProps {
                start: ConnectorEnd { node: Some(branch_comp), point: Vec2::ZERO, magnet: Magnet::Auto },
                end: ConnectorEnd { node: Some(branch_inst), point: Vec2::new(100.0, 100.0), magnet: Magnet::Auto },
                line: ConnectorLine::Elbowed,
                start_cap: ConnectorCap::None,
                end_cap: ConnectorCap::Arrow,
                label: None,
            }),
        );
        conn_node.name = "Branch Connector".into();
        branch.insert(page, conn_node).unwrap();

        // Colliding style, collection, and variable on branch.
        branch.styles.insert("brand/accent".into(), Style::Fill(vec![Paint::solid(Color::rgb(230, 50, 20))]));
        branch.style_meta.insert(
            "brand/accent".into(),
            StyleMeta {
                name: "Branch Accent".into(),
                key: None,
                description: String::new(),
                property: StyleProperty::Fill,
                bindings: Default::default(),
                ..Default::default()
            },
        );
        let alias_var_id = "var-alias".to_string();
        branch.variable_collections.insert(
            col_id.clone(),
            VariableCollection {
                id: col_id.clone(),
                name: "Branch Tokens".into(),
                modes: vec![VariableMode { id: mode_id.clone(), name: "Light".into() }],
                default_mode: mode_id.clone(),
                variables: vec![var_id.clone(), alias_var_id.clone()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        branch.variables.insert(
            var_id.clone(),
            Variable {
                id: var_id.clone(),
                name: "BranchOnlyPrimary".into(),
                collection: col_id.clone(),
                kind: VariableKind::Color,
                values: BTreeMap::from([(mode_id.clone(), VariableValue::Color(Color::rgb(230, 50, 20)))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        branch.variables.insert(
            alias_var_id.clone(),
            Variable {
                id: alias_var_id,
                name: "BranchAlias".into(),
                collection: col_id.clone(),
                kind: VariableKind::Color,
                values: BTreeMap::from([(mode_id.clone(), VariableValue::Alias(var_id.clone()))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        // Branch flow, guide, and comment referencing the colliding `branch_comp`.
        branch.flows.push(Flow { name: "Branch Checkout".into(), page, start: branch_comp, description: String::new() });
        branch.guides.insert(page, vec![Guide { axis: GuideAxis::Y, offset: 32.0, frame: Some(branch_comp) }]);
        branch.comments.push(CommentThread {
            id: "thread-branch-1".into(),
            anchor: CommentAnchor { page, node: Some(branch_comp), offset: Vec2::new(10.0, 10.0) },
            messages: vec![Comment {
                id: "msg-1".into(),
                author: "Alice".into(),
                author_id: None,
                author_color: None,
                text: "Check this CTA".into(),
                created: "2026-10-19T00:00:00Z".into(),
                edited: None,
            }],
            resolved: false,
        });

        let diff = diff3(&base, &main, &branch);
        let plan = merge(&base, &main, &branch, &diff, &Resolution::default()).expect("merge with auto-remapped branch additions should succeed");

        assert!(plan.id_remap.contains_key(&branch_comp));
        assert!(plan.id_remap.contains_key(&branch_slot));
        let remapped_comp = plan.id_remap[&branch_comp];
        let remapped_slot = plan.id_remap[&branch_slot];
        assert_ne!(remapped_comp, main_card);
        assert_ne!(remapped_slot, main_extra);

        let merged = &plan.merged_document;
        assert!(merged.nodes.contains_key(&main_card));
        assert!(merged.nodes.contains_key(&remapped_comp));
        assert!(merged.nodes.contains_key(&remapped_slot));

        let dangling = validate_merged_references(merged);
        assert!(dangling.is_empty(), "expected no dangling refs, got {dangling:?}");

        // Check remapped style, collection, and variable keys (`<key>-branch`).
        assert!(merged.styles.contains_key("brand/accent"));
        assert!(merged.styles.contains_key("brand/accent-branch"));
        assert!(merged.variable_collections.contains_key("col-tokens-branch"));
        assert!(merged.variables.contains_key("var-primary-branch"));
    }

    #[test]
    fn branch_merge_remaps_colliding_variable_ids_in_prop_default_variables() {
        let (base, page) = new_doc_with_page();
        let mut main = base.clone();
        let mut branch = base.clone();

        let col_id = "col-tokens".to_string();
        let mode_id = "mode-1".to_string();
        let var_id = "var-label".to_string();

        for (doc, text_val) in [(&mut main, "Main Label"), (&mut branch, "Branch Label")] {
            doc.variable_collections.insert(
                col_id.clone(),
                VariableCollection {
                    id: col_id.clone(),
                    name: "Tokens".into(),
                    modes: vec![VariableMode { id: mode_id.clone(), name: "Default".into() }],
                    default_mode: mode_id.clone(),
                    variables: vec![var_id.clone()],
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
            doc.variables.insert(
                var_id.clone(),
                Variable {
                    id: var_id.clone(),
                    name: text_val.into(),
                    collection: col_id.clone(),
                    kind: VariableKind::String,
                    values: BTreeMap::from([(mode_id.clone(), VariableValue::String(text_val.into()))]),
                    description: String::new(),
                    scopes: Vec::new(),
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: BTreeMap::new(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
        }

        let branch_comp = branch.alloc_id();
        let mut comp_node = Node::new(branch_comp, NodeKind::Component(FrameProps::default()));
        comp_node.name = "Branch Button".into();
        comp_node.component.props.push(ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("CTA".into()) });
        comp_node.component.prop_default_variables.insert("Label".into(), var_id.clone());
        branch.insert(page, comp_node).unwrap();

        let diff = diff3(&base, &main, &branch);
        let plan = merge(&base, &main, &branch, &diff, &Resolution::default()).expect("merge succeeds");
        let merged = &plan.merged_document;
        let merged_comp = merged.nodes.values().find(|n| n.name == "Branch Button").unwrap();
        assert_eq!(
            merged_comp.component.prop_default_variables.get("Label").map(String::as_str),
            Some("var-label-branch"),
            "branch merge must remap colliding variable ids in prop_default_variables"
        );
    }

    #[test]
    fn chosen_side_applies_per_layer() {
        let (mut base, page) = new_doc_with_page();
        let rect = add_rect(&mut base, page, "Hero", 10.0, 20.0);

        let mut main = base.clone();
        let mut branch = base.clone();

        // Both sides edit `Opacity` (conflict), while only `main` edits `Name`
        // (non-conflicting) and only `branch` edits `Size` (non-conflicting).
        {
            let m = main.nodes.get_mut(&rect).unwrap();
            m.opacity = 0.5;
            m.name = "Hero Renamed On Main".into();
        }
        {
            let b = branch.nodes.get_mut(&rect).unwrap();
            b.opacity = 0.85;
            b.size = Vec2::new(320.0, 180.0);
        }

        let diff = diff3(&base, &main, &branch);
        assert_eq!(diff.conflicts.len(), 1);

        // 1) Choose `Side::Main` for `rect`: conflicting `Opacity` comes from `main` (0.5),
        // while non-conflicting `Size` from `branch` (320x180) and `Name` from `main` both apply!
        let res_main = Resolution::new().with_layer(rect, Side::Main);
        let plan_main = merge(&base, &main, &branch, &diff, &res_main).unwrap();
        let merged_main_rect = &plan_main.merged_document.nodes[&rect];
        assert_eq!(merged_main_rect.opacity, 0.5);
        assert_eq!(merged_main_rect.name, "Hero Renamed On Main");
        assert_eq!(merged_main_rect.size, Vec2::new(320.0, 180.0));

        // 2) Choose `Side::Branch` for `rect`: conflicting `Opacity` comes from `branch` (0.85),
        // and non-conflicting groups from both sides still apply.
        let res_branch = Resolution::new().with_layer(rect, Side::Branch);
        let plan_branch = merge(&base, &main, &branch, &diff, &res_branch).unwrap();
        let merged_branch_rect = &plan_branch.merged_document.nodes[&rect];
        assert_eq!(merged_branch_rect.opacity, 0.85);
        assert_eq!(merged_branch_rect.name, "Hero Renamed On Main");
        assert_eq!(merged_branch_rect.size, Vec2::new(320.0, 180.0));
    }

    #[test]
    fn unresolved_refused() {
        let (mut base, page) = new_doc_with_page();
        let rect = add_rect(&mut base, page, "Card", 0.0, 0.0);

        let mut main = base.clone();
        let mut branch = base.clone();
        main.nodes.get_mut(&rect).unwrap().opacity = 0.25;
        branch.nodes.get_mut(&rect).unwrap().opacity = 0.75;

        let diff = diff3(&base, &main, &branch);
        let err = merge(&base, &main, &branch, &diff, &Resolution::default()).expect_err("missing conflict choice must return MergeError::Unresolved");
        assert_eq!(err, MergeError::Unresolved(vec![UnresolvedItem::Layer(rect)]));
    }

    #[test]
    fn deleted_component_referenced_by_main_instance_refused_as_dangling() {
        // `T10.3 §8.1`: Branch deletes a component that `main` added a new instance of.
        let (mut base, page) = new_doc_with_page();
        let comp_id = base.alloc_id();
        let mut comp_node = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
        comp_node.name = "Card Component".into();
        base.insert(page, comp_node).unwrap();

        let mut main = base.clone();
        let mut branch = base.clone();

        // Main adds a new instance of `comp_id`.
        let inst_id = main.alloc_id();
        let mut inst_node = Node::new(inst_id, NodeKind::Instance { frame: FrameProps::default(), component: Some(comp_id), component_key: None });
        inst_node.name = "Card Instance".into();
        main.insert(page, inst_node).unwrap();

        // Branch removes `comp_id`.
        branch.remove(comp_id).unwrap();

        let diff = diff3(&base, &main, &branch);
        assert!(
            diff.conflicts.iter().any(|c| c.node == comp_id && c.kind == ConflictKind::DeleteModify),
            "component referenced by surviving main instance must record DeleteModify conflict"
        );

        // 1) Unresolved without a choice on `comp_id`:
        let unresolved_err = merge(&base, &main, &branch, &diff, &Resolution::default()).expect_err("missing component choice must be Unresolved");
        assert!(matches!(unresolved_err, MergeError::Unresolved(ref items) if items.contains(&UnresolvedItem::Layer(comp_id))));

        // 2) Choosing `Side::Branch` removes the component and fails reference validation naming `inst_id`:
        let res_branch = Resolution::new().with_layer(comp_id, Side::Branch);
        let err = merge(&base, &main, &branch, &diff, &res_branch).expect_err("deleting component referenced by main instance must fail validation");
        match err {
            MergeError::DanglingReference(refs) => {
                assert!(
                    refs.iter().any(|r| r.owner == format!("node {}", inst_id.0) && r.field == "NodeKind::Instance.component"),
                    "DanglingReference must name main's instance, got: {refs:?}"
                );
            }
            other => panic!("expected DanglingReference, got {other:?}"),
        }

        // 3) Choosing `Side::Main` on the component keeps BOTH the component and main's instance:
        let res_main = Resolution::new().with_layer(comp_id, Side::Main);
        let plan_main = merge(&base, &main, &branch, &diff, &res_main).expect("Side::Main on component must keep both component and instance");
        assert!(plan_main.merged_document.nodes.contains_key(&comp_id), "component must survive when Side::Main is chosen");
        assert!(plan_main.merged_document.nodes.contains_key(&inst_id), "main instance must survive when Side::Main is chosen");

        // 4) Same when the instance already existed in `base` (unmodified on `main`) and `branch` deletes `comp_id`:
        let base_with_inst = main.clone();
        let main_untouched = base_with_inst.clone();
        let mut branch_del_comp = base_with_inst.clone();
        branch_del_comp.remove(comp_id).unwrap();
        let diff_existing = diff3(&base_with_inst, &main_untouched, &branch_del_comp);
        assert!(
            diff_existing.conflicts.iter().any(|c| c.node == comp_id && c.kind == ConflictKind::DeleteModify),
            "component referenced by unmodified base instance must also record DeleteModify conflict"
        );
        let plan_keep = merge(&base_with_inst, &main_untouched, &branch_del_comp, &diff_existing, &res_main)
            .expect("Side::Main must keep component referenced by unmodified base instance");
        assert!(plan_keep.merged_document.nodes.contains_key(&comp_id));
        assert!(plan_keep.merged_document.nodes.contains_key(&inst_id));
    }

    #[test]
    fn guides_diffed_per_page_conflict_and_branch_deletion() {
        let (mut base, page) = new_doc_with_page();
        base.guides.insert(page, vec![Guide { axis: GuideAxis::X, offset: 5.0, frame: None }]);

        // Case 1: Branch deletes the page's guide entry (key removed from `Document.guides`), main untouched -> deletion lands.
        let main_untouched = base.clone();
        let mut branch_del = base.clone();
        branch_del.guides.remove(&page);
        let (merged_del, _) =
            merge_branch(&base, &main_untouched, &branch_del, &Resolution::default()).expect("one-sided branch guide deletion should succeed");
        assert!(!merged_del.guides.contains_key(&page), "branch-side deletion of page guide entry must remove key from main");

        // Case 2: Main adds a guide at y=10 and branch adds one at y=40 on the same page -> conflict!
        let (base_empty, page2) = new_doc_with_page();
        let mut main_add = base_empty.clone();
        let mut branch_add = base_empty.clone();
        main_add.guides.insert(page2, vec![Guide { axis: GuideAxis::Y, offset: 10.0, frame: None }]);
        branch_add.guides.insert(page2, vec![Guide { axis: GuideAxis::Y, offset: 40.0, frame: None }]);

        let diff_add = diff3(&base_empty, &main_add, &branch_add);
        assert!(diff_add.entity_conflicts.iter().any(|c| c.key == EntityKey::Guides(page2)), "add/add guides on the same page must record EntityConflict");
        let err = merge(&base_empty, &main_add, &branch_add, &diff_add, &Resolution::default()).expect_err("conflicting guide edits must require resolution");
        assert_eq!(err, MergeError::Unresolved(vec![UnresolvedItem::Entity(EntityKey::Guides(page2))]));

        let res_branch = Resolution::new().with_entity(EntityKey::Guides(page2), Side::Branch);
        let plan_branch = merge(&base_empty, &main_add, &branch_add, &diff_add, &res_branch).expect("resolving guides via per_entity should succeed");
        assert_eq!(plan_branch.merged_document.guides[&page2], vec![Guide { axis: GuideAxis::Y, offset: 40.0, frame: None }]);

        let res_page_main = Resolution::new().with_layer(page2, Side::Main);
        let plan_main = merge(&base_empty, &main_add, &branch_add, &diff_add, &res_page_main).expect("resolving guides via page choice should succeed");
        assert_eq!(plan_main.merged_document.guides[&page2], vec![Guide { axis: GuideAxis::Y, offset: 10.0, frame: None }]);
    }

    #[test]
    fn branch_deleted_image_still_referenced_on_main_conflicts_and_validates() {
        use crate::{ImageScaleMode, PaintKind};

        let (mut base, page) = new_doc_with_page();
        let hash = "img_shared_hash".to_string();
        base.images.insert(hash.clone(), vec![1, 2, 3, 4]);

        let main_rect = add_rect(&mut base, page, "Main Painted Rect", 0.0, 0.0);
        base.nodes.get_mut(&main_rect).unwrap().fills = vec![Paint {
            kind: PaintKind::Image { image_ref: hash.clone(), scale_mode: ImageScaleMode::Fill, params: Default::default() },
            opacity: 1.0,
            visible: true,
            blend: crate::BlendMode::Normal,
        }];
        let branch_rect = add_rect(&mut base, page, "Branch Painted Rect", 120.0, 0.0);
        base.nodes.get_mut(&branch_rect).unwrap().fills = vec![Paint {
            kind: PaintKind::Image { image_ref: hash.clone(), scale_mode: ImageScaleMode::Fill, params: Default::default() },
            opacity: 1.0,
            visible: true,
            blend: crate::BlendMode::Normal,
        }];

        let main = base.clone();
        let mut branch = base.clone();
        // Branch replaces its own rect's fill and drops `hash` from `branch.images`, while `main_rect` still paints `hash`.
        branch.nodes.get_mut(&branch_rect).unwrap().fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        branch.images.remove(&hash);

        let diff = diff3(&base, &main, &branch);
        assert!(
            diff.entity_conflicts.iter().any(|c| c.key == EntityKey::Image(hash.clone()) && c.kind == ConflictKind::DeleteModify),
            "dropping an image hash still referenced by a surviving node must raise DeleteModify conflict"
        );

        // Unresolved without a choice:
        let unresolved = merge(&base, &main, &branch, &diff, &Resolution::default()).expect_err("must require resolution when branch deletes referenced image");
        assert_eq!(unresolved, MergeError::Unresolved(vec![UnresolvedItem::Entity(EntityKey::Image(hash.clone()))]));

        // Choosing Side::Branch removes the image and fails reference validation naming `main_rect`:
        let res_branch = Resolution::new().with_entity(EntityKey::Image(hash.clone()), Side::Branch);
        let err = merge(&base, &main, &branch, &diff, &res_branch).expect_err("removing image referenced by surviving fill must fail validation");
        match err {
            MergeError::DanglingReference(refs) => {
                assert!(
                    refs.iter().any(|r| r.owner == format!("node {}", main_rect.0) && r.target == format!("image {hash}")),
                    "validator must report dangling fill image hash on main_rect, got {refs:?}"
                );
            }
            other => panic!("expected DanglingReference, got {other:?}"),
        }

        // Choosing Side::Main keeps the image bytes and succeeds:
        let res_main = Resolution::new().with_entity(EntityKey::Image(hash.clone()), Side::Main);
        let plan_main = merge(&base, &main, &branch, &diff, &res_main).expect("Side::Main must keep referenced image bytes");
        assert!(plan_main.merged_document.images.contains_key(&hash));
        assert!(validate_merged_references(&plan_main.merged_document).is_empty());
    }

    #[test]
    fn branch_instance_of_deleted_main_component_conflicts_and_dangles() {
        // `T10.3 §8.2`: `main` deletes a component that the branch added a new instance of.
        let (mut base, page) = new_doc_with_page();
        let comp_id = base.alloc_id();
        let mut comp_node = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
        comp_node.name = "Card Component".into();
        base.insert(page, comp_node).unwrap();

        let mut main = base.clone();
        let mut branch = base.clone();

        main.remove(comp_id).unwrap();

        let inst_id = branch.alloc_id();
        let mut inst_node = Node::new(inst_id, NodeKind::Instance { frame: FrameProps::default(), component: Some(comp_id), component_key: None });
        inst_node.name = "Branch Card Instance".into();
        branch.insert(page, inst_node).unwrap();

        let diff = diff3(&base, &main, &branch);
        // Unresolved without a choice:
        assert!(matches!(merge(&base, &main, &branch, &diff, &Resolution::default()), Err(MergeError::Unresolved(_))));
        // Choosing `Side::Branch` without keeping the component fails reference validation:
        let res_branch = Resolution::new().with_layer(inst_id, Side::Branch);
        assert!(matches!(merge(&base, &main, &branch, &diff, &res_branch), Err(MergeError::DanglingReference(_))));
        // Choosing `Side::Main` drops the dangling branch instance and succeeds:
        let res_main = Resolution::new().with_layer(inst_id, Side::Main);
        assert!(merge(&base, &main, &branch, &diff, &res_main).is_ok());
    }

    #[test]
    fn same_named_variables_with_different_ids_kept_and_renamed() {
        // `T10.3 §8.4`: Both sides add a variable with the same name and different IDs.
        let (mut base, _page) = new_doc_with_page();
        let col_id = "col-1".to_string();
        let mode_id = "mode-1".to_string();
        base.variable_collections.insert(
            col_id.clone(),
            VariableCollection {
                id: col_id.clone(),
                name: "Primitives".into(),
                modes: vec![VariableMode { id: mode_id.clone(), name: "Default".into() }],
                default_mode: mode_id.clone(),
                variables: Vec::new(),
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );

        let mut main = base.clone();
        let mut branch = base.clone();

        main.variables.insert(
            "var-main-accent".into(),
            Variable {
                id: "var-main-accent".into(),
                name: "brand/accent".into(),
                collection: col_id.clone(),
                kind: VariableKind::Color,
                values: BTreeMap::from([(mode_id.clone(), VariableValue::Color(Color::rgb(20, 40, 60)))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        branch.variables.insert(
            "var-branch-accent".into(),
            Variable {
                id: "var-branch-accent".into(),
                name: "brand/accent".into(),
                collection: col_id,
                kind: VariableKind::Color,
                values: BTreeMap::from([(mode_id, VariableValue::Color(Color::rgb(220, 100, 30)))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        let (merged, plan) = merge_branch(&base, &main, &branch, &Resolution::default()).unwrap();
        assert_eq!(merged.variables["var-main-accent"].name, "brand/accent");
        assert_eq!(merged.variables["var-branch-accent"].name, "brand/accent (branch)");
        assert_eq!(plan.report.renamed_variables.get("var-branch-accent").map(String::as_str), Some("brand/accent (branch)"));
    }

    #[test]
    fn single_undo_reverts_entire_merge_batch() {
        let (mut base, page) = new_doc_with_page();
        let shared = add_rect(&mut base, page, "Shared", 0.0, 0.0);

        let mut main = base.clone();
        let mut branch = base.clone();

        let _main_added = add_rect(&mut main, page, "Main Added", 50.0, 50.0);
        let _branch_added = add_rect(&mut branch, page, "Branch Added", 150.0, 150.0);
        branch.nodes.get_mut(&shared).unwrap().fills = vec![Paint::solid(Color::rgb(50, 200, 100))];

        let snapshot_before_merge = main.clone();
        let diff = diff3(&base, &main, &branch);
        let plan = merge(&base, &main, &branch, &diff, &Resolution::default()).unwrap();

        let mut history = History::default();
        history.exec(&mut main, plan.command).unwrap();
        assert_ne!(main.nodes.len(), snapshot_before_merge.nodes.len());

        // Single undo reverts the entire merge batch (`T10.3 §3.5`).
        assert!(history.undo(&mut main).unwrap());
        assert_eq!(main.pages, snapshot_before_merge.pages);
        assert_eq!(main.nodes, snapshot_before_merge.nodes);
    }

    #[test]
    fn branch_merge_rejects_instance_cycle_created_by_add_then_move() {
        let mut base = Document::new("branch-cycle");
        let page = base.add_page("Page");
        let comp_id = base.alloc_id();
        let comp = Node::new(comp_id, NodeKind::Component(crate::FrameProps::default()));
        base.insert(page, comp).unwrap();
        let x_id = base.alloc_id();
        base.insert(page, Node::frame(x_id, crate::Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();

        let mut main = base.clone();
        main.reparent(x_id, comp_id, 0).unwrap();

        let mut branch = base.clone();
        let inst_id = branch.alloc_id();
        let inst = Node::new(inst_id, NodeKind::Instance { frame: crate::FrameProps::default(), component: Some(comp_id), component_key: None });
        branch.insert(x_id, inst).unwrap();

        let result = merge_branch(&base, &main, &branch, &Resolution::default());
        let err = result.expect_err("merge must reject the cycle formed after add then move");
        let text = err.to_string();
        assert_eq!(text, "Can't merge: Component would contain itself (Component \u{2192} Frame \u{2192} Component)");
        assert!(!text.contains("(id "), "error omits internal ids: {text}");
    }

    #[test]
    fn branch_merge_refuses_self_holding_cycle_with_component_name_and_path_without_partial_apply() {
        let mut base = Document::new("branch-cycle-named");
        let page = base.add_page("Page");
        let comp_id = base.alloc_id();
        let mut comp = Node::new(comp_id, NodeKind::Component(crate::FrameProps::default()));
        comp.name = "B".into();
        base.insert(page, comp).unwrap();
        let x_id = base.alloc_id();
        let mut x = Node::frame(x_id, crate::Rect::new(0.0, 0.0, 100.0, 100.0));
        x.name = "X".into();
        base.insert(page, x).unwrap();

        // Case 1: main moves X into B; branch puts an instance of B into X and also adds an unrelated layer.
        let mut main_a = base.clone();
        main_a.reparent(x_id, comp_id, 0).unwrap();
        let main_a_before = main_a.clone();

        let mut branch_a = base.clone();
        let inst_id = branch_a.alloc_id();
        let mut inst = Node::new(inst_id, NodeKind::Instance { frame: crate::FrameProps::default(), component: Some(comp_id), component_key: None });
        inst.name = "B".into();
        branch_a.insert(x_id, inst).unwrap();
        let _extra = add_rect(&mut branch_a, page, "Unrelated", 200.0, 200.0);

        let err_a = merge_branch(&base, &main_a, &branch_a, &Resolution::default()).expect_err("add-then-move cycle must be refused");
        let msg_a = err_a.to_string();
        assert_eq!(msg_a, "Can't merge: B would contain itself (B \u{2192} X \u{2192} B)");
        assert_eq!(main_a.nodes, main_a_before.nodes, "whole merge is refused; main is not half-applied");

        // Case 2: main puts an instance of B into X; branch moves X into B and adds an unrelated layer.
        let mut main_b = base.clone();
        let inst_main_id = main_b.alloc_id();
        let mut inst_main = Node::new(inst_main_id, NodeKind::Instance { frame: crate::FrameProps::default(), component: Some(comp_id), component_key: None });
        inst_main.name = "B".into();
        main_b.insert(x_id, inst_main).unwrap();
        let main_b_before = main_b.clone();

        let mut branch_b = base.clone();
        branch_b.reparent(x_id, comp_id, 0).unwrap();
        let _extra_b = add_rect(&mut branch_b, page, "Unrelated", 200.0, 200.0);

        let err_b = merge_branch(&base, &main_b, &branch_b, &Resolution::default()).expect_err("move-then-add cycle must be refused");
        let msg_b = err_b.to_string();
        assert_eq!(msg_b, "Can't merge: B would contain itself (B \u{2192} X \u{2192} B)");
        assert_eq!(main_b.nodes, main_b_before.nodes, "whole merge is refused; main is not half-applied");

        // Case 3: base has an instance of B inside X; branch removes the instance and moves X into B (legal).
        let base_with_inst = main_b.clone();
        let main_c = base_with_inst.clone();
        let mut branch_c = base_with_inst.clone();
        branch_c.remove(inst_main_id).unwrap();
        branch_c.reparent(x_id, comp_id, 0).unwrap();
        let (merged_c, _) = merge_branch(&base_with_inst, &main_c, &branch_c, &Resolution::default()).expect("remove-then-move must succeed");
        assert_eq!(merged_c.get(x_id).unwrap().parent, Some(comp_id));
        assert!(merged_c.get(inst_main_id).is_none());
    }

    #[test]
    fn merge_into_a_file_with_an_existing_cycle_still_applies_unrelated_changes() {
        let mut base = Document::new("existing-cycle");
        let page = base.add_page("Page");
        let b = base.alloc_id();
        let mut comp = Node::new(b, NodeKind::Component(crate::FrameProps::default()));
        comp.name = "B".into();
        base.insert(page, comp).unwrap();
        let self_inst_id = base.alloc_id();
        let mut self_inst = Node::new(self_inst_id, NodeKind::Instance { frame: crate::FrameProps::default(), component: Some(b), component_key: None });
        self_inst.name = "B self".into();
        self_inst.component.source = Some(b);
        base.insert(b, self_inst).unwrap();

        let main = base.clone();
        let mut branch = base.clone();
        let rect_id = add_rect(&mut branch, page, "Unrelated", 300.0, 300.0);

        let (merged, _) = merge_branch(&base, &main, &branch, &Resolution::default()).expect("existing cycle in file must not block an unrelated branch merge");
        assert!(merged.get(rect_id).is_some(), "unrelated branch change must be applied");
    }

    #[test]
    fn colliding_extended_collection_keeps_parent_overrides_and_modes() {
        use crate::VariableStore;

        let (base, page) = new_doc_with_page();

        let mut main = base.clone();
        let mut ms = VariableStore::from_doc(&main);
        let spacing = ms.add_collection("Spacing");
        let gap = ms.add_variable(&spacing, "gap", VariableKind::Number).unwrap();
        let spacing_mode = ms.collections[&spacing].default_mode.clone();
        assert!(ms.set_value(&gap, &spacing_mode, VariableValue::Number(12.0)));
        let sizing = ms.add_collection("Sizing");
        let pad = ms.add_variable(&sizing, "pad", VariableKind::Number).unwrap();
        let sizing_mode = ms.collections[&sizing].default_mode.clone();
        assert!(ms.set_value(&pad, &sizing_mode, VariableValue::Number(24.0)));
        main.variables = ms.variables;
        main.variable_collections = ms.collections;

        let mut branch = base.clone();
        let mut bs = VariableStore::from_doc(&branch);
        let theme = bs.add_collection("Theme");
        let light = bs.collections[&theme].default_mode.clone();
        let dark = bs.add_mode(&theme, "Dark").unwrap();
        let bg = bs.add_variable(&theme, "bg", VariableKind::Color).unwrap();
        let accent = bs.add_variable(&theme, "accent", VariableKind::Color).unwrap();
        assert!(bs.set_value(&bg, &light, VariableValue::Color(Color::WHITE)));
        assert!(bs.set_value(&bg, &dark, VariableValue::Color(Color::BLACK)));
        assert!(bs.set_value(&accent, &light, VariableValue::Color(Color::rgb(0, 100, 200))));
        assert!(bs.set_value(&accent, &dark, VariableValue::Color(Color::rgb(0, 50, 100))));

        let brand = bs.extend_collection(&theme, "Brand").unwrap();
        let brand_light = bs.collections[&brand].modes[0].id.clone();
        let brand_dark = bs.collections[&brand].modes[1].id.clone();
        assert!(bs.set_value(&bg, &brand_light, VariableValue::Color(Color::rgb(255, 0, 0))));
        assert!(bs.set_alias(&accent, &brand_dark, &bg));
        branch.variables = bs.variables;
        branch.variable_collections = bs.collections;

        let rect = add_rect(&mut branch, page, "Themed Swatch", 10.0, 10.0);
        branch.get_mut(rect).unwrap().variables.bindings.insert("fills/0".into(), bg.clone());
        branch.get_mut(rect).unwrap().variables.modes.insert(brand.clone(), brand_light);

        let (merged, plan) = merge_branch(&base, &main, &branch, &Resolution::default()).expect("merge with colliding extended collection must succeed");
        let merged_brand = merged.variable_collections.values().find(|c| c.name == "Brand").expect("Brand collection merged");
        let merged_theme = merged.variable_collections.values().find(|c| c.name == "Theme").expect("Theme collection merged");
        assert_eq!(merged_brand.extension_parent_id(), Some(merged_theme.id.as_str()), "Brand must still point at Theme, not Spacing");

        let ext = merged_brand.extension().expect("Brand extension data");
        for (var_id, by_mode) in &ext.overrides {
            let var = merged.variables.get(var_id).expect("override key must exist in merged variables");
            assert_eq!(var.collection, merged_theme.id, "override must target a variable in Theme");
            assert_eq!(var.kind, VariableKind::Color, "Color override must stay on a Color variable, never on Spacing's Number variable");
            for (mode_id, val) in by_mode {
                assert!(merged_brand.modes.iter().any(|m| &m.id == mode_id), "override mode must belong to Brand's remapped modes");
                if let VariableValue::Alias(target) = val {
                    let target_var = merged.variables.get(target).expect("override alias target must exist");
                    assert_eq!(target_var.collection, merged_theme.id);
                    assert_eq!(target_var.kind, VariableKind::Color);
                }
            }
        }
        for (own_mode, parent_mode) in &ext.mode_map {
            assert!(merged_brand.modes.iter().any(|m| &m.id == own_mode), "mode_map key must be a mode of Brand");
            assert!(merged_theme.modes.iter().any(|m| &m.id == parent_mode), "mode_map value must be a mode of Theme");
        }

        let merged_store = VariableStore::from_doc(&merged);
        assert!(merged_store.extension_violations().is_empty(), "merged store must have no extension rule violations");
        let merged_bg = merged.variables.values().find(|v| v.name == "bg").unwrap();
        let merged_accent = merged.variables.values().find(|v| v.name == "accent").unwrap();
        let merged_brand_light = &merged_brand.modes[0].id;
        let merged_brand_dark = &merged_brand.modes[1].id;
        assert_eq!(merged_store.resolve(&merged_bg.id, merged_brand_light), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        assert_eq!(merged_store.resolve(&merged_accent.id, merged_brand_dark), Some(VariableValue::Color(Color::BLACK)));
        assert_eq!(merged_store.resolve(&gap, &spacing_mode), Some(VariableValue::Number(12.0)));

        let merged_rect = plan.id_remap.get(&rect).copied().unwrap_or(rect);
        assert_eq!(merged.bound_value(merged_rect, "fills/0"), Some(VariableValue::Color(Color::rgb(255, 0, 0))));

        // Also verify when only the parent collection id collides (main has only Spacing d1; branch has Theme d1 + Brand d2).
        let mut main_single = base.clone();
        let mut ms1 = VariableStore::from_doc(&main_single);
        let spacing1 = ms1.add_collection("Spacing");
        let gap1 = ms1.add_variable(&spacing1, "gap", VariableKind::Number).unwrap();
        let spacing1_mode = ms1.collections[&spacing1].default_mode.clone();
        assert!(ms1.set_value(&gap1, &spacing1_mode, VariableValue::Number(12.0)));
        main_single.variables = ms1.variables;
        main_single.variable_collections = ms1.collections;

        let (merged1, _) =
            merge_branch(&base, &main_single, &branch, &Resolution::default()).expect("merge with single colliding parent collection must succeed");
        let merged1_brand = merged1.variable_collections.values().find(|c| c.name == "Brand").unwrap();
        let merged1_theme = merged1.variable_collections.values().find(|c| c.name == "Theme").unwrap();
        assert_eq!(merged1_brand.extension_parent_id(), Some(merged1_theme.id.as_str()));
        let ext1 = merged1_brand.extension().unwrap();
        for var_id in ext1.overrides.keys() {
            assert_eq!(merged1.variables[var_id].kind, VariableKind::Color);
        }
        assert!(VariableStore::from_doc(&merged1).extension_violations().is_empty());
    }

    #[test]
    fn merge_refuses_variable_alias_cycle_closed_across_branches() {
        use crate::VariableStore;

        // 1. Plain collection: main aliases bg -> fg, branch aliases fg -> bg.
        let (mut base, _page) = new_doc_with_page();
        let mut s = VariableStore::from_doc(&base);
        let theme = s.add_collection("Theme");
        let light = s.collections[&theme].default_mode.clone();
        let bg = s.add_variable(&theme, "bg", VariableKind::Color).unwrap();
        let fg = s.add_variable(&theme, "fg", VariableKind::Color).unwrap();
        assert!(s.set_value(&bg, &light, VariableValue::Color(Color::WHITE)));
        assert!(s.set_value(&fg, &light, VariableValue::Color(Color::BLACK)));
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let brand_light = s.collections[&brand].modes[0].id.clone();
        base.variables = s.variables;
        base.variable_collections = s.collections;

        let mut main = base.clone();
        let mut ms = VariableStore::from_doc(&main);
        assert!(ms.set_alias(&bg, &light, &fg));
        main.variables = ms.variables;
        main.variable_collections = ms.collections;
        let main_before = main.clone();

        let mut branch = base.clone();
        let mut bs = VariableStore::from_doc(&branch);
        assert!(bs.set_alias(&fg, &light, &bg));
        branch.variables = bs.variables;
        branch.variable_collections = bs.collections;

        let err = merge_branch(&base, &main, &branch, &Resolution::default()).expect_err("merge closing a direct alias cycle must be refused");
        assert!(err.to_string().to_ascii_lowercase().contains("cycle"), "error should mention cycle: {err}");
        assert_eq!(main.variables, main_before.variables, "main must remain unchanged when merge is refused");

        // 2. Extended collection override: main aliases parent bg -> fg, branch overrides fg -> bg in Brand.
        let mut branch_ext = base.clone();
        let mut bes = VariableStore::from_doc(&branch_ext);
        assert!(bes.set_alias(&fg, &brand_light, &bg));
        branch_ext.variables = bes.variables;
        branch_ext.variable_collections = bes.collections;

        let err_ext = merge_branch(&base, &main, &branch_ext, &Resolution::default())
            .expect_err("merge closing an alias cycle through an extended collection override must be refused");
        assert!(err_ext.to_string().to_ascii_lowercase().contains("cycle"), "error should mention cycle: {err_ext}");
    }
}
