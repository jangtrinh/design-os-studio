//! `.fig` (fig-kiwi) binary writer: [`Document`] -> `.fig` archive bytes.
//!
//! Produces a standard Figma local-copy ZIP archive (`canvas.fig` + `meta.json` +
//! `images/<hash>`) whose `canvas.fig` payload is a `fig-kiwi` container encoded
//! against [`crate::testkit::figma_like_schema`].

use crate::container::{self, Compression};
use crate::effect_units;
use crate::kiwi::KiwiValue as V;
use crate::rest_import::{grid_child_align_name, utf16_to_byte_offsets, writes_grid_child_align};
use crate::testkit::{self, en, figma_like_schema, guid, msg, st};
use crate::FigError;
use designos_doc::*;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashSet};

mod carrier;
mod grid;
mod kept_derived;
mod kept_overrides;
pub(crate) use carrier::{CARRIED_FIELDS, CARRIER_KEY_PREFIX};

/// Plugin id under which Studio keeps node data that has no field in Figma's NodeChange schema. It travels
/// as ordinary `pluginData` entries, which Figma keeps for any plugin id and ignores when it does not know the
/// plugin. [`crate::fig`] reads these entries back into typed node data and never into `Node::plugin_data`.
pub(crate) const STUDIO_PLUGIN_ID: &str = "designos-studio";
/// Carrier key: an instance's slot content id, as written (`"7:42"`).
pub(crate) const STUDIO_SLOT_CONTENT_ID_KEY: &str = "slot-content-id";
/// Carrier key: `"true"` for a sticky that shows its author; absent means hidden.
pub(crate) const STUDIO_AUTHOR_VISIBLE_KEY: &str = "author-visible";

fn is_studio_carrier_key(full_key: &str) -> bool {
    full_key
        .strip_prefix(STUDIO_PLUGIN_ID)
        .and_then(|rest| rest.strip_prefix(':'))
        .is_some_and(|key| key == STUDIO_SLOT_CONTENT_ID_KEY || key == STUDIO_AUTHOR_VISIBLE_KEY || key.starts_with(carrier::CARRIER_KEY_PREFIX))
}

// The kiwi encoder writes whatever float bits it is given, and a reader treats a NaN or an infinity
// as an absent field, so a single bad number would drop a whole `size` or transform on the way back.
// The exporter therefore never hands the encoder a non-finite float: it writes the neutral value of
// the slot instead, one for opacity, alpha and the diagonal of a matrix, zero for every other number.
// The one exception is the unset side of `maxSize`, which Figma's own files write as infinity (see `size_limit`).
fn finite_or(x: f32, neutral: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        neutral
    }
}

fn fl(x: f32) -> V {
    testkit::fl(finite_or(x, 0.0))
}

/// [`fl`] for a number whose neutral value is one: opacity, alpha, the diagonal of a matrix.
fn fl_one(x: f32) -> V {
    testkit::fl(finite_or(x, 1.0))
}

/// `minSize` / `maxSize`: an `OptionalVector` whose unset side holds `unset`.
fn size_limit(x: Option<f32>, y: Option<f32>, unset: f32) -> V {
    let side = |v: Option<f32>| v.map_or_else(|| testkit::fl(unset), fl);
    msg(vec![("value", st(vec![("x", side(x)), ("y", side(y))]))])
}

fn vec2(x: f32, y: f32) -> V {
    st(vec![("x", fl(x)), ("y", fl(y))])
}

fn color(r: f32, g: f32, b: f32, a: f32) -> V {
    st(vec![("r", fl(r)), ("g", fl(g)), ("b", fl(b)), ("a", fl_one(a))])
}

/// Little-endian bytes of a float inside a binary blob, with the same rule.
fn blob_f32(x: f32) -> [u8; 4] {
    finite_or(x, 0.0).to_le_bytes()
}

/// Encodes a [`Path`] into Figma's binary path-commands blob format
/// (`0` = Close, `1` = MoveTo, `2` = LineTo, `3` = QuadTo, `4` = CubicTo,
/// little-endian `f32` coordinates). Exact inverse of [`crate::commands_blob_to_path`].
pub fn path_to_commands_blob(path: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    let push_pt = |out: &mut Vec<u8>, p: Vec2| {
        out.extend_from_slice(&blob_f32(p.x));
        out.extend_from_slice(&blob_f32(p.y));
    };
    for cmd in &path.cmds {
        match *cmd {
            PathCmd::Close => out.push(0),
            PathCmd::MoveTo(p) => {
                out.push(1);
                push_pt(&mut out, p);
            }
            PathCmd::LineTo(p) => {
                out.push(2);
                push_pt(&mut out, p);
            }
            PathCmd::QuadTo(c, p) => {
                out.push(3);
                push_pt(&mut out, c);
                push_pt(&mut out, p);
            }
            PathCmd::CubicTo(c1, c2, p) => {
                out.push(4);
                push_pt(&mut out, c1);
                push_pt(&mut out, c2);
                push_pt(&mut out, p);
            }
        }
    }
    out
}

fn export_schema() -> &'static crate::kiwi::Schema {
    static SCHEMA: std::sync::OnceLock<crate::kiwi::Schema> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(figma_like_schema)
}

/// What a shader message that cannot be written whole tells the person saving: the layer, what is wrong, what to do.
fn shader_refusal(owner_name: &str, problem: String) -> FigError {
    FigError::Format(format!("layer {owner_name:?}: {problem}; save as .designos to keep it, or remove the shader"))
}

/// Rebuilds the fields of a shader message from its raw JSON, or refuses. The encoder writes only the fields a schema
/// definition names, so a key it does not name, at any depth, would be dropped by a save that still reports success.
fn decode_shader_ref(
    schema: &crate::kiwi::Schema,
    def_name: &str,
    owner_name: &str,
    shader: &ShaderRef,
    skip: &[&str],
) -> Result<Vec<(std::sync::Arc<str>, V)>, FigError> {
    let Some(obj) = shader.raw.as_object() else {
        return Err(shader_refusal(owner_name, format!("shader payload on {def_name} must be a JSON object")));
    };
    let def_idx = schema.find(def_name).ok_or_else(|| FigError::Format(format!("missing {def_name} in schema")))?;
    let def = &schema.defs[def_idx];
    let mut out = Vec::new();
    for (k, v) in obj {
        if skip.contains(&k.as_str()) || v.is_null() {
            continue;
        }
        let Some(sf) = def.fields.iter().find(|f| &*f.name == k.as_str()) else {
            return Err(shader_refusal(owner_name, format!("shader key {k:?} is not a field of {def_name} in the .fig schema")));
        };
        if let Some(path) = schema.first_unknown_json_key(sf, v) {
            return Err(shader_refusal(owner_name, format!("shader key {path:?} is not a field of {def_name} in the .fig schema")));
        }
        let Some(kv) = schema.field_from_json_strict(sf, v, 0) else {
            return Err(shader_refusal(owner_name, format!("invalid shader field {k:?} on {def_name}")));
        };
        if let (Some(arr), V::Array(items)) = (v.as_array(), &kv) {
            if arr.len() != items.len() {
                return Err(shader_refusal(owner_name, format!("invalid shader field {k:?} on {def_name}")));
            }
        }
        out.push((sf.name.clone(), kv));
    }
    Ok(out)
}

fn validate_paint_list(schema: &crate::kiwi::Schema, owner_name: &str, paints: &[Paint]) -> Result<(), FigError> {
    for p in paints {
        if let PaintKind::Shader(shader) = &p.kind {
            decode_shader_ref(schema, "Paint", owner_name, shader, &["type", "opacity", "visible", "blendMode"])?;
        }
    }
    Ok(())
}

fn validate_effect_list(schema: &crate::kiwi::Schema, owner_name: &str, effects: &[Effect]) -> Result<(), FigError> {
    for e in effects {
        if let Effect::Shader { shader, .. } = e {
            decode_shader_ref(schema, "Effect", owner_name, shader, &["type", "visible"])?;
        }
    }
    Ok(())
}

fn validate_shader_refs(doc: &Document, schema: &crate::kiwi::Schema) -> Result<(), FigError> {
    for node in doc.nodes.values() {
        validate_paint_list(schema, &node.name, &node.fills)?;
        validate_paint_list(schema, &node.name, &node.stroke.paints)?;
        if let NodeKind::Text(tp) = &node.kind {
            validate_paint_list(schema, &node.name, &tp.style.fills)?;
            for r in &tp.runs {
                validate_paint_list(schema, &node.name, &r.style.fills)?;
            }
        }
        validate_effect_list(schema, &node.name, &node.effects)?;
    }
    for (sid, style) in &doc.styles {
        let name = doc.style_name(sid);
        match style {
            Style::Fill(paints) => validate_paint_list(schema, name, paints)?,
            Style::Text(ts) => validate_paint_list(schema, name, &ts.fills)?,
            Style::Effect(effects) => validate_effect_list(schema, name, effects)?,
            Style::Grid(_) | Style::Animation(_) => {}
        }
    }
    Ok(())
}

/// Serializes `doc` into a `.fig` binary archive (ZIP containing `canvas.fig`,
/// `meta.json`, and `images/<hash>`).
///
/// `doc` is only read. An instance on a page the document's page loader has not opened yet has no children to read
/// its overrides from, so it is written with the override entries and the derived data the file stated for it (the
/// derived data only while its main component is as it was when the file was opened); no page is opened and the loader
/// is left as it was.
pub fn export_fig(doc: &Document) -> Result<Vec<u8>, FigError> {
    let schema = export_schema();
    validate_shader_refs(doc, schema)?;
    let root = schema.find("Message").ok_or_else(|| FigError::Format("missing Message in schema".into()))?;
    let message = build_message(doc, schema);
    let schema_bytes = schema.encode();
    let msg_bytes = schema.encode_message(root, &message).map_err(|e| FigError::Format(format!("kiwi encode: {e}")))?;
    let canvas = container::write_container(&schema_bytes, &msg_bytes, Compression::Zstd, 106);
    let meta = json!({
        "file_name": doc.name,
        "client_meta": {
            "background_color": {"r": 0.9, "g": 0.9, "b": 0.9, "a": 1.0},
            "thumbnail_size": {"width": 400, "height": 200}
        }
    });
    Ok(container::write_zip(&canvas, &doc.images, Some(&meta)))
}

/// Serializes `doc` into a `.fig` binary archive and returns a sibling report of any variable or
/// collection `extensions` / `mode_extensions` that `.fig` `VARIABLE_SET` / `VARIABLE` nodes do not carry (`E1-9` §4, §10).
pub fn export_fig_with_report(doc: &Document) -> Result<(Vec<u8>, Vec<crate::variables_dtcg::SkippedVariable>), FigError> {
    let bytes = export_fig(doc)?;
    let omitted = crate::variables_dtcg::collect_omitted_extensions(doc);
    Ok((bytes, omitted))
}

// ------------------------------------------------------------------ state & IDs

struct ExportCtx<'a> {
    doc: &'a Document,
    schema: &'a crate::kiwi::Schema,
    nc_def: usize,
    used_guids: HashSet<(u32, u32)>,
    next_local: u32,
    node_guids: BTreeMap<NodeId, (u32, u32)>,
    /// `node_guids` the other way round, built the first time a guid has to be resolved to a node.
    nodes_by_guid: Option<BTreeMap<(u32, u32), NodeId>>,
    /// Fingerprints of the main components whose instances are checked for stale derived data.
    fingerprints: crate::component_fingerprint::Fingerprints,
    /// Outline id (see `PendingOutline`) -> its index in `blobs`, so an outline several entries share is stored once.
    shared_blobs: BTreeMap<u64, u32>,
    coll_guids: BTreeMap<String, (u32, u32)>,
    mode_guids: BTreeMap<String, (u32, u32)>,
    var_guids: BTreeMap<String, (u32, u32)>,
    style_guids: BTreeMap<String, (u32, u32)>,
    prop_guids: BTreeMap<(NodeId, String), (u32, u32)>,
    comp_keys: BTreeMap<NodeId, String>,
    grid_tracks: BTreeMap<NodeId, grid::GridTrackIds>,
    timeline_guids: BTreeMap<String, (u32, u32)>,
    track_guids: BTreeMap<(NodeId, usize), (u32, u32)>,
    track_guids_by_id: BTreeMap<String, (u32, u32)>,
    reserved_kf_guids: HashSet<(u32, u32)>,
    emitted_track_guids: HashSet<(u32, u32)>,
    blobs: Vec<Vec<u8>>,
    changes: Vec<V>,
}

fn parse_guid_pair(s: &str) -> Option<(u32, u32)> {
    let s = s.strip_prefix("VariableID:").or_else(|| s.strip_prefix("VariableCollectionId:")).unwrap_or(s);
    let (a, b) = s.split_once(':')?;
    if b.contains(':') || a.contains(';') || b.contains(';') {
        return None;
    }
    let session = a.parse::<u32>().ok()?;
    let local = b.parse::<u32>().ok()?;
    if (session == 0 && (local == 0 || local == 999_999)) || (session == u32::MAX && local == u32::MAX) {
        return None;
    }
    Some((session, local))
}

fn collect_json_guids(v: &serde_json::Value, out: &mut Vec<(u32, u32)>) {
    match v {
        serde_json::Value::Object(m) => {
            if let (Some(s), Some(l)) = (m.get("sessionID").and_then(serde_json::Value::as_u64), m.get("localID").and_then(serde_json::Value::as_u64)) {
                if let (Ok(s), Ok(l)) = (u32::try_from(s), u32::try_from(l)) {
                    if !((s == 0 && (l == 0 || l == 999_999)) || (s == u32::MAX && l == u32::MAX)) {
                        out.push((s, l));
                    }
                }
            }
            for child in m.values() {
                collect_json_guids(child, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_json_guids(item, out);
            }
        }
        _ => {}
    }
}

/// Fallback component key for `id` when no explicit `componentKey` or library key is present.
/// Uses the bare `"{s}:{l}"` GUID string unless another component already owns that string, in
/// which case it disambiguates as `"comp:{s}:{l}"`, `"comp:{s}:{l}:2"`, `"comp:{s}:{l}:3"`, …
fn fallback_component_key(cx: &ExportCtx<'_>, id: NodeId) -> Option<String> {
    if let Some(k) = cx.comp_keys.get(&id) {
        return Some(k.clone());
    }
    let g = cx.node_guids.get(&id)?;
    let taken_by_other = |candidate: &str| cx.comp_keys.iter().any(|(&other_id, k)| other_id != id && k == candidate);
    let bare = format!("{}:{}", g.0, g.1);
    if !taken_by_other(&bare) {
        return Some(bare);
    }
    let prefixed = format!("comp:{}:{}", g.0, g.1);
    if !taken_by_other(&prefixed) {
        return Some(prefixed);
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("comp:{}:{}:{n}", g.0, g.1);
        if !taken_by_other(&candidate) {
            return Some(candidate);
        }
        n += 1;
    }
}

impl<'a> ExportCtx<'a> {
    fn new(doc: &'a Document, schema: &'a crate::kiwi::Schema) -> Self {
        let mut used = HashSet::new();
        used.insert((0, 0));
        used.insert((0, 999_999));
        used.insert((u32::MAX, u32::MAX));
        let nc_def = schema.find("NodeChange").unwrap_or(0);
        let mut cx = Self {
            doc,
            schema,
            nc_def,
            used_guids: used,
            next_local: 1,
            node_guids: BTreeMap::new(),
            nodes_by_guid: None,
            fingerprints: Default::default(),
            shared_blobs: BTreeMap::new(),
            coll_guids: BTreeMap::new(),
            mode_guids: BTreeMap::new(),
            var_guids: BTreeMap::new(),
            style_guids: BTreeMap::new(),
            prop_guids: BTreeMap::new(),
            comp_keys: BTreeMap::new(),
            grid_tracks: BTreeMap::new(),
            timeline_guids: BTreeMap::new(),
            track_guids: BTreeMap::new(),
            track_guids_by_id: BTreeMap::new(),
            reserved_kf_guids: HashSet::new(),
            emitted_track_guids: HashSet::new(),
            blobs: Vec::new(),
            changes: Vec::new(),
        };
        let bump_next_local = |next_local: &mut u32, local: u32| {
            if let Some(next) = local.checked_add(1) {
                *next_local = (*next_local).max(next);
            }
        };
        // First pass: reserve explicit `s:l` GUIDs so generated ones never collide.
        for (&id, n) in &doc.nodes {
            if doc.library_page.is_some_and(|lp| n.parent == Some(lp)) && n.source_id.as_deref().is_some_and(|sid| doc.styles.contains_key(sid)) {
                continue;
            }
            if let Some(g) = n.source_id.as_deref().and_then(parse_guid_pair) {
                if cx.used_guids.insert(g) {
                    bump_next_local(&mut cx.next_local, g.1);
                    cx.node_guids.insert(id, g);
                }
            }
        }
        for id in doc.variable_collections.keys() {
            if let Some(g) = parse_guid_pair(id) {
                if cx.used_guids.insert(g) {
                    bump_next_local(&mut cx.next_local, g.1);
                    cx.coll_guids.insert(id.clone(), g);
                }
            }
        }
        for c in doc.variable_collections.values() {
            for m in &c.modes {
                if let Some(g) = parse_guid_pair(&m.id) {
                    if cx.used_guids.insert(g) {
                        bump_next_local(&mut cx.next_local, g.1);
                        cx.mode_guids.insert(m.id.clone(), g);
                    }
                }
            }
            if let Some(ext) = c.extension() {
                for pm in ext.mode_map.values() {
                    if !cx.mode_guids.contains_key(pm) {
                        if let Some(g) = parse_guid_pair(pm) {
                            if cx.used_guids.insert(g) {
                                bump_next_local(&mut cx.next_local, g.1);
                                cx.mode_guids.insert(pm.clone(), g);
                            }
                        }
                    }
                }
                for (vid, by_mode) in &ext.overrides {
                    if !cx.var_guids.contains_key(vid) {
                        if let Some(g) = parse_guid_pair(vid) {
                            if cx.used_guids.insert(g) {
                                bump_next_local(&mut cx.next_local, g.1);
                                cx.var_guids.insert(vid.clone(), g);
                            }
                        }
                    }
                    for val in by_mode.values() {
                        if let VariableValue::Alias(target) = val {
                            if !cx.var_guids.contains_key(target) {
                                if let Some(g) = parse_guid_pair(target) {
                                    if cx.used_guids.insert(g) {
                                        bump_next_local(&mut cx.next_local, g.1);
                                        cx.var_guids.insert(target.clone(), g);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for id in doc.variables.keys() {
            if let Some(g) = parse_guid_pair(id) {
                if cx.used_guids.insert(g) {
                    bump_next_local(&mut cx.next_local, g.1);
                    cx.var_guids.insert(id.clone(), g);
                }
            }
        }
        for id in doc.styles.keys() {
            if let Some(g) = parse_guid_pair(id) {
                if cx.used_guids.insert(g) {
                    bump_next_local(&mut cx.next_local, g.1);
                    cx.style_guids.insert(id.clone(), g);
                }
            }
        }
        for n in doc.nodes.values() {
            for vid in n.variables.bindings.values() {
                if let Some(g) = parse_guid_pair(vid) {
                    if cx.used_guids.insert(g) {
                        bump_next_local(&mut cx.next_local, g.1);
                    }
                }
            }
        }
        let mut sorted_motion_nodes: Vec<NodeId> = doc.nodes.iter().filter_map(|(&id, n)| n.motion.is_some().then_some(id)).collect();
        sorted_motion_nodes.sort_by_key(|id| id.0);
        for &id in &sorted_motion_nodes {
            let Some(motion) = doc.nodes[&id].motion.as_deref() else { continue };
            let tl_ids = motion
                .timelines
                .iter()
                .flat_map(|t| std::iter::once(t.id.as_str()).chain(t.parent.as_deref()))
                .chain(motion.assignments.iter().flat_map(|a| [a.timeline.as_str(), a.containing.as_str()]))
                .chain(motion.tracks.iter().map(|tr| tr.timeline.as_str()));
            for tid in tl_ids {
                if tid.is_empty() || cx.timeline_guids.contains_key(tid) {
                    continue;
                }
                if let Some(g) = parse_guid_pair(tid) {
                    if cx.used_guids.insert(g) {
                        bump_next_local(&mut cx.next_local, g.1);
                        cx.timeline_guids.insert(tid.to_string(), g);
                    }
                }
            }
            for (t_idx, tr) in motion.tracks.iter().enumerate() {
                if !tr.id.is_empty() {
                    if let Some(&existing) = cx.track_guids_by_id.get(&tr.id) {
                        cx.track_guids.insert((id, t_idx), existing);
                        continue;
                    }
                }
                if let Some(g) = parse_guid_pair(&tr.id) {
                    if cx.used_guids.insert(g) {
                        bump_next_local(&mut cx.next_local, g.1);
                        cx.track_guids.insert((id, t_idx), g);
                        if !tr.id.is_empty() {
                            cx.track_guids_by_id.insert(tr.id.clone(), g);
                        }
                    }
                }
            }
        }
        for style in doc.styles.values() {
            let Style::Animation(timelines) = style else { continue };
            for tid in timelines.iter().flat_map(|t| std::iter::once(t.id.as_str()).chain(t.parent.as_deref())) {
                if tid.is_empty() || cx.timeline_guids.contains_key(tid) {
                    continue;
                }
                if let Some(g) = parse_guid_pair(tid) {
                    if cx.used_guids.insert(g) {
                        bump_next_local(&mut cx.next_local, g.1);
                        cx.timeline_guids.insert(tid.to_string(), g);
                    }
                }
            }
        }
        for n in doc.nodes.values() {
            if let Some(raw_tm) = n.fig_metadata.get("_motionTrackMeta") {
                if let Ok(serde_json::Value::Object(tm_map)) = serde_json::from_str::<serde_json::Value>(raw_tm) {
                    for tmeta in tm_map.values() {
                        if let Some(kf_str) = tmeta.get("_keyframes").and_then(serde_json::Value::as_str) {
                            if let Ok(serde_json::Value::Array(kf_arr)) = serde_json::from_str::<serde_json::Value>(kf_str) {
                                for kf_meta in kf_arr {
                                    if let Some(g) = kf_meta.get("_guid").and_then(serde_json::Value::as_str).and_then(parse_guid_pair) {
                                        if cx.used_guids.insert(g) {
                                            bump_next_local(&mut cx.next_local, g.1);
                                            cx.reserved_kf_guids.insert(g);
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(pc_str) = tmeta.get("_passthroughChildren").and_then(serde_json::Value::as_str) {
                            if let Ok(pc_val) = serde_json::from_str::<serde_json::Value>(pc_str) {
                                let mut guids = Vec::new();
                                collect_json_guids(&pc_val, &mut guids);
                                for g in guids {
                                    if cx.used_guids.insert(g) {
                                        bump_next_local(&mut cx.next_local, g.1);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for key in ["timelineDefinitions", "timelineAssignments", "parameterConsumptionMap", "styleIdsForAnimation"] {
                if let Some(raw) = n.fig_metadata.get(key) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
                        let mut guids = Vec::new();
                        collect_json_guids(&val, &mut guids);
                        for g in guids {
                            if cx.used_guids.insert(g) {
                                bump_next_local(&mut cx.next_local, g.1);
                            }
                        }
                    }
                }
            }
        }
        // Second pass: allocate fresh GUIDs for anything without an explicit `s:l`.
        for &id in doc.nodes.keys() {
            if !cx.node_guids.contains_key(&id) {
                let g = cx.alloc_guid(1);
                cx.node_guids.insert(id, g);
            }
        }
        for id in doc.variable_collections.keys() {
            if !cx.coll_guids.contains_key(id) {
                let g = cx.alloc_guid(3);
                cx.coll_guids.insert(id.clone(), g);
            }
        }
        for c in doc.variable_collections.values() {
            for m in &c.modes {
                if !cx.mode_guids.contains_key(&m.id) {
                    let g = cx.alloc_guid(3);
                    cx.mode_guids.insert(m.id.clone(), g);
                }
            }
            if let Some(ext) = c.extension() {
                for pm in ext.mode_map.values() {
                    if !cx.mode_guids.contains_key(pm) {
                        let g = cx.alloc_guid(3);
                        cx.mode_guids.insert(pm.clone(), g);
                    }
                }
                for (vid, by_mode) in &ext.overrides {
                    if !cx.var_guids.contains_key(vid) {
                        let g = cx.alloc_guid(3);
                        cx.var_guids.insert(vid.clone(), g);
                    }
                    for val in by_mode.values() {
                        if let VariableValue::Alias(target) = val {
                            if !cx.var_guids.contains_key(target) {
                                let g = cx.alloc_guid(3);
                                cx.var_guids.insert(target.clone(), g);
                            }
                        }
                    }
                }
            }
        }
        for id in doc.variables.keys() {
            if !cx.var_guids.contains_key(id) {
                let g = cx.alloc_guid(3);
                cx.var_guids.insert(id.clone(), g);
            }
        }
        for id in doc.styles.keys() {
            if !cx.style_guids.contains_key(id) {
                let g = cx.alloc_guid(2);
                cx.style_guids.insert(id.clone(), g);
            }
        }
        for (&id, n) in &doc.nodes {
            for p in &n.component.props {
                let g = cx.alloc_guid(4);
                cx.prop_guids.insert((id, p.name.clone()), g);
            }
        }
        for (ext_key, asset) in &doc.library.external {
            if let Some(nid) = asset.node {
                let prefix = format!("{}:", asset.library_id);
                let raw_key = ext_key.strip_prefix(&prefix).unwrap_or(ext_key);
                if !raw_key.is_empty() {
                    cx.comp_keys.insert(nid, raw_key.to_string());
                }
            }
        }
        // Keys flow from a component to its instances, never the other way. Owners first: the
        // component's stored key (`componentKey` metadata) and the library assets that point at it.
        for (&id, n) in &doc.nodes {
            if matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
                if let Some(ck) = n.fig_metadata.get("componentKey").filter(|s| !s.is_empty()) {
                    cx.comp_keys.insert(id, ck.clone());
                }
            }
        }
        // An instance only names a component that has no key yet, and only with a key no other
        // component owns. Otherwise its key would make a second component claim the same key.
        let mut owned: std::collections::HashSet<String> = cx.comp_keys.values().cloned().collect();
        for n in doc.nodes.values() {
            if let NodeKind::Instance { component: Some(cid), component_key: Some(ck), .. } = &n.kind {
                if !ck.is_empty() && !cx.comp_keys.contains_key(cid) && !owned.contains(ck) {
                    cx.comp_keys.insert(*cid, ck.clone());
                    owned.insert(ck.clone());
                }
            }
        }
        let mut comp_ids: Vec<NodeId> =
            doc.nodes.iter().filter_map(|(&id, n)| matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)).then_some(id)).collect();
        comp_ids.sort_by_key(|id| id.0);
        for id in comp_ids {
            if !cx.comp_keys.contains_key(&id) {
                if let Some(key) = fallback_component_key(&cx, id) {
                    cx.comp_keys.insert(id, key);
                }
            }
        }
        grid::allocate(&mut cx);
        for &id in &sorted_motion_nodes {
            let Some(motion) = doc.nodes[&id].motion.as_deref() else { continue };
            let tl_ids: Vec<&str> = motion
                .timelines
                .iter()
                .flat_map(|t| std::iter::once(t.id.as_str()).chain(t.parent.as_deref()))
                .chain(motion.assignments.iter().flat_map(|a| [a.timeline.as_str(), a.containing.as_str()]))
                .chain(motion.tracks.iter().map(|tr| tr.timeline.as_str()))
                .collect();
            for tid in tl_ids {
                if !tid.is_empty() && !cx.timeline_guids.contains_key(tid) {
                    let s = crate::motion::guid_session_hint(tid, 3);
                    let g = cx.alloc_guid(s);
                    cx.timeline_guids.insert(tid.to_string(), g);
                }
            }
            for (t_idx, tr) in motion.tracks.iter().enumerate() {
                if !cx.track_guids.contains_key(&(id, t_idx)) {
                    if !tr.id.is_empty() {
                        if let Some(&existing) = cx.track_guids_by_id.get(&tr.id) {
                            cx.track_guids.insert((id, t_idx), existing);
                            continue;
                        }
                    }
                    let s = crate::motion::guid_session_hint(&tr.id, 1);
                    let g = cx.alloc_guid(s);
                    cx.track_guids.insert((id, t_idx), g);
                    if !tr.id.is_empty() {
                        cx.track_guids_by_id.insert(tr.id.clone(), g);
                    }
                }
            }
        }
        for style in doc.styles.values() {
            let Style::Animation(timelines) = style else { continue };
            let tl_ids: Vec<&str> = timelines.iter().flat_map(|t| std::iter::once(t.id.as_str()).chain(t.parent.as_deref())).collect();
            for tid in tl_ids {
                if !tid.is_empty() && !cx.timeline_guids.contains_key(tid) {
                    let s = crate::motion::guid_session_hint(tid, 3);
                    let g = cx.alloc_guid(s);
                    cx.timeline_guids.insert(tid.to_string(), g);
                }
            }
        }
        cx
    }

    fn timeline_guid(&self, id: &str) -> Option<(u32, u32)> {
        if id.is_empty() {
            return None;
        }
        self.timeline_guids.get(id).copied()
    }

    fn timeline_guid_or_alloc(&mut self, id: &str) -> (u32, u32) {
        if let Some(g) = self.timeline_guid(id) {
            return g;
        }
        let s = crate::motion::guid_session_hint(id, 3);
        let g = self.alloc_guid(s);
        if !id.is_empty() {
            self.timeline_guids.insert(id.to_string(), g);
        }
        g
    }

    fn track_guid(&self, nid: NodeId, idx: usize) -> (u32, u32) {
        self.track_guids.get(&(nid, idx)).copied().unwrap_or((1, 1))
    }

    fn alloc_guid(&mut self, session: u32) -> (u32, u32) {
        loop {
            let g = (session, self.next_local);
            self.next_local = self.next_local.wrapping_add(1).max(1);
            if self.used_guids.insert(g) {
                return g;
            }
        }
    }

    /// The node written with `g` as its guid.
    fn node_with_guid(&mut self, g: (u32, u32)) -> Option<NodeId> {
        self.nodes_by_guid.get_or_insert_with(|| self.node_guids.iter().map(|(&id, &guid)| (guid, id)).collect()).get(&g).copied()
    }

    fn add_blob(&mut self, bytes: Vec<u8>) -> u32 {
        let idx = self.blobs.len() as u32;
        self.blobs.push(bytes);
        idx
    }

    fn resolve_var_guid(&self, id: &str) -> Option<(u32, u32)> {
        self.var_guids.get(id).copied().or_else(|| self.var_guids.get(&format!("VariableID:{id}")).copied()).or_else(|| parse_guid_pair(id))
    }

    /// `VariableSetID` of an extended collection's parent: the parent's node when this file holds it, else the
    /// library key it was imported with.
    fn extension_parent_ref(&self, parent: &ParentCollection) -> Option<V> {
        let local_guid = (!parent.collection.is_empty() && (self.doc.variable_collections.contains_key(&parent.collection) || parent.set_key.is_none()))
            .then(|| self.resolve_coll_guid(&parent.collection))
            .flatten();
        match local_guid {
            Some(g) => Some(msg(vec![("guid", guid(g.0, g.1))])),
            None => parent.set_key.as_ref().map(|key| msg(vec![("assetRef", msg(vec![("key", V::String(key.clone()))]))])),
        }
    }

    fn resolve_coll_guid(&self, id: &str) -> Option<(u32, u32)> {
        self.coll_guids.get(id).copied().or_else(|| self.coll_guids.get(&format!("VariableCollectionId:{id}")).copied()).or_else(|| parse_guid_pair(id))
    }

    fn resolve_prop_guid(&self, scope_node: NodeId, prop_name: &str) -> (u32, u32) {
        if let Some(rest) = prop_name.strip_prefix("propDefId:") {
            if let Some(g) = parse_guid_pair(rest) {
                return g;
            }
        }
        let key = prop_name.split('#').next().unwrap_or(prop_name).to_string();
        let mut cur = Some(scope_node);
        while let Some(c) = cur {
            if let Some(&g) = self.prop_guids.get(&(c, key.clone())) {
                return g;
            }
            if let Some(n) = self.doc.get(c) {
                if let NodeKind::Instance { component: Some(master_id), .. } = n.kind {
                    if let Some(&g) = self.prop_guids.get(&(master_id, key.clone())) {
                        return g;
                    }
                    if let Some(set_id) = self.doc.get(master_id).and_then(|m| m.parent) {
                        if let Some(&g) = self.prop_guids.get(&(set_id, key.clone())) {
                            return g;
                        }
                    }
                }
                cur = n.parent;
            } else {
                break;
            }
        }
        if let Some((_, &g)) = self.prop_guids.iter().find(|((_, name), _)| name == &key) {
            return g;
        }
        let mut h = 2166136261u32;
        for b in key.as_bytes() {
            h = (h ^ (*b as u32)).wrapping_mul(16777619);
        }
        (4, h.max(1))
    }
}

fn pos_str(idx: usize) -> String {
    format!("!{idx:06}")
}

fn build_message(doc: &Document, schema: &crate::kiwi::Schema) -> V {
    let mut cx = ExportCtx::new(doc, schema);
    // 1. Root DOCUMENT record.
    let mut doc_fields = vec![("guid", guid(0, 0)), ("phase", en("CREATED")), ("type", en("DOCUMENT")), ("name", V::String(doc.name.clone()))];
    if let Some(profile) = doc.color_profile.figma_name() {
        doc_fields.push(("documentColorProfile", en(profile)));
    }
    if !doc.check_designs.ignored.is_empty() {
        let ignores: Vec<serde_json::Value> = doc
            .check_designs
            .ignored
            .iter()
            .map(|key| {
                let guid_str = cx.node_guids.get(&key.node).map(|&(s, l)| format!("{s}:{l}"));
                json!({
                    "rule": key.rule,
                    "node": key.node.0,
                    "guid": guid_str,
                    "property": key.property,
                })
            })
            .collect();
        if let Ok(serialized) = serde_json::to_string(&ignores) {
            doc_fields.push((
                "pluginData",
                V::Array(vec![msg(vec![
                    ("pluginID", V::String("designos".into())),
                    ("key", V::String("lint_ignores".into())),
                    ("value", V::String(serialized)),
                ])]),
            ));
        }
    }
    cx.changes.push(msg(doc_fields));

    // 2. User pages (CANVAS records) and their subtrees.
    for (pidx, &pid) in doc.pages.iter().enumerate() {
        let Some(page) = doc.get(pid) else { continue };
        let pguid = cx.node_guids[&pid];
        let bg = match page.kind {
            NodeKind::Page { background } => background,
            _ => Color::rgb(0xE5, 0xE5, 0xE5),
        };
        let mut fields = vec![
            ("guid", guid(pguid.0, pguid.1)),
            ("phase", en("CREATED")),
            ("parentIndex", st(vec![("guid", guid(0, 0)), ("position", V::String(pos_str(pidx)))])),
            ("type", en("CANVAS")),
            ("name", V::String(page.name.clone())),
            ("backgroundColor", color(bg.r, bg.g, bg.b, bg.a)),
        ];
        if let Some(guides) = doc.guides.get(&pid) {
            let page_guides: Vec<V> = guides
                .iter()
                .filter(|g| g.frame.is_none())
                .map(|g| msg(vec![("axis", en(if g.axis == GuideAxis::X { "X" } else { "Y" })), ("offset", fl(g.offset))]))
                .collect();
            if !page_guides.is_empty() {
                fields.push(("guides", V::Array(page_guides)));
            }
        }
        let handoff = collect_page_handoff(&cx, pid);
        if !handoff.is_empty() {
            fields.push(("handoffStatusMap", msg(vec![("entries", V::Array(handoff))])));
        }
        cx.changes.push(msg(fields));
        let children = page.children.clone();
        for (cidx, cid) in children.into_iter().enumerate() {
            export_node_rec(&mut cx, cid, pguid, cidx, None, false);
        }
    }

    // 3. Internal-only canvas for variables, styles, and library components.
    let detached_preferred = detached_slot_preferred(doc);
    let has_internal = !doc.variable_collections.is_empty()
        || !doc.variables.is_empty()
        || !doc.styles.is_empty()
        || doc.library_page.is_some()
        || !detached_preferred.is_empty();
    if has_internal {
        let iguid = (0, 999_999);
        cx.changes.push(msg(vec![
            ("guid", guid(iguid.0, iguid.1)),
            ("phase", en("CREATED")),
            ("parentIndex", st(vec![("guid", guid(0, 0)), ("position", V::String(pos_str(doc.pages.len())))])),
            ("type", en("CANVAS")),
            ("name", V::String("Internal Only Canvas".into())),
            ("internalOnly", V::Bool(true)),
        ]));
        let mut internal_idx = 0usize;
        export_variables_on_internal(&mut cx, iguid, &mut internal_idx);
        export_styles_on_internal(&mut cx, iguid, &mut internal_idx);
        if let Some(lp) = doc.library_page {
            if let Some(lib_page) = doc.get(lp) {
                let kids = lib_page.children.clone();
                for cid in kids {
                    if doc.get(cid).and_then(|n| n.source_id.as_deref()).is_some_and(|sid| doc.styles.contains_key(sid)) {
                        continue;
                    }
                    export_node_rec(&mut cx, cid, iguid, internal_idx, None, false);
                    internal_idx += 1;
                }
            }
        }
        for cid in detached_preferred {
            export_node_rec(&mut cx, cid, iguid, internal_idx, None, false);
            internal_idx += 1;
        }
    }

    let blobs: Vec<V> = cx.blobs.into_iter().map(|b| st(vec![("bytes", V::Bytes(b))])).collect();
    msg(vec![("type", en("NODE_CHANGES")), ("sessionID", V::Uint(0)), ("ackID", V::Uint(0)), ("nodeChanges", V::Array(cx.changes)), ("blobs", V::Array(blobs))])
}

/// Whether `id` hangs under a page or the library page.
fn is_attached(doc: &Document, id: NodeId) -> bool {
    let mut cur = id;
    // A parent chain longer than the node count is a cycle: such a node is not attached.
    for _ in 0..=doc.nodes.len() {
        if doc.pages.contains(&cur) || doc.library_page == Some(cur) {
            return true;
        }
        match doc.get(cur).and_then(|n| n.parent) {
            Some(p) => cur = p,
            None => return false,
        }
    }
    false
}

/// Whether `id` or one of its ancestors is in `set`.
fn self_or_ancestor_in(doc: &Document, id: NodeId, set: &std::collections::BTreeSet<NodeId>) -> bool {
    let mut cur = Some(id);
    for _ in 0..=doc.nodes.len() {
        let Some(c) = cur else { return false };
        if set.contains(&c) {
            return true;
        }
        cur = doc.get(c).and_then(|n| n.parent);
    }
    false
}

/// Components that only a slot's preferred list keeps alive: they sit on no page, yet the slot definition
/// writes their guid. They are exported on the internal canvas so the guid resolves on import.
///
/// A detached component that is itself written can in turn prefer another detached one, so the search runs
/// to a fixed point. A node under another collected node (a variant of a preferred set) is written with
/// that node, not again on its own.
fn detached_slot_preferred(doc: &Document) -> Vec<NodeId> {
    let mut found = std::collections::BTreeSet::new();
    loop {
        let before = found.len();
        for (&holder, n) in &doc.nodes {
            if n.component.slot_configs.is_empty() || !(is_attached(doc, holder) || self_or_ancestor_in(doc, holder, &found)) {
                continue;
            }
            for pid in n.component.slot_configs.values().flat_map(|c| c.preferred.iter().copied()) {
                if doc.get(pid).is_some() && !is_attached(doc, pid) {
                    found.insert(pid);
                }
            }
        }
        if found.len() == before {
            break;
        }
    }
    found
        .iter()
        .copied()
        .filter(|&id| {
            let parent = doc.get(id).and_then(|n| n.parent);
            !parent.is_some_and(|p| self_or_ancestor_in(doc, p, &found))
        })
        .collect()
}

fn collect_page_handoff(cx: &ExportCtx, page: NodeId) -> Vec<V> {
    let mut entries = Vec::new();
    let mut stack: Vec<NodeId> = cx.doc.get(page).map(|n| n.children.iter().rev().copied().collect()).unwrap_or_default();
    while let Some(id) = stack.pop() {
        let Some(n) = cx.doc.get(id) else { continue };
        let status = match n.dev.status {
            Some(DevStatus::ReadyForDev | DevStatus::Changed) => Some("BUILD"),
            Some(DevStatus::Completed) => Some("COMPLETED"),
            None => None,
        };
        if let Some(st_name) = status {
            let g = cx.node_guids[&id];
            entries.push(msg(vec![("guid", guid(g.0, g.1)), ("handoffStatus", msg(vec![("status", en(st_name))]))]));
        }
        stack.extend(n.children.iter().rev().copied());
    }
    entries
}

fn export_variables_on_internal(cx: &mut ExportCtx, internal_guid: (u32, u32), idx: &mut usize) {
    for c in cx.doc.variable_collections.values() {
        let cguid = cx.coll_guids[&c.id];
        // Ensure default_mode sorts first so `variables::from_fig_entries` picks it as `defaultModeId`.
        let mut ordered_modes: Vec<&VariableMode> = Vec::with_capacity(c.modes.len());
        if let Some(dm) = c.modes.iter().find(|m| m.id == c.default_mode) {
            ordered_modes.push(dm);
        }
        for m in &c.modes {
            if m.id != c.default_mode {
                ordered_modes.push(m);
            }
        }
        let extension = c.extension();
        let modes: Vec<V> = ordered_modes
            .iter()
            .enumerate()
            .map(|(mi, m)| {
                let mguid = cx.mode_guids[&m.id];
                let mut fields = vec![("id", guid(mguid.0, mguid.1)), ("name", V::String(m.name.clone())), ("sortPosition", V::String(pos_str(mi)))];
                if let Some(ext) = &extension {
                    if let Some(parent) = cx.extension_parent_ref(&ext.parent) {
                        fields.push(("parentVariableSetId", parent));
                        if let Some(&pm) = ext.mode_map.get(&m.id).and_then(|pm| cx.mode_guids.get(pm)) {
                            fields.push(("parentModeId", guid(pm.0, pm.1)));
                        }
                    }
                }
                msg(fields)
            })
            .collect();
        let mut fields = vec![
            ("guid", guid(cguid.0, cguid.1)),
            ("phase", en("CREATED")),
            ("parentIndex", st(vec![("guid", guid(internal_guid.0, internal_guid.1)), ("position", V::String(pos_str(*idx)))])),
            ("type", en("VARIABLE_SET")),
            ("name", V::String(c.name.clone())),
            ("variableSetModes", V::Array(modes)),
        ];
        if let Some(k) = &c.key {
            fields.push(("key", V::String(k.clone())));
        }
        if let Some(lib) = &c.source_library_key {
            fields.push(("sourceLibraryKey", V::String(lib.clone())));
        }
        if c.is_extendable() {
            fields.push(("isCollectionExtendable", V::Bool(true)));
        }
        // A collection the file deleted but kept for the variables still in use is written back as deleted.
        if c.is_soft_deleted() {
            fields.push(("isSoftDeleted", V::Bool(true)));
        }
        cx.changes.push(msg(fields));
        *idx += 1;
    }

    let encode = |cx: &ExportCtx, val: &VariableValue| match val {
        VariableValue::Color(c) => Some(msg(vec![("dataType", en("COLOR")), ("value", msg(vec![("colorValue", color(c.r, c.g, c.b, c.a))]))])),
        VariableValue::Number(f) => Some(msg(vec![("dataType", en("FLOAT")), ("value", msg(vec![("floatValue", fl(*f))]))])),
        VariableValue::String(s) => Some(msg(vec![("dataType", en("STRING")), ("value", msg(vec![("textValue", V::String(s.clone()))]))])),
        VariableValue::Boolean(b) => Some(msg(vec![("dataType", en("BOOLEAN")), ("value", msg(vec![("boolValue", V::Bool(*b))]))])),
        VariableValue::Alias(target) => {
            let tguid = cx.resolve_var_guid(target)?;
            Some(msg(vec![("dataType", en("ALIAS")), ("value", msg(vec![("alias", msg(vec![("guid", guid(tguid.0, tguid.1))]))]))]))
        }
    };
    let mut emitted_vars = HashSet::new();
    let mut sort_idx = 0usize;
    for v in cx.doc.variables.values() {
        if !cx.doc.variable_collections.contains_key(&v.collection)
            && cx.doc.variable_collections.values().any(|c| c.extension().is_some_and(|e| e.overrides.contains_key(&v.id) && e.parent.set_key.is_some()))
        {
            continue;
        }
        let vguid = cx.var_guids[&v.id];
        let Some(cguid) = cx.resolve_coll_guid(&v.collection) else { continue };
        emitted_vars.insert(v.id.clone());
        let rtype = match v.kind {
            VariableKind::Color => "COLOR",
            VariableKind::Number => "FLOAT",
            VariableKind::String => "STRING",
            VariableKind::Boolean => "BOOLEAN",
        };
        let mut val_entries = Vec::new();
        for (mode_id, val) in &v.values {
            let Some(&mguid) = cx.mode_guids.get(mode_id) else { continue };
            let Some(vdata) = encode(cx, val) else { continue };
            val_entries.push(msg(vec![("modeID", guid(mguid.0, mguid.1)), ("variableData", vdata)]));
        }
        // UNVERIFIED layout (no real extended-collection `.fig` was available; the alternative is `VARIABLE_OVERRIDE`
        // nodes, which the importer counts and does not read): the overrides of an extended collection are written as
        // extra entries on the overridden variable, keyed by the extension's own mode ids.
        for ext in cx.doc.variable_collections.values().filter(|c| c.is_extension()) {
            for (mode_id, val) in ext.extension().iter().filter_map(|e| e.overrides.get(&v.id).cloned()).flatten() {
                let Some(&mguid) = cx.mode_guids.get(&mode_id) else { continue };
                let Some(vdata) = encode(cx, &val) else { continue };
                val_entries.push(msg(vec![("modeID", guid(mguid.0, mguid.1)), ("variableData", vdata)]));
            }
        }
        // The REST variable payload spells the stroke colour scope `STROKE_COLOR`,
        // while the native `.fig` schema uses `STROKE`. Normalize before schema
        // validation so stroke-only variables are not silently widened to all
        // scopes, and preserve input order while removing duplicate spellings.
        let mut seen_scopes = HashSet::new();
        let scopes: Vec<V> = v
            .scopes
            .iter()
            .filter_map(|scope| {
                let fig_scope = if scope == "STROKE_COLOR" { "STROKE" } else { scope.as_str() };
                (cx.schema.has_enum_variant("VariableScope", fig_scope) && seen_scopes.insert(fig_scope)).then(|| en(fig_scope))
            })
            .collect();
        let mut fields = vec![
            ("guid", guid(vguid.0, vguid.1)),
            ("phase", en("CREATED")),
            ("parentIndex", st(vec![("guid", guid(internal_guid.0, internal_guid.1)), ("position", V::String(pos_str(*idx)))])),
            ("type", en("VARIABLE")),
            ("name", V::String(v.name.clone())),
            ("description", V::String(v.description.clone())),
            ("sortPosition", V::String(pos_str(sort_idx))),
            ("variableSetID", msg(vec![("guid", guid(cguid.0, cguid.1))])),
            ("variableResolvedType", en(rtype)),
            ("variableDataValues", msg(vec![("entries", V::Array(val_entries))])),
        ];
        sort_idx += 1;
        if !scopes.is_empty() {
            fields.push(("variableScopes", V::Array(scopes)));
        }
        let syntax: Vec<V> = v
            .code_syntax
            .iter()
            .filter(|(_, code)| !code.is_empty())
            .map(|(p, code)| msg(vec![("platform", en(p.label())), ("value", V::String(code.clone()))]))
            .collect();
        if !syntax.is_empty() {
            fields.push(("codeSyntax", msg(vec![("entries", V::Array(syntax))])));
        }
        if let Some(k) = &v.key {
            fields.push(("key", V::String(k.clone())));
        }
        // The file's own publish switch: only a variable hidden from publishing writes it.
        if v.hidden_from_publishing {
            fields.push(("isPublishable", V::Bool(false)));
        }
        // A deleted variable the file still uses stays in the file as deleted; it must not come back as a live one.
        if v.is_soft_deleted() {
            fields.push(("isSoftDeleted", V::Bool(true)));
        }
        cx.changes.push(msg(fields));
        *idx += 1;
    }

    // Overrides of variables whose parent collection is known only by library key (`set_key`).
    let ext_colls: Vec<CollectionExtension> = cx.doc.variable_collections.values().filter_map(VariableCollection::extension).collect();
    for ext in &ext_colls {
        let Some(parent_ref) = cx.extension_parent_ref(&ext.parent) else { continue };
        for var_id in ext.overrides.keys() {
            if !emitted_vars.insert(var_id.clone()) {
                continue;
            }
            let Some(&vguid) = cx.var_guids.get(var_id) else { continue };
            let mut val_entries = Vec::new();
            let mut first_val = None;
            for other in &ext_colls {
                for (mode_id, val) in other.overrides.get(var_id).into_iter().flatten() {
                    let Some(&mguid) = cx.mode_guids.get(mode_id) else { continue };
                    let Some(vdata) = encode(cx, val) else { continue };
                    if first_val.is_none() {
                        first_val = Some(val.clone());
                    }
                    val_entries.push(msg(vec![("modeID", guid(mguid.0, mguid.1)), ("variableData", vdata)]));
                }
            }
            if val_entries.is_empty() {
                continue;
            }
            let rtype = match cx.doc.variables.get(var_id).map(|v| v.kind) {
                Some(VariableKind::Color) => "COLOR",
                Some(VariableKind::Number) => "FLOAT",
                Some(VariableKind::String) => "STRING",
                Some(VariableKind::Boolean) => "BOOLEAN",
                None => match &first_val {
                    Some(VariableValue::Color(_)) => "COLOR",
                    Some(VariableValue::Number(_)) => "FLOAT",
                    Some(VariableValue::String(_)) => "STRING",
                    Some(VariableValue::Boolean(_)) => "BOOLEAN",
                    Some(VariableValue::Alias(target)) => match cx.doc.variables.get(target).map(|v| v.kind) {
                        Some(VariableKind::Number) => "FLOAT",
                        Some(VariableKind::String) => "STRING",
                        Some(VariableKind::Boolean) => "BOOLEAN",
                        _ => "COLOR",
                    },
                    None => continue,
                },
            };
            let name = cx.doc.variables.get(var_id).map(|v| v.name.clone()).unwrap_or_else(|| var_id.clone());
            let fields = vec![
                ("guid", guid(vguid.0, vguid.1)),
                ("phase", en("CREATED")),
                ("parentIndex", st(vec![("guid", guid(internal_guid.0, internal_guid.1)), ("position", V::String(pos_str(*idx)))])),
                ("type", en("VARIABLE")),
                ("name", V::String(name)),
                ("description", V::String(String::new())),
                ("sortPosition", V::String(pos_str(sort_idx))),
                ("variableSetID", parent_ref.clone()),
                ("variableResolvedType", en(rtype)),
                ("variableDataValues", msg(vec![("entries", V::Array(val_entries))])),
            ];
            sort_idx += 1;
            cx.changes.push(msg(fields));
            *idx += 1;
        }
    }
}

/// Generates a deterministic `sortPosition` key for a style that has no stored `sort_position`
/// (`None`) in Studio, using the format `"{max_existing}{ordinal:06}"` (where `max_existing` is
/// the byte-wise largest existing `sort_position` among exported styles, or `""` when none
/// exists, and `ordinal` is the 1-based position among unpositioned styles in
/// [`designos_doc::ordered_style_keys`] walk order).
///
/// Why this format:
/// - Appending a non-empty suffix to `max_existing` makes every generated key compare strictly
///   greater than `max_existing` (and therefore after all positioned styles) under ASCII byte
///   order (`str::cmp`), while fixed-width 6-digit zero-padded numbers sort the generated keys
///   among themselves in walk order.
/// - Using only characters from `0-9A-Za-z` (`0x30..=0x7A`) keeps every byte strictly above
///   Figma's smallest fractional-index character (`'!'`, `0x21`) and below `'~'` (`0x7E`), so
///   a key never ends in the smallest character and Figma can always insert between any two keys.
fn generated_style_sort_position(max_existing: &str, ordinal: usize) -> String {
    format!("{max_existing}{ordinal:06}")
}

fn style_paints_for_export(doc: &Document, paints: &[Paint], bindings: &BTreeMap<String, String>) -> Vec<Paint> {
    if !paints.is_empty() {
        return paints.to_vec();
    }
    let Some(var_id) = bindings
        .get("fills/0")
        .or_else(|| bindings.get("fills/0/color"))
        .or_else(|| bindings.get("fills"))
        .or_else(|| bindings.iter().find(|(k, _)| k.starts_with("fills/")).map(|(_, v)| v))
    else {
        return Vec::new();
    };
    let color = doc
        .resolve_variable(NodeId(0), var_id)
        .and_then(|val| match val {
            VariableValue::Color(c) => Some(c),
            _ => None,
        })
        .unwrap_or(Color::BLACK);
    vec![Paint::solid(color)]
}

fn export_styles_on_internal(cx: &mut ExportCtx, internal_guid: (u32, u32), idx: &mut usize) {
    // Import-seeded orphans (GUID / `key:` refs with no `style_meta`) stay referenced by layers but are not style
    // nodes, so a re-import must not resurrect them. Consumer-seeded `S:` styles are real styles and still export.
    let styles: Vec<(String, Style)> = designos_doc::ordered_style_keys(cx.doc, None)
        .into_iter()
        .filter(|k| k.starts_with("S:") || cx.doc.is_table_style(k))
        .filter_map(|k| cx.doc.styles.get(&k).cloned().map(|s| (k, s)))
        .collect();
    let max_existing_pos: String = styles
        .iter()
        .filter_map(|(sid, _)| cx.doc.style_meta.get(sid).and_then(|m| m.sort_position.as_deref()).filter(|s| !s.is_empty()))
        .max()
        .unwrap_or("")
        .to_string();
    let mut unpositioned_ordinal = 0usize;
    let empty_bindings = BTreeMap::new();
    for (sid, style) in styles {
        let sguid = cx.style_guids[&sid];
        let meta = cx.doc.style_meta.get(&sid);
        let bindings = meta.map(|m| &m.bindings).unwrap_or(&empty_bindings);
        let name = cx.doc.style_name(&sid).to_string();
        let desc = meta.map(|m| m.description.clone()).unwrap_or_default();
        let (node_ty, style_ty) = match &style {
            Style::Fill(_) => ("RECTANGLE", "FILL"),
            Style::Text(_) => ("TEXT", "TEXT"),
            Style::Effect(_) => ("RECTANGLE", "EFFECT"),
            Style::Grid(_) => ("FRAME", "GRID"),
            Style::Animation(_) => ("FRAME", "ANIMATION"),
        };
        let mut fields = vec![
            ("guid", guid(sguid.0, sguid.1)),
            ("phase", en("CREATED")),
            ("parentIndex", st(vec![("guid", guid(internal_guid.0, internal_guid.1)), ("position", V::String(pos_str(*idx)))])),
            ("type", en(node_ty)),
            ("name", V::String(name)),
            ("styleType", en(style_ty)),
            ("styleDescription", V::String(desc)),
            ("size", vec2(1.0, 1.0)),
        ];
        if let Some(k) = meta.and_then(|m| m.key.as_ref()) {
            fields.push(("key", V::String(k.clone())));
        }
        if cx.doc.is_table_style(&sid) {
            let sort_pos = match meta.and_then(|m| m.sort_position.as_deref()).filter(|s| !s.is_empty()) {
                Some(sp) => sp.to_string(),
                None => {
                    unpositioned_ordinal += 1;
                    generated_style_sort_position(&max_existing_pos, unpositioned_ordinal)
                }
            };
            fields.push(("sortPosition", V::String(sort_pos)));
        }
        match &style {
            Style::Fill(paints) => {
                let export_paints = style_paints_for_export(cx.doc, paints, bindings);
                fields.push((
                    "fillPaints",
                    V::Array(export_paints.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "fills", i, bindings)).collect()),
                ));
            }
            Style::Text(ts) => {
                append_shared_text_style_fields(&mut fields, ts);
                // The paints of the style are the text colour a layer takes from it; a style without paints writes no field.
                let export_paints = style_paints_for_export(cx.doc, &ts.fills, bindings);
                if !export_paints.is_empty() {
                    fields.push((
                        "fillPaints",
                        V::Array(export_paints.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "fills", i, bindings)).collect()),
                    ));
                }
                let var_entries = encode_var_entries_from_bindings(cx, bindings);
                if !var_entries.is_empty() {
                    fields.push(("variableConsumptionMap", msg(vec![("entries", V::Array(var_entries))])));
                }
            }
            Style::Effect(effects) => {
                fields.push(("effects", V::Array(effects.iter().enumerate().map(|(i, e)| encode_effect_with_bindings(cx, e, i, bindings)).collect())));
            }
            Style::Grid(grids) => {
                fields.push(("layoutGrids", V::Array(grids.iter().map(encode_grid).collect())));
            }
            Style::Animation(timelines) => {
                if !timelines.is_empty() {
                    let tl_json = crate::motion::timelines_to_kiwi_with(timelines, |tid| cx.timeline_guid_or_alloc(tid));
                    if let Some((_, val)) = cx.schema.def_field_from_json(cx.nc_def, "timelineDefinitions", &tl_json) {
                        fields.push(("timelineDefinitions", val));
                    }
                }
            }
        }
        cx.changes.push(msg(fields));
        *idx += 1;
    }
}

/// Slot content id written on the definition of slot property `prop_name`. It comes from the
/// per-property map only: the node's own `slot_content_id` is a different field (carried in the
/// plugin data entry `slot-content-id`) and is never turned into a property assignment.
fn def_slot_content_id<'a>(link: &'a ComponentLink, prop_name: &str) -> Option<&'a str> {
    link.slot_content_ids.get(prop_name).map(String::as_str)
}

fn export_node_rec(cx: &mut ExportCtx, id: NodeId, parent_guid: (u32, u32), sibling_idx: usize, parent_dir: Option<LayoutDirection>, in_component: bool) {
    let Some(node) = cx.doc.get(id).cloned() else { return };
    let nguid = cx.node_guids[&id];
    let lowered_conn = match (&node.kind, cx.doc.kind) {
        (NodeKind::Connector(cp), k) if k != DocumentKind::Board => {
            let fmt_guid = |nid: NodeId| cx.node_guids.get(&nid).map(|&(s, l)| format!("{s}:{l}"));
            let carrier = crate::board::StudioConnectorCarrier::from_node(&node, cp, cp.start.node.and_then(fmt_guid), cp.end.node.and_then(fmt_guid));
            let carrier_val = serde_json::to_value(&carrier).unwrap_or(serde_json::Value::Null);
            let lowered = crate::board::lower_design_connector(cx.doc, id, &node, cp, true);
            Some((lowered, carrier_val))
        }
        _ => None,
    };
    let (ty, is_state_group) = match &node.kind {
        NodeKind::Page { .. } => ("CANVAS", false),
        NodeKind::Frame(_) => ("FRAME", false),
        NodeKind::Group => ("GROUP", false),
        NodeKind::Section => ("SECTION", false),
        NodeKind::Rectangle => ("ROUNDED_RECTANGLE", false),
        NodeKind::Ellipse(_) => ("ELLIPSE", false),
        NodeKind::Line => ("LINE", false),
        NodeKind::Polygon { .. } => ("REGULAR_POLYGON", false),
        NodeKind::Star { .. } => ("STAR", false),
        NodeKind::Vector { .. } => (if node.fig_metadata.contains_key("brushType") { "BRUSH" } else { "VECTOR" }, false),
        NodeKind::Text(_) => ("TEXT", false),
        NodeKind::BooleanOp(_) => ("BOOLEAN_OPERATION", false),
        NodeKind::Component(_) => ("SYMBOL", false),
        NodeKind::ComponentSet(_) => ("FRAME", true),
        NodeKind::Instance { .. } => ("INSTANCE", false),
        NodeKind::Sticky(_) => ("STICKY", false),
        NodeKind::ShapeWithText(_) => ("SHAPE_WITH_TEXT", false),
        NodeKind::Connector(_) => match &lowered_conn {
            Some((lc, _)) if lc.label_chip.is_some() => ("GROUP", false),
            Some(_) => ("VECTOR", false),
            None => ("CONNECTOR", false),
        },
        NodeKind::Stamp { .. } => ("STAMP", false),
        NodeKind::Table(_) => ("TABLE", false),
        NodeKind::TableCell(_) => ("TABLE_CELL", false),
        NodeKind::Unknown { type_name } => (if cx.schema.has_enum_variant("NodeType", type_name) { type_name.as_str() } else { "FRAME" }, false),
    };

    let mut fields = vec![
        ("guid", guid(nguid.0, nguid.1)),
        ("phase", en("CREATED")),
        ("parentIndex", st(vec![("guid", guid(parent_guid.0, parent_guid.1)), ("position", V::String(pos_str(sibling_idx)))])),
        ("type", en(ty)),
        ("name", V::String(node.name.clone())),
        ("visible", V::Bool(node.visible)),
        ("locked", V::Bool(node.locked)),
        ("opacity", fl_one(node.opacity)),
        ("size", vec2(node.size.x, node.size.y)),
        ("transform", encode_matrix(&node.transform)),
        ("blendMode", en(blend_name(node.blend))),
    ];
    if is_state_group {
        fields.push(("isStateGroup", V::Bool(true)));
    }
    if node.is_mask {
        fields.push(("mask", V::Bool(true)));
    }
    if node.is_mask || node.mask_type != MaskType::Alpha {
        // A type kept only in fig_metadata (imported before mask types were mapped) is written back.
        let mt = match node.mask_type {
            MaskType::Alpha => match node.fig_metadata.get("maskType").map(String::as_str) {
                Some("OUTLINE" | "VECTOR") => "OUTLINE",
                Some("LUMINANCE") => "LUMINANCE",
                _ => "ALPHA",
            },
            MaskType::Vector => "OUTLINE",
            MaskType::Luminance => "LUMINANCE",
        };
        fields.push(("maskType", en(mt)));
    }
    if node.lock_aspect {
        fields.push(("proportionsConstrained", V::Bool(true)));
    }
    if let Some(hl) = node.hyperlink.as_deref().filter(|s| !s.is_empty()) {
        let hl_msg = if let Some((s_str, l_str)) = hl.strip_prefix("guid:").and_then(|r| r.split_once(':')) {
            if let (Ok(s), Ok(l)) = (s_str.parse::<u32>(), l_str.parse::<u32>()) {
                msg(vec![("guid", guid(s, l))])
            } else {
                msg(vec![("url", V::String(hl.to_string()))])
            }
        } else {
            msg(vec![("url", V::String(hl.to_string()))])
        };
        fields.push(("hyperlink", hl_msg));
    }
    // A node's own plugin data goes out unchanged; the reserved carrier keys are rebuilt below from typed
    // data, so a stale copy of one in `plugin_data` is not written twice.
    let mut carried: carrier::Carried = Vec::new();
    let studio_entry =
        |key: &str, value: String| msg(vec![("pluginID", V::String(STUDIO_PLUGIN_ID.into())), ("key", V::String(key.into())), ("value", V::String(value))]);
    let mut pd_entries: Vec<V> = node
        .plugin_data
        .iter()
        .filter(|(full_key, _)| !is_studio_carrier_key(full_key) && *full_key != "designos:in_progress")
        .map(|(full_key, val)| {
            let mut m = Vec::with_capacity(3);
            if let Some((pid, key)) = full_key.split_once(':') {
                if !pid.is_empty() {
                    m.push(("pluginID", V::String(pid.to_string())));
                }
                m.push(("key", V::String(key.to_string())));
            } else {
                m.push(("key", V::String(full_key.clone())));
            }
            m.push(("value", V::String(val.clone())));
            msg(m)
        })
        .collect();
    // Figma's NodeChange has no field for either value, so they ride in plugin data under the reserved id.
    if matches!(&node.kind, NodeKind::Sticky(sp) if sp.show_author) {
        pd_entries.push(studio_entry(STUDIO_AUTHOR_VISIBLE_KEY, "true".into()));
    }
    if let Some(id) = node.component.slot_content_id.as_deref().filter(|s| !s.is_empty()) {
        pd_entries.push(studio_entry(STUDIO_SLOT_CONTENT_ID_KEY, id.to_string()));
    }
    if let Some(ip) = cx.doc.in_progress.get(&id) {
        if let Ok(val) = serde_json::to_string(ip) {
            pd_entries.push(msg(vec![("pluginID", V::String("designos".into())), ("key", V::String("in_progress".into())), ("value", V::String(val))]));
        }
    }
    if !node.component.description.is_empty() {
        fields.push(("symbolDescription", V::String(node.component.description.clone())));
    }
    if node.component.expose_nested_instances {
        fields.push(("propsAreBubbled", V::Bool(true)));
    }
    if let Some(lvl) = node.component.override_level {
        fields.push(("overrideLevel", V::Int(lvl)));
    }
    if let Some(ds) = node.detached_symbol_id.as_deref().or(node.component.detached_symbol_id.as_deref()).filter(|s| !s.is_empty()) {
        fields.push(("detachedSymbolId", encode_detached_symbol_id(ds)));
    }
    if node.component.is_slot {
        fields.push(("isSlot", V::Bool(true)));
    }
    if node.component.is_slot_content {
        fields.push(("isSlotContent", V::Bool(true)));
    }
    if let Some(src) = authored_source_guid(cx, id) {
        fields.push(("overrideKey", guid(src.0, src.1)));
    } else if in_component {
        fields.push(("overrideKey", guid(nguid.0, nguid.1)));
    }

    if let Some(ds) = node.dev.status {
        let st_name = match ds {
            DevStatus::ReadyForDev | DevStatus::Changed => "BUILD",
            DevStatus::Completed => "COMPLETED",
        };
        fields.push(("sectionStatusInfo", msg(vec![("status", en(st_name))])));
    }

    // Fills & strokes (with bound colorVar / stopsVar / opacityVar when present).
    let sticky_fill_buf;
    let fills_slice: &[Paint] = match &node.kind {
        NodeKind::Text(t) => &t.style.fills,
        NodeKind::Sticky(s) if node.fills.is_empty() => {
            sticky_fill_buf = [Paint::solid(s.color)];
            &sticky_fill_buf
        }
        _ => &node.fills,
    };
    let fill_paints: Vec<V> = fills_slice.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "fills", i, &node.variables.bindings)).collect();
    fields.push(("fillPaints", V::Array(fill_paints)));

    let is_lowered_conn_group = lowered_conn.as_ref().is_some_and(|(lc, _)| lc.label_chip.is_some());
    let stroke_paints: Vec<V> = if is_lowered_conn_group {
        Vec::new()
    } else {
        node.stroke.paints.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "strokes", i, &node.variables.bindings)).collect()
    };
    fields.push(("strokePaints", V::Array(stroke_paints)));
    fields.push(("strokeWeight", fl(if is_lowered_conn_group { 0.0 } else { node.stroke.weight })));
    fields.push((
        "strokeAlign",
        en(match node.stroke.align {
            StrokeAlign::Inside => "INSIDE",
            StrokeAlign::Center => "CENTER",
            StrokeAlign::Outside => "OUTSIDE",
        }),
    ));
    fields.push((
        "strokeJoin",
        en(match node.stroke.join {
            StrokeJoin::Miter => "MITER",
            StrokeJoin::Bevel => "BEVEL",
            StrokeJoin::Round => "ROUND",
        }),
    ));
    fields.push((
        "strokeCap",
        en(match node.stroke.cap {
            StrokeCap::None => node.fig_metadata.get("strokeCap").map(String::as_str).filter(|s| cx.schema.has_enum_variant("StrokeCap", s)).unwrap_or("NONE"),
            c => crate::vecnet::cap_name(c),
        }),
    ));
    if !node.stroke.dashes.is_empty() {
        fields.push(("dashPattern", V::Array(node.stroke.dashes.iter().map(|&d| fl(d)).collect())));
    }
    if node.stroke.miter_limit >= 1.0 {
        fields.push(("miterLimit", fl(node.stroke.miter_limit)));
    }
    if let Some([top, right, bottom, left]) = node.stroke.side_weights {
        fields.push(("borderStrokeWeightsIndependent", V::Bool(true)));
        fields.push(("borderTopWeight", fl(top)));
        fields.push(("borderRightWeight", fl(right)));
        fields.push(("borderBottomWeight", fl(bottom)));
        fields.push(("borderLeftWeight", fl(left)));
    }

    // Effects.
    let effects: Vec<V> = node.effects.iter().enumerate().map(|(i, e)| encode_effect_with_bindings(cx, e, i, &node.variables.bindings)).collect();
    if !effects.is_empty() {
        fields.push(("effects", V::Array(effects)));
    }

    // Corner radii & smoothing.
    let r = node.corner_radii;
    if r.iter().all(|&x| (x - r[0]).abs() < 1e-5) {
        if r[0] != 0.0 {
            fields.push(("cornerRadius", fl(r[0])));
        }
    } else {
        fields.push(("cornerRadius", fl(r[0])));
        fields.push(("rectangleCornerRadiiIndependent", V::Bool(true)));
        fields.push(("rectangleTopLeftCornerRadius", fl(r[0])));
        fields.push(("rectangleTopRightCornerRadius", fl(r[1])));
        fields.push(("rectangleBottomRightCornerRadius", fl(r[2])));
        fields.push(("rectangleBottomLeftCornerRadius", fl(r[3])));
    }
    if node.corner_smoothing != 0.0 {
        fields.push(("cornerSmoothing", fl(node.corner_smoothing)));
    }

    // Constraints.
    let cname = |c: Constraint| match c {
        Constraint::Min => "MIN",
        Constraint::Center => "CENTER",
        Constraint::Max => "MAX",
        Constraint::Stretch => "STRETCH",
        Constraint::Scale => "SCALE",
    };
    fields.push(("horizontalConstraint", en(cname(node.constraints.horizontal))));
    fields.push(("verticalConstraint", en(cname(node.constraints.vertical))));

    // Frame & Auto-Layout properties.
    let my_dir = node.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|a| a.direction);
    if let Some(fp) = node.kind.frame_props() {
        fields.push(("frameMaskDisabled", V::Bool(!fp.clips_content)));
        if !fp.grids.is_empty() {
            fields.push(("layoutGrids", V::Array(fp.grids.iter().map(encode_grid).collect())));
        }
        if let Some(al) = &fp.auto_layout {
            let sm = match al.direction {
                LayoutDirection::Horizontal => "HORIZONTAL",
                LayoutDirection::Vertical => "VERTICAL",
                LayoutDirection::Grid => "GRID",
            };
            fields.push(("stackMode", en(sm)));
            grid::push_frame_fields(cx, id, &node, al, &mut fields, &mut carried);
            if al.direction == LayoutDirection::Grid {
                fields.push(("gridColumnGap", fl(al.item_spacing)));
                fields.push(("gridRowGap", fl(al.counter_spacing)));
            }
            fields.push(("stackSpacing", fl(al.item_spacing)));
            fields.push(("stackCounterSpacing", fl(al.counter_spacing)));
            // Figma stores top and left padding on the vertical and horizontal padding fields.
            fields.push(("stackVerticalPadding", fl(al.padding[0])));
            fields.push(("stackPaddingRight", fl(al.padding[1])));
            fields.push(("stackPaddingBottom", fl(al.padding[2])));
            fields.push(("stackHorizontalPadding", fl(al.padding[3])));
            let align_name = |a: LayoutAlign| match a {
                LayoutAlign::Start => "MIN",
                LayoutAlign::Center => "CENTER",
                LayoutAlign::End => "MAX",
                LayoutAlign::SpaceBetween => "SPACE_BETWEEN",
                LayoutAlign::Baseline => "BASELINE",
                // never reached for the counter axis; the nearest StackAlign
                LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly => "SPACE_BETWEEN",
            };
            // StackJustify has no baseline and no distribution beyond the main axis.
            let justify_name = match al.primary_align {
                LayoutAlign::SpaceAround => "SPACE_AROUND",
                // the legacy SPACE_EVENLY (3) is space-between in real files: always write 6
                LayoutAlign::SpaceEvenly => "SPACE_EVENLY_CSS",
                LayoutAlign::Baseline => "MIN",
                other => align_name(other),
            };
            fields.push(("stackPrimaryAlignItems", en(justify_name)));
            fields.push(("stackCounterAlignItems", en(align_name(al.counter_align))));
            if al.counter_align_content == CounterAlignContent::SpaceBetween {
                fields.push(("stackCounterAlignContent", en("SPACE_BETWEEN")));
            }
            if al.layout_version > 0 {
                fields.push(("stackLayoutVersion", V::Uint(al.layout_version as u32)));
            }
            fields.push(("stackPrimarySizing", en(if al.primary_sizing == Sizing::Hug { "RESIZE_TO_FIT" } else { "FIXED" })));
            fields.push(("stackCounterSizing", en(if al.counter_sizing == Sizing::Hug { "RESIZE_TO_FIT_WITH_IMPLICIT_SIZE" } else { "FIXED" })));
            fields.push(("stackWrap", en(if al.wrap { "WRAP" } else { "NO_WRAP" })));
            fields.push(("bordersTakeSpace", V::Bool(al.stroke_included)));
            fields.push(("stackReverseZIndex", V::Bool(al.reverse_z)));
        } else if clears_inherited_auto_layout(&node) {
            // An instance without auto layout over a component that has it must say so, or an importer
            // would hand the component's layout back to it.
            fields.push(("stackMode", en("NONE")));
        }
    }

    // Child layout properties.
    let lc = &node.layout_child;
    let (prim_fill, counter_fill) = match parent_dir {
        Some(LayoutDirection::Horizontal | LayoutDirection::Grid) => (lc.horizontal == Sizing::Fill || lc.grow > 0.0, lc.vertical == Sizing::Fill),
        Some(LayoutDirection::Vertical) => (lc.vertical == Sizing::Fill || lc.grow > 0.0, lc.horizontal == Sizing::Fill),
        None => (lc.grow > 0.0, false),
    };
    if prim_fill {
        fields.push(("stackChildPrimaryGrow", fl(if lc.grow > 0.0 { lc.grow } else { 1.0 })));
    }
    let align_self_name = match lc.align_self {
        AlignSelf::Start => Some("MIN"),
        AlignSelf::Center => Some("CENTER"),
        AlignSelf::End => Some("MAX"),
        AlignSelf::Stretch => Some("STRETCH"),
        AlignSelf::Baseline => Some("BASELINE"),
        AlignSelf::Auto => counter_fill.then_some("STRETCH"),
    };
    if let Some(a) = align_self_name {
        fields.push(("stackChildAlignSelf", en(a)));
    }
    if lc.absolute {
        fields.push(("stackPositioning", en("ABSOLUTE")));
    }
    for (val, key) in [
        (lc.margin[0], "stackChildMarginTop"),
        (lc.margin[1], "stackChildMarginRight"),
        (lc.margin[2], "stackChildMarginBottom"),
        (lc.margin[3], "stackChildMarginLeft"),
    ] {
        if val != 0.0 {
            fields.push((key, fl(val)));
        }
    }
    grid::push_child_fields(grid::parent_ids(cx, &node), lc, &mut fields, &mut carried);
    for (k, a) in [("gridChildHorizontalAlign", lc.grid_align_h), ("gridChildVerticalAlign", lc.grid_align_v)] {
        let eff = if a == GridChildAlign::Start { node.fig_metadata.get(k).and_then(|s| GridChildAlign::from_figma_name(s)).unwrap_or(a) } else { a };
        if writes_grid_child_align(eff, parent_dir) {
            fields.push((k, en(grid_child_align_name(eff))));
        }
    }
    // Figma writes both components of a size limit: an unset minimum is 0 and an unset maximum is infinite.
    if lc.min_w.is_some() || lc.min_h.is_some() {
        fields.push(("minSize", size_limit(lc.min_w, lc.min_h, 0.0)));
    }
    if lc.max_w.is_some() || lc.max_h.is_some() {
        fields.push(("maxSize", size_limit(lc.max_w, lc.max_h, f32::INFINITY)));
    }

    // Export settings & frame guides.
    if !node.export_settings.is_empty() {
        let exs: Vec<V> = node
            .export_settings
            .iter()
            .map(|e| {
                let img_ty = match e.format {
                    ExportFileFormat::Png => "PNG",
                    ExportFileFormat::Jpg => "JPEG",
                    ExportFileFormat::Svg => "SVG",
                    ExportFileFormat::Pdf => "PDF",
                };
                let (cty, cval) = match e.constraint {
                    ExportConstraint::Scale(v) => ("CONTENT_SCALE", v),
                    ExportConstraint::Width(v) => ("CONTENT_WIDTH", v),
                    ExportConstraint::Height(v) => ("CONTENT_HEIGHT", v),
                };
                msg(vec![("suffix", V::String(e.suffix.clone())), ("imageType", en(img_ty)), ("constraint", st(vec![("type", en(cty)), ("value", fl(cval))]))])
            })
            .collect();
        fields.push(("exportSettings", V::Array(exs)));
    }
    if let Some(page) = cx.doc.page_of(id) {
        if let Some(guides) = cx.doc.guides.get(&page) {
            let fg: Vec<V> = guides
                .iter()
                .filter(|g| g.frame == Some(id))
                .map(|g| msg(vec![("axis", en(if g.axis == GuideAxis::X { "X" } else { "Y" })), ("offset", fl(g.offset))]))
                .collect();
            if !fg.is_empty() {
                fields.push(("guides", V::Array(fg)));
            }
        }
    }

    // Style references.
    for (k, field_name) in
        [("fill", "styleIdForFill"), ("stroke", "styleIdForStrokeFill"), ("text", "styleIdForText"), ("effect", "styleIdForEffect"), ("grid", "styleIdForGrid")]
    {
        if let Some(sid) = node.style_refs.get(k) {
            if k == "effect" && inherits_effect_link_with_own_cache(cx, &node, sid) {
                continue;
            }
            if let Some(&sg) = cx.style_guids.get(sid) {
                fields.push((field_name, msg(vec![("guid", guid(sg.0, sg.1))])));
            } else if let Some(key) = sid.strip_prefix("key:") {
                fields.push((field_name, msg(vec![("assetRef", msg(vec![("key", V::String(key.to_string()))]))])));
            }
        }
    }

    // Node variable bindings & explicit modes.
    let (pcm_var_entries, mut pcm_var_fields) =
        crate::motion::reconstruct_pcm_variable_entries(node.fig_metadata.get("_pcmVarFields").map(String::as_str), &node.variables.bindings, |vid| {
            cx.resolve_var_guid(vid)
        });
    if let Some(raw_pcm) = node.fig_metadata.get("parameterConsumptionMap") {
        if let Ok(pcm_val) = serde_json::from_str::<serde_json::Value>(raw_pcm) {
            if let Some(entries) = pcm_val.get("entries").and_then(serde_json::Value::as_array) {
                for e in entries {
                    let dt = e.get("variableData").and_then(|d| d.get("dataType")).and_then(serde_json::Value::as_str);
                    if matches!(dt, Some("KEYFRAME_TRACK_PARAMETER_DATA" | "KEYFRAME_TRACK_ID")) {
                        continue;
                    }
                    if let Some(vf) = e.get("variableField").and_then(serde_json::Value::as_str) {
                        pcm_var_fields.insert(vf.to_string());
                        match vf {
                            "STACK_HORIZONTAL_PADDING" => {
                                pcm_var_fields.insert("STACK_PADDING_LEFT".into());
                                pcm_var_fields.insert("STACK_PADDING_RIGHT".into());
                            }
                            "STACK_VERTICAL_PADDING" => {
                                pcm_var_fields.insert("STACK_PADDING_TOP".into());
                                pcm_var_fields.insert("STACK_PADDING_BOTTOM".into());
                            }
                            "RECTANGLE_CORNER_RADII" => {
                                pcm_var_fields.insert("RECTANGLE_TOP_LEFT_CORNER_RADIUS".into());
                                pcm_var_fields.insert("RECTANGLE_TOP_RIGHT_CORNER_RADIUS".into());
                                pcm_var_fields.insert("RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS".into());
                                pcm_var_fields.insert("RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS".into());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    let var_entries = encode_node_var_entries_skipping(cx, &node.variables, &pcm_var_fields);
    if !var_entries.is_empty() {
        fields.push(("variableConsumptionMap", msg(vec![("entries", V::Array(var_entries))])));
    }
    let mode_entries = encode_node_mode_entries(cx, &node.variables);
    if !mode_entries.is_empty() {
        fields.push(("variableModeBySetMap", msg(vec![("entries", V::Array(mode_entries))])));
    }

    // Component property definitions, assignments, and references.
    if !node.component.props.is_empty() {
        let mut prop_defs = Vec::new();
        let mut variant_orders = Vec::new();
        for p in &node.component.props {
            let def_g = cx.resolve_prop_guid(id, &p.name);
            let bare_name = p.name.split('#').next().unwrap_or(&p.name).to_string();
            let mut df = vec![("id", guid(def_g.0, def_g.1)), ("name", V::String(bare_name.clone()))];
            match &p.kind {
                PropKind::Boolean => {
                    df.push(("type", en("BOOL")));
                    let b = match &p.default {
                        PropValue::Bool(b) => *b,
                        _ => true,
                    };
                    df.push(("initialValue", msg(vec![("boolValue", V::Bool(b))])));
                }
                PropKind::Text => {
                    df.push(("type", en("TEXT")));
                    let s = match &p.default {
                        PropValue::Text(s) => s.clone(),
                        _ => String::new(),
                    };
                    df.push(("initialValue", msg(vec![("textValue", msg(vec![("characters", V::String(s))]))])));
                }
                PropKind::InstanceSwap { preferred } => {
                    df.push(("type", en("INSTANCE_SWAP")));
                    let tg = match &p.default {
                        PropValue::Instance(tid) => cx.node_guids.get(tid).copied().unwrap_or((0, 0)),
                        _ => (0, 0),
                    };
                    df.push(("initialValue", msg(vec![("guidValue", guid(tg.0, tg.1))])));
                    if let Some(pv) = encode_preferred_values(cx, preferred) {
                        df.push(("preferredValues", pv));
                    }
                }
                PropKind::Variant { options } => {
                    df.push(("type", en("VARIANT")));
                    let s = match &p.default {
                        PropValue::Variant(s) | PropValue::Text(s) => s.clone(),
                        _ => options.first().cloned().unwrap_or_default(),
                    };
                    df.push(("initialValue", msg(vec![("textValue", msg(vec![("characters", V::String(s))]))])));
                    variant_orders.push(msg(vec![("property", V::String(bare_name)), ("values", V::Array(options.iter().cloned().map(V::String).collect()))]));
                }
                PropKind::Slot => {
                    df.push(("type", en("SLOT")));
                    df.push(("initialValue", msg(vec![("textValue", msg(vec![("characters", V::String(String::new()))]))])));
                    if let Some(cfg) = node.component.slot_configs.get(&p.name) {
                        if cfg.has_settings() {
                            df.push(("slotPropConfig", encode_slot_prop_config(cfg)));
                        }
                        if let Some(pv) = encode_preferred_values(cx, &cfg.preferred) {
                            df.push(("preferredValues", pv));
                        }
                    }
                    if let Some(g) = def_slot_content_id(&node.component, &p.name).and_then(parse_guid_pair) {
                        df.push((
                            "varValue",
                            msg(vec![("dataType", en("SLOT_CONTENT_ID")), ("value", msg(vec![("slotContentIdValue", msg(vec![("guid", guid(g.0, g.1))]))]))]),
                        ));
                    }
                }
            }
            if !matches!(p.kind, PropKind::Slot) {
                if let Some(alias) = prop_def_alias(cx, &node, p) {
                    df.push(("varValue", alias));
                }
            }
            prop_defs.push(msg(df));
        }
        fields.push(("componentPropDefs", V::Array(prop_defs)));
        if !variant_orders.is_empty() {
            fields.push(("stateGroupPropertyValueOrders", V::Array(variant_orders)));
        }
    }
    if !node.component.values.is_empty() || has_prop_binding(&node) || (!node.component.slot_content_ids.is_empty() && node.component.props.is_empty()) {
        let mut prop_assigns = Vec::new();
        for (prop_name, pval) in &node.component.values {
            let def_g = cx.resolve_prop_guid(id, prop_name);
            let val_msg = match pval {
                PropValue::Bool(b) => msg(vec![("boolValue", V::Bool(*b))]),
                PropValue::Text(s) | PropValue::Variant(s) => msg(vec![("textValue", msg(vec![("characters", V::String(s.clone()))]))]),
                PropValue::Instance(tid) => {
                    let tg = cx.node_guids.get(tid).copied().unwrap_or((0, 0));
                    msg(vec![("guidValue", guid(tg.0, tg.1))])
                }
                PropValue::Slot(ids) => {
                    let s = ids.iter().filter_map(|i| cx.node_guids.get(i).map(|g| format!("{}:{}", g.0, g.1))).collect::<Vec<_>>().join(",");
                    msg(vec![("textValue", msg(vec![("characters", V::String(s))]))])
                }
            };
            let mut assign = vec![("defID", guid(def_g.0, def_g.1)), ("value", val_msg)];
            if let Some(g) = node.component.slot_content_ids.get(prop_name).and_then(|s| parse_guid_pair(s)) {
                assign.push((
                    "varValue",
                    msg(vec![("dataType", en("SLOT_CONTENT_ID")), ("value", msg(vec![("slotContentIdValue", msg(vec![("guid", guid(g.0, g.1))]))]))]),
                ));
            } else if let Some(alias) = prop_alias(cx, &node, prop_name) {
                assign.push(("varValue", alias));
            }
            prop_assigns.push(msg(assign));
        }
        // A bound property whose value the instance does not store still names its variable.
        for (prop_name, _) in prop_bindings_of(&node) {
            if node.component.values.contains_key(prop_name) {
                continue;
            }
            let Some(alias) = prop_alias(cx, &node, prop_name) else { continue };
            let def_g = cx.resolve_prop_guid(id, prop_name);
            prop_assigns.push(msg(vec![("defID", guid(def_g.0, def_g.1)), ("varValue", alias)]));
        }
        if node.component.props.is_empty() {
            for (prop_name, scid) in &node.component.slot_content_ids {
                if node.component.values.contains_key(prop_name) {
                    continue;
                }
                let Some(g) = parse_guid_pair(scid) else { continue };
                let def_g = cx.resolve_prop_guid(id, prop_name);
                prop_assigns.push(msg(vec![
                    ("defID", guid(def_g.0, def_g.1)),
                    (
                        "varValue",
                        msg(vec![("dataType", en("SLOT_CONTENT_ID")), ("value", msg(vec![("slotContentIdValue", msg(vec![("guid", guid(g.0, g.1))]))]))]),
                    ),
                ]));
            }
        }
        if !prop_assigns.is_empty() {
            fields.push(("componentPropAssignments", V::Array(prop_assigns)));
        }
    }
    if !node.component.bindings.is_empty() {
        let mut prop_refs = Vec::new();
        for (&field, prop_name) in &node.component.bindings {
            let Some(node_field) = (match field {
                OverrideField::Visible => Some("VISIBLE"),
                OverrideField::Characters => Some("TEXT_DATA"),
                OverrideField::MainComponent => Some("OVERRIDDEN_SYMBOL_ID"),
                OverrideField::SlotContent => Some("SLOT_CONTENT_ID"),
                _ => None,
            }) else {
                continue;
            };
            let def_g = cx.resolve_prop_guid(id, prop_name);
            prop_refs.push(msg(vec![("defID", guid(def_g.0, def_g.1)), ("componentPropNodeField", en(node_field))]));
        }
        if !prop_refs.is_empty() {
            fields.push(("componentPropRefs", V::Array(prop_refs)));
        }
    }
    if !node.component.variant_props.is_empty() {
        let mut vspecs = Vec::new();
        // Figma's spec holds the property's id and the value; the name is found through the id. A property with no
        // definition in this file has no name to find, so the specs travel once more in the legacy shape with it.
        let mut legacy = Vec::new();
        let mut name_lost = false;
        for (prop_name, val) in &node.component.variant_props {
            let def_g = cx.resolve_prop_guid(id, prop_name);
            let bare_name =
                prop_name.strip_prefix("propDefId:").map(|_| String::new()).unwrap_or_else(|| prop_name.split('#').next().unwrap_or(prop_name).to_string());
            vspecs.push(msg(vec![("propDefId", guid(def_g.0, def_g.1)), ("value", V::String(val.clone()))]));
            let mut spec = serde_json::json!({"propDefId": {"sessionID": def_g.0, "localID": def_g.1}, "value": val});
            if !bare_name.is_empty() {
                name_lost |= !cx.prop_guids.values().any(|g| *g == def_g);
                spec["propertyName"] = bare_name.into();
            }
            legacy.push(spec);
        }
        if name_lost {
            carried.push(("variantPropSpecs", serde_json::Value::Array(legacy)));
        }
        fields.push(("variantPropSpecs", V::Array(vspecs)));
    }

    // Geometry blobs.
    if !matches!(node.kind, NodeKind::Text(_)) {
        let fill_paths = node.fill_geometry.as_ref().or(match &node.kind {
            NodeKind::Vector { paths, .. } if !paths.is_empty() && node.stroke_geometry.as_ref() != Some(paths) => Some(paths),
            _ => None,
        });
        if let Some(paths) = fill_paths {
            let fgs = encode_geometry_blobs(cx, paths);
            if !fgs.is_empty() {
                fields.push(("fillGeometry", V::Array(fgs)));
            }
        }
        if let Some(paths) = &node.stroke_geometry {
            let sgs = encode_geometry_blobs(cx, paths);
            if !sgs.is_empty() {
                fields.push(("strokeGeometry", V::Array(sgs)));
            }
        }
    }

    // Kind-specific fields.
    match &node.kind {
        NodeKind::Ellipse(arc) => {
            fields.push(("arcData", msg(vec![("startingAngle", fl(arc.start)), ("endingAngle", fl(arc.end)), ("innerRadius", fl(arc.inner_radius))])));
        }
        NodeKind::Polygon { count } => {
            fields.push(("count", V::Uint(*count)));
        }
        NodeKind::Star { count, inner_ratio } => {
            fields.push(("count", V::Uint(*count)));
            fields.push(("starInnerScale", fl(*inner_ratio)));
        }
        NodeKind::BooleanOp(op) => {
            let op_name = match op {
                BooleanOp::Union => "UNION",
                BooleanOp::Subtract => "SUBTRACT",
                BooleanOp::Intersect => "INTERSECT",
                BooleanOp::Exclude => "XOR",
            };
            fields.push(("booleanOperation", en(op_name)));
        }
        NodeKind::Component(_) | NodeKind::ComponentSet(_) => {
            if let Some(ck) = cx.comp_keys.get(&id) {
                fields.push(("componentKey", V::String(ck.clone())));
            }
        }
        NodeKind::Instance { component, component_key, .. } => {
            let mut sd = Vec::new();
            if let Some(comp_id) = component.and_then(|c| cx.node_guids.get(&c).copied()) {
                sd.push(("symbolID", guid(comp_id.0, comp_id.1)));
            }
            // An instance whose page is not loaded has no children to read overrides from: the entries the file stated
            // for it are written as they were. Any other instance is read from its children.
            let kept = component.and_then(|main| kept_overrides::pending_symbol_overrides(cx, id, main));
            let sym_overrides = kept.unwrap_or_else(|| collect_instance_overrides(cx, id));
            if !sym_overrides.is_empty() {
                sd.push(("symbolOverrides", V::Array(sym_overrides)));
            }
            if !sd.is_empty() {
                fields.push(("symbolData", msg(sd)));
            }
            // Likewise the derived data (sizes, transforms, outlines) the file stated for an instance on a page that is
            // not loaded, while its main component is as it was when the file was opened.
            if let Some(derived) = component.and_then(|main| kept_derived::pending_derived_symbol_data(cx, id, main)) {
                fields.push(("derivedSymbolData", V::Array(derived)));
            }
            if let Some(k) = component_key {
                fields.push(("componentKey", V::String(k.clone())));
            }
        }
        NodeKind::Vector { network: Some(net), .. } if !net.vertices.is_empty() => {
            let (blob_bytes, style_table) = encode_vector_network_blob(net);
            let blob_idx = cx.add_blob(blob_bytes);
            let mut vd = vec![("vectorNetworkBlob", V::Uint(blob_idx)), ("normalizedSize", vec2(node.size.x, node.size.y))];
            if !style_table.is_empty() {
                vd.push(("styleOverrideTable", V::Array(style_table)));
            }
            fields.push(("vectorData", msg(vd)));
        }
        NodeKind::Text(tp) => {
            let link = node.style_refs.contains_key("text").then_some(node.text_style_overrides);
            encode_text_node(&mut fields, tp, link);
        }
        NodeKind::Sticky(sp) => {
            encode_text_node(&mut fields, &sp.text, None);
        }
        NodeKind::ShapeWithText(sp) => {
            let st_name = node.fig_metadata.get("shapeWithTextType").map(String::as_str).filter(|s| !s.is_empty()).unwrap_or(match sp.shape {
                ShapeKind::Square => "SQUARE",
                ShapeKind::RoundedRect => "ROUNDED_RECTANGLE",
                ShapeKind::Ellipse => "ELLIPSE",
                ShapeKind::Diamond => "DIAMOND",
                ShapeKind::Triangle => "TRIANGLE_UP",
                ShapeKind::TriangleDown => "TRIANGLE_DOWN",
                ShapeKind::Parallelogram => "PARALLELOGRAM_RIGHT",
                ShapeKind::Star => "STAR",
                ShapeKind::Hexagon => "HEXAGON",
                ShapeKind::Pentagon => "PENTAGON",
            });
            let st_name = if cx.schema.has_enum_variant("ShapeWithTextType", st_name) { st_name } else { "SQUARE" };
            fields.push(("shapeWithTextType", en(st_name)));
            encode_text_node(&mut fields, &sp.text, None);
        }
        NodeKind::Connector(cp) => {
            if let Some((lc, carrier_val)) = &lowered_conn {
                carried.push(("studioConnector", carrier_val.clone()));
                if lc.label_chip.is_none() {
                    let sgs = encode_geometry_blobs(cx, std::slice::from_ref(&lc.local_route.path()));
                    if !sgs.is_empty() {
                        fields.push(("strokeGeometry", V::Array(sgs)));
                    }
                    let (blob_bytes, style_table) =
                        encode_vector_network_blob(&crate::export::network_with_node_cap_on_open_ends(&lc.network, node.stroke.cap));
                    let blob_idx = cx.add_blob(blob_bytes);
                    let mut vd = vec![("vectorNetworkBlob", V::Uint(blob_idx)), ("normalizedSize", vec2(node.size.x, node.size.y))];
                    if !style_table.is_empty() {
                        vd.push(("styleOverrideTable", V::Array(style_table)));
                    }
                    fields.push(("vectorData", msg(vd)));
                }
            } else {
                let line_style = match cp.line {
                    ConnectorLine::Elbowed => "ELBOWED",
                    ConnectorLine::Straight => "STRAIGHT",
                    ConnectorLine::Curved => "CURVED",
                };
                fields.push(("connectorLineStyle", en(line_style)));
                let cap_name = |c: ConnectorCap| match c {
                    ConnectorCap::None => "NONE",
                    ConnectorCap::Arrow => "ARROW_LINES",
                    ConnectorCap::Triangle => "ARROW_EQUILATERAL",
                    ConnectorCap::Circle => "CIRCLE_FILLED",
                    ConnectorCap::Diamond => "DIAMOND_FILLED",
                };
                fields.push(("connectorStartCap", en(cap_name(cp.start_cap))));
                fields.push(("connectorEndCap", en(cap_name(cp.end_cap))));
                let mag_name = |m: Magnet| match m {
                    Magnet::Auto => "AUTO",
                    Magnet::Top => "TOP",
                    Magnet::Right => "RIGHT",
                    Magnet::Bottom => "BOTTOM",
                    Magnet::Left => "LEFT",
                    Magnet::Center => "CENTER",
                };
                let end_msg = |ep: &ConnectorEnd, mag_meta_key: &str, rp_meta_key: &str| {
                    let mag = if ep.magnet == Magnet::Auto {
                        node.fig_metadata.get(mag_meta_key).map(String::as_str).filter(|s| cx.schema.has_enum_variant("ConnectorMagnet", s)).unwrap_or("AUTO")
                    } else {
                        mag_name(ep.magnet)
                    };
                    let mut ef = vec![("position", vec2(ep.point.x, ep.point.y)), ("magnet", en(mag))];
                    if let Some(tg) = ep.node.and_then(|nid| cx.node_guids.get(&nid).copied()) {
                        ef.push(("endpointNodeID", guid(tg.0, tg.1)));
                    }
                    if let Some(rp_str) = node.fig_metadata.get(rp_meta_key) {
                        if let Ok(rp_val) = serde_json::from_str::<serde_json::Value>(rp_str) {
                            if let (Some(rx), Some(ry)) =
                                (rp_val.get("x").and_then(serde_json::Value::as_f64), rp_val.get("y").and_then(serde_json::Value::as_f64))
                            {
                                ef.push(("relativePosition", vec2(rx as f32, ry as f32)));
                            }
                        }
                    }
                    msg(ef)
                };
                fields.push(("connectorStart", end_msg(&cp.start, "connectorStartMagnet", "connectorStartRelativePosition")));
                fields.push(("connectorEnd", end_msg(&cp.end, "connectorEndMagnet", "connectorEndRelativePosition")));
                if let Some(lbl) = &cp.label {
                    encode_text_node(&mut fields, lbl, None);
                }
            }
        }
        _ => {}
    }

    // The exported `strokeCap` is the modelled one. An arrow cap the stroke model has no name for rides along, so
    // that the file read back still knows the baked outline carries it.
    if let (NodeKind::Vector { .. }, Some(cap)) = (&node.kind, node.fig_metadata.get("strokeCap")) {
        if !matches!(cap.as_str(), "" | "NONE" | "ROUND" | "SQUARE") {
            carried.push(("figMetadata", json!({"strokeCap": cap})));
        }
    }
    // The bound variant properties a person picked by hand on a top-level instance. A nested copy's ride in the entry that
    // overrides it (see `collect_instance_overrides_for`).
    if matches!(node.kind, NodeKind::Instance { .. }) && is_top_instance(&node) && !node.component.variant_picks.is_empty() {
        carried.push(("variantPicks", json!(node.component.variant_picks)));
    }
    pd_entries.extend(carrier::carrier_entries(&carried));
    if !pd_entries.is_empty() {
        fields.push(("pluginData", V::Array(pd_entries)));
    }
    let mut msg_fields: Vec<(std::sync::Arc<str>, V)> = fields.into_iter().map(|(k, v)| (k.into(), v)).collect();
    let track_meta_map: serde_json::Map<String, serde_json::Value> = node
        .fig_metadata
        .get("_motionTrackMeta")
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if let Some(motion) = node.motion.as_deref() {
        if !motion.timelines.is_empty() {
            let mut tl_json = crate::motion::timelines_to_kiwi_with(&motion.timelines, |tid| cx.timeline_guid_or_alloc(tid));
            if let Some(existing_tl) = node.fig_metadata.get("timelineDefinitions") {
                if let Ok(existing_val) = serde_json::from_str::<serde_json::Value>(existing_tl) {
                    if let (Some(dst), Some(src)) = (
                        tl_json.get_mut("entries").and_then(serde_json::Value::as_array_mut),
                        existing_val.get("entries").and_then(serde_json::Value::as_array),
                    ) {
                        dst.extend(src.iter().cloned());
                    }
                }
            }
            if let Some(pair) = cx.schema.def_field_from_json(cx.nc_def, "timelineDefinitions", &tl_json) {
                msg_fields.push(pair);
            }
        }
        if !motion.assignments.is_empty() {
            let mut as_json = crate::motion::assignments_to_kiwi_with(&motion.assignments, |tid| cx.timeline_guid_or_alloc(tid));
            if let Some(existing_as) = node.fig_metadata.get("timelineAssignments") {
                if let Ok(existing_val) = serde_json::from_str::<serde_json::Value>(existing_as) {
                    if let (Some(dst), Some(src)) = (
                        as_json.get_mut("entries").and_then(serde_json::Value::as_array_mut),
                        existing_val.get("entries").and_then(serde_json::Value::as_array),
                    ) {
                        dst.extend(src.iter().cloned());
                    }
                }
            }
            if let Some(pair) = cx.schema.def_field_from_json(cx.nc_def, "timelineAssignments", &as_json) {
                msg_fields.push(pair);
            }
        }
    }
    if let Some(sid) = node.style_refs.get("animation") {
        let anim_style_ref = if let Some(&sg) = cx.style_guids.get(sid) {
            Some(serde_json::json!({"guid": crate::motion::guid_pair_json(sg)}))
        } else {
            sid.strip_prefix("key:").map(|k| serde_json::json!({"assetRef": {"key": k}}))
        };
        if let Some(sr) = anim_style_ref {
            let existing_arr =
                node.fig_metadata.get("styleIdsForAnimation").and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok()).and_then(|v| match v {
                    serde_json::Value::Array(arr) if !arr.is_empty() => Some(arr),
                    _ => None,
                });
            let val = if let Some(mut arr) = existing_arr {
                if let Some(first) = arr.first_mut().and_then(serde_json::Value::as_object_mut) {
                    first.insert("animationStyleId".into(), sr);
                }
                serde_json::Value::Array(arr)
            } else {
                let mut entry = serde_json::Map::new();
                entry.insert("id".into(), crate::motion::guid_pair_json(cx.alloc_guid(4)));
                entry.insert("animationStyleId".into(), sr);
                let first_tl = node.motion.as_deref().and_then(|m| m.timelines.first()).or_else(|| match cx.doc.styles.get(sid) {
                    Some(Style::Animation(tls)) => tls.first(),
                    _ => None,
                });
                if let Some(tl) = first_tl {
                    let tg = cx.timeline_guid_or_alloc(&tl.id);
                    entry.insert("timelineDefId".into(), crate::motion::guid_pair_json(tg));
                }
                let offset_us = node.motion.as_deref().and_then(|m| m.assignments.first().map(|a| a.offset_us)).unwrap_or(0);
                entry.insert("timelineOffset".into(), serde_json::json!(offset_us));
                entry.insert("sortPosition".into(), serde_json::json!("!"));
                serde_json::Value::Array(vec![serde_json::Value::Object(entry)])
            };
            if let Some(pair) = cx.schema.def_field_from_json(cx.nc_def, "styleIdsForAnimation", &val) {
                msg_fields.push(pair);
            }
        }
    }
    let mut pcm_entries = pcm_var_entries;
    if let Some(motion) = node.motion.as_deref() {
        if !motion.tracks.is_empty() {
            pcm_entries.extend(crate::motion::tracks_to_parameter_consumption_entries_with_shape(
                &motion.tracks,
                |t_idx, _tr| cx.track_guid(id, t_idx),
                |tid| cx.timeline_guid(tid),
                |t_idx, _prop| {
                    track_meta_map
                        .get(&t_idx.to_string())
                        .and_then(|m| m.get("_nodeField"))
                        .and_then(serde_json::Value::as_str)
                        .and_then(|s| s.parse::<u64>().ok())
                },
                |t_idx| track_meta_map.get(&t_idx.to_string()).and_then(|m| m.get("_trackIdShape")).and_then(serde_json::Value::as_str) == Some("true"),
            ));
        }
    }
    if let Some(existing_pcm) = node.fig_metadata.get("parameterConsumptionMap") {
        if let Ok(existing_val) = serde_json::from_str::<serde_json::Value>(existing_pcm) {
            if let Some(arr) = existing_val.get("entries").and_then(serde_json::Value::as_array) {
                pcm_entries.extend(arr.iter().cloned());
            }
        }
    }
    if !pcm_entries.is_empty() {
        let pcm_json = json!({"entries": pcm_entries});
        if let Some(pair) = cx.schema.def_field_from_json(cx.nc_def, "parameterConsumptionMap", &pcm_json) {
            msg_fields.push(pair);
        }
    }
    append_fig_metadata(cx.schema, cx.nc_def, &mut msg_fields, &node.fig_metadata);
    cx.changes.push(V::Message(msg_fields));

    if let Some((lc, carrier_val)) = lowered_conn {
        if let Some((bg_rect, radius, text_rect, lbl)) = lc.label_chip {
            let v_guid = cx.alloc_guid(1);
            let sps: Vec<V> =
                node.stroke.paints.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "strokes", i, &node.variables.bindings)).collect();
            let mut v_fields = vec![
                ("guid", guid(v_guid.0, v_guid.1)),
                ("phase", en("CREATED")),
                ("parentIndex", st(vec![("guid", guid(nguid.0, nguid.1)), ("position", V::String(pos_str(0)))])),
                ("type", en("VECTOR")),
                ("name", V::String(node.name.clone())),
                ("visible", V::Bool(true)),
                ("locked", V::Bool(false)),
                ("opacity", fl_one(1.0)),
                ("size", vec2(node.size.x, node.size.y)),
                ("transform", encode_matrix(&Affine::IDENTITY)),
                ("blendMode", en("NORMAL")),
                ("fillPaints", V::Array(Vec::new())),
                ("strokePaints", V::Array(sps)),
                ("strokeWeight", fl(node.stroke.weight)),
                ("strokeAlign", en("CENTER")),
                (
                    "strokeJoin",
                    en(match node.stroke.join {
                        StrokeJoin::Miter => "MITER",
                        StrokeJoin::Bevel => "BEVEL",
                        StrokeJoin::Round => "ROUND",
                    }),
                ),
                ("strokeCap", en(crate::vecnet::cap_name(node.stroke.cap))),
            ];
            if !node.stroke.dashes.is_empty() {
                v_fields.push(("dashPattern", V::Array(node.stroke.dashes.iter().map(|&d| fl(d)).collect())));
            }
            let sgs = encode_geometry_blobs(cx, std::slice::from_ref(&lc.local_route.path()));
            if !sgs.is_empty() {
                v_fields.push(("strokeGeometry", V::Array(sgs)));
            }
            let (blob_bytes, style_table) = encode_vector_network_blob(&crate::export::network_with_node_cap_on_open_ends(&lc.network, node.stroke.cap));
            let blob_idx = cx.add_blob(blob_bytes);
            let mut vd = vec![("vectorNetworkBlob", V::Uint(blob_idx)), ("normalizedSize", vec2(node.size.x, node.size.y))];
            if !style_table.is_empty() {
                vd.push(("styleOverrideTable", V::Array(style_table)));
            }
            v_fields.push(("vectorData", msg(vd)));
            v_fields.push(("pluginData", V::Array(carrier::carrier_entries(&[("studioConnector", carrier_val)]))));
            cx.changes.push(msg(v_fields));

            let r_guid = cx.alloc_guid(1);
            let chip_fill = encode_paint_with_bindings(cx, &Paint::solid(Color::WHITE), "fills", 0, &BTreeMap::new());
            let r_fields = vec![
                ("guid", guid(r_guid.0, r_guid.1)),
                ("phase", en("CREATED")),
                ("parentIndex", st(vec![("guid", guid(nguid.0, nguid.1)), ("position", V::String(pos_str(1)))])),
                ("type", en("ROUNDED_RECTANGLE")),
                ("name", V::String("Label background".into())),
                ("visible", V::Bool(true)),
                ("locked", V::Bool(false)),
                ("opacity", fl_one(1.0)),
                ("size", vec2(bg_rect.w, bg_rect.h)),
                ("transform", encode_matrix(&Affine::translate(bg_rect.x, bg_rect.y))),
                ("blendMode", en("NORMAL")),
                ("cornerRadius", fl(radius)),
                ("fillPaints", V::Array(vec![chip_fill])),
                ("strokePaints", V::Array(Vec::new())),
                ("strokeWeight", fl(0.0)),
                ("strokeAlign", en("INSIDE")),
            ];
            cx.changes.push(msg(r_fields));

            let t_guid = cx.alloc_guid(1);
            let t_fills: Vec<V> = lbl.style.fills.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "fills", i, &BTreeMap::new())).collect();
            let mut t_fields = vec![
                ("guid", guid(t_guid.0, t_guid.1)),
                ("phase", en("CREATED")),
                ("parentIndex", st(vec![("guid", guid(nguid.0, nguid.1)), ("position", V::String(pos_str(2)))])),
                ("type", en("TEXT")),
                ("name", V::String(lbl.content.clone())),
                ("visible", V::Bool(true)),
                ("locked", V::Bool(false)),
                ("opacity", fl_one(1.0)),
                ("size", vec2(text_rect.w, text_rect.h)),
                ("transform", encode_matrix(&Affine::translate(text_rect.x, text_rect.y))),
                ("blendMode", en("NORMAL")),
                ("fillPaints", V::Array(t_fills)),
                ("strokePaints", V::Array(Vec::new())),
                ("strokeWeight", fl(0.0)),
                ("strokeAlign", en("INSIDE")),
            ];
            encode_text_node(&mut t_fields, &lbl, None);
            cx.changes.push(msg(t_fields));
        }
    }

    let child_in_comp = in_component || matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_));
    let child_count = node.children.len();
    for (cidx, cid) in node.children.into_iter().enumerate() {
        export_node_rec(cx, cid, nguid, cidx, my_dir, child_in_comp);
    }
    if let Some(motion) = node.motion.as_deref() {
        for (t_idx, track) in motion.tracks.iter().enumerate() {
            if track.property.kiwi_name().is_empty() {
                continue;
            }
            let t_guid = cx.track_guid(id, t_idx);
            if !cx.emitted_track_guids.insert(t_guid) {
                continue;
            }
            let meta_obj = track_meta_map.get(&t_idx.to_string()).and_then(serde_json::Value::as_object);
            let track_name = meta_obj.and_then(|m| m.get("_name")).and_then(serde_json::Value::as_str).unwrap_or("Track");
            let mut t_json = serde_json::Map::new();
            crate::motion::track_modes_to_kiwi(track.operation, track.loop_mode, &mut t_json);
            let mut t_msg_fields: Vec<(std::sync::Arc<str>, V)> = vec![
                ("guid".into(), guid(t_guid.0, t_guid.1)),
                ("phase".into(), en("CREATED")),
                ("parentIndex".into(), st(vec![("guid", guid(nguid.0, nguid.1)), ("position", V::String(pos_str(child_count + t_idx)))])),
                ("type".into(), en("KEYFRAME_TRACK")),
                ("name".into(), V::String(track_name.to_string())),
                ("visible".into(), V::Bool(true)),
                ("opacity".into(), fl_one(1.0)),
                ("size".into(), vec2(0.0, 0.0)),
                ("transform".into(), encode_matrix(&Affine::IDENTITY)),
            ];
            for (k, v) in &t_json {
                if let Some(pair) = cx.schema.def_field_from_json(cx.nc_def, k, v) {
                    t_msg_fields.push(pair);
                }
            }
            if let Some(meta_obj) = meta_obj {
                let meta_btree: BTreeMap<String, String> = meta_obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect();
                append_fig_metadata(cx.schema, cx.nc_def, &mut t_msg_fields, &meta_btree);
            }
            cx.changes.push(V::Message(t_msg_fields));

            let kf_metas: Vec<serde_json::Value> = meta_obj
                .and_then(|m| m.get("_keyframes"))
                .and_then(serde_json::Value::as_str)
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();

            for (k_idx, kf) in track.keyframes.iter().enumerate() {
                let kf_meta_obj = kf_metas.get(k_idx).and_then(serde_json::Value::as_object);
                let k_guid = kf_meta_obj
                    .and_then(|m| m.get("_guid"))
                    .and_then(serde_json::Value::as_str)
                    .and_then(parse_guid_pair)
                    .filter(|g| cx.reserved_kf_guids.remove(g))
                    .unwrap_or_else(|| cx.alloc_guid(t_guid.0.max(1)));
                let kf_name = kf_meta_obj.and_then(|m| m.get("_name")).and_then(serde_json::Value::as_str).unwrap_or("Keyframe");
                let mut k_json = serde_json::Map::new();
                crate::motion::keyframe_to_kiwi(kf, &mut k_json);
                let mut k_msg_fields: Vec<(std::sync::Arc<str>, V)> = vec![
                    ("guid".into(), guid(k_guid.0, k_guid.1)),
                    ("phase".into(), en("CREATED")),
                    ("parentIndex".into(), st(vec![("guid", guid(t_guid.0, t_guid.1)), ("position", V::String(pos_str(k_idx)))])),
                    ("type".into(), en("KEYFRAME")),
                    ("name".into(), V::String(kf_name.to_string())),
                    ("visible".into(), V::Bool(true)),
                    ("opacity".into(), fl_one(1.0)),
                    ("size".into(), vec2(0.0, 0.0)),
                    ("transform".into(), encode_matrix(&Affine::IDENTITY)),
                ];
                for (k, v) in &k_json {
                    if let Some(pair) = cx.schema.def_field_from_json(cx.nc_def, k, v) {
                        k_msg_fields.push(pair);
                    }
                }
                if let Some(kf_meta_obj) = kf_meta_obj {
                    let k_meta_btree: BTreeMap<String, String> =
                        kf_meta_obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect();
                    append_fig_metadata(cx.schema, cx.nc_def, &mut k_msg_fields, &k_meta_btree);
                }
                cx.changes.push(V::Message(k_msg_fields));
            }

            if let Some(pc_str) = meta_obj.and_then(|m| m.get("_passthroughChildren")).and_then(serde_json::Value::as_str) {
                if let Ok(serde_json::Value::Array(pc_arr)) = serde_json::from_str::<serde_json::Value>(pc_str) {
                    for mut child_val in pc_arr {
                        if let Some(child_obj) = child_val.as_object_mut() {
                            if let Some(pi) = child_obj.get_mut("parentIndex").and_then(serde_json::Value::as_object_mut) {
                                if pi.get("guid").and_then(crate::motion::guid_text).as_deref() == Some(track.id.as_str()) {
                                    pi.insert("guid".into(), crate::motion::guid_pair_json(t_guid));
                                }
                            }
                            let mut p_msg_fields: Vec<(std::sync::Arc<str>, V)> = Vec::new();
                            for (k, v) in child_obj.iter() {
                                if let Some(pair) = cx.schema.def_field_from_json(cx.nc_def, k, v) {
                                    p_msg_fields.push(pair);
                                }
                            }
                            if !p_msg_fields.is_empty() {
                                cx.changes.push(V::Message(p_msg_fields));
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn format_grid_tracks(count: u32, tracks: &[GridTrack]) -> String {
    (0..(count as usize).min(tracks.len()))
        .map(|i| match tracks[i] {
            GridTrack::Flex(w) => {
                let w = finite_or(w, 1.0);
                if (w - w.round()).abs() < 1e-4 {
                    format!("{}fr", w.round() as i32)
                } else {
                    format!("{w}fr")
                }
            }
            GridTrack::Fixed(px) => {
                let px = finite_or(px, 0.0);
                if (px - px.round()).abs() < 1e-4 {
                    format!("{}px", px.round() as i32)
                } else {
                    format!("{px}px")
                }
            }
            GridTrack::Hug => "auto".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `gridRows` count and `gridRowsSizing` of grid `node`. A grid with no explicit rows still gives every
/// Fixed row slot its height (a slot that is not Fixed takes the Fixed auto-track height), and the file
/// has no other place for row tracks, so those slots are written as explicit rows up to the last Fixed one.
/// Only slots inside the rows the children fill ([`placed_grid_rows`]) are written: a slot past them is unused,
/// and as a real row it would add an empty row and a gap.
pub(crate) fn grid_rows_for_export(doc: &Document, node: &Node, al: &AutoLayout) -> Option<(u32, String)> {
    grid_row_tracks_for_export(doc, node, al).map(|(count, tracks)| (count, format_grid_tracks(count, &tracks)))
}

/// The tracks behind [`grid_rows_for_export`]: the row count and the size of every row up to it.
pub(crate) fn grid_row_tracks_for_export(doc: &Document, node: &Node, al: &AutoLayout) -> Option<(u32, Vec<GridTrack>)> {
    if al.grid_rows > 0 {
        return Some((al.grid_rows, al.grid_row_tracks.as_slice().to_vec()));
    }
    if al.direction != LayoutDirection::Grid || !al.grid_row_tracks.iter().any(|t| matches!(t, GridTrack::Fixed(_))) {
        return None;
    }
    let kids: Vec<_> = node.children.iter().filter_map(|c| doc.flow_child_input(*c, true)).collect();
    let placed = designos_doc::layout_solver::placed_grid_rows(al, &kids);
    let count = al.grid_row_tracks[..placed.min(al.grid_row_tracks.len())].iter().rposition(|t| matches!(t, GridTrack::Fixed(_)))? + 1;
    let auto = match al.grid_auto_tracks {
        GridAutoTracks::Track(t @ GridTrack::Fixed(_)) => Some(t),
        _ => None,
    };
    let tracks: Vec<GridTrack> = al.grid_row_tracks[..count].iter().map(|&t| if matches!(t, GridTrack::Fixed(_)) { t } else { auto.unwrap_or(t) }).collect();
    Some((count as u32, tracks))
}

pub(crate) fn format_grid_auto_tracks(gat: GridAutoTracks) -> Option<String> {
    match gat {
        GridAutoTracks::None => None,
        GridAutoTracks::Rows => Some("ROWS".to_string()),
        GridAutoTracks::Track(t) => {
            let mut arr = [GridTrack::default(); 8];
            arr[0] = t;
            Some(format_grid_tracks(1, &arr))
        }
    }
}

/// True for a frame or instance whose auto layout was removed on purpose (`layout_cleared`): the file needs
/// an explicit `stackMode: NONE` to keep it that way. A node that merely lacks auto layout, or only has
/// some other layout field overridden, must not be written with it.
fn clears_inherited_auto_layout(n: &Node) -> bool {
    n.component.layout_cleared && n.kind.frame_props().is_some_and(|fp| fp.auto_layout.is_none())
}

fn encode_node_var_entries_skipping(cx: &ExportCtx, vars: &NodeVariables, skip_fields: &HashSet<String>) -> Vec<V> {
    encode_var_entries_from_bindings_skipping(cx, &vars.bindings, skip_fields)
}

fn encode_var_entries_from_bindings(cx: &ExportCtx, bindings: &BTreeMap<String, String>) -> Vec<V> {
    encode_var_entries_from_bindings_skipping(cx, bindings, &HashSet::new())
}

fn encode_var_entries_from_bindings_skipping(cx: &ExportCtx, bindings: &BTreeMap<String, String>, skip_fields: &HashSet<String>) -> Vec<V> {
    let mut var_entries = Vec::new();
    let mut font_style_done = false;
    for (k, vid) in bindings {
        if k.starts_with("fills/") || k.starts_with("strokes/") || k.starts_with("effects/") {
            continue;
        }
        if k == "fontWeight" || k == "fontStyle" {
            if font_style_done || skip_fields.contains("FONT_STYLE") {
                continue;
            }
            font_style_done = true;
            let mut fs_fields = Vec::new();
            if !skip_fields.contains("FONT_STYLE:asString") {
                if let Some(vg) = bindings.get("fontStyle").and_then(|v| cx.resolve_var_guid(v)) {
                    fs_fields.push(("asString", alias_var_data(vg)));
                }
            }
            if !skip_fields.contains("FONT_STYLE:asFloat") {
                if let Some(vg) = bindings.get("fontWeight").and_then(|v| cx.resolve_var_guid(v)) {
                    fs_fields.push(("asFloat", alias_var_data(vg)));
                }
            }
            if !fs_fields.is_empty() {
                var_entries.push(msg(vec![
                    ("variableField", en("FONT_STYLE")),
                    ("variableData", msg(vec![("dataType", en("FONT_STYLE")), ("value", msg(vec![("fontStyleValue", msg(fs_fields))]))])),
                ]));
            }
            continue;
        }
        let Some(vg) = cx.resolve_var_guid(vid) else { continue };
        let vfield = match k.as_str() {
            "visible" => "VISIBLE",
            "characters" => "TEXT_DATA",
            "component" => "OVERRIDDEN_SYMBOL_ID",
            "paddingLeft" => "STACK_PADDING_LEFT",
            "paddingRight" => "STACK_PADDING_RIGHT",
            "paddingTop" => "STACK_PADDING_TOP",
            "paddingBottom" => "STACK_PADDING_BOTTOM",
            "itemSpacing" => "STACK_SPACING",
            "counterAxisSpacing" => "STACK_COUNTER_SPACING",
            "topLeftRadius" => "RECTANGLE_TOP_LEFT_CORNER_RADIUS",
            "topRightRadius" => "RECTANGLE_TOP_RIGHT_CORNER_RADIUS",
            "bottomLeftRadius" => "RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS",
            "bottomRightRadius" => "RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS",
            "width" | "size/x" => "WIDTH",
            "height" | "size/y" => "HEIGHT",
            "minWidth" => "MIN_WIDTH",
            "maxWidth" => "MAX_WIDTH",
            "minHeight" => "MIN_HEIGHT",
            "maxHeight" => "MAX_HEIGHT",
            "opacity" => "OPACITY",
            "strokeWeight" => "STROKE_WEIGHT",
            "strokeTopWeight" => "BORDER_TOP_WEIGHT",
            "strokeRightWeight" => "BORDER_RIGHT_WEIGHT",
            "strokeBottomWeight" => "BORDER_BOTTOM_WEIGHT",
            "strokeLeftWeight" => "BORDER_LEFT_WEIGHT",
            "fontSize" => "FONT_SIZE",
            "fontFamily" => "FONT_FAMILY",
            "lineHeight" => "LINE_HEIGHT",
            "letterSpacing" => "LETTER_SPACING",
            "paragraphSpacing" => "PARAGRAPH_SPACING",
            "paragraphIndent" => "PARAGRAPH_INDENT",
            _ => continue,
        };
        if skip_fields.contains(vfield) {
            continue;
        }
        var_entries.push(msg(vec![("variableField", en(vfield)), ("variableData", alias_var_data(vg))]));
    }
    var_entries
}

fn encode_node_mode_entries(cx: &ExportCtx, vars: &NodeVariables) -> Vec<V> {
    let mut mode_entries = Vec::new();
    for (cid, mid) in &vars.modes {
        let Some(cg) = cx.resolve_coll_guid(cid) else { continue };
        let Some(&mg) = cx.mode_guids.get(mid) else { continue };
        // A mode of an extended collection is named next to the collection at the root of its chain.
        let root =
            cx.doc.variable_collections.get(cid).filter(|c| c.is_extension()).and_then(|_| extension_root(cx.doc, cid)).and_then(|r| cx.resolve_coll_guid(&r));
        let entry = match root {
            Some(rg) => vec![
                ("variableSetID", msg(vec![("guid", guid(rg.0, rg.1))])),
                ("variableModeID", guid(mg.0, mg.1)),
                ("variableSetExtensionID", msg(vec![("guid", guid(cg.0, cg.1))])),
            ],
            None => vec![("variableSetID", msg(vec![("guid", guid(cg.0, cg.1))])), ("variableModeID", guid(mg.0, mg.1))],
        };
        mode_entries.push(msg(entry));
    }
    mode_entries
}

/// Id of the collection at the root of the chain `coll` extends.
fn extension_root(doc: &Document, coll: &str) -> Option<String> {
    let mut cur = doc.variable_collections.get(coll)?.extension_parent_id_in(&doc.variable_collections)?;
    for _ in 0..8 {
        match doc.variable_collections.get(cur)?.extension_parent_id_in(&doc.variable_collections) {
            Some(next) => cur = next,
            None => return Some(cur.to_string()),
        }
    }
    None
}

fn node_fills(node: &Node) -> &[Paint] {
    match &node.kind {
        NodeKind::Text(t) => &t.style.fills,
        _ => &node.fills,
    }
}

fn node_paints_for_prefix<'a>(node: &'a Node, prefix: &str) -> &'a [Paint] {
    if prefix.starts_with("strokes") {
        &node.stroke.paints
    } else {
        node_fills(node)
    }
}

fn has_prefix_binding(node: &Node, prefix: &str) -> bool {
    node.variables.bindings.keys().any(|k| k == prefix || k.starts_with(prefix))
}

fn paint_bindings_match(doc: &Document, n: &Node, sn: &Node, prefix: &str) -> bool {
    let n_iter = n.variables.bindings.iter().filter(|(k, _)| *k == prefix || k.starts_with(prefix));
    let s_iter = sn.variables.bindings.iter().filter(|(k, _)| *k == prefix || k.starts_with(prefix));
    if !n_iter.eq(s_iter) {
        return false;
    }
    let count = node_paints_for_prefix(n, prefix).len().max(node_paints_for_prefix(sn, prefix).len());
    for i in 0..count {
        let key = format!("{prefix}{i}");
        if doc.effective_binding(n.id, &key) != doc.effective_binding(sn.id, &key) {
            return false;
        }
        let op_key = format!("{prefix}{i}/opacity");
        if doc.effective_binding(n.id, &op_key) != doc.effective_binding(sn.id, &op_key) {
            return false;
        }
    }
    true
}

fn gradient_matches_with_bindings(
    doc: &Document,
    n: &Node,
    sn: &Node,
    prefix: &str,
    i: usize,
    (h1, s1): (&[Vec2; 3], &[GradientStop]),
    (h2, s2): (&[Vec2; 3], &[GradientStop]),
) -> bool {
    if h1 != h2 || s1.len() != s2.len() {
        return false;
    }
    for (j, (st1, st2)) in s1.iter().zip(s2.iter()).enumerate() {
        if (st1.pos - st2.pos).abs() > 1e-5 {
            return false;
        }
        let stop_key = format!("{prefix}{i}/stops/{j}");
        let stop_bound = doc.effective_binding(n.id, &stop_key).is_some() && doc.effective_binding(n.id, &stop_key) == doc.effective_binding(sn.id, &stop_key);
        if !stop_bound && st1.color != st2.color {
            return false;
        }
    }
    true
}

fn paints_match_with_bindings(doc: &Document, n: &Node, sn: &Node, prefix: &str) -> bool {
    let p_n = node_paints_for_prefix(n, prefix);
    let p_s = node_paints_for_prefix(sn, prefix);
    if p_n.len() != p_s.len() {
        return false;
    }
    for (i, (a, b)) in p_n.iter().zip(p_s.iter()).enumerate() {
        if a.visible != b.visible || a.blend != b.blend {
            return false;
        }
        let op_key = format!("{prefix}{i}/opacity");
        let op_bound = doc.effective_binding(n.id, &op_key).is_some() && doc.effective_binding(n.id, &op_key) == doc.effective_binding(sn.id, &op_key);
        if !op_bound && (a.opacity - b.opacity).abs() > 1e-5 {
            return false;
        }
        let color_key = format!("{prefix}{i}");
        let color_key_alt = format!("{prefix}{i}/color");
        let color_bound = (doc.effective_binding(n.id, &color_key).is_some()
            && doc.effective_binding(n.id, &color_key) == doc.effective_binding(sn.id, &color_key))
            || (doc.effective_binding(n.id, &color_key_alt).is_some()
                && doc.effective_binding(n.id, &color_key_alt) == doc.effective_binding(sn.id, &color_key_alt));
        match (&a.kind, &b.kind) {
            (PaintKind::Solid(_), PaintKind::Solid(_)) if color_bound => {}
            (PaintKind::LinearGradient { handles: h1, stops: s1 }, PaintKind::LinearGradient { handles: h2, stops: s2 })
            | (PaintKind::RadialGradient { handles: h1, stops: s1 }, PaintKind::RadialGradient { handles: h2, stops: s2 })
            | (PaintKind::AngularGradient { handles: h1, stops: s1 }, PaintKind::AngularGradient { handles: h2, stops: s2 })
            | (PaintKind::DiamondGradient { handles: h1, stops: s1 }, PaintKind::DiamondGradient { handles: h2, stops: s2 }) => {
                if std::mem::discriminant(&a.kind) != std::mem::discriminant(&b.kind)
                    || !gradient_matches_with_bindings(doc, n, sn, prefix, i, (h1, s1), (h2, s2))
                {
                    return false;
                }
            }
            _ => {
                if a.kind != b.kind {
                    return false;
                }
            }
        }
    }
    true
}

fn fills_differ_from_source(doc: &Document, n: &Node, src: Option<&Node>) -> bool {
    let Some(sn) = src else {
        return n.component.overrides.contains(&OverrideField::Fills)
            || (matches!(n.kind, NodeKind::Text(_)) && n.component.overrides.contains(&OverrideField::TextStyle))
            || has_prefix_binding(n, "fills/");
    };
    if n.style_refs.get("fill") != sn.style_refs.get("fill") {
        return true;
    }
    if !paint_bindings_match(doc, n, sn, "fills/") {
        return true;
    }
    !paints_match_with_bindings(doc, n, sn, "fills/")
}

fn strokes_differ_from_source(doc: &Document, n: &Node, src: Option<&Node>) -> bool {
    let Some(sn) = src else {
        return n.component.overrides.contains(&OverrideField::Stroke) || has_prefix_binding(n, "strokes/");
    };
    if n.style_refs.get("stroke") != sn.style_refs.get("stroke") {
        return true;
    }
    if !paint_bindings_match(doc, n, sn, "strokes/") {
        return true;
    }
    if !paints_match_with_bindings(doc, n, sn, "strokes/") {
        return true;
    }
    if n.component.overrides.contains(&OverrideField::Stroke) {
        let s1 = &n.stroke;
        let s2 = &sn.stroke;
        if (s1.weight - s2.weight).abs() > 1e-5
            || s1.align != s2.align
            || s1.cap != s2.cap
            || s1.join != s2.join
            || s1.dashes != s2.dashes
            || (s1.miter_limit - s2.miter_limit).abs() > 1e-5
            || s1.side_weights != s2.side_weights
            || n.stroke_geometry != sn.stroke_geometry
        {
            return true;
        }
    }
    false
}

fn effect_bindings_match(n: &Node, sn: &Node) -> bool {
    let n_iter = n.variables.bindings.iter().filter(|(k, _)| *k == "effects/" || k.starts_with("effects/"));
    let s_iter = sn.variables.bindings.iter().filter(|(k, _)| *k == "effects/" || k.starts_with("effects/"));
    n_iter.eq(s_iter)
}

fn effects_differ_from_source(_doc: &Document, n: &Node, src: Option<&Node>) -> bool {
    let Some(sn) = src else {
        return n.component.overrides.contains(&OverrideField::Effects) || has_prefix_binding(n, "effects/");
    };
    if n.style_refs.get("effect") != sn.style_refs.get("effect") {
        return true;
    }
    if !effect_bindings_match(n, sn) {
        return true;
    }
    n.effects != sn.effects
}

fn overridden_node_bindings(n: &Node, src: Option<&Node>) -> BTreeMap<String, String> {
    let Some(sn) = src else {
        return n.variables.bindings.clone();
    };
    let mut diff: BTreeMap<String, String> =
        n.variables.bindings.iter().filter(|(k, v)| sn.variables.bindings.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    if diff.contains_key("fontStyle") || diff.contains_key("fontWeight") {
        for key in ["fontStyle", "fontWeight"] {
            if let Some(v) = n.variables.bindings.get(key) {
                diff.entry(key.to_string()).or_insert_with(|| v.clone());
            }
        }
    }
    diff
}

fn overridden_node_modes(n: &Node, src: Option<&Node>) -> BTreeMap<String, String> {
    let Some(sn) = src else {
        return n.variables.modes.clone();
    };
    n.variables.modes.iter().filter(|(k, v)| sn.variables.modes.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// Walks `component.source` from `id` to the authored template node inside a main component,
/// stopping before an `Instance` steps to the `NodeKind::Component` it instantiates.
fn authored_source_node(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = id;
    let mut origin = None;
    for _ in 0..64 {
        let Some(n) = doc.get(cur) else { break };
        let Some(src) = n.component.source else { break };
        if src == cur {
            break;
        }
        if doc.get(src).is_some_and(|sn| matches!(sn.kind, NodeKind::Component(_))) {
            break;
        }
        origin = Some(src);
        cur = src;
    }
    origin
}

fn authored_source_guid(cx: &ExportCtx<'_>, id: NodeId) -> Option<(u32, u32)> {
    let orig = authored_source_node(cx.doc, id)?;
    if let Some(g) = cx.doc.get(orig).and_then(|n| n.source_id.as_deref()).and_then(|s| s.rsplit(';').next()).and_then(parse_guid_pair) {
        return Some(g);
    }
    cx.node_guids.get(&orig).copied()
}

fn collect_instance_overrides(cx: &mut ExportCtx, inst_id: NodeId) -> Vec<V> {
    collect_instance_overrides_for(cx, inst_id, None)
}

/// [`collect_instance_overrides`] where `own_fields`, when given, replaces the fields the instance itself is read as
/// overriding (its own entry then states exactly those).
fn collect_instance_overrides_for(cx: &mut ExportCtx, inst_id: NodeId, own_fields: Option<&BTreeSet<OverrideField>>) -> Vec<V> {
    let mut out = Vec::new();
    let Some(inst) = cx.doc.get(inst_id) else { return out };
    let mut stack: Vec<(NodeId, Vec<(u32, u32)>)> = inst.children.iter().rev().map(|&cid| (cid, Vec::new())).collect();
    // What the instance itself overrides is an entry whose path is its main's guid (its `symbolID`), ahead of the
    // entries of its descendants. Without it a reopened instance follows its main again: the importer reads every key
    // of that entry as an override flag of the instance.
    if let NodeKind::Instance { component: Some(main), .. } = inst.kind {
        if let Some(main_guid) = cx.node_guids.get(&main).copied() {
            stack.push((inst_id, vec![main_guid]));
        }
    }

    while let Some((cid, mut path)) = stack.pop() {
        let Some(n) = cx.doc.get(cid) else { continue };
        let own = cid == inst_id;
        if !own {
            let step_g = authored_source_guid(cx, cid)
                .or_else(|| n.source_id.as_deref().and_then(|s| s.rsplit(';').next()).and_then(parse_guid_pair))
                .or_else(|| cx.node_guids.get(&cid).copied())
                .unwrap_or((1, cid.0 as u32));
            path.push(step_g);
        }

        let src_node = if own {
            n.component
                .source
                .or(match n.kind {
                    NodeKind::Instance { component, .. } => component,
                    _ => None,
                })
                .and_then(|s| cx.doc.get(s))
        } else {
            n.component.source.and_then(|s| cx.doc.get(s))
        };

        // Swapped, as far as the file can tell: both this instance and the one it mirrors in the main have a component,
        // and they are not the same. A main-side instance with none is a gap, not a swap to write.
        let nested_comp_differs = !own
            && match (&n.kind, src_node.map(|sn| &sn.kind)) {
                (NodeKind::Instance { component: Some(c), .. }, Some(NodeKind::Instance { component: Some(sc), .. })) => c != sc,
                _ => false,
            };
        let has_nested_prop_ov = !own && !n.component.values.is_empty() && src_node.is_none_or(|sn| sn.component.values != n.component.values);
        let has_own_variant_bindings = !own && !n.component.own_variant_bindings.is_empty();
        let has_variant_picks = !own && !n.component.variant_picks.is_empty();

        // A swapped nested instance inherits its shell paints and variable bindings from the component it was swapped to.
        let inherited_src = if nested_comp_differs {
            match n.kind {
                NodeKind::Instance { component: Some(c), .. } => cx.doc.get(c).or(src_node),
                _ => src_node,
            }
        } else {
            src_node
        };

        let fill_differs = fills_differ_from_source(cx.doc, n, inherited_src);
        let stroke_differs = strokes_differ_from_source(cx.doc, n, inherited_src);
        let effect_differs = effects_differ_from_source(cx.doc, n, inherited_src);
        let has_fill_var = !own && (has_prefix_binding(n, "fills/") || inherited_src.is_some_and(|sn| has_prefix_binding(sn, "fills/"))) && fill_differs;
        let has_stroke_var =
            !own && (has_prefix_binding(n, "strokes/") || inherited_src.is_some_and(|sn| has_prefix_binding(sn, "strokes/"))) && stroke_differs;
        let has_effect_var =
            !own && (has_prefix_binding(n, "effects/") || inherited_src.is_some_and(|sn| has_prefix_binding(sn, "effects/"))) && effect_differs;
        let ve = if own { Vec::new() } else { encode_var_entries_from_bindings(cx, &overridden_node_bindings(n, inherited_src)) };
        let me = if own {
            Vec::new()
        } else {
            let modes = overridden_node_modes(n, inherited_src);
            encode_node_mode_entries(cx, &NodeVariables { bindings: BTreeMap::new(), modes })
        };

        // The instance's own bindings, override level and detached id are fields of its node change already, so
        // only a flagged field makes an entry for it; they are left out of it below.
        let ovs = match own_fields {
            Some(fields) if own => fields,
            _ => &n.component.overrides,
        };
        // The instance keeps the effects Figma cached for it under a link it inherits. Figma's file states that as the
        // record's own list next to a keyless entry that restates the main's list, and the loader reads the pair back
        // the same way; a link pinned on the record would not survive the reopen (see the pin check above).
        let restated_style = if own { n.style_refs.get("effect").filter(|sid| inherits_effect_link_with_own_cache(cx, n, sid)).cloned() } else { None };
        if !ovs.is_empty()
            || restated_style.is_some()
            || (!own
                && (has_fill_var
                    || has_stroke_var
                    || has_effect_var
                    || !ve.is_empty()
                    || !me.is_empty()
                    || n.component.override_level.is_some()
                    || n.component.detached_symbol_id.is_some()
                    || nested_comp_differs
                    || has_nested_prop_ov
                    || has_own_variant_bindings
                    || has_variant_picks))
        {
            let mut ov: Vec<(&'static str, V)> = Vec::new();
            let mut carried: carrier::Carried = Vec::new();
            let guids: Vec<V> = path.iter().map(|&(s, l)| guid(s, l)).collect();
            ov.push(("guidPath", msg(vec![("guids", V::Array(guids))])));
            if let Some(lvl) = n.component.override_level.filter(|_| !own) {
                ov.push(("overrideLevel", V::Int(lvl)));
            }
            if let Some(ds) = n.component.detached_symbol_id.as_deref().or(n.detached_symbol_id.as_deref()).filter(|s| !own && !s.is_empty()) {
                ov.push(("detachedSymbolId", encode_detached_symbol_id(ds)));
            }

            if ovs.contains(&OverrideField::Name) {
                ov.push(("name", V::String(n.name.clone())));
            }
            if ovs.contains(&OverrideField::Visible) {
                ov.push(("visible", V::Bool(n.visible)));
            }
            if ovs.contains(&OverrideField::Locked) {
                ov.push(("locked", V::Bool(n.locked)));
            }
            if ovs.contains(&OverrideField::LockAspect) {
                ov.push(("proportionsConstrained", V::Bool(n.lock_aspect)));
            }
            if ovs.contains(&OverrideField::Opacity) {
                ov.push(("opacity", fl_one(n.opacity)));
            }
            if ovs.contains(&OverrideField::Blend) {
                ov.push(("blendMode", en(blend_name(n.blend))));
            }
            if ovs.contains(&OverrideField::Size) {
                ov.push(("size", vec2(n.size.x, n.size.y)));
            }
            if ovs.contains(&OverrideField::Transform) {
                ov.push(("transform", encode_matrix(&n.transform)));
            }
            // A paint the file records as an override is written as it is, even when it equals the main's paint:
            // the record is what keeps the node from following a later change of the main. Only a paint the node
            // merely inherits (no flag) is left out when it matches what the main component supplies.
            let text_fill_flagged = matches!(n.kind, NodeKind::Text(_)) && ovs.contains(&OverrideField::TextStyle);
            if ovs.contains(&OverrideField::Fills) || (text_fill_flagged && fill_differs) || has_fill_var {
                let fills_slice = node_fills(n);
                let fps: Vec<V> = fills_slice.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "fills", i, &n.variables.bindings)).collect();
                ov.push(("fillPaints", V::Array(fps)));
            }
            if ovs.contains(&OverrideField::Stroke) || has_stroke_var {
                let sps: Vec<V> =
                    n.stroke.paints.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "strokes", i, &n.variables.bindings)).collect();
                ov.push(("strokePaints", V::Array(sps)));
                ov.push(("strokeWeight", fl(n.stroke.weight)));
                ov.push((
                    "strokeAlign",
                    en(match n.stroke.align {
                        StrokeAlign::Inside => "INSIDE",
                        StrokeAlign::Center => "CENTER",
                        StrokeAlign::Outside => "OUTSIDE",
                    }),
                ));
                ov.push((
                    "strokeJoin",
                    en(match n.stroke.join {
                        StrokeJoin::Miter => "MITER",
                        StrokeJoin::Bevel => "BEVEL",
                        StrokeJoin::Round => "ROUND",
                    }),
                ));
                ov.push(("strokeCap", en(crate::vecnet::cap_name(n.stroke.cap))));
                ov.push(("dashPattern", V::Array(n.stroke.dashes.iter().map(|&d| fl(d)).collect())));
                if n.stroke.miter_limit >= 1.0 {
                    ov.push(("miterLimit", fl(n.stroke.miter_limit)));
                }
                if let Some(paths) = &n.stroke_geometry {
                    let geometry = encode_geometry_blobs(cx, paths);
                    if !geometry.is_empty() {
                        ov.push(("strokeGeometry", V::Array(geometry)));
                    }
                }
                if let Some([top, right, bottom, left]) = n.stroke.side_weights {
                    ov.push(("borderStrokeWeightsIndependent", V::Bool(true)));
                    ov.push(("borderTopWeight", fl(top)));
                    ov.push(("borderRightWeight", fl(right)));
                    ov.push(("borderBottomWeight", fl(bottom)));
                    ov.push(("borderLeftWeight", fl(left)));
                }
            }
            if ovs.contains(&OverrideField::Effects) || has_effect_var {
                let fx: Vec<V> = n.effects.iter().enumerate().map(|(i, e)| encode_effect_with_bindings(cx, e, i, &n.variables.bindings)).collect();
                if let Some(style_override) = instance_style_override(cx, n, "effect") {
                    ov.push(("styleIdForEffect", style_override));
                }
                ov.push(("effects", V::Array(fx)));
            }
            if let Some(Style::Effect(fx)) = restated_style.as_ref().and_then(|sid| cx.doc.styles.get(sid)) {
                let bindings = restated_style.as_ref().and_then(|sid| cx.doc.style_meta.get(sid)).map(|m| m.bindings.clone()).unwrap_or_default();
                let list: Vec<V> = fx.iter().enumerate().map(|(i, e)| encode_effect_with_bindings(cx, e, i, &bindings)).collect();
                ov.push(("effects", V::Array(list)));
            }
            if ovs.contains(&OverrideField::CornerRadii) {
                let r = n.corner_radii;
                ov.push(("cornerRadius", fl(r[0])));
                // The reader drops the baked fill outline of every other kind when a corner radius is overridden, so it is
                // written again. For a vector or boolean node it keeps the outline, and a `fillGeometry` key would replace
                // the vector network with that outline.
                if !matches!(n.kind, NodeKind::Vector { .. } | NodeKind::BooleanOp(_)) {
                    if let Some(paths) = &n.fill_geometry {
                        let geometry = encode_geometry_blobs(cx, paths);
                        if !geometry.is_empty() {
                            ov.push(("fillGeometry", V::Array(geometry)));
                        }
                    }
                }
                if !r.iter().all(|&x| (x - r[0]).abs() < 1e-5) {
                    ov.push(("rectangleCornerRadiiIndependent", V::Bool(true)));
                    ov.push(("rectangleTopLeftCornerRadius", fl(r[0])));
                    ov.push(("rectangleTopRightCornerRadius", fl(r[1])));
                    ov.push(("rectangleBottomRightCornerRadius", fl(r[2])));
                    ov.push(("rectangleBottomLeftCornerRadius", fl(r[3])));
                }
            }
            // The arc is written whole, as the file states it: a reader takes only the keys an entry carries, and the
            // baked outline is left out so the reopened ring is drawn from the arc and the size.
            if let (true, NodeKind::Ellipse(arc)) = (ovs.contains(&OverrideField::Arc), &n.kind) {
                ov.push(("arcData", msg(vec![("startingAngle", fl(arc.start)), ("endingAngle", fl(arc.end)), ("innerRadius", fl(arc.inner_radius))])));
            }
            if let NodeKind::Text(tp) = &n.kind {
                let src_tp = src_node.and_then(|sn| match &sn.kind {
                    NodeKind::Text(stp) => Some(stp),
                    _ => None,
                });
                let runs_differ = !tp.runs.is_empty() && (ovs.contains(&OverrideField::TextStyle) || src_tp.is_none_or(|stp| stp.runs != tp.runs));
                let lists_differ = (tp.has_lists() || tp.direction != TextDirection::Auto)
                    && (ovs.contains(&OverrideField::Characters)
                        || ovs.contains(&OverrideField::TextStyle)
                        || src_tp.is_none_or(|stp| stp.lists != tp.lists || stp.direction != tp.direction));
                // Only a node that overrides its characters records them: a record that names the characters pins the
                // text, and a node with a style-only override must keep following its main component's text.
                if ovs.contains(&OverrideField::Characters) {
                    ov.push(("textData", encode_text_data(tp, runs_differ, lists_differ, None)));
                }
                if ovs.contains(&OverrideField::TextStyle) {
                    // Figma applies an instance text-style relink/detach from the
                    // override record. The ordinary node style reference is not
                    // consulted while replaying symbolOverrides, so preserve the
                    // effective reference whenever it differs from the source.
                    if let Some(style_override) = instance_style_override(cx, n, "text") {
                        ov.push(("styleIdForText", style_override));
                    }
                    append_text_style_fields(&mut ov, &tp.style);
                    append_paragraph_fields(&mut ov, tp);
                }
            }
            if ovs.contains(&OverrideField::Layout) {
                if let Some(fp) = n.kind.frame_props() {
                    ov.push(("frameMaskDisabled", V::Bool(!fp.clips_content)));
                    if let Some(al) = &fp.auto_layout {
                        let sm = match al.direction {
                            LayoutDirection::Horizontal => "HORIZONTAL",
                            LayoutDirection::Vertical => "VERTICAL",
                            LayoutDirection::Grid => "GRID",
                        };
                        ov.push(("stackMode", en(sm)));
                        grid::push_frame_fields(cx, cid, n, al, &mut ov, &mut carried);
                        if al.direction == LayoutDirection::Grid {
                            ov.push(("gridColumnGap", fl(al.item_spacing)));
                            ov.push(("gridRowGap", fl(al.counter_spacing)));
                        }
                        ov.push(("stackSpacing", fl(al.item_spacing)));
                        ov.push(("stackCounterSpacing", fl(al.counter_spacing)));
                        ov.push(("stackVerticalPadding", fl(al.padding[0])));
                        ov.push(("stackPaddingRight", fl(al.padding[1])));
                        ov.push(("stackPaddingBottom", fl(al.padding[2])));
                        ov.push(("stackHorizontalPadding", fl(al.padding[3])));
                        let align_name = |a: LayoutAlign| match a {
                            LayoutAlign::Start => "MIN",
                            LayoutAlign::Center => "CENTER",
                            LayoutAlign::End => "MAX",
                            LayoutAlign::SpaceBetween => "SPACE_BETWEEN",
                            LayoutAlign::Baseline => "BASELINE",
                            LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly => "SPACE_BETWEEN",
                        };
                        let justify_name = match al.primary_align {
                            LayoutAlign::SpaceAround => "SPACE_AROUND",
                            LayoutAlign::SpaceEvenly => "SPACE_EVENLY_CSS",
                            LayoutAlign::Baseline => "MIN",
                            other => align_name(other),
                        };
                        ov.push(("stackPrimaryAlignItems", en(justify_name)));
                        ov.push(("stackCounterAlignItems", en(align_name(al.counter_align))));
                        if al.counter_align_content == CounterAlignContent::SpaceBetween {
                            ov.push(("stackCounterAlignContent", en("SPACE_BETWEEN")));
                        }
                        if al.layout_version > 0 {
                            ov.push(("stackLayoutVersion", V::Uint(al.layout_version as u32)));
                        }
                        ov.push(("stackPrimarySizing", en(if al.primary_sizing == Sizing::Hug { "RESIZE_TO_FIT" } else { "FIXED" })));
                        ov.push(("stackCounterSizing", en(if al.counter_sizing == Sizing::Hug { "RESIZE_TO_FIT_WITH_IMPLICIT_SIZE" } else { "FIXED" })));
                        ov.push(("stackWrap", en(if al.wrap { "WRAP" } else { "NO_WRAP" })));
                        ov.push(("bordersTakeSpace", V::Bool(al.stroke_included)));
                        ov.push(("stackReverseZIndex", V::Bool(al.reverse_z)));
                    } else if clears_inherited_auto_layout(n) {
                        ov.push(("stackMode", en("NONE")));
                    }
                }
            }
            // How a node sits in its parent. The instance's own is a field of its node change already, and the frame
            // fields above keep its Layout flag.
            if ovs.contains(&OverrideField::Layout) && !own {
                let parent_dir =
                    n.parent.and_then(|p| cx.doc.get(p)).and_then(|p| p.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).map(|a| a.direction);
                let lc = &n.layout_child;
                let (prim_fill, counter_fill) = match parent_dir {
                    Some(LayoutDirection::Horizontal | LayoutDirection::Grid) => (lc.horizontal == Sizing::Fill || lc.grow > 0.0, lc.vertical == Sizing::Fill),
                    Some(LayoutDirection::Vertical) => (lc.vertical == Sizing::Fill || lc.grow > 0.0, lc.horizontal == Sizing::Fill),
                    None => (lc.grow > 0.0, lc.horizontal == Sizing::Fill && lc.vertical == Sizing::Fill),
                };
                if prim_fill {
                    ov.push(("stackChildPrimaryGrow", fl(if lc.grow > 0.0 { lc.grow } else { 1.0 })));
                } else if parent_dir.is_some() {
                    ov.push(("stackChildPrimaryGrow", fl(0.0)));
                }
                let align_self_ov = match lc.align_self {
                    AlignSelf::Start => Some("MIN"),
                    AlignSelf::Center => Some("CENTER"),
                    AlignSelf::End => Some("MAX"),
                    AlignSelf::Stretch => Some("STRETCH"),
                    AlignSelf::Baseline => Some("BASELINE"),
                    AlignSelf::Auto if counter_fill => Some("STRETCH"),
                    AlignSelf::Auto if parent_dir.is_some() => Some("AUTO"),
                    AlignSelf::Auto => None,
                };
                if let Some(a) = align_self_ov {
                    ov.push(("stackChildAlignSelf", en(a)));
                }
                grid::push_child_fields(grid::override_parent_ids(cx, inst_id, n), lc, &mut ov, &mut carried);
                // An alignment key is written only for an axis that differs from the main component's child: a reader
                // keeps the main's value for any key the override does not name, so naming one that still matches
                // would freeze it (a margin, span or sizing override takes this same Layout flag). A reset to Start
                // differs from a Center or End main and is written as MIN. With no main child to compare against,
                // both axes are written.
                let main_lc = n.component.source.and_then(|s| cx.doc.get(s)).map(|s| s.layout_child);
                let axes = [
                    ("gridChildHorizontalAlign", lc.grid_align_h, main_lc.map(|m| m.grid_align_h), n.component.grid_align_overrides[0]),
                    ("gridChildVerticalAlign", lc.grid_align_v, main_lc.map(|m| m.grid_align_v), n.component.grid_align_overrides[1]),
                ];
                for (k, a, main, axis_overridden) in axes {
                    if axis_overridden || main != Some(a) {
                        ov.push((k, en(grid_child_align_name(a))));
                    }
                }
                // Every side is written, zero included: a reader rebuilds the instance from its master and keeps the
                // master's margin for any side the override does not name, so a cleared margin has to be explicit.
                ov.push(("stackChildMarginTop", fl(lc.margin[0])));
                ov.push(("stackChildMarginRight", fl(lc.margin[1])));
                ov.push(("stackChildMarginBottom", fl(lc.margin[2])));
                ov.push(("stackChildMarginLeft", fl(lc.margin[3])));
            }
            // The instance's own main is its `symbolID`; naming it again would make the reader swap it for itself.
            if (ovs.contains(&OverrideField::MainComponent) || nested_comp_differs) && !own {
                if let NodeKind::Instance { component: Some(comp_id), .. } = &n.kind {
                    if let Some(cg) = cx.node_guids.get(comp_id).copied() {
                        ov.push(("overriddenSymbolID", guid(cg.0, cg.1)));
                    }
                }
            }
            if has_nested_prop_ov {
                let mut prop_assigns = Vec::new();
                for (prop_name, pval) in &n.component.values {
                    let def_g = cx.resolve_prop_guid(cid, prop_name);
                    let val_msg = match pval {
                        PropValue::Bool(b) => msg(vec![("boolValue", V::Bool(*b))]),
                        PropValue::Text(s) | PropValue::Variant(s) => msg(vec![("textValue", msg(vec![("characters", V::String(s.clone()))]))]),
                        PropValue::Instance(tid) => {
                            let tg = cx.node_guids.get(tid).copied().unwrap_or((0, 0));
                            msg(vec![("guidValue", guid(tg.0, tg.1))])
                        }
                        PropValue::Slot(ids) => {
                            let s = ids.iter().filter_map(|i| cx.node_guids.get(i).map(|g| format!("{}:{}", g.0, g.1))).collect::<Vec<_>>().join(",");
                            msg(vec![("textValue", msg(vec![("characters", V::String(s))]))])
                        }
                    };
                    prop_assigns.push(msg(vec![("defID", guid(def_g.0, def_g.1)), ("value", val_msg)]));
                }
                if !prop_assigns.is_empty() {
                    ov.push(("componentPropAssignments", V::Array(prop_assigns)));
                }
            }
            if ovs.contains(&OverrideField::SlotContent) {
                carried.push(("slotContent", serde_json::Value::Bool(true)));
            }
            if has_own_variant_bindings {
                carried.push(("ownVariantBindings", json!(n.component.own_variant_bindings)));
            }
            if has_variant_picks {
                carried.push(("variantPicks", json!(n.component.variant_picks)));
            }
            if !ve.is_empty() {
                ov.push(("variableConsumptionMap", msg(vec![("entries", V::Array(ve))])));
            }
            if !me.is_empty() {
                ov.push(("variableModeBySetMap", msg(vec![("entries", V::Array(me))])));
            }
            if !carried.is_empty() {
                ov.push(("pluginData", V::Array(carrier::carrier_entries(&carried))));
            }
            if ov.len() > 1 {
                out.push(msg(ov));
            }
        }

        if !own {
            for &child in n.children.iter().rev() {
                stack.push((child, path.clone()));
            }
        }
    }
    out
}

/// Preferred components of an instance-swap or slot property: Figma lists them as `COMPONENT` entries keyed by
/// the component's library key. A component of this file has no library key, so its key is its GUID as `"s:l"`.
/// `varValue` of a component property assignment bound to a variable: the alias, with the type the property takes.
fn prop_alias(cx: &ExportCtx, node: &Node, prop_name: &str) -> Option<V> {
    let var = cx.doc.variables.get(bound_prop_variable_of(node, prop_name)?)?;
    let vg = cx.resolve_var_guid(&var.id)?;
    let resolved = match var.kind {
        VariableKind::Boolean => "BOOLEAN",
        VariableKind::String => "STRING",
        VariableKind::Number => "FLOAT",
        VariableKind::Color => "COLOR",
    };
    Some(msg(vec![("dataType", en("ALIAS")), ("resolvedDataType", en(resolved)), ("value", msg(vec![("alias", msg(vec![("guid", guid(vg.0, vg.1))]))]))]))
}

fn prop_def_alias(cx: &ExportCtx, node: &Node, p: &ComponentPropDef) -> Option<V> {
    let var_id = node.component.prop_default_variables.get(&p.name)?;
    if let Some(key) = var_id.strip_prefix("key:") {
        let resolved = match &p.kind {
            PropKind::Boolean => "BOOLEAN",
            PropKind::Text | PropKind::Variant { .. } => "STRING",
            PropKind::InstanceSwap { .. } | PropKind::Slot => return None,
        };
        return Some(msg(vec![
            ("dataType", en("ALIAS")),
            ("resolvedDataType", en(resolved)),
            ("value", msg(vec![("alias", msg(vec![("assetRef", msg(vec![("key", V::String(key.to_string()))]))]))])),
        ]));
    }
    let var = cx.doc.variables.get(var_id)?;
    let vg = cx.resolve_var_guid(&var.id)?;
    let resolved = match var.kind {
        VariableKind::Boolean => "BOOLEAN",
        VariableKind::String => "STRING",
        VariableKind::Number => "FLOAT",
        VariableKind::Color => "COLOR",
    };
    Some(msg(vec![("dataType", en("ALIAS")), ("resolvedDataType", en(resolved)), ("value", msg(vec![("alias", msg(vec![("guid", guid(vg.0, vg.1))]))]))]))
}

fn bound_prop_variable_of<'a>(node: &'a Node, prop_name: &str) -> Option<&'a str> {
    node.variables.bindings.get(&prop_binding_key(prop_name)).map(String::as_str)
}

fn alias_var_data(vg: (u32, u32)) -> V {
    msg(vec![("dataType", en("ALIAS")), ("value", msg(vec![("alias", msg(vec![("guid", guid(vg.0, vg.1))]))]))])
}

fn encode_geometry_blobs(cx: &mut ExportCtx, paths: &[Path]) -> Vec<V> {
    paths
        .iter()
        .map(|p| {
            let blob_idx = cx.add_blob(path_to_commands_blob(p));
            let rule = if p.fill_rule == FillRule::EvenOdd { "ODD" } else { "NONZERO" };
            msg(vec![("windingRule", en(rule)), ("commandsBlob", V::Uint(blob_idx))])
        })
        .collect()
}

fn encode_vector_network_blob(net: &VectorNetwork) -> (Vec<u8>, Vec<V>) {
    let mut b = Vec::new();
    let mut style_table = Vec::new();
    for n in [net.vertices.len(), net.segments.len(), net.regions.len()] {
        b.extend_from_slice(&(n as u32).to_le_bytes());
    }
    for (i, v) in net.vertices.iter().enumerate() {
        let has_style = v.cap.is_some() || v.join.is_some() || v.corner_radius > 0.0 || v.mirror != HandleMirror::None;
        let sid = if has_style {
            let id = (style_table.len() + 1) as u32;
            let mut so = vec![("styleID", V::Uint(id))];
            if let Some(c) = v.cap {
                so.push(("strokeCap", en(crate::vecnet::cap_name(c))));
            }
            if let Some(j) = v.join {
                so.push((
                    "strokeJoin",
                    en(match j {
                        StrokeJoin::Miter => "MITER",
                        StrokeJoin::Bevel => "BEVEL",
                        StrokeJoin::Round => "ROUND",
                    }),
                ));
            }
            if v.corner_radius > 0.0 {
                so.push(("cornerRadius", fl(v.corner_radius)));
            }
            if v.mirror != HandleMirror::None {
                let m = match v.mirror {
                    HandleMirror::None => "NONE",
                    HandleMirror::Angle => "ANGLE",
                    HandleMirror::AngleAndLength => "ANGLE_AND_LENGTH",
                };
                so.push(("handleMirroring", en(m)));
            }
            style_table.push(msg(so));
            id
        } else {
            let _ = i;
            0
        };
        b.extend_from_slice(&sid.to_le_bytes());
        b.extend_from_slice(&blob_f32(v.pos.x));
        b.extend_from_slice(&blob_f32(v.pos.y));
    }
    for seg in &net.segments {
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&(seg.a as u32).to_le_bytes());
        b.extend_from_slice(&blob_f32(seg.ta.x));
        b.extend_from_slice(&blob_f32(seg.ta.y));
        b.extend_from_slice(&(seg.b as u32).to_le_bytes());
        b.extend_from_slice(&blob_f32(seg.tb.x));
        b.extend_from_slice(&blob_f32(seg.tb.y));
    }
    for reg in &net.regions {
        let rule = if reg.rule == FillRule::NonZero { 1u32 } else { 0u32 };
        b.extend_from_slice(&rule.to_le_bytes());
        b.extend_from_slice(&(reg.loops.len() as u32).to_le_bytes());
        for lp in &reg.loops {
            b.extend_from_slice(&(lp.len() as u32).to_le_bytes());
            for &seg_idx in lp {
                b.extend_from_slice(&(seg_idx as u32).to_le_bytes());
            }
        }
    }
    (b, style_table)
}

fn encode_matrix(xf: &Affine) -> V {
    st(vec![("m00", fl_one(xf.a)), ("m01", fl(xf.c)), ("m02", fl(xf.tx)), ("m10", fl(xf.b)), ("m11", fl_one(xf.d)), ("m12", fl(xf.ty))])
}

fn gradient_matrix_from_handles(ty: &str, h: &[Vec2; 3]) -> V {
    // Inverse of `fig.rs` `convert_paint`:
    // - `GRADIENT_LINEAR`: `inv` maps `(0, 0.5) -> h[0]`, `(1, 0.5) -> h[1]`, `(0, 1.0) -> h[2]`.
    // - `GRADIENT_RADIAL` / `GRADIENT_ANGULAR` / `GRADIENT_DIAMOND`: `inv` maps `(0.5, 0.5) -> h[0]`, `(1, 0.5) -> h[1]`, `(0.5, 1.0) -> h[2]`.
    let inv = if ty == "GRADIENT_LINEAR" {
        Affine {
            a: h[1].x - h[0].x,
            b: h[1].y - h[0].y,
            c: 2.0 * (h[2].x - h[0].x),
            d: 2.0 * (h[2].y - h[0].y),
            tx: 2.0 * h[0].x - h[2].x,
            ty: 2.0 * h[0].y - h[2].y,
        }
    } else {
        Affine {
            a: 2.0 * (h[1].x - h[0].x),
            b: 2.0 * (h[1].y - h[0].y),
            c: 2.0 * (h[2].x - h[0].x),
            d: 2.0 * (h[2].y - h[0].y),
            tx: 3.0 * h[0].x - h[1].x - h[2].x,
            ty: 3.0 * h[0].y - h[1].y - h[2].y,
        }
    };
    let xf = inv.invert().unwrap_or(Affine::IDENTITY);
    encode_matrix(&xf)
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() || !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

fn encode_paint(p: &Paint, color_var: Option<V>, opacity_var: Option<V>) -> V {
    encode_paint_full(p, color_var, opacity_var, None)
}

fn encode_paint_with_bindings(cx: &ExportCtx, p: &Paint, prefix: &str, idx: usize, bindings: &BTreeMap<String, String>) -> V {
    let color_var = bindings.get(&format!("{prefix}/{idx}")).and_then(|vid| cx.resolve_var_guid(vid)).map(alias_var_data);
    let opacity_var = bindings.get(&format!("{prefix}/{idx}/opacity")).and_then(|vid| cx.resolve_var_guid(vid)).map(alias_var_data);
    let stops_slice = match &p.kind {
        PaintKind::LinearGradient { stops, .. }
        | PaintKind::RadialGradient { stops, .. }
        | PaintKind::AngularGradient { stops, .. }
        | PaintKind::DiamondGradient { stops, .. } => Some(stops.as_slice()),
        _ => None,
    };
    let stops_var = stops_slice.and_then(|stops| {
        let has_any = (0..stops.len()).any(|j| bindings.get(&format!("{prefix}/{idx}/stops/{j}")).and_then(|vid| cx.resolve_var_guid(vid)).is_some());
        if !has_any {
            return None;
        }
        let sv: Vec<V> = stops
            .iter()
            .enumerate()
            .map(|(j, s)| {
                let mut f = vec![("color", color(s.color.r, s.color.g, s.color.b, s.color.a)), ("position", fl(s.pos))];
                if let Some(vg) = bindings.get(&format!("{prefix}/{idx}/stops/{j}")).and_then(|vid| cx.resolve_var_guid(vid)) {
                    f.push(("colorVar", alias_var_data(vg)));
                }
                msg(f)
            })
            .collect();
        Some(V::Array(sv))
    });
    encode_paint_full(p, color_var, opacity_var, stops_var)
}

fn encode_paint_full(p: &Paint, color_var: Option<V>, opacity_var: Option<V>, stops_var: Option<V>) -> V {
    let mut fields = Vec::new();
    let push_grad = |fields: &mut Vec<(&str, V)>, ty: &str, handles: &[Vec2; 3], stops: &[GradientStop]| {
        fields.push(("type", en(ty)));
        let st_list: Vec<V> = stops.iter().map(|s| st(vec![("color", color(s.color.r, s.color.g, s.color.b, s.color.a)), ("position", fl(s.pos))])).collect();
        fields.push(("stops", V::Array(st_list)));
        fields.push(("transform", gradient_matrix_from_handles(ty, handles)));
    };
    match &p.kind {
        PaintKind::Solid(c) => {
            fields.push(("type", en("SOLID")));
            fields.push(("color", color(c.r, c.g, c.b, c.a)));
        }
        PaintKind::LinearGradient { handles, stops } => push_grad(&mut fields, "GRADIENT_LINEAR", handles, stops),
        PaintKind::RadialGradient { handles, stops } => push_grad(&mut fields, "GRADIENT_RADIAL", handles, stops),
        PaintKind::AngularGradient { handles, stops } => push_grad(&mut fields, "GRADIENT_ANGULAR", handles, stops),
        PaintKind::DiamondGradient { handles, stops } => push_grad(&mut fields, "GRADIENT_DIAMOND", handles, stops),
        PaintKind::Image { image_ref, scale_mode, params } => {
            fields.push(("type", en("IMAGE")));
            if let Some(raw) = unhex(image_ref) {
                fields.push(("image", msg(vec![("hash", V::Bytes(raw))])));
            } else {
                // Not a hash: the reference rides in the image's name, with an empty hash.
                fields.push(("image", msg(vec![("hash", V::Bytes(Vec::new())), ("name", V::String(image_ref.clone()))])));
            }
            let sm = match scale_mode {
                ImageScaleMode::Fill => "FILL",
                ImageScaleMode::Fit => "FIT",
                ImageScaleMode::Crop => "STRETCH",
                ImageScaleMode::Tile => "TILE",
            };
            fields.push(("imageScaleMode", en(sm)));
            if let Some(m) = params.transform {
                fields.push((
                    "transform",
                    st(vec![("m00", fl_one(m[0])), ("m01", fl(m[1])), ("m02", fl(m[2])), ("m10", fl(m[3])), ("m11", fl_one(m[4])), ("m12", fl(m[5]))]),
                ));
            }
            if params.rotation != 0.0 {
                fields.push(("rotation", fl(params.rotation)));
            }
            if (params.tile_scale - 1.0).abs() > 1e-6 {
                fields.push(("scale", fl_one(params.tile_scale)));
            }
            if !params.adjust.is_default() {
                let adj = &params.adjust;
                let mut pf = Vec::new();
                for (k, val) in [
                    ("tint", adj.tint),
                    ("shadows", adj.shadows),
                    ("highlights", adj.highlights),
                    ("detail", adj.detail),
                    ("exposure", adj.exposure),
                    ("vignette", adj.vignette),
                    ("temperature", adj.temperature),
                    ("vibrance", adj.vibrance),
                    ("contrast", adj.contrast),
                    ("brightness", adj.brightness),
                ] {
                    if val != 0.0 {
                        pf.push((k, fl(val)));
                    }
                }
                if !pf.is_empty() {
                    fields.push(("paintFilter", msg(pf)));
                }
            }
            if let Some(cm) = params.color_manage {
                fields.push(("imageShouldColorManage", V::Bool(cm)));
            }
            if let Some((w, h)) = params.original_size {
                fields.push(("originalImageWidth", V::Uint(w)));
                fields.push(("originalImageHeight", V::Uint(h)));
            }
        }
        PaintKind::Shader(_) => {
            fields.push(("type", en("CUSTOM")));
        }
    }
    fields.push(("opacity", fl_one(p.opacity)));
    fields.push(("visible", V::Bool(p.visible)));
    fields.push(("blendMode", en(blend_name(p.blend))));
    if let Some(cv) = color_var {
        fields.push(("colorVar", cv));
    }
    if let Some(ov) = opacity_var {
        fields.push(("opacityVar", ov));
    }
    if let Some(sv) = stops_var {
        fields.push(("stopsVar", sv));
    }
    let mut out = msg(fields);
    if let (PaintKind::Shader(shader), V::Message(ref mut out_fields)) = (&p.kind, &mut out) {
        if let Ok(extra) = decode_shader_ref(export_schema(), "Paint", "", shader, &["type", "opacity", "visible", "blendMode"]) {
            for (k, v) in extra {
                if !out_fields.iter().any(|(existing, _)| *existing == k) {
                    out_fields.push((k, v));
                }
            }
        }
    }
    out
}

fn encode_effect_with_bindings(cx: &ExportCtx, e: &Effect, idx: usize, bindings: &BTreeMap<String, String>) -> V {
    let var_data = |prop: &str| -> Option<V> {
        let vid = bindings.get(&format!("effects/{idx}/{prop}"))?;
        let vg = cx.resolve_var_guid(vid)?;
        Some(alias_var_data(vg))
    };
    encode_effect_full(
        e,
        var_data("color"),
        var_data("opacity"),
        var_data("radius").or_else(|| var_data("frost")),
        var_data("spread"),
        var_data("offsetX"),
        var_data("offsetY"),
        var_data("lightAngle"),
        var_data("lightIntensity"),
        var_data("refraction"),
        var_data("depth"),
        var_data("dispersion"),
    )
}

/// The stored `noiseSize` vector of a noise or texture size.
fn noise_size_vec(size: f32, size_y: Option<f32>) -> V {
    let (x, y) = effect_units::noise_size_to_kiwi(size, size_y);
    vec2(x, y)
}

#[allow(clippy::too_many_arguments)]
fn encode_effect_full(
    e: &Effect,
    color_var: Option<V>,
    opacity_var: Option<V>,
    radius_var: Option<V>,
    spread_var: Option<V>,
    x_var: Option<V>,
    y_var: Option<V>,
    specular_angle_var: Option<V>,
    specular_intensity_var: Option<V>,
    refraction_intensity_var: Option<V>,
    refraction_radius_var: Option<V>,
    chromatic_aberration_var: Option<V>,
) -> V {
    let mut fields = match e {
        Effect::DropShadow { color: c, offset, radius, spread, visible, show_behind, blend } => vec![
            ("type", en("DROP_SHADOW")),
            ("color", color(c.r, c.g, c.b, c.a)),
            ("offset", vec2(offset.x, offset.y)),
            ("radius", fl(*radius)),
            ("spread", fl(*spread)),
            ("visible", V::Bool(*visible)),
            ("showShadowBehindNode", V::Bool(*show_behind)),
            ("blendMode", en(blend_name(*blend))),
        ],
        Effect::InnerShadow { color: c, offset, radius, spread, visible, blend } => vec![
            ("type", en("INNER_SHADOW")),
            ("color", color(c.r, c.g, c.b, c.a)),
            ("offset", vec2(offset.x, offset.y)),
            ("radius", fl(*radius)),
            ("spread", fl(*spread)),
            ("visible", V::Bool(*visible)),
            ("blendMode", en(blend_name(*blend))),
        ],
        Effect::LayerBlur { radius, visible } => vec![("type", en("FOREGROUND_BLUR")), ("radius", fl(*radius)), ("visible", V::Bool(*visible))],
        Effect::BackgroundBlur { radius, visible } => vec![("type", en("BACKGROUND_BLUR")), ("radius", fl(*radius)), ("visible", V::Bool(*visible))],
        Effect::Noise(n) => {
            let mut f = vec![
                ("type", en("NOISE")),
                ("noiseSize", noise_size_vec(n.size, n.size_y)),
                ("density", fl(n.density)),
                ("opacity", fl_one(n.opacity)),
                ("blendMode", en(blend_name(n.blend))),
                ("seed", V::Uint(n.seed)),
                ("visible", V::Bool(n.visible)),
            ];
            match &n.mode {
                NoiseMode::Mono { color: c } => {
                    f.push(("noiseType", en("MONOTONE")));
                    f.push(("color", color(c.r, c.g, c.b, c.a)));
                }
                NoiseMode::Duo { color: c, secondary: s } => {
                    f.push(("noiseType", en("DUOTONE")));
                    f.push(("color", color(c.r, c.g, c.b, c.a)));
                    f.push(("secondaryColor", color(s.r, s.g, s.b, s.a)));
                }
                NoiseMode::Multi => {
                    f.push(("noiseType", en("MULTITONE")));
                }
            }
            f
        }
        Effect::Texture(t) => vec![
            ("type", en("GRAIN")),
            ("noiseSize", noise_size_vec(t.size, t.size_y)),
            ("radius", fl(t.radius)),
            ("clipToShape", V::Bool(t.clip_to_shape)),
            ("seed", V::Uint(t.seed)),
            ("visible", V::Bool(t.visible)),
        ],
        Effect::Glass(g) => vec![
            ("type", en("GLASS")),
            ("specularAngle", fl(effect_units::specular_angle_to_kiwi(g.light_angle))),
            ("specularIntensity", fl(g.light_intensity)),
            ("refractionIntensity", fl(g.refraction)),
            ("refractionRadius", fl(g.depth)),
            ("chromaticAberration", fl(g.dispersion)),
            ("reflectionDistance", fl(g.splay)),
            ("bevelSize", fl(g.bevel)),
            ("radius", fl(g.frost)),
            ("visible", V::Bool(g.visible)),
        ],
        Effect::Shader { visible, .. } => vec![("type", en("CUSTOM")), ("visible", V::Bool(*visible))],
    };
    if let Some(cv) = color_var {
        fields.push(("colorVar", cv));
    }
    if let Some(ov) = opacity_var {
        fields.push(("effectOpacityVar", ov));
    }
    if let Some(rv) = radius_var {
        fields.push(("radiusVar", rv));
    }
    if let Some(sv) = spread_var {
        fields.push(("spreadVar", sv));
    }
    if let Some(xv) = x_var {
        fields.push(("xVar", xv));
    }
    if let Some(yv) = y_var {
        fields.push(("yVar", yv));
    }
    if let Some(av) = specular_angle_var {
        fields.push(("specularAngleVar", av));
    }
    if let Some(iv) = specular_intensity_var {
        fields.push(("specularIntensityVar", iv));
    }
    if let Some(rv) = refraction_intensity_var {
        fields.push(("refractionIntensityVar", rv));
    }
    if let Some(dv) = refraction_radius_var {
        fields.push(("refractionRadiusVar", dv));
    }
    if let Some(cv) = chromatic_aberration_var {
        fields.push(("chromaticAberrationVar", cv));
    }
    let mut out = msg(fields);
    if let (Effect::Shader { shader, .. }, V::Message(ref mut out_fields)) = (e, &mut out) {
        if let Ok(extra) = decode_shader_ref(export_schema(), "Effect", "", shader, &["type", "visible"]) {
            for (k, v) in extra {
                if !out_fields.iter().any(|(existing, _)| *existing == k) {
                    out_fields.push((k, v));
                }
            }
        }
    }
    out
}

/// A layout guide in Figma's own shape: a square grid is `pattern: GRID`, columns and rows are `pattern: STRIPES` on
/// axis X or Y, with `type` as the alignment and `numSections` as the count (negative for an automatic count).
fn encode_grid(g: &LayoutGrid) -> V {
    match g {
        LayoutGrid::Grid { size, color: c, visible } => msg(vec![
            ("type", en("STRETCH")),
            ("axis", en("X")),
            ("visible", V::Bool(*visible)),
            ("sectionSize", fl(*size)),
            ("color", color(c.r, c.g, c.b, c.a)),
            ("pattern", en("GRID")),
        ]),
        LayoutGrid::Columns { count, gutter, offset, section_size, alignment, color: c, visible }
        | LayoutGrid::Rows { count, gutter, offset, section_size, alignment, color: c, visible } => {
            let axis = if matches!(g, LayoutGrid::Columns { .. }) { "X" } else { "Y" };
            let align = match alignment {
                LayoutGridAlign::Min => "MIN",
                LayoutGridAlign::Center => "CENTER",
                LayoutGridAlign::Stretch => "STRETCH",
                LayoutGridAlign::Max => "MAX",
            };
            let num_sections = if *count == 0 { -1 } else { i32::try_from(*count).unwrap_or(-1) };
            msg(vec![
                ("type", en(align)),
                ("axis", en(axis)),
                ("visible", V::Bool(*visible)),
                ("numSections", V::Int(num_sections)),
                ("offset", fl(*offset)),
                // 0 is a real mode (equal-width stripes), so it is written like any other width
                ("sectionSize", fl(*section_size)),
                ("gutterSize", fl(*gutter)),
                ("color", color(c.r, c.g, c.b, c.a)),
                ("pattern", en("STRIPES")),
            ])
        }
    }
}

fn style_name_from_weight(weight: u16, italic: bool) -> String {
    let w = match weight {
        0..=149 => "Thin",
        150..=249 => "ExtraLight",
        250..=349 => "Light",
        350..=449 => "Regular",
        450..=549 => "Medium",
        550..=649 => "SemiBold",
        650..=749 => "Bold",
        750..=849 => "ExtraBold",
        _ => "Black",
    };
    if italic {
        if w == "Regular" {
            "Italic".into()
        } else {
            format!("{w} Italic")
        }
    } else {
        w.into()
    }
}

/// OpenType features Figma lists in `toggledOnOTFeatures` / `toggledOffOTFeatures`. The others Studio knows (`tnum`,
/// `onum`, `lnum`, `pnum`, `frac`, `smcp`, `c2sc`, `sups`, `subs`) are not features of that list in Figma: they ride
/// in the `fontVariant*` fields written beside it.
const KNOWN_OT_FEATURES: &[&str] = &[
    "LIGA", "DLIG", "CLIG", "CALT", "KERN", "SS01", "SS02", "SS03", "SS04", "SS05", "SS06", "SS07", "SS08", "SS09", "SS10", "AFRC", "ZERO", "PCAP", "C2PC",
    "UNIC", "TITL", "ORDN", "SWSH", "SALT", "CASE",
];

fn encode_detached_symbol_id(ds: &str) -> V {
    if let Some(rest) = ds.strip_prefix("assetRef:") {
        let (key, ver) = rest.split_once(':').unwrap_or((rest, ""));
        let mut ar = vec![("key", V::String(key.to_string()))];
        if !ver.is_empty() {
            ar.push(("version", V::String(ver.to_string())));
        }
        return msg(vec![("assetRef", msg(ar))]);
    }
    let guid_part = ds.strip_prefix("guid:").unwrap_or(ds);
    if let Some(g) = parse_guid_pair(guid_part) {
        return msg(vec![("guid", guid(g.0, g.1))]);
    }
    msg(vec![("assetRef", msg(vec![("key", V::String(ds.to_string()))]))])
}

/// Encodes preferred component/component-set `NodeId`s as a real `ComponentPropPreferredValues` message.
fn encode_preferred_values(cx: &ExportCtx, preferred: &[NodeId]) -> Option<V> {
    let mut swap_vals = Vec::new();
    for pid in preferred {
        let Some(target) = cx.doc.get(*pid) else { continue };
        let ty = match target.kind {
            NodeKind::ComponentSet(_) => "STATE_GROUP",
            _ => "COMPONENT",
        };
        let key = fallback_component_key(cx, *pid)?;
        swap_vals.push(msg(vec![("type", en(ty)), ("key", V::String(key))]));
    }
    if swap_vals.is_empty() {
        return None;
    }
    Some(msg(vec![("stringValues", V::Array(Vec::new())), ("instanceSwapValues", V::Array(swap_vals)), ("parameterValues", V::Array(Vec::new()))]))
}

/// `SlotPropConfig`: only the fields the document holds are written.
fn encode_slot_prop_config(cfg: &designos_doc::SlotPropConfig) -> V {
    let mut f: Vec<(&str, V)> = Vec::new();
    if let Some(b) = cfg.stretch_child_on_insert {
        f.push(("stretchChildOnInsert", V::Bool(b)));
    }
    if let Some(b) = cfg.display_by_default {
        f.push(("displayByDefault", V::Bool(b)));
    }
    if let Some(n) = cfg.min_children {
        f.push(("minChildren", V::Uint(n)));
    }
    if let Some(n) = cfg.max_children {
        f.push(("maxChildren", V::Uint(n)));
    }
    if let Some(b) = cfg.allow_preferred_values_only {
        f.push(("allowPreferredValuesOnly", V::Bool(b)));
    }
    msg(f)
}

fn append_fig_metadata(schema: &crate::kiwi::Schema, nc_def: usize, fields: &mut Vec<(std::sync::Arc<str>, V)>, meta: &BTreeMap<String, String>) {
    if meta.is_empty() {
        return;
    }
    for (k, v) in meta {
        if v.is_empty() || fields.iter().any(|(existing, _)| &**existing == k.as_str()) {
            continue;
        }
        if let Some(pair) = schema.def_field_from_metadata(nc_def, k, v) {
            fields.push(pair);
        }
    }
}

fn append_ot_and_variation_fields(fields: &mut Vec<(&'static str, V)>, ts: &TextStyle) {
    if !ts.features.is_empty() {
        let mut on_f = Vec::new();
        let mut off_f = Vec::new();
        let is_on = |name: &str| ts.features.iter().any(|(t, v)| t.eq_ignore_ascii_case(name) && *v != 0);
        for (tag, enabled) in &ts.features {
            let up = tag.to_ascii_uppercase();
            if KNOWN_OT_FEATURES.contains(&up.as_str()) {
                if *enabled != 0 {
                    on_f.push(en(&up));
                } else {
                    off_f.push(en(&up));
                }
            }
        }
        if !on_f.is_empty() {
            fields.push(("toggledOnOTFeatures", V::Array(on_f)));
        }
        if !off_f.is_empty() {
            fields.push(("toggledOffOTFeatures", V::Array(off_f)));
        }
        if is_on("onum") {
            fields.push(("fontVariantNumericFigure", en("OLDSTYLE")));
        } else if is_on("lnum") {
            fields.push(("fontVariantNumericFigure", en("LINING")));
        }
        if is_on("tnum") {
            fields.push(("fontVariantNumericSpacing", en("TABULAR")));
        } else if is_on("pnum") {
            fields.push(("fontVariantNumericSpacing", en("PROPORTIONAL")));
        }
        if is_on("frac") {
            fields.push(("fontVariantNumericFraction", en("DIAGONAL")));
        } else if is_on("afrc") {
            fields.push(("fontVariantNumericFraction", en("STACKED")));
        }
        if is_on("c2sc") {
            fields.push(("fontVariantCaps", en("ALL_SMALL")));
        } else if is_on("smcp") {
            fields.push(("fontVariantCaps", en("SMALL")));
        } else if is_on("c2pc") {
            fields.push(("fontVariantCaps", en("ALL_PETITE")));
        } else if is_on("pcap") {
            fields.push(("fontVariantCaps", en("PETITE")));
        } else if is_on("unic") {
            fields.push(("fontVariantCaps", en("UNICASE")));
        } else if is_on("titl") {
            fields.push(("fontVariantCaps", en("TITLING")));
        }
        if is_on("subs") {
            fields.push(("fontVariantPosition", en("SUB")));
        } else if is_on("sups") {
            fields.push(("fontVariantPosition", en("SUPER")));
        }
    }
    if !ts.variation_axes.is_empty() {
        let vars: Vec<V> = ts
            .variation_axes
            .iter()
            .map(|(tag, val)| {
                let mut bytes = [b' '; 4];
                for (i, b) in tag.as_bytes().iter().take(4).enumerate() {
                    bytes[i] = *b;
                }
                msg(vec![("axisTag", V::Uint(u32::from_be_bytes(bytes))), ("axisName", V::String(tag.clone())), ("value", fl(*val))])
            })
            .collect();
        fields.push(("fontVariations", V::Array(vars)));
    }
}

/// An instance that only inherits its main's effect style and shows effects of its own (the cache Figma wrote for a
/// resized instance) keeps no link of its own in the file. A pin on the record would read as the instance's own link on
/// reopen, and an own link makes the style's list win over a cache that binds no variable of its own, so the render
/// would change with the save. A cache that binds a variable the style lacks is kept either way and keeps its pin.
fn inherits_effect_link_with_own_cache(cx: &ExportCtx<'_>, node: &Node, sid: &str) -> bool {
    let NodeKind::Instance { component: Some(main), .. } = node.kind else { return false };
    if node.component.overrides.contains(&OverrideField::Effects) || cx.doc.get(main).and_then(|m| m.style_refs.get("effect")).map(String::as_str) != Some(sid)
    {
        return false;
    }
    let Some(Style::Effect(fx)) = cx.doc.styles.get(sid) else { return false };
    let style_vars: Vec<&String> =
        cx.doc.style_meta.get(sid).into_iter().flat_map(|m| m.bindings.iter()).filter(|(k, _)| k.starts_with("effects/")).map(|(_, v)| v).collect();
    let own_foreign = node.variables.bindings.iter().filter(|(k, _)| k.starts_with("effects/")).any(|(_, v)| !style_vars.contains(&v));
    *fx != node.effects && !own_foreign
}

/// The style reference an override entry writes for `slot` ("text", "effect"): the relinked style, or the detach
/// marker, whenever the node's effective reference differs from the one its main component carries.
fn instance_style_override(cx: &ExportCtx<'_>, n: &Node, slot: &str) -> Option<V> {
    let source_style = n.component.source.and_then(|source| cx.doc.get(source)).and_then(|source| source.style_refs.get(slot));
    let effective_style = n.style_refs.get(slot);
    if effective_style == source_style {
        return None;
    }
    match effective_style {
        Some(id) if id.starts_with("key:") => Some(msg(vec![("assetRef", msg(vec![("key", V::String(id[4..].to_string()))]))])),
        Some(id) => cx
            .style_guids
            .get(id)
            .copied()
            .map(|(session, local)| msg(vec![("guid", guid(session, local))]))
            .or_else(|| Some(msg(vec![("guid", guid(u32::MAX, u32::MAX))]))),
        None => Some(msg(vec![("guid", guid(u32::MAX, u32::MAX))])),
    }
}

fn append_text_style_fields(fields: &mut Vec<(&'static str, V)>, ts: &TextStyle) {
    let style_str = style_name_from_weight(ts.font_weight, ts.italic);
    fields
        .push(("fontName", st(vec![("family", V::String(ts.font_family.clone())), ("style", V::String(style_str)), ("postscript", V::String(String::new()))])));
    fields.push(("fontSize", fl(ts.font_size)));
    fields.push(("letterSpacing", st(vec![("value", fl(ts.letter_spacing)), ("units", en("PIXELS"))])));
    let lh = match ts.line_height {
        LineHeight::Auto => st(vec![("value", fl(1.0)), ("units", en("RAW"))]),
        LineHeight::Pixels(px) => st(vec![("value", fl(px)), ("units", en("PIXELS"))]),
        LineHeight::Percent(p) => st(vec![("value", fl(p)), ("units", en("PERCENT"))]),
    };
    fields.push(("lineHeight", lh));
    fields.push((
        "textAlignHorizontal",
        en(match ts.align_h {
            TextAlignH::Left => "LEFT",
            TextAlignH::Center => "CENTER",
            TextAlignH::Right => "RIGHT",
            TextAlignH::Justified => "JUSTIFIED",
        }),
    ));
    fields.push((
        "textAlignVertical",
        en(match ts.align_v {
            TextAlignV::Top => "TOP",
            TextAlignV::Center => "CENTER",
            TextAlignV::Bottom => "BOTTOM",
        }),
    ));
    fields.push((
        "textCase",
        en(match ts.case {
            TextCase::Original => "ORIGINAL",
            TextCase::Upper => "UPPER",
            TextCase::Lower => "LOWER",
            TextCase::Title => "TITLE",
            TextCase::SmallCaps => "SMALL_CAPS",
        }),
    ));
    fields.push((
        "textDecoration",
        en(match ts.decoration {
            TextDecoration::None => "NONE",
            TextDecoration::Underline => "UNDERLINE",
            TextDecoration::Strikethrough => "STRIKETHROUGH",
        }),
    ));
    if ts.paragraph_spacing != 0.0 {
        fields.push(("paragraphSpacing", fl(ts.paragraph_spacing)));
    }
    append_ot_and_variation_fields(fields, ts);
}

fn append_shared_text_style_fields(fields: &mut Vec<(&'static str, V)>, ts: &TextStyle) {
    append_text_style_fields(fields, ts);
    if ts.paragraph_indent != 0.0 {
        fields.push(("paragraphIndent", fl(ts.paragraph_indent)));
    }
    if ts.list_spacing != 0.0 {
        fields.push(("listSpacing", fl(ts.list_spacing)));
    }
    if ts.leading_trim == LeadingTrim::CapHeight {
        fields.push(("leadingTrim", en("CAP_HEIGHT")));
    }
    // Alignment is a layer property and must never be serialized as part of a
    // shared text style.
    fields.retain(|(key, _)| *key != "textAlignHorizontal" && *key != "textAlignVertical");
}

fn append_paragraph_fields(fields: &mut Vec<(&'static str, V)>, tp: &TextProps) {
    if tp.paragraph.indent != 0.0 {
        fields.push(("paragraphIndent", fl(tp.paragraph.indent)));
    }
    if tp.paragraph.list_spacing != 0.0 {
        fields.push(("listSpacing", fl(tp.paragraph.list_spacing)));
    }
    if tp.paragraph.hanging_punctuation {
        fields.push(("hangingPunctuation", V::Bool(true)));
    }
    if tp.paragraph.hanging_list {
        fields.push(("hangingList", V::Bool(true)));
    }
    if tp.paragraph.leading_trim == LeadingTrim::CapHeight {
        fields.push(("leadingTrim", en("CAP_HEIGHT")));
    }
}

/// One row of the character style table of a text node.
struct TableRun<'a> {
    start: usize,
    end: usize,
    style: &'a TextStyle,
    /// Set when the row is an override over the layer's text style (Figma's `isOverrideOverTextStyle`):
    /// the weight, italic and decoration it carries, and nothing else of the style.
    over: Option<OwnedFieldSet>,
}

/// The rows of the character style table of `tp`. On a layer linked to a text style (`link` holds its
/// stored overrides), a range that differs from the base style only in weight, italic or decoration is
/// an override row, and the characters outside every range get an override row of the layer's stored
/// overrides: Figma reads the base values of a linked layer from the style, so a layer-wide override has
/// to travel as a range to survive.
fn table_runs(tp: &TextProps, link: Option<OwnedFieldSet>) -> Vec<TableRun<'_>> {
    let stored = link.unwrap_or_default();
    let mut layer_wide = OwnedFieldSet::EMPTY;
    for f in [OwnedField::Weight, OwnedField::Italic, OwnedField::Decoration] {
        if stored.contains(f) {
            layer_wide.insert(f);
        }
    }
    // A range row carries its own difference from the base and the layer-wide overrides, because Figma
    // reads a linked layer's base values from the style: a row that left them out would lose them.
    let mut rows: Vec<TableRun<'_>> = tp
        .runs
        .iter()
        .map(|r| {
            let mut over = designos_doc::text_style_diff(&r.style, &tp.style);
            let overridable_only = !over.has_non_overridable();
            over.union(layer_wide);
            let over = link.filter(|_| overridable_only && !over.is_empty()).map(|_| over);
            TableRun { start: r.start, end: r.end, style: &r.style, over }
        })
        .collect();
    if !layer_wide.is_empty() && !tp.content.is_empty() {
        let len = tp.content.len();
        let mut covered: Vec<(usize, usize)> = tp.runs.iter().map(|r| (r.start.min(len), r.end.min(len))).collect();
        covered.sort_unstable();
        let mut at = 0;
        for (s, e) in covered.into_iter().chain([(len, len)]) {
            if s > at {
                rows.push(TableRun { start: at, end: s, style: &tp.style, over: Some(layer_wide) });
            }
            at = at.max(e);
        }
    }
    rows
}

/// A style override table row that overrides only `over` of the text style: its id, the flag, and the
/// weight (with the font style name and the semantic weight and italic) and decoration it carries.
fn override_row(sid: u32, style: &TextStyle, base: &TextStyle, over: OwnedFieldSet) -> V {
    let mut f = vec![("styleID", V::Uint(sid)), ("isOverrideOverTextStyle", V::Bool(true))];
    if over.contains(OwnedField::Weight) || over.contains(OwnedField::Italic) {
        f.push((
            "fontName",
            st(vec![
                ("family", V::String(style.font_family.clone())),
                ("style", V::String(style_name_from_weight(style.font_weight, style.italic))),
                ("postscript", V::String(String::new())),
            ]),
        ));
        f.push(("semanticWeight", en(if style.font_weight >= 600 { "BOLD" } else { "NORMAL" })));
        f.push(("semanticItalic", en(if style.italic { "ITALIC" } else { "NORMAL" })));
    }
    if over.contains(OwnedField::Decoration) {
        f.push((
            "textDecoration",
            en(match style.decoration {
                TextDecoration::None => "NONE",
                TextDecoration::Underline => "UNDERLINE",
                TextDecoration::Strikethrough => "STRIKETHROUGH",
            }),
        ));
    }
    if style.fills != base.fills {
        f.push(("fillPaints", V::Array(style.fills.iter().map(|p| encode_paint(p, None, None)).collect())));
    }
    msg(f)
}

fn encode_text_data(tp: &TextProps, include_runs: bool, include_lines: bool, link: Option<OwnedFieldSet>) -> V {
    let mut td = vec![("characters", V::String(tp.content.clone()))];
    let rows = table_runs(tp, link);
    if include_runs && !rows.is_empty() {
        let offs = utf16_to_byte_offsets(&tp.content);
        let mut char_ids = vec![0u32; offs.len().saturating_sub(1)];
        let mut table = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            let sid = (i + 1) as u32;
            let s = offs.partition_point(|&o| o < r.start);
            let e = offs.partition_point(|&o| o < r.end);
            let cap = e.min(char_ids.len());
            for slot in char_ids.iter_mut().take(cap).skip(s) {
                *slot = sid;
            }
            if let Some(over) = r.over {
                table.push(override_row(sid, r.style, &tp.style, over));
                continue;
            }
            let st_name = style_name_from_weight(r.style.font_weight, r.style.italic);
            let lh = match r.style.line_height {
                LineHeight::Auto => st(vec![("value", fl(1.0)), ("units", en("RAW"))]),
                LineHeight::Pixels(px) => st(vec![("value", fl(px)), ("units", en("PIXELS"))]),
                LineHeight::Percent(p) => st(vec![("value", fl(p)), ("units", en("PERCENT"))]),
            };
            let mut so_fields = vec![
                ("styleID", V::Uint(sid)),
                ("fontSize", fl(r.style.font_size)),
                (
                    "fontName",
                    st(vec![("family", V::String(r.style.font_family.clone())), ("style", V::String(st_name)), ("postscript", V::String(String::new()))]),
                ),
                ("fillPaints", V::Array(r.style.fills.iter().map(|p| encode_paint(p, None, None)).collect())),
                (
                    "textDecoration",
                    en(match r.style.decoration {
                        TextDecoration::None => "NONE",
                        TextDecoration::Underline => "UNDERLINE",
                        TextDecoration::Strikethrough => "STRIKETHROUGH",
                    }),
                ),
                (
                    "textCase",
                    en(match r.style.case {
                        TextCase::Original => "ORIGINAL",
                        TextCase::Upper => "UPPER",
                        TextCase::Lower => "LOWER",
                        TextCase::Title => "TITLE",
                        TextCase::SmallCaps => "SMALL_CAPS",
                    }),
                ),
                ("letterSpacing", st(vec![("value", fl(r.style.letter_spacing)), ("units", en("PIXELS"))])),
                ("lineHeight", lh),
            ];
            append_ot_and_variation_fields(&mut so_fields, r.style);
            table.push(msg(so_fields));
        }
        td.push(("characterStyleIDs", V::Array(char_ids.into_iter().map(V::Uint).collect())));
        td.push(("styleOverrideTable", V::Array(table)));
    }
    if include_lines && (tp.has_lists() || tp.direction != TextDirection::Auto) {
        let dir = match tp.direction {
            TextDirection::Auto => "AUTO",
            TextDirection::Ltr => "LTR",
            TextDirection::Rtl => "RTL",
        };
        let lists = tp.paragraph_lists();
        let lines: Vec<V> = lists
            .iter()
            .map(|item| {
                let (lty, indent) = match item {
                    None => ("PLAIN", 0u32),
                    Some(ListItem { kind: ListKind::Numbered, level }) => ("ORDERED_LIST", *level as u32 + 1),
                    Some(ListItem { kind: ListKind::Bullet, level }) => ("UNORDERED_LIST", *level as u32 + 1),
                };
                msg(vec![("lineType", en(lty)), ("indentationLevel", V::Int(indent as i32)), ("sourceDirectionality", en(dir))])
            })
            .collect();
        if !lines.is_empty() {
            td.push(("lines", V::Array(lines)));
        }
    }
    msg(td)
}

/// `link` holds the stored overrides of a layer linked to a text style (`None` for any other text).
fn encode_text_node(fields: &mut Vec<(&'static str, V)>, tp: &TextProps, link: Option<OwnedFieldSet>) {
    append_text_style_fields(fields, &tp.style);
    fields.push(("autoRename", V::Bool(tp.auto_rename)));
    fields.push(("textDecorationSkipInk", V::Bool(tp.decoration_skip_ink)));
    match tp.text_wrap {
        TextWrap::Auto => {}
        TextWrap::Balance => fields.push(("textWrapStyle", en("BALANCE"))),
        TextWrap::Pretty => fields.push(("textWrapStyle", en("PRETTY"))),
    }
    fields.push((
        "textAutoResize",
        en(match tp.auto_resize {
            TextAutoResize::None => "NONE",
            TextAutoResize::WidthAndHeight => "WIDTH_AND_HEIGHT",
            TextAutoResize::Height => "HEIGHT",
            // Figma has no `TRUNCATE` resize mode: a truncating text is fixed in size and says so in `textTruncation`.
            TextAutoResize::Truncate => "NONE",
        }),
    ));
    if tp.auto_resize == TextAutoResize::Truncate {
        fields.push(("textTruncation", en("ENDING")));
    }
    if let Some(m) = tp.max_lines {
        fields.push(("maxLines", V::Int(i32::try_from(m).unwrap_or(i32::MAX))));
    }
    append_paragraph_fields(fields, tp);
    fields.push(("textData", encode_text_data(tp, true, true, link)));
}

fn blend_name(b: BlendMode) -> &'static str {
    match b {
        BlendMode::Normal => "NORMAL",
        BlendMode::PassThrough => "PASS_THROUGH",
        BlendMode::Darken => "DARKEN",
        BlendMode::Multiply => "MULTIPLY",
        BlendMode::ColorBurn => "COLOR_BURN",
        BlendMode::LinearBurn => "LINEAR_BURN",
        BlendMode::Lighten => "LIGHTEN",
        BlendMode::Screen => "SCREEN",
        BlendMode::ColorDodge => "COLOR_DODGE",
        BlendMode::LinearDodge => "LINEAR_DODGE",
        BlendMode::Overlay => "OVERLAY",
        BlendMode::SoftLight => "SOFT_LIGHT",
        BlendMode::HardLight => "HARD_LIGHT",
        BlendMode::Difference => "DIFFERENCE",
        BlendMode::Exclusion => "EXCLUSION",
        BlendMode::Hue => "HUE",
        BlendMode::Saturation => "SATURATION",
        BlendMode::Color => "COLOR",
        BlendMode::Luminosity => "LUMINOSITY",
    }
}

#[cfg(test)]
mod parked_override_export_tests {
    use super::*;

    #[test]
    fn parked_overrides_are_never_written_to_exported_fig_bytes() {
        let mut d = Document::new("FigParkedOverrideSkip");
        let page = d.add_page("Page 1");

        let mk_variant = |d: &mut Document, name: &str, x: f32, with_sub: bool| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 120.0, 60.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let mut title = Node::text(tid, Rect::new(8.0, 8.0, 80.0, 16.0), "Title", TextStyle::default());
            title.name = "Title".into();
            d.insert(comp, title).unwrap();
            if with_sub {
                let sid = d.alloc_id();
                let mut sub = Node::text(sid, Rect::new(8.0, 28.0, 80.0, 16.0), "Default subtitle", TextStyle::default());
                sub.name = "Subtitle".into();
                d.insert(comp, sub).unwrap();
            }
            create_component(d, comp).apply(d).unwrap();
            comp
        };

        let a = mk_variant(&mut d, "State=A", 0.0, true);
        let b = mk_variant(&mut d, "State=B", 160.0, false);
        combine_as_variants(&mut d, &[a, b]).apply(&mut d).unwrap();

        create_instance(&mut d, a, page, Vec2::new(0.0, 120.0)).apply(&mut d).unwrap();
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let sub = *d.get(inst).unwrap().children.iter().find(|&&c| d.get(c).unwrap().name == "Subtitle").unwrap();
        let mut sn = d.get(sub).unwrap().clone();
        if let NodeKind::Text(tp) = &mut sn.kind {
            tp.content = "Parked subtitle text".into();
        }
        sn.component.overrides.insert(OverrideField::Characters);
        Command::Update { node: Box::new(sn) }.apply(&mut d).unwrap();

        set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("B".into()))).apply(&mut d).unwrap();
        assert_eq!(d.get(inst).unwrap().component.parked_overrides.len(), 1);

        let mut clean = d.clone();
        clean.get_mut(inst).unwrap().component.parked_overrides.clear();

        let bytes_with_parked = export_fig(&d).unwrap();
        let bytes_without_parked = export_fig(&clean).unwrap();
        assert_eq!(bytes_with_parked, bytes_without_parked);

        let pen_with_parked = crate::export_pen_json(&d).unwrap();
        let pen_without_parked = crate::export_pen_json(&clean).unwrap();
        assert_eq!(pen_with_parked, pen_without_parked, "pen export omits parked overrides and writes only what the instance shows");
        assert!(!pen_with_parked.contains("parked_overrides"));
        assert!(!pen_with_parked.contains("Parked subtitle text"));

        let rest_with_parked = crate::export_rest_json(&d).to_string();
        let rest_without_parked = crate::export_rest_json(&clean).to_string();
        assert_eq!(rest_with_parked, rest_without_parked, "REST export omits parked overrides and writes only what the instance shows");
        assert!(!rest_with_parked.contains("parked_overrides"));
        assert!(!rest_with_parked.contains("Parked subtitle text"));

        let native_json = d.to_json().unwrap();
        assert!(native_json.contains("parked_overrides"), "native file writes parked_overrides");
    }
}
