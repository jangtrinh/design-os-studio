//! The two steps around a main sync that keep an imported instance's stored geometry, see
//! `crate::component_imported_layout`: [`before_sync`] notes it, [`after_sync`] writes it back or drops the record.

use crate::component_imported_layout::{canonical_solve_hash, trial};
use crate::component_imported_layout_fields::{layout_hash, noted_if_stored, same_layers, LayoutFields};
use crate::components::{instance_root_of, sync_one, Ctx};
use crate::{Document, ImportedLayout, NodeId, NodeKind};
use std::collections::{BTreeMap, BTreeSet};

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
        let before = noted_if_stored(doc, holder, record.stored);
        let solved_by_sync = inner.len() == 1 && target_set.contains(&holder) && !*in_a_main.get_or_insert_with(any_target_in_a_main);
        held.push(Held { holder, record, before, solved_by_sync, inner });
    }
    held
}

/// Number of main components around `id`.
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

/// The order [`after_sync`] decides in: every instance after the recorded instances its synced copies mirror (so
/// they are back on what they keep when it is checked), and otherwise instances inside more mains first.
fn decision_order(doc: &Document, mut held: Vec<Held>) -> Vec<Held> {
    held.sort_by_key(|h| (std::cmp::Reverse(mains_around(doc, h.holder)), h.holder.0));
    let index: BTreeMap<NodeId, usize> = held.iter().enumerate().map(|(k, h)| (h.holder, k)).collect();
    let deps: Vec<Vec<usize>> = held
        .iter()
        .enumerate()
        .map(|(k, h)| {
            h.inner
                .iter()
                .filter_map(|t| doc.get(*t).and_then(|n| n.component.source))
                .filter_map(|s| instance_root_of(doc, s))
                .filter_map(|r| index.get(&r).copied())
                .filter(|&d| d != k)
                .collect()
        })
        .collect();
    // Depth-first, dependencies first; a dependency met again while still open (a cycle) is left where it is.
    let mut state = vec![0u8; held.len()];
    let mut order = Vec::with_capacity(held.len());
    for start in 0..held.len() {
        let mut stack = vec![(start, 0usize)];
        while let Some((k, next)) = stack.pop() {
            if next == 0 {
                if state[k] != 0 {
                    continue;
                }
                state[k] = 1;
            }
            if let Some(&d) = deps[k].get(next) {
                stack.push((k, next + 1));
                if state[d] == 0 {
                    stack.push((d, 0));
                }
            } else {
                state[k] = 2;
                order.push(k);
            }
        }
    }
    let mut slots: Vec<Option<Held>> = held.into_iter().map(Some).collect();
    order.into_iter().filter_map(|k| slots[k].take()).collect()
}

/// After the sync: for each instance [`before_sync`] noted, writes its stored geometry back when the instance held it
/// before the sync, the sync left its layers, and the shape parameters of every baked outline, as they were, and one of
/// these shows Studio's layout of it is still the one Figma's geometry stands for:
///
/// - the sync laid it out itself ([`Held::solved_by_sync`]) and the result is the solve recorded at import;
/// - this sync's writes into it, made again against its references as they are kept, give its stored layout back;
/// - Studio's canonical solve of the whole instance is the one recorded at import.
///
/// Otherwise the record is dropped and the sync's own layout stands. When the sync wrote no layout field at all, the
/// instance already is the record-free result and keeps its record. An instance whose synced copy mirrors a recorded
/// instance that this sync dropped is dropped too: its copy took that instance's new layout.
pub(crate) fn after_sync(c: &mut Ctx, held: Vec<Held>) {
    if held.is_empty() {
        return;
    }
    let mut dropped = BTreeSet::new();
    for h in decision_order(c.s, held) {
        let Some(before) = h.before.as_ref() else {
            drop_record(c, &mut dropped, h.holder);
            continue;
        };
        if holds(c.s, h.holder, before) {
            continue;
        }
        let keep = same_layers(c.s, h.holder, before, |b, n| b.outline_meaning.as_ref().is_none_or(|m| m.held_by(n)))
            && if h.solved_by_sync {
                layout_hash(c.s, h.holder) == Some(h.record.solved)
            } else {
                !mirrors_dropped(c.s, &h.inner, &dropped)
                    && (resync_holds(c.s, h.holder, &h.inner, before) || canonical_solve_hash(c.s, h.holder) == Some(h.record.solved))
            };
        if !keep {
            drop_record(c, &mut dropped, h.holder);
            continue;
        }
        for b in before {
            if let Some(n) = c.s.get(b.id).filter(|n| !b.held_by(n, b.id == h.holder)) {
                let mut n = n.clone();
                b.write_to(&mut n, b.id == h.holder);
                c.update(n);
            }
        }
    }
}

/// Whether a synced copy in `inner` mirrors a layer of a recorded instance this sync dropped.
fn mirrors_dropped(doc: &Document, inner: &[NodeId], dropped: &BTreeSet<NodeId>) -> bool {
    !dropped.is_empty()
        && inner.iter().filter_map(|t| doc.get(*t).and_then(|n| n.component.source)).filter_map(|s| instance_root_of(doc, s)).any(|r| dropped.contains(&r))
}

/// Whether this sync's own writes into `holder` (the sync of each instance in `inner`, in the sync's order), made again
/// on the document as it is now, leave `holder` on the layout it held before the sync. "Now" is after the instances its
/// copies mirror were put back on what they keep, so this is the record-free sync of `holder` against its references as
/// they stand. The document is left as it was.
fn resync_holds(doc: &mut Document, holder: NodeId, inner: &[NodeId], before: &[LayoutFields]) -> bool {
    trial(doc, |c| {
        for &id in inner {
            sync_one(c, id);
        }
        holds(c.s, holder, before)
    })
}

/// Whether the layers of `holder` are the ones noted in `before`, in order, each still holding the layout fields noted.
fn holds(doc: &Document, holder: NodeId, before: &[LayoutFields]) -> bool {
    same_layers(doc, holder, before, |b, n| b.held_by(n, n.id == holder))
}

fn drop_record(c: &mut Ctx, dropped: &mut BTreeSet<NodeId>, holder: NodeId) {
    dropped.insert(holder);
    if let Some(mut n) = c.s.get(holder).cloned() {
        n.component.imported_layout = None;
        c.update(n);
    }
}
