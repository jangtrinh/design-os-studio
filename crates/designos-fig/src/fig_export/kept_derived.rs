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
use crate::kiwi::KiwiValue as V;
use crate::testkit::{en, guid, msg};
use designos_doc::*;

fn guid_of(text: &str) -> Option<(u32, u32)> {
    let (session, local) = text.split_once(':')?;
    Some((session.parse().ok()?, local.parse().ok()?))
}

/// An outline list of one entry. Each outline of the file is added to the table of the file being written once, however
/// many entries name it.
fn outlines(cx: &mut ExportCtx<'_>, list: &[PendingOutline]) -> Vec<V> {
    list.iter()
        .map(|o| {
            let index = match cx.shared_blobs.get(&o.id) {
                Some(&i) => i,
                None => {
                    let i = cx.add_blob(o.bytes.to_vec());
                    cx.shared_blobs.insert(o.id, i);
                    i
                }
            };
            msg(vec![("windingRule", en(if o.even_odd { "ODD" } else { "NONZERO" })), ("commandsBlob", V::Uint(index))])
        })
        .collect()
}

fn entry(cx: &mut ExportCtx<'_>, e: &PendingDerivedEntry) -> Option<V> {
    let steps: Option<Vec<V>> = e.path.iter().map(|s| guid_of(s).map(|(a, b)| guid(a, b))).collect();
    let mut fields = vec![("guidPath", msg(vec![("guids", V::Array(steps?))]))];
    if let Some([x, y]) = e.size {
        fields.push(("size", vec2(x, y)));
    }
    if let Some([a, b, c, d, tx, ty]) = e.transform {
        fields.push(("transform", encode_matrix(&Affine { a, b, c, d, tx, ty })));
    }
    for (list, field) in [(&e.fill, "fillGeometry"), (&e.stroke, "strokeGeometry")] {
        if !list.is_empty() {
            fields.push((field, V::Array(outlines(cx, list))));
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
    let held = cx.doc.pending_instance_derived(inst_id)?;
    if held.main != main || cx.fingerprints.of_main(cx.doc, main) != held.fingerprint {
        return None;
    }
    let entries: Vec<V> = held.entries.iter().filter_map(|e| entry(cx, e)).collect();
    (!entries.is_empty()).then_some(entries)
}
