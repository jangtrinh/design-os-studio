//! `.fig` (fig-kiwi) import.
//!
//! Pipeline: container -> kiwi schema + message -> REST-shaped JSON tree ->
//! the same mapper as the REST importer. Field names follow community
//! reverse engineering of Figma's schema; everything is optional and unknown
//! fields are ignored.

use crate::container::{self, Archive};
use crate::fig_export::{CARRIED_FIELDS, CARRIER_KEY_PREFIX, STUDIO_AUTHOR_VISIBLE_KEY, STUDIO_PLUGIN_ID, STUDIO_SLOT_CONTENT_ID_KEY};
use crate::kiwi::{self, KiwiValue, Schema};
use crate::rest_import::{self, fnum, fstr};
use crate::svgpath::write_path;
use crate::FigError;
use designos_doc::{Affine, Document, FillRule, Path, PathCmd, Vec2};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

struct Decoded {
    archive: Archive,
    message: KiwiValue,
    /// Written by Studio's own old exporter: its schema numbers `Message.nodeChanges` 1 (Figma's id is 4) and its
    /// `StrokeCap` has no `CIRCLE_FILLED`.
    studio_connector_caps: bool,
}

/// Studio's own export used to write a circle cap as `ROUND` and a diamond cap as `SQUARE`. Figma's `ROUND` and
/// `SQUARE` are plain stroke ends, so they are read as circle and diamond only in a file that carries Studio's own
/// old schema.
fn remap_studio_connector_caps(change: &mut KiwiValue) {
    let KiwiValue::Message(fields) = change else { return };
    if !fields.iter().any(|(k, v)| &**k == "type" && matches!(v, KiwiValue::Enum(t) if &**t == "CONNECTOR")) {
        return;
    }
    for (name, value) in fields.iter_mut() {
        if !matches!(&**name, "connectorStartCap" | "connectorEndCap" | "connectorStartStrokeCap" | "connectorEndStrokeCap") {
            continue;
        }
        if let KiwiValue::Enum(cap) = value {
            match &**cap {
                "ROUND" => *cap = "CIRCLE_FILLED".into(),
                "SQUARE" => *cap = "DIAMOND_FILLED".into(),
                _ => {}
            }
        }
    }
}

#[inline]
pub(crate) fn release_os_memory() {
    #[cfg(target_os = "macos")]
    unsafe {
        extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
        }
        malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
    }
}

fn decode_inner(bytes: &[u8], for_import: bool) -> Result<Decoded, FigError> {
    let mut archive = container::open(bytes)?;
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive)?;
    archive.canvas = Vec::new();
    let schema = Schema::decode(&schema_bin).map_err(|e| FigError::Format(format!("kiwi schema: {e}")))?;
    drop(schema_bin);
    let root = schema.root_message().ok_or_else(|| FigError::Format("schema has no message definition".into()))?;
    let message = if for_import { schema.decode_message_for_import(root, &msg_bin) } else { schema.decode_message(root, &msg_bin) }
        .map_err(|e| FigError::Format(format!("kiwi message: {e}")))?;
    drop(msg_bin);
    let old_numbering = schema.defs[root].fields.iter().any(|f| &*f.name == "nodeChanges" && f.value == 1);
    let studio_connector_caps = old_numbering && schema.find("StrokeCap").is_some() && !schema.has_enum_variant("StrokeCap", "CIRCLE_FILLED");
    Ok(Decoded { archive, message, studio_connector_caps })
}

fn decode(bytes: &[u8]) -> Result<Decoded, FigError> {
    decode_inner(bytes, false)
}

/// Raw kiwi message as JSON (enums by name, byte arrays as hex) for debugging.
pub fn decode_fig_to_json(bytes: &[u8]) -> Result<Value, FigError> {
    let d = decode(bytes)?;
    let mut out = Map::new();
    out.insert("message".into(), kiwi::to_json(&d.message));
    if let Some(m) = d.archive.meta {
        out.insert("meta".into(), m);
    }
    out.insert("images".into(), Value::Array(d.archive.images.keys().map(|k| Value::String(k.clone())).collect()));
    Ok(Value::Object(out))
}

/// Pre-decoded `derivedSymbolData` entry for fast `.fig` import without JSON intermediate.
#[derive(Debug, Clone)]
pub(crate) struct FastDerivedEntry {
    pub path: Vec<String>,
    pub size: Option<Vec2>,
    pub transform: Option<Affine>,
    pub fill_geometry: Option<Vec<(usize, bool)>>,
    pub stroke_geometry: Option<Vec<(usize, bool)>>,
}

fn kiwi_guid_str(g: &KiwiValue) -> Option<String> {
    let u64_of = |v: &KiwiValue| match v {
        KiwiValue::Uint(u) => Some(*u as u64),
        KiwiValue::Uint64(u) => Some(*u),
        KiwiValue::Int(i) if *i >= 0 => Some(*i as u64),
        KiwiValue::Int64(i) if *i >= 0 => Some(*i as u64),
        _ => None,
    };
    let (s, l) = (u64_of(g.get("sessionID")?)?, u64_of(g.get("localID")?)?);
    // Only the null GUID sentinel is unset; session `u32::MAX` is a real id (paste uses it for its synthetic page).
    if s == u64::from(u32::MAX) && l == u64::from(u32::MAX) {
        return None;
    }
    Some(format!("{s}:{l}"))
}

/// A kiwi float field as `f32`. A NaN or infinity is treated as absent, the way the JSON path does
/// ([`kiwi::clean_f32`] turns it into `null`); this fast path skips that path and used to hand a
/// NaN straight to a node's size.
fn kiwi_f32(v: &KiwiValue, k: &str) -> Option<f32> {
    match v.get(k)? {
        KiwiValue::Float(f) => f.is_finite().then_some(*f),
        KiwiValue::Uint(u) => Some(*u as f32),
        KiwiValue::Int(i) => Some(*i as f32),
        _ => None,
    }
}

fn kiwi_str(v: &KiwiValue) -> Option<&str> {
    match v {
        KiwiValue::String(s) => Some(s.as_str()),
        KiwiValue::Enum(s) => Some(s),
        _ => None,
    }
}

fn extract_fast_derived(raw_changes: &mut [KiwiValue], num_blobs: usize) -> HashMap<String, Vec<FastDerivedEntry>> {
    let mut needs_derived_geo = HashSet::new();
    for nc in raw_changes.iter() {
        if matches!(nc.get("type").and_then(kiwi_str), Some("VECTOR" | "BOOLEAN_OPERATION" | "STAR" | "REGULAR_POLYGON" | "ELLIPSE" | "LINE" | "BRUSH")) {
            if let Some(id) = nc.get("guid").and_then(kiwi_guid_str) {
                needs_derived_geo.insert(id);
            }
        }
    }
    let mut map = HashMap::new();
    for nc in raw_changes.iter_mut() {
        if nc.get("type").and_then(kiwi_str) != Some("INSTANCE") {
            continue;
        }
        let derived_val = nc.take_field("derivedSymbolData").or_else(|| nc.get_mut("symbolData").and_then(|s| s.take_field("derivedSymbolData")));
        let Some(KiwiValue::Array(arr)) = derived_val else {
            continue;
        };
        if arr.is_empty() {
            continue;
        }
        let Some(id) = nc.get("guid").and_then(kiwi_guid_str) else { continue };
        let mut entries = Vec::with_capacity(arr.len());
        for o in arr {
            let p = o.get("guidPath");
            let list = p.and_then(|p| p.get("guids")).or(p).and_then(KiwiValue::as_array);
            let path: Vec<String> = list.map(|a| a.iter().filter_map(kiwi_guid_str).collect()).unwrap_or_default();
            let size = o.get("size").and_then(|s| Some(Vec2::new(kiwi_f32(s, "x")?, kiwi_f32(s, "y")?)));
            let transform = o.get("transform").map(|t| {
                let g = |k: &str, d: f32| kiwi_f32(t, k).unwrap_or(d);
                Affine { a: g("m00", 1.0), c: g("m01", 0.0), tx: g("m02", 0.0), b: g("m10", 0.0), d: g("m11", 1.0), ty: g("m12", 0.0) }
            });
            let parse_geo = |k: &str| -> Option<Vec<(usize, bool)>> {
                let g_arr = o.get(k).and_then(KiwiValue::as_array)?;
                if g_arr.is_empty() {
                    return None;
                }
                let mut geo = Vec::with_capacity(g_arr.len());
                for g in g_arr {
                    let idx = match g.get("commandsBlob") {
                        Some(KiwiValue::Uint(u)) => *u as usize,
                        Some(KiwiValue::Uint64(u)) => *u as usize,
                        Some(KiwiValue::Int(i)) => *i as usize,
                        _ => continue,
                    };
                    if idx >= num_blobs {
                        continue;
                    }
                    let odd = matches!(g.get("windingRule").and_then(kiwi_str), Some("ODD" | "EVENODD"));
                    geo.push((idx, odd));
                }
                (!geo.is_empty()).then_some(geo)
            };
            let keep_geo = path.last().is_some_and(|last| needs_derived_geo.contains(last));
            let fill_geometry = if keep_geo { parse_geo("fillGeometry") } else { None };
            let stroke_geometry = if keep_geo { parse_geo("strokeGeometry") } else { None };
            if size.is_none() && transform.is_none() && fill_geometry.is_none() && stroke_geometry.is_none() {
                continue;
            }
            entries.push(FastDerivedEntry { path, size, transform, fill_geometry, stroke_geometry });
        }
        if !entries.is_empty() {
            entries.shrink_to_fit();
            map.insert(id, entries);
        }
    }
    map
}

/// Imports a `.fig` file (raw fig-kiwi or zip-wrapped) from bytes.
pub fn import_fig(bytes: &[u8]) -> Result<Document, FigError> {
    let mut d = decode_inner(bytes, true)?;
    let name = d.archive.meta.as_ref().and_then(|m| fstr(m, "file_name").or_else(|| fstr(m, "name"))).unwrap_or("Untitled").to_string();
    let owned_blobs: Vec<Vec<u8>> = match d.message.take_field("blobs") {
        Some(KiwiValue::Array(arr)) => arr
            .into_iter()
            .map(|mut b| match b.take_field("bytes") {
                Some(KiwiValue::Bytes(bytes)) => bytes,
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut raw_changes: Vec<KiwiValue> = match d.message.take_field("nodeChanges") {
        Some(KiwiValue::Array(arr)) => arr,
        _ => Vec::new(),
    };
    drop(d.message);

    let large = raw_changes.len() > 5_000;
    let mut fast_derived = large.then(|| extract_fast_derived(&mut raw_changes, owned_blobs.len()));
    if large {
        release_os_memory();
    }
    let into_j: fn(KiwiValue) -> Value = if large { kiwi::into_json_fast } else { kiwi::into_json };
    let mut entries: Vec<Entry> = Vec::with_capacity(raw_changes.len());
    for mut kc in raw_changes {
        if d.studio_connector_caps {
            remap_studio_connector_caps(&mut kc);
        }
        let j = into_j(kc);
        if let Some(entry) = entry_from_json(j) {
            entries.push(entry);
        }
    }
    if large {
        release_os_memory();
    }
    let blob_refs: Vec<&[u8]> = owned_blobs.iter().map(Vec::as_slice).collect();
    let rest = entries_to_rest(entries, &blob_refs, &name);
    if large {
        release_os_memory();
    }
    let mut doc = rest_import::import_rest_value_owned(rest, true, &blob_refs, fast_derived.as_mut())?;
    drop(fast_derived);
    drop(blob_refs);
    drop(owned_blobs);
    doc.images = d.archive.images;
    if large {
        release_os_memory();
    }
    Ok(doc)
}

// ------------------------------------------------------------------ tree

struct Entry {
    id: String,
    parent: Option<String>,
    pos: String,
    json: Value,
}

/// The legacy-typed fields a Studio export carried in `pluginData` as `field:<name>` entries (see
/// `fig_export::grid`). Only the names in [`CARRIED_FIELDS`] are restored.
fn field_carriers(nc: &Value) -> Vec<(&'static str, Value)> {
    let Some(entries) = nc.get("pluginData").and_then(Value::as_array) else { return Vec::new() };
    entries
        .iter()
        .filter(|e| fstr(e, "pluginID") == Some(STUDIO_PLUGIN_ID))
        .filter_map(|e| {
            let name = fstr(e, "key")?.strip_prefix(CARRIER_KEY_PREFIX)?;
            let field = CARRIED_FIELDS.iter().copied().find(|f| *f == name)?;
            Some((field, serde_json::from_str::<Value>(fstr(e, "value")?).ok()?))
        })
        .collect()
}

/// `nc` with its carried fields put back. The carried value replaces what the real field holds, so the legacy
/// readers below take it as they take a file written before the real shapes.
fn with_field_carriers(nc: &Value) -> std::borrow::Cow<'_, Value> {
    let restored = field_carriers(nc);
    if restored.is_empty() {
        return std::borrow::Cow::Borrowed(nc);
    }
    let mut copy = nc.clone();
    if let Some(obj) = copy.as_object_mut() {
        for (field, value) in restored {
            obj.insert(field.into(), value);
        }
    }
    std::borrow::Cow::Owned(copy)
}

fn entry_from_json(j: Value) -> Option<Entry> {
    let j = match with_field_carriers(&j) {
        std::borrow::Cow::Borrowed(_) => j,
        std::borrow::Cow::Owned(restored) => restored,
    };
    if fstr(&j, "phase") == Some("REMOVED") || rest_import::fbool(&j, "isSoftDeleted") == Some(true) {
        return None;
    }
    let id = j.get("guid").and_then(guid_str)?;
    let parent = j.get("parentIndex").and_then(|p| p.get("guid")).and_then(guid_str);
    let pos = j.get("parentIndex").and_then(|p| fstr(p, "position")).unwrap_or("").to_string();
    Some(Entry { id, parent, pos, json: j })
}

fn guid_str(g: &Value) -> Option<String> {
    let (s, l) = (g.get("sessionID")?.as_u64()?, g.get("localID")?.as_u64()?);
    // Only the null GUID sentinel is unset; session `u32::MAX` is a real id (paste uses it for its synthetic page).
    if s == u64::from(u32::MAX) && l == u64::from(u32::MAX) {
        return None;
    }
    Some(format!("{s}:{l}"))
}

/// Library asset key of an `{assetRef: {key, version}}` reference.
pub(crate) fn asset_key(v: &Value) -> Option<&str> {
    v.get("assetRef").and_then(|a| fstr(a, "key"))
}

/// Cross-node lookups of a message: blobs, library keys and property
/// definitions. Library assets (styles, variables, variable sets) are
/// referenced by `{assetRef: {key}}`; files that define them locally use a
/// plain `{guid}` instead.
struct Lk<'a> {
    blobs: &'a [&'a [u8]],
    large_file: bool,
    geo_cache: std::cell::RefCell<HashMap<(usize, bool), Value>>,
    /// Variable key -> `VariableID:s:l`.
    var_by_key: HashMap<String, String>,
    /// Variable-set key -> `VariableCollectionId:s:l`.
    set_by_key: HashMap<String, String>,
    /// Style key -> `s:l` of the style node.
    style_by_key: HashMap<String, String>,
    /// Style key -> its node (for paints/typography the style supplies).
    style_nodes: HashMap<String, Value>,
    /// Style node id (`s:l`) -> node, for styles defined in this file.
    style_by_id: HashMap<String, Value>,
    /// Ids (`s:l`) of style nodes the file marks as deleted: their values are never used and
    /// the nodes that still point at them are left unlinked.
    deleted_styles: std::collections::HashSet<String>,
    /// Property definition id (including the `parentPropDefId` aliases that
    /// variants carry) -> (`Name#s:l`, REST type).
    prop_defs: HashMap<String, (String, &'static str)>,
    /// Component or state-group key (`componentKey` / `key`) -> node guid (`s:l`). Only masters
    /// are indexed; an instance contributes a key no master carries, mapped to the master it instances.
    comp_by_key: HashMap<String, String>,
    /// Node guid (`s:l`) -> Dev Mode handoff status (`READY_FOR_DEV` / `COMPLETED`)
    /// from `CANVAS.handoffStatusMap`.
    handoff_status: HashMap<String, &'static str>,
    /// Grid track id (`s:l`) from any `gridColumns` map -> 0-based column index, or `None` when two
    /// maps place the id differently. Instance overrides name a cell by the track id of the
    /// component's grid, so they resolve against this while the maps are still intact.
    grid_column_index: HashMap<String, Option<usize>>,
    /// Same for `gridRows`.
    grid_row_index: HashMap<String, Option<usize>>,
}

impl<'a> Lk<'a> {
    fn new(entries: &[Entry], blobs: &'a [&'a [u8]]) -> Self {
        let mut lk = Lk {
            blobs,
            large_file: entries.len() > 5_000,
            geo_cache: std::cell::RefCell::new(HashMap::with_capacity(if entries.len() > 5_000 { 0 } else { blobs.len() })),
            var_by_key: HashMap::new(),
            set_by_key: HashMap::new(),
            style_by_key: HashMap::new(),
            style_nodes: HashMap::new(),
            style_by_id: HashMap::new(),
            deleted_styles: std::collections::HashSet::new(),
            prop_defs: HashMap::new(),
            comp_by_key: HashMap::new(),
            handoff_status: HashMap::new(),
            grid_column_index: HashMap::new(),
            grid_row_index: HashMap::new(),
        };
        let mut aliases: Vec<(String, String)> = Vec::new();
        let mut instance_keys: Vec<(String, String)> = Vec::new();
        for e in entries {
            let j = &e.json;
            let key = fstr(j, "key").filter(|k| !k.is_empty());
            let live_style = fstr(j, "styleType").is_some() && !style_soft_deleted(j);
            if live_style {
                lk.style_by_id.insert(e.id.clone(), j.clone());
            } else if fstr(j, "styleType").is_some() {
                lk.deleted_styles.insert(e.id.clone());
            }
            // An instance restates tracks for its own overrides only (see `override_entries`).
            if fstr(j, "type") != Some("INSTANCE") {
                for (field, index) in [("gridColumns", &mut lk.grid_column_index), ("gridRows", &mut lk.grid_row_index)] {
                    for (i, id) in grid_position_guids(j, field).into_iter().enumerate() {
                        let seen = index.entry(id).or_insert(Some(i));
                        if *seen != Some(i) {
                            *seen = None;
                        }
                    }
                }
            }
            for h in j.get("handoffStatusMap").map(|m| rest_import::farr(m, "entries")).unwrap_or(&[]) {
                let Some(gid) = h.get("guid").and_then(guid_str) else { continue };
                let status = h.get("handoffStatus").and_then(|s| fstr(s, "status").or_else(|| s.as_str())).or_else(|| fstr(h, "status"));
                let mapped = match status {
                    Some("BUILD" | "READY_FOR_DEV") => Some("READY_FOR_DEV"),
                    Some("COMPLETED") => Some("COMPLETED"),
                    _ => None,
                };
                if let Some(st) = mapped {
                    lk.handoff_status.insert(gid, st);
                }
            }
            if matches!(fstr(j, "type"), Some("SYMBOL")) || rest_import::fbool(j, "isStateGroup") == Some(true) {
                if let Some(ck) = fstr(j, "componentKey").filter(|k| !k.is_empty()).or(key) {
                    lk.comp_by_key.insert(ck.to_string(), e.id.clone());
                }
            } else if let Some(ck) = fstr(j, "componentKey").filter(|k| !k.is_empty()) {
                // An instance repeats its master's key. It only stands in for the master when
                // nothing else carries the key, and then it points at the master it instances.
                if let Some(master) = j.get("symbolData").and_then(|s| s.get("symbolID")).and_then(guid_str) {
                    instance_keys.push((ck.to_string(), master));
                }
            }
            match (fstr(j, "type"), key) {
                (Some("VARIABLE"), Some(k)) => {
                    lk.var_by_key.insert(k.to_string(), format!("VariableID:{}", e.id));
                }
                (Some("VARIABLE_SET"), Some(k)) => {
                    lk.set_by_key.insert(k.to_string(), format!("VariableCollectionId:{}", e.id));
                }
                (_, Some(k)) if fstr(j, "styleType").is_some() => {
                    lk.style_by_key.insert(k.to_string(), e.id.clone());
                    if live_style {
                        lk.style_nodes.insert(k.to_string(), j.clone());
                    }
                }
                _ => {}
            }
            for d in rest_import::farr(j, "componentPropDefs") {
                let Some(id) = d.get("id").and_then(guid_str) else { continue };
                match fstr(d, "name") {
                    Some(name) => {
                        let ty = match fstr(d, "type") {
                            Some("BOOL") => "BOOLEAN",
                            Some("TEXT") => "TEXT",
                            Some("INSTANCE_SWAP") => "INSTANCE_SWAP",
                            Some("SLOT") => "SLOT",
                            _ => "VARIANT",
                        };
                        let key = if ty == "VARIANT" { name.to_string() } else { format!("{name}#{id}") };
                        lk.prop_defs.insert(id, (key, ty));
                    }
                    None => {
                        if let Some(parent) = d.get("parentPropDefId").and_then(guid_str) {
                            aliases.push((id, parent));
                        }
                    }
                }
            }
        }
        for (ck, master) in instance_keys {
            lk.comp_by_key.entry(ck).or_insert(master);
        }
        for (id, parent) in aliases {
            if let Some(def) = lk.prop_defs.get(&parent).cloned() {
                lk.prop_defs.entry(id).or_insert(def);
            }
        }
        lk
    }

    /// Variable id for an alias value (`{assetRef}` or a local `{guid}`).
    fn variable_id(&self, alias: &Value) -> Option<String> {
        if let Some(k) = asset_key(alias) {
            return self.var_by_key.get(k).cloned();
        }
        alias.get("guid").and_then(guid_str).map(|g| format!("VariableID:{g}"))
    }

    fn set_id(&self, r: &Value) -> Option<String> {
        if let Some(k) = asset_key(r) {
            return self.set_by_key.get(k).cloned();
        }
        r.get("guid").and_then(guid_str).map(|g| format!("VariableCollectionId:{g}"))
    }

    /// Style id of a `styleIdFor*` reference: the style node's id, or
    /// `key:<key>` for a library style that is not in the file.
    fn style_id(&self, r: &Value) -> Option<String> {
        if let Some(k) = asset_key(r) {
            return Some(self.style_by_key.get(k).cloned().unwrap_or_else(|| format!("key:{k}")));
        }
        r.get("guid").and_then(guid_str)
    }

    /// The style node a `styleIdFor*` reference points at.
    fn style_node(&self, r: &Value) -> Option<&Value> {
        match asset_key(r) {
            Some(k) => self.style_nodes.get(k),
            None => self.style_by_id.get(r.get("guid").and_then(guid_str)?.as_str()),
        }
    }

    /// The live style node a consumer points at: its `modern` `styleIdFor*` reference, else the
    /// legacy GUID reference `legacy` (`inherit*StyleID`).
    fn style_node_of(&self, nc: &Value, modern: &str, legacy: &str) -> Option<&Value> {
        nc.get(modern).and_then(|r| self.style_node(r)).or_else(|| self.style_by_id.get(guid_str(nc.get(legacy)?)?.as_str()))
    }

    /// Style id for a consumer's `modern` reference, else its `legacy` GUID reference. None when
    /// the consumer has neither or the style is deleted.
    fn style_id_of(&self, nc: &Value, modern: &str, legacy: Option<&str>) -> Option<String> {
        let id = nc.get(modern).and_then(|r| self.style_id(r)).or_else(|| nc.get(legacy?).and_then(guid_str))?;
        (!self.deleted_styles.contains(&id)).then_some(id)
    }

    fn prop_def(&self, r: &Value) -> Option<&(String, &'static str)> {
        self.prop_defs.get(&r.get("defID").and_then(guid_str)?)
    }

    fn convert_geometry_cached(&self, nc: &Value, key: &str) -> Option<Value> {
        let arr = rest_import::farr(nc, key);
        if arr.is_empty() {
            return None;
        }
        let mut geo = Vec::with_capacity(arr.len());
        for g in arr {
            let Some(idx) = g.get("commandsBlob").and_then(Value::as_u64).map(|x| x as usize) else { continue };
            if idx >= self.blobs.len() {
                continue;
            }
            let odd = matches!(fstr(g, "windingRule"), Some("ODD" | "EVENODD"));
            if self.large_file {
                geo.push(json!({"_blob": idx, "windingRule": if odd { "EVENODD" } else { "NONZERO" }}));
                continue;
            }
            if let Some(cached) = self.geo_cache.borrow().get(&(idx, odd)).cloned() {
                geo.push(cached);
                continue;
            }
            let Some(blob) = self.blobs.get(idx) else { continue };
            let path = commands_blob_to_path(blob, odd);
            let val = json!({"path": write_path(&path), "windingRule": if odd { "EVENODD" } else { "NONZERO" }});
            self.geo_cache.borrow_mut().insert((idx, odd), val.clone());
            geo.push(val);
        }
        (!geo.is_empty()).then_some(Value::Array(geo))
    }
}

/// Builds the REST-shaped `{name, document, ...}` value from a decoded message.
#[allow(dead_code)]
pub(crate) fn message_to_rest(msg: &KiwiValue, name: &str) -> Value {
    let blobs: Vec<&[u8]> = msg
        .get("blobs")
        .and_then(KiwiValue::as_array)
        .map(|a| a.iter().map(|b| b.get("bytes").and_then(KiwiValue::as_bytes).unwrap_or(&[])).collect())
        .unwrap_or_default();
    let raw_changes = msg.get("nodeChanges").and_then(KiwiValue::as_array).unwrap_or(&[]);
    let to_j = if raw_changes.len() > 5_000 { kiwi::to_json_fast } else { kiwi::to_json };
    let changes: Vec<Value> = raw_changes.iter().map(to_j).collect();
    nodes_to_rest(changes, &blobs, name)
}

/// Imports node changes given as kiwi-shaped JSON (`[{guid, parentIndex, type, ...}]`,
/// enums by name), as `fig2json` prints them. For synthetic test fixtures.
#[doc(hidden)]
pub fn import_fig_node_changes(changes: &Value, name: &str) -> Result<Document, FigError> {
    let list = changes.as_array().or_else(|| changes.get("nodeChanges").and_then(Value::as_array)).cloned().unwrap_or_default();
    let owned_blobs: Vec<Vec<u8>> = changes
        .get("blobs")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|b| {
                    b.get("bytes").and_then(Value::as_array).map(|bytes| bytes.iter().filter_map(|x| x.as_u64().map(|n| n as u8)).collect()).unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    let blob_refs: Vec<&[u8]> = owned_blobs.iter().map(Vec::as_slice).collect();
    let rest = nodes_to_rest(list, &blob_refs, name);
    rest_import::import_rest_value(&rest, true)
}

fn nodes_to_rest(changes: Vec<Value>, blobs: &[&[u8]], name: &str) -> Value {
    let entries: Vec<Entry> = changes.into_iter().filter_map(entry_from_json).collect();
    entries_to_rest(entries, blobs, name)
}

fn entries_to_rest(mut entries: Vec<Entry>, blobs: &[&[u8]], name: &str) -> Value {
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();
    {
        let index: HashMap<&str, usize> = entries.iter().enumerate().map(|(i, e)| (e.id.as_str(), i)).collect();
        for (i, e) in entries.iter().enumerate() {
            match e.parent.as_deref().and_then(|p| index.get(p)) {
                Some(&p) if p != i => children.entry(p).or_default().push(i),
                _ => roots.push(i),
            }
        }
    }
    for list in children.values_mut() {
        list.sort_by(|a, b| entries[*a].pos.cmp(&entries[*b].pos));
    }

    let doc_idx = roots.iter().copied().find(|&i| fstr(&entries[i].json, "type") == Some("DOCUMENT"));
    let canvases: Vec<usize> = match doc_idx {
        Some(d) => children.get(&d).cloned().unwrap_or_default(),
        None => roots.iter().copied().filter(|&i| fstr(&entries[i].json, "type") == Some("CANVAS")).collect(),
    };
    let canvas_internal: Vec<(usize, bool)> =
        canvases.into_iter().map(|i| (i, entries[i].json.get("internalOnly").and_then(Value::as_bool) == Some(true))).collect();

    let lk = Lk::new(&entries, blobs);
    let styles = style_table(&entries);
    let variables = {
        let var_entries: Vec<&Value> = entries.iter().map(|e| &e.json).filter(|j| matches!(fstr(j, "type"), Some("VARIABLE_SET" | "VARIABLE"))).collect();
        crate::variables::from_fig_entries(&var_entries)
    };

    let mut visited: HashSet<usize> = HashSet::with_capacity(entries.len());
    let pages: Vec<Value> = canvas_internal
        .iter()
        .copied()
        .filter(|&(_, internal)| !internal)
        .filter_map(|(i, _)| build(&mut entries, &children, &lk, i, &mut visited, 0))
        .map(|mut p| {
            crate::proto::lift_flow_starts(&mut p);
            p
        })
        .collect();
    // Internal-only canvases hold what the file uses from libraries (remote
    // components and styles); they become the hidden library page.
    let library_pages: Vec<Value> = canvas_internal
        .iter()
        .copied()
        .filter(|&(_, internal)| internal)
        .filter_map(|(i, _)| build(&mut entries, &children, &lk, i, &mut visited, 0))
        .filter(|p| p.get("children").and_then(Value::as_array).is_some_and(|c| !c.is_empty()))
        .collect();

    drop(entries);
    drop(children);
    drop(visited);
    drop(lk);

    let mut out = json!({
        "name": name,
        "document": {"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": pages},
        "styles": styles,
    });
    if let (Some(v), Some(o)) = (variables, out.as_object_mut()) {
        o.insert("variables".into(), v);
    }
    if !library_pages.is_empty() {
        out["libraryPages"] = Value::Array(library_pages);
    }
    out
}

/// REST-style `styles` map from shared-style nodes (nodes carrying `styleType`
/// FILL/TEXT/EFFECT/GRID; they normally live on the internal-only canvas). The
/// library key is the node's `key`, the description its `styleDescription`.
fn style_table(entries: &[Entry]) -> Value {
    let mut out = Map::new();
    for e in entries {
        let j = &e.json;
        if style_soft_deleted(j) {
            continue;
        }
        let Some(ty) = fstr(j, "styleType").filter(|t| matches!(*t, "FILL" | "STROKE" | "TEXT" | "EFFECT" | "GRID")) else { continue };
        let key = fstr(j, "key").unwrap_or("");
        let mut m = Map::new();
        if !key.is_empty() {
            m.insert("key".into(), key.into());
        }
        m.insert("name".into(), fstr(j, "name").unwrap_or("").into());
        m.insert("description".into(), fstr(j, "styleDescription").unwrap_or("").into());
        m.insert("styleType".into(), ty.into());
        // The style node is the source of truth for the values it shares.
        match ty {
            "EFFECT" => {
                m.insert("effects".into(), Value::Array(rest_import::farr(j, "effects").iter().filter_map(convert_effect).collect()));
            }
            "GRID" => {
                m.insert("layoutGrids".into(), convert_grids(j).unwrap_or_else(|| Value::Array(Vec::new())));
            }
            _ => {}
        }
        out.insert(e.id.clone(), Value::Object(m));
    }
    Value::Object(out)
}

/// A style node the file marks as deleted (`isSoftDeletedStyle`, or the node-level `isSoftDeleted`).
fn style_soft_deleted(j: &Value) -> bool {
    rest_import::fbool(j, "isSoftDeletedStyle") == Some(true) || rest_import::fbool(j, "isSoftDeleted") == Some(true)
}

fn symbol_id_str(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str().filter(|s| !s.is_empty()) {
        return Some(s.to_string());
    }
    if let Some(g) = v.get("guid").and_then(guid_str).or_else(|| guid_str(v)) {
        return Some(format!("guid:{g}"));
    }
    if let Some(ar) = v.get("assetRef") {
        let key = fstr(ar, "key").unwrap_or("");
        let ver = fstr(ar, "version").unwrap_or("");
        if !key.is_empty() {
            return Some(if ver.is_empty() { format!("assetRef:{key}") } else { format!("assetRef:{key}:{ver}") });
        }
    }
    None
}

const MAPPED_NODE_CHANGE_FIELDS: &[&str] = &[
    "guid",
    "phase",
    "parentIndex",
    "type",
    "name",
    "visible",
    "locked",
    "opacity",
    "size",
    "transform",
    "blendMode",
    "mask",
    "isMask",
    "internalOnly",
    "isStateGroup",
    "backgroundColor",
    "isSoftDeleted",
    "isSoftDeletedStyle",
    "overrideKey",
    "overriddenSymbolID",
    "componentKey",
    "sourceLibraryKey",
    "symbolDescription",
    "description",
    "propsAreBubbled",
    "overrideLevel",
    "detachedSymbolId",
    "symbolData",
    "derivedSymbolData",
    "componentPropDefs",
    "componentPropAssignments",
    "componentPropRefs",
    "stateGroupPropertyValueOrders",
    "variantPropSpecs",
    "proportionsConstrained",
    "preserveRatio",
    "targetAspectRatio",
    "hyperlink",
    "pluginData",
    "exportSettings",
    "guides",
    "devStatus",
    "sectionStatusInfo",
    "handoffStatusMap",
    "key",
    "fillPaints",
    "strokePaints",
    "strokeWeight",
    "strokeAlign",
    "strokeCap",
    "strokeJoin",
    "dashPattern",
    "strokeMiterAngle",
    "miterLimit",
    "borderStrokeWeightsIndependent",
    "borderTopWeight",
    "borderRightWeight",
    "borderBottomWeight",
    "borderLeftWeight",
    "effects",
    "cornerRadius",
    "cornerSmoothing",
    "rectangleCornerRadiiIndependent",
    "rectangleTopLeftCornerRadius",
    "rectangleTopRightCornerRadius",
    "rectangleBottomRightCornerRadius",
    "rectangleBottomLeftCornerRadius",
    "styleIdForFill",
    "styleIdForStrokeFill",
    "styleIdForText",
    "styleIdForEffect",
    "styleIdForGrid",
    "inheritEffectStyleID",
    "inheritGridStyleID",
    "styleType",
    "styleDescription",
    "horizontalConstraint",
    "verticalConstraint",
    "frameMaskDisabled",
    "layoutGrids",
    "stackMode",
    "gridColumns",
    "gridRows",
    "gridColumnsSizing",
    "gridRowsSizing",
    "gridAutoTracks",
    "gridColumnGap",
    "gridColumnsGap",
    "gridRowGap",
    "gridRowsGap",
    "stackSpacing",
    "stackCounterSpacing",
    "stackPadding",
    "stackHorizontalPadding",
    "stackVerticalPadding",
    "stackPaddingLeft",
    "stackPaddingTop",
    "stackPaddingRight",
    "stackPaddingBottom",
    "stackPrimaryAlignItems",
    "stackCounterAlignItems",
    "stackCounterAlignContent",
    "stackLayoutVersion",
    "stackPrimarySizing",
    "stackCounterSizing",
    "stackWrap",
    "bordersTakeSpace",
    "stackReverseZIndex",
    "stackChildPrimaryGrow",
    "stackChildAlignSelf",
    "stackPositioning",
    "layoutSizingHorizontal",
    "layoutSizingVertical",
    "gridColumnSpan",
    "gridRowSpan",
    "gridColumnAnchor",
    "gridColumnAnchorIndex",
    "gridRowAnchor",
    "gridRowAnchorIndex",
    "gridChildHorizontalAlign",
    "gridChildVerticalAlign",
    "minSize",
    "maxSize",
    "variableConsumptionMap",
    "parameterConsumptionMap",
    "variableModeBySetMap",
    "variableSetModes",
    "variableSetID",
    "variableResolvedType",
    "variableDataType",
    "variableScopes",
    "codeSyntax",
    "variableDataValues",
    "sortPosition",
    "fillGeometry",
    "strokeGeometry",
    "vectorData",
    "arcData",
    "count",
    "pointCount",
    "starInnerScale",
    "booleanOperation",
    "textData",
    "fontName",
    "fontSize",
    "letterSpacing",
    "textTracking",
    "lineHeight",
    "textAlignHorizontal",
    "textAlignVertical",
    "textAutoResize",
    "textTruncation",
    "maxLines",
    "textCase",
    "textDecoration",
    "leadingTrim",
    "paragraphIndent",
    "paragraphSpacing",
    "listSpacing",
    "hangingPunctuation",
    "hangingList",
    "autoRename",
    "textDecorationSkipInk",
    "textWrapStyle",
    "toggledOnOTFeatures",
    "toggledOffOTFeatures",
    "fontVariations",
    "fontVariantNumericFigure",
    "fontVariantNumericSpacing",
    "fontVariantNumericFraction",
    "fontVariantCaps",
    "fontVariantPosition",
    "fontVariantLigatures",
    "fontVariantCommonLigatures",
    "fontVariantContextualLigatures",
    "numericFigure",
    "numericSpacing",
    "numericFraction",
    "authorName",
    "authorVisible",
    "stampType",
    "connectorStart",
    "connectorEnd",
    "connectorLineType",
    "connectorLineStyle",
    "connectorType",
    "connectorStartStrokeCap",
    "connectorEndStrokeCap",
    "connectorStartCap",
    "connectorEndCap",
    "prototypeInteractions",
    "overflowDirection",
    "scrollBehavior",
    "overlayPositionType",
    "overlayBackgroundInteraction",
    "overlayBackgroundAppearance",
    "flowStartingPoint",
    "isFlowStartingPoint",
    "transitionNodeID",
    "transitionDuration",
    "transitionEasing",
    "guidPath",
    "isSlot",
    "isSlotContent",
    "jsonSerialized",
];

fn clean_json_floats(v: &Value) -> Value {
    match v {
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                Value::Number(n.clone())
            } else if let Some(f) = n.as_f64() {
                kiwi::clean_f32(f as f32)
            } else {
                Value::Number(n.clone())
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(clean_json_floats).collect()),
        Value::Object(o) => {
            let mut m = Map::with_capacity(o.len());
            for (k, val) in o {
                m.insert(k.clone(), clean_json_floats(val));
            }
            Value::Object(m)
        }
        other => other.clone(),
    }
}

fn extract_fig_metadata(nc: &Value, orig_ty: &str, large_file: bool) -> Option<Value> {
    let obj = nc.as_object()?;
    let mut meta = Map::new();
    let mut has_brush_type = false;
    let mut has_swt = false;
    for (k, val) in obj {
        match k.as_str() {
            "figMetadata" => {
                if let Some(existing) = val.as_object() {
                    for (ek, ev) in existing {
                        if let Some(s) = ev.as_str() {
                            meta.insert(ek.clone(), Value::String(s.to_string()));
                        }
                    }
                }
            }
            "brushType" => {
                if let Some(bt) = val.as_str().filter(|s| !s.is_empty() || orig_ty == "BRUSH") {
                    meta.insert("brushType".into(), Value::String(bt.to_string()));
                    has_brush_type = true;
                }
            }
            "shapeWithTextType" => {
                if let Some(swt) = val.as_str().filter(|s| !s.is_empty()) {
                    meta.insert("shapeWithTextType".into(), Value::String(swt.to_string()));
                    has_swt = true;
                }
            }
            "shapeType" if orig_ty == "SHAPE_WITH_TEXT" && !has_swt => {
                if let Some(swt) = val.as_str().filter(|s| !s.is_empty()) {
                    meta.insert("shapeWithTextType".into(), Value::String(swt.to_string()));
                }
            }
            "userFacingVersion"
            | "sharedSymbolVersion"
            | "publishedVersion"
            | "version"
            | "maskType"
            | "scrollDirection"
            | "lockMode"
            | "codeExportName"
            | "documentColorProfile"
            | "codeObjectType" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    meta.insert(k.clone(), Value::String(s.to_string()));
                }
            }
            "isSymbolPublishable"
            | "isPublishable"
            | "resizeToFit"
            | "shapeTruncates"
            | "isPageDivider"
            | "backgroundEnabled"
            | "gridReflowEnabled"
            | "isCollectionExtendable"
            | "shouldHideCursorsOnWidgetHover"
            | "isWidgetStickable" => {
                if let Some(b) = val.as_bool() {
                    meta.insert(k.clone(), Value::String(if b { "true".into() } else { "false".into() }));
                }
            }
            "shapeUserHeight" | "backgroundOpacity" | "connectorVersion" | "vectorOperationVersion" => {
                if let Some(n) = rest_import::num(val) {
                    meta.insert(k.clone(), Value::String(n.to_string()));
                }
            }
            "publishID" => {
                if let Some(g) = guid_str(val) {
                    meta.insert("publishID".into(), Value::String(g));
                }
            }
            "editInfo" => {
                if !large_file && val.is_object() {
                    if let Ok(s) = serde_json::to_string(&clean_json_floats(val)) {
                        meta.insert("editInfo".into(), Value::String(s));
                    }
                }
            }
            "widgetCachedAncestor" => {
                if let Some(g) = val.get("nodeId").and_then(guid_str).or_else(|| guid_str(val)) {
                    meta.insert("widgetCachedAncestor".into(), Value::String(g));
                }
            }
            "pluginRelaunchData" => {
                if val.as_array().is_some_and(|a| !a.is_empty()) {
                    if let Ok(s) = serde_json::to_string(&clean_json_floats(val)) {
                        meta.insert("pluginRelaunchData".into(), Value::String(s));
                    }
                }
            }
            "nodeGenerationData" if val.is_object() => {
                if let Ok(s) = serde_json::to_string(&clean_json_floats(val)) {
                    meta.insert("nodeGenerationData".into(), Value::String(s));
                }
            }
            "derivedTextData" | "derivedImmutableFrameData" => {
                if !large_file && !val.is_null() {
                    if let Ok(s) = serde_json::to_string(&clean_json_floats(val)) {
                        meta.insert(k.clone(), Value::String(s));
                    }
                }
            }
            other if !MAPPED_NODE_CHANGE_FIELDS.contains(&other) && !other.starts_with('_') => match val {
                Value::Null => {}
                Value::String(s) if !s.is_empty() => {
                    meta.insert(k.clone(), Value::String(s.clone()));
                }
                Value::Bool(b) => {
                    meta.insert(k.clone(), Value::String(if *b { "true".into() } else { "false".into() }));
                }
                Value::Number(_) => {
                    let cleaned = clean_json_floats(val);
                    if !cleaned.is_null() {
                        meta.insert(k.clone(), Value::String(cleaned.to_string()));
                    }
                }
                Value::Array(a) if !a.is_empty() => {
                    if let Ok(s) = serde_json::to_string(&clean_json_floats(val)) {
                        meta.insert(k.clone(), Value::String(s));
                    }
                }
                Value::Object(o) if !o.is_empty() => {
                    if let Ok(s) = serde_json::to_string(&clean_json_floats(val)) {
                        meta.insert(k.clone(), Value::String(s));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    if orig_ty == "BRUSH" && !has_brush_type {
        meta.insert("brushType".into(), Value::String("STRETCH".into()));
    }
    (!meta.is_empty()).then_some(Value::Object(meta))
}

fn grid_position_guids(nc: &Value, key: &str) -> Vec<String> {
    let Some(entries) = nc.get(key).and_then(|v| v.get("entries")).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut pairs: Vec<(&str, String)> = entries
        .iter()
        .filter_map(|e| {
            let id = e.get("id").and_then(guid_str)?;
            let pos = fstr(e, "position").unwrap_or("");
            Some((pos, id))
        })
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs.into_iter().map(|(_, id)| id).collect()
}

fn format_v106_track_size(ts: &Value) -> Option<String> {
    let sizing = ts.get("minSizing").or_else(|| ts.get("maxSizing"))?;
    match fstr(sizing, "type")? {
        "FLEX" => {
            let v = fnum(sizing, "value").unwrap_or(1.0);
            if (v - v.round()).abs() < 1e-4 {
                Some(format!("{}fr", v.round() as i32))
            } else {
                Some(format!("{v}fr"))
            }
        }
        "FIXED" => {
            let v = fnum(sizing, "value").unwrap_or(0.0);
            if (v - v.round()).abs() < 1e-4 {
                Some(format!("{}px", v.round() as i32))
            } else {
                Some(format!("{v}px"))
            }
        }
        "HUG" => Some("auto".to_string()),
        _ => None,
    }
}

fn convert_grid_track_fields(m: &mut Map<String, Value>, nc: &Value) {
    for (count_key, sizing_key, guids_key) in [("gridColumns", "gridColumnsSizing", "_gridColumnTrackIds"), ("gridRows", "gridRowsSizing", "_gridRowTrackIds")]
    {
        let ordered_guids = grid_position_guids(nc, count_key);
        if !ordered_guids.is_empty() {
            m.insert(count_key.into(), json!(ordered_guids.len()));
            m.insert(guids_key.into(), json!(ordered_guids));
        } else {
            copy(m, nc, count_key, count_key);
        }
        if let Some(sz) = nc.get(sizing_key) {
            if sz.is_string() {
                m.insert(sizing_key.into(), sz.clone());
            } else if let Some(entries) = sz.get("entries").and_then(Value::as_array) {
                let mut by_id: HashMap<String, String> = HashMap::new();
                let mut in_order: Vec<String> = Vec::new();
                for e in entries {
                    if let Some(t) = e.get("trackSize").and_then(format_v106_track_size) {
                        if let Some(id) = e.get("id").and_then(guid_str) {
                            by_id.insert(id, t.clone());
                        }
                        in_order.push(t);
                    }
                }
                let tracks: Vec<String> = if !ordered_guids.is_empty() {
                    ordered_guids.iter().map(|id| by_id.get(id).cloned().unwrap_or_else(|| "1fr".to_string())).collect()
                } else {
                    in_order
                };
                if !tracks.is_empty() {
                    m.insert(sizing_key.into(), Value::String(tracks.join(" ")));
                }
            }
        }
    }
    copy(m, nc, "gridAutoTracks", "gridAutoTracks");
}

fn build(entries: &mut [Entry], children: &HashMap<usize, Vec<usize>>, lk: &Lk, i: usize, visited: &mut HashSet<usize>, depth: usize) -> Option<Value> {
    if depth > 250 || !visited.insert(i) {
        return None;
    }
    let node_json = std::mem::take(&mut entries[i].json);
    let is_container = matches!(fstr(&node_json, "type"), Some("CANVAS" | "FRAME" | "GROUP" | "SYMBOL" | "INSTANCE" | "SECTION" | "BOOLEAN_OPERATION"));
    let col_guids = grid_position_guids(&node_json, "gridColumns");
    let row_guids = grid_position_guids(&node_json, "gridRows");
    let mut m = convert_node(&node_json, &entries[i].id, lk)?;
    drop(node_json);
    let mut kids: Vec<Value> =
        children.get(&i).map(|c| c.iter().filter_map(|&k| build(entries, children, lk, k, visited, depth + 1)).collect()).unwrap_or_default();
    if !col_guids.is_empty() || !row_guids.is_empty() {
        for kid in &mut kids {
            if let Some(km) = kid.as_object_mut() {
                if let Some(cg) = km.get("gridColumnAnchorIndex").and_then(guid_str) {
                    if let Some(idx) = col_guids.iter().position(|id| id == &cg) {
                        km.insert("gridColumnAnchorIndex".into(), json!(idx));
                    }
                }
                if let Some(rg) = km.get("gridRowAnchorIndex").and_then(guid_str) {
                    if let Some(idx) = row_guids.iter().position(|id| id == &rg) {
                        km.insert("gridRowAnchorIndex".into(), json!(idx));
                    }
                }
            }
        }
    }
    if !kids.is_empty() || is_container {
        m.insert("children".into(), Value::Array(kids));
    }
    Some(Value::Object(m))
}

// ------------------------------------------------------------------ node

fn copy(m: &mut Map<String, Value>, from: &Value, src: &str, dst: &str) {
    if let Some(v) = from.get(src) {
        m.insert(dst.into(), v.clone());
    }
}

fn enum_map(m: &mut Map<String, Value>, from: &Value, src: &str, dst: &str, table: &[(&str, &str)]) {
    if let Some(s) = fstr(from, src) {
        let out = table.iter().find(|(k, _)| *k == s).map(|(_, v)| *v).unwrap_or(s);
        m.insert(dst.into(), out.into());
    }
}

/// REST `exportSettings` from kiwi export settings: `imageType` (PNG/JPEG/SVG/PDF) and a
/// `constraint` whose type is CONTENT_SCALE / CONTENT_WIDTH / CONTENT_HEIGHT.
fn convert_export_settings(nc: &Value) -> Option<Value> {
    let list = rest_import::farr(nc, "exportSettings");
    let out: Vec<Value> = list
        .iter()
        .map(|e| {
            let format = match fstr(e, "imageType").or_else(|| fstr(e, "format")) {
                Some("JPEG") => "JPG",
                Some(other) => other,
                None => "PNG",
            };
            let c = e.get("constraint");
            let ty = match c.and_then(|c| fstr(c, "type")) {
                Some("CONTENT_WIDTH") | Some("WIDTH") => "WIDTH",
                Some("CONTENT_HEIGHT") | Some("HEIGHT") => "HEIGHT",
                _ => "SCALE",
            };
            let value = c.and_then(|c| fnum(c, "value")).unwrap_or(1.0);
            json!({"suffix": fstr(e, "suffix").unwrap_or(""), "format": format, "constraint": {"type": ty, "value": value}})
        })
        .collect();
    (!out.is_empty()).then_some(Value::Array(out))
}

/// REST-shaped `guides` (`[{axis, offset}]`) from kiwi guides.
fn convert_guides(nc: &Value) -> Option<Value> {
    let out: Vec<Value> = rest_import::farr(nc, "guides")
        .iter()
        .filter_map(|g| {
            let axis = fstr(g, "axis")?;
            let offset = fnum(g, "offset").or_else(|| fnum(g, "position"))?;
            Some(json!({"axis": axis, "offset": offset}))
        })
        .collect();
    (!out.is_empty()).then_some(Value::Array(out))
}

fn convert_node(nc: &Value, id: &str, lk: &Lk) -> Option<Map<String, Value>> {
    let blobs = lk.blobs;
    if fstr(nc, "styleType").is_some() {
        return None;
    }
    let orig_ty = fstr(nc, "type")?;
    let ty = match orig_ty {
        // Variables are mapped separately (`variables::from_fig_entries`); they are not canvas content.
        "DOCUMENT" | "NONE" | "VARIABLE" | "VARIABLE_SET" | "VARIABLE_OVERRIDE" => return None,
        "SYMBOL" => "COMPONENT",
        "ROUNDED_RECTANGLE" => "RECTANGLE",
        "BRUSH" => "VECTOR",
        "FRAME" if nc.get("isStateGroup").and_then(Value::as_bool) == Some(true) => "COMPONENT_SET",
        other => other,
    };
    let mut m = Map::new();
    m.insert("id".into(), id.into());
    m.insert("type".into(), ty.into());
    // Override paths address nodes by `overrideKey`, not by guid.
    if let Some(k) = nc.get("overrideKey").and_then(guid_str) {
        m.insert("_overrideKey".into(), k.into());
    }
    copy(&mut m, nc, "name", "name");
    copy(&mut m, nc, "componentKey", "componentKey");
    // A symbol or state group indexed by its publish `key` (see `Lk::new`) keeps that key the way
    // `componentKey` is kept, so export writes it back. An explicit `componentKey` wins.
    if !m.contains_key("componentKey") && matches!(ty, "COMPONENT" | "COMPONENT_SET") {
        if let Some(k) = fstr(nc, "key").filter(|k| !k.is_empty()) {
            m.insert("componentKey".into(), Value::String(k.to_string()));
        }
    }
    copy(&mut m, nc, "sourceLibraryKey", "sourceLibraryKey");
    if let Some(desc) = fstr(nc, "symbolDescription").or_else(|| fstr(nc, "description")).filter(|s| !s.is_empty()) {
        m.insert("symbolDescription".into(), Value::String(desc.to_string()));
    }
    if nc.get("propsAreBubbled").and_then(Value::as_bool) == Some(true) {
        m.insert("propsAreBubbled".into(), Value::Bool(true));
    }
    if let Some(lvl) = nc.get("overrideLevel").and_then(Value::as_i64) {
        m.insert("overrideLevel".into(), json!(lvl));
    }
    if let Some(ds) = nc.get("detachedSymbolId").and_then(symbol_id_str) {
        m.insert("detachedSymbolId".into(), Value::String(ds));
    }
    for flag in ["isSlot", "isSlotContent"] {
        if nc.get(flag).and_then(Value::as_bool) == Some(true) {
            m.insert(flag.into(), Value::Bool(true));
        }
    }
    // Slot content guid: the editor reads it from the `jsonSerialized` bag, key `slot-content-id`.
    if let Some(g) = nc.get("jsonSerialized").and_then(|j| j.get("slot-content-id")).and_then(|s| s.get("guid")).and_then(guid_str) {
        m.insert("slotContentId".into(), Value::String(g));
    }
    if let Some(fm) = extract_fig_metadata(nc, orig_ty, lk.large_file) {
        m.insert("figMetadata".into(), fm);
    }
    copy(&mut m, nc, "visible", "visible");
    copy(&mut m, nc, "locked", "locked");
    copy(&mut m, nc, "opacity", "opacity");
    copy(&mut m, nc, "blendMode", "blendMode");
    copy(&mut m, nc, "mask", "isMask");
    if nc.get("proportionsConstrained").and_then(Value::as_bool) == Some(true)
        || nc.get("targetAspectRatio").and_then(|v| v.get("value")).is_some()
        || nc.get("targetAspectRatio").and_then(Value::as_object).is_some_and(|o| !o.is_empty())
    {
        m.insert("preserveRatio".into(), Value::Bool(true));
    } else {
        copy(&mut m, nc, "preserveRatio", "preserveRatio");
    }
    if let Some(hl) = nc.get("hyperlink") {
        if let Some(u) = fstr(hl, "url").filter(|s| !s.is_empty()) {
            m.insert("hyperlink".into(), Value::String(u.to_string()));
        } else if let Some(g) = hl.get("guid").and_then(guid_str) {
            m.insert("hyperlink".into(), Value::String(format!("guid:{g}")));
        } else if let Some(s) = hl.as_str().filter(|s| !s.is_empty()) {
            m.insert("hyperlink".into(), Value::String(s.to_string()));
        }
    }
    if let Some(pd_arr) = nc.get("pluginData").and_then(Value::as_array) {
        let mut pd_map = Map::new();
        for entry in pd_arr {
            let Some(k) = fstr(entry, "key") else { continue };
            let val = fstr(entry, "value").unwrap_or("");
            let pid = fstr(entry, "pluginID").unwrap_or("");
            // Studio's reserved carrier entries become typed node data, not plugin data.
            if pid == STUDIO_PLUGIN_ID {
                if k.starts_with(CARRIER_KEY_PREFIX) {
                    continue;
                }
                match k {
                    STUDIO_SLOT_CONTENT_ID_KEY => {
                        if !val.is_empty() {
                            m.insert("slotContentId".into(), Value::String(val.to_string()));
                        }
                        continue;
                    }
                    STUDIO_AUTHOR_VISIBLE_KEY => {
                        m.insert("authorVisible".into(), Value::Bool(val == "true"));
                        continue;
                    }
                    _ => {}
                }
            }
            let full_key = if pid.is_empty() { k.to_string() } else { format!("{pid}:{k}") };
            pd_map.insert(full_key, Value::String(val.to_string()));
        }
        if !pd_map.is_empty() {
            m.insert("pluginData".into(), Value::Object(pd_map));
        }
    }
    copy(&mut m, nc, "size", "size");
    if let Some(t) = nc.get("transform") {
        m.insert("relativeTransform".into(), matrix_json(t));
    }
    if let Some(e) = convert_export_settings(nc) {
        m.insert("exportSettings".into(), e);
    }
    if let Some(g) = convert_guides(nc) {
        m.insert("guides".into(), g);
    }
    if let Some(ds) = nc.get("devStatus") {
        m.insert("devStatus".into(), ds.clone());
    } else if let Some(st) = nc.get("sectionStatusInfo").and_then(|s| fstr(s, "status")).or_else(|| lk.handoff_status.get(id).copied()) {
        let mapped = match st {
            "BUILD" => "READY_FOR_DEV",
            other => other,
        };
        m.insert("devStatus".into(), json!({"type": mapped}));
    }
    if ty == "CANVAS" {
        copy(&mut m, nc, "backgroundColor", "backgroundColor");
        return Some(m);
    }
    crate::proto::convert_fig(&mut m, nc);

    // Paint.
    // A style the file defines wins over the node's own (possibly stale) copy of its values,
    // unless a variable is bound to one of the paints.
    let paints = |k: &str, style_key: &str| -> Vec<Value> {
        let own = rest_import::farr(nc, k);
        let from_style = nc.get(style_key).and_then(|r| lk.style_node(r)).filter(|_| !own.iter().any(|p| p.get("colorVar").is_some()));
        let list = from_style.map(|s| rest_import::farr(s, "fillPaints")).unwrap_or(own);
        list.iter().filter_map(convert_paint).collect()
    };
    let fills = paints("fillPaints", "styleIdForFill");
    if !fills.is_empty() {
        m.insert("fills".into(), Value::Array(fills));
    }
    let strokes = paints("strokePaints", "styleIdForStrokeFill");
    if !strokes.is_empty() {
        m.insert("strokes".into(), Value::Array(strokes));
    }
    copy(&mut m, nc, "strokeWeight", "strokeWeight");
    copy(&mut m, nc, "strokeAlign", "strokeAlign");
    enum_map(&mut m, nc, "strokeCap", "strokeCap", &[("ARROW_LINES", "NONE"), ("ARROW_EQUILATERAL", "NONE")]);
    copy(&mut m, nc, "strokeJoin", "strokeJoin");
    copy(&mut m, nc, "dashPattern", "strokeDashes");
    copy(&mut m, nc, "strokeMiterAngle", "strokeMiterAngle");
    copy(&mut m, nc, "miterLimit", "strokeMiterLimit");
    if nc.get("borderStrokeWeightsIndependent").and_then(Value::as_bool) == Some(true) {
        let g = |k: &str| fnum(nc, k).unwrap_or(0.0);
        m.insert(
            "individualStrokeWeights".into(),
            json!({"top": g("borderTopWeight"), "right": g("borderRightWeight"), "bottom": g("borderBottomWeight"), "left": g("borderLeftWeight")}),
        );
    }
    // Effects follow the same rule as paints: the style node's list wins unless a variable is bound.
    let own_effects = rest_import::farr(nc, "effects");
    let fx_style = lk.style_node_of(nc, "styleIdForEffect", "inheritEffectStyleID").filter(|_| !own_effects.iter().any(|e| e.get("colorVar").is_some()));
    let effects: Vec<Value> = fx_style.map(|s| rest_import::farr(s, "effects")).unwrap_or(own_effects).iter().filter_map(convert_effect).collect();
    if !effects.is_empty() {
        m.insert("effects".into(), Value::Array(effects));
    }

    // Corners.
    copy(&mut m, nc, "cornerRadius", "cornerRadius");
    copy(&mut m, nc, "cornerSmoothing", "cornerSmoothing");
    if nc.get("rectangleCornerRadiiIndependent").and_then(Value::as_bool) == Some(true)
        || ["rectangleTopLeftCornerRadius", "rectangleTopRightCornerRadius", "rectangleBottomRightCornerRadius", "rectangleBottomLeftCornerRadius"]
            .iter()
            .any(|k| nc.get(*k).is_some())
    {
        let base = fnum(nc, "cornerRadius").unwrap_or(0.0);
        let g = |k: &str| fnum(nc, k).unwrap_or(base);
        m.insert(
            "rectangleCornerRadii".into(),
            json!([
                g("rectangleTopLeftCornerRadius"),
                g("rectangleTopRightCornerRadius"),
                g("rectangleBottomRightCornerRadius"),
                g("rectangleBottomLeftCornerRadius")
            ]),
        );
    }

    // Constraints.
    let h = match fstr(nc, "horizontalConstraint") {
        Some("CENTER") => "CENTER",
        Some("MAX") => "RIGHT",
        Some("STRETCH") => "LEFT_RIGHT",
        Some("SCALE") => "SCALE",
        _ => "LEFT",
    };
    let v = match fstr(nc, "verticalConstraint") {
        Some("CENTER") => "CENTER",
        Some("MAX") => "BOTTOM",
        Some("STRETCH") => "TOP_BOTTOM",
        Some("SCALE") => "SCALE",
        _ => "TOP",
    };
    if h != "LEFT" || v != "TOP" {
        m.insert("constraints".into(), json!({"horizontal": h, "vertical": v}));
    }

    // Frames / auto layout.
    if matches!(ty, "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE") {
        let masked = nc.get("frameMaskDisabled").and_then(Value::as_bool) != Some(true);
        m.insert("clipsContent".into(), masked.into());
        convert_stack(&mut m, nc);
        if let Some(g) = convert_grids(lk.style_node_of(nc, "styleIdForGrid", "inheritGridStyleID").unwrap_or(nc)) {
            m.insert("layoutGrids".into(), g);
        }
    }
    // Child layout.
    if let Some(g) = fnum(nc, "stackChildPrimaryGrow") {
        m.insert("layoutGrow".into(), g.into());
    }
    if let Some(a) = fstr(nc, "stackChildAlignSelf") {
        m.insert("layoutAlign".into(), a.into());
    }
    if fstr(nc, "stackPositioning") == Some("ABSOLUTE") {
        m.insert("layoutPositioning".into(), "ABSOLUTE".into());
    }
    for k in ["stackChildMarginTop", "stackChildMarginRight", "stackChildMarginBottom", "stackChildMarginLeft"] {
        if let Some(v) = fnum(nc, k) {
            m.insert(k.into(), v.into());
        }
    }
    for k in ["gridColumnSpan", "gridRowSpan", "gridChildHorizontalAlign", "gridChildVerticalAlign"] {
        copy(&mut m, nc, k, k);
    }
    if let Some(c) = nc.get("gridColumnAnchor").or_else(|| nc.get("gridColumnAnchorIndex")) {
        if let Some(g) = guid_str(c) {
            m.insert("_gridColumnAnchorGuid".into(), g.into());
        }
        m.insert("gridColumnAnchorIndex".into(), c.clone());
    }
    if let Some(r) = nc.get("gridRowAnchor").or_else(|| nc.get("gridRowAnchorIndex")) {
        if let Some(g) = guid_str(r) {
            m.insert("_gridRowAnchorGuid".into(), g.into());
        }
        m.insert("gridRowAnchorIndex".into(), r.clone());
    }
    for (src, dst_x, dst_y) in [("minSize", "minWidth", "minHeight"), ("maxSize", "maxWidth", "maxHeight")] {
        if let Some(s) = nc.get(src) {
            let s = s.get("value").unwrap_or(s);
            // Figma writes 0 for a minimum that is not set and infinity (no number here) for a maximum that is not.
            let unset = |v: f32| src == "minSize" && v == 0.0;
            if let Some(x) = fnum(s, "x").filter(|v| !unset(*v)) {
                m.insert(dst_x.into(), x.into());
            }
            if let Some(y) = fnum(s, "y").filter(|v| !unset(*v)) {
                m.insert(dst_y.into(), y.into());
            }
        }
    }

    // Style references (`{assetRef: {key}}` for library styles, `{guid}` for local ones).
    let mut styles = Map::new();
    for (src, legacy, dst) in [
        ("styleIdForFill", None, "fill"),
        ("styleIdForStrokeFill", None, "stroke"),
        ("styleIdForText", None, "text"),
        ("styleIdForEffect", Some("inheritEffectStyleID"), "effect"),
        ("styleIdForGrid", Some("inheritGridStyleID"), "grid"),
    ] {
        if let Some(g) = lk.style_id_of(nc, src, legacy) {
            styles.insert(dst.into(), g.into());
        }
    }
    if !styles.is_empty() {
        m.insert("styles".into(), Value::Object(styles));
    }
    if let Some(b) = bound_variables(nc, lk) {
        m.insert("boundVariables".into(), b);
    }
    if let Some(modes) = explicit_modes(nc, lk) {
        m.insert("explicitVariableModes".into(), modes);
    }
    component_props(&mut m, nc, lk);

    // Type-specific.
    match ty {
        "ELLIPSE" => copy(&mut m, nc, "arcData", "arcData"),
        "STAR" => {
            if let Some(c) = nc.get("count").or_else(|| nc.get("pointCount")) {
                m.insert("pointCount".into(), c.clone());
            }
            copy(&mut m, nc, "starInnerScale", "starInnerRadius");
        }
        "REGULAR_POLYGON" => {
            if let Some(c) = nc.get("count").or_else(|| nc.get("pointCount")) {
                m.insert("pointCount".into(), c.clone());
            }
        }
        "BOOLEAN_OPERATION" => enum_map(&mut m, nc, "booleanOperation", "booleanOperation", &[("XOR", "EXCLUDE")]),
        "INSTANCE" => {
            let sd = nc.get("symbolData");
            if let Some(g) = sd.and_then(|s| s.get("symbolID")).and_then(guid_str) {
                m.insert("componentId".into(), g.into());
            }
            if let Some(s) = sd.and_then(|s| s.get("uniformScaleFactor")) {
                m.insert("_uniformScale".into(), s.clone());
            }
            let ov = override_entries(nc, sd.and_then(|s| s.get("symbolOverrides")), lk, false);
            if !ov.is_empty() {
                m.insert("_overrides".into(), Value::Array(ov));
            }
            let derived = override_entries(nc, nc.get("derivedSymbolData").or_else(|| sd.and_then(|s| s.get("derivedSymbolData"))), lk, true);
            if !derived.is_empty() {
                m.insert("_derived".into(), Value::Array(derived));
            }
        }
        "TEXT" => convert_text(&mut m, nc, lk),
        "STICKY" => {
            copy(&mut m, nc, "authorName", "authorName");
            copy(&mut m, nc, "authorVisible", "authorVisible");
            convert_text(&mut m, nc, lk);
        }
        "SHAPE_WITH_TEXT" => {
            if let Some(st) = nc.get("shapeWithTextType").or_else(|| nc.get("shapeType")) {
                m.insert("shapeType".into(), st.clone());
            }
            convert_text(&mut m, nc, lk);
        }
        "STAMP" => {
            copy(&mut m, nc, "stampType", "stampType");
        }
        "CONNECTOR" => {
            let conv_ep = |ep: &Value| -> Value {
                let mut o = ep.as_object().cloned().unwrap_or_default();
                // Figma's field is `endpointNodeID`; files Studio wrote before that carry `endpointNodeId`.
                if let Some(id_val) = ep.get("endpointNodeID").or_else(|| ep.get("endpointNodeId")) {
                    if let Some(gs) = guid_str(id_val) {
                        o.insert("endpointNodeId".into(), Value::String(gs));
                    }
                }
                Value::Object(o)
            };
            if let Some(s) = nc.get("connectorStart") {
                m.insert("connectorStart".into(), conv_ep(s));
            }
            if let Some(e) = nc.get("connectorEnd") {
                m.insert("connectorEnd".into(), conv_ep(e));
            }
            if let Some(lt) = fstr(nc, "connectorLineType").or_else(|| fstr(nc, "connectorLineStyle")).or_else(|| fstr(nc, "connectorType")) {
                m.insert("connectorLineType".into(), lt.into());
            }
            if let Some(sc) = nc.get("connectorStartStrokeCap").or_else(|| nc.get("connectorStartCap")) {
                m.insert("connectorStartStrokeCap".into(), sc.clone());
            }
            if let Some(ec) = nc.get("connectorEndStrokeCap").or_else(|| nc.get("connectorEndCap")) {
                m.insert("connectorEndStrokeCap".into(), ec.clone());
            }
            if nc.get("textData").is_some() {
                convert_text(&mut m, nc, lk);
            }
        }
        _ => {}
    }
    // Vector networks: vertices, segments and regions of editable vectors.
    if ty == "VECTOR" {
        if let Some(net) = convert_vector_network(nc, blobs) {
            m.insert("vectorNetwork".into(), net);
        }
    }
    // Precomputed outlines (text and lowered board items keep none: the renderer lays them out itself).
    // For large files (> 5,000 nodes), skip precomputed outlines on rectangular containers and rectangles
    // so they render analytically and take the GPU unit-quad instancing fast path.
    let has_own_outline = matches!(ty, "TEXT" | "STICKY" | "SHAPE_WITH_TEXT" | "CONNECTOR" | "STAMP");
    let analytic_fast_path = lk.large_file && matches!(ty, "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE" | "SECTION" | "RECTANGLE");
    if !(has_own_outline || analytic_fast_path) {
        for k in ["fillGeometry", "strokeGeometry"] {
            if let Some(g) = lk.convert_geometry_cached(nc, k) {
                m.insert(k.into(), g);
            }
        }
    }
    Some(m)
}

/// `boundVariables` (REST shape) from `variableConsumptionMap` and
/// `parameterConsumptionMap` entries (`{variableField, variableData: {dataType: ALIAS, value: {alias}}}`)
/// and the `colorVar` of fill, stroke and effect colors.
fn bound_variables(nc: &Value, lk: &Lk) -> Option<Value> {
    fn alias_target(data: &Value) -> Option<&Value> {
        let v = data.get("value")?;
        if let Some(a) = v.get("alias") {
            return Some(a);
        }
        let fs = v.get("fontStyleValue")?;
        fs.get("asFloat").and_then(|inner| inner.get("value")?.get("alias")).or_else(|| fs.get("asString").and_then(|inner| inner.get("value")?.get("alias")))
    }
    let alias = |data: &Value| -> Option<Value> {
        let a = alias_target(data)?;
        Some(json!({"type": "VARIABLE_ALIAS", "id": lk.variable_id(a)?}))
    };
    let mut out = Map::new();
    let var_entries = nc.get("variableConsumptionMap").map(|m| rest_import::farr(m, "entries")).unwrap_or(&[]);
    let param_entries = nc.get("parameterConsumptionMap").map(|m| rest_import::farr(m, "entries")).unwrap_or(&[]);
    for e in var_entries.iter().chain(param_entries.iter()) {
        let (Some(field), Some(a)) = (fstr(e, "variableField"), e.get("variableData").and_then(alias)) else { continue };
        let keys: &[&str] = match field {
            "STACK_PADDING_LEFT" => &["paddingLeft"],
            "STACK_PADDING_RIGHT" => &["paddingRight"],
            "STACK_PADDING_TOP" => &["paddingTop"],
            "STACK_PADDING_BOTTOM" => &["paddingBottom"],
            "STACK_HORIZONTAL_PADDING" => &["paddingLeft", "paddingRight"],
            "STACK_VERTICAL_PADDING" => &["paddingTop", "paddingBottom"],
            "STACK_PADDING" => &["paddingLeft", "paddingRight", "paddingTop", "paddingBottom"],
            "STACK_SPACING" | "GRID_COLUMN_GAP" => &["itemSpacing"],
            "STACK_COUNTER_SPACING" | "GRID_ROW_GAP" => &["counterAxisSpacing"],
            "RECTANGLE_TOP_LEFT_CORNER_RADIUS" => &["topLeftRadius"],
            "RECTANGLE_TOP_RIGHT_CORNER_RADIUS" => &["topRightRadius"],
            "RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS" => &["bottomLeftRadius"],
            "RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS" => &["bottomRightRadius"],
            "CORNER_RADIUS" => &["topLeftRadius", "topRightRadius", "bottomLeftRadius", "bottomRightRadius"],
            "WIDTH" => &["width"],
            "HEIGHT" => &["height"],
            "MIN_WIDTH" => &["minWidth"],
            "MAX_WIDTH" => &["maxWidth"],
            "MIN_HEIGHT" => &["minHeight"],
            "MAX_HEIGHT" => &["maxHeight"],
            "OPACITY" => &["opacity"],
            "VISIBLE" => &["visible"],
            "TEXT_DATA" => &["characters"],
            "OVERRIDDEN_SYMBOL_ID" => &["component"],
            "STROKE_WEIGHT" => &["strokeWeight"],
            "BORDER_TOP_WEIGHT" => &["strokeTopWeight"],
            "BORDER_RIGHT_WEIGHT" => &["strokeRightWeight"],
            "BORDER_BOTTOM_WEIGHT" => &["strokeBottomWeight"],
            "BORDER_LEFT_WEIGHT" => &["strokeLeftWeight"],
            "FONT_SIZE" => &["fontSize"],
            "FONT_FAMILY" => &["fontFamily"],
            "FONT_WEIGHT" | "FONT_STYLE" => &["fontWeight"],
            "LINE_HEIGHT" => &["lineHeight"],
            "LETTER_SPACING" => &["letterSpacing"],
            "PARAGRAPH_SPACING" => &["paragraphSpacing"],
            "PARAGRAPH_INDENT" => &["paragraphIndent"],
            _ => &[],
        };
        for k in keys {
            out.insert((*k).into(), a.clone());
        }
    }
    for (src, dst, opacity_dst) in [("fillPaints", "fills", "fillOpacities"), ("strokePaints", "strokes", "strokeOpacities")] {
        let list: Vec<Value> = rest_import::farr(nc, src).iter().map(|p| p.get("colorVar").and_then(alias).unwrap_or(Value::Null)).collect();
        if list.iter().any(|v| !v.is_null()) {
            out.insert(dst.into(), Value::Array(list));
        }
        // `Paint.opacityVar`: REST has no field for it, `variables::parse_node_variables` reads this key.
        let opacities: Vec<Value> = rest_import::farr(nc, src).iter().map(|p| p.get("opacityVar").and_then(alias).unwrap_or(Value::Null)).collect();
        if opacities.iter().any(|v| !v.is_null()) {
            out.insert(opacity_dst.into(), Value::Array(opacities));
        }
    }
    let fx: Vec<Value> = rest_import::farr(nc, "effects")
        .iter()
        .map(|e| {
            let mut bound = Map::new();
            if let Some(a) = e.get("colorVar").and_then(alias) {
                bound.insert("color".into(), a);
            }
            // `Effect.effectOpacityVar`
            if let Some(a) = e.get("effectOpacityVar").and_then(alias) {
                bound.insert("opacity".into(), a);
            }
            if bound.is_empty() {
                Value::Null
            } else {
                Value::Object(bound)
            }
        })
        .collect();
    if fx.iter().any(|v| !v.is_null()) {
        out.insert("effects".into(), Value::Array(fx));
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

/// `explicitVariableModes` (collection id -> mode id) from `variableModeBySetMap`.
fn explicit_modes(nc: &Value, lk: &Lk) -> Option<Value> {
    let entries = nc.get("variableModeBySetMap").map(|m| rest_import::farr(m, "entries")).unwrap_or(&[]);
    let out: Map<String, Value> =
        entries.iter().filter_map(|e| Some((lk.set_id(e.get("variableSetID")?)?, Value::String(guid_str(e.get("variableModeID")?)?)))).collect();
    (!out.is_empty()).then_some(Value::Object(out))
}

/// Component properties in REST shape: `componentPropertyDefinitions` (from
/// `componentPropDefs`), `componentProperties` (instance
/// `componentPropAssignments`) and `componentPropertyReferences` (node
/// `componentPropRefs`). Variants refer to their set's definitions through
/// `parentPropDefId`; see [`Lk::prop_defs`].
fn component_props(m: &mut Map<String, Value>, nc: &Value, lk: &Lk) {
    // `textValue` is a message with `characters` in `value`, a plain string in `varValue.value`.
    let text_of = |v: &Value| -> Option<Value> {
        let t = v.get("textValue")?;
        Some(Value::String(t.as_str().or_else(|| fstr(t, "characters"))?.to_string()))
    };
    let value_of = |v: &Value| -> Option<Value> {
        v.get("boolValue")
            .cloned()
            .or_else(|| text_of(v))
            .or_else(|| v.get("guidValue").and_then(guid_str).map(Value::String))
            .or_else(|| v.get("symbolIdValue").and_then(|s| s.get("guid")).and_then(guid_str).map(Value::String))
    };
    // Newer files leave `holder.<plain>` empty (`{}`) and keep the payload in `holder.varValue.value`.
    let held = |holder: &Value, plain: &str| -> Option<Value> {
        holder.get(plain).and_then(value_of).or_else(|| holder.get("varValue").and_then(|v| v.get("value")).and_then(value_of))
    };
    let slot_content_id_of = |holder: &Value, plain: &str| -> Option<String> {
        holder
            .get("varValue")
            .and_then(|v| v.get("value"))
            .or_else(|| holder.get(plain))
            .and_then(|v| v.get("slotContentIdValue"))
            .and_then(|s| s.get("guid").or(Some(s)))
            .and_then(guid_str)
    };
    let mut defs = Map::new();
    for d in rest_import::farr(nc, "componentPropDefs") {
        let (Some(id), Some(_)) = (d.get("id").and_then(guid_str), fstr(d, "name")) else { continue };
        let Some((key, ty)) = lk.prop_defs.get(&id) else { continue };
        let mut def = Map::new();
        def.insert("type".into(), (*ty).into());
        if let Some(v) = held(d, "initialValue") {
            def.insert("defaultValue".into(), v);
        }
        if *ty == "VARIANT" {
            let options: Vec<Value> = rest_import::farr(nc, "stateGroupPropertyValueOrders")
                .iter()
                .find(|o| fstr(o, "property") == fstr(d, "name"))
                .map(|o| rest_import::farr(o, "values").to_vec())
                .unwrap_or_default();
            def.insert("variantOptions".into(), Value::Array(options));
        } else if *ty == "INSTANCE_SWAP" || *ty == "SLOT" {
            let mut prefs: Vec<Value> = Vec::new();
            if let Some(pv) = d.get("preferredValues") {
                if pv.is_object() {
                    for entry in rest_import::farr(pv, "instanceSwapValues") {
                        if let Some(k) = fstr(entry, "key") {
                            let resolved = lk.comp_by_key.get(k).cloned().unwrap_or_else(|| k.to_string());
                            prefs.push(Value::String(resolved));
                        }
                    }
                    for s in rest_import::farr(pv, "stringValues") {
                        if let Some(k) = s.as_str() {
                            let resolved = lk.comp_by_key.get(k).cloned().unwrap_or_else(|| k.to_string());
                            prefs.push(Value::String(resolved));
                        }
                    }
                } else {
                    for p in rest_import::farr(d, "preferredValues") {
                        if let Some(id_or_key) = guid_str(p)
                            .or_else(|| p.as_str().map(String::from))
                            .or_else(|| fstr(p, "key").map(|k| lk.comp_by_key.get(k).cloned().unwrap_or_else(|| k.to_string())))
                        {
                            prefs.push(Value::String(id_or_key));
                        }
                    }
                }
            }
            if !prefs.is_empty() {
                def.insert("preferredValues".into(), Value::Array(prefs));
            }
            // Only the keys the file wrote are carried; an empty object still marks "settings present".
            if let Some(cfg) = d.get("slotPropConfig").and_then(Value::as_object).filter(|_| *ty == "SLOT") {
                def.insert("slotPropConfig".into(), Value::Object(cfg.clone()));
            }
            if *ty == "SLOT" {
                if let Some(scid) = slot_content_id_of(d, "initialValue") {
                    def.insert("slotContentId".into(), Value::String(scid));
                }
            }
        }
        defs.insert(key.clone(), Value::Object(def));
    }
    if !defs.is_empty() {
        m.insert("componentPropertyDefinitions".into(), Value::Object(defs));
    }
    let mut vals = Map::new();
    for a in rest_import::farr(nc, "componentPropAssignments") {
        let Some((key, ty)) = a.get("defID").and_then(guid_str).and_then(|id| lk.prop_defs.get(&id)) else { continue };
        let scid = (*ty == "SLOT").then(|| slot_content_id_of(a, "value")).flatten();
        let v = held(a, "value");
        if v.is_none() && scid.is_none() {
            continue;
        }
        let mut entry = Map::new();
        entry.insert("type".into(), (*ty).into());
        if let Some(v) = v {
            entry.insert("value".into(), v);
        }
        if let Some(scid) = scid {
            entry.insert("slotContentId".into(), Value::String(scid));
        }
        vals.insert(key.clone(), Value::Object(entry));
    }
    if !vals.is_empty() {
        m.insert("componentProperties".into(), Value::Object(vals));
    }
    let mut refs = Map::new();
    for r in rest_import::farr(nc, "componentPropRefs") {
        let Some((key, _)) = lk.prop_def(r) else { continue };
        let field = match fstr(r, "componentPropNodeField") {
            Some("VISIBLE") => "visible",
            Some("TEXT_DATA") => "characters",
            Some("OVERRIDDEN_SYMBOL_ID") => "mainComponent",
            Some("SLOT_CONTENT_ID") => "slotContent",
            _ => continue,
        };
        refs.insert(field.into(), key.clone().into());
    }
    if !refs.is_empty() {
        m.insert("componentPropertyReferences".into(), Value::Object(refs));
    }
    let mut vspecs = Map::new();
    for s in rest_import::farr(nc, "variantPropSpecs") {
        let Some(val) = fstr(s, "value") else { continue };
        let prop_name = fstr(s, "propertyName").or_else(|| fstr(s, "name")).map(String::from).or_else(|| {
            let gid = s.get("propDefId").or_else(|| s.get("defID")).and_then(guid_str)?;
            lk.prop_defs.get(&gid).map(|(k, _)| k.split('#').next().unwrap_or(k).to_string()).or_else(|| Some(format!("propDefId:{gid}")))
        });
        if let Some(name) = prop_name.filter(|n| !n.is_empty()) {
            vspecs.insert(name, Value::String(val.to_string()));
        }
    }
    if !vspecs.is_empty() {
        m.insert("variantPropSpecs".into(), Value::Object(vspecs));
    }
}

/// REST-shaped `vectorNetwork` from the `vectorData` blob of a `.fig` vector.
/// The blob is stored in the normalised size; vertices and tangents are
/// scaled to the node's size.
fn convert_vector_network(nc: &Value, blobs: &[&[u8]]) -> Option<Value> {
    let vd = nc.get("vectorData")?;
    let blob = blobs.get(vd.get("vectorNetworkBlob")?.as_u64()? as usize)?;
    let size = |v: Option<&Value>| v.and_then(|s| Some((fnum(s, "x")?, fnum(s, "y")?)));
    let scale = match (size(nc.get("size")), size(vd.get("normalizedSize"))) {
        (Some((w, h)), Some((nw, nh))) if nw > 0.0 && nh > 0.0 => (w / nw, h / nh),
        _ => (1.0, 1.0),
    };
    let (mut net, styles) = crate::vecnet::network_from_blob(blob, scale)?;
    if let Some(table) = vd.get("styleOverrideTable").and_then(Value::as_array) {
        crate::vecnet::apply_style_table(&mut net, &styles, table);
    }
    Some(crate::vecnet::network_to_json(&net))
}

fn matrix_json(t: &Value) -> Value {
    let g = |k: &str, d: f64| t.get(k).and_then(Value::as_f64).unwrap_or(d);
    json!([[g("m00", 1.0), g("m01", 0.0), g("m02", 0.0)], [g("m10", 0.0), g("m11", 1.0), g("m12", 0.0)]])
}

/// REST `layoutGrids` from kiwi grids: `pattern` (GRID/COLUMNS/ROWS; else `type`,
/// else `axis` X = columns / Y = rows), `sectionSize`, `gutterSize`, `offset`,
/// `count` (or `numSections`), `color`, `visible`.
fn convert_grids(nc: &Value) -> Option<Value> {
    let grids: Vec<Value> = rest_import::farr(nc, "layoutGrids")
        .iter()
        .filter_map(|g| {
            let is_pat = |s: &&str| matches!(*s, "GRID" | "COLUMNS" | "ROWS");
            let pattern = fstr(g, "pattern").filter(is_pat).or_else(|| fstr(g, "type").filter(is_pat)).or(match fstr(g, "axis") {
                Some("X") => Some("COLUMNS"),
                Some("Y") => Some("ROWS"),
                _ => None,
            })?;
            let mut m = Map::new();
            m.insert("pattern".into(), pattern.into());
            copy(&mut m, g, "sectionSize", "sectionSize");
            copy(&mut m, g, "gutterSize", "gutterSize");
            copy(&mut m, g, "offset", "offset");
            copy(&mut m, g, "color", "color");
            copy(&mut m, g, "visible", "visible");
            if let Some(a) = fstr(g, "alignment").or_else(|| fstr(g, "type").filter(|s| matches!(*s, "MIN" | "CENTER" | "STRETCH" | "MAX"))) {
                m.insert("alignment".into(), a.into());
            }
            if let Some(c) = g.get("numSections").or_else(|| g.get("count")) {
                m.insert("count".into(), c.clone());
            }
            Some(Value::Object(m))
        })
        .collect();
    (!grids.is_empty()).then_some(Value::Array(grids))
}

/// `guidPath` as `"s:l"` strings (accepts `{guids: [...]}` or a bare array).
fn guid_path(o: &Value) -> Vec<String> {
    let p = o.get("guidPath");
    let list = p.and_then(|p| p.get("guids")).or(p).and_then(Value::as_array);
    list.map(|a| a.iter().filter_map(guid_str).collect()).unwrap_or_default()
}

fn stack_align(s: Option<&str>) -> &'static str {
    match s {
        Some("CENTER") => "CENTER",
        Some("MAX") => "MAX",
        Some("SPACE_BETWEEN") | Some("SPACE_EVENLY") => "SPACE_BETWEEN",
        Some("BASELINE") => "BASELINE",
        _ => "MIN",
    }
}

/// Primary-axis `StackJustify`. The legacy `SPACE_EVENLY` (3) is Figma's space-between (real files
/// carry it and never `SPACE_BETWEEN`; Figma lints it when `stackLayoutVersion < 1`), so it maps to
/// space-between; only the CSS-aligned `SPACE_EVENLY_CSS` (6) is space-evenly.
fn stack_justify(s: Option<&str>) -> &'static str {
    match s {
        Some("SPACE_AROUND") => "SPACE_AROUND",
        Some("SPACE_EVENLY_CSS") => "SPACE_EVENLY",
        other => stack_align(other),
    }
}

/// Wrap-line distribution and layout engine revision, which only matter once a stack is converted.
fn convert_stack_extras(m: &mut Map<String, Value>, nc: &Value) {
    if fstr(nc, "stackCounterAlignContent") == Some("SPACE_BETWEEN") {
        m.insert("counterAxisAlignContent".into(), "SPACE_BETWEEN".into());
    }
    if let Some(n) = fnum(nc, "stackLayoutVersion").filter(|n| *n > 0.0) {
        m.insert("stackLayoutVersion".into(), n.into());
    }
}

fn stack_sizing(s: Option<&str>) -> &'static str {
    if s.map(|s| s.starts_with("RESIZE_TO_FIT") || s == "AUTO" || s == "HUG").unwrap_or(false) {
        "AUTO"
    } else {
        "FIXED"
    }
}

/// Partial REST node fields from an override / derived-data entry: only keys
/// present in the entry are emitted.
fn convert_override(o: &Value, lk: &Lk) -> Map<String, Value> {
    let mut m = Map::new();
    let Some(obj) = o.as_object() else { return m };
    let mut has_borders = false;
    let mut has_corners = false;
    let mut has_vars = false;
    let mut has_comp_props = false;
    let mut has_layout = false;
    let mut has_text = false;
    for (k, val) in obj {
        match k.as_str() {
            "name" | "visible" | "locked" | "opacity" | "blendMode" | "size" | "strokeWeight" | "strokeAlign" | "cornerRadius" => {
                m.insert(k.clone(), val.clone());
            }
            "mask" => {
                m.insert("isMask".into(), val.clone());
            }
            "overrideLevel" => {
                if let Some(lvl) = val.as_i64() {
                    m.insert("overrideLevel".into(), json!(lvl));
                }
            }
            "detachedSymbolId" => {
                if let Some(ds) = symbol_id_str(val) {
                    m.insert("detachedSymbolId".into(), Value::String(ds));
                }
            }
            "frameMaskDisabled" => {
                if let Some(d) = val.as_bool() {
                    m.insert("clipsContent".into(), (!d).into());
                }
            }
            "transform" => {
                m.insert("relativeTransform".into(), matrix_json(val));
            }
            "effects" => {
                m.insert("effects".into(), Value::Array(val.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().filter_map(convert_effect).collect()));
                has_vars = true;
            }
            "fillGeometry" | "strokeGeometry" => {
                if let Some(g) = lk.convert_geometry_cached(o, k) {
                    m.insert(k.clone(), g);
                }
            }
            "overriddenSymbolID" => {
                if let Some(cid) = guid_str(val) {
                    m.insert("componentId".into(), cid.into());
                }
            }
            "symbolData" => {
                if !m.contains_key("componentId") {
                    if let Some(cid) = val.get("symbolID").and_then(guid_str) {
                        m.insert("componentId".into(), cid.into());
                    }
                }
            }
            "fillPaints" | "strokePaints" => {
                has_vars = true;
            }
            "variableConsumptionMap" | "parameterConsumptionMap" | "variableModeBySetMap" => {
                has_vars = true;
            }
            "borderStrokeWeightsIndependent" | "borderTopWeight" | "borderRightWeight" | "borderBottomWeight" | "borderLeftWeight" => {
                has_borders = true;
            }
            "rectangleCornerRadiiIndependent"
            | "rectangleTopLeftCornerRadius"
            | "rectangleTopRightCornerRadius"
            | "rectangleBottomRightCornerRadius"
            | "rectangleBottomLeftCornerRadius" => {
                has_corners = true;
            }
            "componentPropDefs" | "componentPropAssignments" | "componentPropRefs" | "variantPropSpecs" => {
                has_comp_props = true;
            }
            "stackMode"
            | "gridColumnGap"
            | "gridColumnsGap"
            | "stackSpacing"
            | "gridRowGap"
            | "gridRowsGap"
            | "stackCounterSpacing"
            | "stackPadding"
            | "stackHorizontalPadding"
            | "stackVerticalPadding"
            | "stackPaddingLeft"
            | "stackPaddingTop"
            | "stackPaddingRight"
            | "stackPaddingBottom"
            | "stackPrimaryAlignItems"
            | "stackCounterAlignItems"
            | "stackCounterAlignContent"
            | "stackLayoutVersion"
            | "stackPrimarySizing"
            | "stackCounterSizing"
            | "stackWrap"
            | "bordersTakeSpace"
            | "stackReverseZIndex"
            | "stackChildPrimaryGrow"
            | "stackChildAlignSelf"
            | "stackPositioning"
            | "stackChildMarginTop"
            | "stackChildMarginRight"
            | "stackChildMarginBottom"
            | "stackChildMarginLeft"
            | "layoutSizingHorizontal"
            | "layoutSizingVertical"
            | "gridColumns"
            | "gridRows"
            | "gridColumnsSizing"
            | "gridRowsSizing"
            | "gridAutoTracks"
            | "gridColumnSpan"
            | "gridRowSpan"
            | "gridColumnAnchor"
            | "gridColumnAnchorIndex"
            | "gridRowAnchor"
            | "gridRowAnchorIndex"
            | "gridChildHorizontalAlign"
            | "gridChildVerticalAlign" => {
                has_layout = true;
            }
            "fontName"
            | "fontSize"
            | "textAlignHorizontal"
            | "textAlignVertical"
            | "textCase"
            | "textDecoration"
            | "paragraphSpacing"
            | "paragraphIndent"
            | "listSpacing"
            | "hangingPunctuation"
            | "hangingList"
            | "leadingTrim"
            | "autoRename"
            | "textDecorationSkipInk"
            | "toggledOnOTFeatures"
            | "toggledOffOTFeatures"
            | "numericFigure"
            | "numericSpacing"
            | "numericFraction"
            | "fontVariantCaps"
            | "fontVariantPosition"
            | "fontVariations"
            | "letterSpacing"
            | "textTracking"
            | "lineHeight"
            | "textAutoResize"
            | "textTruncation"
            | "maxLines"
            | "styleIdForText"
            | "textData" => {
                has_text = true;
            }
            _ => {}
        }
    }
    for (src, dst, style) in [("fillPaints", "fills", "styleIdForFill"), ("strokePaints", "strokes", "styleIdForStrokeFill")] {
        if obj.contains_key(src) {
            m.insert(dst.into(), Value::Array(rest_import::farr(o, src).iter().filter_map(convert_paint).collect()));
        } else if let Some(sn) = obj.get(style).and_then(|r| lk.style_node(r)) {
            // An override that only names a library style: the style supplies the paints.
            m.insert(dst.into(), Value::Array(rest_import::farr(sn, "fillPaints").iter().filter_map(convert_paint).collect()));
        }
    }
    if has_borders {
        let mut iw = Map::new();
        iw.insert("fromFig".into(), Value::Bool(true));
        if let Some(indep) = obj.get("borderStrokeWeightsIndependent").and_then(Value::as_bool) {
            iw.insert("independent".into(), Value::Bool(indep));
        }
        for (src, dst) in [("borderTopWeight", "top"), ("borderRightWeight", "right"), ("borderBottomWeight", "bottom"), ("borderLeftWeight", "left")] {
            if let Some(v) = obj.get(src) {
                iw.insert(dst.into(), v.clone());
            }
        }
        m.insert("individualStrokeWeights".into(), Value::Object(iw));
    }
    if has_corners {
        let base = fnum(o, "cornerRadius").unwrap_or(0.0);
        let g = |k: &str| fnum(o, k).unwrap_or(base);
        m.insert(
            "rectangleCornerRadii".into(),
            json!([
                g("rectangleTopLeftCornerRadius"),
                g("rectangleTopRightCornerRadius"),
                g("rectangleBottomRightCornerRadius"),
                g("rectangleBottomLeftCornerRadius")
            ]),
        );
    }
    if has_vars {
        if let Some(bv) = bound_variables(o, lk) {
            m.insert("boundVariables".into(), bv);
        }
        if let Some(modes) = explicit_modes(o, lk) {
            m.insert("explicitVariableModes".into(), modes);
        }
    }
    if has_comp_props {
        component_props(&mut m, o, lk);
    }
    // Auto-layout overrides: emit only keys explicitly present on the override entry so
    // inherited main-component alignment and sizing are not overwritten with defaults.
    if has_layout {
        if let Some(mode) = match fstr(o, "stackMode") {
            Some("HORIZONTAL") => Some("HORIZONTAL"),
            Some("VERTICAL") => Some("VERTICAL"),
            Some("GRID") => Some("GRID"),
            Some("NONE") => Some("NONE"),
            _ => None,
        } {
            m.insert("layoutMode".into(), mode.into());
        }
        convert_stack_extras(&mut m, o);
        if let Some(x) = fnum(o, "gridColumnGap").or_else(|| fnum(o, "gridColumnsGap")).or_else(|| fnum(o, "stackSpacing")) {
            m.insert("itemSpacing".into(), x.into());
        }
        if let Some(x) = fnum(o, "gridRowGap").or_else(|| fnum(o, "gridRowsGap")).or_else(|| fnum(o, "stackCounterSpacing")) {
            m.insert("counterAxisSpacing".into(), x.into());
        }
        let p = fnum(o, "stackPadding");
        let h = fnum(o, "stackHorizontalPadding").or(p);
        let v = fnum(o, "stackVerticalPadding").or(p);
        for (k, val) in [
            ("paddingLeft", fnum(o, "stackPaddingLeft").or(h)),
            ("paddingTop", fnum(o, "stackPaddingTop").or(v)),
            ("paddingRight", fnum(o, "stackPaddingRight").or(h)),
            ("paddingBottom", fnum(o, "stackPaddingBottom").or(v)),
        ] {
            if let Some(x) = val {
                m.insert(k.into(), x.into());
            }
        }
        if let Some(s) = fstr(o, "stackPrimaryAlignItems") {
            m.insert("primaryAxisAlignItems".into(), stack_justify(Some(s)).into());
        }
        if let Some(s) = fstr(o, "stackCounterAlignItems") {
            m.insert("counterAxisAlignItems".into(), stack_align(Some(s)).into());
        }
        if let Some(s) = fstr(o, "stackCounterAlignContent") {
            m.insert("counterAxisAlignContent".into(), (if s == "SPACE_BETWEEN" { "SPACE_BETWEEN" } else { "AUTO" }).into());
        }
        if let Some(n) = fnum(o, "stackLayoutVersion") {
            m.insert("stackLayoutVersion".into(), n.into());
        }
        if let Some(s) = fstr(o, "stackPrimarySizing") {
            m.insert("primaryAxisSizingMode".into(), stack_sizing(Some(s)).into());
        }
        if let Some(s) = fstr(o, "stackCounterSizing") {
            m.insert("counterAxisSizingMode".into(), stack_sizing(Some(s)).into());
        }
        if let Some(w) = fstr(o, "stackWrap") {
            m.insert("layoutWrap".into(), (if w == "WRAP" { "WRAP" } else { "NO_WRAP" }).into());
        } else if let Some(b) = rest_import::fbool(o, "stackWrap") {
            m.insert("layoutWrap".into(), (if b { "WRAP" } else { "NO_WRAP" }).into());
        }
        if let Some(b) = rest_import::fbool(o, "bordersTakeSpace") {
            m.insert("strokesIncludedInLayout".into(), b.into());
        }
        if let Some(b) = rest_import::fbool(o, "stackReverseZIndex") {
            m.insert("itemReverseZIndex".into(), b.into());
        }
        if let Some(g) = fnum(o, "stackChildPrimaryGrow") {
            m.insert("layoutGrow".into(), g.into());
        }
        if let Some(a) = fstr(o, "stackChildAlignSelf") {
            m.insert("layoutAlign".into(), a.into());
        }
        if let Some(pos) = fstr(o, "stackPositioning") {
            m.insert("layoutPositioning".into(), pos.into());
        }
        for k in ["stackChildMarginTop", "stackChildMarginRight", "stackChildMarginBottom", "stackChildMarginLeft"] {
            if let Some(v) = fnum(o, k) {
                m.insert(k.into(), v.into());
            }
        }
        copy(&mut m, o, "layoutSizingHorizontal", "layoutSizingHorizontal");
        copy(&mut m, o, "layoutSizingVertical", "layoutSizingVertical");
        convert_grid_track_fields(&mut m, o);
        for k in ["gridColumnSpan", "gridRowSpan", "gridChildHorizontalAlign", "gridChildVerticalAlign"] {
            copy(&mut m, o, k, k);
        }
        if let Some(c) = obj.get("gridColumnAnchor").or_else(|| obj.get("gridColumnAnchorIndex")) {
            m.insert("gridColumnAnchorIndex".into(), c.clone());
        }
        if let Some(r) = obj.get("gridRowAnchor").or_else(|| obj.get("gridRowAnchorIndex")) {
            m.insert("gridRowAnchorIndex".into(), r.clone());
        }
    }
    if has_text {
        let mut st = Map::new();
        if let Some(shared) = obj.get("styleIdForText").and_then(|r| lk.style_node(r)) {
            let mut linked = convert_type_style(shared, None);
            for (key, default) in [("textCase", "ORIGINAL"), ("textDecoration", "NONE")] {
                linked.entry(key.to_string()).or_insert_with(|| default.into());
            }
            for key in ["fills", "textAlignHorizontal", "textAlignVertical"] {
                linked.remove(key);
            }
            st.extend(linked);
        }
        let base_sz = fnum(o, "fontSize").or_else(|| st.get("fontSize").and_then(Value::as_f64).map(|n| n as f32));
        let mut inline_st = convert_type_style(o, base_sz);
        inline_st.remove("fills");
        st.extend(inline_st);
        if let Some(ls) = obj.get("letterSpacing") {
            if fstr(ls, "units") == Some("PERCENT") && base_sz.is_none() {
                // Resolved against the target node's own font size when applied.
                st.remove("letterSpacing");
                st.insert("letterSpacingPercent".into(), kiwi::clean_f32(fnum(ls, "value").unwrap_or(0.0)));
            }
        }
        for k in ["textAutoResize", "textTruncation", "maxLines"] {
            copy(&mut st, o, k, k);
        }
        if !st.is_empty() {
            m.insert("style".into(), Value::Object(st));
        }
        if let Some(td) = obj.get("textData") {
            if let Some(c) = fstr(td, "characters") {
                m.insert("characters".into(), c.into());
            }
            if let Some(lines) = td.get("lines").and_then(Value::as_array) {
                let mut st = m.get("style").and_then(Value::as_object).cloned().unwrap_or_default();
                crate::textfmt::lines_from_kiwi(lines, &mut m, &mut st);
                if !st.is_empty() {
                    m.insert("style".into(), Value::Object(st));
                }
            }
            text_overrides(&mut m, td, base_sz, lk);
        }
    }
    m
}

fn convert_derived_override(o: &Value, lk: &Lk) -> Map<String, Value> {
    let mut m = Map::new();
    copy(&mut m, o, "size", "size");
    if let Some(t) = o.get("transform") {
        m.insert("relativeTransform".into(), matrix_json(t));
    }
    for k in ["fillGeometry", "strokeGeometry"] {
        if let Some(g) = lk.convert_geometry_cached(o, k) {
            m.insert(k.into(), g);
        }
    }
    m
}

/// Track ids of the grid maps an override entry restates, by the entry's path: (columns, rows).
type RestatedTracks = HashMap<Vec<String>, (Vec<String>, Vec<String>)>;

/// Turns a cell anchor that an override names by track id into the 0-based track index. The tracks
/// the instance restates for the parent grid (`restated`) win over the file-wide map, since an
/// instance that restates the tracks can shift them. An anchor that matches no track, or an id that
/// two grids place differently, is left as it is: a wrong cell is worse than the component's cell.
fn resolve_override_grid_anchor(
    set: &mut Map<String, Value>,
    key: &str,
    guid_key: &str,
    restated: Option<&[String]>,
    file_wide: &HashMap<String, Option<usize>>,
) {
    let Some(id) = set.get(key).and_then(guid_str) else { return };
    let index = match restated.filter(|ids| !ids.is_empty()) {
        Some(ids) => ids.iter().position(|t| *t == id),
        None => file_wide.get(&id).copied().flatten(),
    };
    if let Some(i) = index {
        set.insert(key.into(), json!(i));
        set.insert(guid_key.into(), Value::String(id));
    }
}

/// Override entries of `instance`. Cell anchors in them resolve against the grid they sit in: for a
/// child one step below the instance that is the instance's own tracks, deeper down it is the tracks
/// an entry restates for the parent.
fn override_entries(instance: &Value, list: Option<&Value>, lk: &Lk, derived: bool) -> Vec<Value> {
    let Some(raw) = list.and_then(Value::as_array) else { return Vec::new() };
    let carried: Vec<std::borrow::Cow<'_, Value>> = raw.iter().map(with_field_carriers).collect();
    let a: Vec<&Value> = carried.iter().map(|c| &**c).collect();
    let mut restated = RestatedTracks::new();
    let mut out = Vec::new();
    if !derived {
        let sym_id = instance.get("symbolData").and_then(|sd| sd.get("symbolID")).and_then(guid_str);
        for (path, owner) in std::iter::once((Vec::new(), instance)).chain(a.iter().map(|&o| (guid_path(o), o))) {
            let (cols, rows) = (grid_position_guids(owner, "gridColumns"), grid_position_guids(owner, "gridRows"));
            if !cols.is_empty() || !rows.is_empty() {
                let is_root_sym = path.len() == 1 && sym_id.as_deref() == Some(path[0].as_str());
                let entry = restated.entry(path).or_default();
                if !cols.is_empty() {
                    entry.0 = cols.clone();
                }
                if !rows.is_empty() {
                    entry.1 = rows.clone();
                }
                if is_root_sym {
                    let root_entry = restated.entry(Vec::new()).or_default();
                    if !cols.is_empty() {
                        root_entry.0 = cols;
                    }
                    if !rows.is_empty() {
                        root_entry.1 = rows;
                    }
                }
            }
        }
        let mut inst_grid = Map::new();
        convert_grid_track_fields(&mut inst_grid, instance);
        if !inst_grid.is_empty() {
            out.push(json!({"path": Vec::<String>::new(), "set": Value::Object(inst_grid)}));
        }
    }
    out.extend(a.iter().filter_map(|o| {
        let mut set = if derived { convert_derived_override(o, lk) } else { convert_override(o, lk) };
        if set.is_empty() {
            return None;
        }
        let path = guid_path(o);
        if !derived {
            let parent = path.split_last().and_then(|(_, up)| restated.get(up));
            resolve_override_grid_anchor(&mut set, "gridColumnAnchorIndex", "_gridColumnAnchorGuid", parent.map(|p| p.0.as_slice()), &lk.grid_column_index);
            resolve_override_grid_anchor(&mut set, "gridRowAnchorIndex", "_gridRowAnchorGuid", parent.map(|p| p.1.as_slice()), &lk.grid_row_index);
        }
        Some(json!({"path": path, "set": Value::Object(set)}))
    }));
    out
}

fn convert_stack(m: &mut Map<String, Value>, nc: &Value) {
    let mode = match fstr(nc, "stackMode") {
        Some("HORIZONTAL") => "HORIZONTAL",
        Some("VERTICAL") => "VERTICAL",
        Some("GRID") => "GRID",
        Some("NONE") => {
            m.insert("layoutMode".into(), "NONE".into());
            return;
        }
        _ => return,
    };
    m.insert("layoutMode".into(), mode.into());
    convert_grid_track_fields(m, nc);
    if let Some(x) = fnum(nc, "gridColumnGap").or_else(|| fnum(nc, "gridColumnsGap")).or_else(|| fnum(nc, "stackSpacing")) {
        m.insert("itemSpacing".into(), x.into());
    }
    if let Some(x) = fnum(nc, "gridRowGap").or_else(|| fnum(nc, "gridRowsGap")).or_else(|| fnum(nc, "stackCounterSpacing")) {
        m.insert("counterAxisSpacing".into(), x.into());
    }
    let p = fnum(nc, "stackPadding");
    let h = fnum(nc, "stackHorizontalPadding").or(p);
    let v = fnum(nc, "stackVerticalPadding").or(p);
    let ins = |m: &mut Map<String, Value>, k: &str, x: Option<f32>| {
        if let Some(x) = x {
            m.insert(k.into(), x.into());
        }
    };
    ins(m, "paddingLeft", fnum(nc, "stackPaddingLeft").or(h));
    ins(m, "paddingTop", fnum(nc, "stackPaddingTop").or(v));
    ins(m, "paddingRight", fnum(nc, "stackPaddingRight").or(h));
    ins(m, "paddingBottom", fnum(nc, "stackPaddingBottom").or(v));
    m.insert("primaryAxisAlignItems".into(), stack_justify(fstr(nc, "stackPrimaryAlignItems")).into());
    m.insert("counterAxisAlignItems".into(), stack_align(fstr(nc, "stackCounterAlignItems")).into());
    // Real files only ever write `FIXED` for the primary axis (absent = hug) and
    // `RESIZE_TO_FIT_WITH_IMPLICIT_SIZE` for the counter axis (absent = fixed).
    let primary = if fstr(nc, "stackPrimarySizing") == Some("FIXED") { "FIXED" } else { "AUTO" };
    m.insert("primaryAxisSizingMode".into(), primary.into());
    m.insert("counterAxisSizingMode".into(), stack_sizing(fstr(nc, "stackCounterSizing")).into());
    if fstr(nc, "stackWrap") == Some("WRAP") || rest_import::fbool(nc, "stackWrap") == Some(true) {
        m.insert("layoutWrap".into(), "WRAP".into());
    }
    if let Some(b) = rest_import::fbool(nc, "stackReverseZIndex") {
        m.insert("itemReverseZIndex".into(), b.into());
    }
    if let Some(b) = rest_import::fbool(nc, "bordersTakeSpace") {
        m.insert("strokesIncludedInLayout".into(), b.into());
    }
    convert_stack_extras(m, nc);
}

// ------------------------------------------------------------------ paint / effect

fn convert_paint(p: &Value) -> Option<Value> {
    let ty = fstr(p, "type")?;
    let mut m = Map::new();
    m.insert("type".into(), ty.into());
    copy(&mut m, p, "opacity", "opacity");
    copy(&mut m, p, "visible", "visible");
    copy(&mut m, p, "blendMode", "blendMode");
    match ty {
        "SOLID" => copy(&mut m, p, "color", "color"),
        t if t.starts_with("GRADIENT") => {
            let stops: Vec<Value> = rest_import::farr(p, "stops").iter().map(|s| json!({"position": s.get("position"), "color": s.get("color")})).collect();
            m.insert("gradientStops".into(), Value::Array(stops));
            let t = p.get("transform");
            let g = |k: &str, d: f32| t.and_then(|t| fnum(t, k)).unwrap_or(d);
            let xf = Affine { a: g("m00", 1.0), c: g("m01", 0.0), tx: g("m02", 0.0), b: g("m10", 0.0), d: g("m11", 1.0), ty: g("m12", 0.0) };
            let inv = xf.invert().unwrap_or(Affine::IDENTITY);
            let pts = if ty == "GRADIENT_LINEAR" {
                [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)]
            } else {
                [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)]
            };
            let h = pts.map(|q| {
                let r = inv.apply(q);
                json!({"x": kiwi::clean_f32(r.x), "y": kiwi::clean_f32(r.y)})
            });
            m.insert("gradientHandlePositions".into(), Value::Array(h.to_vec()));
        }
        "IMAGE" => {
            let image = p.get("image");
            let hash = image
                .and_then(|i| fstr(i, "hash"))
                .filter(|h| !h.is_empty())
                .or_else(|| fstr(p, "imageHash"))
                .or_else(|| image.and_then(|i| fstr(i, "name")))?;
            m.insert("imageRef".into(), hash.into());
            enum_map(&mut m, p, "imageScaleMode", "scaleMode", &[]);
        }
        _ => return None,
    }
    Some(Value::Object(m))
}

fn convert_effect(e: &Value) -> Option<Value> {
    let ty = match fstr(e, "type")? {
        "FOREGROUND_BLUR" => "LAYER_BLUR",
        "GRAIN" => "TEXTURE",
        other => other,
    };
    let mut m = Map::new();
    m.insert("type".into(), ty.into());
    for k in
        ["color", "secondaryColor", "offset", "radius", "spread", "visible", "showShadowBehindNode", "blendMode", "density", "opacity", "seed", "clipToShape"]
    {
        copy(&mut m, e, k, k);
    }
    if let Some(nt) = fstr(e, "noiseType") {
        let norm = match nt {
            "MONO" => "MONOTONE",
            "DUO" => "DUOTONE",
            "MULTI" => "MULTITONE",
            other => other,
        };
        m.insert("noiseType".into(), norm.into());
    }
    if let Some(ns) = e.get("noiseSize") {
        if ns.is_number() {
            m.insert("noiseSize".into(), ns.clone());
        } else if let Some(sz) = fnum(ns, "x").or_else(|| fnum(ns, "y")) {
            m.insert("noiseSize".into(), json!(kiwi::clean_f32(sz)));
        }
    }
    for (kiwi_key, rest_key) in [
        ("specularAngle", "lightAngle"),
        ("specularIntensity", "lightIntensity"),
        ("refractionIntensity", "refraction"),
        ("refractionRadius", "depth"),
        ("chromaticAberration", "dispersion"),
    ] {
        if e.get(kiwi_key).is_some() {
            copy(&mut m, e, kiwi_key, rest_key);
        } else {
            copy(&mut m, e, rest_key, rest_key);
        }
    }
    Some(Value::Object(m))
}

// ------------------------------------------------------------------ text

/// "SemiBold Italic" -> (600, true)
pub(crate) fn weight_from_style_name(style: &str) -> (u16, bool) {
    let norm: String = style.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    let italic = norm.contains("italic") || norm.contains("oblique");
    const TABLE: &[(&str, u16)] = &[
        ("extralight", 200),
        ("ultralight", 200),
        ("extrabold", 800),
        ("ultrabold", 800),
        ("semibold", 600),
        ("demibold", 600),
        ("thin", 100),
        ("hairline", 100),
        ("light", 300),
        ("medium", 500),
        ("bold", 700),
        ("black", 900),
        ("heavy", 900),
        ("regular", 400),
        ("normal", 400),
        ("book", 400),
    ];
    let w = TABLE.iter().find(|(k, _)| norm.contains(k)).map(|(_, w)| *w).unwrap_or(400);
    (w, italic)
}

/// Partial REST `TypeStyle` from kiwi text properties (`base_size` resolves
/// percentage letter spacing for override entries without their own size).
fn convert_type_style(o: &Value, base_size: Option<f32>) -> Map<String, Value> {
    let mut m = Map::new();
    if let Some(f) = o.get("fontName") {
        if let Some(fam) = fstr(f, "family") {
            m.insert("fontFamily".into(), fam.into());
        }
        if let Some(st) = fstr(f, "style") {
            let (w, i) = weight_from_style_name(st);
            m.insert("fontWeight".into(), w.into());
            m.insert("italic".into(), i.into());
        }
    }
    let size = fnum(o, "fontSize").or(base_size);
    if let Some(s) = fnum(o, "fontSize") {
        m.insert("fontSize".into(), s.into());
    }
    copy(&mut m, o, "textAlignHorizontal", "textAlignHorizontal");
    copy(&mut m, o, "textAlignVertical", "textAlignVertical");
    copy(&mut m, o, "textCase", "textCase");
    copy(&mut m, o, "textDecoration", "textDecoration");
    copy(&mut m, o, "paragraphSpacing", "paragraphSpacing");
    for k in ["paragraphIndent", "listSpacing", "hangingPunctuation", "hangingList", "leadingTrim", "autoRename", "textDecorationSkipInk"] {
        copy(&mut m, o, k, k);
    }
    if let Some(flags) = crate::textfmt::flags_from_kiwi(o) {
        m.insert("opentypeFlags".into(), flags);
    }
    if let Some(fvs) = o.get("fontVariations").and_then(Value::as_array).filter(|a| !a.is_empty()) {
        m.insert("fontVariations".into(), Value::Array(fvs.clone()));
    }
    if let Some(ls) = o.get("letterSpacing") {
        let v = fnum(ls, "value").unwrap_or(0.0);
        let px = if fstr(ls, "units") == Some("PERCENT") { v / 100.0 * size.unwrap_or(0.0) } else { v };
        m.insert("letterSpacing".into(), kiwi::clean_f32(px));
    } else if let Some(tr) = fnum(o, "textTracking").filter(|t| t.abs() > 1e-5) {
        m.insert("letterSpacing".into(), kiwi::clean_f32(tr));
    }
    if let Some(lh) = o.get("lineHeight") {
        // Kiwi `Number {value, units}`: PIXELS = px; PERCENT = percent of the font
        // size (100 means exactly 1.0 x font size, NOT auto); RAW = multiplier,
        // where RAW 1 is Figma's "auto". A missing value also means auto.
        let intrinsic = |m: &mut Map<String, Value>| {
            m.insert("lineHeightUnit".into(), "INTRINSIC_%".into());
        };
        let percent = |m: &mut Map<String, Value>, p: f32| {
            m.insert("lineHeightUnit".into(), "FONT_SIZE_%".into());
            m.insert("lineHeightPercentFontSize".into(), kiwi::clean_f32(p));
        };
        match (fnum(lh, "value"), fstr(lh, "units")) {
            (None, _) => intrinsic(&mut m),
            (Some(v), Some("PIXELS")) => {
                m.insert("lineHeightUnit".into(), "PIXELS".into());
                m.insert("lineHeightPx".into(), kiwi::clean_f32(v));
            }
            (Some(v), Some("RAW")) if (v - 1.0).abs() < 1e-3 => intrinsic(&mut m),
            (Some(v), Some("RAW")) => percent(&mut m, v * 100.0),
            (Some(v), _) => percent(&mut m, v),
        }
    }
    if o.get("fillPaints").is_some() {
        let fills: Vec<Value> = rest_import::farr(o, "fillPaints").iter().filter_map(convert_paint).collect();
        m.insert("fills".into(), Value::Array(fills));
    }
    m
}

fn convert_text(m: &mut Map<String, Value>, nc: &Value, lk: &Lk) {
    let td = nc.get("textData");
    let chars = td.and_then(|t| fstr(t, "characters")).unwrap_or("");
    m.insert("characters".into(), chars.into());
    let mut style = convert_type_style(nc, None);
    // Linked styles are authoritative: kiwi keeps pre-link properties on the
    // node (e.g. 48px even though the active shared style specifies 18px).
    // Character overrides are applied afterwards, against this resolved base.
    if let Some(shared) = nc.get("styleIdForText").and_then(|r| lk.style_node(r)) {
        let mut linked = convert_type_style(shared, None);
        for (key, default) in [("textCase", "ORIGINAL"), ("textDecoration", "NONE")] {
            linked.entry(key.to_string()).or_insert_with(|| default.into());
        }
        // A text style does not supply the layer's paint or box alignment.
        for key in ["fills", "textAlignHorizontal", "textAlignVertical"] {
            linked.remove(key);
        }
        style.extend(linked);
    }
    style.remove("fills"); // node-level fills are the text color
    copy(&mut style, nc, "textAutoResize", "textAutoResize");
    if let Some(a) = fstr(nc, "textAutoResize") {
        m.insert("textAutoResize".into(), a.into());
    }
    // textTruncation ENDING and maxLines (kiwi fields of the same names).
    copy(m, nc, "textTruncation", "textTruncation");
    copy(m, nc, "maxLines", "maxLines");
    copy(m, nc, "autoRename", "autoRename");
    copy(m, nc, "textDecorationSkipInk", "textDecorationSkipInk");
    copy(m, nc, "textWrapStyle", "textWrapStyle");
    if let Some(lines) = td.and_then(|t| t.get("lines")).and_then(Value::as_array) {
        crate::textfmt::lines_from_kiwi(lines, m, &mut style);
    }
    let base_size = style.get("fontSize").and_then(Value::as_f64).map(|n| n as f32);
    m.insert("style".into(), Value::Object(style));
    if let Some(td) = td {
        text_overrides(m, td, base_size, lk);
    }
}

/// Per-character style ids and the override table from kiwi `textData`.
fn text_overrides(m: &mut Map<String, Value>, td: &Value, base_size: Option<f32>, lk: &Lk) {
    if let Some(ids) = td.get("characterStyleIDs").and_then(Value::as_array) {
        m.insert("characterStyleOverrides".into(), Value::Array(ids.clone()));
        let mut table = Map::new();
        for o in rest_import::farr(td, "styleOverrideTable") {
            if let Some(sid) = o.get("styleID").and_then(Value::as_u64) {
                let mut st = Map::new();
                if let Some(shared) = o.get("styleIdForText").and_then(|r| lk.style_node(r)) {
                    let mut linked = convert_type_style(shared, base_size);
                    for key in ["fills", "textAlignHorizontal", "textAlignVertical"] {
                        linked.remove(key);
                    }
                    st.extend(linked);
                }
                let eff_base = fnum(o, "fontSize").or_else(|| st.get("fontSize").and_then(Value::as_f64).map(|n| n as f32)).or(base_size);
                st.extend(convert_type_style(o, eff_base));
                if !st.contains_key("fills") {
                    if let Some(sn) = o.get("styleIdForFill").and_then(|r| lk.style_node(r)) {
                        let fills: Vec<Value> = rest_import::farr(sn, "fillPaints").iter().filter_map(convert_paint).collect();
                        if !fills.is_empty() {
                            st.insert("fills".into(), Value::Array(fills));
                        }
                    }
                }
                table.insert(sid.to_string(), Value::Object(st));
            }
        }
        m.insert("styleOverrideTable".into(), Value::Object(table));
    }
}

// ------------------------------------------------------------------ blobs

/// Decodes a Figma path commands blob (0=close, 1=move, 2=line, 3=quad, 4=cubic; f32 LE coords).
pub fn commands_blob_to_path(b: &[u8], even_odd: bool) -> Path {
    let mut cmds = Vec::new();
    let mut i = 0usize;
    let f = |i: usize| -> Option<f32> { b.get(i..i + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]])) };
    let pt = |i: usize| -> Option<Vec2> { Some(Vec2::new(f(i)?, f(i + 4)?)) };
    while i < b.len() {
        let op = b[i];
        i += 1;
        let ok = match op {
            0 => {
                cmds.push(PathCmd::Close);
                Some(())
            }
            1 => pt(i).map(|p| {
                cmds.push(PathCmd::MoveTo(p));
                i += 8;
            }),
            2 => pt(i).map(|p| {
                cmds.push(PathCmd::LineTo(p));
                i += 8;
            }),
            3 => pt(i).zip(pt(i + 8)).map(|(c, p)| {
                cmds.push(PathCmd::QuadTo(c, p));
                i += 16;
            }),
            4 => pt(i).zip(pt(i + 8)).zip(pt(i + 16)).map(|((c1, c2), p)| {
                cmds.push(PathCmd::CubicTo(c1, c2, p));
                i += 24;
            }),
            _ => None,
        };
        if ok.is_none() {
            break;
        }
    }
    Path { cmds, fill_rule: if even_odd { FillRule::EvenOdd } else { FillRule::NonZero } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_names() {
        assert_eq!(weight_from_style_name("Regular"), (400, false));
        assert_eq!(weight_from_style_name("Semi Bold Italic"), (600, true));
        assert_eq!(weight_from_style_name("ExtraBold"), (800, false));
        assert_eq!(weight_from_style_name("Bold"), (700, false));
        assert_eq!(weight_from_style_name("Extra-Light"), (200, false));
    }

    #[test]
    fn kiwi_export_settings_and_guides_map_to_rest_shapes() {
        let nc = json!({
            "exportSettings": [
                {"suffix": "@2x", "imageType": "JPEG", "constraint": {"type": "CONTENT_SCALE", "value": 2.0}},
                {"imageType": "SVG", "constraint": {"type": "CONTENT_WIDTH", "value": 512.0}},
                {"imageType": "PDF", "constraint": {"type": "CONTENT_HEIGHT", "value": 64.0}}
            ],
            "guides": [{"axis": "X", "offset": 10.0}, {"axis": "Y", "offset": 4.5}, {"offset": 1.0}]
        });
        let doc = rest_import::parse_export_settings(&json!({"exportSettings": convert_export_settings(&nc).unwrap()}));
        use designos_doc::{ExportConstraint as C, ExportFileFormat as F};
        assert_eq!((doc[0].format, doc[0].constraint, doc[0].suffix.as_str()), (F::Jpg, C::Scale(2.0), "@2x"));
        assert_eq!((doc[1].format, doc[1].constraint), (F::Svg, C::Width(512.0)));
        assert_eq!((doc[2].format, doc[2].constraint), (F::Pdf, C::Height(64.0)));
        assert_eq!(convert_guides(&nc).unwrap(), json!([{"axis": "X", "offset": 10.0}, {"axis": "Y", "offset": 4.5}]));
        assert!(convert_export_settings(&json!({})).is_none() && convert_guides(&json!({})).is_none());
    }

    #[test]
    fn blob_decoding_is_tolerant() {
        let mut b = vec![1u8];
        for v in [1.0f32, 2.0] {
            b.extend(v.to_le_bytes());
        }
        b.push(2);
        b.extend(3.0f32.to_le_bytes()); // truncated lineTo
        let p = commands_blob_to_path(&b, false);
        assert_eq!(p.cmds, vec![PathCmd::MoveTo(Vec2::new(1.0, 2.0))]);
        assert!(commands_blob_to_path(&[9, 9, 9], false).cmds.is_empty());
    }
}
