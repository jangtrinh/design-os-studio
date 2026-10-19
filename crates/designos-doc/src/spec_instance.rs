//! Instance placement, variant resolution, and descendant overrides for `apply_spec` / `export_spec` (E15-1).

#![allow(clippy::result_large_err)]

use crate::spec::{estimate_intrinsic_text_size, parse_sizing};
use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Allowed top-level fields on a `NodeSpec` when `kind == "instance"`.
pub const ALLOWED_INSTANCE_SPEC_FIELDS: &[&str] = &[
    "id",
    "kind",
    "name",
    "component",
    "variant",
    "props",
    "overrides",
    "x",
    "y",
    "width",
    "height",
    "width_sizing",
    "height_sizing",
    "min_width",
    "max_width",
    "min_height",
    "max_height",
    "col_span",
    "row_span",
    "opacity",
    "dev_status",
    "annotation",
    "annotations",
];

/// Structured error envelope for `apply_spec` instance/variant/override failures (E15-1 / E15-5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpecError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub valid_options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_example: Option<Value>,
    pub safe_to_retry: bool,
}

impl SpecError {
    pub fn named(
        code: impl Into<String>,
        message: impl Into<String>,
        field_path: Option<String>,
        valid_options: Vec<String>,
        corrected_example: Option<Value>,
    ) -> Self {
        Self { code: code.into(), message: message.into(), field_path, valid_options, corrected_example, safe_to_retry: true }
    }

    pub fn plain(message: impl Into<String>) -> Self {
        Self { code: String::new(), message: message.into(), field_path: None, valid_options: Vec::new(), corrected_example: None, safe_to_retry: true }
    }
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.code.is_empty() {
            write!(f, "{}", self.message)
        } else {
            match serde_json::to_string(self) {
                Ok(s) => write!(f, "{s}"),
                Err(_) => write!(f, "{}: {}", self.code, self.message),
            }
        }
    }
}

impl std::error::Error for SpecError {}

/// Metadata for one placed component instance returned by `apply_spec_with_libraries`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacedInstanceInfo {
    pub node_id: u64,
    pub component_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_key: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variant: BTreeMap<String, String>,
}

/// Structured report produced by `apply_spec_with_libraries` / `apply_spec_command`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplySpecReport {
    pub root_node_id: NodeId,
    pub parent_id: NodeId,
    pub name_to_id: BTreeMap<String, u64>,
    pub instances: Vec<PlacedInstanceInfo>,
    pub imported_assets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale_library_assets: Vec<String>,
}

#[derive(Clone, Debug)]
struct ExternalCompEntry {
    raw_key: String,
    library_id: String,
    library_name: String,
    node_id: NodeId,
    name: String,
    stale: bool,
}

#[derive(Clone, Debug)]
struct SnapCompEntry {
    snap_idx: usize,
    library_id: String,
    library_name: String,
    asset_key: String,
    asset_name: String,
    snap_node: Option<NodeId>,
}

/// Per-call component lookup cache (O(components) one-time build per `apply_spec` call).
pub(crate) struct SpecBuildCtx<'a> {
    pub snapshots: &'a [LibrarySnapshot],
    local_by_name: HashMap<String, Vec<NodeId>>,
    local_list: Vec<(NodeId, String)>,
    ext_by_full_key: HashMap<String, ExternalCompEntry>,
    ext_by_raw_key: HashMap<String, Vec<ExternalCompEntry>>,
    ext_by_name: HashMap<String, Vec<ExternalCompEntry>>,
    snap_by_key: HashMap<String, Vec<SnapCompEntry>>,
    snap_by_name: HashMap<String, Vec<SnapCompEntry>>,
    snap_list: Vec<SnapCompEntry>,
    imported_in_call: HashMap<(String, String), NodeId>,
    pub import_cmds: Vec<Command>,
    pub imported_assets: Vec<String>,
    pub stale_library_assets: Vec<String>,
    pub instances: Vec<PlacedInstanceInfo>,
}

impl<'a> SpecBuildCtx<'a> {
    pub fn new(doc: &Document, snapshots: &'a [LibrarySnapshot]) -> Self {
        let enabled_snap_indices: Vec<usize> = snapshots
            .iter()
            .enumerate()
            .filter(|(_, s)| doc.library.enabled.is_empty() || doc.library.enabled.iter().any(|l| l.id == s.library_id))
            .map(|(i, _)| i)
            .collect();

        let local_list = local_component_roots(doc);
        let mut local_by_name: HashMap<String, Vec<NodeId>> = HashMap::with_capacity(local_list.len());
        for (id, name) in &local_list {
            local_by_name.entry(name.clone()).or_default().push(*id);
        }

        // Precompute staleness per enabled snapshot
        let mut stale_keys_by_lib: HashMap<String, BTreeSet<String>> = HashMap::new();
        for &sidx in &enabled_snap_indices {
            let snap = &snapshots[sidx];
            let updates = pending_updates(doc, snap);
            let set = stale_keys_by_lib.entry(snap.library_id.clone()).or_default();
            for u in updates {
                if matches!(u.change, UpdateKind::Changed | UpdateKind::Removed) {
                    set.insert(u.key);
                }
            }
        }

        let mut ext_by_full_key = HashMap::new();
        let mut ext_by_raw_key: HashMap<String, Vec<ExternalCompEntry>> = HashMap::new();
        let mut ext_by_name: HashMap<String, Vec<ExternalCompEntry>> = HashMap::new();

        for (ekey, e) in &doc.library.external {
            if !matches!(e.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet) {
                continue;
            }
            let lib_enabled = e.library_id.starts_with(REMOTE_LIBRARY_PREFIX)
                || doc.library.enabled.is_empty()
                || doc.library.enabled.iter().any(|l| l.id == e.library_id)
                || enabled_snap_indices.iter().any(|&i| snapshots[i].library_id == e.library_id);
            if !lib_enabled {
                continue;
            }
            let Some(node_id) = e.node.filter(|n| doc.get(*n).is_some()) else {
                continue;
            };
            let name = doc.get(node_id).map(|n| n.name.clone()).unwrap_or_default();
            let prefix = format!("{}/", e.library_id);
            let raw_key = ekey.strip_prefix(&prefix).unwrap_or(ekey).to_string();
            let library_name = doc
                .library
                .enabled
                .iter()
                .find(|l| l.id == e.library_id)
                .map(|l| l.name.clone())
                .or_else(|| enabled_snap_indices.iter().map(|&i| &snapshots[i]).find(|s| s.library_id == e.library_id).map(|s| s.name.clone()))
                .unwrap_or_else(|| e.library_id.strip_prefix(REMOTE_LIBRARY_PREFIX).unwrap_or(&e.library_id).to_string());
            let snap_version_diff =
                enabled_snap_indices.iter().map(|&i| &snapshots[i]).find(|s| s.library_id == e.library_id).is_some_and(|s| s.version != e.version);
            let stale = snap_version_diff || stale_keys_by_lib.get(&e.library_id).is_some_and(|s| s.contains(&raw_key));

            let entry = ExternalCompEntry { raw_key: raw_key.clone(), library_id: e.library_id.clone(), library_name, node_id, name: name.clone(), stale };
            ext_by_full_key.insert(ekey.clone(), entry.clone());
            ext_by_raw_key.entry(raw_key).or_default().push(entry.clone());
            ext_by_name.entry(name).or_default().push(entry);
        }

        let mut snap_by_key: HashMap<String, Vec<SnapCompEntry>> = HashMap::new();
        let mut snap_by_name: HashMap<String, Vec<SnapCompEntry>> = HashMap::new();
        let mut snap_list: Vec<SnapCompEntry> = Vec::new();

        for &sidx in &enabled_snap_indices {
            let snap = &snapshots[sidx];
            for asset in &snap.assets {
                if !matches!(asset.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet) {
                    continue;
                }
                let entry = SnapCompEntry {
                    snap_idx: sidx,
                    library_id: snap.library_id.clone(),
                    library_name: snap.name.clone(),
                    asset_key: asset.key.clone(),
                    asset_name: asset.name.clone(),
                    snap_node: asset.node,
                };
                snap_by_key.entry(asset.key.clone()).or_default().push(entry.clone());
                snap_by_name.entry(asset.name.clone()).or_default().push(entry.clone());
                snap_list.push(entry);
            }
        }

        Self {
            snapshots,
            local_by_name,
            local_list,
            ext_by_full_key,
            ext_by_raw_key,
            ext_by_name,
            snap_by_key,
            snap_by_name,
            snap_list,
            imported_in_call: HashMap::new(),
            import_cmds: Vec::new(),
            imported_assets: Vec::new(),
            stale_library_assets: Vec::new(),
            instances: Vec::new(),
        }
    }

    fn record_stale(&mut self, raw_key: &str) {
        if !self.stale_library_assets.iter().any(|k| k == raw_key) {
            self.stale_library_assets.push(raw_key.to_string());
        }
    }

    fn ensure_snap_imported(&mut self, doc: &mut Document, entry: &SnapCompEntry) -> Result<NodeId, SpecError> {
        let cache_key = (entry.library_id.clone(), entry.asset_key.clone());
        if let Some(&id) = self.imported_in_call.get(&cache_key) {
            return Ok(id);
        }
        let full_key = format!("{}/{}", entry.library_id, entry.asset_key);
        if let Some(existing) = self.ext_by_full_key.get(&full_key).cloned() {
            if existing.stale {
                self.record_stale(&existing.raw_key);
            }
            self.imported_in_call.insert(cache_key, existing.node_id);
            return Ok(existing.node_id);
        }
        if let Some(ext) = doc.library.external.get(&full_key) {
            if let Some(nid) = ext.node.filter(|n| doc.get(*n).is_some()) {
                self.imported_in_call.insert(cache_key, nid);
                return Ok(nid);
            }
        }

        let snap = &self.snapshots[entry.snap_idx];
        let cmd = import_asset(doc, snap, &entry.asset_key, None);
        cmd.clone().apply(doc).map_err(|e| SpecError::plain(e.to_string()))?;
        self.import_cmds.push(cmd);
        if !self.imported_assets.contains(&entry.asset_key) {
            self.imported_assets.push(entry.asset_key.clone());
        }

        let nid = doc
            .library
            .external
            .get(&full_key)
            .and_then(|e| e.node)
            .ok_or_else(|| SpecError::plain(format!("Failed to import library component '{}'", entry.asset_key)))?;
        self.imported_in_call.insert(cache_key, nid);
        Ok(nid)
    }

    fn closest_component_names(&self, query: &str) -> Vec<String> {
        let mut ranked: Vec<(usize, usize, usize, String)> = Vec::new();
        let mut seen = BTreeSet::new();
        for (idx, (_, name)) in self.local_list.iter().enumerate() {
            if seen.insert(name.clone()) {
                ranked.push((levenshtein(query, name), 0, idx, name.clone()));
            }
        }
        for (idx, s) in self.snap_list.iter().enumerate() {
            if seen.insert(s.asset_name.clone()) {
                ranked.push((levenshtein(query, &s.asset_name), 1 + s.snap_idx, idx, s.asset_name.clone()));
            }
        }
        for (idx, entries) in self.ext_by_name.values().enumerate() {
            for e in entries {
                if seen.insert(e.name.clone()) {
                    ranked.push((levenshtein(query, &e.name), 100, idx, e.name.clone()));
                }
            }
        }
        ranked.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        ranked.into_iter().take(5).map(|(_, _, _, n)| n).collect()
    }

    pub fn resolve_component(&mut self, doc: &mut Document, reference: &str, spec_for_example: Option<&NodeSpec>) -> Result<NodeId, SpecError> {
        let r = reference.trim();
        if r.is_empty() {
            let valid = self.closest_component_names("");
            let corrected = corrected_component_example(spec_for_example, valid.first());
            return Err(SpecError::named("UNKNOWN_COMPONENT", "Component reference is empty", Some("component".into()), valid, corrected));
        }

        // Rule 1: "node:<id>" (or "node:<id> (<library>)" from AMBIGUOUS_COMPONENT valid_options)
        if let Some(rest) = r.strip_prefix("node:") {
            let (id_part, lib_hint) = match rest.split_once(' ') {
                Some((id_str, hint)) => (id_str.trim(), Some(hint.trim().trim_start_matches('(').trim_end_matches(')'))),
                None => (rest.trim(), None),
            };
            if let Ok(raw_id) = id_part.parse::<u64>() {
                let target_id = NodeId(raw_id);
                if let Some(hint) = lib_hint {
                    if hint != "local" {
                        if let Some(s) =
                            self.snap_list.iter().find(|s| (s.library_name == hint || s.library_id == hint) && s.snap_node == Some(target_id)).cloned()
                        {
                            return self.ensure_snap_imported(doc, &s);
                        }
                    }
                }
                if let Some(n) = doc.get(target_id) {
                    if matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
                        return Ok(target_id);
                    }
                }
            }
            let valid = self.closest_component_names(r);
            let corrected = corrected_component_example(spec_for_example, valid.first());
            return Err(SpecError::named(
                "UNKNOWN_COMPONENT",
                format!("Component '{r}' not found or is not a Component/ComponentSet"),
                Some("component".into()),
                valid,
                corrected,
            ));
        }

        // Rule 2: Component key in Document.library.external or enabled LibrarySnapshot
        if let Some(ext) = self.ext_by_full_key.get(r).cloned() {
            if ext.stale {
                self.record_stale(&ext.raw_key);
            }
            return Ok(ext.node_id);
        }
        let ext_raw_matches = self.ext_by_raw_key.get(r).cloned().unwrap_or_default();
        let snap_key_matches = self.snap_by_key.get(r).cloned().unwrap_or_default();
        if !ext_raw_matches.is_empty() || !snap_key_matches.is_empty() {
            // Deduplicate by (library_id, raw_key)
            let mut combined: Vec<ResolvedLibraryHit> = Vec::new();
            for e in &ext_raw_matches {
                combined.push(ResolvedLibraryHit::External(e.clone()));
            }
            for s in &snap_key_matches {
                if !combined.iter().any(|h| h.library_id() == s.library_id && h.raw_key() == s.asset_key) {
                    combined.push(ResolvedLibraryHit::Snapshot(s.clone()));
                }
            }
            if combined.len() == 1 {
                return match &combined[0] {
                    ResolvedLibraryHit::External(e) => {
                        if e.stale {
                            self.record_stale(&e.raw_key);
                        }
                        Ok(e.node_id)
                    }
                    ResolvedLibraryHit::Snapshot(s) => self.ensure_snap_imported(doc, s),
                };
            } else if combined.len() > 1 {
                let valid: Vec<String> = combined.iter().map(|h| h.ambiguous_option()).collect();
                let corrected = corrected_component_example(spec_for_example, valid.first());
                return Err(SpecError::named(
                    "AMBIGUOUS_COMPONENT",
                    format!("Component key '{r}' matches multiple library components"),
                    Some("component".into()),
                    valid,
                    corrected,
                ));
            }
        }

        // Rule 3a: Exact, case-sensitive match on local components
        if let Some(local_ids) = self.local_by_name.get(r) {
            if local_ids.len() == 1 {
                return Ok(local_ids[0]);
            } else if local_ids.len() > 1 {
                let valid: Vec<String> = local_ids.iter().map(|id| format!("node:{}", id.0)).collect();
                let corrected = corrected_component_example(spec_for_example, valid.first());
                return Err(SpecError::named(
                    "AMBIGUOUS_COMPONENT",
                    format!("Component name '{r}' is ambiguous ({} local matches)", local_ids.len()),
                    Some("component".into()),
                    valid,
                    corrected,
                ));
            }
        }

        // Rule 3b: Exact match across all enabled libraries together
        let ext_name_matches = self.ext_by_name.get(r).cloned().unwrap_or_default();
        let snap_name_matches = self.snap_by_name.get(r).cloned().unwrap_or_default();
        if !ext_name_matches.is_empty() || !snap_name_matches.is_empty() {
            let mut combined: Vec<ResolvedLibraryHit> = Vec::new();
            for e in &ext_name_matches {
                combined.push(ResolvedLibraryHit::External(e.clone()));
            }
            for s in &snap_name_matches {
                if !combined.iter().any(|h| h.library_id() == s.library_id && h.raw_key() == s.asset_key) {
                    combined.push(ResolvedLibraryHit::Snapshot(s.clone()));
                }
            }
            if combined.len() == 1 {
                return match &combined[0] {
                    ResolvedLibraryHit::External(e) => {
                        if e.stale {
                            self.record_stale(&e.raw_key);
                        }
                        Ok(e.node_id)
                    }
                    ResolvedLibraryHit::Snapshot(s) => self.ensure_snap_imported(doc, s),
                };
            } else if combined.len() > 1 {
                let valid: Vec<String> = combined.iter().map(|h| h.ambiguous_option()).collect();
                let corrected = corrected_component_example(spec_for_example, valid.first());
                return Err(SpecError::named(
                    "AMBIGUOUS_COMPONENT",
                    format!("Component name '{r}' matches {} components across enabled libraries", combined.len()),
                    Some("component".into()),
                    valid,
                    corrected,
                ));
            }
        }

        // Rule 3c: Library prefix "<Library Name>/<Component Name>"
        if let Some((lib_prefix, comp_rest)) = r.split_once('/') {
            let mut combined: Vec<ResolvedLibraryHit> = Vec::new();
            if let Some(ext_list) = self.ext_by_name.get(comp_rest) {
                for e in ext_list {
                    if e.library_name == lib_prefix || e.library_id == lib_prefix {
                        combined.push(ResolvedLibraryHit::External(e.clone()));
                    }
                }
            }
            if let Some(snap_list) = self.snap_by_name.get(comp_rest) {
                for s in snap_list {
                    if (s.library_name == lib_prefix || s.library_id == lib_prefix)
                        && !combined.iter().any(|h| h.library_id() == s.library_id && h.raw_key() == s.asset_key)
                    {
                        combined.push(ResolvedLibraryHit::Snapshot(s.clone()));
                    }
                }
            }
            if combined.len() == 1 {
                return match &combined[0] {
                    ResolvedLibraryHit::External(e) => {
                        if e.stale {
                            self.record_stale(&e.raw_key);
                        }
                        Ok(e.node_id)
                    }
                    ResolvedLibraryHit::Snapshot(s) => self.ensure_snap_imported(doc, s),
                };
            } else if combined.len() > 1 {
                let valid: Vec<String> = combined.iter().map(|h| h.ambiguous_option()).collect();
                let corrected = corrected_component_example(spec_for_example, valid.first());
                return Err(SpecError::named(
                    "AMBIGUOUS_COMPONENT",
                    format!("Component '{r}' matches multiple components in library '{lib_prefix}'"),
                    Some("component".into()),
                    valid,
                    corrected,
                ));
            }
        }

        let valid = self.closest_component_names(r);
        let corrected = corrected_component_example(spec_for_example, valid.first());
        Err(SpecError::named("UNKNOWN_COMPONENT", format!("Component '{r}' not found"), Some("component".into()), valid, corrected))
    }
}

enum ResolvedLibraryHit {
    External(ExternalCompEntry),
    Snapshot(SnapCompEntry),
}

impl ResolvedLibraryHit {
    fn library_id(&self) -> &str {
        match self {
            Self::External(e) => &e.library_id,
            Self::Snapshot(s) => &s.library_id,
        }
    }
    fn raw_key(&self) -> &str {
        match self {
            Self::External(e) => &e.raw_key,
            Self::Snapshot(s) => &s.asset_key,
        }
    }
    fn ambiguous_option(&self) -> String {
        match self {
            Self::External(e) => format!("{}/{}", e.library_name, e.name),
            Self::Snapshot(s) => format!("{}/{}", s.library_name, s.asset_name),
        }
    }
}

fn corrected_component_example(spec: Option<&NodeSpec>, first_valid: Option<&String>) -> Option<Value> {
    let comp = first_valid?.clone();
    if let Some(s) = spec {
        let mut ex = json!({
            "kind": "instance",
            "component": comp,
        });
        if !s.variant.is_empty() {
            ex["variant"] = json!(s.variant);
        }
        Some(ex)
    } else {
        Some(json!({
            "kind": "instance",
            "component": comp,
        }))
    }
}

/// Resolves a component reference (`"node:<id>"`, component key, or component/library name)
/// against `doc` and `snapshots`, importing from an enabled snapshot if not yet copied locally.
pub fn resolve_component(doc: &mut Document, reference: &str, snapshots: &[LibrarySnapshot]) -> Result<NodeId, SpecError> {
    let mut ctx = SpecBuildCtx::new(doc, snapshots);
    ctx.resolve_component(doc, reference, None)
}

/// Resolves the target `Component` node for `resolved_id` (a `Component` or `ComponentSet`)
/// given `variant_map` (`{property: value}`).
pub fn resolve_variant_component(
    doc: &Document,
    resolved_id: NodeId,
    variant_map: &BTreeMap<String, String>,
    base_variant_id: Option<NodeId>,
    spec_for_example: Option<&NodeSpec>,
) -> Result<NodeId, SpecError> {
    let node = doc.get(resolved_id).ok_or_else(|| SpecError::plain(format!("Component node {} not found", resolved_id.0)))?;

    match &node.kind {
        NodeKind::ComponentSet(_) => {
            let variants = components_of(doc, resolved_id);
            if variants.is_empty() {
                return Err(SpecError::named(
                    "NO_SUCH_VARIANT",
                    format!("Component set '{}' has no variant components", node.name),
                    Some("variant".into()),
                    Vec::new(),
                    None,
                ));
            }
            if variant_map.is_empty() {
                return Ok(base_variant_id.unwrap_or(variants[0]));
            }

            let parsed: Vec<(NodeId, Vec<(String, String)>)> = variants
                .iter()
                .map(|&vid| {
                    let vname = doc.get(vid).map(|n| n.name.as_str()).unwrap_or("");
                    (vid, parse_variant_name(vname))
                })
                .collect();

            // Ordered property names and values in variant discovery order
            let mut prop_names: Vec<String> = Vec::new();
            let mut prop_values: HashMap<String, Vec<String>> = HashMap::new();
            for (_, pairs) in &parsed {
                for (k, v) in pairs {
                    if !prop_names.contains(k) {
                        prop_names.push(k.clone());
                    }
                    let vals = prop_values.entry(k.clone()).or_default();
                    if !vals.contains(v) {
                        vals.push(v.clone());
                    }
                }
            }

            let comp_label = spec_for_example.and_then(|s| s.component.clone()).unwrap_or_else(|| node.name.clone());

            // 1. Validate property names
            for k in variant_map.keys() {
                if !prop_names.contains(k) {
                    let corrected = prop_names.first().map(|first_prop| {
                        let mut new_var = BTreeMap::new();
                        for (ok, ov) in variant_map {
                            if ok == k {
                                let default_val = prop_values.get(first_prop).and_then(|v| v.first()).cloned().unwrap_or_else(|| ov.clone());
                                new_var.insert(first_prop.clone(), default_val);
                            } else {
                                new_var.insert(ok.clone(), ov.clone());
                            }
                        }
                        json!({
                            "kind": "instance",
                            "component": comp_label,
                            "variant": new_var,
                        })
                    });
                    return Err(SpecError::named(
                        "UNKNOWN_VARIANT_PROPERTY",
                        format!("Unknown variant property '{k}' on component set '{}'", node.name),
                        Some(format!("variant.{k}")),
                        prop_names,
                        corrected,
                    ));
                }
            }

            // 2. Validate property values
            for (k, v) in variant_map {
                let vals = prop_values.get(k).cloned().unwrap_or_default();
                if !vals.contains(v) {
                    let corrected = vals.first().map(|first_val| {
                        let mut new_var = variant_map.clone();
                        new_var.insert(k.clone(), first_val.clone());
                        json!({
                            "kind": "instance",
                            "component": comp_label,
                            "variant": new_var,
                        })
                    });
                    return Err(SpecError::named(
                        "UNKNOWN_VARIANT_VALUE",
                        format!("Unknown value '{v}' for variant property '{k}' on '{}'", node.name),
                        Some(format!("variant.{k}")),
                        vals,
                        corrected,
                    ));
                }
            }

            // 3. Build target pairs starting from base/default variant
            let base_id = base_variant_id.unwrap_or(variants[0]);
            let mut target_pairs = parsed.iter().find(|(vid, _)| *vid == base_id).map(|(_, p)| p.clone()).unwrap_or_else(|| parsed[0].1.clone());

            for (k, v) in variant_map {
                if let Some(slot) = target_pairs.iter_mut().find(|(pk, _)| pk == k) {
                    slot.1 = v.clone();
                } else {
                    target_pairs.push((k.clone(), v.clone()));
                }
            }

            let target_map: BTreeMap<&str, &str> = target_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

            if let Some((vid, _)) = parsed.iter().find(|(_, pairs)| {
                let m: BTreeMap<&str, &str> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                m == target_map
            }) {
                return Ok(*vid);
            }

            // 4. Combination missing -> NO_SUCH_VARIANT with 5 nearest combinations
            let mut nearest: Vec<(usize, usize, String, BTreeMap<String, String>)> = parsed
                .iter()
                .enumerate()
                .map(|(idx, (_, pairs))| {
                    let m: BTreeMap<&str, &str> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                    let diff = target_map.iter().filter(|(k, v)| m.get(*k) != Some(*v)).count() + m.keys().filter(|k| !target_map.contains_key(*k)).count();
                    let label = variant_name(pairs);
                    let map_owned: BTreeMap<String, String> = pairs.iter().cloned().collect();
                    (diff, idx, label, map_owned)
                })
                .collect();
            nearest.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            let valid_options: Vec<String> = nearest.iter().take(5).map(|(_, _, label, _)| label.clone()).collect();
            let corrected = nearest.first().map(|(_, _, _, map_owned)| {
                json!({
                    "kind": "instance",
                    "component": comp_label,
                    "variant": map_owned,
                })
            });
            Err(SpecError::named(
                "NO_SUCH_VARIANT",
                format!("No variant of '{}' matches {:?}", node.name, variant_map),
                Some("variant".into()),
                valid_options,
                corrected,
            ))
        }
        NodeKind::Component(_) => {
            if variant_map.is_empty() {
                return Ok(resolved_id);
            }
            // If this component is a variant inside a ComponentSet and base_variant_id is set,
            // delegate to its parent ComponentSet.
            if base_variant_id.is_some() {
                if let Some(parent_set) = node.parent.filter(|&p| matches!(doc.get(p).map(|x| &x.kind), Some(NodeKind::ComponentSet(_)))) {
                    return resolve_variant_component(doc, parent_set, variant_map, Some(resolved_id), spec_for_example);
                }
            }
            let own_pairs = parse_variant_name(&node.name);
            let own_map: BTreeMap<&str, &str> = own_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let prop_names: Vec<String> = own_pairs.iter().map(|(k, _)| k.clone()).collect();
            for (k, v) in variant_map {
                match own_map.get(k.as_str()) {
                    None => {
                        return Err(SpecError::named(
                            "UNKNOWN_VARIANT_PROPERTY",
                            format!("Component '{}' is not a component set and does not have variant property '{k}'", node.name),
                            Some(format!("variant.{k}")),
                            prop_names,
                            None,
                        ));
                    }
                    Some(&own_val) if own_val != v => {
                        return Err(SpecError::named(
                            "UNKNOWN_VARIANT_VALUE",
                            format!("Component '{}' has '{k}={own_val}', not '{v}'", node.name),
                            Some(format!("variant.{k}")),
                            vec![own_val.to_string()],
                            None,
                        ));
                    }
                    Some(_) => {}
                }
            }
            Ok(resolved_id)
        }
        _ => Err(SpecError::named(
            "UNKNOWN_COMPONENT",
            format!("Node {} is not a Component or ComponentSet", resolved_id.0),
            Some("component".into()),
            Vec::new(),
            None,
        )),
    }
}

/// Resolves a `/`-separated override path (supporting 1-based `Name[2]` sibling indexing)
/// from `instance_root` to a descendant `NodeId`.
pub fn resolve_override_path(doc: &Document, instance_root: NodeId, path: &str) -> Result<NodeId, SpecError> {
    if path.is_empty() {
        let valid = child_names_at(doc, instance_root);
        return Err(SpecError::named("UNKNOWN_OVERRIDE_PATH", "Override path cannot be empty", Some("overrides.path".into()), valid, None));
    }

    let mut cur = instance_root;
    for seg in path.split('/') {
        if seg.is_empty() {
            let valid = child_names_at(doc, cur);
            return Err(SpecError::named(
                "UNKNOWN_OVERRIDE_PATH",
                format!("Empty path segment in override path '{path}'"),
                Some("overrides.path".into()),
                valid,
                None,
            ));
        }

        let children = doc.get(cur).map(|n| n.children.clone()).unwrap_or_default();

        if let Some((base_name, idx_1based)) = parse_indexed_segment(seg) {
            let matches: Vec<NodeId> = children.iter().copied().filter(|&cid| doc.get(cid).is_some_and(|cn| cn.name == base_name)).collect();
            if idx_1based >= 1 && idx_1based <= matches.len() {
                cur = matches[idx_1based - 1];
                continue;
            }
            let valid = child_names_at(doc, cur);
            return Err(SpecError::named(
                "UNKNOWN_OVERRIDE_PATH",
                format!("Override path segment '{seg}' (in '{path}') did not match at node '{}'", doc.get(cur).map(|n| n.name.as_str()).unwrap_or("")),
                Some("overrides.path".into()),
                valid,
                None,
            ));
        }

        let matches: Vec<NodeId> = children.iter().copied().filter(|&cid| doc.get(cid).is_some_and(|cn| cn.name == seg)).collect();
        match matches.len() {
            0 => {
                let valid = child_names_at(doc, cur);
                return Err(SpecError::named(
                    "UNKNOWN_OVERRIDE_PATH",
                    format!("No child named '{seg}' (in path '{path}') under '{}'", doc.get(cur).map(|n| n.name.as_str()).unwrap_or("")),
                    Some("overrides.path".into()),
                    valid,
                    None,
                ));
            }
            1 => {
                cur = matches[0];
            }
            n => {
                let valid: Vec<String> = (1..=n).map(|i| format!("{seg}[{i}]")).collect();
                return Err(SpecError::named(
                    "AMBIGUOUS_OVERRIDE_PATH",
                    format!("Segment '{seg}' in path '{path}' matches {n} siblings; use '{seg}[1]'..'{seg}[{n}]'"),
                    Some("overrides.path".into()),
                    valid,
                    None,
                ));
            }
        }
    }

    Ok(cur)
}

fn parse_indexed_segment(seg: &str) -> Option<(&str, usize)> {
    let rest = seg.strip_suffix(']')?;
    let (base, idx_str) = rest.rsplit_once('[')?;
    if base.is_empty() || idx_str.is_empty() || !idx_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let idx = idx_str.parse::<usize>().ok()?;
    Some((base, idx))
}

fn child_names_at(doc: &Document, parent: NodeId) -> Vec<String> {
    let Some(p) = doc.get(parent) else {
        return Vec::new();
    };
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for &cid in &p.children {
        if let Some(cn) = doc.get(cid) {
            *counts.entry(cn.name.as_str()).or_insert(0) += 1;
        }
    }
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut out = Vec::with_capacity(p.children.len());
    for &cid in &p.children {
        if let Some(cn) = doc.get(cid) {
            let total = counts.get(cn.name.as_str()).copied().unwrap_or(1);
            if total > 1 {
                let idx = seen.entry(cn.name.as_str()).or_insert(0);
                *idx += 1;
                out.push(format!("{}[{}]", cn.name, *idx));
            } else {
                out.push(cn.name.clone());
            }
        }
    }
    out
}

fn resolve_override_target(doc: &Document, instance_root: NodeId, ov: &OverrideSpec) -> Result<NodeId, SpecError> {
    match (&ov.path, ov.id) {
        (Some(p), None) => resolve_override_path(doc, instance_root, p),
        (None, Some(src_raw)) => {
            let src_id = NodeId(src_raw);
            let mut matches = Vec::new();
            doc.walk(instance_root, |n, depth| {
                if depth > 0 && n.component.source == Some(src_id) {
                    matches.push(n.id);
                }
            });
            match matches.len() {
                0 => Err(SpecError::named(
                    "UNKNOWN_OVERRIDE_PATH",
                    format!("No descendant of instance {} mirrors main-component node {}", instance_root.0, src_raw),
                    Some("overrides.id".into()),
                    child_names_at(doc, instance_root),
                    None,
                )),
                1 => Ok(matches[0]),
                _ => Err(SpecError::named(
                    "AMBIGUOUS_OVERRIDE_PATH",
                    format!("Multiple descendants of instance {} mirror main-component node {}", instance_root.0, src_raw),
                    Some("overrides.id".into()),
                    child_names_at(doc, instance_root),
                    None,
                )),
            }
        }
        _ => Err(SpecError::named(
            "UNKNOWN_OVERRIDE_PATH",
            "Each override entry must specify exactly one of 'path' or 'id'",
            Some("overrides".into()),
            child_names_at(doc, instance_root),
            None,
        )),
    }
}

/// Applies component `props` (`Boolean`, `Text`, `InstanceSwap`) onto an instance node `instance_id`.
pub(crate) fn apply_instance_props_with_ctx(
    doc: &mut Document,
    instance_id: NodeId,
    props: &BTreeMap<String, Value>,
    ctx: &mut SpecBuildCtx<'_>,
) -> Result<(), SpecError> {
    if props.is_empty() {
        return Ok(());
    }
    let comp_id = doc.get(instance_id).and_then(instance_component).ok_or_else(|| {
        SpecError::named("OVERRIDE_FIELD_NOT_APPLICABLE", format!("Node {} is not an instance", instance_id.0), Some("props".into()), Vec::new(), None)
    })?;

    let defs = prop_defs(doc, comp_id);
    let non_variant_defs: Vec<&ComponentPropDef> = defs.iter().filter(|d| !matches!(d.kind, PropKind::Variant { .. } | PropKind::Slot)).collect();
    let valid_prop_names: Vec<String> = non_variant_defs.iter().map(|d| d.name.clone()).collect();

    for (key, raw_val) in props {
        let def = non_variant_defs
            .iter()
            .copied()
            .find(|d| d.name == *key)
            .or_else(|| non_variant_defs.iter().copied().find(|d| d.name.split('#').next() == Some(key.as_str())))
            .ok_or_else(|| {
                SpecError::named(
                    "UNKNOWN_COMPONENT_PROPERTY",
                    format!("Unknown component property '{key}'"),
                    Some(format!("props.{key}")),
                    valid_prop_names.clone(),
                    None,
                )
            })?;

        let pv = match &def.kind {
            PropKind::Boolean => {
                let b = raw_val.as_bool().ok_or_else(|| {
                    SpecError::named(
                        "PROPERTY_TYPE_MISMATCH",
                        format!("Property '{key}' expects a boolean, got {raw_val}"),
                        Some(format!("props.{key}")),
                        vec!["true".into(), "false".into()],
                        None,
                    )
                })?;
                PropValue::Bool(b)
            }
            PropKind::Text => {
                let s = raw_val.as_str().ok_or_else(|| {
                    SpecError::named(
                        "PROPERTY_TYPE_MISMATCH",
                        format!("Property '{key}' expects a string, got {raw_val}"),
                        Some(format!("props.{key}")),
                        vec!["string".into()],
                        None,
                    )
                })?;
                PropValue::Text(s.to_string())
            }
            PropKind::InstanceSwap { .. } => {
                let comp_ref = if let Some(s) = raw_val.as_str() {
                    s.to_string()
                } else if let Some(n) = raw_val.as_u64() {
                    format!("node:{n}")
                } else {
                    return Err(SpecError::named(
                        "PROPERTY_TYPE_MISMATCH",
                        format!("Property '{key}' expects a component reference string, got {raw_val}"),
                        Some(format!("props.{key}")),
                        vec!["component reference".into()],
                        None,
                    ));
                };
                let resolved = ctx.resolve_component(doc, &comp_ref, None)?;
                let target_comp = resolve_variant_component(doc, resolved, &BTreeMap::new(), None, None)?;
                PropValue::Instance(target_comp)
            }
            PropKind::Variant { .. } | PropKind::Slot => unreachable!(),
        };

        let def_name = def.name.clone();
        let cmd = set_instance_prop(doc, instance_id, &def_name, Some(pv));
        cmd.apply(doc).map_err(|e| SpecError::plain(e.to_string()))?;
    }
    Ok(())
}

/// Applies ordered `overrides` to descendants of `instance_root`.
pub fn apply_overrides(doc: &mut Document, instance_root: NodeId, overrides: &[OverrideSpec], snapshots: &[LibrarySnapshot]) -> Result<(), SpecError> {
    let mut ctx = SpecBuildCtx::new(doc, snapshots);
    apply_overrides_with_ctx(doc, instance_root, overrides, &mut ctx)
}

pub(crate) fn apply_overrides_with_ctx(
    doc: &mut Document,
    instance_root: NodeId,
    overrides: &[OverrideSpec],
    ctx: &mut SpecBuildCtx<'_>,
) -> Result<(), SpecError> {
    for ov in overrides {
        let target_id = resolve_override_target(doc, instance_root, ov)?;

        // 1. Instance swap (`swap`)
        let mut swapped_with_variant = false;
        if let Some(swap_ref) = &ov.swap {
            let is_inst = doc.get(target_id).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. }));
            if !is_inst {
                return Err(SpecError::named(
                    "OVERRIDE_FIELD_NOT_APPLICABLE",
                    format!("Cannot apply 'swap' override to non-instance layer '{}'", doc.get(target_id).map(|n| n.name.as_str()).unwrap_or("")),
                    Some("overrides.swap".into()),
                    Vec::new(),
                    None,
                ));
            }
            let resolved = ctx.resolve_component(doc, swap_ref, None)?;
            let new_comp = resolve_variant_component(doc, resolved, &ov.variant, None, None)?;
            if !ov.variant.is_empty() {
                swapped_with_variant = true;
            }
            if new_comp == target_id || doc.is_ancestor(new_comp, target_id) {
                return Err(SpecError::named(
                    "INSTANCE_CYCLE",
                    "Cannot swap nested instance to an ancestor component (cycle)",
                    Some("overrides.swap".into()),
                    Vec::new(),
                    None,
                ));
            }
            let prev_name = doc.get(target_id).map(|n| n.name.clone());
            let had_size_override = doc.get(target_id).is_some_and(|n| n.component.overrides.contains(&OverrideField::Size));
            let new_comp_size = doc.get(new_comp).map(|n| n.size);
            let cmd = swap_instance(doc, target_id, new_comp);
            cmd.apply(doc).map_err(|e| SpecError::plain(e.to_string()))?;

            if let Some(target_mut) = doc.get_mut(target_id) {
                if let Some(name) = prev_name {
                    target_mut.name = name;
                }
                if !had_size_override {
                    if let Some(sz) = new_comp_size {
                        target_mut.size = sz;
                    }
                }
                target_mut.component.overrides.insert(OverrideField::MainComponent);
            }
            let comp_key = library_key_for_component(doc, new_comp);
            if let Some(target_mut) = doc.get_mut(target_id) {
                if let NodeKind::Instance { component_key, .. } = &mut target_mut.kind {
                    *component_key = comp_key;
                }
            }
        }

        // 2. Variant switch on a nested instance (`variant`)
        if !ov.variant.is_empty() && !swapped_with_variant {
            let cur_comp = doc.get(target_id).and_then(instance_component).ok_or_else(|| {
                SpecError::named(
                    "OVERRIDE_FIELD_NOT_APPLICABLE",
                    format!("Cannot apply 'variant' override to non-instance layer '{}'", doc.get(target_id).map(|n| n.name.as_str()).unwrap_or("")),
                    Some("overrides.variant".into()),
                    Vec::new(),
                    None,
                )
            })?;
            let owner = doc
                .get(cur_comp)
                .and_then(|c| c.parent)
                .filter(|&p| matches!(doc.get(p).map(|x| &x.kind), Some(NodeKind::ComponentSet(_))))
                .unwrap_or(cur_comp);
            let new_comp = resolve_variant_component(doc, owner, &ov.variant, Some(cur_comp), None)?;
            let prev_name = doc.get(target_id).map(|n| n.name.clone());
            let had_size_override = doc.get(target_id).is_some_and(|n| n.component.overrides.contains(&OverrideField::Size));
            let new_comp_size = doc.get(new_comp).map(|n| n.size);
            for (k, v) in &ov.variant {
                let cmd = set_instance_prop(doc, target_id, k, Some(PropValue::Variant(v.clone())));
                cmd.apply(doc).map_err(|e| SpecError::plain(e.to_string()))?;
            }
            if new_comp != cur_comp {
                let cmd = swap_instance(doc, target_id, new_comp);
                cmd.apply(doc).map_err(|e| SpecError::plain(e.to_string()))?;
                if let Some(target_mut) = doc.get_mut(target_id) {
                    if let Some(name) = prev_name {
                        target_mut.name = name;
                    }
                    if !had_size_override {
                        if let Some(sz) = new_comp_size {
                            target_mut.size = sz;
                        }
                    }
                }
            }
        }

        // 3. Nested instance component properties (`props`)
        if !ov.props.is_empty() {
            let is_inst = doc.get(target_id).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. }));
            if !is_inst {
                return Err(SpecError::named(
                    "OVERRIDE_FIELD_NOT_APPLICABLE",
                    format!("Cannot apply 'props' override to non-instance layer '{}'", doc.get(target_id).map(|n| n.name.as_str()).unwrap_or("")),
                    Some("overrides.props".into()),
                    Vec::new(),
                    None,
                ));
            }
            apply_instance_props_with_ctx(doc, target_id, &ov.props, ctx)?;
        }

        // 4. Text characters (`text`)
        if let Some(text_val) = &ov.text {
            let is_text = doc.get(target_id).is_some_and(|n| matches!(n.kind, NodeKind::Text(_)));
            if !is_text {
                return Err(SpecError::named(
                    "OVERRIDE_FIELD_NOT_APPLICABLE",
                    format!("Cannot apply 'text' override to non-text layer '{}'", doc.get(target_id).map(|n| n.name.as_str()).unwrap_or("")),
                    Some("overrides.text".into()),
                    Vec::new(),
                    None,
                ));
            }
            if let Some(node) = doc.get_mut(target_id) {
                let has_size_override = node.component.overrides.contains(&OverrideField::Size);
                if let NodeKind::Text(tp) = &mut node.kind {
                    tp.content = text_val.clone();
                    tp.runs.clear();
                    if !has_size_override {
                        let est = estimate_intrinsic_text_size(tp);
                        match tp.auto_resize {
                            TextAutoResize::WidthAndHeight => {
                                node.size = est;
                            }
                            TextAutoResize::Height => {
                                let lines = (est.x / node.size.x.max(20.0)).ceil().max(1.0);
                                node.size.y = (lines * tp.style.font_size.max(8.0) * 1.45).ceil();
                            }
                            TextAutoResize::None | TextAutoResize::Truncate => {}
                        }
                    }
                }
                node.component.overrides.insert(OverrideField::Characters);
            }
        }

        // 5. Fill (`fill`)
        if let Some(fill_str) = &ov.fill {
            let paint_opt = if fill_str.eq_ignore_ascii_case("none") || fill_str.eq_ignore_ascii_case("transparent") {
                Some(Vec::new())
            } else {
                resolve_easeui_color(fill_str).map(|c| vec![Paint::solid(c)])
            };
            let paints = paint_opt.ok_or_else(|| {
                SpecError::named("OVERRIDE_FIELD_NOT_APPLICABLE", format!("Invalid fill color '{fill_str}'"), Some("overrides.fill".into()), Vec::new(), None)
            })?;
            if let Some(node) = doc.get_mut(target_id) {
                if let NodeKind::Text(tp) = &mut node.kind {
                    tp.style.fills = paints;
                } else {
                    node.fills = paints;
                }
                node.component.overrides.insert(OverrideField::Fills);
            }
        }

        // 6. Stroke (`stroke`)
        if let Some(stroke_str) = &ov.stroke {
            let paint_opt = if stroke_str.eq_ignore_ascii_case("none") || stroke_str.eq_ignore_ascii_case("transparent") {
                Some(Vec::new())
            } else {
                resolve_easeui_color(stroke_str).map(|c| vec![Paint::solid(c)])
            };
            let paints = paint_opt.ok_or_else(|| {
                SpecError::named(
                    "OVERRIDE_FIELD_NOT_APPLICABLE",
                    format!("Invalid stroke color '{stroke_str}'"),
                    Some("overrides.stroke".into()),
                    Vec::new(),
                    None,
                )
            })?;
            if let Some(node) = doc.get_mut(target_id) {
                let non_empty = !paints.is_empty();
                node.stroke.paints = paints;
                if non_empty && node.stroke.weight == 0.0 {
                    node.stroke.weight = 1.0;
                }
                node.component.overrides.insert(OverrideField::Stroke);
            }
        }

        // 7. Visibility (`visible`)
        if let Some(vis) = ov.visible {
            if let Some(node) = doc.get_mut(target_id) {
                node.visible = vis;
                node.component.overrides.insert(OverrideField::Visible);
            }
        }

        // 8. Opacity (`opacity`)
        if let Some(op) = ov.opacity {
            if let Some(node) = doc.get_mut(target_id) {
                node.opacity = op.clamp(0.0, 1.0);
                node.component.overrides.insert(OverrideField::Opacity);
            }
        }
    }

    Ok(())
}

fn library_key_for_component(doc: &Document, comp_id: NodeId) -> Option<String> {
    library_asset_of_component(doc, comp_id).map(|(ekey, e)| {
        let prefix = format!("{}/", e.library_id);
        ekey.strip_prefix(&prefix).unwrap_or(ekey).to_string()
    })
}

fn check_forbidden_instance_fields(spec: &NodeSpec) -> Result<(), SpecError> {
    let mut forbidden: Option<&str> = None;
    if !spec.children.is_empty() {
        forbidden = Some("children");
    } else if spec.preset.is_some() {
        forbidden = Some("preset");
    } else if spec.layout.is_some() {
        forbidden = Some("layout");
    } else if spec.text.is_some() {
        forbidden = Some("text");
    } else if spec.fill.is_some() {
        forbidden = Some("fill");
    }

    if let Some(field) = forbidden {
        let valid_options: Vec<String> = ALLOWED_INSTANCE_SPEC_FIELDS.iter().map(|s| (*s).to_string()).collect();
        let corrected = Some(json!({
            "kind": "instance",
            "component": spec.component.clone().unwrap_or_else(|| "Button".into()),
        }));
        return Err(SpecError::named(
            "FIELD_NOT_ALLOWED_ON_INSTANCE",
            format!("Field '{field}' is not allowed on an instance root; use 'overrides' for descendant edits"),
            Some(field.into()),
            valid_options,
            corrected,
        ));
    }
    Ok(())
}

impl Document {
    pub(crate) fn build_instance_spec_node(&mut self, parent: NodeId, spec: &NodeSpec, ctx: &mut SpecBuildCtx<'_>) -> Result<NodeId, SpecError> {
        check_forbidden_instance_fields(spec)?;

        let comp_ref = spec.component.as_deref().unwrap_or("");
        let resolved_id = ctx.resolve_component(self, comp_ref, Some(spec))?;
        let variant_comp_id = resolve_variant_component(self, resolved_id, &spec.variant, None, Some(spec))?;

        // Cycle check (§8.5): cannot instantiate a component inside itself or its ComponentSet
        if parent == variant_comp_id || self.is_ancestor(variant_comp_id, parent) || parent == resolved_id || self.is_ancestor(resolved_id, parent) {
            return Err(SpecError::named(
                "INSTANCE_CYCLE",
                format!("Cannot place an instance of component '{}' inside its own definition", comp_ref),
                Some("component".into()),
                Vec::new(),
                None,
            ));
        }

        let cmd = create_instance(self, variant_comp_id, parent, Vec2::new(spec.x, spec.y));
        let inst_id = match &cmd {
            Command::Insert { nodes, .. } => nodes.first().map(|n| n.id).ok_or_else(|| SpecError::plain("Failed to create instance"))?,
            _ => return Err(SpecError::plain("Failed to create instance")),
        };
        cmd.apply(self).map_err(|e| SpecError::plain(e.to_string()))?;

        let comp_key = library_key_for_component(self, variant_comp_id);
        let variant_pairs: BTreeMap<String, String> = self
            .get(variant_comp_id)
            .and_then(|c| {
                let in_set = c.parent.is_some_and(|p| matches!(self.get(p).map(|x| &x.kind), Some(NodeKind::ComponentSet(_))));
                in_set.then(|| parse_variant_name(&c.name).into_iter().collect())
            })
            .unwrap_or_default();

        if let Some(inst_node) = self.get_mut(inst_id) {
            if let NodeKind::Instance { component_key, .. } = &mut inst_node.kind {
                *component_key = comp_key.clone();
            }
            for (vk, vv) in &variant_pairs {
                inst_node.component.values.insert(vk.clone(), PropValue::Variant(vv.clone()));
            }
        }

        // Apply root component properties (`props`)
        apply_instance_props_with_ctx(self, inst_id, &spec.props, ctx)?;

        // Apply descendant overrides (`overrides`)
        apply_overrides_with_ctx(self, inst_id, &spec.overrides, ctx)?;

        // Apply root geometry, sizing, opacity, name, and dev metadata (§3.4)
        if let Some(inst_node) = self.get_mut(inst_id) {
            if !spec.name.is_empty() && spec.name != inst_node.name {
                inst_node.name = spec.name.clone();
            }
            if let Some(w_explicit) = spec.width {
                inst_node.size.x = w_explicit.max(0.0);
                inst_node.component.overrides.insert(OverrideField::Size);
                if spec.width_sizing.is_none() {
                    if let Some(al) = inst_node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                        if al.primary_is_horizontal() {
                            al.primary_sizing = Sizing::Fixed;
                        } else {
                            al.counter_sizing = Sizing::Fixed;
                        }
                    }
                }
            }
            if let Some(h_explicit) = spec.height {
                inst_node.size.y = h_explicit.max(0.0);
                inst_node.component.overrides.insert(OverrideField::Size);
                if spec.height_sizing.is_none() {
                    if let Some(al) = inst_node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                        if !al.primary_is_horizontal() {
                            al.primary_sizing = Sizing::Fixed;
                        } else {
                            al.counter_sizing = Sizing::Fixed;
                        }
                    }
                }
            }
            if (spec.opacity - 1.0).abs() >= 1e-6 {
                inst_node.opacity = spec.opacity.clamp(0.0, 1.0);
                inst_node.component.overrides.insert(OverrideField::Opacity);
            }
            if let Some(ws) = &spec.width_sizing {
                let s = parse_sizing(ws);
                inst_node.layout_child.horizontal = s;
                if let Some(al) = inst_node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                    if al.primary_is_horizontal() {
                        al.primary_sizing = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                    } else {
                        al.counter_sizing = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                    }
                }
            }
            if let Some(hs) = &spec.height_sizing {
                let s = parse_sizing(hs);
                inst_node.layout_child.vertical = s;
                if let Some(al) = inst_node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                    if !al.primary_is_horizontal() {
                        al.primary_sizing = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                    } else {
                        al.counter_sizing = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                    }
                }
            }
            inst_node.layout_child.min_w = spec.min_width;
            inst_node.layout_child.max_w = spec.max_width;
            inst_node.layout_child.min_h = spec.min_height;
            inst_node.layout_child.max_h = spec.max_height;
            if let Some(cs) = spec.col_span {
                inst_node.layout_child.col_span = cs;
            }
            if let Some(rs) = spec.row_span {
                inst_node.layout_child.row_span = rs;
            }
            if let Some(ann) = &spec.annotation {
                if !ann.trim().is_empty() {
                    inst_node.dev.annotations.push(Annotation { text: ann.clone(), properties: Vec::new(), category: None });
                }
            }
            for ann in &spec.annotations {
                inst_node.dev.annotations.push(Annotation { text: ann.clone(), properties: Vec::new(), category: None });
            }
            if let Some(status) = &spec.dev_status {
                match status.to_ascii_lowercase().as_str() {
                    "ready" | "ready_for_dev" | "readyfordev" => {
                        inst_node.dev.status = Some(DevStatus::ReadyForDev);
                    }
                    "completed" | "done" => {
                        inst_node.dev.status = Some(DevStatus::Completed);
                    }
                    _ => {}
                }
            }
        }

        ctx.instances.push(PlacedInstanceInfo { node_id: inst_id.0, component_id: variant_comp_id.0, component_key: comp_key, variant: variant_pairs });

        Ok(inst_id)
    }

    /// Materializes `spec` under `parent` with access to enabled library `snapshots`.
    /// Atomic: on any error `self` is unchanged.
    pub fn apply_spec_with_libraries(&mut self, parent: NodeId, spec: &NodeSpec, snapshots: &[LibrarySnapshot]) -> Result<ApplySpecReport, SpecError> {
        let (cmd, report) = apply_spec_command(self, parent, spec, snapshots)?;
        cmd.apply(self).map_err(|e| SpecError::plain(e.to_string()))?;
        let relayout_root =
            if self.get(parent).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).is_some() { parent } else { report.root_node_id };
        self.relayout_subtree(relayout_root);
        self.flush_pending_relayout();
        Ok(report)
    }

    pub(crate) fn export_instance_spec_node(&self, id: NodeId, all_comments: &[ResolvedCommentThread]) -> Result<NodeSpec, String> {
        let n = self.get(id).ok_or_else(|| format!("Node {} not found", id.0))?;
        let NodeKind::Instance { component, component_key, .. } = &n.kind else {
            return Err(format!("Node {} is not an instance", id.0));
        };

        let comp_ref =
            component_key.clone().filter(|k| !k.is_empty()).or_else(|| component.and_then(|cid| library_key_for_component(self, cid))).or_else(|| {
                component.map(|cid| {
                    let owner = self
                        .get(cid)
                        .and_then(|c| c.parent)
                        .filter(|&p| matches!(self.get(p).map(|x| &x.kind), Some(NodeKind::ComponentSet(_))))
                        .unwrap_or(cid);
                    format!("node:{}", owner.0)
                })
            });

        let variant: BTreeMap<String, String> = component
            .and_then(|cid| {
                let c = self.get(cid)?;
                let in_set = c.parent.is_some_and(|p| matches!(self.get(p).map(|x| &x.kind), Some(NodeKind::ComponentSet(_))));
                in_set.then(|| parse_variant_name(&c.name).into_iter().collect())
            })
            .unwrap_or_default();

        let mut props = BTreeMap::new();
        for (k, v) in &n.component.values {
            match v {
                PropValue::Bool(b) => {
                    props.insert(k.clone(), Value::Bool(*b));
                }
                PropValue::Text(s) => {
                    props.insert(k.clone(), Value::String(s.clone()));
                }
                PropValue::Instance(cid) => {
                    let s = library_key_for_component(self, *cid).unwrap_or_else(|| format!("node:{}", cid.0));
                    props.insert(k.clone(), Value::String(s));
                }
                PropValue::Variant(_) | PropValue::Slot(_) => {}
            }
        }

        let mut overrides = Vec::new();
        self.collect_exported_overrides(id, "", &mut overrides);

        let size_overridden = n.component.overrides.contains(&OverrideField::Size);
        let opacity = if n.component.overrides.contains(&OverrideField::Opacity) { n.opacity } else { 1.0 };
        let dev_status = n.dev.status.map(|s| s.label().to_string());
        let annotations: Vec<String> = n.dev.annotations.iter().map(|a| a.text.clone()).collect();
        let comments: Vec<ResolvedCommentThread> = all_comments
            .iter()
            .filter(|c| c.target_node_id == Some(id.0) || (c.target_node_id.is_none() && c.target_frame_id == Some(id.0)))
            .cloned()
            .collect();

        Ok(NodeSpec {
            id: Some(id.0),
            kind: "instance".into(),
            name: n.name.clone(),
            component: comp_ref,
            variant,
            props,
            overrides,
            preset: None,
            x: n.transform.tx,
            y: n.transform.ty,
            width: size_overridden.then_some(n.size.x),
            height: size_overridden.then_some(n.size.y),
            width_sizing: (n.layout_child.horizontal != Sizing::Fixed).then(|| format!("{:?}", n.layout_child.horizontal).to_ascii_lowercase()),
            height_sizing: (n.layout_child.vertical != Sizing::Fixed).then(|| format!("{:?}", n.layout_child.vertical).to_ascii_lowercase()),
            min_width: n.layout_child.min_w,
            max_width: n.layout_child.max_w,
            min_height: n.layout_child.min_h,
            max_height: n.layout_child.max_h,
            col_span: (n.layout_child.col_span > 1).then_some(n.layout_child.col_span),
            row_span: (n.layout_child.row_span > 1).then_some(n.layout_child.row_span),
            fill: None,
            stroke: None,
            stroke_weight: None,
            corner_radius: None,
            corner_radii: None,
            opacity,
            clips_content: None,
            text: None,
            font_size: None,
            font_weight: None,
            text_color: None,
            layout: None,
            dev_status,
            annotation: None,
            annotations,
            comments,
            children: Vec::new(),
        })
    }

    fn collect_exported_overrides(&self, parent: NodeId, prefix: &str, out: &mut Vec<OverrideSpec>) {
        let Some(pnode) = self.get(parent) else {
            return;
        };
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for &cid in &pnode.children {
            if let Some(cn) = self.get(cid) {
                *counts.entry(cn.name.as_str()).or_insert(0) += 1;
            }
        }
        let mut seen: HashMap<&str, usize> = HashMap::new();

        for &cid in &pnode.children {
            let Some(cn) = self.get(cid) else {
                continue;
            };
            let total = counts.get(cn.name.as_str()).copied().unwrap_or(1);
            let seg = if total > 1 {
                let idx = seen.entry(cn.name.as_str()).or_insert(0);
                *idx += 1;
                format!("{}[{}]", cn.name, *idx)
            } else {
                cn.name.clone()
            };
            let rel_path = if prefix.is_empty() { seg } else { format!("{prefix}/{seg}") };

            let ov_set = &cn.component.overrides;
            let mut spec_ov = OverrideSpec { path: Some(rel_path.clone()), ..OverrideSpec::default() };
            let mut any_field = false;

            if ov_set.contains(&OverrideField::MainComponent) {
                if let NodeKind::Instance { component: Some(comp_id), component_key, .. } = &cn.kind {
                    spec_ov.swap = component_key
                        .clone()
                        .filter(|k| !k.is_empty())
                        .or_else(|| library_key_for_component(self, *comp_id))
                        .or_else(|| {
                            self.get(*comp_id).and_then(|c| {
                                let in_set = c.parent.is_some_and(|p| matches!(self.get(p).map(|x| &x.kind), Some(NodeKind::ComponentSet(_))));
                                (!in_set && !c.name.is_empty()).then(|| c.name.clone())
                            })
                        })
                        .or_else(|| Some(format!("node:{}", comp_id.0)));
                    any_field = true;
                }
            }

            if matches!(cn.kind, NodeKind::Instance { .. }) && !cn.component.values.is_empty() {
                for (k, v) in &cn.component.values {
                    match v {
                        PropValue::Variant(val) => {
                            spec_ov.variant.insert(k.clone(), val.clone());
                            any_field = true;
                        }
                        PropValue::Bool(b) => {
                            spec_ov.props.insert(k.clone(), Value::Bool(*b));
                            any_field = true;
                        }
                        PropValue::Text(s) => {
                            spec_ov.props.insert(k.clone(), Value::String(s.clone()));
                            any_field = true;
                        }
                        PropValue::Instance(cid) => {
                            let s = library_key_for_component(self, *cid).unwrap_or_else(|| format!("node:{}", cid.0));
                            spec_ov.props.insert(k.clone(), Value::String(s));
                            any_field = true;
                        }
                        PropValue::Slot(_) => {}
                    }
                }
            }

            if ov_set.contains(&OverrideField::Characters) {
                if let NodeKind::Text(tp) = &cn.kind {
                    spec_ov.text = Some(tp.content.clone());
                    any_field = true;
                }
            }
            if ov_set.contains(&OverrideField::Fills) {
                let first_paint = match &cn.kind {
                    NodeKind::Text(tp) => tp.style.fills.first(),
                    _ => cn.fills.first(),
                };
                spec_ov.fill = Some(
                    first_paint
                        .and_then(|p| match &p.kind {
                            PaintKind::Solid(c) => Some(format!("#{}", c.to_hex())),
                            _ => None,
                        })
                        .unwrap_or_else(|| "none".into()),
                );
                any_field = true;
            }
            if ov_set.contains(&OverrideField::Stroke) {
                spec_ov.stroke = Some(
                    cn.stroke
                        .paints
                        .first()
                        .and_then(|p| match &p.kind {
                            PaintKind::Solid(c) => Some(format!("#{}", c.to_hex())),
                            _ => None,
                        })
                        .unwrap_or_else(|| "none".into()),
                );
                any_field = true;
            }
            if ov_set.contains(&OverrideField::Visible) {
                spec_ov.visible = Some(cn.visible);
                any_field = true;
            }
            if ov_set.contains(&OverrideField::Opacity) {
                spec_ov.opacity = Some(cn.opacity);
                any_field = true;
            }

            if any_field {
                out.push(spec_ov);
            }

            self.collect_exported_overrides(cid, &rel_path, out);
        }
    }
}

/// Builds `spec` on a trial clone of `doc` and returns a single undoable [`Command`]
/// (combining any library `import_asset` commands and the subtree `Command::Insert`)
/// along with [`ApplySpecReport`]. On any error, `doc` is untouched.
pub fn apply_spec_command(doc: &mut Document, parent: NodeId, spec: &NodeSpec, snapshots: &[LibrarySnapshot]) -> Result<(Command, ApplySpecReport), SpecError> {
    let pnode = doc.get(parent).ok_or_else(|| SpecError::plain(format!("Parent node {} not found", parent.0)))?;
    if !pnode.kind.is_container() {
        return Err(SpecError::plain(format!("Parent node {} is not a container", parent.0)));
    }

    let mut trial = doc.clone();
    let mut ctx = SpecBuildCtx::new(&trial, snapshots);
    let root_id = trial.build_spec_node_with_ctx(parent, spec, &mut ctx)?;

    let relayout_root = if trial.get(parent).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).is_some() { parent } else { root_id };
    trial.relayout_subtree(relayout_root);
    trial.flush_pending_relayout();

    let mut nodes = Vec::new();
    trial.walk(root_id, |n, _| nodes.push(n.clone()));
    let index = doc.get(parent).ok_or_else(|| SpecError::plain("parent not found"))?.children.len();
    let insert_cmd = Command::Insert { parent, index, nodes };

    // Build slash-separated name_to_id map from the spec root
    let mut name_to_id = BTreeMap::new();
    populate_name_to_id(&trial, root_id, "", &mut name_to_id);

    // If library assets were imported during the trial, ensure `doc` has the empty library page
    // container and any copied images so `import_cmds` apply cleanly on `doc`.
    let final_cmd = if ctx.import_cmds.is_empty() {
        insert_cmd
    } else {
        if let Some(lp_id) = trial.library_page {
            if let std::collections::btree_map::Entry::Vacant(e) = doc.nodes.entry(lp_id) {
                let mut lp_node = Node::new(lp_id, NodeKind::Page { background: Color::rgb(0x5A, 0x5A, 0x5A) });
                lp_node.name = "Library".into();
                e.insert(lp_node);
                doc.library_page = Some(lp_id);
                doc.bump_ids();
            }
        }
        for (k, v) in &trial.images {
            doc.images.entry(k.clone()).or_insert_with(|| v.clone());
        }
        let mut cmds = ctx.import_cmds;
        cmds.push(insert_cmd);
        Command::Batch(cmds)
    };

    let report = ApplySpecReport {
        root_node_id: root_id,
        parent_id: parent,
        name_to_id,
        instances: ctx.instances,
        imported_assets: ctx.imported_assets,
        stale_library_assets: ctx.stale_library_assets,
    };

    Ok((final_cmd, report))
}

fn populate_name_to_id(doc: &Document, id: NodeId, prefix: &str, out: &mut BTreeMap<String, u64>) {
    let Some(n) = doc.get(id) else {
        return;
    };
    let path = if prefix.is_empty() { n.name.clone() } else { format!("{prefix}/{}", n.name) };
    if !path.is_empty() {
        out.entry(path.clone()).or_insert(id.0);
    }
    for &cid in &n.children {
        populate_name_to_id(doc, cid, &path, out);
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Code-built fixtures for E15-1 component instance tests (§10).
pub mod fixtures {
    use crate::*;

    /// Returns a `Document` containing:
    /// - A base icon component `"Icon/Default"` and a second component `"Icon/Arrow"`.
    /// - A `ComponentSet` `"Button"` with variants `Size=sm|md|lg` x `State=default|hover`,
    ///   each holding a text `"Label"` and a nested instance `"Icon"`.
    pub fn button_library() -> Document {
        let mut doc = Document::new("Button Library");
        let page = doc.add_page("Components");

        // 1. "Icon/Default" component
        let icon_default_id = doc.alloc_id();
        let mut icon_default = Node::new(icon_default_id, NodeKind::Component(FrameProps::default()));
        icon_default.name = "Icon/Default".into();
        icon_default.size = Vec2::new(16.0, 16.0);
        icon_default.fills = vec![Paint::solid(Color::rgb(0xA1, 0xA1, 0xAA))];
        doc.insert(page, icon_default).unwrap();

        // 2. "Icon/Arrow" component
        let icon_arrow_id = doc.alloc_id();
        let mut icon_arrow = Node::new(icon_arrow_id, NodeKind::Component(FrameProps::default()));
        icon_arrow.name = "Icon/Arrow".into();
        icon_arrow.size = Vec2::new(20.0, 20.0);
        icon_arrow.fills = vec![Paint::solid(Color::rgb(0xE3, 0x63, 0x23))];
        doc.insert(page, icon_arrow).unwrap();

        // 3. "Button" ComponentSet with Size=sm|md|lg x State=default|hover
        let set_id = doc.alloc_id();
        let mut set_node = Node::new(set_id, NodeKind::ComponentSet(FrameProps::default()));
        set_node.name = "Button".into();
        set_node.size = Vec2::new(400.0, 240.0);
        set_node.component.props = vec![
            ComponentPropDef {
                name: "Size".into(),
                kind: PropKind::Variant { options: vec!["sm".into(), "md".into(), "lg".into()] },
                default: PropValue::Variant("sm".into()),
            },
            ComponentPropDef {
                name: "State".into(),
                kind: PropKind::Variant { options: vec!["default".into(), "hover".into()] },
                default: PropValue::Variant("default".into()),
            },
        ];
        doc.insert(page, set_node).unwrap();

        for (s_idx, size) in ["sm", "md", "lg"].iter().enumerate() {
            for (st_idx, state) in ["default", "hover"].iter().enumerate() {
                let vid = doc.alloc_id();
                let mut vnode = Node::new(vid, NodeKind::Component(FrameProps::default()));
                vnode.name = format!("Size={size}, State={state}");
                let w = match *size {
                    "sm" => 88.0,
                    "md" => 112.0,
                    _ => 140.0,
                };
                let h = match *size {
                    "sm" => 32.0,
                    "md" => 40.0,
                    _ => 48.0,
                };
                vnode.size = Vec2::new(w, h);
                vnode.transform = Affine::translate(s_idx as f32 * 150.0, st_idx as f32 * 60.0);
                vnode.fills = vec![Paint::solid(if *state == "hover" { Color::rgb(0xF0, 0x70, 0x30) } else { Color::rgb(0xE3, 0x63, 0x23) })];
                doc.insert(set_id, vnode).unwrap();

                // Child 1: Text "Label"
                let lid = doc.alloc_id();
                let mut label = Node::text(lid, Rect::new(12.0, 8.0, 48.0, 20.0), "Button", TextStyle::default());
                label.name = "Label".into();
                doc.insert(vid, label).unwrap();

                // Child 2: Nested instance "Icon" pointing at "Icon/Default"
                let icmd = create_instance(&mut doc, icon_default_id, vid, Vec2::new(64.0, 8.0));
                let icon_inst_id = match &icmd {
                    Command::Insert { nodes, .. } => nodes[0].id,
                    _ => unreachable!(),
                };
                icmd.apply(&mut doc).unwrap();
                if let Some(inode) = doc.get_mut(icon_inst_id) {
                    inode.name = "Icon".into();
                }
            }
        }

        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn find_variant_id(doc: &Document, set_name: &str, variant_label: &str) -> NodeId {
        let set_id = doc.nodes.values().find(|n| matches!(n.kind, NodeKind::ComponentSet(_)) && n.name == set_name).unwrap().id;
        components_of(doc, set_id).into_iter().find(|&vid| doc.get(vid).unwrap().name == variant_label).unwrap()
    }

    #[test]
    fn places_linked_variant() {
        let mut doc = fixtures::button_library();
        let page = doc.pages[0];
        let expected_variant = find_variant_id(&doc, "Button", "Size=lg, State=default");

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "Button",
            "variant": { "Size": "lg" }
        }))
        .unwrap();

        let inst_id = doc.apply_spec(page, &spec).unwrap();
        let inst = doc.get(inst_id).unwrap();
        assert_eq!(instance_component(inst), Some(expected_variant));
        assert!(is_linked(inst), "placed instance must remain linked, not detached");
        for &cid in &inst.children {
            let child = doc.get(cid).unwrap();
            assert!(child.component.source.is_some(), "descendant '{}' must keep component.source link", child.name);
        }
    }

    #[test]
    fn name_path_override_only_touches_target() {
        let mut doc = fixtures::button_library();
        let page = doc.pages[0];

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "Button",
            "variant": { "Size": "lg" },
            "overrides": [
                { "path": "Label", "text": "Buy" }
            ]
        }))
        .unwrap();

        let inst_id = doc.apply_spec(page, &spec).unwrap();
        let label_id = resolve_override_path(&doc, inst_id, "Label").unwrap();
        let icon_id = resolve_override_path(&doc, inst_id, "Icon").unwrap();

        let label = doc.get(label_id).unwrap();
        match &label.kind {
            NodeKind::Text(tp) => assert_eq!(tp.content, "Buy"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(label.component.overrides.contains(&OverrideField::Characters));

        let icon = doc.get(icon_id).unwrap();
        assert!(icon.component.overrides.is_empty(), "Icon must have no overrides, got {:?}", icon.component.overrides);
    }

    #[test]
    fn invalid_variant_value_is_named_and_atomic() {
        let mut doc = fixtures::button_library();
        let page = doc.pages[0];
        let mut history = History::default();
        let before_json = doc.to_json().unwrap();

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "Button",
            "variant": { "Size": "xl" }
        }))
        .unwrap();

        let err = doc.apply_spec_with_libraries(page, &spec, &[]).expect_err("Size=xl must fail with UNKNOWN_VARIANT_VALUE");
        assert_eq!(err.code, "UNKNOWN_VARIANT_VALUE");
        assert_eq!(err.valid_options, vec!["sm".to_string(), "md".to_string(), "lg".to_string()]);
        assert!(err.safe_to_retry);
        assert_eq!(
            err.corrected_example,
            Some(json!({
                "kind": "instance",
                "component": "Button",
                "variant": { "Size": "sm" }
            }))
        );

        let after_json = doc.to_json().unwrap();
        assert_eq!(before_json, after_json, "document must be unchanged on error");
        assert!(!history.can_undo(), "history depth must be unchanged");
        let _ = &mut history;
    }

    #[test]
    fn swap_nested_instance() {
        let mut doc = fixtures::button_library();
        let page = doc.pages[0];
        let arrow_id = doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Icon/Arrow").unwrap().id;

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "Button",
            "overrides": [
                { "path": "Icon", "swap": "Icon/Arrow" }
            ]
        }))
        .unwrap();

        let inst_id = doc.apply_spec(page, &spec).unwrap();
        let icon_id = resolve_override_path(&doc, inst_id, "Icon").unwrap();
        let icon = doc.get(icon_id).unwrap();
        assert_eq!(instance_component(icon), Some(arrow_id));
        assert!(icon.component.overrides.contains(&OverrideField::MainComponent));
        // §8.4: Icon had no Size override, so swapping to 20x20 Icon/Arrow adopts 20x20
        assert_eq!(icon.size, Vec2::new(20.0, 20.0));
    }

    #[test]
    fn library_component_imports_once() {
        let lib_doc = fixtures::button_library();
        let mut snap = snapshot(&lib_doc, "lib_ui", "UI Kit", 1);
        // Set deterministic key "k1" on the Button ComponentSet asset in the snapshot
        let button_asset = snap.assets.iter_mut().find(|a| a.name == "Button").unwrap();
        button_asset.key = "k1".into();

        let mut consumer = Document::new("Consumer");
        let page = consumer.add_page("Page 1");
        consumer.library.enabled.push(LibraryRef { id: "lib_ui".into(), name: "UI Kit".into(), version: 1 });

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "frame",
            "name": "Toolbar",
            "children": [
                {
                    "kind": "instance",
                    "name": "Primary",
                    "component": "k1",
                    "variant": { "Size": "sm" }
                },
                {
                    "kind": "instance",
                    "name": "Secondary",
                    "component": "k1",
                    "variant": { "Size": "lg" }
                }
            ]
        }))
        .unwrap();

        let mut history = History::default();
        let (cmd, report) = apply_spec_command(&mut consumer, page, &spec, &[snap]).unwrap();
        history.exec(&mut consumer, cmd).unwrap();

        assert_eq!(report.imported_assets, vec!["k1".to_string()]);
        assert_eq!(report.instances.len(), 2);

        let lp = consumer.library_page.expect("library_page must exist");
        let button_sets: Vec<NodeId> = consumer
            .get(lp)
            .unwrap()
            .children
            .iter()
            .copied()
            .filter(|&cid| consumer.get(cid).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet(_)) && n.name == "Button"))
            .collect();
        assert_eq!(button_sets.len(), 1, "component set must be imported once onto library_page");
        let imported_set_id = button_sets[0];

        let inst1 = NodeId(report.instances[0].node_id);
        let inst2 = NodeId(report.instances[1].node_id);
        let comp1 = instance_component(consumer.get(inst1).unwrap()).unwrap();
        let comp2 = instance_component(consumer.get(inst2).unwrap()).unwrap();
        assert_eq!(consumer.get(comp1).unwrap().parent, Some(imported_set_id));
        assert_eq!(consumer.get(comp2).unwrap().parent, Some(imported_set_id));

        // One undo step removes both placed instances and the imported library copy
        assert!(history.undo(&mut consumer).unwrap());
        assert!(consumer.get(page).unwrap().children.is_empty());
        assert!(consumer.library.external.is_empty());
        assert!(consumer.get(lp).unwrap().children.is_empty());
    }

    #[test]
    fn round_trip_designos_and_fig() {
        let mut doc = fixtures::button_library();
        let page = doc.pages[0];
        let expected_variant = find_variant_id(&doc, "Button", "Size=lg, State=hover");

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "name": "CTA",
            "component": "Button",
            "variant": { "Size": "lg", "State": "hover" },
            "overrides": [
                { "path": "Label", "text": "Launch" }
            ]
        }))
        .unwrap();

        let inst_id = doc.apply_spec(page, &spec).unwrap();
        let json_str = doc.to_json().unwrap();
        let restored = Document::from_json(&json_str).unwrap();

        let restored_inst = restored.get(inst_id).unwrap();
        assert_eq!(instance_component(restored_inst), Some(expected_variant));
        let restored_comp = restored.get(expected_variant).unwrap();
        assert_eq!(restored_comp.name, "Size=lg, State=hover");
        let label_id = resolve_override_path(&restored, inst_id, "Label").unwrap();
        match &restored.get(label_id).unwrap().kind {
            NodeKind::Text(tp) => assert_eq!(tp.content, "Launch"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn export_then_apply_round_trips() {
        let mut doc = fixtures::button_library();
        let page = doc.pages[0];

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "name": "Buy Button",
            "component": "Button",
            "variant": { "Size": "md", "State": "hover" },
            "overrides": [
                { "path": "Label", "text": "Checkout", "fill": "#FFFFFF" },
                { "path": "Icon", "swap": "Icon/Arrow", "opacity": 0.8 }
            ]
        }))
        .unwrap();

        let inst1 = doc.apply_spec(page, &spec).unwrap();
        let exported = doc.export_spec(inst1).unwrap();
        assert_eq!(exported.kind, "instance");

        let inst2 = doc.apply_spec(page, &exported).unwrap();
        assert_eq!(instance_component(doc.get(inst1).unwrap()), instance_component(doc.get(inst2).unwrap()));

        for rel in ["Label", "Icon"] {
            let n1 = doc.get(resolve_override_path(&doc, inst1, rel).unwrap()).unwrap();
            let n2 = doc.get(resolve_override_path(&doc, inst2, rel).unwrap()).unwrap();
            assert_eq!(n1.component.overrides, n2.component.overrides, "override set mismatch on '{rel}'");
        }
    }

    #[test]
    fn zero_variants_component_set_rejected() {
        let mut doc = Document::new("Empty Set");
        let page = doc.add_page("Page 1");
        let set_id = doc.alloc_id();
        let mut set_node = Node::new(set_id, NodeKind::ComponentSet(FrameProps::default()));
        set_node.name = "EmptySet".into();
        doc.insert(page, set_node).unwrap();

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "EmptySet"
        }))
        .unwrap();

        let err = doc.apply_spec_with_libraries(page, &spec, &[]).unwrap_err();
        assert_eq!(err.code, "NO_SUCH_VARIANT");
    }

    #[test]
    fn ambiguous_across_two_libraries() {
        let lib1_doc = fixtures::button_library();
        let snap1 = snapshot(&lib1_doc, "lib_a", "Library A", 1);
        let lib2_doc = fixtures::button_library();
        let snap2 = snapshot(&lib2_doc, "lib_b", "Library B", 1);

        let mut consumer = Document::new("Consumer");
        let page = consumer.add_page("Page 1");
        consumer.library.enabled.push(LibraryRef { id: "lib_a".into(), name: "Library A".into(), version: 1 });
        consumer.library.enabled.push(LibraryRef { id: "lib_b".into(), name: "Library B".into(), version: 1 });

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "Button"
        }))
        .unwrap();

        let err = consumer.apply_spec_with_libraries(page, &spec, &[snap1, snap2]).unwrap_err();
        assert_eq!(err.code, "AMBIGUOUS_COMPONENT");
        assert!(err.valid_options.contains(&"Library A/Button".to_string()));
        assert!(err.valid_options.contains(&"Library B/Button".to_string()));

        // Qualified with library name succeeds
        let qualified: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "Library A/Button"
        }))
        .unwrap();
        let snap1_again = snapshot(&lib1_doc, "lib_a", "Library A", 1);
        let snap2_again = snapshot(&lib2_doc, "lib_b", "Library B", 1);
        let report = consumer.apply_spec_with_libraries(page, &qualified, &[snap1_again, snap2_again]).unwrap();
        assert_eq!(report.instances.len(), 1);
    }

    #[test]
    fn swap_preserves_size_only_when_overridden() {
        let mut doc = fixtures::button_library();
        let page = doc.pages[0];

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "Button"
        }))
        .unwrap();

        let inst_id = doc.apply_spec(page, &spec).unwrap();
        let icon_id = resolve_override_path(&doc, inst_id, "Icon").unwrap();
        // Give the nested Icon instance an explicit Size override before swapping
        {
            let icon_mut = doc.get_mut(icon_id).unwrap();
            icon_mut.size = Vec2::new(24.0, 24.0);
            icon_mut.component.overrides.insert(OverrideField::Size);
        }
        let swap_ov: OverrideSpec = serde_json::from_value(json!({
            "path": "Icon",
            "swap": "Icon/Arrow"
        }))
        .unwrap();
        apply_overrides(&mut doc, inst_id, &[swap_ov], &[]).unwrap();

        let icon = doc.get(icon_id).unwrap();
        assert_eq!(icon.size, Vec2::new(24.0, 24.0));
        assert!(icon.component.overrides.contains(&OverrideField::Size));
        assert!(icon.component.overrides.contains(&OverrideField::MainComponent));
    }

    #[test]
    fn instance_cycle_rejected() {
        let mut doc = fixtures::button_library();
        let set_id = doc.nodes.values().find(|n| matches!(n.kind, NodeKind::ComponentSet(_)) && n.name == "Button").unwrap().id;

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "Button"
        }))
        .unwrap();

        let err = doc.apply_spec_with_libraries(set_id, &spec, &[]).unwrap_err();
        assert_eq!(err.code, "INSTANCE_CYCLE");
    }

    #[test]
    fn stale_library_copy_reported() {
        let lib_doc = fixtures::button_library();
        let mut snap_v1 = snapshot(&lib_doc, "lib_ui", "UI Kit", 1);
        snap_v1.assets.iter_mut().find(|a| a.name == "Button").unwrap().key = "k_btn".into();

        let mut consumer = Document::new("Consumer");
        let page = consumer.add_page("Page 1");
        consumer.library.enabled.push(LibraryRef { id: "lib_ui".into(), name: "UI Kit".into(), version: 1 });

        let spec: NodeSpec = serde_json::from_value(json!({
            "kind": "instance",
            "component": "k_btn"
        }))
        .unwrap();
        consumer.apply_spec_with_libraries(page, &spec, &[snap_v1]).unwrap();

        // Newer snapshot v2
        let mut snap_v2 = snapshot(&lib_doc, "lib_ui", "UI Kit", 2);
        snap_v2.assets.iter_mut().find(|a| a.name == "Button").unwrap().key = "k_btn".into();

        let report2 = consumer.apply_spec_with_libraries(page, &spec, &[snap_v2]).unwrap();
        assert_eq!(report2.stale_library_assets, vec!["k_btn".to_string()]);
    }
}
