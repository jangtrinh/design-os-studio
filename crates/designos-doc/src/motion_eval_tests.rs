//! Sampling tests for [`crate::evaluate_motion`].

use crate::*;

const D: u64 = 1_000_000;

fn linear_key(time_us: i64, value: f32) -> Keyframe {
    let mut key = Keyframe::new(time_us, KeyValue::Float(value));
    key.easing = Some(KeyEasing::new(EasingType::Linear));
    key
}

fn float_track(id: &str, timeline: &str, property: TrackProperty, op: KeyframeOperation, keys: &[(i64, f32)]) -> Track {
    let mut track = Track::new(id, timeline, property);
    track.operation = op;
    for (time, value) in keys {
        track.set_keyframe(linear_key(*time, *value));
    }
    track
}

fn assign(timeline: &str, containing: &str, offset_us: i64) -> TimelineAssignment {
    TimelineAssignment { timeline: timeline.into(), containing: containing.into(), offset_us, disabled: false, autoplay: false }
}

fn install(node: &mut Node, timelines: Vec<Timeline>, assignments: Vec<TimelineAssignment>, tracks: Vec<Track>) {
    node.motion = Some(Box::new(NodeMotion { timelines, assignments, tracks }));
}

fn doc_with(build: impl FnOnce(&mut Node)) -> (Document, NodeId) {
    let mut doc = Document::new("anim");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut node = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 40.0));
    build(&mut node);
    doc.insert(page, node).unwrap();
    (doc, id)
}

fn playing(playback: PlaybackStyle, duration: u64, track: Track) -> (Document, NodeId) {
    doc_with(|node| {
        let mut timeline = Timeline::new("tl", duration);
        timeline.playback = playback;
        install(node, vec![timeline], vec![assign("tl", "tl", 0)], vec![track]);
    })
}

fn float_at(doc: &Document, t_us: i64, property: TrackProperty) -> f32 {
    let samples = evaluate_motion(doc, t_us);
    match samples.into_iter().find(|sample| sample.property == property).map(|sample| sample.value) {
        Some(KeyValue::Float(value)) => value,
        other => panic!("expected a float {property:?}, got {other:?}"),
    }
}

#[test]
fn boomerang_at_one_and_half_duration_equals_half_duration() {
    let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (D as i64, 80.0)]);
    let (doc, _) = playing(PlaybackStyle::Boomerang, D, track);
    let half = float_at(&doc, (D / 2) as i64, TrackProperty::Opacity);
    let one_and_half = float_at(&doc, (D + D / 2) as i64, TrackProperty::Opacity);
    assert_eq!(half, 40.0, "halfway through the forward pass");
    assert_eq!(one_and_half, half, "the return pass at 1.5d is the same sample as 0.5d");
}

#[test]
fn loop_wraps() {
    let x = 400_000i64;
    let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (D as i64, 100.0)]);
    let (doc, _) = playing(PlaybackStyle::Loop, D, track);
    let at_x = float_at(&doc, x, TrackProperty::Opacity);
    let wrapped = float_at(&doc, D as i64 + x, TrackProperty::Opacity);
    assert_eq!(at_x, 40.0);
    assert_eq!(wrapped, at_x, "d + x plays the same sample as x");
}

#[test]
fn once_clamps_after_end() {
    // The segment runs past the timeline so a missing clamp would reach the key at 2d.
    let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), ((D * 2) as i64, 100.0)]);
    let (doc, _) = playing(PlaybackStyle::Once, D, track);
    let at_end = float_at(&doc, D as i64, TrackProperty::Opacity);
    let after = float_at(&doc, (D * 2) as i64, TrackProperty::Opacity);
    assert_eq!(at_end, 50.0, "duration d is the midpoint of the 0..2d segment");
    assert_eq!(after, at_end, "past the end holds the frame at d");
}

#[test]
fn spring_easing_matches_spring_at() {
    let params = SpringParams { mass: 1.0, stiffness: 200.0, damping: 12.0, normalized: false };
    let mut end = Keyframe::new(D as i64, KeyValue::Float(1.0));
    end.interpolation = InterpolationType::Spring;
    end.easing = Some(KeyEasing { easing_type: EasingType::CustomSpring, bezier: None, spring: Some(params) });
    let mut track = Track::new("tr", "tl", TrackProperty::Opacity);
    track.set_keyframe(Keyframe::new(0, KeyValue::Float(0.0)));
    track.set_keyframe(end);
    let (doc, _) = playing(PlaybackStyle::Once, D, track);
    let settle = spring_settle_secs(params.mass, params.stiffness, params.damping);
    for t_us in [0i64, 250_000, 500_000, 750_000, 990_000] {
        let u = (t_us as f64 / D as f64) as f32;
        let want = spring_at(params.mass, params.stiffness, params.damping, u * settle);
        let got = float_at(&doc, t_us, TrackProperty::Opacity);
        assert_eq!(got, want, "progress at {t_us} us");
    }
}

#[test]
fn set_scale_offset_operations() {
    let (doc, id) = doc_with(|node| {
        node.opacity = 0.5;
        let timeline = Timeline::new("tl", D);
        let set = float_track("set", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.2)]);
        let scale = float_track("scale", "tl", TrackProperty::Width, KeyframeOperation::Scale, &[(0, 1.5)]);
        let offset = float_track("offset", "tl", TrackProperty::Height, KeyframeOperation::Offset, &[(0, 7.0)]);
        install(node, vec![timeline], vec![assign("tl", "tl", 0)], vec![set, scale, offset]);
    });
    assert_eq!(doc.get(id).unwrap().opacity, 0.5, "the stored opacity is the Set base, and Set replaces it");
    assert_eq!(float_at(&doc, 0, TrackProperty::Opacity), 0.2);
    assert_eq!(float_at(&doc, 0, TrackProperty::Width), 150.0, "width 100 scaled by 1.5");
    assert_eq!(float_at(&doc, 0, TrackProperty::Height), 47.0, "height 40 offset by 7");
}

#[test]
fn quantized_timeline_snaps_to_frames() {
    let (doc, _) = doc_with(|node| {
        let mut timeline = Timeline::new("tl", 2_000_000);
        timeline.snap_fps = Some(FrameRate::new(10, 1));
        timeline.quantize_playback = true;
        let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (1_000_000, 1000.0)]);
        install(node, vec![timeline], vec![assign("tl", "tl", 0)], vec![track]);
    });
    let frame = FrameRate::new(10, 1).frame_us().unwrap();
    assert_eq!(frame, 100_000);
    // 250_000 is 2.5 frames. Timeline::snap rounds half away from zero, to 300_000.
    let at_half_frame = float_at(&doc, 250_000, TrackProperty::Opacity);
    let at_third_frame = float_at(&doc, 300_000, TrackProperty::Opacity);
    let at_second_frame = float_at(&doc, 200_000, TrackProperty::Opacity);
    assert_eq!(at_half_frame, at_third_frame, "2.5 frames snaps to frame 3");
    assert_ne!(at_half_frame, at_second_frame, "frame 2 is a different sample");
}

#[test]
fn evaluation_is_pure() {
    let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (D as i64, 80.0)]);
    let (doc, id) = playing(PlaybackStyle::Boomerang, D, track);
    let before_json = doc.to_json().unwrap();
    let before_nodes: Vec<Node> = doc.nodes.values().cloned().collect();
    let before_hash: Vec<(NodeId, u64)> = doc.nodes.values().map(|node| (node.id, crate::branch_diff::node_content_hash(node))).collect();
    let samples = evaluate_motion(&doc, (D + D / 2) as i64);
    assert_eq!(samples.len(), 1, "the sample ran against a real track");
    assert_eq!(doc.to_json().unwrap(), before_json);
    let after_nodes: Vec<Node> = doc.nodes.values().cloned().collect();
    assert_eq!(after_nodes, before_nodes);
    let after_hash: Vec<(NodeId, u64)> = doc.nodes.values().map(|node| (node.id, crate::branch_diff::node_content_hash(node))).collect();
    assert_eq!(after_hash, before_hash);
    assert_eq!(doc.get(id).unwrap(), before_nodes.iter().find(|node| node.id == id).unwrap());
}

#[test]
fn nested_timeline_time_is_parent_local_minus_start() {
    let (doc, _) = doc_with(|node| {
        let outer = Timeline::new("outer", 2_000_000);
        let inner = Timeline::new("inner", D);
        let track = float_track("tr", "inner", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (D as i64, 100.0)]);
        install(node, vec![outer, inner], vec![assign("inner", "outer", 200_000)], vec![track]);
    });
    assert_eq!(float_at(&doc, 200_000, TrackProperty::Opacity), 0.0, "the child starts when the parent reaches the offset");
    assert_eq!(float_at(&doc, 700_000, TrackProperty::Opacity), 50.0, "parent local 700_000 minus start 200_000 is halfway");
}
