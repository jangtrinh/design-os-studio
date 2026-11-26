//! The internal canvas guid a save writes under, and the records Figma marked deleted that Studio has no entry for.

use super::kept_fields::{id_maps, IdMaps};
use super::kept_record_fields::{decode_kept, guid_value, modes_for_variable, shape_of, Numbers, Shape};
use super::kept_records::STUDIO_INTERNAL_CANVAS_GUID;
use super::ExportCtx;
use crate::kiwi::KiwiValue as V;
use crate::testkit::guid;
use designos_doc::{KeptRecordKind, KeptRecordRef};
use std::collections::HashSet;
use std::sync::Arc;

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
/// The most rounds the pass for deleted records runs: a chain of records each naming the next drops one record a round, so a file
/// could make the pass quadratic. Records still alive when the rounds are spent are left out and counted as not settled.
pub(super) const MAX_SETTLE_ROUNDS: usize = 32;

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
        } else if rec.guid() != (0, 0) && Some(rec.guid()) != cx.records.internal_guid {
            // not written by an entry of this save, and not one the file deleted: a second internal canvas, a variable override, a
            // record the importer reads as deleted by its phase, an entry the user removed
            *cx.kept.report.records.others_not_written.entry(shape.label()).or_insert(0) += 1;
        }
    }
    if alive.is_empty() {
        return Vec::new();
    }
    let base = id_maps(cx);
    loop {
        cx.kept.report.records.settle_rounds += 1;
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
        let mut not_written = 0usize;
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
                    not_written += d.not_written;
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
            report.soft_deleted_fields_not_written += not_written;
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
        if report.settle_rounds >= MAX_SETTLE_ROUNDS {
            // the rounds are spent and records are still being dropped: what is left is not known to be sound
            let gone: HashSet<usize> = failed.into_iter().collect();
            for (_, shape) in alive.iter().enumerate().filter(|(i, _)| !gone.contains(i)).map(|(_, a)| a) {
                *report.soft_deleted_dropped.entry(format!("{}: not_settled", shape.label())).or_insert(0) += 1;
            }
            return Vec::new();
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
