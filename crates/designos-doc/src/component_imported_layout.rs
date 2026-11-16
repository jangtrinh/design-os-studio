//! Keeps the geometry an imported `.fig` stored for an instance across a main sync that does not change Studio's own
//! solve of that instance.
//!
//! Figma stores the laid-out sizes, positions and outlines of a resized instance's layers. Studio's sync lays the
//! instance out again with its own solver, which does not always land on Figma's numbers, so a sync with no change to
//! the main moved layers (#1919). The rule here:
//!
//! *An imported top-level instance keeps its stored geometry exactly as long as Studio's canonical solve of the whole
//! instance (the sync a record-free instance gets) gives the result it gave right after import.*
//!
//! The sync itself always runs unchanged. Around it, [`before_sync`] notes the geometry of each recorded instance the
//! sync reaches, and [`after_sync`] either writes that geometry back (only the layout fields, see
//! [`LayoutFields`]) or drops the record. The result can therefore differ from a record-free sync only in the layout
//! fields, and only while Studio's own solve of the instance is unchanged since import. Nothing about the solver's
//! inputs is enumerated: any input that moves the solve (stroke insets, variable values, modes, nested hug frames, text
//! measure) moves the hash.

use crate::branch_diff::StableHasher;
use crate::component_imported_layout_fields::{layout_hash, meaning_hash, subtree, LayoutFields};
use crate::components::{instance_root_of, sync_one, Ctx};
use crate::{Document, ImportedLayout, NodeId, NodeKind};
use std::collections::{BTreeMap, BTreeSet};

/// Studio's canonical solve of instance `holder` (the sync a record-free instance gets), hashed like [`layout_hash`].
/// The document is left as it was, id counter included: the trial's inserts and updates are all undone.
fn canonical_solve_hash(doc: &mut Document, holder: NodeId) -> Option<u64> {
    trial(doc, |c| {
        sync_one(c, holder);
        layout_hash(c.s, holder)
    })
}

/// Whether this sync's own writes into `holder` (the sync of each instance in `inner`, in the sync's order), made again
/// on the document as it is now, leave `holder` on the layout it held before the sync. "Now" is after the instances
/// inside the mains `holder` mirrors were put back on what they keep, so this is the record-free sync of `holder` against
/// its references as they stand. The document is left as it was.
fn resync_holds(doc: &mut Document, holder: NodeId, inner: &[NodeId], before: &[LayoutFields]) -> bool {
    trial(doc, |c| {
        for &id in inner {
            sync_one(c, id);
        }
        holds(c.s, holder, before)
    })
}

/// Runs `f` on the document and undoes everything it wrote, id counter included.
fn trial<R>(doc: &mut Document, f: impl FnOnce(&mut Ctx) -> R) -> R {
    let saved = (doc.next_id, doc.gap_cursor, doc.layout_epoch);
    let mut c = Ctx::new(doc);
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

/// A recorded instance a sync is about to reach.
pub(crate) struct Held {
    holder: NodeId,
    record: ImportedLayout,
    /// The layout of every layer as it was before the sync, when the instance still held its imported geometry.
    before: Option<Vec<LayoutFields>>,
    /// The sync runs Studio's canonical solve of this instance itself: it is a target, no other target lies inside
    /// it, and no target lies inside a main (so nothing it mirrors is written by the sync).
    solved_by_sync: bool,
    /// The instances inside it the sync syncs, in the sync's order.
    inner: Vec<NodeId>,
}

/// Notes, before a sync of `targets`, the geometry of every recorded instance the sync reaches. Cheap when no record
/// is reached: one walk up from each target.
pub(crate) fn before_sync(doc: &Document, targets: &[NodeId]) -> Vec<Held> {
    let mut inner_of: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for &t in targets {
        if let Some(holder) = instance_root_of(doc, t) {
            inner_of.entry(holder).or_default().push(t);
        }
    }
    let target_set: BTreeSet<NodeId> = targets.iter().copied().collect();
    let any_target_in_a_main = || targets.iter().any(|&t| crate::components::enclosing_component(doc, t).is_some());
    let mut in_a_main: Option<bool> = None;
    let mut held = Vec::new();
    for (holder, inner) in inner_of {
        let Some(record) = doc.get(holder).and_then(|n| n.component.imported_layout) else { continue };
        let before = (layout_hash(doc, holder) == Some(record.stored))
            .then(|| subtree(doc, holder))
            .flatten()
            .and_then(|ids| ids.iter().map(|id| doc.get(*id).map(LayoutFields::of)).collect::<Option<Vec<_>>>());
        let solved_by_sync = inner.len() == 1 && target_set.contains(&holder) && !*in_a_main.get_or_insert_with(any_target_in_a_main);
        held.push(Held { holder, record, before, solved_by_sync, inner });
    }
    held
}

/// Number of main components around `id`: an instance inside a main is laid out before the instances of that main.
fn mains_around(doc: &Document, id: NodeId) -> usize {
    let mut n = 0;
    let mut cur = doc.get(id).and_then(|x| x.parent);
    while let Some(p) = cur {
        let Some(pn) = doc.get(p) else { break };
        if matches!(pn.kind, NodeKind::Component(_)) {
            n += 1;
        }
        cur = pn.parent;
    }
    n
}

/// After the sync: for each instance [`before_sync`] noted, writes its stored geometry back when the instance held it
/// before the sync, Studio's canonical solve of it is the one recorded at import, and the sync left its layers, and the
/// shape parameters of every baked outline, as they were; otherwise drops the record, and the sync's own layout stands.
/// When the sync wrote no layout field at all, the instance already is the record-free result and keeps its record.
///
/// The solve is checked on the document as the sync left it, so the instance sees its references already synced, as
/// at import. Instances inside a main go first, so an instance of that main sees them back on their stored geometry.
pub(crate) fn after_sync(c: &mut Ctx, mut held: Vec<Held>) {
    if held.is_empty() {
        return;
    }
    held.sort_by_key(|h| (std::cmp::Reverse(mains_around(c.s, h.holder)), h.holder.0));
    for h in held {
        let Some(before) = h.before.as_ref() else {
            drop_record(c, h.holder);
            continue;
        };
        if holds(c.s, h.holder, before) {
            continue;
        }
        let same_layers = subtree(c.s, h.holder).is_some_and(|ids| {
            ids.len() == before.len()
                && ids.iter().zip(before).all(|(id, b)| *id == b.id && b.outline_meaning.is_none_or(|m| c.s.get(*id).is_some_and(|n| meaning_hash(n) == m)))
        });
        let keep = same_layers
            && if h.solved_by_sync {
                layout_hash(c.s, h.holder) == Some(h.record.solved)
            } else {
                resync_holds(c.s, h.holder, &h.inner, before) || canonical_solve_hash(c.s, h.holder) == Some(h.record.solved)
            };
        if !keep {
            drop_record(c, h.holder);
            continue;
        }
        for b in before {
            if let Some(mut n) = c.s.get(b.id).cloned() {
                b.write_to(&mut n, b.id == h.holder);
                c.update(n);
            }
        }
    }
}

/// Whether the layers of `holder` are the ones noted in `before`, in order, each still holding the layout fields noted.
fn holds(doc: &Document, holder: NodeId, before: &[LayoutFields]) -> bool {
    subtree(doc, holder)
        .is_some_and(|ids| ids.len() == before.len() && ids.iter().zip(before).all(|(id, b)| doc.get(*id).is_some_and(|n| b.held_by(n, *id == holder))))
}

fn drop_record(c: &mut Ctx, holder: NodeId) {
    if let Some(mut n) = c.s.get(holder).cloned() {
        n.component.imported_layout = None;
        c.update(n);
    }
}
