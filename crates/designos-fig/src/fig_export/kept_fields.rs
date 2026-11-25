//! Figma's own fields of an unedited layer, written back by a `.fig` save.
//!
//! A `.fig` open keeps every field Figma wrote for a layer, as wire bytes (see `designos_doc::FigKeptStore`). Studio models
//! only part of them, and writes its own form of the part it models, so a save of a layer nobody touched lost or changed
//! them. For a layer whose value still equals the value it had when the file was opened, each kept field takes the place of
//! the field Studio wrote under the same name: the whole value of one or the whole value of the other, never a mix. A layer
//! that was edited, or whose record is not the one written under its guid, is written exactly as before.
//!
//! A kept field that holds no id and no blob is copied into the output as it is. A field that holds one is decoded, each id is
//! checked against what this save writes (a reference to a node, a variable, a mode, a style or an image that this save does
//! not write drops the field), and a blob is added to this save's table.
//!
//! What stays Studio's, for now, is listed in [`NOT_WRITTEN_FROM_THE_STORE`].

use super::{is_attached, parse_guid_pair, ExportCtx};
use crate::kiwi::{DefKind, FieldType, KiwiValue as V, Native, Schema};
use designos_doc::{Document, KeptRecordKind, KeptRecordRef, NodeId};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

/// Fields a save does not write from the store, so Studio's own value (or none) is written as it was before.
///
/// - The override key: Studio's writer gives it from the record (see [`kept_override_key`]), edited layer or not.
/// - What an instance carries (its overrides and the data Figma derived for it): it is gated on the instance's content and on
///   the component it names, not on the instance alone.
/// - Outlines and vector networks, which name blobs and which Studio re-encodes.
/// - Prototype interactions, which name other layers and have no writer.
/// - What Studio writes from outside the layer: guides (kept in the document), plugin data (which also carries the in-progress
///   badge and Studio's own carrier entries), the effect style link of an instance (written from the main's link and the style's
///   content), and the child layout keys that read the parent (the parent's direction, the ids of its grid tracks). A layer
///   whose own value is unchanged can still be written differently after its parent or its style was edited.
/// - The grid track definitions (`gridColumns`, `gridRows` and their sizings): Studio gives every track a fresh id on every save
///   and writes the anchors of the children with them, so the definitions and the anchors have to come from the same writer.
/// - Every field whose name starts with `derived`: a cache of the layer's content, its children or its main.
pub(crate) const NOT_WRITTEN_FROM_THE_STORE: &[&str] = &[
    "overrideKey",
    "symbolData",
    "fillGeometry",
    "strokeGeometry",
    "vectorData",
    "prototypeInteractions",
    "scrollBehavior",
    "guides",
    "pluginData",
    "styleIdForEffect",
    "stackChildPrimaryGrow",
    "stackChildAlignSelf",
    "gridChildHorizontalAlign",
    "gridChildVerticalAlign",
    "gridColumnAnchor",
    "gridRowAnchor",
    "gridColumns",
    "gridRows",
    "gridColumnsSizing",
    "gridRowsSizing",
];

/// Whether a field named `name` is written from the store.
fn is_written_from_the_store(name: &str) -> bool {
    !NOT_WRITTEN_FROM_THE_STORE.contains(&name) && !name.starts_with("derived")
}

/// A field that holds a number of the file's blob table.
const BLOB_FIELDS: &[&str] = &["commandsBlob", "vectorNetworkBlob", "dataBlob"];

/// Fields that hold an id as a string: a node id (`"session:local"`) or the hash of media.
const STRING_ID_FIELDS: &[&str] = &["nodeId", "mediaHash"];

/// The edited layers a report names.
const MAX_EDITED_SAMPLES: usize = 20;

/// What one save wrote from the store, counted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeptWriteReport {
    /// Layers with at least one kept field written.
    pub nodes_written: usize,
    /// Layers written from the store that also had a kept field left out, so their record mixes the store's fields and Studio's.
    pub nodes_partial: usize,
    /// Layers the store has a record for whose value is no longer the one the file had: Studio's own record is written.
    pub nodes_edited: usize,
    /// Layers the store has a record for that this save writes under another guid: Studio's own record is written.
    pub nodes_guid_mismatch: usize,
    /// Fields written in place of Studio's.
    pub fields_written: usize,
    /// Property references Studio wrote under an id of its own session because no definition in scope has the name.
    pub prop_name_unresolved: usize,
    /// Consumption entries Studio left out because they bind to a property the save does not write for the layer's owner.
    pub prop_refs_dropped: usize,
    /// Kept fields left out, by why. Studio's own field (if it writes one) is written instead.
    pub fields_dropped_by_reason: BTreeMap<&'static str, usize>,
    /// Layers written from the store with no kept field left out.
    pub records_written: usize,
    /// The first edited layers, to tell a layer a person edited from one the open itself changed.
    pub edited_sample: Vec<NodeId>,
    /// Where a field was refused for a guid or an image hash at a place the closed list does not name: the field and the path
    /// below it (array positions as `#`), and how often. A new path here is a field Figma added and nothing classifies.
    pub unlisted_paths: BTreeMap<String, usize>,
}

/// The paths [`KeptWriteReport::unlisted_paths`] keeps apart; a hostile file cannot grow the report past this.
const MAX_UNLISTED_PATHS: usize = 256;

impl KeptWriteReport {
    fn note_unlisted(&mut self, path: &[Arc<str>]) {
        let key = path.iter().map(|p| &**p).collect::<Vec<_>>().join("/");
        if self.unlisted_paths.len() < MAX_UNLISTED_PATHS || self.unlisted_paths.contains_key(&key) {
            *self.unlisted_paths.entry(key).or_insert(0) += 1;
        }
    }

    fn drop_field(&mut self, reason: &'static str) {
        *self.fields_dropped_by_reason.entry(reason).or_insert(0) += 1;
    }

    /// How many kept fields were left out, whatever the reason.
    pub fn fields_dropped(&self) -> usize {
        self.fields_dropped_by_reason.values().sum()
    }
}

/// What the writer knows about one `NodeChange` field of the export schema.
struct FieldPlan {
    name: Arc<str>,
    written: bool,
    /// The field's type reaches a guid, an image or a blob, so its value is decoded and its ids checked.
    decode: bool,
}

/// What the save works out once about the fields of a `NodeChange`.
struct Plan {
    fields: HashMap<u32, FieldPlan>,
    zero_wire: Vec<bool>,
}

/// The ids this save writes, by the id the file used.
#[derive(Default)]
struct IdMaps {
    vars: HashMap<(u32, u32), (u32, u32)>,
    sets: HashMap<(u32, u32), (u32, u32)>,
    modes: HashMap<(u32, u32), (u32, u32)>,
    styles: HashMap<(u32, u32), (u32, u32)>,
    /// The written modes of each written variable set.
    modes_of_set: HashMap<(u32, u32), HashSet<(u32, u32)>>,
}

/// The save's state for the store.
#[derive(Default)]
pub(crate) struct KeptState {
    plan: Option<Arc<Plan>>,
    maps: Option<Arc<IdMaps>>,
    /// The file's blob number -> the number it has in this save.
    blobs: HashMap<u32, u32>,
    /// The field number of `overrideKey` in the file's schema.
    override_key_no: Option<u32>,
    /// The layer whose kept fields are being written.
    pub(super) scope: Option<NodeId>,
    /// What Studio wrote for the layer being written that counts against the report, by kind (see [`fold_unresolved`]).
    pub(super) pending: [usize; 5],
    /// Whether each layer asked about still has the value its snapshot took.
    matches: HashMap<NodeId, bool>,
    /// Layers whose file key is not used because another layer of the same main has it already.
    rejected_keys: HashSet<NodeId>,
    pub(crate) report: KeptWriteReport,
}

/// Per definition: whether a value of it can hold a guid, an image hash or a blob number.
fn id_bearing_defs(schema: &Schema) -> Vec<bool> {
    let mut reach = vec![false; schema.defs.len()];
    loop {
        let mut changed = false;
        for (i, d) in schema.defs.iter().enumerate() {
            if reach[i] || d.kind == DefKind::Enum {
                continue;
            }
            let hit = matches!(d.name.as_str(), "GUID" | "Image" | "Video")
                || d.fields.iter().any(|f| match f.ty {
                    FieldType::Native(Native::Uint) => BLOB_FIELDS.contains(&&*f.name),
                    FieldType::Native(Native::String) => STRING_ID_FIELDS.contains(&&*f.name),
                    FieldType::Def(c) => reach.get(c).copied().unwrap_or(false),
                    FieldType::Native(_) => false,
                });
            if hit {
                reach[i] = true;
                changed = true;
            }
        }
        if !changed {
            return reach;
        }
    }
}

impl KeptState {
    pub(crate) fn new(doc: &Document, schema: &Schema, nc_def: usize) -> Self {
        if doc.fig_kept.is_empty() {
            return Self::default();
        }
        let reach = id_bearing_defs(schema);
        let fields = schema
            .defs
            .get(nc_def)
            .map(|d| {
                d.fields
                    .iter()
                    .map(|f| {
                        let decode = match f.ty {
                            FieldType::Def(c) => reach.get(c).copied().unwrap_or(false),
                            FieldType::Native(Native::Uint) => BLOB_FIELDS.contains(&&*f.name),
                            FieldType::Native(Native::String) => STRING_ID_FIELDS.contains(&&*f.name),
                            FieldType::Native(_) => false,
                        };
                        (f.value, FieldPlan { name: f.name.clone(), written: is_written_from_the_store(&f.name), decode })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let override_key_no = schema.defs.get(nc_def).and_then(|d| d.field_named("overrideKey")).map(|f| f.value);
        Self { plan: Some(Arc::new(Plan { fields, zero_wire: schema.zero_wire_table() })), override_key_no, ..Self::default() }
    }

    /// Whether any layer can be written from the store.
    pub(crate) fn is_active(&self) -> bool {
        self.plan.is_some()
    }

    /// The field number the file's schema gives the `NodeChange` field `name`.
    pub(super) fn field_number(&self, name: &str) -> Option<u32> {
        self.plan.as_ref()?.fields.iter().find(|(_, f)| &*f.name == name).map(|(no, _)| *no)
    }

    /// Reads the kept bytes of one field as a value, with no check of the ids in it.
    pub(super) fn decode_unchecked(&self, schema: &Schema, nc_def: usize, no: u32, bytes: &[u8]) -> Option<(Arc<str>, V)> {
        let plan = self.plan.as_ref()?;
        schema.decode_field_value_prepared(nc_def, no, bytes, &plan.zero_wire).ok()
    }
}

/// The key the file's record for node `id` gives the layer in the overrides of its instances (`overrideKey`), when the record is the
/// layer's own, whatever has become of the layer since: the key names the layer and is no value of it. Studio's writer names the layer
/// by this key in the overrides it writes, and writes it as the layer's own key, so an override entry and the layer it names agree
/// whichever of the two Studio wrote and whichever the file's bytes give back.
pub(super) fn kept_override_key(cx: &ExportCtx<'_>, id: NodeId) -> Option<(u32, u32)> {
    if cx.kept.rejected_keys.contains(&id) {
        return None;
    }
    record_override_key(cx, id)
}

/// [`kept_override_key`] before the one-key-per-main rule.
fn record_override_key(cx: &ExportCtx<'_>, id: NodeId) -> Option<(u32, u32)> {
    let no = cx.kept.override_key_no.filter(|_| cx.kept.is_active())?;
    let rec = cx.doc.fig_kept.record_of_node(id)?;
    if rec.kind() != KeptRecordKind::Layer || cx.node_guids.get(&id) != Some(&rec.guid()) {
        return None;
    }
    guid_struct(rec.field(no)?)
}

/// A `GUID` struct as the wire holds it: the session and the local id, one variable-length number each, and nothing after.
fn guid_struct(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut at = 0;
    let mut next = || -> Option<u32> {
        let mut value = 0u64;
        for shift in (0..35).step_by(7) {
            let b = *bytes.get(at)?;
            at += 1;
            value |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return u32::try_from(value).ok();
            }
        }
        None
    };
    let (session, local) = (next()?, next()?);
    (at == bytes.len()).then_some((session, local))
}

/// Whether node `id` has the value its snapshot took (asked once per layer in a save).
fn snapshot_matches(cx: &mut ExportCtx<'_>, id: NodeId) -> bool {
    if let Some(&m) = cx.kept.matches.get(&id) {
        return m;
    }
    let m = cx.doc.fig_kept_node_matches_snapshot(id) == Some(true);
    cx.kept.matches.insert(id, m);
    m
}

/// Whether the save writes node `id` as the file had it: it has a layer record under the guid the save writes it with, its value is
/// still the one the file had, and it sits under the parent the file put it under.
pub(super) fn node_is_verbatim(cx: &mut ExportCtx<'_>, id: NodeId) -> bool {
    let doc = cx.doc;
    let (Some(rec), Some(&guid)) = (doc.fig_kept.record_of_node(id), cx.node_guids.get(&id)) else { return false };
    rec.kind() == KeptRecordKind::Layer && rec.guid() == guid && snapshot_matches(cx, id) && doc.fig_kept_node_moved(id) == Some(false)
}

/// The record to write for node `id` (exported under `guid`), or `None` for a layer that is written as Studio models it.
/// Counts the layers the store has a record for and that are not written from it.
pub(crate) fn select<'a>(cx: &mut ExportCtx<'a>, doc: &'a Document, id: NodeId, guid: (u32, u32)) -> Option<KeptRecordRef<'a>> {
    if !cx.kept.is_active() {
        return None;
    }
    let rec = doc.fig_kept.record_of_node(id)?;
    if rec.kind() != KeptRecordKind::Layer {
        return None;
    }
    if rec.guid() != guid {
        cx.kept.report.nodes_guid_mismatch += 1;
        return None;
    }
    if !snapshot_matches(cx, id) {
        cx.kept.report.nodes_edited += 1;
        if cx.kept.report.edited_sample.len() < MAX_EDITED_SAMPLES {
            cx.kept.report.edited_sample.push(id);
        }
        return None;
    }
    Some(rec)
}

pub(super) const UNRESOLVED_ASSIGNMENT: usize = 0;
pub(super) const UNRESOLVED_BINDING: usize = 1;
pub(super) const UNRESOLVED_SPEC: usize = 2;
pub(super) const UNRESOLVED_NESTED: usize = 3;
pub(super) const PENDING_DROPPED_ENTRY: usize = 4;

/// Starts counting what Studio writes for the next layer.
pub(super) fn begin_layer(cx: &mut ExportCtx<'_>) {
    cx.kept.pending = [0; 5];
}

/// Adds what Studio wrote for the layer to the report, except what the store then replaced: a reference Studio could not resolve
/// by name is only counted when it is the one that ends up in the file.
pub(super) fn fold_unresolved(cx: &mut ExportCtx<'_>, applied: Option<&Applied>) {
    let replaced = applied.map(|a| a.replaced).unwrap_or([false; 5]);
    let p = cx.kept.pending;
    for (i, n) in p.iter().enumerate() {
        if replaced[i] {
            continue;
        }
        if i == PENDING_DROPPED_ENTRY {
            cx.kept.report.prop_refs_dropped += n;
        } else {
            cx.kept.report.prop_name_unresolved += n;
        }
    }
    cx.kept.pending = [0; 5];
}

/// What [`apply`] did that the rest of the node's record depends on.
pub(crate) struct Applied {
    /// Which of the counted kinds of reference the store wrote the field of (index: the `UNRESOLVED_*` and `PENDING_*` constants).
    pub(super) replaced: [bool; 5],
    /// The kept `textData` is written, so the glyph style ids Figma stored are the ones its rows use.
    pub(crate) text_data: bool,
}

/// Writes the kept fields of `rec` into `fields`, each in place of the field of the same name Studio wrote.
pub(crate) fn apply(cx: &mut ExportCtx<'_>, id: NodeId, rec: KeptRecordRef<'_>, fields: &mut Vec<(Arc<str>, V)>) -> Applied {
    cx.kept.scope = Some(id);
    let Some(plan) = cx.kept.plan.clone() else { return Applied { text_data: false, replaced: [false; 5] } };
    let mut applied = Applied { text_data: false, replaced: [false; 5] };
    let (mut written_here, mut dropped_here) = (0usize, 0usize);
    for (no, bytes) in rec.fields() {
        let Some(fp) = plan.fields.get(&no) else {
            cx.kept.report.drop_field("field_unknown_to_the_schema");
            dropped_here += 1;
            continue;
        };
        if !fp.written {
            continue;
        }
        // the definition side of a property group is the store's only while the whole group is as the file had it
        if !cx.store_may_write_property_key(id, &fp.name) {
            cx.kept.report.drop_field("group_not_verbatim");
            dropped_here += 1;
            continue;
        }
        let value = if fp.decode {
            match decoded(cx, &plan, no, bytes) {
                Ok(v) => v,
                Err(reason) => {
                    cx.kept.report.drop_field(reason);
                    dropped_here += 1;
                    continue;
                }
            }
        } else {
            V::Raw(bytes.to_vec())
        };
        match fields.iter_mut().find(|(k, _)| **k == *fp.name) {
            Some(slot) => slot.1 = value,
            None => fields.push((fp.name.clone(), value)),
        }
        if &*fp.name == "textData" {
            applied.text_data = true;
        }
        match &*fp.name {
            "componentPropAssignments" => applied.replaced[UNRESOLVED_ASSIGNMENT] = true,
            "componentPropRefs" => applied.replaced[UNRESOLVED_BINDING] = true,
            "variantPropSpecs" => applied.replaced[UNRESOLVED_SPEC] = true,
            "parameterConsumptionMap" => applied.replaced[PENDING_DROPPED_ENTRY] = true,
            _ => {}
        }
        cx.kept.report.fields_written += 1;
        written_here += 1;
    }
    if written_here > 0 {
        cx.kept.report.nodes_written += 1;
        if dropped_here == 0 {
            cx.kept.report.records_written += 1;
        } else {
            cx.kept.report.nodes_partial += 1;
        }
    }
    applied
}

fn decoded(cx: &mut ExportCtx<'_>, plan: &Plan, no: u32, bytes: &[u8]) -> Result<V, &'static str> {
    let (name, mut value) = cx.schema.decode_field_value_prepared(cx.nc_def, no, bytes, &plan.zero_wire).map_err(|_| "decode_refused")?;
    let maps = id_maps(cx);
    let mut path = vec![name.clone()];
    walk(cx, &maps, &mut value, &mut path)?;
    if &*name == "variableModeBySetMap" && !modes_belong_to_their_sets(&maps, &value) {
        return Err("mode_not_in_its_variable_set");
    }
    Ok(value)
}

/// What a guid at some path of a kept field refers to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum IdKind {
    Node,
    Variable,
    VariableSet,
    Mode,
    Style,
    /// A component property definition: see `prop_ids` for who may write a reference to one.
    PropDef,
    /// An id the record defines or remembers, not one that points at something that has to exist.
    Definition,
}

/// The closed list of places a guid sits in the fields this save writes from the store. A guid anywhere else is not written:
/// nothing says what it points at, so nothing can say it still points at something.
fn classify(path: &[Arc<str>]) -> Option<IdKind> {
    let names: Vec<&str> = path.iter().map(|p| &**p).collect();
    let n = names.len();
    let last = names.last().copied()?;
    let before = n.checked_sub(2).map(|i| names[i]);
    if last == "guid" && before == Some("alias") {
        return Some(IdKind::Variable);
    }
    if last == "guid" && before.is_some_and(|b| b.starts_with("styleIdFor")) {
        return Some(IdKind::Style);
    }
    if last == "guid" && before == Some("variableSetID") {
        return Some(IdKind::VariableSet);
    }
    // the layer a component property value or a symbol id names
    if (last == "guidValue" && matches!(names[0], "componentPropAssignments" | "componentPropDefs")) || (last == "guid" && before == Some("symbolIdValue")) {
        return Some(IdKind::Node);
    }
    if last == "defId" && before == Some("propRefValue") {
        return Some(IdKind::PropDef);
    }
    match names.as_slice() {
        ["componentPropAssignments" | "componentPropRefs", "#", "defID"] | ["variantPropSpecs", "#", "propDefId"] | ["componentPropDefs", "#", "parentPropDefId"] => {
            Some(IdKind::PropDef)
        }
        ["componentPropDefs", "#", "id"] => Some(IdKind::Definition),
        ["variableModeBySetMap", "entries", "#", "variableModeID"] => Some(IdKind::Mode),
        ["handoffStatusMap", "entries", "#", "guid"]
        | ["hyperlink", "guid"]
        | ["detachedSymbolId", "guid"]
        | ["connectorStart" | "connectorEnd", "endpointNodeID"] => Some(IdKind::Node),
        ["variableSetModes", "#", "id"]
        | ["gridColumnsSizing" | "gridRowsSizing" | "gridColumns" | "gridRows", "entries", "#", "id"]
        | ["gridColumnAnchor" | "gridRowAnchor" | "publishID"]
        | ["annotationCategories", "items", "#", "id"]
        // the layers an ancestor path remembers were deleted: the field says what was there, and nothing is meant to resolve it
        | ["ancestorPathBeforeDeletion", "#"] => Some(IdKind::Definition),
        _ => None,
    }
}

/// Whether the node a kept reference names is written by this save under that guid: attached, owner of the record the guid
/// belongs to (a copy that carries the same source id is not).
fn node_reference_is_written(cx: &mut ExportCtx<'_>, g: (u32, u32)) -> bool {
    let doc = cx.doc;
    let Some(nid) = cx.node_with_guid(g) else { return false };
    is_attached(doc, nid) && doc.fig_kept.record_of_node(nid).is_some_and(|r| r.guid() == g)
}

fn guid_of(fields: &[(Arc<str>, V)]) -> Option<(u32, u32)> {
    match fields {
        [(a, V::Uint(s)), (b, V::Uint(l))] if &**a == "sessionID" && &**b == "localID" => Some((*s, *l)),
        _ => None,
    }
}

/// The null guid the file writes for "no reference".
fn is_null_guid(g: (u32, u32)) -> bool {
    g == (0, 0) || g == (u32::MAX, u32::MAX)
}

fn reverse(written: &BTreeMap<String, (u32, u32)>) -> HashMap<(u32, u32), (u32, u32)> {
    written.iter().filter_map(|(id, g)| Some((parse_guid_pair(id)?, *g))).collect()
}

fn id_maps(cx: &mut ExportCtx<'_>) -> Arc<IdMaps> {
    if let Some(m) = &cx.kept.maps {
        return m.clone();
    }
    let mut modes_of_set: HashMap<(u32, u32), HashSet<(u32, u32)>> = HashMap::new();
    for (cid, coll) in &cx.doc.variable_collections {
        let Some(set) = cx.coll_guids.get(cid) else { continue };
        let written: HashSet<(u32, u32)> = coll.modes.iter().filter_map(|m| cx.mode_guids.get(&m.id).copied()).collect();
        modes_of_set.insert(*set, written);
    }
    let maps = Arc::new(IdMaps {
        vars: reverse(&cx.var_guids),
        sets: reverse(&cx.coll_guids),
        modes: reverse(&cx.mode_guids),
        styles: reverse(&cx.style_guids),
        modes_of_set,
    });
    cx.kept.maps = Some(maps.clone());
    maps
}

/// Whether each entry of a decoded `variableModeBySetMap` names a mode its variable set has.
fn modes_belong_to_their_sets(maps: &IdMaps, value: &V) -> bool {
    let Some(entries) = value.get("entries").and_then(V::as_array) else { return true };
    entries.iter().all(|e| {
        let mode = e.get("variableModeID").and_then(|g| guid_of(fields_of(g)?));
        let set = e.get("variableSetID").and_then(|s| s.get("guid")).and_then(|g| guid_of(fields_of(g)?));
        match (mode, set) {
            (Some(m), Some(s)) => maps.modes_of_set.get(&s).is_some_and(|modes| modes.contains(&m)),
            // a set held by an asset reference: the mode is the library's
            _ => true,
        }
    })
}

fn fields_of(v: &V) -> Option<&[(Arc<str>, V)]> {
    match v {
        V::Struct(f) | V::Message(f) => Some(f),
        _ => None,
    }
}

/// Checks every id of `value` against this save and rewrites it to the id this save writes, and re-adds every blob. An id
/// that cannot be written, or sits where the closed list has no place for it, refuses the whole field.
fn walk(cx: &mut ExportCtx<'_>, maps: &IdMaps, value: &mut V, path: &mut Vec<Arc<str>>) -> Result<(), &'static str> {
    match value {
        V::Struct(fields) if guid_of(fields).is_some() => {
            let Some(g) = guid_of(fields) else { return Ok(()) };
            let Some(kind) = classify(path) else {
                cx.kept.report.note_unlisted(path);
                return Err("id_at_an_unlisted_path");
            };
            if is_null_guid(g) {
                return Ok(());
            }
            let mapped = match kind {
                IdKind::Definition => g,
                IdKind::Node => {
                    if !node_reference_is_written(cx, g) {
                        return Err("node_not_written");
                    }
                    g
                }
                IdKind::Variable => *maps.vars.get(&g).ok_or("variable_not_written")?,
                IdKind::VariableSet => *maps.sets.get(&g).ok_or("variable_set_not_written")?,
                IdKind::Mode => *maps.modes.get(&g).ok_or("mode_not_written")?,
                IdKind::Style => *maps.styles.get(&g).ok_or("style_not_written")?,
                IdKind::PropDef => {
                    cx.kept_prop_ref_is_written(path.first().map(|p| &**p).unwrap_or(""), g)?;
                    g
                }
            };
            for (name, v) in fields.iter_mut() {
                *v = V::Uint(if &**name == "sessionID" { mapped.0 } else { mapped.1 });
            }
        }
        V::Struct(fields) | V::Message(fields) => {
            for (name, v) in fields.iter_mut() {
                match (&**name, &mut *v) {
                    (n, V::Uint(old)) if BLOB_FIELDS.contains(&n) => *old = add_kept_blob(cx, *old).ok_or("blob_missing")?,
                    ("nodeId", V::String(id)) => {
                        let g = parse_guid_pair(id).ok_or("string_id_not_a_guid")?;
                        if !node_reference_is_written(cx, g) {
                            return Err("node_not_written");
                        }
                    }
                    ("mediaHash", V::String(hash)) => {
                        if !hash.is_empty() && !cx.doc.images.contains_key(hash.as_str()) {
                            return Err("image_not_written");
                        }
                    }
                    ("hash", V::Bytes(hash)) => {
                        if let Err(reason) = check_image_hash(cx, path, hash) {
                            if reason == "hash_at_an_unlisted_path" {
                                cx.kept.report.note_unlisted(path);
                            }
                            return Err(reason);
                        }
                    }
                    _ => {}
                }
                path.push(name.clone());
                let res = walk(cx, maps, v, path);
                path.pop();
                res?;
            }
        }
        V::Array(items) => {
            path.push(Arc::from("#"));
            let mut res = Ok(());
            for item in items {
                res = walk(cx, maps, item, path);
                if res.is_err() {
                    break;
                }
            }
            path.pop();
            res?;
        }
        _ => {}
    }
    Ok(())
}

/// An image hash must name an image this save writes. A hash that names a video, or that sits anywhere but in an image, is
/// not one this writer knows.
fn check_image_hash(cx: &ExportCtx<'_>, path: &[Arc<str>], hash: &[u8]) -> Result<(), &'static str> {
    let owner = path.last().map(|p| &**p);
    if !matches!(owner, Some("image" | "imageThumbnail" | "animatedImage")) {
        return Err("hash_at_an_unlisted_path");
    }
    if hash.is_empty() || cx.doc.images.contains_key(&crate::kiwi::hex(hash)) {
        Ok(())
    } else {
        Err("image_not_written")
    }
}

/// The number this save gives the file's blob `old`, adding the blob to the table the first time.
fn add_kept_blob(cx: &mut ExportCtx<'_>, old: u32) -> Option<u32> {
    if let Some(&slot) = cx.kept.blobs.get(&old) {
        return Some(slot);
    }
    let bytes = cx.doc.fig_kept.blob(old as usize)?.to_vec();
    let slot = cx.add_blob(bytes);
    cx.kept.blobs.insert(old, slot);
    Some(slot)
}

#[cfg(test)]
mod tests {
    use super::guid_struct;

    #[test]
    fn a_guid_struct_reads_two_variable_length_numbers_and_nothing_else() {
        assert_eq!(guid_struct(&[0x87, 0x01, 0x05]), Some((135, 5)));
        assert_eq!(guid_struct(&[0x00, 0x00]), Some((0, 0)));
        assert_eq!(guid_struct(&[0xff, 0xff, 0xff, 0xff, 0x0f, 0x01]), Some((u32::MAX, 1)));
        assert_eq!(guid_struct(&[0x87, 0x01]), None, "the local id is missing");
        assert_eq!(guid_struct(&[0x01, 0x02, 0x03]), None, "bytes are left over");
        assert_eq!(guid_struct(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01, 0x01]), None, "a number past 32 bits");
        assert_eq!(guid_struct(&[0x81]), None, "a number that does not end");
    }
}

/// Works out which layers must not use the key the file gave them. The overrides of an instance name a layer of its main by key, so
/// two layers of one main with the same key would be one layer to them. Figma gives a copy of a component the keys of the original,
/// so a layer moved from the copy into the original can meet its own key there: the layer with the smaller id keeps it.
pub(super) fn index_override_keys(cx: &mut ExportCtx<'_>) {
    if !cx.kept.is_active() {
        return;
    }
    let doc = cx.doc;
    let mut seen: HashMap<(NodeId, (u32, u32)), NodeId> = HashMap::new();
    let mut rejected: HashSet<NodeId> = HashSet::new();
    for &id in doc.nodes.keys() {
        let Some(key) = record_override_key(cx, id).filter(|k| *k != (u32::MAX, u32::MAX)) else { continue };
        let mut cur = Some(id);
        let mut main = None;
        for _ in 0..=doc.nodes.len() {
            let Some(c) = cur else { break };
            let Some(n) = doc.get(c) else { break };
            if matches!(n.kind, designos_doc::NodeKind::Component(_)) {
                main = Some(c);
                break;
            }
            cur = n.parent;
        }
        let Some(main) = main else { continue };
        // nodes are visited by id: the first layer to hold a key keeps it
        if *seen.entry((main, key)).or_insert(id) != id {
            rejected.insert(id);
        }
    }
    cx.kept.rejected_keys = rejected;
}
