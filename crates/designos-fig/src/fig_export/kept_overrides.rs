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
//! writer's schema has no definition for (prototype interactions on an override, text layout versions: Studio reads none
//! of them) is left out of the entry; the rest of the entry is written as it was.

use super::{collect_instance_overrides_for, parse_guid_pair, ExportCtx};
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
    let symbol_data = schema.defs.get(nc_def)?.fields.iter().find(|f| &*f.name == "symbolData")?;
    let FieldType::Def(sd_idx) = symbol_data.ty else { return None };
    schema.defs.get(sd_idx)?.fields.iter().find(|f| &*f.name == "symbolOverrides")
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
    for e in &mut entries {
        for key in ["fillGeometry", "strokeGeometry"] {
            if let Some(list) = e.get_mut(key) {
                remap_geometry(cx, &kept, list);
            }
        }
        remap_prop_assignments(cx, &kept, e, inst_id, main_guid);
    }
    restate_edited_self_fields(cx, inst_id, main_guid, &mut entries);
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

/// The record states the instance's own fields as they were when the file was opened. A field the user changed on the
/// instance since then (its digest moved, see [`crate::instance_self_digest`]) is written from the node instead, in
/// place of what the record says for it; every other part of the record is written as it was.
fn restate_edited_self_fields(cx: &mut ExportCtx<'_>, inst_id: NodeId, main_guid: (u32, u32), entries: &mut Vec<V>) {
    let edited = edited_self_fields(cx.doc, inst_id);
    if edited.is_empty() {
        return;
    }
    // The instance has no children, so what it reads from the node is its own entry alone.
    let Some(mut fresh) = collect_instance_overrides_for(cx, inst_id, Some(&edited)).into_iter().find(|e| is_own_entry(e, main_guid)) else { return };
    let own: Vec<usize> = entries.iter().enumerate().filter(|(_, e)| is_own_entry(e, main_guid)).map(|(i, _)| i).collect();
    let Some(&last) = own.last() else {
        entries.insert(0, fresh);
        return;
    };
    let Some(restated) = message_fields_mut(&mut fresh) else { return };
    restated.retain(|(k, _)| &**k != "guidPath");
    // Entries on one path apply in list order, so a key left in an earlier one would still be read where the last does not
    // override it: the old value goes from all of them and the new one is stated by the last.
    for &i in &own {
        let Some(record) = message_fields_mut(&mut entries[i]) else { continue };
        record.retain(|(k, _)| !field_removed_with(k).is_some_and(|f| edited.contains(&f)));
    }
    if let Some(record) = message_fields_mut(&mut entries[last]) {
        record.append(restated);
    }
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
