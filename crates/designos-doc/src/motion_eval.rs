//! Sample Motion at one time, without changing the document.
//!
//! [`evaluate_motion`] walks layers that have an enabled timeline assignment and returns one
//! override per track whose timeline can be timed. The document is only borrowed.
//!
//! Time. A self-assignment (`containing` equal to the timeline) places that timeline on the
//! document clock: raw time is `t` minus `offset_us`. [`PlaybackStyle`] then maps raw time into
//! the timeline: Once clamps to `[0, duration]`, Loop wraps with `t mod duration`, Boomerang
//! ping-pongs over a period of `2 * duration`. A timeline with `quantize_playback` then snaps
//! that local time with [`Timeline::snap`] (which uses [`FrameRate::frame_us`]). `snap_fps`
//! without `quantize_playback` is the editing grid and does not change playback.
//!
//! Nesting. The model has it. [`TimelineAssignment`] places a timeline inside `containing` at
//! `offset_us`, and [`Timeline::parent`] is the definition-level parent. Child raw time is the
//! parent's local time minus that start; the child's own playback and quantize then run. A parent
//! cycle, or a chain deeper than [`MAX_NEST`], does not play.
//!
//! Tracks. [`TrackLoop::Loop`] repeats the track's own keyframe span (first key to last key)
//! inside the timeline. Between keys the sample uses the destination keyframe's
//! [`Keyframe::progress`]: that easing is the curve of the segment that ends on it. Before the
//! first key the value holds; after the last key it holds. Springs go through that progress,
//! which calls [`crate::spring_at`].
//!
//! Operations. [`static_motion_value`] reads the layer's stored property. Set replaces it. Scale
//! multiplies it. Offset adds to it. A property that is not stored on the layer (rotation, scale,
//! shear, origin, path trim, text, or any other name) has no base, so Scale and Offset keep the
//! sampled track value.

use crate::{
    interpolate, Document, KeyValue, Keyframe, KeyframeOperation, MotionId, MotionIndex, Node, NodeId, PlaybackStyle, Timeline, Track, TrackLoop,
    TrackProperty, Vec2,
};

/// Parent chains longer than this are treated as a cycle so a hand-edited file cannot overflow the stack.
const MAX_NEST: usize = 64;

/// One animated property of one layer at the sampled time.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyOverride {
    pub node: NodeId,
    pub property: TrackProperty,
    pub timeline: MotionId,
    pub operation: KeyframeOperation,
    /// Track value at this time, before Set, Scale, or Offset.
    pub sampled: KeyValue,
    /// `sampled` combined with [`static_motion_value`].
    pub value: KeyValue,
}

/// Overrides for every playing track at `t_us` (microseconds on the document clock).
///
/// Nodes with no enabled assignment are skipped. The result owns its values; `doc` is not changed
/// and is not serialized.
pub fn evaluate_motion(doc: &Document, t_us: i64) -> Vec<PropertyOverride> {
    let index = MotionIndex::build(doc);
    let mut stack: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for raw_node in doc.nodes.values() {
        let Some(raw_motion) = raw_node.motion.as_deref() else { continue };
        if !raw_motion.assignments.iter().any(|a| !a.disabled) {
            continue;
        }
        let resolved = crate::resolve_node(doc, raw_node);
        let node = resolved.as_ref();
        let Some(motion) = node.motion.as_deref() else { continue };
        for mut track in motion.tracks.clone() {
            let node_has_dur_var = raw_node.variables.bindings.keys().any(|k| crate::is_duration_binding_key(k));
            let (tl_owner_id, tl_has_dur_var, raw_tl_dur_us) = if let Some((owner_id, _)) = index.timeline(&track.timeline) {
                let owner_has_dur = doc.get(owner_id).is_some_and(|on| on.variables.bindings.keys().any(|k| crate::is_duration_binding_key(k)));
                let raw_dur = doc
                    .get(owner_id)
                    .and_then(|on| on.motion.as_deref())
                    .and_then(|m| m.timelines.iter().find(|t| t.id == track.timeline))
                    .map(|t| t.duration_us)
                    .unwrap_or(0);
                (Some(owner_id), owner_has_dur, raw_dur)
            } else {
                (None, false, 0)
            };

            if !node_has_dur_var && tl_has_dur_var {
                if let Some((_, resolved_tl)) = index.timeline(&track.timeline) {
                    crate::retime_track_keyframes(&mut track, resolved_tl.duration_us, raw_tl_dur_us);
                }
            }

            let node_has_ease_var = raw_node.variables.bindings.keys().any(|k| crate::is_easing_binding_key(k));
            if !node_has_ease_var {
                if let Some(owner_id) = tl_owner_id {
                    if let Some(owner_node) = doc.get(owner_id) {
                        for (k, var_id) in &owner_node.variables.bindings {
                            if crate::is_easing_binding_key(k) {
                                if let Some(crate::VariableValue::String(s)) = doc.resolve_variable(owner_id, var_id) {
                                    if let Some((interp, key_easing)) = crate::parse_key_easing_preset(&s) {
                                        for kf in &mut track.keyframes {
                                            kf.interpolation = interp;
                                            kf.easing = Some(key_easing);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            stack.clear();
            let Some(local) = timeline_local(&index, node, track.timeline.as_str(), t_us, &mut stack) else { continue };
            let Some(sampled) = sample_track(&track, loop_time(&track, local)) else { continue };
            let base = static_motion_value(node, &track.property);
            let value = apply_operation(track.operation, base.as_ref(), sampled.clone());
            out.push(PropertyOverride {
                node: node.id,
                property: track.property.clone(),
                timeline: track.timeline.clone(),
                operation: track.operation,
                sampled,
                value,
            });
        }
    }
    out
}

/// The layer's stored value for `property`, the base Scale multiplies and Offset adds to.
///
/// Rotation, scale, shear, transform origin, path trim, text, and unknown names are not stored as
/// their own fields, so they return `None`. Corner radius is the shared radius when all four
/// corners match, and `None` when they differ. Stroke weight is the single weight, not per-side
/// weights. Translation is read from the layer transform.
pub fn static_motion_value(node: &Node, property: &TrackProperty) -> Option<KeyValue> {
    Some(match property {
        TrackProperty::Opacity => KeyValue::Float(node.opacity),
        TrackProperty::CornerRadius => {
            let [a, b, c, d] = node.corner_radii;
            if a == b && b == c && c == d {
                KeyValue::Float(a)
            } else {
                return None;
            }
        }
        TrackProperty::StrokeWeight => KeyValue::Float(node.stroke.weight),
        TrackProperty::Width => KeyValue::Float(node.size.x),
        TrackProperty::Height => KeyValue::Float(node.size.y),
        TrackProperty::TranslationX => KeyValue::Float(node.transform.tx),
        TrackProperty::TranslationY => KeyValue::Float(node.transform.ty),
        TrackProperty::TranslationXy => KeyValue::Vector(Vec2::new(node.transform.tx, node.transform.ty)),
        TrackProperty::Rotation
        | TrackProperty::ScaleX
        | TrackProperty::ScaleY
        | TrackProperty::ScaleXy
        | TrackProperty::Shear
        | TrackProperty::TransformOriginX
        | TrackProperty::TransformOriginY
        | TrackProperty::TransformOriginXy
        | TrackProperty::PathTrimStart
        | TrackProperty::PathTrimEnd
        | TrackProperty::TextData
        | TrackProperty::Other(_) => return None,
    })
}

fn timeline_local(index: &MotionIndex, node: &Node, timeline_id: &str, t_us: i64, stack: &mut Vec<String>) -> Option<i64> {
    if stack.len() >= MAX_NEST || stack.iter().any(|s| s == timeline_id) {
        return None;
    }
    if index.timeline(timeline_id).is_none() {
        return None;
    }
    stack.push(timeline_id.to_string());
    let raw = raw_clock(index, node, timeline_id, t_us, stack);
    stack.pop();
    let raw = raw?;
    let (_, timeline) = index.timeline(timeline_id)?;
    Some(map_playback(timeline, raw))
}

fn raw_clock(index: &MotionIndex, node: &Node, timeline_id: &str, t_us: i64, stack: &mut Vec<String>) -> Option<i64> {
    let motion = node.motion.as_deref()?;
    if let Some(a) = motion.assignments.iter().find(|a| !a.disabled && a.timeline == timeline_id) {
        let offset = a.offset_us;
        if a.containing == timeline_id {
            return Some(sat_sub(t_us, offset));
        }
        let parent_local = timeline_local(index, node, a.containing.as_str(), t_us, stack)?;
        return Some(sat_sub(parent_local, offset));
    }
    if motion.assignments.iter().any(|a| a.timeline == timeline_id) {
        return None;
    }
    if let Some(parent_id) = index.timeline(timeline_id).and_then(|(_, timeline)| timeline.parent.as_deref()) {
        let parent_local = timeline_local(index, node, parent_id, t_us, stack)?;
        return Some(sat_sub(parent_local, child_start(index, node, timeline_id, parent_id)));
    }
    if is_root_container(node, timeline_id) {
        return Some(t_us);
    }
    None
}

/// Start of `timeline` inside `parent`: this layer's assignment, else any layer's, else zero.
fn child_start(index: &MotionIndex, node: &Node, timeline_id: &str, parent_id: &str) -> i64 {
    let on_node = node
        .motion
        .as_deref()
        .and_then(|motion| motion.assignments.iter().find(|a| !a.disabled && a.timeline == timeline_id && a.containing == parent_id).map(|a| a.offset_us));
    if let Some(offset) = on_node {
        return offset;
    }
    index.assignments().iter().find(|(_, a)| !a.disabled && a.timeline == timeline_id && a.containing == parent_id).map(|(_, a)| a.offset_us).unwrap_or(0)
}

fn is_root_container(node: &Node, timeline_id: &str) -> bool {
    node.motion.as_deref().is_some_and(|motion| motion.assignments.iter().any(|a| !a.disabled && a.containing == timeline_id && a.timeline != timeline_id))
}

fn map_playback(timeline: &Timeline, raw: i64) -> i64 {
    let duration = u64_as_i64(timeline.duration_us);
    let local = match timeline.playback {
        PlaybackStyle::Once => raw.clamp(0, duration),
        PlaybackStyle::Loop => {
            if duration <= 0 {
                0
            } else {
                rem_euclid_pos(raw, duration)
            }
        }
        PlaybackStyle::Boomerang => boomerang(raw, duration),
    };
    if timeline.quantize_playback {
        timeline.snap(local)
    } else {
        local
    }
}

fn boomerang(raw: i64, duration: i64) -> i64 {
    if duration <= 0 {
        return 0;
    }
    let period = i128::from(duration) * 2;
    let mut wrapped = i128::from(raw) % period;
    if wrapped < 0 {
        wrapped += period;
    }
    let local = if wrapped <= i128::from(duration) { wrapped } else { period - wrapped };
    i64::try_from(local).unwrap_or(0)
}

fn loop_time(track: &Track, timeline_us: i64) -> i64 {
    if track.loop_mode != TrackLoop::Loop {
        return timeline_us;
    }
    let (Some(first), Some(last)) = (track.keyframes.first(), track.keyframes.last()) else {
        return timeline_us;
    };
    let span = last.time_us.saturating_sub(first.time_us);
    if span <= 0 || timeline_us <= first.time_us {
        return timeline_us;
    }
    let delta = rem_euclid_pos(sat_sub(timeline_us, first.time_us), span);
    // An exact cycle boundary is the end key, so the last value is reached. The next microsecond
    // steps back into the span.
    if delta == 0 {
        last.time_us
    } else {
        first.time_us.saturating_add(delta)
    }
}

fn sample_track(track: &Track, t_us: i64) -> Option<KeyValue> {
    let keys = &track.keyframes;
    let last = keys.last()?;
    let first = &keys[0];
    if t_us < first.time_us {
        return Some(first.value.clone());
    }
    if t_us >= last.time_us {
        return Some(last.value.clone());
    }
    let left_at = keys.partition_point(|key| key.time_us <= t_us) - 1;
    let left = &keys[left_at];
    let right = &keys[left_at + 1];
    let eased = right.progress(segment_u(left, right, t_us));
    Some(interpolate(&left.value, &right.value, eased, right.color_space))
}

fn segment_u(from: &Keyframe, to: &Keyframe, t_us: i64) -> f32 {
    crate::segment_progress(from, to, t_us)
}

fn apply_operation(op: KeyframeOperation, base: Option<&KeyValue>, sampled: KeyValue) -> KeyValue {
    if op == KeyframeOperation::Set {
        return sampled;
    }
    match base.and_then(|base| combine(op, base, &sampled)) {
        Some(value) => value,
        None => sampled,
    }
}

fn mix(op: KeyframeOperation, base: f32, sample: f32) -> f32 {
    match op {
        KeyframeOperation::Scale => base * sample,
        KeyframeOperation::Offset => base + sample,
        KeyframeOperation::Set => sample,
    }
}

fn mix2(op: KeyframeOperation, base: Vec2, sample: Vec2) -> Vec2 {
    Vec2::new(mix(op, base.x, sample.x), mix(op, base.y, sample.y))
}

fn combine(op: KeyframeOperation, base: &KeyValue, sample: &KeyValue) -> Option<KeyValue> {
    use KeyValue::*;
    Some(match (base, sample) {
        (Float(b), Float(v)) => Float(mix(op, *b, *v)),
        (Vector(b), Vector(v)) => Vector(mix2(op, *b, *v)),
        (Color(b), Color(v)) => Color(crate::Color { r: mix(op, b.r, v.r), g: mix(op, b.g, v.g), b: mix(op, b.b, v.b), a: mix(op, b.a, v.a) }),
        (Circle { center: c0, radius: r0 }, Circle { center: c1, radius: r1 }) => Circle { center: mix2(op, *c0, *c1), radius: mix(op, *r0, *r1) },
        (Line { a: a0, b: b0 }, Line { a: a1, b: b1 }) => Line { a: mix2(op, *a0, *a1), b: mix2(op, *b0, *b1) },
        (CirclePoint { center: c0, radius: r0, angle: g0 }, CirclePoint { center: c1, radius: r1, angle: g1 }) => {
            CirclePoint { center: mix2(op, *c0, *c1), radius: mix(op, *r0, *r1), angle: mix(op, *g0, *g1) }
        }
        (Rotation3D { x: x0, y: y0, z: z0, translate_z: t0 }, Rotation3D { x: x1, y: y1, z: z1, translate_z: t1 }) => {
            Rotation3D { x: mix(op, *x0, *x1), y: mix(op, *y0, *y1), z: mix(op, *z0, *z1), translate_z: mix(op, *t0, *t1) }
        }
        _ => return None,
    })
}

fn sat_sub(a: i64, b: i64) -> i64 {
    i64::try_from(i128::from(a) - i128::from(b)).unwrap_or(if a >= b { i64::MAX } else { i64::MIN })
}

fn rem_euclid_pos(value: i64, divisor: i64) -> i64 {
    let divisor = i128::from(divisor);
    let mut remainder = i128::from(value) % divisor;
    if remainder < 0 {
        remainder += divisor;
    }
    i64::try_from(remainder).unwrap_or(0)
}

fn u64_as_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
