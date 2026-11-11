//! Motion edit operations: one pure function per edit, one undo step each.
//!
//! Every operation reads the document and returns the [`Command`] that makes the edit. The command is a
//! `Command::Update` of the owning layer (or a `Command::Batch` of them when an edit reaches several layers, such
//! as removing a timeline that tracks on other layers use). The caller runs it through `History::exec_user` or
//! `exec_agent`, so undo and redo come from the history and nothing here mutates the document.
//!
//! [`MotionIndex`] is the read side: one pass over the layers that carry Motion, then lookups by timeline id.

use crate::{Command, Document, Keyframe, MotionId, NodeId, NodeMotion, Timeline, TimelineAssignment, Track};
use std::collections::{BTreeMap, BTreeSet};

/// Why a Motion edit was refused. Each message names the next action.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MotionError {
    #[error("layer {0:?} not found; pick an existing layer")]
    NodeNotFound(NodeId),
    #[error("{kind} id \"{id}\" is empty or already used; pick a unique id")]
    BadId { kind: &'static str, id: String },
    #[error("timeline \"{0}\" does not exist; create it first")]
    NoTimeline(MotionId),
    #[error("track \"{0}\" does not exist on this layer")]
    NoTrack(MotionId),
    #[error("no keyframe at {0} us on this track")]
    NoKeyframe(i64),
    #[error("a keyframe already sits at {0} us; remove it or pick another time")]
    KeyframeTaken(i64),
    #[error("timeline \"{0}\" would contain itself; pick another parent")]
    Cycle(MotionId),
    #[error("the edit may not change the id of \"{0}\"")]
    IdChanged(MotionId),
    #[error("timeline \"{timeline}\" is already assigned inside \"{containing}\" on this layer")]
    AlreadyAssigned { timeline: MotionId, containing: MotionId },
    #[error("nothing changes; the edit leaves the Motion data as it is")]
    NoChange,
    #[error("a track needs a property name; pick the property to animate")]
    EmptyProperty,
    #[error("no assignment of \"{timeline}\" inside \"{containing}\" on this layer")]
    NoAssignment { timeline: MotionId, containing: MotionId },
}

/// Every timeline, track and assignment of a document, found in one pass over the layers that carry Motion.
#[derive(Debug, Default)]
pub struct MotionIndex<'a> {
    timelines: BTreeMap<&'a str, (NodeId, Timeline)>,
    tracks: BTreeMap<&'a str, Vec<(NodeId, Track)>>,
    track_ids: BTreeSet<&'a str>,
    assignments: Vec<(NodeId, TimelineAssignment)>,
    duplicate_ids: Vec<&'a str>,
}

impl<'a> MotionIndex<'a> {
    /// Indexes `doc`. Timeline and track ids must be unique in the document, but a duplicated layer, a paste or
    /// a merge copies `motion` with the same ids: the first timeline (in layer id order) keeps the id, and every
    /// repeat is listed in [`Self::duplicate_ids`] instead of silently replacing it.
    pub fn build(doc: &'a Document) -> Self {
        let mut ix = Self::default();
        for raw_node in doc.nodes.values() {
            let Some(raw_motion) = raw_node.motion.as_deref() else { continue };
            let resolved = crate::resolve_node(doc, raw_node);
            let m = resolved.motion.as_deref().unwrap_or(raw_motion);
            for (idx, raw_t) in raw_motion.timelines.iter().enumerate() {
                let id = raw_t.id.as_str();
                let t = m.timelines.get(idx).unwrap_or(raw_t);
                if ix.timelines.contains_key(id) {
                    ix.duplicate_ids.push(id);
                } else {
                    ix.timelines.insert(id, (raw_node.id, t.clone()));
                }
            }
            for (idx, raw_t) in raw_motion.tracks.iter().enumerate() {
                let id = raw_t.id.as_str();
                let t = m.tracks.get(idx).unwrap_or(raw_t);
                if !ix.track_ids.insert(id) {
                    ix.duplicate_ids.push(id);
                }
                ix.tracks.entry(raw_t.timeline.as_str()).or_default().push((raw_node.id, t.clone()));
            }
            for a in &m.assignments {
                ix.assignments.push((raw_node.id, a.clone()));
            }
        }
        ix
    }

    /// Every timeline and track id in the document.
    pub(crate) fn all_ids(&self) -> BTreeSet<MotionId> {
        self.timelines.keys().chain(self.track_ids.iter()).map(|s| s.to_string()).collect()
    }

    /// Timeline and track ids that appear more than once in the document. Empty for a healthy document; a
    /// layer duplicate must give its timelines and tracks new ids to keep it so.
    pub fn duplicate_ids(&self) -> &[&'a str] {
        &self.duplicate_ids
    }

    /// The layer that owns timeline `id`, and the timeline.
    pub fn timeline(&self, id: &str) -> Option<(NodeId, &Timeline)> {
        self.timelines.get(id).map(|(nid, t)| (*nid, t))
    }

    /// All timelines, in id order.
    pub fn timelines(&self) -> impl Iterator<Item = (NodeId, &Timeline)> + '_ {
        self.timelines.values().map(|(nid, t)| (*nid, t))
    }

    /// The tracks that run on `timeline`, with the layer each animates.
    pub fn tracks_on(&self, timeline: &str) -> &[(NodeId, Track)] {
        self.tracks.get(timeline).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every assignment in the document with the layer it plays on.
    pub fn assignments(&self) -> &[(NodeId, TimelineAssignment)] {
        &self.assignments
    }

    /// The assignments nested inside `containing`.
    pub fn assignments_in<'s>(&'s self, containing: &'s str) -> impl Iterator<Item = (NodeId, &'s TimelineAssignment)> + 's {
        self.assignments.iter().map(|(nid, a)| (*nid, a)).filter(move |(_, a)| a.containing == containing)
    }

    /// The timelines `id` is nested in through its `parent` chain, nearest first. Stops at a missing parent or
    /// a repeat, so a hand-edited file with a loop cannot hang the caller.
    pub fn ancestors(&self, id: &str) -> Vec<&Timeline> {
        let mut out: Vec<&Timeline> = Vec::new();
        let mut cur = self.timeline(id).and_then(|(_, t)| t.parent.as_deref());
        while let Some(pid) = cur {
            match self.timeline(pid) {
                Some((_, p)) if p.id != id && !out.iter().any(|seen| seen.id == p.id) => {
                    out.push(p);
                    cur = p.parent.as_deref();
                }
                _ => break,
            }
        }
        out
    }

    fn assignment_reaches(&self, from: &str, target: &str) -> bool {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut stack = vec![from];
        while let Some(cur) = stack.pop() {
            if cur == target {
                return true;
            }
            if seen.insert(cur) {
                stack.extend(self.assignments_in(cur).map(|(_, a)| a.timeline.as_str()));
            }
        }
        false
    }
}

/// Applies `f` to a copy of the Motion data of `node` and returns the `Update` that stores the result. A result
/// equal to the current data is refused with [`MotionError::NoChange`], so no undo step is made and redo stays. An empty
/// result clears the field, so a layer whose last timeline, track and assignment went away serializes as before.
pub fn edit_motion(doc: &Document, node: NodeId, f: impl FnOnce(&mut NodeMotion) -> Result<(), MotionError>) -> Result<Command, MotionError> {
    let mut n = doc.get(node).ok_or(MotionError::NodeNotFound(node))?.clone();
    let before = n.motion.take();
    let mut motion = before.as_deref().cloned().unwrap_or_default();
    // keyframe lookups assume time order; programmatic data may not have it
    for t in &mut motion.tracks {
        t.normalize();
    }
    f(&mut motion)?;
    for t in &mut motion.tracks {
        t.normalize();
    }
    let after = (!motion.is_empty()).then(|| Box::new(motion));
    if after == before {
        return Err(MotionError::NoChange);
    }
    n.motion = after;
    Ok(Command::Update { node: Box::new(n) })
}

fn check_new_id(kind: &'static str, id: &str, taken: bool) -> Result<(), MotionError> {
    if id.is_empty() || taken {
        Err(MotionError::BadId { kind, id: id.to_string() })
    } else {
        Ok(())
    }
}

fn check_parent(ix: &MotionIndex, id: &str, parent: &Option<MotionId>) -> Result<(), MotionError> {
    let Some(pid) = parent else { return Ok(()) };
    if pid == id {
        return Err(MotionError::Cycle(id.to_string()));
    }
    let (_, p) = ix.timeline(pid).ok_or_else(|| MotionError::NoTimeline(pid.clone()))?;
    if ix.ancestors(&p.id).iter().any(|a| a.id == id) {
        return Err(MotionError::Cycle(id.to_string()));
    }
    Ok(())
}

/// Adds `timeline` to the timelines owned by `owner`.
pub fn add_timeline(doc: &Document, owner: NodeId, timeline: Timeline) -> Result<Command, MotionError> {
    let ix = MotionIndex::build(doc);
    check_new_id("timeline", &timeline.id, ix.timeline(&timeline.id).is_some())?;
    check_parent(&ix, &timeline.id, &timeline.parent)?;
    edit_motion(doc, owner, |m| {
        m.timelines.push(timeline);
        Ok(())
    })
}

/// Edits timeline `id` in place with `f`. The id cannot change; a new `parent` is checked.
pub fn update_timeline(doc: &Document, id: &str, f: impl FnOnce(&mut Timeline)) -> Result<Command, MotionError> {
    let ix = MotionIndex::build(doc);
    let (owner, current) = ix.timeline(id).ok_or_else(|| MotionError::NoTimeline(id.to_string()))?;
    let mut next = current.clone();
    f(&mut next);
    if next.id != id {
        return Err(MotionError::IdChanged(id.to_string()));
    }
    check_parent(&ix, id, &next.parent)?;
    edit_motion(doc, owner, |m| {
        if let Some(t) = m.timelines.iter_mut().find(|t| t.id == id) {
            *t = next;
        }
        Ok(())
    })
}

/// Removes timeline `id` together with what depends on it: the tracks bound to it and the assignments that name
/// it, on every layer, and the `parent` link of timelines nested in it. One undo step.
pub fn remove_timeline(doc: &Document, id: &str) -> Result<Command, MotionError> {
    let ix = MotionIndex::build(doc);
    ix.timeline(id).ok_or_else(|| MotionError::NoTimeline(id.to_string()))?;
    let touched: BTreeSet<NodeId> = doc
        .nodes
        .values()
        .filter(|n| {
            n.motion.as_deref().is_some_and(|m| {
                m.timelines.iter().any(|t| t.id == id || t.parent.as_deref() == Some(id))
                    || m.tracks.iter().any(|t| t.timeline == id)
                    || m.assignments.iter().any(|a| a.timeline == id || a.containing == id)
            })
        })
        .map(|n| n.id)
        .collect();
    let mut cmds = Vec::with_capacity(touched.len());
    for node in touched {
        cmds.push(edit_motion(doc, node, |m| {
            m.timelines.retain(|t| t.id != id);
            for t in m.timelines.iter_mut().filter(|t| t.parent.as_deref() == Some(id)) {
                t.parent = None;
            }
            m.tracks.retain(|t| t.timeline != id);
            m.assignments.retain(|a| a.timeline != id && a.containing != id);
            Ok(())
        })?);
    }
    Ok(if cmds.len() == 1 { cmds.remove(0) } else { Command::Batch(cmds) })
}

/// Plays `assignment.timeline` on `node` inside `assignment.containing`.
pub fn assign_timeline(doc: &Document, node: NodeId, assignment: TimelineAssignment) -> Result<Command, MotionError> {
    let ix = MotionIndex::build(doc);
    for id in [&assignment.timeline, &assignment.containing] {
        ix.timeline(id).ok_or_else(|| MotionError::NoTimeline(id.clone()))?;
    }
    if ix.assignment_reaches(&assignment.timeline, &assignment.containing) {
        return Err(MotionError::Cycle(assignment.timeline));
    }
    edit_motion(doc, node, |m| {
        if m.assignments.iter().any(|a| a.timeline == assignment.timeline && a.containing == assignment.containing) {
            return Err(MotionError::AlreadyAssigned { timeline: assignment.timeline.clone(), containing: assignment.containing.clone() });
        }
        m.assignments.push(assignment);
        Ok(())
    })
}

/// Edits the assignment of `timeline` inside `containing` on `node` with `f`; its key cannot change.
pub fn update_assignment(
    doc: &Document,
    node: NodeId,
    timeline: &str,
    containing: &str,
    f: impl FnOnce(&mut TimelineAssignment),
) -> Result<Command, MotionError> {
    edit_motion(doc, node, |m| {
        let a = m
            .assignments
            .iter_mut()
            .find(|a| a.timeline == timeline && a.containing == containing)
            .ok_or_else(|| MotionError::NoAssignment { timeline: timeline.to_string(), containing: containing.to_string() })?;
        f(a);
        if a.timeline != timeline || a.containing != containing {
            return Err(MotionError::IdChanged(timeline.to_string()));
        }
        Ok(())
    })
}

/// Stops playing `timeline` inside `containing` on `node`.
pub fn unassign_timeline(doc: &Document, node: NodeId, timeline: &str, containing: &str) -> Result<Command, MotionError> {
    edit_motion(doc, node, |m| {
        let before = m.assignments.len();
        m.assignments.retain(|a| !(a.timeline == timeline && a.containing == containing));
        if m.assignments.len() == before {
            return Err(MotionError::NoAssignment { timeline: timeline.to_string(), containing: containing.to_string() });
        }
        Ok(())
    })
}

/// Adds `track` to `node`. Its timeline must exist and its id must be new in the document.
pub fn add_track(doc: &Document, node: NodeId, track: Track) -> Result<Command, MotionError> {
    let ix = MotionIndex::build(doc);
    check_new_id("track", &track.id, ix.track_ids.contains(track.id.as_str()))?;
    if track.property.kiwi_name().is_empty() {
        return Err(MotionError::EmptyProperty);
    }
    ix.timeline(&track.timeline).ok_or_else(|| MotionError::NoTimeline(track.timeline.clone()))?;
    edit_motion(doc, node, |m| {
        m.tracks.push(track);
        Ok(())
    })
}

/// Edits track `id` of `node` with `f`. The id cannot change.
pub fn update_track(doc: &Document, node: NodeId, id: &str, f: impl FnOnce(&mut Track)) -> Result<Command, MotionError> {
    if let Some(m) = doc.get(node).and_then(|n| n.motion.as_deref()) {
        if let Some(next) = m.track(id).map(|t| {
            let mut next = t.clone();
            f(&mut next);
            next
        }) {
            if next.id != id {
                return Err(MotionError::IdChanged(id.to_string()));
            }
            if next.property.kiwi_name().is_empty() {
                return Err(MotionError::EmptyProperty);
            }
            if next.timeline != m.track(id).map(|t| t.timeline.clone()).unwrap_or_default() {
                MotionIndex::build(doc).timeline(&next.timeline).ok_or_else(|| MotionError::NoTimeline(next.timeline.clone()))?;
            }
            return edit_motion(doc, node, |m| {
                if let Some(t) = m.track_mut(id) {
                    *t = next;
                }
                Ok(())
            });
        }
    }
    edit_motion(doc, node, |_| Err(MotionError::NoTrack(id.to_string())))
}

/// Removes track `id` from `node`.
pub fn remove_track(doc: &Document, node: NodeId, id: &str) -> Result<Command, MotionError> {
    edit_motion(doc, node, |m| {
        let before = m.tracks.len();
        m.tracks.retain(|t| t.id != id);
        if m.tracks.len() == before {
            return Err(MotionError::NoTrack(id.to_string()));
        }
        Ok(())
    })
}

fn with_track<'m>(m: &'m mut NodeMotion, id: &str) -> Result<&'m mut Track, MotionError> {
    m.track_mut(id).ok_or_else(|| MotionError::NoTrack(id.to_string()))
}

/// Adds `key` to track `track` of `node`, or replaces the keyframe at the same time.
pub fn set_keyframe(doc: &Document, node: NodeId, track: &str, key: Keyframe) -> Result<Command, MotionError> {
    edit_motion(doc, node, |m| {
        with_track(m, track)?.set_keyframe(key);
        Ok(())
    })
}

/// Removes the keyframe at `time_us` from track `track` of `node`.
pub fn remove_keyframe(doc: &Document, node: NodeId, track: &str, time_us: i64) -> Result<Command, MotionError> {
    edit_motion(doc, node, |m| with_track(m, track)?.remove_keyframe(time_us).map(|_| ()).ok_or(MotionError::NoKeyframe(time_us)))
}

/// Moves the keyframe at `from_us` to `to_us` on track `track` of `node`. Another keyframe at `to_us` refuses
/// the move. The target time snaps to the frame grid of the track's timeline when it has one.
pub fn move_keyframe(doc: &Document, node: NodeId, track: &str, from_us: i64, to_us: i64) -> Result<Command, MotionError> {
    let snapped = {
        let tl = doc.get(node).and_then(|n| n.motion.as_deref()).and_then(|m| m.track(track)).map(|t| t.timeline.clone());
        match tl.and_then(|id| MotionIndex::build(doc).timeline(&id).map(|(_, t)| t.snap(to_us))) {
            Some(t) => t,
            None => to_us,
        }
    };
    edit_motion(doc, node, |m| {
        let t = with_track(m, track)?;
        if snapped == from_us {
            // the keyframe stays where it is: no move, and no remove-then-set that would lose one of several at that time
            return Err(if t.keyframe(from_us).is_some() { MotionError::NoChange } else { MotionError::NoKeyframe(from_us) });
        }
        if t.keyframe(snapped).is_some() {
            return Err(MotionError::KeyframeTaken(snapped));
        }
        let mut key = t.remove_keyframe(from_us).ok_or(MotionError::NoKeyframe(from_us))?;
        key.time_us = snapped;
        t.set_keyframe(key);
        Ok(())
    })
}
