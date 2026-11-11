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

fn var_def(id: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> Variable {
    Variable {
        id: id.into(),
        name: id.into(),
        collection: "col".into(),
        kind,
        values: [("light".to_string(), light), ("dark".to_string(), dark)].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

fn setup_variables(doc: &mut Document) {
    doc.variable_collections.insert(
        "col".into(),
        VariableCollection {
            id: "col".into(),
            name: "Collection".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec![],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
}

#[test]
fn bound_easing_switches_per_mode_on_transition_and_track() {
    let mut doc = Document::new("easing_test");
    setup_variables(&mut doc);
    doc.variables
        .insert("v_ease".into(), var_def("v_ease", VariableKind::String, VariableValue::String("linear".into()), VariableValue::String("ease-in-out".into())));

    let page = doc.add_page("Page 1");
    let nid = doc.alloc_id();
    let mut node = Node::frame(nid, Rect::new(0.0, 0.0, 100.0, 100.0));
    node.prototype.reactions.push(Reaction {
        trigger: Trigger::OnClick,
        actions: vec![Action::Navigate { dest: nid, preserve_scroll: false }],
        transition: Some(Transition { kind: TransitionKind::SmartAnimate, easing: Easing::Linear, duration_ms: 1000 }),
    });

    let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (1_000_000, 100.0)]);
    let timeline = Timeline::new("tl", 1_000_000);
    install(&mut node, vec![timeline], vec![assign("tl", "tl", 0)], vec![track]);

    node.variables.bindings.insert("easingData".into(), "v_ease".into());
    let id = doc.insert(page, node).unwrap();

    // Mode: Light (linear)
    let tr_progress_light = evaluate_transition(&doc, id, 0, 250.0).unwrap();
    assert_eq!(tr_progress_light, 0.25);

    let motion_sample_light = float_at(&doc, 250_000, TrackProperty::Opacity);
    assert_eq!(motion_sample_light, 25.0);

    // Switch to Dark mode (ease-in-out)
    doc.get_mut(page).unwrap().variables.modes.insert("col".into(), "dark".into());

    let expected_eased = ease(&Easing::EaseInOut, 0.25);
    let tr_progress_dark = evaluate_transition(&doc, id, 0, 250.0).unwrap();
    assert!((tr_progress_dark - expected_eased).abs() < 1e-4);
    assert!((tr_progress_dark - 0.25).abs() > 0.05, "must differ from linear");

    let motion_sample_dark = float_at(&doc, 250_000, TrackProperty::Opacity);
    assert!((motion_sample_dark - expected_eased * 100.0).abs() < 1e-3);
}

#[test]
fn bound_duration_retimes_transition_and_motion_tracks() {
    let mut doc = Document::new("duration_test");
    setup_variables(&mut doc);
    doc.variables.insert("v_dur".into(), var_def("v_dur", VariableKind::Number, VariableValue::Number(1000.0), VariableValue::Number(2000.0)));

    let page = doc.add_page("Page 1");
    let nid = doc.alloc_id();
    let mut node = Node::frame(nid, Rect::new(0.0, 0.0, 100.0, 100.0));
    node.prototype.reactions.push(Reaction {
        trigger: Trigger::OnClick,
        actions: vec![Action::Navigate { dest: nid, preserve_scroll: false }],
        transition: Some(Transition { kind: TransitionKind::SmartAnimate, easing: Easing::Linear, duration_ms: 1000 }),
    });

    let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (1_000_000, 100.0)]);
    let timeline = Timeline::new("tl", 1_000_000);
    install(&mut node, vec![timeline], vec![assign("tl", "tl", 0)], vec![track]);

    node.variables.bindings.insert("timelineDuration".into(), "v_dur".into());
    let id = doc.insert(page, node).unwrap();

    // In Light mode (1000 ms duration):
    // Transition at 500 ms should be 500 / 1000 = 0.5
    assert_eq!(evaluate_transition(&doc, id, 0, 500.0).unwrap(), 0.5);
    // Motion track at 500_000 us should be 50.0
    assert_eq!(float_at(&doc, 500_000, TrackProperty::Opacity), 50.0);
    // Motion track at 1_000_000 us should be 100.0
    assert_eq!(float_at(&doc, 1_000_000, TrackProperty::Opacity), 100.0);

    // Switch to Dark mode (2000 ms duration):
    doc.get_mut(page).unwrap().variables.modes.insert("col".into(), "dark".into());

    // Transition at 500 ms is now 500 / 2000 = 0.25
    assert_eq!(evaluate_transition(&doc, id, 0, 500.0).unwrap(), 0.25);
    // Transition at 1000 ms is now 1000 / 2000 = 0.5
    assert_eq!(evaluate_transition(&doc, id, 0, 1000.0).unwrap(), 0.5);
    // Motion track at 500_000 us is now 500_000 / 2_000_000 * 100 = 25.0
    assert_eq!(float_at(&doc, 500_000, TrackProperty::Opacity), 25.0);
    // Motion track at 1_000_000 us is now 1_000_000 / 2_000_000 * 100 = 50.0
    assert_eq!(float_at(&doc, 1_000_000, TrackProperty::Opacity), 50.0);
}

#[test]
fn lint_reports_unknown_easing_presets_and_clears_on_valid() {
    let mut doc = Document::new("lint_ease");
    setup_variables(&mut doc);
    doc.variables.insert(
        "v_ease".into(),
        var_def("v_ease", VariableKind::String, VariableValue::String("invalid-curve-xyz".into()), VariableValue::String("ease-in-out".into())),
    );

    let page = doc.add_page("Page 1");
    let nid = doc.alloc_id();
    let mut node = Node::frame(nid, Rect::new(0.0, 0.0, 100.0, 100.0));
    node.variables.bindings.insert("easingData".into(), "v_ease".into());
    doc.insert(page, node).unwrap();

    let config = lint::config::LintConfig::default();
    let env = lint::NoLintEnv;
    let findings = lint::lint_single_node(&doc, nid, &config, &env);
    let easing_findings: Vec<_> = findings.iter().filter(|f| f.rule == "easing-variable").collect();
    assert_eq!(easing_findings.len(), 1, "must report unknown easing preset");
    assert!(easing_findings[0].message.contains("Unknown easing preset"));

    // Switch to Dark mode (valid preset "ease-in-out")
    doc.get_mut(page).unwrap().variables.modes.insert("col".into(), "dark".into());
    let findings_valid = lint::lint_single_node(&doc, nid, &config, &env);
    let easing_findings_valid: Vec<_> = findings_valid.iter().filter(|f| f.rule == "easing-variable").collect();
    assert!(easing_findings_valid.is_empty(), "valid preset must produce zero findings");
}

#[test]
fn unbound_document_evaluates_and_serializes_without_binding_keys() {
    let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (D as i64, 80.0)]);
    let (doc, _) = playing(PlaybackStyle::Loop, D, track);
    let json = doc.to_json().unwrap();
    assert!(!json.contains("easingData") && !json.contains("timelineDuration"), "no binding keys appear for a file that never bound them");

    // A native file written before easing and duration bindings existed loads and samples the same.
    let reloaded = Document::from_json(&json).unwrap();
    for t in [0, 250_000, 500_000, 900_000] {
        assert_eq!(float_at(&reloaded, t, TrackProperty::Opacity), float_at(&doc, t, TrackProperty::Opacity));
    }
    assert_eq!(float_at(&reloaded, 500_000, TrackProperty::Opacity), 40.0);
}

#[test]
fn unknown_easing_name_keeps_the_stored_easing() {
    let mut doc = Document::new("unknown_ease");
    setup_variables(&mut doc);
    doc.variables.insert(
        "v_ease".into(),
        var_def("v_ease", VariableKind::String, VariableValue::String("no-such-curve".into()), VariableValue::String("no-such-curve".into())),
    );
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut node = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
    node.variables.bindings.insert("easingData".into(), "v_ease".into());
    let track = float_track("tr", "tl", TrackProperty::Opacity, KeyframeOperation::Set, &[(0, 0.0), (D as i64, 100.0)]);
    install(&mut node, vec![Timeline::new("tl", D)], vec![assign("tl", "tl", 0)], vec![track]);
    doc.insert(page, node).unwrap();
    assert_eq!(float_at(&doc, 500_000, TrackProperty::Opacity), 50.0, "linear stored easing survives an unknown preset name");
}
