//! Override entries of an instance whose page has not been loaded.
//!
//! A large multi-page `.fig` opens with the instances of inactive pages unexpanded: they have no children yet, and the
//! page loader keeps what the file said about them. A save that wrote only what is in the document would write those
//! instances with no `symbolOverrides`, and the saved file would lose every text, paint and variable override they
//! carry. This module writes the entries the file stated, as it stated them, without loading the page.
//!
//! The entries are read-only data from the loader ([`Document::pending_instance_overrides`]). Two kinds of reference in
//! them only mean something inside the file they came from, so they are mapped into the file being written: a geometry
//! blob is added to this file's blob table, and a component property is named by this file's definition id. A field the
//! writer's schema holds but no writer gates yet (prototype interactions on an override, text layout versions: see
//! `HELD_BACK_UNTIL_KEPT`) is left out of the entry, and so is one the schema has no definition for; the rest of the
//! entry is written as it was.

use super::{collect_instance_overrides_for, parse_guid_pair, strip_held_back, ExportCtx};
use crate::instance_self_digest::edited_fields;
use crate::kiwi::{decode_hex_bytes, FieldType, KiwiValue as V};
use crate::own_entry_keys::field_removed_with;
use crate::testkit::guid;
use designos_doc::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// What the loader recorded for one instance, parsed.
struct Kept {
    entries: Value,
    /// Blob index in the source file -> the blob's bytes.
    blobs: BTreeMap<u64, Vec<u8>>,
    /// Component property definition id (`s:l`) in the source file -> the property's name.
    props: BTreeMap<String, String>,
}

fn parse_kept(text: &str, main_guid: (u32, u32)) -> Option<Kept> {
    let v: Value = serde_json::from_str(text).ok()?;
    // Entries are paths into the component the instance was made from; they mean nothing for another one.
    if v.get("main").and_then(Value::as_str).and_then(parse_guid_pair) != Some(main_guid) {
        return None;
    }
    let entries = v.get("entries").filter(|e| e.as_array().is_some_and(|a| !a.is_empty()))?.clone();
    let blobs = v
        .get("blobs")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, b)| Some((k.parse::<u64>().ok()?, decode_hex_bytes(b.as_str()?)?))).collect())
        .unwrap_or_default();
    let props = v
        .get("props")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, n)| Some((k.clone(), n.as_str()?.to_string()))).collect())
        .unwrap_or_default();
    Some(Kept { entries, blobs, props })
}

/// The kiwi field `NodeChange.symbolData.symbolOverrides` of the schema being written.
fn symbol_overrides_field(schema: &crate::kiwi::Schema, nc_def: usize) -> Option<&crate::kiwi::Field> {
    let symbol_data = schema.defs.get(nc_def)?.field_named("symbolData")?;
    let FieldType::Def(sd_idx) = symbol_data.ty else { return None };
    schema.defs.get(sd_idx)?.field_named("symbolOverrides")
}

fn field_guid(v: &V) -> Option<(u32, u32)> {
    let num = |k: &str| match v.get(k)? {
        V::Uint(u) => Some(*u),
        V::Int(i) => u32::try_from(*i).ok(),
        V::Uint64(u) => u32::try_from(*u).ok(),
        _ => None,
    };
    Some((num("sessionID")?, num("localID")?))
}

fn message_fields_mut(v: &mut V) -> Option<&mut Vec<(std::sync::Arc<str>, V)>> {
    match v {
        V::Message(f) | V::Struct(f) => Some(f),
        _ => None,
    }
}

fn kiwi_float(v: &V) -> Option<f32> {
    match v {
        V::Float(f) => Some(*f),
        V::Int(i) => Some(*i as f32),
        V::Uint(u) => Some(*u as f32),
        _ => None,
    }
}

fn own_float(own_entries: &[&V], key: &str) -> Option<f32> {
    own_entries.iter().rev().find_map(|e| e.get(key).and_then(kiwi_float))
}

fn own_size_axis(own_entries: &[&V], axis: &str) -> Option<f32> {
    own_entries.iter().rev().find_map(|e| e.get("size").and_then(|s| s.get(axis)).and_then(kiwi_float))
}

fn consumption_field_removed(field: &str, key: OverrideField) -> bool {
    matches!(
        (field, key),
        ("WIDTH" | "HEIGHT", OverrideField::Size)
            | ("VISIBLE", OverrideField::Visible)
            | ("OPACITY", OverrideField::Opacity)
            | ("STROKE_WEIGHT" | "BORDER_TOP_WEIGHT" | "BORDER_RIGHT_WEIGHT" | "BORDER_BOTTOM_WEIGHT" | "BORDER_LEFT_WEIGHT", OverrideField::Stroke)
            | (
                "CORNER_RADIUS"
                    | "RECTANGLE_CORNER_RADII"
                    | "RECTANGLE_TOP_LEFT_CORNER_RADIUS"
                    | "RECTANGLE_TOP_RIGHT_CORNER_RADIUS"
                    | "RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS"
                    | "RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS",
                OverrideField::CornerRadii
            )
    )
}

fn stale_consumption_fields(
    node: &Node,
    main_node: Option<&Node>,
    opened: &BTreeMap<OverrideField, u64>,
    own_entries: &[&V],
    edited: &BTreeSet<OverrideField>,
    reset: &BTreeSet<OverrideField>,
) -> BTreeSet<&'static str> {
    let mut stale = BTreeSet::new();
    const ALL_CONSUMPTION_FIELDS: [&str; 15] = [
        "WIDTH",
        "HEIGHT",
        "VISIBLE",
        "OPACITY",
        "STROKE_WEIGHT",
        "BORDER_TOP_WEIGHT",
        "BORDER_RIGHT_WEIGHT",
        "BORDER_BOTTOM_WEIGHT",
        "BORDER_LEFT_WEIGHT",
        "CORNER_RADIUS",
        "RECTANGLE_CORNER_RADII",
        "RECTANGLE_TOP_LEFT_CORNER_RADIUS",
        "RECTANGLE_TOP_RIGHT_CORNER_RADIUS",
        "RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS",
        "RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS",
    ];
    for &field in reset {
        for &name in &ALL_CONSUMPTION_FIELDS {
            if consumption_field_removed(name, field) {
                stale.insert(name);
            }
        }
    }
    if edited.contains(&OverrideField::Visible) {
        stale.insert("VISIBLE");
    }
    if edited.contains(&OverrideField::Opacity) {
        stale.insert("OPACITY");
    }
    if edited.contains(&OverrideField::Size) {
        let opened_size = crate::instance_self_digest::opened_size(opened);
        let axis_changed = |cur: f32, opened_axis: Option<f32>, axis_key: &str, main_axis: Option<f32>| -> bool {
            if opened_axis.is_some_and(|o| (cur - o).abs() <= 1e-5) {
                return false;
            }
            if let Some(rec) = own_size_axis(own_entries, axis_key) {
                (cur - rec).abs() > 1e-5
            } else if let Some(o) = opened_axis {
                (cur - o).abs() > 1e-5
            } else {
                main_axis.is_none_or(|m| (cur - m).abs() > 1e-5)
            }
        };
        if axis_changed(node.size.x, opened_size.map(|(x, _)| x), "x", main_node.map(|m| m.size.x)) {
            stale.insert("WIDTH");
        }
        if axis_changed(node.size.y, opened_size.map(|(_, y)| y), "y", main_node.map(|m| m.size.y)) {
            stale.insert("HEIGHT");
        }
    }
    if edited.contains(&OverrideField::Stroke) {
        let old_sw = own_float(own_entries, "strokeWeight").or_else(|| main_node.map(|m| m.stroke.weight));
        if old_sw.is_none_or(|w| (node.stroke.weight - w).abs() > 1e-5) {
            stale.insert("STROKE_WEIGHT");
        }
        for (name, key, idx) in [
            ("BORDER_TOP_WEIGHT", "borderTopWeight", 0usize),
            ("BORDER_RIGHT_WEIGHT", "borderRightWeight", 1usize),
            ("BORDER_BOTTOM_WEIGHT", "borderBottomWeight", 2usize),
            ("BORDER_LEFT_WEIGHT", "borderLeftWeight", 3usize),
        ] {
            let cur = node.stroke.side_weights.map(|sw| sw[idx]).unwrap_or(node.stroke.weight);
            let old = own_float(own_entries, key)
                .or_else(|| own_float(own_entries, "strokeWeight"))
                .or_else(|| main_node.map(|m| m.stroke.side_weights.map(|sw| sw[idx]).unwrap_or(m.stroke.weight)));
            if old.is_none_or(|w| (cur - w).abs() > 1e-5) {
                stale.insert(name);
            }
        }
    }
    if edited.contains(&OverrideField::CornerRadii) {
        let mut any_corner_changed = false;
        for (name, key, idx) in [
            ("RECTANGLE_TOP_LEFT_CORNER_RADIUS", "rectangleTopLeftCornerRadius", 0usize),
            ("RECTANGLE_TOP_RIGHT_CORNER_RADIUS", "rectangleTopRightCornerRadius", 1usize),
            ("RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS", "rectangleBottomRightCornerRadius", 2usize),
            ("RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS", "rectangleBottomLeftCornerRadius", 3usize),
        ] {
            let cur = node.corner_radii[idx];
            let old = own_float(own_entries, key).or_else(|| own_float(own_entries, "cornerRadius")).or_else(|| main_node.map(|m| m.corner_radii[idx]));
            if old.is_none_or(|r| (cur - r).abs() > 1e-5) {
                stale.insert(name);
                if idx == 0 {
                    stale.insert("CORNER_RADIUS");
                }
                any_corner_changed = true;
            }
        }
        if any_corner_changed {
            stale.insert("RECTANGLE_CORNER_RADII");
        }
    }
    stale
}

fn drop_stale_consumption_entries(record: &mut Vec<(std::sync::Arc<str>, V)>, stale_fields: &BTreeSet<&'static str>) {
    for (key, value) in record.iter_mut() {
        if !matches!(&**key, "variableConsumptionMap" | "parameterConsumptionMap") {
            continue;
        }
        let Some(entries) = value.get_mut("entries").and_then(|v| match v {
            V::Array(a) => Some(a),
            _ => None,
        }) else {
            continue;
        };
        entries.retain(|entry| {
            let Some(variable_field) = entry.get("variableField") else { return true };
            let name = match variable_field {
                V::Enum(name) => name.as_ref(),
                V::String(name) => name.as_str(),
                _ => return true,
            };
            !stale_fields.contains(name)
        });
    }
    record.retain(|(key, value)| {
        !matches!(&**key, "variableConsumptionMap" | "parameterConsumptionMap")
            || value.get("entries").and_then(V::as_array).is_none_or(|entries| !entries.is_empty())
    });
}

/// Re-adds the blobs a geometry list names to this file's table and points the list at them. A blob the loader holds
/// no bytes for leaves its geometry out: an index into another file's table would draw something else.
fn remap_geometry(cx: &mut ExportCtx<'_>, kept: &Kept, list: &mut V) {
    let V::Array(items) = list else { return };
    items.retain_mut(|item| {
        let Some(fields) = message_fields_mut(item) else { return false };
        let Some(slot) = fields.iter_mut().find(|(k, _)| &**k == "commandsBlob") else { return false };
        let old = match &slot.1 {
            V::Uint(u) => u64::from(*u),
            V::Int(i) => u64::try_from(*i).unwrap_or(u64::MAX),
            V::Uint64(u) => *u,
            _ => return false,
        };
        let Some(bytes) = kept.blobs.get(&old) else { return false };
        slot.1 = V::Uint(cx.add_blob(bytes.clone()));
        true
    });
}

/// The node an entry's `guidPath` ends at, inside the main component: the one whose component properties the entry sets.
fn entry_target(cx: &mut ExportCtx<'_>, entry: &V, inst_id: NodeId, main_guid: (u32, u32)) -> NodeId {
    let last = entry.get("guidPath").and_then(|p| p.get("guids")).and_then(V::as_array).and_then(|a| a.last()).and_then(field_guid);
    match last {
        Some(g) if g != main_guid => cx.node_with_guid(g).unwrap_or(inst_id),
        _ => inst_id,
    }
}

/// Points each component property an entry sets at the definition this file writes for that property.
fn remap_prop_assignments(cx: &mut ExportCtx<'_>, kept: &Kept, entry: &mut V, inst_id: NodeId, main_guid: (u32, u32)) {
    if entry.get("componentPropAssignments").is_none() {
        return;
    }
    let scope = entry_target(cx, entry, inst_id, main_guid);
    let Some(V::Array(assigns)) = entry.get_mut("componentPropAssignments") else { return };
    for a in assigns {
        let Some(old) = a.get("defID").and_then(field_guid) else { continue };
        let Some(name) = kept.props.get(&format!("{}:{}", old.0, old.1)) else { continue };
        let g = cx.resolve_prop_guid(scope, name);
        if let Some(slot) = a.get_mut("defID") {
            *slot = guid(g.0, g.1);
        }
    }
}

/// The `symbolOverrides` entries of the instance `inst_id` (of the main component `main`) as the file stated them, or
/// `None` to write the instance the usual way: its page is loaded, it already has children, the loader recorded nothing,
/// or it is an instance of another main component than the one the entries were written for.
///
/// Entries that the writer's schema cannot hold whole are written without the parts it cannot hold, never as a value
/// that stands for something else.
pub(super) fn pending_symbol_overrides(cx: &mut ExportCtx<'_>, inst_id: NodeId, main: NodeId) -> Option<Vec<V>> {
    if !cx.doc.get(inst_id)?.children.is_empty() {
        return None;
    }
    let main_guid = cx.node_guids.get(&main).copied()?;
    let kept = parse_kept(&cx.doc.pending_instance_overrides(inst_id)?, main_guid)?;
    let schema = cx.schema;
    let field = symbol_overrides_field(schema, cx.nc_def)?;
    // Every part of every entry must fit, or the entries are read again keeping what fits.
    let whole = schema.field_from_json_strict(field, &kept.entries, 0);
    let V::Array(mut entries) = whole.or_else(|| schema.field_from_json(field, &kept.entries, 0))? else { return None };
    // The schema holds Figma's whole `NodeChange`, so an entry can now carry fields no writer gates yet (prototype
    // interactions, text layout versions); they stay out of it, at every depth, exactly as before the schema knew them.
    entries.retain_mut(|e| !strip_held_back(schema, cx.nc_def, cx.nc_def, e, 0));
    for e in &mut entries {
        for key in ["fillGeometry", "strokeGeometry"] {
            if let Some(list) = e.get_mut(key) {
                remap_geometry(cx, &kept, list);
            }
        }
        remap_prop_assignments(cx, &kept, e, inst_id, main_guid);
    }
    restate_edited_self_fields(cx, inst_id, main, main_guid, &mut entries);
    (!entries.is_empty()).then_some(entries)
}

/// The instance's own entry is the one whose path is its main's guid alone.
fn is_own_entry(entry: &V, main_guid: (u32, u32)) -> bool {
    let guids = entry.get("guidPath").and_then(|p| p.get("guids")).and_then(V::as_array);
    guids.is_some_and(|g| g.len() == 1 && field_guid(&g[0]) == Some(main_guid))
}

/// The fields of the unopened instance `inst_id` that the user changed since the file was opened (see
/// [`crate::instance_self_digest`]).
pub(super) fn edited_self_fields(doc: &Document, inst_id: NodeId) -> BTreeSet<OverrideField> {
    let opened = doc.pending_instance_self_digests(inst_id).unwrap_or_default();
    doc.get(inst_id).map(|node| edited_fields(node, &opened).into_iter().collect()).unwrap_or_default()
}

/// Whether the unopened instance `inst_id` has another size than when the file was opened, flagged or not.
pub(super) fn size_edited(doc: &Document, inst_id: NodeId) -> bool {
    let opened = doc.pending_instance_self_digests(inst_id).unwrap_or_default();
    doc.get(inst_id).is_some_and(|node| crate::instance_self_digest::size_moved(node, &opened) || edited_fields(node, &opened).contains(&OverrideField::Size))
}

fn ordered_track_guids(entry: &V, key: &str) -> Option<Vec<(u32, u32)>> {
    let V::Array(items) = entry.get(key)?.get("entries")? else { return None };
    let mut pairs: Vec<(&str, (u32, u32))> = items
        .iter()
        .filter_map(|item| {
            let pos = match item.get("position")? {
                V::String(s) => s.as_ref(),
                _ => return None,
            };
            Some((pos, field_guid(item.get("id")?)?))
        })
        .collect();
    if pairs.is_empty() {
        return None;
    }
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    Some(pairs.into_iter().map(|(_, g)| g).collect())
}

fn ordered_track_guids_in_own(entries: &[V], own: &[usize], key: &str) -> Vec<(u32, u32)> {
    own.iter().rev().find_map(|&i| ordered_track_guids(&entries[i], key)).unwrap_or_default()
}

fn remap_child_grid_anchors(entries: &mut [V], own: &[usize], anchor_key: &str, old_ids: &[(u32, u32)], new_ids: &[(u32, u32)]) {
    if old_ids.is_empty() || new_ids.is_empty() {
        return;
    }
    for (i, entry) in entries.iter_mut().enumerate() {
        if own.contains(&i) {
            continue;
        }
        let Some(slot) = entry.get_mut(anchor_key) else { continue };
        let Some(old) = field_guid(slot) else { continue };
        if let Some(&(s, l)) = old_ids.iter().position(|&g| g == old).and_then(|idx| new_ids.get(idx)) {
            *slot = guid(s, l);
        }
    }
}

fn remove_own_key(entries: &mut [V], own: &[usize], key: &str) {
    for &i in own {
        if let Some(record) = message_fields_mut(&mut entries[i]) {
            record.retain(|(k, _)| &**k != key);
        }
    }
}

fn set_own_key(entries: &mut [V], own: &[usize], last: usize, key: &str, val: V) {
    for &i in own {
        if i != last {
            if let Some(record) = message_fields_mut(&mut entries[i]) {
                record.retain(|(k, _)| &**k != key);
            }
        }
    }
    if let Some(record) = message_fields_mut(&mut entries[last]) {
        if let Some(slot) = record.iter_mut().find(|(k, _)| &**k == key) {
            slot.1 = val;
        } else {
            record.push((key.into(), val));
        }
    }
}

/// Whether a grid track list can be restated from the node: the count the user left, the count the node was seeded
/// with, the count the record states and the count the writer numbers must all be the same. Otherwise the writer's ids
/// belong to another frame than the record's and the track sizes would land on the wrong tracks (or on tracks that were
/// removed), so the tracks stay as recorded and the edit is not saved.
fn track_counts_agree(edited: u32, seeded: Option<u32>, recorded: usize, numbered: usize) -> bool {
    seeded == Some(edited) && recorded == numbered && numbered == edited as usize
}

fn copy_fresh_key(entries: &mut [V], own: &[usize], last: usize, fresh: &V, key: &str) {
    if let Some(val) = fresh.get(key).cloned() {
        set_own_key(entries, own, last, key, val);
    }
}

fn own_has_key(entries: &[V], own: &[usize], key: &str) -> bool {
    own.iter().any(|&i| entries[i].get(key).is_some())
}

fn restate_edited_layout_fields(cx: &mut ExportCtx<'_>, inst_id: NodeId, main_guid: (u32, u32), entries: &mut [V], own: &[usize], last: usize) {
    let layout_only = BTreeSet::from([OverrideField::Layout]);
    let Some(fresh) = collect_instance_overrides_for(cx, inst_id, Some(&layout_only)).into_iter().find(|e| is_own_entry(e, main_guid)) else {
        return;
    };
    let Some(node) = cx.doc.get(inst_id) else { return };
    let Some(now_fp) = node.kind.frame_props() else { return };
    let now_cleared = node.component.layout_cleared;
    let opened = cx.doc.pending_instance_self_digests(inst_id).unwrap_or_default();
    let (seeded_fp, seeded_cleared) = crate::instance_self_digest::unopened_layout_snapshot(&opened)
        .map(|s| (s.seeded_fp, s.seeded_cleared))
        .or_else(|| {
            let comp = match &node.kind {
                NodeKind::Instance { component: Some(c), .. } => Some(*c),
                _ => node.component.source,
            }?;
            Some((cx.doc.get(comp)?.kind.frame_props()?.clone(), false))
        })
        .unwrap_or_default();

    if now_fp.clips_content != seeded_fp.clips_content {
        copy_fresh_key(entries, own, last, &fresh, "frameMaskDisabled");
    }
    if now_fp.grids != seeded_fp.grids {
        if now_fp.grids.is_empty() {
            remove_own_key(entries, own, "layoutGrids");
        } else {
            copy_fresh_key(entries, own, last, &fresh, "layoutGrids");
        }
    }

    let Some(now_al) = now_fp.auto_layout.as_ref() else {
        if now_cleared && (seeded_fp.auto_layout.is_some() || !seeded_cleared) {
            for &i in own {
                if let Some(record) = message_fields_mut(&mut entries[i]) {
                    record.retain(|(k, _)| {
                        !matches!(
                            &**k,
                            "gridColumns"
                                | "gridColumnsSizing"
                                | "gridRows"
                                | "gridRowsSizing"
                                | "gridAutoTracks"
                                | "gridColumnGap"
                                | "gridRowGap"
                                | "stackMode"
                                | "stackSpacing"
                                | "stackCounterSpacing"
                                | "stackPadding"
                                | "stackHorizontalPadding"
                                | "stackVerticalPadding"
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
                        )
                    });
                }
            }
            copy_fresh_key(entries, own, last, &fresh, "stackMode");
        }
        return;
    };

    let seeded_al = seeded_fp.auto_layout.as_ref();
    if seeded_al.map(|a| a.direction) != Some(now_al.direction) || seeded_cleared != now_cleared {
        copy_fresh_key(entries, own, last, &fresh, "stackMode");
    }
    if seeded_al.map(|a| a.item_spacing) != Some(now_al.item_spacing) {
        let key = if now_al.direction == LayoutDirection::Grid || (own_has_key(entries, own, "gridColumnGap") && !own_has_key(entries, own, "stackSpacing")) {
            "gridColumnGap"
        } else {
            "stackSpacing"
        };
        set_own_key(entries, own, last, key, V::Float(now_al.item_spacing));
    }
    if seeded_al.map(|a| a.counter_spacing) != Some(now_al.counter_spacing) {
        let key = if now_al.direction == LayoutDirection::Grid || (own_has_key(entries, own, "gridRowGap") && !own_has_key(entries, own, "stackCounterSpacing"))
        {
            "gridRowGap"
        } else {
            "stackCounterSpacing"
        };
        match now_al.counter_spacing {
            Some(gap) => set_own_key(entries, own, last, key, V::Float(gap)),
            // The grid row gap is always stated; for a stack an unset gap is simply absent from the record.
            None if key == "gridRowGap" => set_own_key(entries, own, last, key, V::Float(now_al.line_gap())),
            None => remove_own_key(entries, own, key),
        }
    }
    if seeded_al.map(|a| a.padding) != Some(now_al.padding) {
        let uniform = now_al.padding[0] == now_al.padding[1] && now_al.padding[1] == now_al.padding[2] && now_al.padding[2] == now_al.padding[3];
        let has_side_keys = own_has_key(entries, own, "stackVerticalPadding")
            || own_has_key(entries, own, "stackPaddingRight")
            || own_has_key(entries, own, "stackPaddingBottom")
            || own_has_key(entries, own, "stackHorizontalPadding");
        if uniform && own_has_key(entries, own, "stackPadding") && !has_side_keys {
            set_own_key(entries, own, last, "stackPadding", V::Float(now_al.padding[0]));
        } else if own_has_key(entries, own, "stackPadding") {
            remove_own_key(entries, own, "stackPadding");
            for (idx, key) in [(0, "stackVerticalPadding"), (1, "stackPaddingRight"), (2, "stackPaddingBottom"), (3, "stackHorizontalPadding")] {
                set_own_key(entries, own, last, key, V::Float(now_al.padding[idx]));
            }
        } else {
            for (idx, key) in [(0, "stackVerticalPadding"), (1, "stackPaddingRight"), (2, "stackPaddingBottom"), (3, "stackHorizontalPadding")] {
                if seeded_al.map(|a| a.padding[idx]) != Some(now_al.padding[idx]) {
                    set_own_key(entries, own, last, key, V::Float(now_al.padding[idx]));
                }
            }
        }
    }
    if seeded_al.map(|a| a.primary_align) != Some(now_al.primary_align) {
        copy_fresh_key(entries, own, last, &fresh, "stackPrimaryAlignItems");
    }
    if seeded_al.map(|a| a.counter_align) != Some(now_al.counter_align) {
        copy_fresh_key(entries, own, last, &fresh, "stackCounterAlignItems");
    }
    if seeded_al.map(|a| a.counter_align_content) != Some(now_al.counter_align_content) {
        copy_fresh_key(entries, own, last, &fresh, "stackCounterAlignContent");
    }
    if seeded_al.map(|a| a.layout_version) != Some(now_al.layout_version) {
        copy_fresh_key(entries, own, last, &fresh, "stackLayoutVersion");
    }
    if seeded_al.map(|a| a.primary_sizing) != Some(now_al.primary_sizing) {
        copy_fresh_key(entries, own, last, &fresh, "stackPrimarySizing");
    }
    if seeded_al.map(|a| a.counter_sizing) != Some(now_al.counter_sizing) {
        copy_fresh_key(entries, own, last, &fresh, "stackCounterSizing");
    }
    if seeded_al.map(|a| a.wrap) != Some(now_al.wrap) {
        copy_fresh_key(entries, own, last, &fresh, "stackWrap");
    }
    if seeded_al.map(|a| a.stroke_included) != Some(now_al.stroke_included) {
        copy_fresh_key(entries, own, last, &fresh, "bordersTakeSpace");
    }
    if seeded_al.map(|a| a.reverse_z) != Some(now_al.reverse_z) {
        copy_fresh_key(entries, own, last, &fresh, "stackReverseZIndex");
    }
    if seeded_al.map(|a| a.grid_auto_tracks) != Some(now_al.grid_auto_tracks) {
        copy_fresh_key(entries, own, last, &fresh, "gridAutoTracks");
    }
    if seeded_al.map(|a| (a.grid_columns, &a.grid_col_tracks)) != Some((now_al.grid_columns, &now_al.grid_col_tracks)) {
        let old_cols = ordered_track_guids_in_own(entries, own, "gridColumns");
        let new_cols = ordered_track_guids(&fresh, "gridColumns").unwrap_or_default();
        if track_counts_agree(now_al.grid_columns, seeded_al.map(|a| a.grid_columns), old_cols.len(), new_cols.len()) {
            copy_fresh_key(entries, own, last, &fresh, "gridColumns");
            copy_fresh_key(entries, own, last, &fresh, "gridColumnsSizing");
            remap_child_grid_anchors(entries, own, "gridColumnAnchor", &old_cols, &new_cols);
        }
    }
    if seeded_al.map(|a| (a.grid_rows, &a.grid_row_tracks)) != Some((now_al.grid_rows, &now_al.grid_row_tracks)) {
        let old_rows = ordered_track_guids_in_own(entries, own, "gridRows");
        let new_rows = ordered_track_guids(&fresh, "gridRows").unwrap_or_default();
        if track_counts_agree(now_al.grid_rows, seeded_al.map(|a| a.grid_rows), old_rows.len(), new_rows.len()) {
            copy_fresh_key(entries, own, last, &fresh, "gridRows");
            copy_fresh_key(entries, own, last, &fresh, "gridRowsSizing");
            remap_child_grid_anchors(entries, own, "gridRowAnchor", &old_rows, &new_rows);
        }
    }
}

/// The record states the instance's own fields as they were when the file was opened. A field the user changed on the
/// instance since then (its digest moved, see [`crate::instance_self_digest`]) is written from the node instead, in
/// place of what the record says for it; every other part of the record is written as it was.
fn restate_edited_self_fields(cx: &mut ExportCtx<'_>, inst_id: NodeId, main: NodeId, main_guid: (u32, u32), entries: &mut Vec<V>) {
    let edited = edited_self_fields(cx.doc, inst_id);
    let opened = cx.doc.pending_instance_self_digests(inst_id).unwrap_or_default();
    let Some(node) = cx.doc.get(inst_id) else { return };
    // A reset removes the override flag before export. The opened digest still records that field, but
    // `edited_fields` cannot see it because it only walks fields currently flagged on the node. Remove the
    // corresponding kept record instead of writing the stale value back.
    let mut reset = BTreeSet::new();
    for entry in entries.iter() {
        if !is_own_entry(entry, main_guid) {
            continue;
        }
        let fields = match entry {
            V::Message(fields) | V::Struct(fields) => fields,
            _ => continue,
        };
        for (key, _) in fields.iter() {
            if let Some(field) = field_removed_with(key) {
                if opened.contains_key(&field) && !node.component.has_override(&field) {
                    reset.insert(field);
                }
            }
        }
    }
    if edited.is_empty() && reset.is_empty() {
        return;
    }
    let mut own: Vec<usize> = entries.iter().enumerate().filter(|(_, e)| is_own_entry(e, main_guid)).map(|(i, _)| i).collect();
    if own.is_empty() {
        let Some(mut fresh) = collect_instance_overrides_for(cx, inst_id, Some(&edited)).into_iter().find(|e| is_own_entry(e, main_guid)) else {
            return;
        };
        if let Some(fields) = message_fields_mut(&mut fresh) {
            fields.retain(|(k, _)| &**k == "guidPath");
        }
        entries.insert(0, fresh);
        own.push(0);
    }
    let last = *own.last().expect("own entry exists");

    // The instance has no children, so what it reads from the node is its own entry alone. A reset has no fresh
    // value to append; edited fields still replace the old value with the current node state. The layout group is not
    // restated wholesale from the childless node: only the layout keys the edit changed are written (below).
    let non_layout: BTreeSet<OverrideField> = edited.iter().copied().filter(|&f| f != OverrideField::Layout).collect();
    let mut restated = if non_layout.is_empty() {
        None
    } else {
        collect_instance_overrides_for(cx, inst_id, Some(&non_layout)).into_iter().find(|e| is_own_entry(e, main_guid)).and_then(|mut fresh| {
            let mut fields = message_fields_mut(&mut fresh).map(std::mem::take)?;
            fields.retain(|(k, _)| &**k != "guidPath");
            Some(fields)
        })
    };
    let stale_consumption = {
        let Some(node) = cx.doc.get(inst_id) else { return };
        let main_node = cx.doc.get(main);
        let own_refs: Vec<&V> = own.iter().map(|&i| &entries[i]).collect();
        stale_consumption_fields(node, main_node, &opened, &own_refs, &edited, &reset)
    };
    let mut removed = non_layout;
    removed.extend(reset);
    // Entries on one path apply in list order, so a key left in an earlier one would still be read where the last does not
    // override it: the old value goes from all of them and the new one is stated by the last.
    for &i in &own {
        let Some(record) = message_fields_mut(&mut entries[i]) else { continue };
        record.retain(|(k, _)| !field_removed_with(k).is_some_and(|f| removed.contains(&f)));
        drop_stale_consumption_entries(record, &stale_consumption);
    }
    if let (Some(record), Some(restated)) = (message_fields_mut(&mut entries[last]), restated.take()) {
        record.extend(restated);
    }
    if edited.contains(&OverrideField::Layout) {
        restate_edited_layout_fields(cx, inst_id, main_guid, entries, &own, last);
    }
    entries.retain(|entry| {
        !is_own_entry(entry, main_guid)
            || match entry {
                V::Message(fields) | V::Struct(fields) => fields.iter().any(|(k, _)| &**k != "guidPath"),
                _ => true,
            }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::figma_like_schema;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    /// Answers one fixed payload for every instance, the way a loader with unloaded pages would.
    #[derive(Debug)]
    struct FixedPayload(String);

    impl PageHydrator for FixedPayload {
        fn hydrate_page(&mut self, _doc: &mut Document, _page: NodeId) -> bool {
            false
        }
        fn hydrate_all(&mut self, _doc: &mut Document) -> bool {
            false
        }
        fn is_hydrated(&self, _page: NodeId) -> bool {
            false
        }
        fn pending_instance_overrides(&self, _instance: NodeId) -> Option<String> {
            Some(self.0.clone())
        }
    }

    /// A document with a main component and an unexpanded instance of it, whose loader answers `payload`.
    fn document_with_pending_instance(payload: serde_json::Value) -> (Document, NodeId, NodeId) {
        let mut doc = Document::new("kept");
        let page = doc.add_page("Page");
        let main_id = doc.alloc_id();
        let mut main = Node::new(main_id, NodeKind::Component(FrameProps::default()));
        main.source_id = Some("1:10".into());
        doc.insert(page, main).unwrap();
        let inst_id = doc.alloc_id();
        let inst = Node::new(inst_id, NodeKind::Instance { frame: FrameProps::default(), component: Some(main_id), component_key: None });
        doc.insert(page, inst).unwrap();
        doc.page_hydrator = Some(Arc::new(Mutex::new(FixedPayload(payload.to_string()))));
        (doc, main_id, inst_id)
    }

    fn payload(entries: serde_json::Value) -> serde_json::Value {
        json!({"main": "1:10", "entries": entries, "blobs": {}, "props": {}})
    }

    fn text_entry() -> serde_json::Value {
        json!({"guidPath": {"guids": [{"sessionID": 1, "localID": 11}]}, "textData": {"characters": "Go back"}})
    }

    fn write(doc: &Document, main: NodeId, inst: NodeId) -> Option<Vec<V>> {
        let schema = figma_like_schema();
        let mut cx = ExportCtx::new(doc, &schema);
        pending_symbol_overrides(&mut cx, inst, main)
    }

    #[test]
    fn entries_are_written_as_the_loader_recorded_them() {
        let (doc, main, inst) = document_with_pending_instance(payload(json!([text_entry()])));
        let entries = write(&doc, main, inst).expect("entries");
        assert_eq!(entries.len(), 1);
        let text = entries[0].get("textData").and_then(|t| t.get("characters"));
        assert_eq!(text, Some(&V::String("Go back".into())));
    }

    #[test]
    fn a_key_the_writer_schema_does_not_know_is_left_out_and_the_rest_of_the_entry_is_kept() {
        let mut entry = text_entry();
        entry["fieldOfALaterFormat"] = json!(7);
        let (doc, main, inst) = document_with_pending_instance(payload(json!([entry])));
        let entries = write(&doc, main, inst).expect("entries");
        assert!(entries[0].get("fieldOfALaterFormat").is_none());
        assert!(entries[0].get("textData").is_some());
    }

    #[test]
    fn a_part_that_does_not_fit_its_field_is_left_out_and_the_rest_of_the_entry_is_kept() {
        let mut entry = text_entry();
        entry["opacity"] = json!("not a number");
        let (doc, main, inst) = document_with_pending_instance(payload(json!([entry])));
        let entries = write(&doc, main, inst).expect("entries");
        assert!(entries[0].get("opacity").is_none());
        assert!(entries[0].get("textData").is_some());
    }

    #[test]
    fn an_instance_that_has_children_is_read_from_them() {
        let (mut doc, main, inst) = document_with_pending_instance(payload(json!([text_entry()])));
        let child_id = doc.alloc_id();
        doc.insert(inst, Node::rectangle(child_id, Rect::new(0.0, 0.0, 4.0, 4.0))).unwrap();
        assert!(write(&doc, main, inst).is_none());
    }

    #[test]
    fn a_payload_for_another_component_is_not_used() {
        let (doc, main, inst) = document_with_pending_instance(json!({"main": "1:99", "entries": [text_entry()]}));
        assert!(write(&doc, main, inst).is_none());
    }
}
