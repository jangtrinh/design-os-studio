//! Figma's own records of the styles, variables and variable sets of a `.fig`, written back by a save.
//!
//! A `.fig` open keeps every record Figma wrote, layers and others alike (see `designos_doc::FigKeptStore`). Studio models a
//! style, a variable and a variable set as an entry of a table of the document and writes its own form of the record, which holds
//! fewer fields than Figma's (its defaults, its versions, its sort positions, the colour spaces of a paint) and renumbers
//! some of what it holds. For an entry nobody edited since the open (see `Document::fig_kept_style_matches_snapshot`), each kept
//! field takes the place of the field Studio wrote under the same name: the whole value of one or the whole value of the other,
//! never a mix. An edited entry is written exactly as before.
//!
//! Every id inside a kept field is checked against what this save writes, as it is for a layer. The record's parent must be a
//! record the save writes, and the entries of a variable's values that name a mode its variable set no longer has are left out
//! and counted.
//!
//! Records Studio has no entry for are written too, when Figma marked them deleted and every id they name is written in this
//! save: a deleted style, a deleted variable nobody uses. They come back as the file had them, so a reopen reads them as the first
//! open did. A record that names an id this save does not write is left out and counted by type.
//!
//! The internal canvas the records sit on is written under the guid the file gave it, so the layers and records below it keep their
//! parent.

use super::kept_fields::{decode_record_field, id_maps, IdMaps};
use super::ExportCtx;
use crate::kiwi::KiwiValue as V;
use crate::testkit::guid;
use designos_doc::{KeptRecordKind, KeptRecordRef};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

/// The guid Studio writes its own internal canvas under when the file gave none.
pub(super) const STUDIO_INTERNAL_CANVAS_GUID: (u32, u32) = (0, 999_999);

/// The records a report names.
const MAX_EDITED_SAMPLES: usize = 20;

/// What one save wrote from the kept records that are not layers, counted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecordsKeptReport {
    /// Style, variable and variable set records written from the store, with at least one kept field in place of Studio's.
    pub styles_written: usize,
    pub variables_written: usize,
    pub sets_written: usize,
    /// Entries the store has a record for whose value is no longer the one the file had: Studio's own record is written.
    pub styles_edited: usize,
    pub variables_edited: usize,
    pub sets_edited: usize,
    /// The internal canvas record written from the store (1), or left as Studio's own because the library page that stands for it was
    /// edited (1 in `internal_canvas_edited`).
    pub internal_canvas_written: usize,
    pub internal_canvas_edited: usize,
    /// The guid the internal canvas was written under, and where it came from.
    pub internal_canvas_guid: Option<(u32, u32)>,
    pub internal_canvas_guid_source: &'static str,
    /// Kept fields written in place of Studio's, over all of those records.
    pub fields_written: usize,
    /// Kept fields left out, by why. Studio's own field (if it writes one) is written instead.
    pub fields_dropped_by_reason: BTreeMap<&'static str, usize>,
    /// Entries of a variable's values left out because they name a mode the variable's set no longer has.
    pub variable_values_dropped_mode_gone: usize,
    /// Records Studio has no entry for that were written, by type (and style type).
    pub soft_deleted_written: BTreeMap<String, usize>,
    /// Records Studio has no entry for that were left out, by type and reason.
    pub soft_deleted_dropped: BTreeMap<String, usize>,
    /// The first entries found edited, to tell an entry a person edited from one the open itself changed.
    pub edited_sample: Vec<String>,
}

impl RecordsKeptReport {
    fn drop_field(&mut self, reason: &'static str) {
        *self.fields_dropped_by_reason.entry(reason).or_insert(0) += 1;
    }

    fn note_edited(&mut self, id: &str) {
        if self.edited_sample.len() < MAX_EDITED_SAMPLES {
            self.edited_sample.push(id.to_string());
        }
    }

    /// How many kept fields were left out of records written from the store, whatever the reason.
    pub fn fields_dropped(&self) -> usize {
        self.fields_dropped_by_reason.values().sum()
    }

    /// How many records Studio has no entry for were written, and how many were left out.
    pub fn soft_deleted_totals(&self) -> (usize, usize) {
        (self.soft_deleted_written.values().sum(), self.soft_deleted_dropped.values().sum())
    }
}

/// The save's state for these records.
#[derive(Default)]
pub(super) struct RecordsState {
    /// The guid of the internal canvas this save writes the styles, variables and library components under.
    internal_guid: Option<(u32, u32)>,
}

/// The field numbers the file's schema gives the fields this module reads, each `None` when the schema has none.
#[derive(Clone, Copy, Default)]
struct Numbers {
    guid: Option<u32>,
    ty: Option<u32>,
    style_type: Option<u32>,
    internal_only: Option<u32>,
    is_soft_deleted: Option<u32>,
    is_soft_deleted_style: Option<u32>,
    variable_set_id: Option<u32>,
    variable_set_modes: Option<u32>,
}

impl Numbers {
    fn of(cx: &ExportCtx<'_>) -> Self {
        let n = |name: &str| cx.kept.field_number(name);
        Self {
            guid: n("guid"),
            ty: n("type"),
            style_type: n("styleType"),
            internal_only: n("internalOnly"),
            is_soft_deleted: n("isSoftDeleted"),
            is_soft_deleted_style: n("isSoftDeletedStyle"),
            variable_set_id: n("variableSetID"),
            variable_set_modes: n("variableSetModes"),
        }
    }
}

/// What a kept record is, read from a few of its fields.
struct Shape {
    ty: String,
    style_type: Option<String>,
    internal_only: bool,
    soft_deleted: bool,
}

impl Shape {
    /// A label for the report: the node type, and the style type of a style.
    fn label(&self) -> String {
        match &self.style_type {
            Some(st) => format!("{}/{st}", self.ty),
            None => self.ty.clone(),
        }
    }
}

fn shape_of(cx: &ExportCtx<'_>, nums: &Numbers, rec: KeptRecordRef<'_>) -> Shape {
    let mut shape = Shape { ty: String::new(), style_type: None, internal_only: false, soft_deleted: false };
    for (no, bytes) in rec.fields() {
        let some = Some(no);
        if some == nums.ty || some == nums.style_type {
            if let Some((_, V::Enum(name))) = cx.kept.decode_unchecked(cx.schema, cx.nc_def, no, bytes) {
                if some == nums.ty {
                    shape.ty = name.to_string();
                } else {
                    shape.style_type = Some(name.to_string());
                }
            }
        } else if some == nums.internal_only {
            shape.internal_only = bytes == [1];
        } else if some == nums.is_soft_deleted || some == nums.is_soft_deleted_style {
            shape.soft_deleted |= bytes == [1];
        }
    }
    shape
}

/// The record the file holds under `g`, when it is one the store keeps whole (not a layer) and this save can write from the store.
fn other_record<'a>(cx: &ExportCtx<'a>, g: (u32, u32)) -> Option<KeptRecordRef<'a>> {
    if !cx.kept.is_active() {
        return None;
    }
    let doc = cx.doc;
    doc.fig_kept.record_by_guid(g).filter(|r| r.kind() == KeptRecordKind::Other)
}

/// `value` is a `GUID` struct: its session and local number.
fn guid_value(value: &V) -> Option<(u32, u32)> {
    match value {
        V::Struct(f) | V::Message(f) => match f.as_slice() {
            [(a, V::Uint(s)), (b, V::Uint(l))] if &**a == "sessionID" && &**b == "localID" => Some((*s, *l)),
            _ => None,
        },
        _ => None,
    }
}

/// Puts `value` in `record` under `name`, in place of the field of that name Studio wrote.
fn put(record: &mut V, name: Arc<str>, value: V) {
    let V::Message(fields) = record else { return };
    match fields.iter_mut().find(|(k, _)| *k == name) {
        Some(slot) => slot.1 = value,
        None => fields.push((name, value)),
    }
}

impl ExportCtx<'_> {
    /// Whether `g` is the guid of a record this save writes that a record that is not a layer may sit under: the document, the
    /// internal canvas, or a page or layer.
    pub(super) fn record_parent_is_written(&mut self, g: (u32, u32)) -> bool {
        g == (0, 0) || self.records.internal_guid == Some(g) || self.node_with_guid(g).is_some()
    }
}

/// What the kept fields of one record came to.
#[derive(Default)]
struct Decoded {
    fields: Vec<(Arc<str>, V)>,
    /// Why each field that was left out was.
    refused: Vec<&'static str>,
    values_dropped: usize,
}

/// Decodes the kept fields of `rec` against `maps`, leaving out the record's own guid. `modes` names the modes a variable's values
/// may be for: an entry of `variableDataValues` for any other mode is taken out and counted.
fn decode_kept(cx: &mut ExportCtx<'_>, maps: &IdMaps, nums: &Numbers, rec: KeptRecordRef<'_>, modes: Option<&HashSet<(u32, u32)>>) -> Decoded {
    let mut out = Decoded::default();
    for (no, bytes) in rec.fields() {
        if Some(no) == nums.guid {
            continue;
        }
        let mut dropped = 0usize;
        let mut prefilter = |name: &str, value: &mut V| {
            if let (true, Some(modes)) = (name == "variableDataValues", modes) {
                dropped = retain_values_of_modes(value, modes);
            }
        };
        match decode_record_field(cx, maps, no, bytes, &mut prefilter) {
            Ok(Some((name, value))) => {
                if &*name == "variableDataValues" && dropped > 0 && values_are_empty(&value) {
                    // every value was for a mode that is gone: nothing of the field is left to write
                    out.refused.push("variable_values_all_for_gone_modes");
                    out.values_dropped += dropped;
                    continue;
                }
                out.values_dropped += dropped;
                out.fields.push((name, value));
            }
            Ok(None) => {}
            Err(reason) => out.refused.push(reason),
        }
    }
    out
}

/// Takes the entries of a decoded `variableDataValues` that name a mode outside `modes` out of it, and says how many it took.
fn retain_values_of_modes(value: &mut V, modes: &HashSet<(u32, u32)>) -> usize {
    let V::Message(fields) = value else { return 0 };
    let Some((_, V::Array(entries))) = fields.iter_mut().find(|(k, _)| &**k == "entries") else { return 0 };
    let before = entries.len();
    entries.retain(|e| match e.get("modeID").and_then(guid_value) {
        Some(g) => modes.contains(&g),
        None => true,
    });
    before - entries.len()
}

fn values_are_empty(value: &V) -> bool {
    value.get("entries").and_then(V::as_array).is_some_and(<[V]>::is_empty)
}

/// The modes a value of a variable of set `set` may be for: the set's own, and those of the collections that extend one.
fn modes_for_variable(cx: &ExportCtx<'_>, maps: &IdMaps, set: (u32, u32)) -> HashSet<(u32, u32)> {
    let mut modes = maps.modes_of_set.get(&set).cloned().unwrap_or_default();
    for c in cx.doc.variable_collections.values().filter(|c| c.is_extension()) {
        if let Some(own) = cx.coll_guids.get(&c.id).and_then(|g| maps.modes_of_set.get(g)) {
            modes.extend(own.iter().copied());
        }
    }
    modes
}

/// Which of the three kinds of entry a record is written for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Style,
    Variable,
    Set,
}

/// Writes the kept fields of `rec` into Studio's `record`. `set` is the variable set a variable record is written in (see
/// [`decode_kept`]).
fn write_into(cx: &mut ExportCtx<'_>, kind: Kind, rec: KeptRecordRef<'_>, record: &mut V, set: Option<(u32, u32)>) {
    let nums = Numbers::of(cx);
    let maps = id_maps(cx);
    let modes = set.map(|s| modes_for_variable(cx, &maps, s));
    let d = decode_kept(cx, &maps, &nums, rec, modes.as_ref());
    let report = &mut cx.kept.report.records;
    report.variable_values_dropped_mode_gone += d.values_dropped;
    for reason in &d.refused {
        report.drop_field(reason);
    }
    if d.fields.is_empty() {
        return;
    }
    report.fields_written += d.fields.len();
    match kind {
        Kind::Style => report.styles_written += 1,
        Kind::Variable => report.variables_written += 1,
        Kind::Set => report.sets_written += 1,
    }
    for (name, value) in d.fields {
        put(record, name, value);
    }
}

/// Writes Figma's fields into the record Studio built for style `sid` (written under `g` as a `node_type`/`style_type` record), when the
/// style is unedited. A style whose Figma record is of another kind (a stroke style, for a fill) is written as Studio models it.
pub(super) fn write_style(cx: &mut ExportCtx<'_>, sid: &str, g: (u32, u32), style_types: &[&str], record: &mut V) {
    let Some(rec) = other_record(cx, g) else { return };
    match cx.doc.fig_kept_style_matches_snapshot(sid) {
        Some(true) => {}
        Some(false) => {
            cx.kept.report.records.styles_edited += 1;
            cx.kept.report.records.note_edited(sid);
            return;
        }
        None => return,
    }
    let nums = Numbers::of(cx);
    let shape = shape_of(cx, &nums, rec);
    if shape.style_type.as_deref().is_none_or(|t| !style_types.contains(&t)) {
        return;
    }
    write_into(cx, Kind::Style, rec, record, None);
}

/// Writes Figma's fields into the record Studio built for variable `vid`, written under `g` in the set written under `set`.
pub(super) fn write_variable(cx: &mut ExportCtx<'_>, vid: &str, g: (u32, u32), set: (u32, u32), record: &mut V) {
    let Some(rec) = other_record(cx, g) else { return };
    match cx.doc.fig_kept_variable_matches_snapshot(vid) {
        Some(true) => {}
        Some(false) => {
            cx.kept.report.records.variables_edited += 1;
            cx.kept.report.records.note_edited(vid);
            return;
        }
        None => return,
    }
    let nums = Numbers::of(cx);
    if shape_of(cx, &nums, rec).ty != "VARIABLE" {
        return;
    }
    write_into(cx, Kind::Variable, rec, record, Some(set));
}

/// Writes Figma's fields into the record Studio built for variable set `cid`, written under `g`.
pub(super) fn write_set(cx: &mut ExportCtx<'_>, cid: &str, g: (u32, u32), record: &mut V) {
    let Some(rec) = other_record(cx, g) else { return };
    match cx.doc.fig_kept_collection_matches_snapshot(cid) {
        Some(true) => {}
        Some(false) => {
            cx.kept.report.records.sets_edited += 1;
            cx.kept.report.records.note_edited(cid);
            return;
        }
        None => return,
    }
    let nums = Numbers::of(cx);
    if shape_of(cx, &nums, rec).ty != "VARIABLE_SET" {
        return;
    }
    write_into(cx, Kind::Set, rec, record, None);
}

/// Writes Figma's fields into the record Studio built for the internal canvas, written under `g`: when the file's record under that guid
/// is an internal canvas, and the library page that stands for it was not edited.
pub(super) fn write_internal_canvas(cx: &mut ExportCtx<'_>, g: (u32, u32), record: &mut V) {
    let Some(rec) = other_record(cx, g) else { return };
    let doc = cx.doc;
    if doc.library_page.is_some_and(|lp| doc.fig_kept_node_matches_snapshot(lp) == Some(false)) {
        cx.kept.report.records.internal_canvas_edited += 1;
        return;
    }
    let nums = Numbers::of(cx);
    let shape = shape_of(cx, &nums, rec);
    if shape.ty != "CANVAS" || !shape.internal_only {
        return;
    }
    let maps = id_maps(cx);
    let d = decode_kept(cx, &maps, &nums, rec, None);
    let report = &mut cx.kept.report.records;
    for reason in &d.refused {
        report.drop_field(reason);
    }
    if d.fields.is_empty() {
        return;
    }
    report.internal_canvas_written += 1;
    report.fields_written += d.fields.len();
    for (name, value) in d.fields {
        put(record, name, value);
    }
}

/// The guids this save writes a record or a layer under. A style the document holds only because a layer names it (one the file
/// deleted) has a guid but is not written as a record, so its guid is free for the file's own record of it.
fn claimed_guids(cx: &ExportCtx<'_>) -> HashSet<(u32, u32)> {
    let doc = cx.doc;
    let styles = cx.style_guids.iter().filter(|(id, _)| id.starts_with("S:") || doc.is_table_style(id)).map(|(_, g)| g);
    cx.node_guids.values().chain(cx.coll_guids.values()).chain(cx.var_guids.values()).chain(styles).copied().collect()
}

/// The guid of the internal canvas this save writes, and what decided it: the library page's source id (the first internal canvas
/// the file had, noted at the open), else the file's first internal canvas record when nothing this save writes uses its guid (a file
/// whose internal canvas held only styles and variables has no library page), else Studio's own guid.
fn internal_guid(cx: &ExportCtx<'_>, nums: &Numbers, claimed: &HashSet<(u32, u32)>) -> ((u32, u32), &'static str) {
    let doc = cx.doc;
    let from_page = doc.library_page.and_then(|lp| {
        let g = doc.get(lp)?.source_id.as_deref().and_then(super::parse_guid_pair)?;
        (cx.node_guids.get(&lp) == Some(&g)).then_some(g)
    });
    if let Some(g) = from_page {
        return (g, "library page");
    }
    if cx.kept.is_active() {
        let first = doc.fig_kept.records().filter(|r| r.kind() == KeptRecordKind::Other && !claimed.contains(&r.guid())).find(|r| {
            let shape = shape_of(cx, nums, *r);
            shape.internal_only && shape.ty == "CANVAS"
        });
        if let Some(r) = first {
            return (r.guid(), "file record");
        }
    }
    (STUDIO_INTERNAL_CANVAS_GUID, "studio")
}

/// Settles the guid of the internal canvas and works out which records Studio has no entry for are written. Returns those records, to
/// be pushed after the styles and variables.
pub(super) fn plan_internal(cx: &mut ExportCtx<'_>) -> Vec<V> {
    if !cx.kept.is_active() {
        cx.records.internal_guid = Some(STUDIO_INTERNAL_CANVAS_GUID);
        return Vec::new();
    }
    let nums = Numbers::of(cx);
    let claimed = claimed_guids(cx);
    let (g, source) = internal_guid(cx, &nums, &claimed);
    cx.records.internal_guid = Some(g);
    cx.kept.report.records.internal_canvas_guid = Some(g);
    cx.kept.report.records.internal_canvas_guid_source = source;
    plan_soft_deleted(cx, &nums, &claimed)
}

/// The guid the save writes the internal canvas under.
pub(super) fn internal_canvas_guid(cx: &ExportCtx<'_>) -> (u32, u32) {
    cx.records.internal_guid.unwrap_or(STUDIO_INTERNAL_CANVAS_GUID)
}

/// What a record Studio has no entry for adds to the ids this save writes, once it is written.
fn add_to_maps(cx: &ExportCtx<'_>, nums: &Numbers, maps: &mut IdMaps, rec: KeptRecordRef<'_>, shape: &Shape) {
    let g = rec.guid();
    match shape.ty.as_str() {
        "VARIABLE" => {
            maps.vars.insert(g, g);
        }
        "VARIABLE_SET" => {
            maps.sets.insert(g, g);
            let modes: HashSet<(u32, u32)> = rec
                .fields()
                .filter(|(no, _)| Some(*no) == nums.variable_set_modes)
                .filter_map(|(no, bytes)| cx.kept.decode_unchecked(cx.schema, cx.nc_def, no, bytes))
                .filter_map(|(_, v)| v.as_array().map(<[V]>::to_vec))
                .flatten()
                .filter_map(|mode| mode.get("id").and_then(guid_value))
                .collect();
            for m in &modes {
                maps.modes.insert(*m, *m);
            }
            maps.modes_of_set.insert(g, modes);
        }
        _ if shape.style_type.is_some() => {
            maps.styles.insert(g, g);
        }
        _ => {}
    }
}

/// The set a deleted variable's record names.
fn set_named_by(cx: &ExportCtx<'_>, nums: &Numbers, rec: KeptRecordRef<'_>) -> Option<(u32, u32)> {
    let (no, bytes) = rec.fields().find(|(no, _)| Some(*no) == nums.variable_set_id)?;
    let (_, value) = cx.kept.decode_unchecked(cx.schema, cx.nc_def, no, bytes)?;
    value.get("guid").and_then(guid_value)
}

fn is_soft_deleted_record(shape: &Shape) -> bool {
    shape.soft_deleted && (matches!(shape.ty.as_str(), "VARIABLE" | "VARIABLE_SET") || shape.style_type.is_some())
}

/// The records the file marks as deleted that Studio has no entry for, written as the file had them when every id they name is written
/// in this save. They name each other (a deleted variable aliasing another, a deleted variable in a deleted set), so a record whose
/// reference fails takes its dependents with it: the rounds repeat until one drops nothing.
fn plan_soft_deleted(cx: &mut ExportCtx<'_>, nums: &Numbers, claimed: &HashSet<(u32, u32)>) -> Vec<V> {
    let doc = cx.doc;
    let mut seen = HashSet::new();
    let mut alive: Vec<(KeptRecordRef<'_>, Shape)> = Vec::new();
    for rec in doc.fig_kept.records().filter(|r| r.kind() == KeptRecordKind::Other && !claimed.contains(&r.guid())) {
        if !seen.insert(rec.guid()) {
            continue;
        }
        let shape = shape_of(cx, nums, rec);
        if is_soft_deleted_record(&shape) {
            alive.push((rec, shape));
        }
    }
    if alive.is_empty() {
        return Vec::new();
    }
    let base = id_maps(cx);
    loop {
        let mut maps = (*base).clone();
        for (rec, shape) in &alive {
            add_to_maps(cx, nums, &mut maps, *rec, shape);
        }
        // a round writes real blobs; a round that drops a record takes them out again before the next one
        let table_len = cx.blobs.len();
        let blobs_before = cx.kept.blobs.clone();
        let mut built: Vec<V> = Vec::with_capacity(alive.len());
        let mut failed: Vec<usize> = Vec::new();
        let mut reasons: Vec<&'static str> = Vec::new();
        let mut values_dropped = 0usize;
        for (i, (rec, shape)) in alive.iter().enumerate() {
            let modes = (shape.ty == "VARIABLE").then(|| set_named_by(cx, nums, *rec).map(|s| modes_for_variable(cx, &maps, s)).unwrap_or_default());
            let d = decode_kept(cx, &maps, nums, *rec, modes.as_ref());
            match d.refused.first() {
                Some(reason) => {
                    failed.push(i);
                    reasons.push(reason);
                }
                None => {
                    values_dropped += d.values_dropped;
                    let g = rec.guid();
                    let mut fields: Vec<(Arc<str>, V)> = Vec::with_capacity(d.fields.len() + 1);
                    fields.push(("guid".into(), guid(g.0, g.1)));
                    fields.extend(d.fields);
                    built.push(V::Message(fields));
                }
            }
        }
        if failed.is_empty() {
            let report = &mut cx.kept.report.records;
            report.variable_values_dropped_mode_gone += values_dropped;
            for (_, shape) in &alive {
                *report.soft_deleted_written.entry(shape.label()).or_insert(0) += 1;
            }
            return built;
        }
        cx.blobs.truncate(table_len);
        cx.kept.blobs = blobs_before;
        let report = &mut cx.kept.report.records;
        for (&i, reason) in failed.iter().zip(&reasons) {
            *report.soft_deleted_dropped.entry(format!("{}: {reason}", alive[i].1.label())).or_insert(0) += 1;
        }
        let gone: HashSet<usize> = failed.into_iter().collect();
        let mut i = 0;
        alive.retain(|_| {
            let keep = !gone.contains(&i);
            i += 1;
            keep
        });
        if alive.is_empty() {
            return Vec::new();
        }
    }
}
