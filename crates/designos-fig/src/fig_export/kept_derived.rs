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
/// `None` to write none: the instance has children (its page is loaded), the loader holds none, or the main it is an
/// instance of (or one inside it) is not the main, with the content, the data was computed for, or the instance was
/// resized since the file was opened.
pub(super) fn pending_derived_symbol_data(cx: &mut ExportCtx<'_>, inst_id: NodeId, main: NodeId) -> Option<Vec<V>> {
    if !cx.doc.get(inst_id)?.children.is_empty() {
        return None;
    }
    // The sizes of what is inside the instance were computed for the size it had; a resized instance has other ones, and
    // an entry for the instance itself would put its old size back over the new one.
    // Whatever changed the size: Studio flags it, the agent tools write the node's size and flag nothing.
    if super::kept_overrides::size_edited(cx.doc, inst_id) {
        return None;
    }
    let held = cx.doc.pending_instance_derived(inst_id)?;
    // The fingerprint names the main it was taken from, so an instance pointed at another main does not match.
    let mains = crate::component_fingerprint::instance_mains(cx.doc, inst_id, main, &held.swap_mains);
    if cx.fingerprints.of_mains(cx.doc, &mains) != held.fingerprint {
        return None;
    }
    let entries: Vec<V> = held.entries.iter().filter_map(|e| entry(cx, e)).collect();
    (!entries.is_empty()).then_some(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_fingerprint::Fingerprints;
    use crate::testkit::figma_like_schema;
    use std::sync::{Arc, Mutex};

    /// Answers one fixed record for every instance, the way a loader with unloaded pages would.
    #[derive(Debug)]
    struct Fixed(PendingDerived);

    impl PageHydrator for Fixed {
        fn hydrate_page(&mut self, _doc: &mut Document, _page: NodeId) -> bool {
            false
        }
        fn hydrate_all(&mut self, _doc: &mut Document) -> bool {
            false
        }
        fn is_hydrated(&self, _page: NodeId) -> bool {
            false
        }
        fn pending_instance_derived(&self, _instance: NodeId) -> Option<PendingDerived> {
            Some(self.0.clone())
        }
    }

    /// A document with a main component and an instance of it whose loader holds one entry of derived data, taken from the
    /// main as it is now.
    fn document_with_pending_instance() -> (Document, NodeId, NodeId) {
        let mut doc = Document::new("derived");
        let page = doc.add_page("Page");
        let main_id = doc.alloc_id();
        let mut main = Node::new(main_id, NodeKind::Component(FrameProps::default()));
        main.source_id = Some("1:10".into());
        doc.insert(page, main).unwrap();
        let inst_id = doc.alloc_id();
        let inst = Node::new(inst_id, NodeKind::Instance { frame: FrameProps::default(), component: Some(main_id), component_key: None });
        doc.insert(page, inst).unwrap();
        let entry = PendingDerivedEntry { path: vec!["1:11".into()], size: Some([9.0, 8.0]), ..Default::default() };
        let held = PendingDerived { fingerprint: Fingerprints::default().of_mains(&doc, &[main_id]), entries: vec![entry], swap_mains: Vec::new() };
        doc.page_hydrator = Some(Arc::new(Mutex::new(Fixed(held))));
        (doc, main_id, inst_id)
    }

    fn write(doc: &Document, main: NodeId, inst: NodeId) -> Option<Vec<V>> {
        let schema = figma_like_schema();
        let mut cx = ExportCtx::new(doc, &schema);
        pending_derived_symbol_data(&mut cx, inst, main)
    }

    #[test]
    fn derived_data_is_written_while_the_main_is_as_the_data_was_computed_for() {
        let (doc, main, inst) = document_with_pending_instance();
        let entries = write(&doc, main, inst).expect("entries");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].get("size").is_some());
    }

    #[test]
    fn an_instance_that_has_children_is_read_from_them() {
        let (mut doc, main, inst) = document_with_pending_instance();
        let child_id = doc.alloc_id();
        doc.insert(inst, Node::rectangle(child_id, Rect::new(0.0, 0.0, 4.0, 4.0))).unwrap();
        assert!(write(&doc, main, inst).is_none());
    }
}
