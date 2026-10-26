//! `designos_search_design_system` implementation (`T7.9`, Issue `#272`, extended by `E15`).

use super::library_source::{enabled_snapshots_from_root, resolve_library_root};
use designos_doc::{
    is_slot_container, library_asset_of_component, local_component_roots, local_key, parse_variant_name, property_kind, scope_allows, style_property,
    ComponentPropDef, Document, FrameProps, LibraryAssetKind, LibrarySnapshot, Node, NodeId, NodeKind, OverrideField, PaintKind, PropKind, PropValue,
    SlotPropConfig, Style, StyleMeta, StyleProperty, TextProps, Variable, VariableCollection, VariableKind, VariableValue,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Canonical bindable property keys accepted by `designos_doc::property_kind`.
pub const VALID_PROPERTY_KEYS: &[&str] = &[
    "fills/0",
    "strokes/0",
    "fills/0/opacity",
    "strokes/0/opacity",
    "effects/0/opacity",
    "width",
    "height",
    "topLeftRadius",
    "topRightRadius",
    "bottomLeftRadius",
    "bottomRightRadius",
    "itemSpacing",
    "counterAxisSpacing",
    "paddingLeft",
    "paddingRight",
    "paddingTop",
    "paddingBottom",
    "opacity",
    "fontSize",
    "fontWeight",
    "lineHeight",
    "letterSpacing",
    "strokeWeight",
    "strokeTopWeight",
    "strokeRightWeight",
    "strokeBottomWeight",
    "strokeLeftWeight",
    "visible",
    "characters",
];

fn default_limit() -> usize {
    20
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchDesignSystemArgs {
    pub query: String,
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
    #[serde(default)]
    pub property: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemKind {
    ComponentSet,
    Component,
    Style,
    Variable,
}

impl ItemKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ComponentSet => "component_set",
            Self::Component => "component",
            Self::Style => "style",
            Self::Variable => "variable",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::ComponentSet => 0,
            Self::Component => 1,
            Self::Style => 2,
            Self::Variable => 3,
        }
    }

    fn matches_filter(self, filter: &[String]) -> bool {
        filter.iter().any(|k| match k.as_str() {
            "component" => matches!(self, Self::Component | Self::ComponentSet),
            "component_set" => matches!(self, Self::ComponentSet),
            "style" => matches!(self, Self::Style),
            "variable" => matches!(self, Self::Variable),
            _ => false,
        })
    }
}

#[derive(Clone, Debug)]
struct IndexedItem {
    kind: ItemKind,
    name: String,
    description: String,
    is_local: bool,
    ref_str: String,
    /// Lowercase `/`-separated path segments of the name (plus collection name for variables).
    path_segments: Vec<String>,
    /// Lowercase word tokens split on whitespace, `/`, `-`, `_`.
    name_tokens: Vec<String>,
    /// Lowercase full name (`name.to_lowercase()`).
    name_lower: String,
    /// Optional qualified lowercase name (`<collection>/<name>` for variables).
    qualified_lower: Option<String>,
    /// Variable metadata for `property` scope & kind filtering.
    var_meta: Option<VariableFilterMeta>,
    /// Prebuilt instance-ready payload fields (merged with `score` on match).
    payload: Map<String, Value>,
}

#[derive(Clone, Debug)]
struct VariableFilterMeta {
    kind: VariableKind,
    scopes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SearchIndex {
    items: Vec<IndexedItem>,
    libraries_searched: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IndexCacheKey {
    doc_runtime_id: u64,
    doc_epoch: u64,
    node_count: usize,
    style_count: usize,
    var_count: usize,
    external_count: usize,
    enabled_libs: Vec<(String, u32)>,
    root: PathBuf,
}

type CachedIndex = Option<(IndexCacheKey, Arc<SearchIndex>)>;

fn index_cache() -> &'static Mutex<CachedIndex> {
    static CACHE: OnceLock<Mutex<CachedIndex>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Clears the cached per-session search index.
pub fn clear_search_index_cache() {
    if let Ok(mut guard) = index_cache().lock() {
        *guard = None;
    }
}

/// Executes `designos_search_design_system` against `doc` using the configured library root.
pub fn search_design_system(doc: &Document, raw_args: Value) -> Result<Value, String> {
    let root = resolve_library_root();
    search_design_system_at(doc, raw_args, &root)
}

/// Executes `designos_search_design_system` against `doc` and an explicit library `root`.
pub fn search_design_system_at(doc: &Document, raw_args: Value, root: &Path) -> Result<Value, String> {
    let args: SearchDesignSystemArgs = serde_json::from_value(raw_args).map_err(|e| format!("INVALID_ARGUMENTS: {e}"))?;
    validate_args(&args)?;

    let index = get_or_build_index(doc, root);
    Ok(index.query(&args))
}

fn validate_args(args: &SearchDesignSystemArgs) -> Result<(), String> {
    if args.query.trim().is_empty() || args.query.len() > 256 {
        return Err("INVALID_ARGUMENTS: query must be 1..=256 non-empty characters".into());
    }
    if !(1..=50).contains(&args.limit) {
        return Err("INVALID_ARGUMENTS: limit must be between 1 and 50".into());
    }
    if let Some(kinds) = &args.kinds {
        for k in kinds {
            if !matches!(k.as_str(), "component" | "component_set" | "style" | "variable") {
                return Err(format!("INVALID_ARGUMENTS: unknown kind '{k}' (expected component, style, or variable)"));
            }
        }
    }
    if let Some(prop) = &args.property {
        if property_kind(prop).is_none() {
            let options = serde_json::to_string(VALID_PROPERTY_KEYS).unwrap_or_default();
            return Err(format!("UNKNOWN_PROPERTY_KEY: '{prop}' is not a bindable property key; valid_options: {options}"));
        }
    }
    Ok(())
}

fn get_or_build_index(doc: &Document, root: &Path) -> Arc<SearchIndex> {
    let key = IndexCacheKey {
        doc_runtime_id: doc.runtime_id(),
        doc_epoch: doc.mutation_epoch(),
        node_count: doc.nodes.len(),
        style_count: doc.styles.len(),
        var_count: doc.variables.len(),
        external_count: doc.library.external.len(),
        enabled_libs: doc.library.enabled.iter().map(|r| (r.id.clone(), r.version)).collect(),
        root: root.to_path_buf(),
    };
    if let Ok(guard) = index_cache().lock() {
        if let Some((cached_key, cached_idx)) = guard.as_ref() {
            if cached_key == &key {
                return Arc::clone(cached_idx);
            }
        }
    }
    let built = Arc::new(SearchIndex::build(doc, root));
    if let Ok(mut guard) = index_cache().lock() {
        *guard = Some((key, Arc::clone(&built)));
    }
    built
}

impl SearchIndex {
    /// Builds the search index from `doc`'s local assets and all enabled library snapshots in `root`.
    pub fn build(doc: &Document, root: &Path) -> Self {
        let snapshots = enabled_snapshots_from_root(doc, root);
        Self::build_with_snapshots(doc, &snapshots)
    }

    /// Builds the search index from `doc` and an explicit slice of enabled [`LibrarySnapshot`]s.
    pub fn build_with_snapshots(doc: &Document, snapshots: &[LibrarySnapshot]) -> Self {
        let mut items = Vec::new();
        let mut libraries_searched = Vec::with_capacity(1 + doc.library.enabled.len());
        libraries_searched.push("local".to_string());
        for en in &doc.library.enabled {
            if !libraries_searched.contains(&en.id) {
                libraries_searched.push(en.id.clone());
            }
        }

        let imported_local_keys: BTreeSet<&str> = doc.library.external.values().filter_map(|e| e.local_key.as_deref()).collect();
        let local_origin = json!({"type": "local"});

        // 1. Local components and component sets (`local_component_roots` excludes `doc.library_page`).
        for (node_id, name) in local_component_roots(doc) {
            let Some(node) = doc.get(node_id) else { continue };
            let ref_str = format!("node:{}", node_id.0);
            items.push(build_component_item(node, &name, true, local_origin.clone(), ref_str, None, ComponentSource::Local(doc, snapshots)));
        }

        // 2. Local styles (excluding imported library copies).
        for (style_key, style) in &doc.styles {
            if imported_local_keys.contains(style_key.as_str()) {
                continue;
            }
            let meta = doc.style_meta.get(style_key);
            let name = doc.style_name(style_key).to_string();
            let prop = style_property(doc, style_key);
            items.push(build_style_item(style_key, &name, style, meta, prop, doc, snapshots, None, &imported_local_keys, true, local_origin.clone()));
        }

        // 3. Local variables (excluding imported library copies).
        for (var_id, var) in &doc.variables {
            if imported_local_keys.contains(var_id.as_str()) || imported_local_keys.contains(var.collection.as_str()) {
                continue;
            }
            if doc.variable_collections.get(&var.collection).is_some_and(|c| c.is_library()) {
                continue;
            }
            items.push(build_variable_item(
                var,
                &doc.variables,
                &doc.variable_collections,
                doc,
                snapshots,
                None,
                &imported_local_keys,
                true,
                local_origin.clone(),
            ));
        }

        // 4. Enabled library snapshots.
        for snap in snapshots {
            let lib_name = if !snap.name.is_empty() {
                snap.name.clone()
            } else {
                doc.library.enabled.iter().find(|r| r.id == snap.library_id).map(|r| r.name.clone()).unwrap_or_else(|| snap.library_id.clone())
            };
            let lib_origin = json!({
                "type": "library",
                "id": snap.library_id,
                "name": lib_name,
            });

            // 4a. Library components & component sets.
            for asset in &snap.assets {
                if !matches!(asset.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet) {
                    continue;
                }
                let Some(root_id) = asset.node else { continue };
                let Some(node) = snap.nodes.get(&root_id) else { continue };
                let local_copy = find_local_copy_of_library_component(doc, &snap.library_id, &asset.key);
                let key_collides = component_key_collides(&asset.key, &snap.library_id, doc, snapshots);
                let comp_ref = if key_collides { format!("{}/{}", snap.library_id, asset.key) } else { asset.key.clone() };
                items.push(build_component_item(
                    node,
                    &asset.name,
                    false,
                    lib_origin.clone(),
                    comp_ref,
                    local_copy,
                    ComponentSource::Snapshot(snap, doc, snapshots),
                ));
            }

            // 4b. Library styles.
            let has_style_assets = snap.assets.iter().any(|a| a.kind == LibraryAssetKind::Style);
            for (style_key, style) in &snap.styles {
                if has_style_assets && !snap.assets.iter().any(|a| a.kind == LibraryAssetKind::Style && strip_prefix_or_self(&a.key, "style:") == style_key) {
                    continue;
                }
                let meta = snap.style_meta.get(style_key);
                let name = meta.map(|m| m.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(style_key).to_string();
                let prop = meta.map(|m| m.property).unwrap_or(match style {
                    Style::Text(_) => StyleProperty::Text,
                    Style::Effect(_) => StyleProperty::Effect,
                    Style::Grid(_) => StyleProperty::Grid,
                    Style::Fill(_) => StyleProperty::Fill,
                });
                items.push(build_style_item(
                    style_key,
                    &name,
                    style,
                    meta,
                    prop,
                    doc,
                    snapshots,
                    Some(&snap.library_id),
                    &imported_local_keys,
                    false,
                    lib_origin.clone(),
                ));
            }

            // 4c. Library variables (hidden_from_publishing variables are excluded per §8.5).
            let has_var_assets = snap.assets.iter().any(|a| a.kind == LibraryAssetKind::Variable);
            for (var_id, var) in &snap.variables {
                if var.hidden_from_publishing {
                    continue;
                }
                if has_var_assets && !snap.assets.iter().any(|a| a.kind == LibraryAssetKind::Variable && strip_prefix_or_self(&a.key, "var:") == var_id) {
                    continue;
                }
                items.push(build_variable_item(
                    var,
                    &snap.variables,
                    &snap.variable_collections,
                    doc,
                    snapshots,
                    Some(&snap.library_id),
                    &imported_local_keys,
                    false,
                    lib_origin.clone(),
                ));
            }
        }

        Self { items, libraries_searched }
    }

    fn query(&self, args: &SearchDesignSystemArgs) -> Value {
        let q_norm = args.query.trim().to_lowercase();
        let q_tokens = tokenize(&args.query);
        let prop_filter = args.property.as_deref().and_then(|key| property_kind(key).map(|expected_kind| (key, expected_kind)));

        let mut scored: Vec<(u32, &IndexedItem)> = Vec::new();
        for item in &self.items {
            if let Some(kinds) = &args.kinds {
                if !item.kind.matches_filter(kinds) {
                    continue;
                }
            }
            if let Some((prop_key, expected_kind)) = prop_filter {
                if let Some(vm) = &item.var_meta {
                    if vm.kind != expected_kind || !variable_scope_allows_property(&vm.scopes, prop_key) {
                        continue;
                    }
                }
            }
            let s = score_item(item, &q_norm, &q_tokens);
            if s > 0 {
                scored.push((s, item));
            }
        }

        scored.sort_by(|(score_a, item_a), (score_b, item_b)| {
            score_b
                .cmp(score_a)
                .then_with(|| item_a.kind.rank().cmp(&item_b.kind.rank()))
                .then_with(|| item_a.name.cmp(&item_b.name))
                .then_with(|| item_a.ref_str.cmp(&item_b.ref_str))
        });

        let total = scored.len();
        let results: Vec<Value> = scored
            .into_iter()
            .take(args.limit)
            .map(|(score, item)| {
                let mut obj = item.payload.clone();
                obj.insert("score".into(), json!(score));
                Value::Object(obj)
            })
            .collect();

        json!({
            "results": results,
            "total": total,
            "libraries_searched": self.libraries_searched,
        })
    }
}

fn component_key_collides(asset_key: &str, lib_id: &str, doc: &Document, snapshots: &[LibrarySnapshot]) -> bool {
    if snapshots.iter().any(|s| s.library_id != lib_id && s.asset(asset_key).is_some()) {
        return true;
    }
    if doc.library.external.iter().any(|(k, ext)| {
        ext.library_id != lib_id
            && matches!(ext.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet)
            && (k == asset_key || k.strip_prefix(&format!("{}/", ext.library_id)) == Some(asset_key))
    }) {
        return true;
    }
    doc.nodes.values().any(|n| matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) && n.name == asset_key)
}

enum ComponentSource<'a> {
    Local(&'a Document, &'a [LibrarySnapshot]),
    Snapshot(&'a LibrarySnapshot, &'a Document, &'a [LibrarySnapshot]),
}

impl<'a> ComponentSource<'a> {
    fn get_node(&self, id: NodeId) -> Option<&'a Node> {
        match self {
            Self::Local(doc, _) => doc.get(id),
            Self::Snapshot(snap, _, _) => snap.nodes.get(&id),
        }
    }

    fn resolve_ref_for_node(&self, id: NodeId) -> Option<String> {
        match self {
            Self::Local(doc, snapshots) => {
                if let Some((ekey, e)) = library_asset_of_component(doc, id) {
                    let prefix = format!("{}/", e.library_id);
                    let bare = ekey.strip_prefix(&prefix).unwrap_or(ekey);
                    if component_key_collides(bare, &e.library_id, doc, snapshots) {
                        return Some(format!("{}/{bare}", e.library_id));
                    }
                    return Some(bare.to_string());
                }
                let n = doc.get(id)?;
                if matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
                    Some(format!("node:{}", id.0))
                } else {
                    None
                }
            }
            Self::Snapshot(snap, doc, snapshots) => {
                let scope_if_needed = |key: String| -> String {
                    if component_key_collides(&key, &snap.library_id, doc, snapshots) {
                        format!("{}/{key}", snap.library_id)
                    } else {
                        key
                    }
                };
                if let Some(asset) = snap.assets.iter().find(|a| a.node == Some(id)) {
                    return Some(scope_if_needed(asset.key.clone()));
                }
                if let Some(n) = snap.nodes.get(&id) {
                    if let Some(parent_id) = n.parent {
                        if let Some(asset) = snap.assets.iter().find(|a| a.node == Some(parent_id)) {
                            return Some(scope_if_needed(asset.key.clone()));
                        }
                    }
                    if matches!(n.kind, NodeKind::ComponentSet(_)) {
                        return Some(scope_if_needed(format!("s{}", id.0)));
                    }
                    if matches!(n.kind, NodeKind::Component(_)) {
                        return Some(scope_if_needed(format!("c{}", id.0)));
                    }
                }
                if doc.get(id).is_some() {
                    return Some(format!("node:{}", id.0));
                }
                None
            }
        }
    }
}

fn find_local_copy_of_library_component(doc: &Document, library_id: &str, asset_key: &str) -> Option<String> {
    let ns_key = local_key(library_id, asset_key);
    for (k, ext) in &doc.library.external {
        if ext.library_id != library_id || !matches!(ext.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet) {
            continue;
        }
        let Some(nid) = ext.node else { continue };
        if doc.get(nid).is_none() {
            continue;
        }
        if k == &ns_key || k == asset_key {
            return Some(format!("node:{}", nid.0));
        }
    }
    None
}

fn build_component_item(
    node: &Node,
    name: &str,
    is_local: bool,
    origin: Value,
    ref_str: String,
    local_copy: Option<String>,
    source: ComponentSource<'_>,
) -> IndexedItem {
    let is_set = matches!(node.kind, NodeKind::ComponentSet(_));
    let kind = if is_set { ItemKind::ComponentSet } else { ItemKind::Component };

    let (variant_props_json, default_variant_map) = extract_variant_schema(node, &source);
    let properties_json = extract_component_properties(node, &source);
    let slots_json = extract_slot_definitions(node, &source);

    let mut example = Map::new();
    example.insert("kind".into(), json!("instance"));
    example.insert("component".into(), json!(ref_str));
    if !default_variant_map.is_empty() {
        example.insert("variant".into(), json!(default_variant_map));
    }

    let mut payload = Map::new();
    payload.insert("kind".into(), json!(kind.as_str()));
    payload.insert("name".into(), json!(name));
    if !node.component.description.is_empty() {
        payload.insert("description".into(), json!(node.component.description));
    }
    payload.insert("origin".into(), origin);
    payload.insert("ref".into(), json!(ref_str));
    if let Some(lc) = local_copy {
        payload.insert("local_copy".into(), json!(lc));
    }
    payload.insert("variant_properties".into(), Value::Object(variant_props_json));
    payload.insert("properties".into(), Value::Array(properties_json));
    payload.insert("slots".into(), Value::Array(slots_json));
    payload.insert("example".into(), Value::Object(example));

    let path_segments = split_path_segments(name);
    let name_tokens = tokenize(name);
    IndexedItem {
        kind,
        name: name.to_string(),
        description: node.component.description.clone(),
        is_local,
        ref_str,
        path_segments,
        name_tokens,
        name_lower: name.trim().to_lowercase(),
        qualified_lower: None,
        var_meta: None,
        payload,
    }
}

fn extract_variant_schema(node: &Node, source: &ComponentSource<'_>) -> (Map<String, Value>, BTreeMap<String, String>) {
    let mut prop_order: Vec<String> = Vec::new();
    let mut options_by_prop: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut default_by_prop: BTreeMap<String, String> = BTreeMap::new();

    // 1. Seed from explicit `PropKind::Variant` definitions on the node.
    for def in &node.component.props {
        if let PropKind::Variant { options } = &def.kind {
            if !prop_order.contains(&def.name) {
                prop_order.push(def.name.clone());
            }
            let entry = options_by_prop.entry(def.name.clone()).or_default();
            for opt in options {
                if !entry.contains(opt) {
                    entry.push(opt.clone());
                }
            }
            if let PropValue::Variant(d) | PropValue::Text(d) = &def.default {
                if !d.is_empty() {
                    default_by_prop.insert(def.name.clone(), d.clone());
                }
            }
        }
    }

    // 2. Inspect child variant components (for a ComponentSet) in child order.
    // Per E15-1 §3.2, the first variant in child order is the set default.
    if matches!(node.kind, NodeKind::ComponentSet(_)) {
        for (idx, &child_id) in node.children.iter().enumerate() {
            let Some(child) = source.get_node(child_id) else { continue };
            if !matches!(child.kind, NodeKind::Component(_)) {
                continue;
            }
            let mut pairs = parse_variant_name(&child.name);
            if pairs.is_empty() && !child.component.variant_props.is_empty() {
                pairs = child.component.variant_props.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            }
            for (k, v) in pairs {
                if !prop_order.contains(&k) {
                    prop_order.push(k.clone());
                }
                let entry = options_by_prop.entry(k.clone()).or_default();
                if !entry.contains(&v) {
                    entry.push(v.clone());
                }
                if idx == 0 {
                    default_by_prop.insert(k, v);
                } else {
                    default_by_prop.entry(k).or_insert(v);
                }
            }
        }
    }

    let mut variant_props_json = Map::new();
    let mut default_map = BTreeMap::new();
    for prop_name in prop_order {
        let opts = options_by_prop.remove(&prop_name).unwrap_or_default();
        let def_val = default_by_prop.remove(&prop_name).or_else(|| opts.first().cloned()).unwrap_or_default();
        if !def_val.is_empty() {
            default_map.insert(prop_name.clone(), def_val.clone());
        }
        variant_props_json.insert(
            prop_name,
            json!({
                "options": opts,
                "default": def_val,
            }),
        );
    }

    (variant_props_json, default_map)
}

fn extract_component_properties(node: &Node, source: &ComponentSource<'_>) -> Vec<Value> {
    let mut all_defs: Vec<&ComponentPropDef> = node.component.props.iter().collect();
    if matches!(node.kind, NodeKind::ComponentSet(_)) {
        for &child_id in &node.children {
            if let Some(child) = source.get_node(child_id) {
                for def in &child.component.props {
                    if !all_defs.iter().any(|d| d.name == def.name) {
                        all_defs.push(def);
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    for def in all_defs {
        match &def.kind {
            PropKind::Boolean => {
                let default_val = match &def.default {
                    PropValue::Bool(b) => json!(b),
                    _ => json!(false),
                };
                out.push(json!({
                    "name": def.name,
                    "type": "boolean",
                    "default": default_val,
                }));
            }
            PropKind::Text => {
                let default_val = match &def.default {
                    PropValue::Text(s) => json!(s),
                    _ => json!(""),
                };
                out.push(json!({
                    "name": def.name,
                    "type": "text",
                    "default": default_val,
                }));
            }
            PropKind::InstanceSwap { preferred } => {
                let default_val = match &def.default {
                    PropValue::Instance(id) => source.resolve_ref_for_node(*id).map(|r| json!(r)).unwrap_or(Value::Null),
                    _ => Value::Null,
                };
                let recommended: Vec<String> = preferred.iter().filter_map(|id| source.resolve_ref_for_node(*id)).collect();
                out.push(json!({
                    "name": def.name,
                    "type": "instance_swap",
                    "default": default_val,
                    "recommended": recommended,
                }));
            }
            PropKind::Variant { .. } | PropKind::Slot => {}
        }
    }
    out
}

fn extract_slot_definitions(node: &Node, source: &ComponentSource<'_>) -> Vec<Value> {
    fn collect_from_holder(holder: &Node, slot_names: &mut Vec<String>, configs: &mut BTreeMap<String, SlotPropConfig>) {
        for def in &holder.component.props {
            if matches!(def.kind, PropKind::Slot) && !slot_names.contains(&def.name) {
                slot_names.push(def.name.clone());
            }
        }
        for (name, cfg) in &holder.component.slot_configs {
            if !slot_names.contains(name) {
                slot_names.push(name.clone());
            }
            configs.entry(name.clone()).or_insert_with(|| cfg.clone());
        }
    }

    let mut slot_names: Vec<String> = Vec::new();
    let mut configs: BTreeMap<String, SlotPropConfig> = BTreeMap::new();

    collect_from_holder(node, &mut slot_names, &mut configs);

    // Also inspect subtree for child variants or slot frames (`component.is_slot`).
    let mut stack: Vec<NodeId> = node.children.iter().rev().copied().collect();
    while let Some(nid) = stack.pop() {
        let Some(child) = source.get_node(nid) else { continue };
        collect_from_holder(child, &mut slot_names, &mut configs);
        if child.component.is_slot {
            let slot_name = child.component.bindings.get(&OverrideField::SlotContent).cloned().unwrap_or_else(|| child.name.clone());
            if !slot_name.is_empty() && !slot_names.contains(&slot_name) {
                slot_names.push(slot_name);
            }
        }
        stack.extend(child.children.iter().rev().copied());
    }

    let mut out = Vec::new();
    for name in slot_names {
        let cfg = configs.remove(&name).unwrap_or_default();
        let mut recommended: Vec<String> = Vec::new();
        for &pid in &cfg.preferred {
            if let Some(r) = source.resolve_ref_for_node(pid) {
                if !recommended.contains(&r) {
                    recommended.push(r);
                }
            }
        }
        out.push(json!({
            "name": name,
            "min_children": cfg.min_children,
            "max_children": cfg.max_children,
            "strict": cfg.allow_preferred_values_only.unwrap_or(false),
            "recommended": recommended,
        }));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn build_style_item(
    style_key: &str,
    name: &str,
    style: &Style,
    meta: Option<&StyleMeta>,
    prop: StyleProperty,
    doc: &Document,
    snapshots: &[LibrarySnapshot],
    source_library_id: Option<&str>,
    imported_local_keys: &BTreeSet<&str>,
    is_local: bool,
    origin: Value,
) -> IndexedItem {
    let collides = style_name_collides(name, style_key, source_library_id, doc, snapshots, imported_local_keys);
    let ref_str = match (collides, source_library_id) {
        (true, Some(lib_id)) => format!("style:{lib_id}/{style_key}"),
        (true, None) => format!("style:{style_key}"),
        (false, _) => format!("style:{name}"),
    };
    let style_type = match prop {
        StyleProperty::Fill => "fill",
        StyleProperty::Stroke => "stroke",
        StyleProperty::Text => "text",
        StyleProperty::Effect => "effect",
        StyleProperty::Grid => "grid",
    };
    let summary = summarize_style(style);
    let named_ref = if collides { ref_str.as_str() } else { name };
    let example = match prop {
        StyleProperty::Text => json!({"text_style": named_ref}),
        StyleProperty::Effect => json!({"effect_style": named_ref}),
        StyleProperty::Grid => json!({"grid_style": named_ref}),
        StyleProperty::Stroke => json!({"stroke": ref_str}),
        StyleProperty::Fill => json!({"fill": ref_str}),
    };
    let description = meta.map(|m| m.description.clone()).unwrap_or_default();

    let mut payload = Map::new();
    payload.insert("kind".into(), json!("style"));
    payload.insert("name".into(), json!(name));
    if !description.is_empty() {
        payload.insert("description".into(), json!(description));
    }
    payload.insert("origin".into(), origin);
    payload.insert("ref".into(), json!(ref_str));
    payload.insert("style_type".into(), json!(style_type));
    payload.insert("summary".into(), json!(summary));
    payload.insert("example".into(), example);

    IndexedItem {
        kind: ItemKind::Style,
        name: name.to_string(),
        description,
        is_local,
        ref_str,
        path_segments: split_path_segments(name),
        name_tokens: tokenize(name),
        name_lower: name.trim().to_lowercase(),
        qualified_lower: None,
        var_meta: None,
        payload,
    }
}

fn style_name_collides(
    name: &str,
    style_key: &str,
    source_library_id: Option<&str>,
    doc: &Document,
    snapshots: &[LibrarySnapshot],
    imported_local_keys: &BTreeSet<&str>,
) -> bool {
    let mut local_matches = 0usize;
    for k in doc.styles.keys() {
        if imported_local_keys.contains(k.as_str()) {
            continue;
        }
        let sname = doc.style_name(k);
        if sname.eq_ignore_ascii_case(name) || k.eq_ignore_ascii_case(name) {
            local_matches += 1;
        }
    }
    let Some(lib_id) = source_library_id else {
        return local_matches > 1;
    };
    if local_matches > 0 {
        return true;
    }
    let mut snap_matches = 0usize;
    for snap in snapshots {
        let has_style_assets = snap.assets.iter().any(|a| a.kind == LibraryAssetKind::Style);
        for sk in snap.styles.keys() {
            if has_style_assets && !snap.assets.iter().any(|a| a.kind == LibraryAssetKind::Style && strip_prefix_or_self(&a.key, "style:") == sk) {
                continue;
            }
            let sname = snap.style_meta.get(sk).map(|m| m.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(sk);
            if sname.eq_ignore_ascii_case(name) || sk.eq_ignore_ascii_case(name) || (snap.library_id != lib_id && sk.eq_ignore_ascii_case(style_key)) {
                snap_matches += 1;
            }
        }
    }
    if snap_matches > 1 {
        return true;
    }
    doc.library.external.iter().any(|(k, ext)| {
        if ext.kind != LibraryAssetKind::Style || ext.library_id == lib_id {
            return false;
        }
        let prefix = format!("{}/", ext.library_id);
        let raw_k = k.strip_prefix(&prefix).unwrap_or(k);
        let raw_style_key = raw_k.strip_prefix("style:").unwrap_or(raw_k);
        if raw_style_key.eq_ignore_ascii_case(style_key) || raw_style_key.eq_ignore_ascii_case(name) {
            return true;
        }
        ext.local_key.as_deref().and_then(|lk| doc.style_meta.get(lk)).is_some_and(|m| m.name.eq_ignore_ascii_case(name))
    })
}

fn summarize_style(style: &Style) -> String {
    match style {
        Style::Fill(paints) => {
            if let Some(p) = paints.iter().find(|p| p.visible) {
                match &p.kind {
                    PaintKind::Solid(c) => format!("#{}", c.to_hex()),
                    PaintKind::LinearGradient { .. } => "linear-gradient".into(),
                    PaintKind::RadialGradient { .. } => "radial-gradient".into(),
                    PaintKind::AngularGradient { .. } => "angular-gradient".into(),
                    PaintKind::DiamondGradient { .. } => "diamond-gradient".into(),
                    PaintKind::Image { .. } => "image".into(),
                }
            } else {
                "empty".into()
            }
        }
        Style::Text(ts) => format!("{} {}px/{}", ts.font_family, ts.font_size, ts.font_weight),
        Style::Effect(fx) => format!("{} effect(s)", fx.len()),
        Style::Grid(grids) => format!("{} grid(s)", grids.len()),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_variable_item(
    var: &Variable,
    variables: &BTreeMap<String, Variable>,
    collections: &BTreeMap<String, VariableCollection>,
    doc: &Document,
    snapshots: &[LibrarySnapshot],
    source_library_id: Option<&str>,
    imported_local_keys: &BTreeSet<&str>,
    is_local: bool,
    origin: Value,
) -> IndexedItem {
    let coll = collections.get(&var.collection);
    let collection_name = coll.map(|c| c.name.clone()).unwrap_or_else(|| var.collection.clone());
    let resolved_kind = resolve_variable_kind(var, variables);
    let ref_str = format_variable_ref(var, &collection_name, variables, collections, doc, snapshots, source_library_id, imported_local_keys);

    let mut values_map = Map::new();
    if let Some(c) = coll {
        for mode in &c.modes {
            if let Some(val) = resolve_variable_in_mode(var, &mode.id, &c.default_mode, variables, collections) {
                values_map.insert(mode.name.clone(), format_variable_value(&val));
            }
        }
    } else {
        for (mode_id, val) in &var.values {
            values_map.insert(mode_id.clone(), format_variable_value(val));
        }
    }

    let example = variable_example(&ref_str, resolved_kind, &var.scopes);
    let kind_str = match resolved_kind {
        VariableKind::Color => "color",
        VariableKind::Number => "number",
        VariableKind::String => "string",
        VariableKind::Boolean => "boolean",
    };

    let mut payload = Map::new();
    payload.insert("kind".into(), json!("variable"));
    payload.insert("name".into(), json!(var.name));
    payload.insert("collection".into(), json!(collection_name));
    payload.insert("variable_kind".into(), json!(kind_str));
    if !var.description.is_empty() {
        payload.insert("description".into(), json!(var.description));
    }
    payload.insert("origin".into(), origin);
    payload.insert("ref".into(), json!(ref_str));
    payload.insert("values".into(), Value::Object(values_map));
    payload.insert("code_syntax".into(), json!(var.code_syntax));
    payload.insert("example".into(), example);

    let mut path_segments = split_path_segments(&collection_name);
    path_segments.extend(split_path_segments(&var.name));
    let mut name_tokens = tokenize(&collection_name);
    for t in tokenize(&var.name) {
        if !name_tokens.contains(&t) {
            name_tokens.push(t);
        }
    }

    let qualified_lower = Some(format!("{}/{}", collection_name.trim().to_lowercase(), var.name.trim().to_lowercase()));
    let _ = is_slot_container;

    IndexedItem {
        kind: ItemKind::Variable,
        name: var.name.clone(),
        description: var.description.clone(),
        is_local,
        ref_str,
        path_segments,
        name_tokens,
        name_lower: var.name.trim().to_lowercase(),
        qualified_lower,
        var_meta: Some(VariableFilterMeta { kind: resolved_kind, scopes: var.scopes.clone() }),
        payload,
    }
}

fn resolve_variable_kind(var: &Variable, variables: &BTreeMap<String, Variable>) -> VariableKind {
    let mut cur = var;
    for _ in 0..16 {
        if let Some(VariableValue::Alias(next_id)) = cur.values.values().next() {
            if let Some(next_var) = variables.get(next_id) {
                cur = next_var;
                continue;
            }
        }
        return cur.kind;
    }
    var.kind
}

fn resolve_variable_in_mode(
    var: &Variable,
    mode_id: &str,
    default_mode_id: &str,
    variables: &BTreeMap<String, Variable>,
    collections: &BTreeMap<String, VariableCollection>,
) -> Option<VariableValue> {
    let mut cur_var = var;
    let mut cur_mode = mode_id;
    let mut cur_default = default_mode_id;
    for _ in 0..16 {
        let val = cur_var.values.get(cur_mode).or_else(|| cur_var.values.get(cur_default)).or_else(|| cur_var.values.values().next())?;
        match val {
            VariableValue::Alias(next_id) => {
                cur_var = variables.get(next_id)?;
                if let Some(c) = collections.get(&cur_var.collection) {
                    // Match mode by id or by name if crossing collections, falling back to target collection's default_mode.
                    if !cur_var.values.contains_key(cur_mode) {
                        cur_mode = &c.default_mode;
                    }
                    cur_default = &c.default_mode;
                }
            }
            other => return Some(other.clone()),
        }
    }
    None
}

fn format_variable_value(val: &VariableValue) -> Value {
    match val {
        VariableValue::Color(c) => json!(format!("#{}", c.to_hex())),
        VariableValue::Number(n) => json!(n),
        VariableValue::String(s) => json!(s),
        VariableValue::Boolean(b) => json!(b),
        VariableValue::Alias(a) => json!(format!("alias:{a}")),
    }
}

/// Formats `var:<collection>/<name>` or falls back to `var:id:<id>` (or `var:id:<lib_id>/<id>`) when the path is ambiguous (`E15-3` §3.1, `T7.9` §3.3).
#[allow(clippy::too_many_arguments)]
fn format_variable_ref(
    var: &Variable,
    collection_name: &str,
    variables: &BTreeMap<String, Variable>,
    collections: &BTreeMap<String, VariableCollection>,
    doc: &Document,
    snapshots: &[LibrarySnapshot],
    source_library_id: Option<&str>,
    imported_local_keys: &BTreeSet<&str>,
) -> String {
    let candidate_path = format!("{collection_name}/{}", var.name);
    if is_unambiguous_var_path(&candidate_path, var, variables, collections)
        && count_system_var_path_matches(&candidate_path, doc, snapshots, imported_local_keys) == 1
    {
        return format!("var:{candidate_path}");
    }
    if let Some(lib_id) = source_library_id {
        if raw_var_id_collides(&var.id, lib_id, doc, snapshots, imported_local_keys) {
            return format!("var:id:{lib_id}/{}", var.id);
        }
    }
    format!("var:id:{}", var.id)
}

fn var_matches_candidate_path(candidate_path: &str, var: &Variable, collections: &BTreeMap<String, VariableCollection>) -> bool {
    let cname = collections.get(&var.collection).map(|c| c.name.as_str()).unwrap_or(var.collection.as_str());
    let cid = var.collection.as_str();
    let p_name = format!("{cname}/{}", var.name);
    let p_id = format!("{cid}/{}", var.name);
    p_name == candidate_path || p_id == candidate_path || p_name.eq_ignore_ascii_case(candidate_path)
}

fn count_system_var_path_matches(candidate_path: &str, doc: &Document, snapshots: &[LibrarySnapshot], imported_local_keys: &BTreeSet<&str>) -> usize {
    let mut count = 0usize;
    for (vid, v) in &doc.variables {
        if imported_local_keys.contains(vid.as_str()) || imported_local_keys.contains(v.collection.as_str()) {
            continue;
        }
        if doc.variable_collections.get(&v.collection).is_some_and(|c| c.is_library()) {
            continue;
        }
        if var_matches_candidate_path(candidate_path, v, &doc.variable_collections) {
            count += 1;
        }
    }
    for snap in snapshots {
        let has_var_assets = snap.assets.iter().any(|a| a.kind == LibraryAssetKind::Variable);
        for (vid, v) in &snap.variables {
            if v.hidden_from_publishing {
                continue;
            }
            if has_var_assets && !snap.assets.iter().any(|a| a.kind == LibraryAssetKind::Variable && strip_prefix_or_self(&a.key, "var:") == vid) {
                continue;
            }
            if var_matches_candidate_path(candidate_path, v, &snap.variable_collections) {
                count += 1;
            }
        }
    }
    count
}

fn raw_var_id_collides(var_id: &str, lib_id: &str, doc: &Document, snapshots: &[LibrarySnapshot], imported_local_keys: &BTreeSet<&str>) -> bool {
    if let Some(v) = doc.variables.get(var_id) {
        if !imported_local_keys.contains(var_id) && !doc.variable_collections.get(&v.collection).is_some_and(|c| c.is_library()) {
            return true;
        }
    }
    if snapshots.iter().any(|s| s.library_id != lib_id && s.variables.contains_key(var_id)) {
        return true;
    }
    doc.library.external.iter().any(|(k, ext)| {
        if ext.kind != LibraryAssetKind::Variable || ext.library_id == lib_id {
            return false;
        }
        let prefix = format!("{}/", ext.library_id);
        let raw_k = k.strip_prefix(&prefix).unwrap_or(k);
        let raw_id = raw_k.strip_prefix("var:").unwrap_or(raw_k);
        raw_id == var_id
    })
}

fn is_unambiguous_var_path(path: &str, target: &Variable, variables: &BTreeMap<String, Variable>, collections: &BTreeMap<String, VariableCollection>) -> bool {
    // Find longest collection id or name `c` such that `path` starts with `{c}/`.
    let mut best_len = 0usize;
    let mut best_colls: Vec<&str> = Vec::new();
    for coll in collections.values() {
        for label in [coll.id.as_str(), coll.name.as_str()] {
            if label.is_empty() {
                continue;
            }
            if let Some(rest) = path.strip_prefix(label).and_then(|r| r.strip_prefix('/')) {
                if rest.is_empty() {
                    continue;
                }
                if label.len() > best_len {
                    best_len = label.len();
                    best_colls.clear();
                    best_colls.push(&coll.id);
                } else if label.len() == best_len && !best_colls.contains(&coll.id.as_str()) {
                    best_colls.push(&coll.id);
                }
            }
        }
    }
    if best_colls.len() != 1 || best_colls[0] != target.collection {
        return false;
    }
    let remainder = &path[best_len + 1..];
    let matches_in_coll = variables.values().filter(|v| v.collection == target.collection && v.name == remainder).count();
    matches_in_coll == 1
}

fn variable_example(ref_str: &str, kind: VariableKind, scopes: &[String]) -> Value {
    let has_scope = |target: &str| scopes.iter().any(|s| s.eq_ignore_ascii_case(target));
    match kind {
        VariableKind::Color => {
            if has_scope("STROKE_COLOR")
                && !has_scope("ALL_FILLS")
                && !has_scope("FRAME_FILL")
                && !has_scope("SHAPE_FILL")
                && !has_scope("TEXT_FILL")
                && !has_scope("FILL")
            {
                json!({"stroke": ref_str})
            } else {
                json!({"fill": ref_str})
            }
        }
        VariableKind::Number => {
            if has_scope("CORNER_RADIUS") {
                json!({"corner_radius": ref_str})
            } else if has_scope("GAP") {
                json!({"layout": {"gap": ref_str}})
            } else if has_scope("FONT_SIZE") {
                json!({"font_size": ref_str})
            } else if has_scope("LINE_HEIGHT") {
                json!({"line_height": ref_str})
            } else if has_scope("STROKE_FLOAT") {
                json!({"stroke_weight": ref_str})
            } else if has_scope("OPACITY") || has_scope("COLOR_OPACITY") {
                json!({"opacity": ref_str})
            } else {
                json!({"width": ref_str})
            }
        }
        VariableKind::String => json!({"text": ref_str}),
        VariableKind::Boolean => json!({"visible": ref_str}),
    }
}

fn variable_scope_allows_property(scopes: &[String], prop_key: &str) -> bool {
    scope_allows(scopes, prop_key, &NodeKind::Frame(FrameProps::default()))
        || scope_allows(scopes, prop_key, &NodeKind::Text(TextProps::default()))
        || scope_allows(scopes, prop_key, &NodeKind::Rectangle)
}

fn score_item(item: &IndexedItem, q_norm: &str, q_tokens: &[String]) -> u32 {
    let mut lexical: u32 = 0;

    let exact_name = item.name_lower == q_norm || item.qualified_lower.as_deref() == Some(q_norm);
    let starts_with = item.name_lower.starts_with(q_norm) || item.qualified_lower.as_deref().is_some_and(|ql| ql.starts_with(q_norm));

    if exact_name {
        lexical += 100;
    }
    if starts_with {
        lexical += 60;
    }

    let all_tokens_in_segments = !q_tokens.is_empty()
        && q_tokens
            .iter()
            .all(|qt| item.path_segments.iter().any(|seg| seg.contains(qt.as_str())) || item.name_tokens.iter().any(|nt| nt.contains(qt.as_str())));

    if all_tokens_in_segments && !exact_name {
        lexical += 40;
    }

    // Description token matches: +10 per query token found in description.
    if !item.description.is_empty() {
        let desc_lower = item.description.to_lowercase();
        for qt in q_tokens {
            if desc_lower.contains(qt.as_str()) {
                lexical += 10;
            }
        }
    }

    // Token fuzzy match (Levenshtein <= 1 for tokens of length >= 4) in name: +15.
    if !all_tokens_in_segments {
        let has_fuzzy = q_tokens
            .iter()
            .filter(|qt| qt.chars().count() >= 4)
            .any(|qt| !item.name_tokens.iter().any(|nt| nt.contains(qt.as_str())) && item.name_tokens.iter().any(|nt| levenshtein_at_most_one(qt, nt)));
        if has_fuzzy {
            lexical += 15;
        }
    }

    if lexical == 0 {
        return 0;
    }
    if item.is_local {
        lexical + 5
    } else {
        lexical
    }
}

fn split_path_segments(s: &str) -> Vec<String> {
    s.split('/').map(|seg| seg.trim().to_lowercase()).filter(|seg| !seg.is_empty()).collect()
}

fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || matches!(c, '/' | '-' | '_')).map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect()
}

fn strip_prefix_or_self<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix).unwrap_or(s)
}

/// Returns true when the Levenshtein edit distance between `a` and `b` is `<= 1`.
fn levenshtein_at_most_one(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let (m, n) = (ac.len(), bc.len());
    if m.abs_diff(n) > 1 {
        return false;
    }
    if m == n {
        let diffs = ac.iter().zip(bc.iter()).filter(|(x, y)| x != y).count();
        return diffs <= 1;
    }
    // Length differs by 1: check single insertion/deletion.
    let (shorter, longer) = if m < n { (&ac, &bc) } else { (&bc, &ac) };
    let (mut i, mut j, mut edits) = (0usize, 0usize, 0usize);
    while i < shorter.len() && j < longer.len() {
        if shorter[i] == longer[j] {
            i += 1;
            j += 1;
        } else {
            edits += 1;
            if edits > 1 {
                return false;
            }
            j += 1;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::library_source::clear_snapshot_cache;
    use designos_doc::{import_asset, CodeSyntaxPlatform, Color, LayoutGrid, LibraryRef, Paint, VariableMode, Vec2};
    use designos_library::Library;

    fn make_temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("designos_t7_9_{tag}_{}", designos_library::new_id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn build_brand_kit_doc() -> Document {
        let mut doc = Document::new("Brand kit");
        let page = doc.add_page("Components");

        // Button ComponentSet with variants Size={sm,md,lg}, State={default,hover} and Text prop "Label"
        let set_id = doc.alloc_id();
        let mut set_node = Node::new(set_id, NodeKind::ComponentSet(FrameProps::default()));
        set_node.name = "Button".into();
        set_node.component.description = "Primary interactive action".into();
        set_node.component.props = vec![
            ComponentPropDef {
                name: "Size".into(),
                kind: PropKind::Variant { options: vec!["sm".into(), "md".into(), "lg".into()] },
                default: PropValue::Variant("md".into()),
            },
            ComponentPropDef {
                name: "State".into(),
                kind: PropKind::Variant { options: vec!["default".into(), "hover".into()] },
                default: PropValue::Variant("default".into()),
            },
            ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Button".into()) },
        ];
        doc.insert(page, set_node).unwrap();

        // First variant in child order is Size=md, State=default
        for (size, state) in [("md", "default"), ("sm", "default"), ("lg", "default"), ("md", "hover")] {
            let vid = doc.alloc_id();
            let mut v = Node::new(vid, NodeKind::Component(FrameProps::default()));
            v.name = format!("Size={size}, State={state}");
            v.size = Vec2::new(120.0, 40.0);
            doc.insert(set_id, v).unwrap();
        }

        // Shared text style "Heading L"
        doc.styles.insert("s_heading".into(), Style::Text(designos_doc::TextStyle::default()));
        doc.style_meta.insert(
            "s_heading".into(),
            StyleMeta {
                name: "Heading L".into(),
                key: Some("k_heading".into()),
                description: "Large section heading".into(),
                property: StyleProperty::Text,
                bindings: Default::default(),
                ..Default::default()
            },
        );

        doc
    }

    #[test]
    fn ranked_with_origin() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("ranked_origin");
        let lib = Library::open(&root).unwrap();

        let brand_doc = build_brand_kit_doc();
        lib.publish_library("lib-7f3a", &brand_doc, "v1").unwrap();

        let mut host = Document::new("App");
        let page = host.add_page("Page 1");
        host.library.enabled.push(LibraryRef { id: "lib-7f3a".into(), name: "Brand kit".into(), version: 1 });

        // Add a less-specific local component "ButtonGroup" and local Color/Primary variable
        let bg_id = host.alloc_id();
        let mut bg = Node::new(bg_id, NodeKind::Component(FrameProps::default()));
        bg.name = "Button/Toolbar".into();
        host.insert(page, bg).unwrap();

        let out = search_design_system_at(&host, json!({"query": "button"}), &root).unwrap();
        let results = out["results"].as_array().unwrap();
        assert!(!results.is_empty());
        assert_eq!(out["libraries_searched"], json!(["local", "lib-7f3a"]));

        // Exact match "Button" set from Brand kit ranks first with score 160
        let first = &results[0];
        assert_eq!(first["kind"], "component_set");
        assert_eq!(first["name"], "Button");
        assert_eq!(first["score"], 160);
        assert_eq!(first["origin"], json!({"type": "library", "id": "lib-7f3a", "name": "Brand kit"}));
        assert_eq!(
            first["variant_properties"],
            json!({
                "Size": {"options": ["sm", "md", "lg"], "default": "md"},
                "State": {"options": ["default", "hover"], "default": "default"}
            })
        );
        assert_eq!(first["properties"], json!([{"name": "Label", "type": "text", "default": "Button"}]));

        // If a local component also has the exact name "Button", local ranks higher (+5 -> 165)
        let local_btn_id = host.alloc_id();
        let mut local_btn = Node::new(local_btn_id, NodeKind::Component(FrameProps::default()));
        local_btn.name = "Button".into();
        host.insert(page, local_btn).unwrap();

        let out2 = search_design_system_at(&host, json!({"query": "button"}), &root).unwrap();
        let results2 = out2["results"].as_array().unwrap();
        assert_eq!(results2[0]["origin"], json!({"type": "local"}));
        assert_eq!(results2[0]["score"], 165);
        assert_eq!(results2[1]["origin"]["id"], "lib-7f3a");
        assert_eq!(results2[1]["score"], 160);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn disabled_excluded() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("disabled_excluded");
        let lib = Library::open(&root).unwrap();

        let brand_doc = build_brand_kit_doc();
        lib.publish_library("lib-disabled", &brand_doc, "v1").unwrap();

        // Host does NOT enable `lib-disabled`
        let mut host = Document::new("App");
        host.add_page("Page 1");

        let out = search_design_system_at(&host, json!({"query": "button"}), &root).unwrap();
        assert_eq!(out["results"], json!([]));
        assert_eq!(out["total"], 0);
        assert_eq!(out["libraries_searched"], json!(["local"]));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn result_pastes_into_apply_spec() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("paste_apply_spec");
        let lib = Library::open(&root).unwrap();

        let brand_doc = build_brand_kit_doc();
        lib.publish_library("lib-7f3a", &brand_doc, "v1").unwrap();

        let mut host = Document::new("App");
        let page = host.add_page("Page 1");
        host.library.enabled.push(LibraryRef { id: "lib-7f3a".into(), name: "Brand kit".into(), version: 1 });

        // Colliding local ComponentSet at NodeId(2) (same numeric ID as Brand kit's `s2` Button set!):
        let local_set_id = host.alloc_id();
        assert_eq!(local_set_id, NodeId(2), "host local component set must occupy NodeId(2) to test s2 collision");
        let mut local_set = Node::new(local_set_id, NodeKind::ComponentSet(FrameProps::default()));
        local_set.name = "LocalBanner".into();
        host.insert(page, local_set).unwrap();
        let local_var_id = host.alloc_id();
        let mut local_var = Node::new(local_var_id, NodeKind::Component(FrameProps::default()));
        local_var.name = "Layout=wide".into();
        host.insert(local_set_id, local_var).unwrap();

        // Add local variables (color, gap, corner_radius, font_size, line_height, stroke_weight, opacity, width, string, boolean)
        // and local styles (fill, stroke, text, effect, grid) so we can verify every example shape pastes into designos_apply_spec.
        host.variable_collections.insert(
            "c_tokens".into(),
            VariableCollection {
                extensions: Default::default(),
                id: "c_tokens".into(),
                name: "Tokens".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec![
                    "v_fill".into(),
                    "v_gap".into(),
                    "v_radius".into(),
                    "v_font_size".into(),
                    "v_line_height".into(),
                    "v_stroke_w".into(),
                    "v_opacity".into(),
                    "v_width".into(),
                    "v_str".into(),
                    "v_bool".into(),
                ],
                key: None,
                source_library_key: None,
            },
        );
        let insert_var = |doc: &mut Document, id: &str, name: &str, kind: VariableKind, val: VariableValue, scopes: &[&str]| {
            doc.variables.insert(
                id.into(),
                Variable {
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                    id: id.into(),
                    name: name.into(),
                    collection: "c_tokens".into(),
                    kind,
                    values: BTreeMap::from([("m1".into(), val)]),
                    description: String::new(),
                    scopes: scopes.iter().map(|s| (*s).to_string()).collect(),
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                },
            );
        };
        insert_var(&mut host, "v_fill", "tok-color", VariableKind::Color, VariableValue::Color(Color::rgb(0x0D, 0x99, 0xFF)), &["ALL_FILLS"]);
        insert_var(&mut host, "v_gap", "tok-gap", VariableKind::Number, VariableValue::Number(18.0), &["GAP"]);
        insert_var(&mut host, "v_radius", "tok-radius", VariableKind::Number, VariableValue::Number(10.0), &["CORNER_RADIUS"]);
        insert_var(&mut host, "v_font_size", "tok-fontsize", VariableKind::Number, VariableValue::Number(22.0), &["FONT_SIZE"]);
        insert_var(&mut host, "v_line_height", "tok-lineheight", VariableKind::Number, VariableValue::Number(30.0), &["LINE_HEIGHT"]);
        insert_var(&mut host, "v_stroke_w", "tok-strokew", VariableKind::Number, VariableValue::Number(3.0), &["STROKE_FLOAT"]);
        insert_var(&mut host, "v_opacity", "tok-opacity", VariableKind::Number, VariableValue::Number(50.0), &["OPACITY"]);
        insert_var(&mut host, "v_width", "tok-width", VariableKind::Number, VariableValue::Number(240.0), &["WIDTH_HEIGHT"]);
        insert_var(&mut host, "v_str", "tok-copy", VariableKind::String, VariableValue::String("Get Started".into()), &["TEXT_CONTENT"]);
        insert_var(&mut host, "v_bool", "tok-flag", VariableKind::Boolean, VariableValue::Boolean(false), &[]);

        host.styles.insert("st_fill".into(), Style::Fill(vec![Paint::solid(Color::rgb(0x11, 0x22, 0x33))]));
        host.style_meta.insert(
            "st_fill".into(),
            StyleMeta {
                name: "SurfaceFill".into(),
                key: None,
                description: String::new(),
                property: StyleProperty::Fill,
                bindings: Default::default(),
                ..Default::default()
            },
        );
        host.styles.insert("st_grid".into(), Style::Grid(vec![LayoutGrid::columns(12, 16.0, 24.0, Color::rgba(255, 0, 0, 25))]));
        host.style_meta.insert(
            "st_grid".into(),
            StyleMeta {
                name: "Grid12Col".into(),
                key: None,
                description: String::new(),
                property: StyleProperty::Grid,
                bindings: Default::default(),
                ..Default::default()
            },
        );

        let mut engine = crate::engine::Engine { doc: host, active_page_id: Some(page), ..Default::default() };

        let out = search_design_system_at(&engine.doc, json!({"query": "button"}), &root).unwrap();
        let first = &out["results"].as_array().unwrap()[0];
        let comp_ref = first["ref"].as_str().unwrap().to_string();
        let example = first["example"].clone();

        // Verify the example is a complete, edit-free instance spec fragment
        assert_eq!(
            &example,
            &json!({
                "kind": "instance",
                "component": comp_ref,
                "variant": {
                    "Size": "md",
                    "State": "default"
                }
            })
        );

        // Apply the component example directly through `Engine::call("designos_apply_spec", ...)` without edits!
        let apply_res = engine.call("designos_apply_spec", json!({"spec": example})).expect("component example must apply unchanged via designos_apply_spec");
        let inst_id = NodeId(apply_res["root_node_id"].as_u64().unwrap());

        let ext_entry = engine.doc.library.external.get(&local_key("lib-7f3a", &comp_ref)).unwrap();
        let imported_set_id = ext_entry.node.unwrap();
        let requested_variant: BTreeMap<String, String> = serde_json::from_value(example["variant"].clone()).unwrap();
        let target_variant_id = engine
            .doc
            .get(imported_set_id)
            .unwrap()
            .children
            .iter()
            .copied()
            .find(|&cid| {
                let pairs: BTreeMap<String, String> = parse_variant_name(&engine.doc.get(cid).unwrap().name).into_iter().collect();
                pairs == requested_variant
            })
            .expect("example variant map must match an existing variant in the imported ComponentSet");

        let inst_node = engine.doc.get(inst_id).unwrap();
        assert!(
            matches!(inst_node.kind, NodeKind::Instance { component: Some(c), .. } if c == target_variant_id),
            "expected Instance linked to variant {:?}, got {:?}",
            target_variant_id,
            inst_node.kind
        );

        // Re-running search after import returns the library asset once with `local_copy: "node:<id>"` (§8.2)
        let out_after = search_design_system_at(&engine.doc, json!({"query": "button"}), &root).unwrap();
        let results_after = out_after["results"].as_array().unwrap();
        assert_eq!(results_after.len(), 1);
        assert_eq!(results_after[0]["ref"], comp_ref);
        assert_eq!(results_after[0]["local_copy"], format!("node:{}", imported_set_id.0));

        // Verify variable examples paste directly into `designos_apply_spec` and create bindings + values:
        for (q, expected_prop) in [
            ("tok-color", "fills/0"),
            ("tok-gap", "itemSpacing"),
            ("tok-radius", "topLeftRadius"),
            ("tok-fontsize", "fontSize"),
            ("tok-lineheight", "lineHeight"),
            ("tok-strokew", "strokeWeight"),
            ("tok-opacity", "opacity"),
            ("tok-width", "width"),
            ("tok-copy", "characters"),
            ("tok-flag", "visible"),
        ] {
            let v_out = search_design_system_at(&engine.doc, json!({"query": q, "kinds": ["variable"]}), &root).unwrap();
            let v_hit = &v_out["results"].as_array().unwrap()[0];
            let v_apply = engine
                .call("designos_apply_spec", json!({"spec": v_hit["example"]}))
                .unwrap_or_else(|e| panic!("variable example for {q} ({}) failed apply_spec: {e}", v_hit["example"]));
            let nid = NodeId(v_apply["root_node_id"].as_u64().unwrap());
            let created = engine.doc.get(nid).unwrap();
            assert!(
                created.variables.bindings.contains_key(expected_prop),
                "expected binding `{expected_prop}` on node created from `{q}` example {}, got {:?}",
                v_hit["example"],
                created.variables.bindings
            );
            if q == "tok-copy" {
                let NodeKind::Text(tp) = &created.kind else { panic!("expected text node for string variable") };
                assert_eq!(tp.content, "Get Started", "string variable example must resolve text content instead of literal var: path");
            }
        }

        // Verify style examples (fill, text from library, grid) paste directly into `designos_apply_spec`:
        for (q, expected_style_slot) in [("SurfaceFill", "fill"), ("Heading L", "text"), ("Grid12Col", "grid")] {
            let s_out = search_design_system_at(&engine.doc, json!({"query": q, "kinds": ["style"]}), &root).unwrap();
            let s_hit = &s_out["results"].as_array().unwrap()[0];
            if q == "Grid12Col" {
                assert_eq!(s_hit["example"], json!({"grid_style": "Grid12Col"}), "grid style example must use grid_style, not fill");
            }
            let s_apply = engine
                .call("designos_apply_spec", json!({"spec": s_hit["example"]}))
                .unwrap_or_else(|e| panic!("style example for {q} ({}) failed apply_spec: {e}", s_hit["example"]));
            let nid = NodeId(s_apply["root_node_id"].as_u64().unwrap());
            let created = engine.doc.get(nid).unwrap();
            assert!(
                created.style_refs.contains_key(expected_style_slot),
                "expected style_refs `{expected_style_slot}` on node created from `{q}` example {}, got {:?}",
                s_hit["example"],
                created.style_refs
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn duplicate_component_names_in_same_library_match_local_copy_by_key_only() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("dup_comp_names");
        let lib = Library::open(&root).unwrap();

        let mut lib_doc = Document::new("Dup Kit");
        let lpage = lib_doc.add_page("Components");
        let c1 = lib_doc.alloc_id();
        let mut n1 = Node::new(c1, NodeKind::Component(FrameProps::default()));
        n1.name = "Button".into();
        lib_doc.insert(lpage, n1).unwrap();
        let key_first = format!("c{}", c1.0);

        let c2 = lib_doc.alloc_id();
        let mut n2 = Node::new(c2, NodeKind::Component(FrameProps::default()));
        n2.name = "Button".into();
        lib_doc.insert(lpage, n2).unwrap();
        let key_second = format!("c{}", c2.0);

        lib.publish_library("lib-dup", &lib_doc, "v1").unwrap();

        let mut host = Document::new("App");
        host.add_page("Page 1");
        host.library.enabled.push(LibraryRef { id: "lib-dup".into(), name: "Dup Kit".into(), version: 1 });

        // Import ONLY `key_second` locally
        let snaps = enabled_snapshots_from_root(&host, &root);
        let cmd = import_asset(&mut host, &snaps[0], &key_second, None);
        cmd.apply(&mut host).unwrap();
        let imported_second_id = host.library.external.get(&local_key("lib-dup", &key_second)).unwrap().node.unwrap();

        let out = search_design_system_at(&host, json!({"query": "button"}), &root).unwrap();
        let results = out["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);

        let first_hit = results.iter().find(|r| r["ref"] == key_first).unwrap();
        let second_hit = results.iter().find(|r| r["ref"] == key_second).unwrap();
        assert!(first_hit.get("local_copy").is_none(), "unimported `{key_first}` must not inherit `local_copy` from same-named `{key_second}`: {first_hit}");
        assert_eq!(second_hit["local_copy"], format!("node:{}", imported_second_id.0));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn variable_scope_filter() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("var_scope");
        let mut doc = Document::new("Tokens");
        doc.add_page("Page 1");

        doc.variable_collections.insert(
            "c_tokens".into(),
            VariableCollection {
                extensions: Default::default(),
                id: "c_tokens".into(),
                name: "Spacing".into(),
                modes: vec![VariableMode { id: "m_light".into(), name: "Light".into() }, VariableMode { id: "m_dark".into(), name: "Dark".into() }],
                default_mode: "m_light".into(),
                variables: vec!["v_gap".into(), "v_radius".into(), "v_color".into(), "v_unscoped".into()],
                key: None,
                source_library_key: None,
            },
        );

        // 1. GAP-compatible number variable
        let mut code_syntax = BTreeMap::new();
        code_syntax.insert(CodeSyntaxPlatform::Web, "var(--spacing-md)".into());
        doc.variables.insert(
            "v_gap".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_gap".into(),
                name: "space-md".into(),
                collection: "c_tokens".into(),
                kind: VariableKind::Number,
                values: BTreeMap::from([("m_light".into(), VariableValue::Number(16.0)), ("m_dark".into(), VariableValue::Number(16.0))]),
                description: "Medium spacing token".into(),
                scopes: vec!["GAP".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax,
            },
        );

        // 2. CORNER_RADIUS-only number variable (incompatible with itemSpacing)
        doc.variables.insert(
            "v_radius".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_radius".into(),
                name: "space-radius".into(),
                collection: "c_tokens".into(),
                kind: VariableKind::Number,
                values: BTreeMap::from([("m_light".into(), VariableValue::Number(8.0)), ("m_dark".into(), VariableValue::Number(8.0))]),
                description: String::new(),
                scopes: vec!["CORNER_RADIUS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );

        // 3. Color variable with "space" in name (incompatible with itemSpacing)
        doc.variables.insert(
            "v_color".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_color".into(),
                name: "space-bg".into(),
                collection: "c_tokens".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([
                    ("m_light".into(), VariableValue::Color(Color::from_hex("#0D99FF").unwrap())),
                    ("m_dark".into(), VariableValue::Color(Color::from_hex("#3582FF").unwrap())),
                ]),
                description: String::new(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );

        // 4. Unscoped number variable (empty scopes = allowed everywhere)
        doc.variables.insert(
            "v_unscoped".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_unscoped".into(),
                name: "space-any".into(),
                collection: "c_tokens".into(),
                kind: VariableKind::Number,
                values: BTreeMap::from([("m_light".into(), VariableValue::Number(24.0)), ("m_dark".into(), VariableValue::Number(24.0))]),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );

        let out = search_design_system_at(
            &doc,
            json!({
                "query": "space",
                "kinds": ["variable"],
                "property": "itemSpacing"
            }),
            &root,
        )
        .unwrap();

        let results = out["results"].as_array().unwrap();
        let names: Vec<&str> = results.iter().map(|r| r["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["space-any", "space-md"]);
        assert!(!names.contains(&"space-bg"), "colour variables must be excluded for itemSpacing");
        assert!(!names.contains(&"space-radius"), "CORNER_RADIUS number variables must be excluded for itemSpacing");

        // Verify variable payload fields (ref, values per mode, code_syntax, example)
        let gap_item = results.iter().find(|r| r["name"] == "space-md").unwrap();
        assert_eq!(gap_item["ref"], "var:Spacing/space-md");
        assert_eq!(gap_item["values"], json!({"Light": 16.0, "Dark": 16.0}));
        assert_eq!(gap_item["code_syntax"], json!({"WEB": "var(--spacing-md)"}));
        assert_eq!(gap_item["example"], json!({"layout": {"gap": "var:Spacing/space-md"}}));

        // Verify colour variable formatting when queried with fills/0
        let fill_out = search_design_system_at(
            &doc,
            json!({
                "query": "space-bg",
                "kinds": ["variable"],
                "property": "fills/0"
            }),
            &root,
        )
        .unwrap();
        let fill_res = &fill_out["results"].as_array().unwrap()[0];
        assert_eq!(fill_res["values"], json!({"Light": "#0D99FF", "Dark": "#3582FF"}));
        assert_eq!(fill_res["example"], json!({"fill": "var:Spacing/space-bg"}));

        // Unknown property key returns UNKNOWN_PROPERTY_KEY with valid_options
        let err = search_design_system_at(&doc, json!({"query": "space", "property": "notAProperty"}), &root).unwrap_err();
        assert!(err.starts_with("UNKNOWN_PROPERTY_KEY:"), "{err}");
        assert!(err.contains("itemSpacing"), "{err}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn slot_definitions_included() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("slots_included");
        let mut doc = Document::new("System");
        let page = doc.add_page("Page 1");

        // Recommended child components
        let badge_id = doc.alloc_id();
        let mut badge = Node::new(badge_id, NodeKind::Component(FrameProps::default()));
        badge.name = "Badge".into();
        doc.insert(page, badge).unwrap();

        let avatar_id = doc.alloc_id();
        let mut avatar = Node::new(avatar_id, NodeKind::Component(FrameProps::default()));
        avatar.name = "Avatar".into();
        doc.insert(page, avatar).unwrap();

        // Card component set with a "Header" slot recommending Badge and Avatar
        let card_set_id = doc.alloc_id();
        let mut card_set = Node::new(card_set_id, NodeKind::ComponentSet(FrameProps::default()));
        card_set.name = "Card".into();
        card_set.component.props.push(ComponentPropDef { name: "Header".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
        card_set.component.slot_configs.insert(
            "Header".into(),
            SlotPropConfig {
                min_children: Some(1),
                max_children: Some(2),
                allow_preferred_values_only: Some(true),
                preferred: vec![badge_id, avatar_id],
                ..Default::default()
            },
        );
        doc.insert(page, card_set).unwrap();

        let card_var_id = doc.alloc_id();
        let mut card_var = Node::new(card_var_id, NodeKind::Component(FrameProps::default()));
        card_var.name = "Elevation=raised".into();
        doc.insert(card_set_id, card_var).unwrap();

        let out = search_design_system_at(&doc, json!({"query": "card"}), &root).unwrap();
        let results = out["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        let card = &results[0];
        assert_eq!(card["kind"], "component_set");
        assert_eq!(card["ref"], format!("node:{}", card_set_id.0));

        let slots = card["slots"].as_array().unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0]["name"], "Header");
        assert_eq!(slots[0]["min_children"], 1);
        assert_eq!(slots[0]["max_children"], 2);
        assert_eq!(slots[0]["strict"], true);
        let rec = slots[0]["recommended"].as_array().unwrap();
        assert_eq!(rec, &vec![json!(format!("node:{}", badge_id.0)), json!(format!("node:{}", avatar_id.0))]);

        // Verify every recommended ref resolves to a real Component in `doc`
        for r in rec {
            let id_num: u64 = r.as_str().unwrap().strip_prefix("node:").unwrap().parse().unwrap();
            assert!(matches!(doc.get(NodeId(id_num)).map(|n| &n.kind), Some(NodeKind::Component(_))));
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn edge_cases_hidden_vars_300_variants_description_and_fuzzy() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("edge_cases");
        let lib = Library::open(&root).unwrap();

        // Library with a hidden-from-publishing variable and a published variable
        let mut lib_doc = Document::new("Tokens Lib");
        lib_doc.add_page("P");
        lib_doc.variable_collections.insert(
            "c1".into(),
            VariableCollection {
                extensions: Default::default(),
                id: "c1".into(),
                name: "Brand".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec!["v_pub".into(), "v_secret".into()],
                key: None,
                source_library_key: None,
            },
        );
        lib_doc.variables.insert(
            "v_pub".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_pub".into(),
                name: "accent-token".into(),
                collection: "c1".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(10, 20, 30)))]),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
        lib_doc.variables.insert(
            "v_secret".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_secret".into(),
                name: "accent-secret".into(),
                collection: "c1".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(40, 50, 60)))]),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: true,
                key: None,
                code_syntax: Default::default(),
            },
        );
        lib.publish_library("lib-tokens", &lib_doc, "v1").unwrap();

        let mut host = Document::new("Host");
        let page = host.add_page("Page 1");
        host.library.enabled.push(LibraryRef { id: "lib-tokens".into(), name: "Tokens Lib".into(), version: 1 });

        // Component set with 300 variants (10 sizes x 10 tones x 3 states)
        let mega_id = host.alloc_id();
        let mut mega = Node::new(mega_id, NodeKind::ComponentSet(FrameProps::default()));
        mega.name = "MegaChip".into();
        mega.component.description = "Interactive filter pill tag".into();
        host.insert(page, mega).unwrap();
        for s in 0..10 {
            for t in 0..10 {
                for st in ["rest", "hover", "active"] {
                    let vid = host.alloc_id();
                    let mut v = Node::new(vid, NodeKind::Component(FrameProps::default()));
                    v.name = format!("Size=s{s}, Tone=t{t}, State={st}");
                    host.insert(mega_id, v).unwrap();
                }
            }
        }
        assert_eq!(host.get(mega_id).unwrap().children.len(), 300);

        // Style for testing style output
        host.styles.insert("st_fill".into(), Style::Fill(vec![Paint::solid(Color::from_hex("#FF5500").unwrap())]));
        host.style_meta.insert(
            "st_fill".into(),
            StyleMeta {
                name: "Brand/Warm".into(),
                key: None,
                description: "Warm orange fill".into(),
                property: StyleProperty::Fill,
                bindings: Default::default(),
                ..Default::default()
            },
        );

        // Edge case 5: Hidden library variable excluded
        let var_out = search_design_system_at(&host, json!({"query": "accent"}), &root).unwrap();
        let var_results = var_out["results"].as_array().unwrap();
        assert_eq!(var_results.len(), 1);
        assert_eq!(var_results[0]["name"], "accent-token");

        // Edge case 3: 300 variants -> only unique option lists returned
        let mega_out = search_design_system_at(&host, json!({"query": "MegaChip"}), &root).unwrap();
        let mega_res = &mega_out["results"].as_array().unwrap()[0];
        assert_eq!(mega_res["variant_properties"]["Size"]["options"].as_array().unwrap().len(), 10);
        assert_eq!(mega_res["variant_properties"]["Tone"]["options"].as_array().unwrap().len(), 10);
        assert_eq!(mega_res["variant_properties"]["State"]["options"].as_array().unwrap().len(), 3);
        assert_eq!(mega_res["variant_properties"]["Size"]["default"], "s0");

        // Edge case 4: Query matching only description
        let desc_out = search_design_system_at(&host, json!({"query": "pill"}), &root).unwrap();
        let desc_results = desc_out["results"].as_array().unwrap();
        assert_eq!(desc_results.len(), 1);
        assert_eq!(desc_results[0]["name"], "MegaChip");
        assert_eq!(desc_results[0]["score"], 15); // +10 description + 5 local

        // Fuzzy match (Levenshtein == 1 on token >= 4 chars: "megachp" -> "megachip")
        let fuzzy_out = search_design_system_at(&host, json!({"query": "megachp"}), &root).unwrap();
        let fuzzy_results = fuzzy_out["results"].as_array().unwrap();
        assert_eq!(fuzzy_results.len(), 1);
        assert_eq!(fuzzy_results[0]["name"], "MegaChip");
        assert_eq!(fuzzy_results[0]["score"], 20); // +15 fuzzy + 5 local

        // Style result check
        let style_out = search_design_system_at(&host, json!({"query": "Warm", "kinds": ["style"]}), &root).unwrap();
        let style_res = &style_out["results"].as_array().unwrap()[0];
        assert_eq!(style_res["ref"], "style:Brand/Warm");
        assert_eq!(style_res["style_type"], "fill");
        assert_eq!(style_res["summary"], "#FF5500");
        assert_eq!(style_res["example"], json!({"fill": "style:Brand/Warm"}));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ambiguous_variable_paths_and_local_vs_library_same_path_emit_var_id_and_paste_into_apply_spec() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("ambig_vars");
        let lib = Library::open(&root).unwrap();

        // Library has `Color/Primary` (id `v_lib_primary`, #AA2233) and `Color/Secondary` (id `v_shared_id`, #44BB55)
        let mut lib_doc = Document::new("Brand Tokens");
        lib_doc.add_page("Tokens");
        lib_doc.variable_collections.insert(
            "c_color".into(),
            VariableCollection {
                extensions: Default::default(),
                id: "c_color".into(),
                name: "Color".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec!["v_lib_primary".into(), "v_shared_id".into()],
                key: None,
                source_library_key: None,
            },
        );
        lib_doc.variables.insert(
            "v_lib_primary".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_lib_primary".into(),
                name: "Primary".into(),
                collection: "c_color".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(0xAA, 0x22, 0x33)))]),
                description: String::new(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
        lib_doc.variables.insert(
            "v_shared_id".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_shared_id".into(),
                name: "Secondary".into(),
                collection: "c_color".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(0x44, 0xBB, 0x55)))]),
                description: String::new(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
        lib.publish_library("lib-brand", &lib_doc, "v1").unwrap();

        // Host also has `Color/Primary` (id `v_local_primary`, #112233), `Color/Secondary` (id `v_shared_id`, #1199AA),
        // AND two local collections whose slash paths collide on `Theme/Base/Surface` (`v_ambig_a` and `v_ambig_b`).
        let mut host = Document::new("App");
        let page = host.add_page("Page 1");
        host.library.enabled.push(LibraryRef { id: "lib-brand".into(), name: "Brand Tokens".into(), version: 1 });

        host.variable_collections.insert(
            "c_local_color".into(),
            VariableCollection {
                extensions: Default::default(),
                id: "c_local_color".into(),
                name: "Color".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec!["v_local_primary".into(), "v_shared_id".into()],
                key: None,
                source_library_key: None,
            },
        );
        host.variables.insert(
            "v_local_primary".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_local_primary".into(),
                name: "Primary".into(),
                collection: "c_local_color".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(0x11, 0x22, 0x33)))]),
                description: String::new(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
        host.variables.insert(
            "v_shared_id".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_shared_id".into(),
                name: "Secondary".into(),
                collection: "c_local_color".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(0x11, 0x99, 0xAA)))]),
                description: String::new(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );

        // Two local collections both named "Dup" with variable "Accent" -> local-only path collision
        host.variable_collections.insert(
            "c_dup_1".into(),
            VariableCollection {
                extensions: Default::default(),
                id: "c_dup_1".into(),
                name: "Dup".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec!["v_ambig_a".into()],
                key: None,
                source_library_key: None,
            },
        );
        host.variable_collections.insert(
            "c_dup_2".into(),
            VariableCollection {
                extensions: Default::default(),
                id: "c_dup_2".into(),
                name: "Dup".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec!["v_ambig_b".into()],
                key: None,
                source_library_key: None,
            },
        );
        host.variables.insert(
            "v_ambig_a".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_ambig_a".into(),
                name: "Accent".into(),
                collection: "c_dup_1".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(0xEE, 0x11, 0x22)))]),
                description: String::new(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
        host.variables.insert(
            "v_ambig_b".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_ambig_b".into(),
                name: "Accent".into(),
                collection: "c_dup_2".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(0x22, 0x11, 0xEE)))]),
                description: String::new(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );

        let mut engine = crate::engine::Engine { doc: host, active_page_id: Some(page), ..Default::default() };

        // 1. Local duplicate collection path `Dup/Accent` emits `var:id:v_ambig_a` and `var:id:v_ambig_b` and both paste into apply_spec!
        let dup_out = search_design_system_at(&engine.doc, json!({"query": "Accent", "kinds": ["variable"]}), &root).unwrap();
        let dup_results = dup_out["results"].as_array().unwrap();
        assert_eq!(dup_results.len(), 2);
        let hit_a = dup_results.iter().find(|r| r["ref"] == "var:id:v_ambig_a").expect("expected var:id:v_ambig_a");
        let apply_a = engine.call("designos_apply_spec", json!({"spec": hit_a["example"]})).expect("var:id:v_ambig_a example must apply");
        let node_a = engine.doc.get(NodeId(apply_a["root_node_id"].as_u64().unwrap())).unwrap();
        assert_eq!(node_a.variables.bindings.get("fills/0").map(String::as_str), Some("v_ambig_a"));

        // 2. Local vs library `Color/Primary` emits `var:id:...` for BOTH hits, and pasting the library example imports and binds the library token!
        let prim_out = search_design_system_at(&engine.doc, json!({"query": "Primary", "kinds": ["variable"]}), &root).unwrap();
        let prim_results = prim_out["results"].as_array().unwrap();
        assert_eq!(prim_results.len(), 2);
        let local_prim = prim_results.iter().find(|r| r["origin"]["type"] == "local").unwrap();
        let lib_prim = prim_results.iter().find(|r| r["origin"]["type"] == "library").unwrap();
        assert_eq!(local_prim["ref"], "var:id:v_local_primary");
        assert!(lib_prim["ref"].as_str().unwrap().starts_with("var:id:"), "library colliding path must emit var:id:..., got {}", lib_prim["ref"]);

        let apply_lib_prim = engine.call("designos_apply_spec", json!({"spec": lib_prim["example"]})).expect("library Primary example must apply");
        let node_lib_prim = engine.doc.get(NodeId(apply_lib_prim["root_node_id"].as_u64().unwrap())).unwrap();
        assert_eq!(node_lib_prim.variables.bindings.get("fills/0").map(String::as_str), Some("lib-brand/v_lib_primary"));

        // 3. Local vs library `Color/Secondary` where raw ID `v_shared_id` ALSO collides emits library-scoped `var:id:lib-brand/v_shared_id`!
        let sec_out = search_design_system_at(&engine.doc, json!({"query": "Secondary", "kinds": ["variable"]}), &root).unwrap();
        let sec_results = sec_out["results"].as_array().unwrap();
        assert_eq!(sec_results.len(), 2);
        let lib_sec = sec_results.iter().find(|r| r["origin"]["type"] == "library").unwrap();
        assert_eq!(lib_sec["ref"], "var:id:lib-brand/v_shared_id");

        let apply_lib_sec = engine.call("designos_apply_spec", json!({"spec": lib_sec["example"]})).expect("library Secondary example must apply");
        let node_lib_sec = engine.doc.get(NodeId(apply_lib_sec["root_node_id"].as_u64().unwrap())).unwrap();
        assert_eq!(node_lib_sec.variables.bindings.get("fills/0").map(String::as_str), Some("lib-brand/v_shared_id"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_disabled_library_and_local_style_name_collisions_resolve_to_searched_assets() {
        clear_snapshot_cache();
        clear_search_index_cache();
        let root = make_temp_dir("disabled_lib_and_style_collisions");
        let lib = Library::open(&root).unwrap();

        // Build and publish two libraries ("lib-icons" and "lib-brand") with identical internal keys (`s2`, `c4`, `s_heading`, `v1`).
        let build_kit = |doc_name: &str, set_name: &str, glyph_name: &str, font_size: f32, color: Color| -> Document {
            let mut kit = Document::new(doc_name);
            let p = kit.add_page("Components");

            let set_id = kit.alloc_id();
            let v_def_id = kit.alloc_id();
            let glyph_id = kit.alloc_id();

            let mut set_node = Node::new(set_id, NodeKind::ComponentSet(FrameProps::default()));
            set_node.name = set_name.into();
            set_node.component.props.push(ComponentPropDef { name: "icon".into(), kind: PropKind::Slot, default: PropValue::Text(String::new()) });
            set_node.component.slot_configs.insert(
                "icon".into(),
                SlotPropConfig {
                    preferred: vec![glyph_id],
                    min_children: Some(0),
                    max_children: Some(1),
                    allow_preferred_values_only: Some(true),
                    ..Default::default()
                },
            );
            kit.insert(p, set_node).unwrap();

            let mut v_def = Node::new(v_def_id, NodeKind::Component(FrameProps::default()));
            v_def.name = "State=Default".into();
            v_def.size = Vec2::new(100.0, 40.0);
            kit.insert(set_id, v_def).unwrap();

            let mut glyph = Node::new(glyph_id, NodeKind::Component(FrameProps::default()));
            glyph.name = glyph_name.into();
            kit.insert(p, glyph).unwrap();

            let ts = designos_doc::TextStyle { font_size, ..Default::default() };
            kit.styles.insert("s_heading".into(), Style::Text(ts));
            kit.style_meta.insert(
                "s_heading".into(),
                StyleMeta {
                    name: "Heading L".into(),
                    description: format!("{doc_name} heading"),
                    property: StyleProperty::Text,
                    key: None,
                    bindings: Default::default(),
                    ..Default::default()
                },
            );

            kit.variable_collections.insert(
                "c1".into(),
                VariableCollection {
                    extensions: Default::default(),
                    id: "c1".into(),
                    name: "Color".into(),
                    modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                    default_mode: "m1".into(),
                    variables: vec!["v1".into()],
                    key: None,
                    source_library_key: None,
                },
            );
            kit.variables.insert(
                "v1".into(),
                Variable {
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                    id: "v1".into(),
                    name: "Accent".into(),
                    collection: "c1".into(),
                    kind: VariableKind::Color,
                    values: BTreeMap::from([("m1".into(), VariableValue::Color(color))]),
                    description: String::new(),
                    scopes: vec!["ALL_FILLS".into()],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                },
            );
            kit
        };

        let icons_doc = build_kit("Icons Kit", "IconBadge", "MiniGlyph", 18.0, Color::rgb(0x11, 0x22, 0x33));
        let brand_doc = build_kit("Brand Kit", "BrandBadge", "BrandGlyph", 36.0, Color::rgb(0xAA, 0xBB, 0xCC));
        lib.publish_library("lib-icons", &icons_doc, "v1").unwrap();
        lib.publish_library("lib-brand", &brand_doc, "v1").unwrap();

        let mut host = Document::new("App");
        let page = host.add_page("Page 1");
        host.library.enabled.push(LibraryRef { id: "lib-icons".into(), name: "Icons Kit".into(), version: 1 });
        let icons_snaps = enabled_snapshots_from_root(&host, &root);
        let icons_snap = &icons_snaps[0];

        let set_key = icons_snap.assets.iter().find(|a| a.kind == LibraryAssetKind::ComponentSet).unwrap().key.clone();
        let glyph_key = icons_snap.assets.iter().find(|a| a.kind == LibraryAssetKind::Component).unwrap().key.clone();

        // Import `set_key`, `glyph_key`, `style:s_heading`, and `var:v1` from `lib-icons`, then disable `lib-icons`.
        for key in [set_key.as_str(), glyph_key.as_str(), "style:s_heading", "var:v1"] {
            let cmd = import_asset(&mut host, icons_snap, key, None);
            cmd.apply(&mut host).unwrap();
        }
        host.library.enabled.retain(|r| r.id != "lib-icons");
        host.library.enabled.push(LibraryRef { id: "lib-brand".into(), name: "Brand Kit".into(), version: 1 });
        // Warm snapshot cache for `lib-brand`
        let _ = enabled_snapshots_from_root(&host, &root);

        // Add a local style also named "Heading L" (stored under `z_local_heading` so it sorts AFTER `lib-brand/s_heading` in BTreeMap).
        let local_ts = designos_doc::TextStyle { font_size: 24.0, ..Default::default() };
        host.styles.insert("z_local_heading".into(), Style::Text(local_ts));
        host.style_meta.insert(
            "z_local_heading".into(),
            StyleMeta {
                name: "Heading L".into(),
                description: "Local heading".into(),
                property: StyleProperty::Text,
                key: None,
                bindings: Default::default(),
                ..Default::default()
            },
        );

        // Add a local variable `Color/Accent` so `Color/Accent` falls back to `var:id:...`.
        host.variable_collections.insert(
            "c_local".into(),
            VariableCollection {
                extensions: Default::default(),
                id: "c_local".into(),
                name: "Color".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec!["v_local_accent".into()],
                key: None,
                source_library_key: None,
            },
        );
        host.variables.insert(
            "v_local_accent".into(),
            Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v_local_accent".into(),
                name: "Accent".into(),
                collection: "c_local".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color::rgb(0xFF, 0x00, 0x00)))]),
                description: String::new(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );

        let mut engine = crate::engine::Engine { doc: host, active_page_id: Some(page), ..Default::default() };

        // 1. Finding 1: Component & Slot recommended refs collide with leftover disabled `lib-icons` entries.
        let comp_out = search_design_system_at(&engine.doc, json!({"query": "BrandBadge", "kinds": ["component"]}), &root).unwrap();
        let comp_hit = &comp_out["results"].as_array().unwrap()[0];
        assert_eq!(comp_hit["ref"], format!("lib-brand/{set_key}"));
        assert_eq!(comp_hit["slots"][0]["recommended"], json!([format!("lib-brand/{glyph_key}")]));

        let apply_comp = engine.call("designos_apply_spec", json!({"spec": comp_hit["example"]})).unwrap();
        let inst_node = engine.doc.get(NodeId(apply_comp["root_node_id"].as_u64().unwrap())).unwrap();
        let brand_set_local = engine.doc.library.external.get(&local_key("lib-brand", &set_key)).and_then(|e| e.node).unwrap();
        let NodeKind::Instance { component: Some(main_comp), .. } = inst_node.kind else { panic!("expected instance") };
        assert_eq!(
            engine.doc.get(main_comp).unwrap().parent,
            Some(brand_set_local),
            "must instantiate BrandBadge from enabled lib-brand, not disabled lib-icons"
        );

        let slot_rec_ref = comp_hit["slots"][0]["recommended"][0].as_str().unwrap();
        let apply_slot_rec = engine.call("designos_apply_spec", json!({"spec": {"kind": "instance", "component": slot_rec_ref}})).unwrap();
        let glyph_inst = engine.doc.get(NodeId(apply_slot_rec["root_node_id"].as_u64().unwrap())).unwrap();
        let brand_glyph_local = engine.doc.library.external.get(&local_key("lib-brand", &glyph_key)).and_then(|e| e.node).unwrap();
        assert!(matches!(glyph_inst.kind, NodeKind::Instance { component: Some(c), .. } if c == brand_glyph_local));

        // Bare `set_key` also resolves to enabled `lib-brand` rather than disabled `lib-icons`.
        let apply_bare_comp = engine.call("designos_apply_spec", json!({"spec": {"kind": "instance", "component": set_key}})).unwrap();
        let bare_inst = engine.doc.get(NodeId(apply_bare_comp["root_node_id"].as_u64().unwrap())).unwrap();
        let NodeKind::Instance { component: Some(bare_main), .. } = bare_inst.kind else { panic!("expected instance") };
        assert_eq!(engine.doc.get(bare_main).unwrap().parent, Some(brand_set_local));

        // 2. Finding 2a: Style collision between local "Heading L", disabled `lib-icons` "Heading L", and enabled `lib-brand` "Heading L".
        let style_out = search_design_system_at(&engine.doc, json!({"query": "Heading L", "kinds": ["style"]}), &root).unwrap();
        let style_results = style_out["results"].as_array().unwrap();
        assert_eq!(style_results.len(), 2);
        let local_style_hit = style_results.iter().find(|r| r["origin"]["type"] == "local").unwrap();
        let lib_style_hit = style_results.iter().find(|r| r["origin"]["type"] == "library").unwrap();
        assert_eq!(lib_style_hit["ref"], "style:lib-brand/s_heading");
        assert_eq!(lib_style_hit["example"], json!({"text_style": "style:lib-brand/s_heading"}));

        let mut lib_style_spec = lib_style_hit["example"].clone();
        lib_style_spec.as_object_mut().unwrap().insert("text".into(), json!("Brand Heading"));
        let apply_lib_style = engine.call("designos_apply_spec", json!({"spec": lib_style_spec})).unwrap();
        let lib_text_node = engine.doc.get(NodeId(apply_lib_style["root_node_id"].as_u64().unwrap())).unwrap();
        assert_eq!(lib_text_node.style_refs.get("text").map(String::as_str), Some("lib-brand/s_heading"));
        let NodeKind::Text(ref lib_tp) = lib_text_node.kind else { panic!("expected text") };
        assert_eq!(lib_tp.style.font_size, 36.0);

        let mut local_style_spec = local_style_hit["example"].clone();
        local_style_spec.as_object_mut().unwrap().insert("text".into(), json!("Local Heading"));
        let apply_local_style = engine.call("designos_apply_spec", json!({"spec": local_style_spec})).unwrap();
        let local_text_node = engine.doc.get(NodeId(apply_local_style["root_node_id"].as_u64().unwrap())).unwrap();
        assert_eq!(local_text_node.style_refs.get("text").map(String::as_str), Some("z_local_heading"));
        let NodeKind::Text(ref local_tp) = local_text_node.kind else { panic!("expected text") };
        assert_eq!(local_tp.style.font_size, 24.0);

        // 3. Finding 2b: Variable raw ID `v1` collision with leftover disabled `lib-icons/v1`.
        let var_out = search_design_system_at(&engine.doc, json!({"query": "Accent", "kinds": ["variable"]}), &root).unwrap();
        let var_results = var_out["results"].as_array().unwrap();
        assert_eq!(var_results.len(), 2);
        let lib_var_hit = var_results.iter().find(|r| r["origin"]["type"] == "library").unwrap();
        assert_eq!(lib_var_hit["ref"], "var:id:lib-brand/v1");

        let apply_bare_var = engine.call("designos_apply_spec", json!({"spec": {"fill": "var:id:v1"}})).unwrap();
        let bare_var_node = engine.doc.get(NodeId(apply_bare_var["root_node_id"].as_u64().unwrap())).unwrap();
        assert_eq!(bare_var_node.variables.bindings.get("fills/0").map(String::as_str), Some("lib-brand/v1"));

        let apply_lib_var = engine.call("designos_apply_spec", json!({"spec": lib_var_hit["example"]})).unwrap();
        let lib_var_node = engine.doc.get(NodeId(apply_lib_var["root_node_id"].as_u64().unwrap())).unwrap();
        assert_eq!(lib_var_node.variables.bindings.get("fills/0").map(String::as_str), Some("lib-brand/v1"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
