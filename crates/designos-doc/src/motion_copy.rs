//! Giving copied layers their own timeline and track ids.
//!
//! Timeline and track ids are unique in a document, but a copy of a layer (duplicate, copy and paste, a copy
//! made into another file) carries the same `motion` as the original. [`reid_copied_motion`] runs on the fresh
//! nodes of a copy before they are inserted: it renames every timeline and track the copied set defines and
//! rewrites each reference to them, so [`MotionIndex::duplicate_ids`] stays empty afterwards.

use crate::{Document, MotionId, MotionIndex, Node, NodeMotion};
use std::collections::{BTreeMap, BTreeSet};

/// A free id derived from `old` (`old~1`, `old~2`, ...), recorded in `taken`. Copies of copies keep the base
/// name instead of growing a suffix chain.
fn fresh(old: &str, taken: &mut BTreeSet<MotionId>) -> MotionId {
    let base = old.split('~').next().unwrap_or(old);
    let id = (1u32..).map(|n| format!("{base}~{n}")).find(|c| !taken.contains(c)).expect("a free suffix exists");
    taken.insert(id.clone());
    id
}

/// Renames every timeline and track defined by `nodes`, the fresh copies of layers about to be inserted into
/// `doc`, and rewrites the references to the renamed timelines (a track's timeline, an assignment's two
/// timelines, a timeline's parent).
///
/// A reference to a timeline the copied set does not define is kept when the copy goes into the same document
/// (`same_document`) and that document has the timeline (a copy of a layer that plays a timeline of its parent
/// still plays it). Into another document it is always dropped, whatever that document owns: ids are never matched
/// across documents, so a colliding id cannot make the copy play a foreign timeline. A dropped reference removes
/// the track or assignment and clears a parent link. A layer left with no Motion
/// loses the field.
pub fn reid_copied_motion(doc: &Document, nodes: &mut [&mut Node], same_document: bool) {
    reid_copied_motion_with(doc, nodes, same_document, &mut BTreeSet::new());
}

/// [`reid_copied_motion`] for one of several copies made before any of them is inserted. `reserved` holds the ids
/// the earlier copies of the batch already use, which the document cannot show yet; the new ids chosen here are
/// added to it, so no two copies of one paste or duplicate share a timeline or track id.
pub fn reid_copied_motion_with(doc: &Document, nodes: &mut [&mut Node], same_document: bool, reserved: &mut BTreeSet<MotionId>) {
    if nodes.iter().all(|n| n.motion.is_none()) {
        return;
    }
    let ix = MotionIndex::build(doc);
    let mut taken: BTreeSet<MotionId> = ix.all_ids();
    taken.extend(reserved.iter().cloned());
    let defined: Vec<MotionId> = nodes
        .iter()
        .filter_map(|n| n.motion.as_deref())
        .flat_map(|m| m.timelines.iter().map(|t| t.id.clone()).chain(m.tracks.iter().map(|t| t.id.clone())))
        .collect();
    taken.extend(defined);

    let mut timelines: BTreeMap<MotionId, MotionId> = BTreeMap::new();
    for n in nodes.iter() {
        for t in n.motion.as_deref().map(|m| m.timelines.as_slice()).unwrap_or(&[]) {
            timelines.entry(t.id.clone()).or_insert_with(|| fresh(&t.id, &mut taken));
        }
    }
    // `Some(new)` for a renamed timeline, the id itself for one the document has, `None` for a dangling one.
    let resolve =
        |id: &str| -> Option<MotionId> { timelines.get(id).cloned().or_else(|| (same_document && ix.timeline(id).is_some()).then(|| id.to_string())) };

    for n in nodes.iter_mut() {
        let Some(m) = n.motion.take() else { continue };
        let NodeMotion { timelines: tl, assignments, tracks } = *m;
        let mut out = NodeMotion::default();
        for mut t in tl {
            t.id = timelines[&t.id].clone();
            t.parent = t.parent.as_deref().and_then(resolve);
            out.timelines.push(t);
        }
        for mut a in assignments {
            if let (Some(timeline), Some(containing)) = (resolve(&a.timeline), resolve(&a.containing)) {
                a.timeline = timeline;
                a.containing = containing;
                out.assignments.push(a);
            }
        }
        for mut t in tracks {
            if let Some(timeline) = resolve(&t.timeline) {
                t.timeline = timeline;
                t.id = fresh(&t.id, &mut taken);
                out.tracks.push(t);
            }
        }
        n.motion = (!out.is_empty()).then(|| Box::new(out));
    }
    reserved.extend(taken);
}
