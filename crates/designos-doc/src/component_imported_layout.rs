//! Keeps the geometry an imported `.fig` stored for an instance across a main sync that does not change Studio's own
//! layout of that instance.
//!
//! Figma stores the laid-out sizes, positions and outlines of a resized instance's layers. Studio's sync lays the
//! instance out again with its own solver, which does not always land on Figma's numbers, so a sync with no change to
//! the main moved layers (#1919). The rule here:
//!
//! *A sync puts an imported top-level instance back on the geometry it held before the sync only while Studio's own
//! layout of it is still the one Figma's geometry stands for:* Studio's canonical solve of the whole instance (the sync
//! a record-free instance gets) gives the result it gave right after import, or this sync's own writes into the
//! instance, made again against its references as they are kept, give its stored layout back.
//!
//! The sync itself always runs unchanged. Around it, `component_imported_layout_sync` notes the geometry of each
//! recorded instance the sync reaches and then either writes that geometry back (only the layout fields, see
//! `LayoutFields`) or drops the record. The result can therefore differ from a record-free sync only in the layout
//! fields. Nothing about the solver's inputs is enumerated: both checks run the sync's own code, so any input that moves
//! the layout (stroke insets, variable values, modes, nested hug frames, text measure) shows in their result.

use crate::branch_diff::StableHasher;
use crate::component_imported_layout_fields::layout_hash;
use crate::components::{instance_root_of, sync_one, Ctx};
use crate::{Document, ImportedLayout, NodeId, NodeKind};

/// Studio's canonical solve of instance `holder` (the sync a record-free instance gets), hashed like [`layout_hash`].
/// The document is left as it was, id counter included: the trial's inserts and updates are all undone.
pub(crate) fn canonical_solve_hash(doc: &mut Document, holder: NodeId) -> Option<u64> {
    trial(doc, |c| {
        sync_one(c, holder);
        layout_hash(c.s, holder)
    })
}

/// Runs `f` on the document and undoes everything it wrote, id counter included.
pub(crate) fn trial<R>(doc: &mut Document, f: impl FnOnce(&mut Ctx) -> R) -> R {
    let saved = (doc.next_id, doc.gap_cursor, doc.layout_epoch);
    let mut c = Ctx::scratch(doc);
    let out = f(&mut c);
    drop(c.finish());
    (doc.next_id, doc.gap_cursor, doc.layout_epoch) = saved;
    out
}

/// Whether `id` is an instance root that no other instance holds.
fn is_holder(doc: &Document, id: NodeId) -> bool {
    doc.get(id).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. })) && instance_root_of(doc, id) == Some(id)
}

/// Records, on each top-level instance in `holders`, the two hashes a later main sync needs to keep the geometry the
/// instance holds now. Called by an importer right after it wrote a file's stored geometry into these instances and
/// settled everything a solve reads (variables, styles, style metadata). An instance whose stored geometry Studio's
/// solve of the whole instance already reproduces is recorded too: a sync that reaches only a nested instance of it
/// solves that part on its own and can still move it.
pub fn record_imported_layouts(doc: &mut Document, holders: impl IntoIterator<Item = NodeId>) {
    let mut ids: Vec<NodeId> = holders.into_iter().collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let record = if is_holder(doc, id) {
            match (layout_hash(doc, id), canonical_solve_hash(doc, id)) {
                (Some(stored), Some(solved)) => Some(ImportedLayout { stored, solved }),
                _ => None,
            }
        } else {
            None
        };
        if let Some(n) = doc.get_mut(id) {
            n.component.imported_layout = record;
        }
    }
}

/// Records the instances an importer requested ([`ImportedLayout::REQUESTED`]), see [`record_imported_layouts`].
pub fn record_requested_imported_layouts(doc: &mut Document) {
    let requested: Vec<NodeId> = doc.nodes.values().filter(|n| n.component.imported_layout.is_some_and(|r| r.is_requested())).map(|n| n.id).collect();
    if !requested.is_empty() {
        record_imported_layouts(doc, requested);
    }
}

/// Hash of the document-wide data a solve reads besides the layers: variables, their collections, shared styles and
/// the variables those styles bind (their names, descriptions and sort order are left out). An importer that records
/// geometry later (when a deferred page opens) records only while this is what it was at import, since a stored
/// geometry is only true for the values it was laid out with.
pub fn layout_inputs_outside_layers_hash(doc: &Document) -> u64 {
    let mut h = StableHasher::new();
    h.add(&doc.variables);
    h.add(&doc.variable_collections);
    h.add(&doc.styles);
    for (key, meta) in &doc.style_meta {
        h.add(key);
        h.add(&meta.bindings);
    }
    h.finish()
}
