//! Tests of the Motion model: serialization, interpolation, validation and undo.

use crate::*;

fn doc_with_frame() -> (Document, NodeId) {
    let mut d = Document::new("anim");
    let page = d.add_page("Page 1");
    let frame = d.alloc_id();
    d.insert(page, Node::new(frame, NodeKind::Frame(FrameProps::default()))).unwrap();
    (d, frame)
}

fn add(d: &mut Document, h: &mut History, c: Result<Command, MotionError>) {
    h.exec_user(d, c.expect("motion edit accepted")).expect("command applies");
}

fn motion_of(d: &Document, id: NodeId) -> NodeMotion {
    d.get(id).and_then(|n| n.motion.as_deref().cloned()).unwrap_or_default()
}

fn rich_motion() -> NodeMotion {
    let mut outer = Timeline::new("3:1", 2_000_000);
    outer.is_default = true;
    outer.playback = PlaybackStyle::Boomerang;
    outer.autoplay = true;
    let mut inner = Timeline::new("3:2", 500_000);
    inner.parent = Some("3:1".into());
    inner.snap_fps = Some(FrameRate::new(30, 1));
    inner.quantize_playback = true;
    let mut opacity = Track::new("4:1", "3:1", TrackProperty::Opacity);
    opacity.operation = KeyframeOperation::Scale;
    opacity.loop_mode = TrackLoop::Loop;
    let mut mid = Keyframe::new(500_000, KeyValue::Float(0.75));
    mid.interpolation = InterpolationType::Spring;
    mid.easing = Some(KeyEasing {
        easing_type: EasingType::CustomSpring,
        bezier: None,
        spring: Some(SpringParams { stiffness: 200.0, damping: 12.0, mass: 1.0, normalized: false }),
    });
    opacity.set_keyframe(mid);
    opacity.set_keyframe(Keyframe::new(0, KeyValue::Float(0.0)));
    let mut color = Track::new("4:2", "3:2", TrackProperty::Other("FILL_COLOR".into()));
    let mut c1 = Keyframe::new(250_000, KeyValue::Color(Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }));
    c1.color_space = ColorSpace::Oklch;
    c1.easing = Some(KeyEasing { easing_type: EasingType::CustomCubic, bezier: Some([0.5, 0.0, 0.25, 1.0]), spring: None });
    color.set_keyframe(c1);
    color.set_keyframe(Keyframe::new(0, KeyValue::Raw { kind: "PAINT".into(), json: serde_json::json!({"type": "SOLID"}) }));
    NodeMotion {
        timelines: vec![outer, inner],
        assignments: vec![TimelineAssignment { timeline: "3:2".into(), containing: "3:1".into(), offset_us: -250_000, disabled: true, autoplay: true }],
        tracks: vec![opacity, color],
    }
}

#[test]
fn a_document_without_motion_serializes_byte_identical() {
    let (d, _) = doc_with_frame();
    let json = d.to_json().unwrap();
    assert!(!json.contains("motion"), "a layer without Motion writes no motion key");
    let back = Document::from_json(&json).unwrap();
    assert_eq!(back.to_json().unwrap(), json);
}

#[test]
fn a_fixture_without_motion_re_serializes_byte_identical() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../designos-app/tests/fixtures/mask-types.designos");
    let raw = std::fs::read_to_string(path).expect("committed fixture");
    let doc = Document::from_json(&raw).unwrap();
    assert!(doc.nodes.values().all(|n| n.motion.is_none()));
    assert_eq!(doc.to_json().unwrap(), raw.trim());
}

#[test]
fn nested_timelines_snap_quantize_and_track_loop_round_trip_through_designos_json() {
    let (mut d, frame) = doc_with_frame();
    let motion = rich_motion();
    d.get_mut(frame).unwrap().motion = Some(Box::new(motion.clone()));
    let json = d.to_json().unwrap();
    let back = Document::from_json(&json).unwrap();
    assert_eq!(motion_of(&back, frame), motion);
    assert_eq!(back.to_json().unwrap(), json, "second save is identical");
    // Keyframes are stored in time order.
    assert_eq!(motion.tracks[0].keyframes.iter().map(|k| k.time_us).collect::<Vec<_>>(), vec![0, 500_000]);
}

#[test]
fn defaults_are_not_written() {
    let (mut d, frame) = doc_with_frame();
    let mut m = NodeMotion::default();
    m.timelines.push(Timeline::new("1", 1_000_000));
    let mut t = Track::new("2", "1", TrackProperty::Opacity);
    t.set_keyframe(Keyframe::new(0, KeyValue::Float(1.0)));
    m.tracks.push(t);
    let json = serde_json::to_string(&m).unwrap();
    d.get_mut(frame).unwrap().motion = Some(Box::new(m));
    assert!(d.to_json().unwrap().contains("\"motion\""));
    for absent in [
        "is_default",
        "parent",
        "playback",
        "autoplay",
        "snap_fps",
        "quantize_playback",
        "operation",
        "loop_mode",
        "interpolation",
        "easing",
        "color_space",
        "assignments",
    ] {
        assert!(!json.contains(&format!("\"{absent}\"")), "{absent} should be skipped: {json}");
    }
}

#[test]
fn track_property_names_follow_the_kiwi_variable_field() {
    for name in ["OPACITY", "MOTION_TRANSLATION_XY", "MOTION_SCALE_X", "PATH_TRIM_END", "MOTION_TRANSFORM_ORIGIN_XY", "BLUR_RADIUS"] {
        assert_eq!(TrackProperty::from_kiwi_name(name).kiwi_name(), name);
    }
    assert_eq!(TrackProperty::from_kiwi_name("BLUR_RADIUS"), TrackProperty::Other("BLUR_RADIUS".into()));
    assert!(serde_json::from_str::<TrackProperty>("\"\"").is_err());
}

#[test]
fn easing_reuses_the_prototype_curves() {
    let spring = KeyEasing {
        easing_type: EasingType::CustomSpring,
        bezier: None,
        spring: Some(SpringParams { stiffness: 200.0, damping: 12.0, mass: 1.0, normalized: false }),
    };
    let want = ease(&Easing::Spring { mass: 1.0, stiffness: 200.0, damping: 12.0 }, 0.4);
    assert_eq!(spring.progress(0.4), want);
    let cubic = KeyEasing { easing_type: EasingType::CustomCubic, bezier: Some([0.5, 0.0, 0.25, 1.0]), spring: None };
    assert_eq!(cubic.progress(0.3), cubic_bezier([0.5, 0.0, 0.25, 1.0], 0.3));
    let mut hold = Keyframe::new(1, KeyValue::Float(1.0));
    hold.interpolation = InterpolationType::Hold;
    assert_eq!((hold.progress(0.99), hold.progress(1.0)), (0.0, 1.0));
    let plain = Keyframe::new(1, KeyValue::Float(1.0));
    assert_eq!(plain.progress(0.5), KeyEasing::default().progress(0.5));
}

#[test]
fn numbers_vectors_and_shapes_blend_linearly() {
    let f = |a: f32, b: f32, u: f32| interpolate(&KeyValue::Float(a), &KeyValue::Float(b), u, ColorSpace::Default);
    assert_eq!(f(10.0, 20.0, 0.25), KeyValue::Float(12.5));
    assert_eq!(f(0.0, 1.0, 1.2), KeyValue::Float(1.2), "an overshooting spring goes past the end");
    let v = interpolate(&KeyValue::Vector(Vec2::new(0.0, 10.0)), &KeyValue::Vector(Vec2::new(10.0, 20.0)), 0.5, ColorSpace::Default);
    assert_eq!(v, KeyValue::Vector(Vec2::new(5.0, 15.0)));
    let c = interpolate(
        &KeyValue::Circle { center: Vec2::new(0.0, 0.0), radius: 2.0 },
        &KeyValue::Circle { center: Vec2::new(4.0, 8.0), radius: 6.0 },
        0.5,
        ColorSpace::Default,
    );
    assert_eq!(c, KeyValue::Circle { center: Vec2::new(2.0, 4.0), radius: 4.0 });
}

#[test]
fn values_that_do_not_blend_hold_until_the_end() {
    let (a, b) = (KeyValue::Bool(false), KeyValue::Bool(true));
    assert_eq!(interpolate(&a, &b, 0.99, ColorSpace::Default), a);
    assert_eq!(interpolate(&a, &b, 1.0, ColorSpace::Default), b);
    let raw = |k: &str| KeyValue::Raw { kind: k.into(), json: serde_json::json!(k) };
    assert_eq!(interpolate(&raw("TEXT_DATA"), &raw("PAINT"), 0.5, ColorSpace::Default), raw("TEXT_DATA"));
    assert_eq!(interpolate(&KeyValue::Float(1.0), &KeyValue::Bool(true), 0.5, ColorSpace::Default), KeyValue::Float(1.0));
}

fn rgb(r: f32, g: f32, b: f32) -> KeyValue {
    KeyValue::Color(Color { r, g, b, a: 1.0 })
}

fn rgba(v: &KeyValue) -> [f32; 4] {
    match v {
        KeyValue::Color(c) => [c.r, c.g, c.b, c.a],
        other => panic!("not a colour: {other:?}"),
    }
}

#[test]
fn colours_blend_in_the_chosen_space() {
    let (red, blue) = (rgb(1.0, 0.0, 0.0), rgb(0.0, 0.0, 1.0));
    let mid = |s| rgba(&interpolate(&red, &blue, 0.5, s));
    assert_eq!(mid(ColorSpace::Default), [0.5, 0.0, 0.5, 1.0]);
    // Linear light: the midpoint of the encoded values is brighter than the plain average.
    let lin = mid(ColorSpace::SrgbLinear);
    assert!((lin[0] - 0.7354).abs() < 1e-3 && (lin[2] - 0.7354).abs() < 1e-3, "{lin:?}");
    // Oklab midpoint equals the sRGB of the averaged Oklab coordinates.
    let (p, q) = (srgb_to_oklab(1.0, 0.0, 0.0), srgb_to_oklab(0.0, 0.0, 1.0));
    let want = oklab_to_srgb((p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, (p[2] + q[2]) / 2.0);
    let got = mid(ColorSpace::Oklab);
    assert!((0..3).all(|i| (got[i] - want[i]).abs() < 1e-5), "{got:?} vs {want:?}");
    // Oklch takes the shorter hue arc: red (~29 deg) to blue (~264 deg) goes through magenta, not green.
    let lch = mid(ColorSpace::Oklch);
    assert!(lch[1] < lch[0] && lch[1] < lch[2], "magenta-ish, not green: {lch:?}");
    // HSL red to blue: shorter arc through magenta (hue 300).
    let hsl = mid(ColorSpace::Hsl);
    assert!((hsl[0] - 1.0).abs() < 1e-5 && hsl[1].abs() < 1e-5 && (hsl[2] - 1.0).abs() < 1e-5, "{hsl:?}");
    for s in [ColorSpace::Default, ColorSpace::Oklab, ColorSpace::Oklch, ColorSpace::Hsl, ColorSpace::SrgbLinear] {
        let at0 = rgba(&interpolate(&red, &blue, 0.0, s));
        let at1 = rgba(&interpolate(&red, &blue, 1.0, s));
        assert!((0..3).all(|i| (at0[i] - [1.0, 0.0, 0.0][i]).abs() < 2e-3), "{s:?} start {at0:?}");
        assert!((0..3).all(|i| (at1[i] - [0.0, 0.0, 1.0][i]).abs() < 2e-3), "{s:?} end {at1:?}");
    }
}

#[test]
fn alpha_blends_linearly_in_every_space() {
    let a = KeyValue::Color(Color { r: 1.0, g: 0.0, b: 0.0, a: 0.0 });
    let b = KeyValue::Color(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
    for s in [ColorSpace::Default, ColorSpace::Oklab, ColorSpace::Oklch, ColorSpace::Hsl, ColorSpace::SrgbLinear] {
        assert!((rgba(&interpolate(&a, &b, 0.25, s))[3] - 0.25).abs() < 1e-6);
    }
}

#[test]
fn a_timeline_snaps_a_time_to_whole_frames() {
    let mut t = Timeline::new("1", 1_000_000);
    assert_eq!(t.snap(12_345), 12_345, "no snapping without a rate");
    t.snap_fps = Some(FrameRate::new(30, 1));
    assert_eq!(t.snap(50_000), 66_666, "1.5 frames rounds up to frame 2 of 33_333 us");
    assert_eq!(t.snap(0), 0);
    t.snap_fps = Some(FrameRate::new(0, 1));
    assert_eq!(t.snap(12_345), 12_345, "a zero rate does not snap");
}

#[test]
fn keyframes_keep_time_order_and_one_per_time() {
    let mut t = Track::new("t", "tl", TrackProperty::Opacity);
    for time in [300, 100, 200] {
        t.set_keyframe(Keyframe::new(time, KeyValue::Float(time as f32)));
    }
    assert_eq!(t.set_keyframe(Keyframe::new(200, KeyValue::Float(9.0))).map(|k| k.value), Some(KeyValue::Float(200.0)));
    assert_eq!(t.keyframes.iter().map(|k| k.time_us).collect::<Vec<_>>(), vec![100, 200, 300]);
    let mut loose = Track::new("t", "tl", TrackProperty::Opacity);
    loose.keyframes = vec![Keyframe::new(5, KeyValue::Float(1.0)), Keyframe::new(1, KeyValue::Float(2.0)), Keyframe::new(5, KeyValue::Float(3.0))];
    loose.normalize();
    assert_eq!(loose.keyframes.iter().map(|k| (k.time_us, k.value.clone())).collect::<Vec<_>>(), vec![(1, KeyValue::Float(2.0)), (5, KeyValue::Float(3.0))]);
}

fn setup_with_timeline() -> (Document, History, NodeId) {
    let (mut d, frame) = doc_with_frame();
    let mut h = History::default();
    let c = add_timeline(&d, frame, Timeline::new("tl", 1_000_000));
    add(&mut d, &mut h, c);
    (d, h, frame)
}

#[test]
fn adding_changing_and_removing_a_timeline_each_undo_in_one_step() {
    let (mut d, mut h, frame) = setup_with_timeline();
    let before_edit = motion_of(&d, frame);
    let c = update_timeline(&d, "tl", |t| {
        t.duration_us = 2_000_000;
        t.playback = PlaybackStyle::Loop;
    });
    add(&mut d, &mut h, c);
    assert_eq!(motion_of(&d, frame).timeline("tl").unwrap().duration_us, 2_000_000);
    h.undo(&mut d).unwrap();
    assert_eq!(motion_of(&d, frame), before_edit);
    h.redo(&mut d).unwrap();
    assert_eq!(motion_of(&d, frame).timeline("tl").unwrap().playback, PlaybackStyle::Loop);
    let c = remove_timeline(&d, "tl");
    add(&mut d, &mut h, c);
    assert!(d.get(frame).unwrap().motion.is_none(), "the last timeline gone clears the field");
    h.undo(&mut d).unwrap();
    assert!(motion_of(&d, frame).timeline("tl").is_some());
    h.undo(&mut d).unwrap();
    h.undo(&mut d).unwrap();
    assert!(d.get(frame).unwrap().motion.is_none(), "all undone leaves the layer as it was");
}

#[test]
fn tracks_and_keyframes_add_change_move_and_remove_with_undo() {
    let (mut d, mut h, frame) = setup_with_timeline();
    let c = add_track(&d, frame, Track::new("tr", "tl", TrackProperty::Opacity));
    add(&mut d, &mut h, c);
    let c = set_keyframe(&d, frame, "tr", Keyframe::new(0, KeyValue::Float(0.0)));
    add(&mut d, &mut h, c);
    let c = set_keyframe(&d, frame, "tr", Keyframe::new(1_000_000, KeyValue::Float(1.0)));
    add(&mut d, &mut h, c);
    let times = |d: &Document| motion_of(d, frame).track("tr").unwrap().keyframes.iter().map(|k| k.time_us).collect::<Vec<_>>();
    assert_eq!(times(&d), vec![0, 1_000_000]);

    let c = set_keyframe(&d, frame, "tr", Keyframe::new(0, KeyValue::Float(0.25)));
    add(&mut d, &mut h, c);
    assert_eq!(motion_of(&d, frame).track("tr").unwrap().keyframe(0).unwrap().value, KeyValue::Float(0.25));
    h.undo(&mut d).unwrap();
    assert_eq!(motion_of(&d, frame).track("tr").unwrap().keyframe(0).unwrap().value, KeyValue::Float(0.0));

    let c = move_keyframe(&d, frame, "tr", 1_000_000, 500_000);
    add(&mut d, &mut h, c);
    assert_eq!(times(&d), vec![0, 500_000]);
    h.undo(&mut d).unwrap();
    assert_eq!(times(&d), vec![0, 1_000_000]);
    h.redo(&mut d).unwrap();

    let c = remove_keyframe(&d, frame, "tr", 500_000);
    add(&mut d, &mut h, c);
    assert_eq!(times(&d), vec![0]);
    h.undo(&mut d).unwrap();
    assert_eq!(times(&d), vec![0, 500_000]);

    let c = update_track(&d, frame, "tr", |t| t.operation = KeyframeOperation::Offset);
    add(&mut d, &mut h, c);
    assert_eq!(motion_of(&d, frame).track("tr").unwrap().operation, KeyframeOperation::Offset);
    let c = remove_track(&d, frame, "tr");
    add(&mut d, &mut h, c);
    assert!(motion_of(&d, frame).track("tr").is_none());
    h.undo(&mut d).unwrap();
    assert_eq!(motion_of(&d, frame).track("tr").unwrap().operation, KeyframeOperation::Offset);
}

#[test]
fn moving_a_keyframe_snaps_to_the_timeline_frame_grid_and_refuses_a_taken_time() {
    let (mut d, mut h, frame) = setup_with_timeline();
    let c = update_timeline(&d, "tl", |t| t.snap_fps = Some(FrameRate::new(30, 1)));
    add(&mut d, &mut h, c);
    let c = add_track(&d, frame, Track::new("tr", "tl", TrackProperty::Opacity));
    add(&mut d, &mut h, c);
    for t in [0, 100_000] {
        let c = set_keyframe(&d, frame, "tr", Keyframe::new(t, KeyValue::Float(1.0)));
        add(&mut d, &mut h, c);
    }
    let c = move_keyframe(&d, frame, "tr", 100_000, 33_000);
    add(&mut d, &mut h, c);
    let times = motion_of(&d, frame).track("tr").unwrap().keyframes.iter().map(|k| k.time_us).collect::<Vec<_>>();
    assert_eq!(times, vec![0, 33_333], "33_000 us snaps to frame 1 of a 30 fps grid");
    let c = set_keyframe(&d, frame, "tr", Keyframe::new(66_667, KeyValue::Float(0.5)));
    add(&mut d, &mut h, c);
    assert_eq!(move_keyframe(&d, frame, "tr", 66_667, 33_000).unwrap_err(), MotionError::KeyframeTaken(33_333));
}

#[test]
fn refused_edits_name_the_problem_and_change_nothing() {
    let (mut d, mut h, frame) = setup_with_timeline();
    let err = |r: Result<Command, MotionError>| r.expect_err("edit must be refused");
    assert_eq!(err(add_timeline(&d, frame, Timeline::new("tl", 1))), MotionError::BadId { kind: "timeline", id: "tl".into() });
    assert!(matches!(err(add_timeline(&d, frame, Timeline::new("", 1))), MotionError::BadId { .. }));
    assert_eq!(err(add_timeline(&d, NodeId(9999), Timeline::new("x", 1))), MotionError::NodeNotFound(NodeId(9999)));
    let mut orphan = Timeline::new("child", 1);
    orphan.parent = Some("missing".into());
    assert_eq!(err(add_timeline(&d, frame, orphan)), MotionError::NoTimeline("missing".into()));
    assert_eq!(err(add_track(&d, frame, Track::new("t", "missing", TrackProperty::Opacity))), MotionError::NoTimeline("missing".into()));
    assert_eq!(err(set_keyframe(&d, frame, "nope", Keyframe::new(0, KeyValue::Float(0.0)))), MotionError::NoTrack("nope".into()));
    assert_eq!(err(update_timeline(&d, "tl", |t| t.id = "other".into())), MotionError::IdChanged("tl".into()));
    assert_eq!(err(update_timeline(&d, "tl", |t| t.parent = Some("tl".into()))), MotionError::Cycle("tl".into()));
    let c = add_timeline(&d, frame, {
        let mut t = Timeline::new("kid", 1);
        t.parent = Some("tl".into());
        t
    });
    add(&mut d, &mut h, c);
    assert_eq!(err(update_timeline(&d, "tl", |t| t.parent = Some("kid".into()))), MotionError::Cycle("tl".into()));
    assert_eq!(err(remove_keyframe(&d, frame, "nope", 0)), MotionError::NoTrack("nope".into()));
}

#[test]
fn nesting_goes_through_assignments_and_refuses_a_loop() {
    let (mut d, mut h, frame) = setup_with_timeline();
    let c = add_timeline(&d, frame, Timeline::new("inner", 500_000));
    add(&mut d, &mut h, c);
    let a = TimelineAssignment { timeline: "inner".into(), containing: "tl".into(), offset_us: 250_000, disabled: false, autoplay: false };
    let c = assign_timeline(&d, frame, a.clone());
    add(&mut d, &mut h, c);
    assert_eq!(assign_timeline(&d, frame, a.clone()).unwrap_err(), MotionError::AlreadyAssigned { timeline: "inner".into(), containing: "tl".into() });
    let back = TimelineAssignment { timeline: "tl".into(), containing: "inner".into(), offset_us: 0, disabled: false, autoplay: false };
    assert_eq!(assign_timeline(&d, frame, back).unwrap_err(), MotionError::Cycle("tl".into()));
    let c = update_assignment(&d, frame, "inner", "tl", |a| a.offset_us = 0);
    add(&mut d, &mut h, c);
    assert_eq!(motion_of(&d, frame).assignments[0].offset_us, 0);
    let ix = MotionIndex::build(&d);
    assert_eq!(ix.assignments_in("tl").count(), 1);
    drop(ix);
    let c = unassign_timeline(&d, frame, "inner", "tl");
    add(&mut d, &mut h, c);
    assert!(motion_of(&d, frame).assignments.is_empty());
}

#[test]
fn removing_a_timeline_clears_its_tracks_assignments_and_child_links_on_every_layer_in_one_undo_step() {
    let (mut d, mut h, frame) = setup_with_timeline();
    let other = d.alloc_id();
    let page = d.pages[0];
    d.insert(page, Node::new(other, NodeKind::Frame(FrameProps::default()))).unwrap();
    let c = add_track(&d, other, Track::new("tr", "tl", TrackProperty::Rotation));
    add(&mut d, &mut h, c);
    let c = add_timeline(&d, other, {
        let mut t = Timeline::new("kid", 1);
        t.parent = Some("tl".into());
        t
    });
    add(&mut d, &mut h, c);
    let c = remove_timeline(&d, "tl");
    add(&mut d, &mut h, c);
    assert!(d.get(frame).unwrap().motion.is_none());
    let m = motion_of(&d, other);
    assert!(m.tracks.is_empty() && m.timeline("kid").unwrap().parent.is_none());
    h.undo(&mut d).unwrap();
    assert!(motion_of(&d, frame).timeline("tl").is_some());
    assert_eq!(motion_of(&d, other).tracks.len(), 1);
    assert_eq!(motion_of(&d, other).timeline("kid").unwrap().parent.as_deref(), Some("tl"));
}

#[test]
fn the_index_finds_timelines_tracks_and_ancestors_across_layers() {
    let (mut d, mut h, frame) = setup_with_timeline();
    let c = add_timeline(&d, frame, {
        let mut t = Timeline::new("kid", 1);
        t.parent = Some("tl".into());
        t
    });
    add(&mut d, &mut h, c);
    let c = add_track(&d, frame, Track::new("tr", "kid", TrackProperty::Width));
    add(&mut d, &mut h, c);
    let ix = MotionIndex::build(&d);
    assert_eq!(ix.timeline("kid").map(|(n, _)| n), Some(frame));
    assert_eq!(ix.tracks_on("kid").len(), 1);
    assert!(ix.tracks_on("tl").is_empty());
    assert_eq!(ix.ancestors("kid").iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), vec!["tl"]);
    assert_eq!(ix.timelines().count(), 2);
}

#[test]
fn a_copied_layer_with_the_same_ids_is_reported_not_silently_merged() {
    let (mut d, frame) = doc_with_frame();
    let mut m = NodeMotion::default();
    m.timelines.push(Timeline::new("tl", 1));
    m.tracks.push(Track::new("tr", "tl", TrackProperty::Opacity));
    d.get_mut(frame).unwrap().motion = Some(Box::new(m.clone()));
    let copy = d.alloc_id();
    let page = d.pages[0];
    let mut copy_node = Node::new(copy, NodeKind::Frame(FrameProps::default()));
    copy_node.motion = Some(Box::new(m));
    d.insert(page, copy_node).unwrap();
    let ix = MotionIndex::build(&d);
    assert_eq!(ix.timeline("tl").map(|(n, _)| n), Some(frame), "the first layer keeps the id");
    assert_eq!(ix.duplicate_ids(), ["tl", "tr"]);
    assert!(MotionIndex::build(&doc_with_frame().0).duplicate_ids().is_empty());
}

#[test]
fn duplicating_a_subtree_gives_the_copy_its_own_motion_ids() {
    let (mut d, frame) = doc_with_frame();
    let mut m = NodeMotion::default();
    m.timelines.push(Timeline::new("tl", 1));
    m.tracks.push(Track::new("tr", "tl", TrackProperty::Opacity));
    d.get_mut(frame).unwrap().motion = Some(Box::new(m));
    let page = d.pages[0];
    let copy = duplicate_subtree(&mut d, frame);
    let copy_id = copy[0].id;
    let mut it = copy.into_iter();
    d.insert(page, it.next().unwrap()).unwrap();
    assert!(MotionIndex::build(&d).duplicate_ids().is_empty());
    let cm = motion_of(&d, copy_id);
    assert_eq!(cm.tracks[0].timeline, cm.timelines[0].id);
    assert_ne!(cm.timelines[0].id, "tl");
    assert_eq!(motion_of(&d, frame).timelines[0].id, "tl");
}

#[test]
fn an_ancestor_loop_in_a_hand_edited_file_does_not_hang_the_index() {
    let (mut d, frame) = doc_with_frame();
    let mut a = Timeline::new("a", 1);
    a.parent = Some("b".into());
    let mut b = Timeline::new("b", 1);
    b.parent = Some("a".into());
    d.get_mut(frame).unwrap().motion = Some(Box::new(NodeMotion { timelines: vec![a, b], ..Default::default() }));
    let ix = MotionIndex::build(&d);
    assert_eq!(ix.ancestors("a").len(), 1);
}

#[test]
fn a_motion_edit_changes_node_equality_and_the_saved_json() {
    let (mut d, mut h, frame) = setup_with_timeline();
    let before = d.get(frame).unwrap().clone();
    let c = add_track(&d, frame, Track::new("tr", "tl", TrackProperty::Opacity));
    add(&mut d, &mut h, c);
    assert_ne!(d.get(frame).unwrap(), &before, "node equality includes motion");
    let json = d.to_json().unwrap();
    assert!(json.contains("\"motion\""));
}

#[test]
fn a_branch_that_adds_a_track_merges_into_main() {
    let (base, _, frame) = setup_with_timeline();
    let mut branch = base.clone();
    let mut h = History::default();
    let c = add_track(&branch, frame, Track::new("tr", "tl", TrackProperty::Opacity));
    add(&mut branch, &mut h, c);
    assert!(
        !crate::branch_diff::node_group_eq(base.get(frame).unwrap(), branch.get(frame).unwrap(), crate::branch_diff::Group::Other),
        "the motion change is a difference of the Other group"
    );
    let (merged, _) = merge_branch(&base, &base, &branch, &Resolution::default()).expect("merge");
    assert_eq!(motion_of(&merged, frame).tracks.len(), 1);
}

#[test]
fn a_live_patch_replays_adding_motion_and_its_undo_on_the_editor_document() {
    use crate::live_patch::{live_ops_for_command, replay_live_patch, LivePatchState};
    let (mut editor, frame) = doc_with_frame();
    let agent = editor.clone();
    let original = agent.get(frame).unwrap().clone();
    let add_cmd = add_timeline(&agent, frame, Timeline::new("tl", 1_000_000)).unwrap();
    let undo_cmd = Command::Update { node: Box::new(original.clone()) };
    let replay = |editor: &mut Document, ops| {
        let patch = crate::live_patch::LivePatch::new(ops);
        replay_live_patch(editor, &patch, &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap()
    };

    let ops = live_ops_for_command(&agent, &add_cmd).unwrap();
    let report = replay(&mut editor, ops);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(motion_of(&editor, frame).timelines.len(), 1);

    let mut after_add = agent.clone();
    add_cmd.apply(&mut after_add).unwrap();
    let ops = live_ops_for_command(&after_add, &undo_cmd).unwrap();
    let report = replay(&mut editor, ops);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert!(editor.get(frame).unwrap().motion.is_none(), "the undo removes the motion key on the editor");
    assert_eq!(editor.get(frame).unwrap(), &original);
}
