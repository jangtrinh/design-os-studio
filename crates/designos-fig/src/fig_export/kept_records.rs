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

use super::kept_fields::id_maps;
use super::kept_record_fields::{decode_kept, modes_for_variable, other_record, put, shape_of, Numbers};
use super::ExportCtx;
use crate::kiwi::KiwiValue as V;
use designos_doc::KeptRecordRef;
use std::collections::BTreeMap;

pub(super) use super::kept_deleted_records::{internal_canvas_guid, plan_internal};

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
    pub(super) internal_guid: Option<(u32, u32)>,
}

impl ExportCtx<'_> {
    /// Whether `g` is the guid of a record this save writes that a record that is not a layer may sit under: the document, the
    /// internal canvas, or a page or layer.
    pub(super) fn record_parent_is_written(&mut self, g: (u32, u32)) -> bool {
        g == (0, 0) || self.records.internal_guid == Some(g) || self.node_with_guid(g).is_some()
    }
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
