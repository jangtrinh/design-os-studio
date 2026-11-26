//! What the writers of the records that are not layers read from a kept record: its type, its field numbers, the fields decoded
//! against this save's ids.

use super::kept_fields::{decode_record_field, IdMaps};
use super::ExportCtx;
use crate::kiwi::KiwiValue as V;
use designos_doc::{KeptRecordKind, KeptRecordRef};
use std::collections::HashSet;
use std::sync::Arc;

/// The field numbers the file's schema gives the fields this module reads, each `None` when the schema has none.
#[derive(Clone, Copy, Default)]
pub(super) struct Numbers {
    pub(super) guid: Option<u32>,
    pub(super) ty: Option<u32>,
    pub(super) style_type: Option<u32>,
    pub(super) internal_only: Option<u32>,
    pub(super) is_soft_deleted: Option<u32>,
    pub(super) is_soft_deleted_style: Option<u32>,
    pub(super) variable_set_id: Option<u32>,
    pub(super) variable_set_modes: Option<u32>,
}

impl Numbers {
    pub(super) fn of(cx: &ExportCtx<'_>) -> Self {
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
pub(super) struct Shape {
    pub(super) ty: String,
    pub(super) style_type: Option<String>,
    pub(super) internal_only: bool,
    pub(super) soft_deleted: bool,
}

impl Shape {
    /// A label for the report: the node type, and the style type of a style.
    pub(super) fn label(&self) -> String {
        match &self.style_type {
            Some(st) => format!("{}/{st}", self.ty),
            None => self.ty.clone(),
        }
    }
}

pub(super) fn shape_of(cx: &ExportCtx<'_>, nums: &Numbers, rec: KeptRecordRef<'_>) -> Shape {
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
pub(super) fn other_record<'a>(cx: &ExportCtx<'a>, g: (u32, u32)) -> Option<KeptRecordRef<'a>> {
    if !cx.kept.is_active() {
        return None;
    }
    let doc = cx.doc;
    doc.fig_kept.record_by_guid(g).filter(|r| r.kind() == KeptRecordKind::Other)
}

/// `value` is a `GUID` struct: its session and local number.
pub(super) fn guid_value(value: &V) -> Option<(u32, u32)> {
    match value {
        V::Struct(f) | V::Message(f) => match f.as_slice() {
            [(a, V::Uint(s)), (b, V::Uint(l))] if &**a == "sessionID" && &**b == "localID" => Some((*s, *l)),
            _ => None,
        },
        _ => None,
    }
}

/// Puts `value` in `record` under `name`, in place of the field of that name Studio wrote.
pub(super) fn put(record: &mut V, name: Arc<str>, value: V) {
    let V::Message(fields) = record else { return };
    match fields.iter_mut().find(|(k, _)| *k == name) {
        Some(slot) => slot.1 = value,
        None => fields.push((name, value)),
    }
}

/// What the kept fields of one record came to.
#[derive(Default)]
pub(super) struct Decoded {
    pub(super) fields: Vec<(Arc<str>, V)>,
    /// Why each field that was left out was.
    pub(super) refused: Vec<&'static str>,
    pub(super) values_dropped: usize,
}

/// Decodes the kept fields of `rec` against `maps`, leaving out the record's own guid. `modes` names the modes a variable's values
/// may be for: an entry of `variableDataValues` for any other mode is taken out and counted.
pub(super) fn decode_kept(cx: &mut ExportCtx<'_>, maps: &IdMaps, nums: &Numbers, rec: KeptRecordRef<'_>, modes: Option<&HashSet<(u32, u32)>>) -> Decoded {
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
pub(super) fn retain_values_of_modes(value: &mut V, modes: &HashSet<(u32, u32)>) -> usize {
    let V::Message(fields) = value else { return 0 };
    let Some((_, V::Array(entries))) = fields.iter_mut().find(|(k, _)| &**k == "entries") else { return 0 };
    let before = entries.len();
    entries.retain(|e| match e.get("modeID").and_then(guid_value) {
        Some(g) => modes.contains(&g),
        None => true,
    });
    before - entries.len()
}

pub(super) fn values_are_empty(value: &V) -> bool {
    value.get("entries").and_then(V::as_array).is_some_and(<[V]>::is_empty)
}

/// The modes a value of a variable of set `set` may be for: the set's own, and those of the collections that extend one.
pub(super) fn modes_for_variable(cx: &ExportCtx<'_>, maps: &IdMaps, set: (u32, u32)) -> HashSet<(u32, u32)> {
    let mut modes = maps.modes_of_set.get(&set).cloned().unwrap_or_default();
    for c in cx.doc.variable_collections.values().filter(|c| c.is_extension()) {
        if let Some(own) = cx.coll_guids.get(&c.id).and_then(|g| maps.modes_of_set.get(g)) {
            modes.extend(own.iter().copied());
        }
    }
    modes
}
