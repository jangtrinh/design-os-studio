//! `.fig` (fig-kiwi) binary writer: [`Document`] -> `.fig` archive bytes.
//!
//! Produces a standard Figma local-copy ZIP archive (`canvas.fig` + `meta.json` +
//! `images/<hash>`) whose `canvas.fig` payload is a `fig-kiwi` container encoded
//! against [`crate::testkit::figma_like_schema`].

use crate::container::{self, Compression};
use crate::kiwi::KiwiValue as V;
use crate::rest_import::{grid_child_align_name, utf16_to_byte_offsets, writes_grid_child_align};
use crate::testkit::{self, en, figma_like_schema, guid, msg, st};
use crate::FigError;
use designos_doc::*;
use serde_json::json;
use std::collections::{BTreeMap, HashSet};

mod carrier;
mod grid;
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

/// Serializes `doc` into a `.fig` binary archive (ZIP containing `canvas.fig`,
/// `meta.json`, and `images/<hash>`).
pub fn export_fig(doc: &Document) -> Result<Vec<u8>, FigError> {
    let schema = figma_like_schema();
    let root = schema.find("Message").ok_or_else(|| FigError::Format("missing Message in schema".into()))?;
    let message = build_message(doc, &schema);
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

// ------------------------------------------------------------------ state & IDs

struct ExportCtx<'a> {
    doc: &'a Document,
    schema: &'a crate::kiwi::Schema,
    nc_def: usize,
    used_guids: HashSet<(u32, u32)>,
    next_local: u32,
    node_guids: BTreeMap<NodeId, (u32, u32)>,
    coll_guids: BTreeMap<String, (u32, u32)>,
    mode_guids: BTreeMap<String, (u32, u32)>,
    var_guids: BTreeMap<String, (u32, u32)>,
    style_guids: BTreeMap<String, (u32, u32)>,
    prop_guids: BTreeMap<(NodeId, String), (u32, u32)>,
    comp_keys: BTreeMap<NodeId, String>,
    grid_tracks: BTreeMap<NodeId, grid::GridTrackIds>,
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
    if session == 0 && (local == 0 || local == 999_999) {
        return None;
    }
    Some((session, local))
}

impl<'a> ExportCtx<'a> {
    fn new(doc: &'a Document, schema: &'a crate::kiwi::Schema) -> Self {
        let mut used = HashSet::new();
        used.insert((0, 0));
        used.insert((0, 999_999));
        let nc_def = schema.find("NodeChange").unwrap_or(0);
        let mut cx = Self {
            doc,
            schema,
            nc_def,
            used_guids: used,
            next_local: 1,
            node_guids: BTreeMap::new(),
            coll_guids: BTreeMap::new(),
            mode_guids: BTreeMap::new(),
            var_guids: BTreeMap::new(),
            style_guids: BTreeMap::new(),
            prop_guids: BTreeMap::new(),
            comp_keys: BTreeMap::new(),
            grid_tracks: BTreeMap::new(),
            blobs: Vec::new(),
            changes: Vec::new(),
        };
        // First pass: reserve explicit `s:l` GUIDs so generated ones never collide.
        for (&id, n) in &doc.nodes {
            if doc.library_page.is_some_and(|lp| n.parent == Some(lp)) && n.source_id.as_deref().is_some_and(|sid| doc.styles.contains_key(sid)) {
                continue;
            }
            if let Some(g) = n.source_id.as_deref().and_then(parse_guid_pair) {
                if cx.used_guids.insert(g) {
                    cx.next_local = cx.next_local.max(g.1.saturating_add(1));
                    cx.node_guids.insert(id, g);
                }
            }
        }
        for id in doc.variable_collections.keys() {
            if let Some(g) = parse_guid_pair(id) {
                if cx.used_guids.insert(g) {
                    cx.next_local = cx.next_local.max(g.1.saturating_add(1));
                    cx.coll_guids.insert(id.clone(), g);
                }
            }
        }
        for c in doc.variable_collections.values() {
            for m in &c.modes {
                if let Some(g) = parse_guid_pair(&m.id) {
                    if cx.used_guids.insert(g) {
                        cx.next_local = cx.next_local.max(g.1.saturating_add(1));
                        cx.mode_guids.insert(m.id.clone(), g);
                    }
                }
            }
        }
        for id in doc.variables.keys() {
            if let Some(g) = parse_guid_pair(id) {
                if cx.used_guids.insert(g) {
                    cx.next_local = cx.next_local.max(g.1.saturating_add(1));
                    cx.var_guids.insert(id.clone(), g);
                }
            }
        }
        for id in doc.styles.keys() {
            if let Some(g) = parse_guid_pair(id) {
                if cx.used_guids.insert(g) {
                    cx.next_local = cx.next_local.max(g.1.saturating_add(1));
                    cx.style_guids.insert(id.clone(), g);
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
        for (&id, n) in &doc.nodes {
            if matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) && !cx.comp_keys.contains_key(&id) {
                let g = cx.node_guids[&id];
                cx.comp_keys.insert(id, format!("{}:{}", g.0, g.1));
            }
        }
        grid::allocate(&mut cx);
        cx
    }

    fn alloc_guid(&mut self, session: u32) -> (u32, u32) {
        loop {
            let g = (session, self.next_local);
            self.next_local += 1;
            if self.used_guids.insert(g) {
                return g;
            }
        }
    }

    fn add_blob(&mut self, bytes: Vec<u8>) -> u32 {
        let idx = self.blobs.len() as u32;
        self.blobs.push(bytes);
        idx
    }

    /// `Paint.opacityVar` / `Effect.effectOpacityVar` for the composed-opacity binding `key`
    /// (`fills/0/opacity`, `strokes/0/opacity`, `effects/0/opacity`).
    fn opacity_var_data(&self, vars: &NodeVariables, key: &str) -> Option<V> {
        let vg = self.resolve_var_guid(vars.bindings.get(key)?)?;
        Some(alias_var_data(vg))
    }

    fn resolve_var_guid(&self, id: &str) -> Option<(u32, u32)> {
        self.var_guids.get(id).copied().or_else(|| self.var_guids.get(&format!("VariableID:{id}")).copied()).or_else(|| parse_guid_pair(id))
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
    cx.changes.push(msg(vec![("guid", guid(0, 0)), ("phase", en("CREATED")), ("type", en("DOCUMENT")), ("name", V::String(doc.name.clone()))]));

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
    let has_internal = !doc.variable_collections.is_empty() || !doc.variables.is_empty() || !doc.styles.is_empty() || doc.library_page.is_some();
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
    }

    let blobs: Vec<V> = cx.blobs.into_iter().map(|b| st(vec![("bytes", V::Bytes(b))])).collect();
    msg(vec![("type", en("NODE_CHANGES")), ("sessionID", V::Uint(0)), ("ackID", V::Uint(0)), ("nodeChanges", V::Array(cx.changes)), ("blobs", V::Array(blobs))])
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
        let modes: Vec<V> = ordered_modes
            .iter()
            .enumerate()
            .map(|(mi, m)| {
                let mguid = cx.mode_guids[&m.id];
                msg(vec![("id", guid(mguid.0, mguid.1)), ("name", V::String(m.name.clone())), ("sortPosition", V::String(pos_str(mi)))])
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
        cx.changes.push(msg(fields));
        *idx += 1;
    }

    for (vi, v) in cx.doc.variables.values().enumerate() {
        let vguid = cx.var_guids[&v.id];
        let Some(cguid) = cx.resolve_coll_guid(&v.collection) else { continue };
        let rtype = match v.kind {
            VariableKind::Color => "COLOR",
            VariableKind::Number => "FLOAT",
            VariableKind::String => "STRING",
            VariableKind::Boolean => "BOOLEAN",
        };
        let mut val_entries = Vec::new();
        for (mode_id, val) in &v.values {
            let Some(&mguid) = cx.mode_guids.get(mode_id) else { continue };
            let vdata = match val {
                VariableValue::Color(c) => msg(vec![("dataType", en("COLOR")), ("value", msg(vec![("colorValue", color(c.r, c.g, c.b, c.a))]))]),
                VariableValue::Number(f) => msg(vec![("dataType", en("FLOAT")), ("value", msg(vec![("floatValue", fl(*f))]))]),
                VariableValue::String(s) => msg(vec![("dataType", en("STRING")), ("value", msg(vec![("textValue", V::String(s.clone()))]))]),
                VariableValue::Boolean(b) => msg(vec![("dataType", en("BOOLEAN")), ("value", msg(vec![("boolValue", V::Bool(*b))]))]),
                VariableValue::Alias(target) => {
                    let Some(tguid) = cx.resolve_var_guid(target) else { continue };
                    msg(vec![("dataType", en("ALIAS")), ("value", msg(vec![("alias", msg(vec![("guid", guid(tguid.0, tguid.1))]))]))])
                }
            };
            val_entries.push(msg(vec![("modeID", guid(mguid.0, mguid.1)), ("variableData", vdata)]));
        }
        let scopes: Vec<V> = v.scopes.iter().filter(|s| cx.schema.has_enum_variant("VariableScope", s)).map(|s| en(s)).collect();
        let mut fields = vec![
            ("guid", guid(vguid.0, vguid.1)),
            ("phase", en("CREATED")),
            ("parentIndex", st(vec![("guid", guid(internal_guid.0, internal_guid.1)), ("position", V::String(pos_str(*idx)))])),
            ("type", en("VARIABLE")),
            ("name", V::String(v.name.clone())),
            ("description", V::String(v.description.clone())),
            ("sortPosition", V::String(pos_str(vi))),
            ("variableSetID", msg(vec![("guid", guid(cguid.0, cguid.1))])),
            ("variableResolvedType", en(rtype)),
            ("variableDataValues", msg(vec![("entries", V::Array(val_entries))])),
        ];
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
        cx.changes.push(msg(fields));
        *idx += 1;
    }
}

fn export_styles_on_internal(cx: &mut ExportCtx, internal_guid: (u32, u32), idx: &mut usize) {
    let styles: Vec<(String, Style)> = cx.doc.styles.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
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
        match &style {
            Style::Fill(paints) => {
                fields
                    .push(("fillPaints", V::Array(paints.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "fills", i, bindings)).collect())));
            }
            Style::Text(ts) => {
                append_text_style_fields(&mut fields, ts);
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
        NodeKind::Connector(_) => ("CONNECTOR", false),
        NodeKind::Stamp { .. } => ("STAMP", false),
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
        .filter(|(full_key, _)| !is_studio_carrier_key(full_key))
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
    if let Some(src) = node.component.source.and_then(|s| cx.node_guids.get(&s).copied()) {
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

    let stroke_paints: Vec<V> =
        node.stroke.paints.iter().enumerate().map(|(i, p)| encode_paint_with_bindings(cx, p, "strokes", i, &node.variables.bindings)).collect();
    fields.push(("strokePaints", V::Array(stroke_paints)));
    fields.push(("strokeWeight", fl(node.stroke.weight)));
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
            StrokeCap::None => "NONE",
            StrokeCap::Round => "ROUND",
            StrokeCap::Square => "SQUARE",
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
            if let Some(&sg) = cx.style_guids.get(sid) {
                fields.push((field_name, msg(vec![("guid", guid(sg.0, sg.1))])));
            } else if let Some(key) = sid.strip_prefix("key:") {
                fields.push((field_name, msg(vec![("assetRef", msg(vec![("key", V::String(key.to_string()))]))])));
            }
        }
    }

    // Node variable bindings & explicit modes.
    let var_entries = encode_node_var_entries(cx, &node.variables);
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
                        df.push(("slotPropConfig", encode_slot_prop_config(cfg)));
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
            prop_defs.push(msg(df));
        }
        fields.push(("componentPropDefs", V::Array(prop_defs)));
        if !variant_orders.is_empty() {
            fields.push(("stateGroupPropertyValueOrders", V::Array(variant_orders)));
        }
    }
    if !node.component.values.is_empty() || (!node.component.slot_content_ids.is_empty() && node.component.props.is_empty()) {
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
            }
            prop_assigns.push(msg(assign));
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
            NodeKind::Vector { paths, .. } if !paths.is_empty() => Some(paths),
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
            let sym_overrides = collect_instance_overrides(cx, id);
            if !sym_overrides.is_empty() {
                sd.push(("symbolOverrides", V::Array(sym_overrides)));
            }
            if !sd.is_empty() {
                fields.push(("symbolData", msg(sd)));
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
            encode_text_node(&mut fields, tp);
        }
        NodeKind::Sticky(sp) => {
            encode_text_node(&mut fields, &sp.text);
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
            encode_text_node(&mut fields, &sp.text);
        }
        NodeKind::Connector(cp) => {
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
            let end_msg = |ep: &ConnectorEnd| {
                let mut ef = vec![("position", vec2(ep.point.x, ep.point.y)), ("magnet", en(mag_name(ep.magnet)))];
                if let Some(tg) = ep.node.and_then(|nid| cx.node_guids.get(&nid).copied()) {
                    ef.push(("endpointNodeID", guid(tg.0, tg.1)));
                }
                msg(ef)
            };
            fields.push(("connectorStart", end_msg(&cp.start)));
            fields.push(("connectorEnd", end_msg(&cp.end)));
            if let Some(lbl) = &cp.label {
                encode_text_node(&mut fields, lbl);
            }
        }
        _ => {}
    }

    pd_entries.extend(carrier::carrier_entries(&carried));
    if !pd_entries.is_empty() {
        fields.push(("pluginData", V::Array(pd_entries)));
    }
    let mut msg_fields: Vec<(std::sync::Arc<str>, V)> = fields.into_iter().map(|(k, v)| (k.into(), v)).collect();
    append_fig_metadata(cx.schema, cx.nc_def, &mut msg_fields, &node.fig_metadata);
    cx.changes.push(V::Message(msg_fields));

    let child_in_comp = in_component || matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_));
    for (cidx, cid) in node.children.into_iter().enumerate() {
        export_node_rec(cx, cid, nguid, cidx, my_dir, child_in_comp);
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

fn encode_node_var_entries(cx: &ExportCtx, vars: &NodeVariables) -> Vec<V> {
    encode_var_entries_from_bindings(cx, &vars.bindings)
}

fn encode_var_entries_from_bindings(cx: &ExportCtx, bindings: &BTreeMap<String, String>) -> Vec<V> {
    let mut var_entries = Vec::new();
    let mut font_style_done = false;
    for (k, vid) in bindings {
        if k.starts_with("fills/") || k.starts_with("strokes/") || k.starts_with("effects/") {
            continue;
        }
        if k == "fontWeight" || k == "fontStyle" {
            if font_style_done {
                continue;
            }
            font_style_done = true;
            let mut fs_fields = Vec::new();
            if let Some(vg) = bindings.get("fontStyle").and_then(|v| cx.resolve_var_guid(v)) {
                fs_fields.push(("asString", alias_var_data(vg)));
            }
            if let Some(vg) = bindings.get("fontWeight").and_then(|v| cx.resolve_var_guid(v)) {
                fs_fields.push(("asFloat", alias_var_data(vg)));
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
        var_entries.push(msg(vec![("variableField", en(vfield)), ("variableData", alias_var_data(vg))]));
    }
    var_entries
}

fn encode_node_mode_entries(cx: &ExportCtx, vars: &NodeVariables) -> Vec<V> {
    let mut mode_entries = Vec::new();
    for (cid, mid) in &vars.modes {
        let Some(cg) = cx.resolve_coll_guid(cid) else { continue };
        let Some(&mg) = cx.mode_guids.get(mid) else { continue };
        mode_entries.push(msg(vec![("variableSetID", msg(vec![("guid", guid(cg.0, cg.1))])), ("variableModeID", guid(mg.0, mg.1))]));
    }
    mode_entries
}

fn collect_instance_overrides(cx: &ExportCtx, inst_id: NodeId) -> Vec<V> {
    let mut out = Vec::new();
    let Some(inst) = cx.doc.get(inst_id) else { return out };
    let mut stack: Vec<(NodeId, Vec<(u32, u32)>)> = inst.children.iter().rev().map(|&cid| (cid, Vec::new())).collect();

    while let Some((cid, mut path)) = stack.pop() {
        let Some(n) = cx.doc.get(cid) else { continue };
        let step_g = n.component.source.and_then(|s| cx.node_guids.get(&s).copied()).or_else(|| cx.node_guids.get(&cid).copied()).unwrap_or((1, cid.0 as u32));
        path.push(step_g);

        if !n.component.overrides.is_empty()
            || !n.variables.bindings.is_empty()
            || !n.variables.modes.is_empty()
            || n.component.override_level.is_some()
            || n.component.detached_symbol_id.is_some()
        {
            let mut ov: Vec<(&'static str, V)> = Vec::new();
            let mut carried: carrier::Carried = Vec::new();
            let guids: Vec<V> = path.iter().map(|&(s, l)| guid(s, l)).collect();
            ov.push(("guidPath", msg(vec![("guids", V::Array(guids))])));
            if let Some(lvl) = n.component.override_level {
                ov.push(("overrideLevel", V::Int(lvl)));
            }
            if let Some(ds) = n.component.detached_symbol_id.as_deref().or(n.detached_symbol_id.as_deref()).filter(|s| !s.is_empty()) {
                ov.push(("detachedSymbolId", encode_detached_symbol_id(ds)));
            }

            let ovs = &n.component.overrides;
            if ovs.contains(&OverrideField::Name) {
                ov.push(("name", V::String(n.name.clone())));
            }
            if ovs.contains(&OverrideField::Visible) {
                ov.push(("visible", V::Bool(n.visible)));
            }
            if ovs.contains(&OverrideField::Locked) {
                ov.push(("locked", V::Bool(n.locked)));
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
            let has_fill_var = n.variables.bindings.keys().any(|k| k.starts_with("fills/"));
            if ovs.contains(&OverrideField::Fills) || has_fill_var {
                let fills_slice = match &n.kind {
                    NodeKind::Text(t) => &t.style.fills,
                    _ => &n.fills,
                };
                let fps: Vec<V> = fills_slice
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let cv = n.variables.bindings.get(&format!("fills/{i}")).and_then(|vid| cx.resolve_var_guid(vid)).map(alias_var_data);
                        encode_paint(p, cv, cx.opacity_var_data(&n.variables, &format!("fills/{i}/opacity")))
                    })
                    .collect();
                ov.push(("fillPaints", V::Array(fps)));
            }
            let has_stroke_var = n.variables.bindings.keys().any(|k| k.starts_with("strokes/"));
            if ovs.contains(&OverrideField::Stroke) || has_stroke_var {
                let sps: Vec<V> = n
                    .stroke
                    .paints
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let cv = n.variables.bindings.get(&format!("strokes/{i}")).and_then(|vid| cx.resolve_var_guid(vid)).map(alias_var_data);
                        encode_paint(p, cv, cx.opacity_var_data(&n.variables, &format!("strokes/{i}/opacity")))
                    })
                    .collect();
                ov.push(("strokePaints", V::Array(sps)));
                ov.push(("strokeWeight", fl(n.stroke.weight)));
                if let Some([top, right, bottom, left]) = n.stroke.side_weights {
                    ov.push(("borderStrokeWeightsIndependent", V::Bool(true)));
                    ov.push(("borderTopWeight", fl(top)));
                    ov.push(("borderRightWeight", fl(right)));
                    ov.push(("borderBottomWeight", fl(bottom)));
                    ov.push(("borderLeftWeight", fl(left)));
                }
            }
            if ovs.contains(&OverrideField::Effects) {
                let fx: Vec<V> = n
                    .effects
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        let cv = n.variables.bindings.get(&format!("effects/{i}/color")).and_then(|vid| cx.resolve_var_guid(vid)).map(alias_var_data);
                        encode_effect(e, cv, cx.opacity_var_data(&n.variables, &format!("effects/{i}/opacity")))
                    })
                    .collect();
                ov.push(("effects", V::Array(fx)));
            }
            if ovs.contains(&OverrideField::CornerRadii) {
                let r = n.corner_radii;
                ov.push(("cornerRadius", fl(r[0])));
                if !r.iter().all(|&x| (x - r[0]).abs() < 1e-5) {
                    ov.push(("rectangleCornerRadiiIndependent", V::Bool(true)));
                    ov.push(("rectangleTopLeftCornerRadius", fl(r[0])));
                    ov.push(("rectangleTopRightCornerRadius", fl(r[1])));
                    ov.push(("rectangleBottomRightCornerRadius", fl(r[2])));
                    ov.push(("rectangleBottomLeftCornerRadius", fl(r[3])));
                }
            }
            if ovs.contains(&OverrideField::Characters) {
                if let NodeKind::Text(tp) = &n.kind {
                    ov.push(("textData", msg(vec![("characters", V::String(tp.content.clone()))])));
                }
            }
            if ovs.contains(&OverrideField::TextStyle) {
                if let NodeKind::Text(tp) = &n.kind {
                    append_text_style_fields(&mut ov, &tp.style);
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
            if ovs.contains(&OverrideField::MainComponent) {
                if let NodeKind::Instance { component: Some(comp_id), .. } = &n.kind {
                    if let Some(cg) = cx.node_guids.get(comp_id).copied() {
                        ov.push(("overriddenSymbolID", guid(cg.0, cg.1)));
                    }
                }
            }
            let ve = encode_node_var_entries(cx, &n.variables);
            if !ve.is_empty() {
                ov.push(("variableConsumptionMap", msg(vec![("entries", V::Array(ve))])));
            }
            let me = encode_node_mode_entries(cx, &n.variables);
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

        for &child in n.children.iter().rev() {
            stack.push((child, path.clone()));
        }
    }
    out
}

/// Preferred components of an instance-swap or slot property: Figma lists them as `COMPONENT` entries keyed by
/// the component's library key. A component of this file has no library key, so its key is its GUID as `"s:l"`.
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
                so.push((
                    "strokeCap",
                    en(match c {
                        StrokeCap::None => "NONE",
                        StrokeCap::Round => "ROUND",
                        StrokeCap::Square => "SQUARE",
                    }),
                ));
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
        PaintKind::Image { image_ref, scale_mode } => {
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
    msg(fields)
}

fn encode_effect(e: &Effect, color_var: Option<V>, opacity_var: Option<V>) -> V {
    encode_effect_full(e, color_var, opacity_var, None, None, None, None)
}

fn encode_effect_with_bindings(cx: &ExportCtx, e: &Effect, idx: usize, bindings: &BTreeMap<String, String>) -> V {
    let var_data = |prop: &str| -> Option<V> {
        let vid = bindings.get(&format!("effects/{idx}/{prop}"))?;
        let vg = cx.resolve_var_guid(vid)?;
        Some(alias_var_data(vg))
    };
    encode_effect_full(e, var_data("color"), var_data("opacity"), var_data("radius"), var_data("spread"), var_data("offsetX"), var_data("offsetY"))
}

fn encode_effect_full(
    e: &Effect,
    color_var: Option<V>,
    opacity_var: Option<V>,
    radius_var: Option<V>,
    spread_var: Option<V>,
    x_var: Option<V>,
    y_var: Option<V>,
) -> V {
    let mut fields = match e {
        Effect::DropShadow { color: c, offset, radius, spread, visible, show_behind } => vec![
            ("type", en("DROP_SHADOW")),
            ("color", color(c.r, c.g, c.b, c.a)),
            ("offset", vec2(offset.x, offset.y)),
            ("radius", fl(*radius)),
            ("spread", fl(*spread)),
            ("visible", V::Bool(*visible)),
            ("showShadowBehindNode", V::Bool(*show_behind)),
        ],
        Effect::InnerShadow { color: c, offset, radius, spread, visible } => vec![
            ("type", en("INNER_SHADOW")),
            ("color", color(c.r, c.g, c.b, c.a)),
            ("offset", vec2(offset.x, offset.y)),
            ("radius", fl(*radius)),
            ("spread", fl(*spread)),
            ("visible", V::Bool(*visible)),
        ],
        Effect::LayerBlur { radius, visible } => vec![("type", en("FOREGROUND_BLUR")), ("radius", fl(*radius)), ("visible", V::Bool(*visible))],
        Effect::BackgroundBlur { radius, visible } => vec![("type", en("BACKGROUND_BLUR")), ("radius", fl(*radius)), ("visible", V::Bool(*visible))],
        Effect::Noise(n) => {
            let mut f = vec![
                ("type", en("NOISE")),
                ("noiseSize", vec2(n.size, n.size)),
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
            ("noiseSize", vec2(t.size, t.size)),
            ("radius", fl(t.radius)),
            ("clipToShape", V::Bool(t.clip_to_shape)),
            ("seed", V::Uint(t.seed)),
            ("visible", V::Bool(t.visible)),
        ],
        Effect::Glass(g) => vec![
            ("type", en("GLASS")),
            ("specularAngle", fl(g.light_angle)),
            ("specularIntensity", fl(g.light_intensity)),
            ("refractionIntensity", fl(g.refraction)),
            ("refractionRadius", fl(g.depth)),
            ("chromaticAberration", fl(g.dispersion)),
            ("radius", fl(g.frost)),
            ("visible", V::Bool(g.visible)),
        ],
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
    msg(fields)
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
        let key = cx.comp_keys.get(pid).cloned().or_else(|| cx.node_guids.get(pid).map(|g| format!("{}:{}", g.0, g.1)))?;
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

fn encode_text_node(fields: &mut Vec<(&'static str, V)>, tp: &TextProps) {
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

    let mut td = vec![("characters", V::String(tp.content.clone()))];
    if !tp.runs.is_empty() {
        let offs = utf16_to_byte_offsets(&tp.content);
        let mut char_ids = vec![0u32; offs.len().saturating_sub(1)];
        let mut table = Vec::new();
        for (i, r) in tp.runs.iter().enumerate() {
            let sid = (i + 1) as u32;
            let s = offs.partition_point(|&o| o < r.start);
            let e = offs.partition_point(|&o| o < r.end);
            let cap = e.min(char_ids.len());
            for slot in char_ids.iter_mut().take(cap).skip(s) {
                *slot = sid;
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
            append_ot_and_variation_fields(&mut so_fields, &r.style);
            table.push(msg(so_fields));
        }
        td.push(("characterStyleIDs", V::Array(char_ids.into_iter().map(V::Uint).collect())));
        td.push(("styleOverrideTable", V::Array(table)));
    }
    if tp.has_lists() || tp.direction != TextDirection::Auto {
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
    fields.push(("textData", msg(td)));
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
