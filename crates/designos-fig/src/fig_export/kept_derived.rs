//! Derived data of an instance whose page has not been loaded.
//!
//! A `.fig` stores, next to an instance's overrides, the sizes, transforms and outlines the producing app computed for
//! the nodes inside it (`derivedSymbolData`). The page loader keeps them for instances on pages that are not open; a save
//! that wrote only what is in the document would leave them out, and a reader that takes them from the file (this app
//! does) would show those instances with sizes it works out again, which can differ.
//!
//! They are copied out as the loader holds them, through the same mapping as the override entries (see
//! [`super::kept_overrides`]): node guids are kept, and an outline's blob is added to the table of the file being
//! written. They are true only for the main component they were computed from, so they are written only while that
//! main, and every main inside it, has the content it had when the file was opened.

use super::{encode_matrix, vec2, ExportCtx};
use crate::kiwi::decode_hex_bytes;
use crate::kiwi::KiwiValue as V;
use crate::testkit::{en, guid, msg};
use designos_doc::*;
use serde_json::Value;

fn guid_of(text: &str) -> Option<(u32, u32)> {
    let (session, local) = text.split_once(':')?;
    Some((session.parse().ok()?, local.parse().ok()?))
}

fn floats<const N: usize>(v: &Value) -> Option<[f32; N]> {
    let list = v.as_array()?;
    if list.len() != N {
        return None;
    }
    let mut out = [0.0f32; N];
    for (slot, x) in out.iter_mut().zip(list) {
        *slot = x.as_f64()? as f32;
    }
    Some(out)
}

/// An outline list of one entry, with each blob added to the table of the file being written.
fn geometry(cx: &mut ExportCtx<'_>, list: Option<&Value>) -> Vec<V> {
    list.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let bytes = decode_hex_bytes(g.get("bytes")?.as_str()?)?;
            let rule = if g.get("odd").and_then(Value::as_bool) == Some(true) { "ODD" } else { "NONZERO" };
            Some(msg(vec![("windingRule", en(rule)), ("commandsBlob", V::Uint(cx.add_blob(bytes)))]))
        })
        .collect()
}

fn entry(cx: &mut ExportCtx<'_>, e: &Value) -> Option<V> {
    let steps: Option<Vec<V>> = e.get("path")?.as_array()?.iter().map(|s| guid_of(s.as_str()?).map(|(a, b)| guid(a, b))).collect();
    let mut fields = vec![("guidPath", msg(vec![("guids", V::Array(steps?))]))];
    if let Some([x, y]) = e.get("size").and_then(floats::<2>) {
        fields.push(("size", vec2(x, y)));
    }
    if let Some([a, b, c, d, tx, ty]) = e.get("transform").and_then(floats::<6>) {
        fields.push(("transform", encode_matrix(&Affine { a, b, c, d, tx, ty })));
    }
    for (key, field) in [("fill", "fillGeometry"), ("stroke", "strokeGeometry")] {
        let list = geometry(cx, e.get(key));
        if !list.is_empty() {
            fields.push((field, V::Array(list)));
        }
    }
    (fields.len() > 1).then(|| msg(fields))
}

/// The `derivedSymbolData` entries of the instance `inst_id` (of the main component `main`) as the file stated them, or
/// `None` to write none: the instance has children (its page is loaded), the loader holds none, it is an instance of
/// another main than the data was computed for, or that main (or one inside it) was edited since the file was opened.
pub(super) fn pending_derived_symbol_data(cx: &mut ExportCtx<'_>, inst_id: NodeId, main: NodeId) -> Option<Vec<V>> {
    if !cx.doc.get(inst_id)?.children.is_empty() {
        return None;
    }
    let held: Value = serde_json::from_str(&cx.doc.pending_instance_derived(inst_id)?).ok()?;
    if held.get("main").and_then(Value::as_u64) != Some(main.0) {
        return None;
    }
    let recorded: u64 = held.get("fingerprint")?.as_str()?.parse().ok()?;
    if cx.fingerprints.of_main(cx.doc, main) != recorded {
        return None;
    }
    let entries: Vec<V> = held.get("entries")?.as_array()?.iter().filter_map(|e| entry(cx, e)).collect();
    (!entries.is_empty()).then_some(entries)
}
