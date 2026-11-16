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
use crate::components::{instance_component, instance_root_of, sync_one, Ctx};
use crate::{Document, ImportedLayout, NodeId, NodeKind};
use std::collections::{BTreeMap, BTreeSet};

/// Studio's canonical solve of instance `holder` (the sync a record-free instance gets), hashed like [`layout_hash`].
/// The document is left as it was, id counter included: the trial's inserts and updates are all undone.
fn canonical_solve_hash(doc: &mut Document, holder: NodeId) -> Option<u64> {
    let (next_id, gap_cursor, layout_epoch) = (doc.next_id, doc.gap_cursor, doc.layout_epoch);
    let mut c = Ctx::new(doc);
    sync_one(&mut c, holder);
    let hash = layout_hash(c.s, holder);
    drop(c.finish());
    doc.next_id = next_id;
    doc.gap_cursor = gap_cursor;
    doc.layout_epoch = layout_epoch;
    hash
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
    /// it, and no target lies inside a main it mirrors (so nothing it reads is written by the sync).
    solved_by_sync: bool,
}

/// The main components of the instances in the subtree under `root` (`root` included).
fn instance_mains_in(doc: &Document, root: NodeId) -> Vec<NodeId> {
    let mut mains = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        mains.extend(instance_component(n));
        stack.extend(n.children.iter().copied());
    }
    mains.into_iter().collect()
}

/// Whether a layer of `holder` mirrors, at any depth, a main in `written`.
fn mirrors_any(doc: &Document, holder: NodeId, written: &BTreeSet<NodeId>, inside: &mut BTreeMap<NodeId, Vec<NodeId>>) -> bool {
    let mut seen = BTreeSet::new();
    let mut queue = instance_mains_in(doc, holder);
    while let Some(m) = queue.pop() {
        if !seen.insert(m) {
            continue;
        }
        if written.contains(&m) {
            return true;
        }
        queue.extend(inside.entry(m).or_insert_with(|| instance_mains_in(doc, m)).iter().copied());
    }
    false
}

/// Notes, before a sync of `targets`, the geometry of every recorded instance the sync reaches. Cheap when no record
/// is reached: one walk up from each target.
pub(crate) fn before_sync(doc: &Document, targets: &[NodeId]) -> Vec<Held> {
    let mut targets_in: BTreeMap<NodeId, usize> = BTreeMap::new();
    for &t in targets {
        if let Some(holder) = instance_root_of(doc, t) {
            *targets_in.entry(holder).or_default() += 1;
        }
    }
    let target_set: BTreeSet<NodeId> = targets.iter().copied().collect();
    // The mains the sync writes into: every main around a target.
    let mut written: Option<BTreeSet<NodeId>> = None;
    let mut inside: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    let mut held = Vec::new();
    for (&holder, &count) in &targets_in {
        let Some(record) = doc.get(holder).and_then(|n| n.component.imported_layout) else { continue };
        let before = (layout_hash(doc, holder) == Some(record.stored))
            .then(|| subtree(doc, holder))
            .flatten()
            .and_then(|ids| ids.iter().map(|id| doc.get(*id).map(LayoutFields::of)).collect::<Option<Vec<_>>>());
        let solved_by_sync = before.is_some() && count == 1 && target_set.contains(&holder) && {
            let written = written.get_or_insert_with(|| targets.iter().flat_map(|&t| mains_holding(doc, t)).collect());
            written.is_empty() || !mirrors_any(doc, holder, written, &mut inside)
        };
        held.push(Held { holder, record, before, solved_by_sync });
    }
    held
}

/// Every main component around `id`.
fn mains_holding(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut cur = doc.get(id).and_then(|x| x.parent);
    while let Some(p) = cur {
        let Some(pn) = doc.get(p) else { break };
        if matches!(pn.kind, NodeKind::Component(_)) {
            out.push(p);
        }
        cur = pn.parent;
    }
    out
}

/// Number of main components around `id`: an instance inside a main is laid out before the instances of that main.
fn mains_around(doc: &Document, id: NodeId) -> usize {
    mains_holding(doc, id).len()
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
        if layout_hash(c.s, h.holder) == Some(h.record.stored) {
            continue;
        }
        let solved = if h.solved_by_sync { layout_hash(c.s, h.holder) } else { canonical_solve_hash(c.s, h.holder) };
        let keep = solved == Some(h.record.solved)
            && subtree(c.s, h.holder).is_some_and(|ids| {
                ids.len() == before.len()
                    && ids.iter().zip(before).all(|(id, b)| *id == b.id && b.outline_meaning.is_none_or(|m| c.s.get(*id).is_some_and(|n| meaning_hash(n) == m)))
            });
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

fn drop_record(c: &mut Ctx, holder: NodeId) {
    if let Some(mut n) = c.s.get(holder).cloned() {
        n.component.imported_layout = None;
        c.update(n);
    }
}
