//! Motion records: the converters read what the importer keeps, and write it back in the same shape.

use designos_doc::{
    evaluate_motion, ColorSpace, EasingType, FrameRate, InterpolationType, KeyEasing, KeyValue, Keyframe, KeyframeOperation, NodeKind, NodeMotion,
    PlaybackStyle, SpringParams, Timeline, TimelineAssignment, Track, TrackLoop, TrackProperty, Vec2,
};
use designos_fig::motion::*;
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes, testkit};
use serde_json::{json, Map, Value};

fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(id: (u64, u64), parent: (u64, u64), ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": g(id.0, id.1),
        "parentIndex": {"guid": g(parent.0, parent.1), "position": "!"},
        "type": ty,
        "name": name,
        "phase": "CREATED",
        "visible": true,
        "opacity": 1.0,
        "size": {"x": 120.0, "y": 60.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

fn motion_file() -> Value {
    json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((1, 20), (0, 1), "FRAME", "Stage", json!({
            "timelineDefinitions": {"entries": [
                {"id": g(3, 10), "data": {"durationUs": 2000000, "defaultTimeline": true, "playbackStyle": "BOOMERANG", "autoplay": true}},
                {"id": g(3, 11), "data": {"durationUs": 500000, "parentTimelineDefId": g(3, 10), "keyframeSnappingFps": {"numerator": 30, "denominator": 1}, "quantizePlayback": true}}
            ]},
            "timelineAssignments": {"entries": [
                {"key": {"assignedTimelineId": g(3, 11), "containingTimelineId": g(3, 10)}, "value": {"offsetUs": -250000, "disabled": true, "autoplay": true}}
            ]}
        })),
        node((1, 21), (1, 20), "KEYFRAME_TRACK", "Track", json!({"keyframeOperation": "SCALE", "trackLoopMode": "LOOP"})),
        node((1, 22), (1, 21), "KEYFRAME", "Key", json!({
            "timelinePosition": 500000,
            "keyframeValue": {"value": {"floatValue": 0.75}, "valueType": "FLOAT"},
            "interpolationType": "SPRING",
            "bezierHandles": {"p1x": -0.25, "p1y": 0.0, "p2x": 0.25, "p2y": 1.0},
            "easingData": {"easingType": "CUSTOM_SPRING", "easingValue": {"springEasing": {"stiffness": 200.0, "damping": 12.0, "mass": 1.0, "springType": "PHYSICAL"}}},
            "interpolationColorSpace": "OKLCH"
        })),
    ]})
}

/// All kept fields of a node as one object: JSON where the importer wrote JSON, the plain text (an enum name) otherwise.
fn kept_node(meta: &std::collections::BTreeMap<String, String>) -> Value {
    Value::Object(meta.iter().map(|(k, v)| (k.clone(), serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.clone())))).collect::<Map<_, _>>())
}

#[test]
fn timelines_and_assignments_read_what_the_importer_keeps_and_write_it_back() {
    let doc = import_fig_node_changes(&motion_file(), "Motion").expect("import");
    let stage = doc.nodes.values().find(|n| n.name == "Stage").expect("stage");
    let motion = stage.motion.as_deref().expect("stage motion");

    assert!(
        !stage.fig_metadata.contains_key("timelineDefinitions") && !stage.fig_metadata.contains_key("timelineAssignments"),
        "modelled timeline records are dropped from fig_metadata"
    );

    let mut outer = Timeline::new("3:10", 2_000_000);
    outer.is_default = true;
    outer.playback = PlaybackStyle::Boomerang;
    outer.autoplay = true;
    let mut inner = Timeline::new("3:11", 500_000);
    inner.parent = Some("3:10".into());
    inner.snap_fps = Some(FrameRate::new(30, 1));
    inner.quantize_playback = true;
    assert_eq!(motion.timelines, vec![outer, inner]);
    assert_eq!(timelines_from_kiwi(Some(&timelines_to_kiwi(&motion.timelines))), motion.timelines);

    assert_eq!(
        motion.assignments,
        vec![TimelineAssignment { timeline: "3:11".into(), containing: "3:10".into(), offset_us: -250_000, disabled: true, autoplay: true }]
    );
    let again = assignments_to_kiwi(&motion.assignments);
    assert_eq!(assignments_from_kiwi(Some(&again)), motion.assignments);
}

#[test]
fn a_keyframe_node_converts_both_ways() {
    let doc = import_fig_node_changes(&motion_file(), "Motion").expect("import");
    let key_node = doc.nodes.values().find(|n| n.name == "Key").expect("key");
    let track_node = doc.nodes.values().find(|n| n.name == "Track").expect("track");

    let as_node = kept_node(&key_node.fig_metadata);
    let key = keyframe_from_kiwi(&as_node).expect("keyframe");
    let mut want = Keyframe::new(500_000, KeyValue::Float(0.75));
    want.interpolation = InterpolationType::Spring;
    want.bezier_handles = Some([-0.25, 0.0, 0.25, 1.0]);
    want.easing = Some(KeyEasing {
        easing_type: EasingType::CustomSpring,
        bezier: None,
        spring: Some(SpringParams { stiffness: 200.0, damping: 12.0, mass: 1.0, normalized: false }),
    });
    want.color_space = ColorSpace::Oklch;
    assert_eq!(key, want);

    let mut out = Map::new();
    keyframe_to_kiwi(&key, &mut out);
    assert_eq!(keyframe_from_kiwi(&Value::Object(out)), Some(want));

    let track_meta = kept_node(&track_node.fig_metadata);
    assert_eq!(track_modes_from_kiwi(&track_meta), (KeyframeOperation::Scale, TrackLoop::Loop));
    let mut out = Map::new();
    track_modes_to_kiwi(KeyframeOperation::Scale, TrackLoop::Loop, &mut out);
    assert_eq!((out["keyframeOperation"].as_str(), out["trackLoopMode"].as_str()), (Some("SCALE"), Some("LOOP")));
}

#[test]
fn every_value_kind_and_the_unmapped_easing_survive_a_conversion() {
    let values = [
        KeyValue::Float(1.5),
        KeyValue::Bool(true),
        KeyValue::Vector(designos_doc::Vec2::new(1.0, -2.0)),
        KeyValue::Color(designos_doc::Color { r: 0.25, g: 0.5, b: 0.75, a: 0.5 }),
        KeyValue::Circle { center: designos_doc::Vec2::new(1.0, 2.0), radius: 3.0 },
        KeyValue::Line { a: designos_doc::Vec2::new(0.0, 1.0), b: designos_doc::Vec2::new(2.0, 3.0) },
        KeyValue::CirclePoint { center: designos_doc::Vec2::new(1.0, 2.0), radius: 3.0, angle: 45.0 },
        KeyValue::Rotation3D { x: 1.0, y: 2.0, z: 3.0, translate_z: 4.0 },
        KeyValue::Raw { kind: "TEXT_DATA".into(), json: json!({"textDataValue": {"characters": "Hi"}}) },
    ];
    for v in values {
        assert_eq!(key_value_from_kiwi(&key_value_to_kiwi(&v)), v);
    }
    let ease_in = easing_from_kiwi(&json!({"easingType": "EASE_IN"}));
    assert_eq!(ease_in.easing_type, EasingType::EaseIn);
    assert_eq!(easing_to_kiwi(&ease_in)["easingType"], "EASE_IN");
    let cubic = easing_from_kiwi(&json!({"easingType": "CUSTOM_CUBIC", "easingValue": {"bezierEasing": {"p1x": 0.5, "p1y": 0.0, "p2x": 0.25, "p2y": 1.0}}}));
    assert_eq!((cubic.easing_type, cubic.bezier), (EasingType::CustomCubic, Some([0.5, 0.0, 0.25, 1.0])));
}

#[test]
fn a_kiwi_keyframe_without_an_interpolation_type_is_hold_and_stays_hold_going_back() {
    let node = json!({"timelinePosition": 10, "keyframeValue": {"value": {"floatValue": 1.0}, "valueType": "FLOAT"}});
    let key = keyframe_from_kiwi(&node).expect("keyframe");
    assert_eq!(key.interpolation, InterpolationType::Hold, "kiwi tag 0 is HOLD, the absent-field value");
    let mut out = Map::new();
    keyframe_to_kiwi(&key, &mut out);
    assert_eq!(out["interpolationType"], "HOLD");
    assert_eq!(keyframe_from_kiwi(&Value::Object(out)).unwrap().interpolation, InterpolationType::Hold);
    // A keyframe made by a user keeps the Bezier default.
    assert_eq!(Keyframe::new(0, KeyValue::Float(0.0)).interpolation, InterpolationType::Bezier);
    // An unknown name is not silently turned into a curve either.
    let odd = json!({"timelinePosition": 1, "keyframeValue": {"value": {"floatValue": 1.0}, "valueType": "FLOAT"}, "interpolationType": "WOBBLE"});
    assert_eq!(keyframe_from_kiwi(&odd).unwrap().interpolation, InterpolationType::Hold);
}

#[test]
fn damaged_records_read_as_defaults_instead_of_failing() {
    assert!(timelines_from_kiwi(None).is_empty());
    assert!(timelines_from_kiwi(Some(&json!({"entries": [{"data": {}}]}))).is_empty(), "an entry without an id is skipped");
    let t = timelines_from_kiwi(Some(&json!({"entries": [{"id": g(1, 1), "data": {"playbackStyle": "NOPE", "durationUs": "x"}}]})));
    assert_eq!((t[0].playback, t[0].duration_us), (PlaybackStyle::Once, 0));
    assert_eq!(keyframe_from_kiwi(&json!({"timelinePosition": 5})), None, "no value, nothing to place");
    assert_eq!(key_value_from_kiwi(&json!({"valueType": "FLOAT"})), KeyValue::Raw { kind: "FLOAT".into(), json: Value::Null });
}

#[test]
fn the_motion_records_still_survive_a_fig_round_trip_through_the_passthrough() {
    let doc = import_fig_node_changes(&motion_file(), "Motion").expect("import");
    let bytes = export_fig(&doc).expect("export");
    let back = import_fig(&bytes).expect("re-import");
    for name in ["Stage", "Track", "Key"] {
        let before = doc.nodes.values().find(|n| n.name == name).unwrap();
        let after = back.nodes.values().find(|n| n.name == name).unwrap();
        assert_eq!(after.motion, before.motion, "{name} motion");
        assert_eq!(after.fig_metadata, before.fig_metadata, "{name} metadata");
    }
}

fn synthetic_motion_message() -> Value {
    json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((1, 20), (0, 1), "FRAME", "Hero", json!({
            "opacity": 1.0,
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 10.0, "m10": 0.0, "m11": 1.0, "m12": 20.0},
            "timelineDefinitions": {"entries": [
                {
                    "id": g(3, 1),
                    "data": {
                        "durationUs": 1000000,
                        "defaultTimeline": true,
                        "playbackStyle": "LOOP",
                        "autoplay": true,
                        "keyframeSnappingFps": {"numerator": 60, "denominator": 1}
                    }
                }
            ]},
            "timelineAssignments": {"entries": [
                {
                    "key": {"assignedTimelineId": g(3, 1), "containingTimelineId": g(3, 1)},
                    "value": {"offsetUs": 0, "disabled": false, "autoplay": true}
                }
            ]},
            "parameterConsumptionMap": {"entries": [
                {
                    "nodeField": 18,
                    "variableField": "OPACITY",
                    "variableData": {
                        "value": {
                            "keyframeTrackParameterValue": {
                                "parameters": [{
                                    "type": "MANUAL",
                                    "value": {
                                        "manual": {
                                            "keyframeTrackId": {"guid": g(1, 21)},
                                            "timelineDefId": g(3, 1)
                                        }
                                    }
                                }]
                            }
                        },
                        "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
                        "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
                    }
                },
                {
                    "nodeField": 21,
                    "variableField": "MOTION_TRANSLATION_XY",
                    "variableData": {
                        "value": {
                            "keyframeTrackParameterValue": {
                                "parameters": [{
                                    "type": "MANUAL",
                                    "value": {
                                        "manual": {
                                            "keyframeTrackId": {"guid": g(1, 31)},
                                            "timelineDefId": g(3, 1)
                                        }
                                    }
                                }]
                            }
                        },
                        "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
                        "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
                    }
                }
            ]}
        })),
        node((1, 21), (1, 20), "KEYFRAME_TRACK", "OpacityTrack", json!({
            "keyframeOperation": "SET",
            "trackLoopMode": "NONE"
        })),
        node((1, 22), (1, 21), "KEYFRAME", "OpacityStart", json!({
            "timelinePosition": 0,
            "keyframeValue": {"value": {"floatValue": 0.2}, "valueType": "FLOAT"},
            "interpolationType": "HOLD",
            "interpolationColorSpace": "SRGB_LINEAR"
        })),
        node((1, 23), (1, 21), "KEYFRAME", "OpacityEnd", json!({
            "timelinePosition": 1000000,
            "keyframeValue": {"value": {"floatValue": 0.8}, "valueType": "FLOAT"},
            "interpolationType": "BEZIER",
            "bezierHandles": {"p1x": 0.42, "p1y": 0.0, "p2x": 0.58, "p2y": 1.0},
            "easingData": {
                "easingType": "CUSTOM_CUBIC",
                "easingValue": {"bezierEasing": {"p1x": 0.42, "p1y": 0.0, "p2x": 0.58, "p2y": 1.0}}
            },
            "interpolationColorSpace": "SRGB_LINEAR"
        })),
        node((1, 31), (1, 20), "KEYFRAME_TRACK", "TranslationTrack", json!({
            "keyframeOperation": "OFFSET",
            "trackLoopMode": "LOOP"
        })),
        node((1, 32), (1, 31), "KEYFRAME", "TransStart", json!({
            "timelinePosition": 0,
            "keyframeValue": {"value": {"vectorValue": {"x": 0.0, "y": 0.0}}, "valueType": "VECTOR"},
            "interpolationType": "HOLD",
            "interpolationColorSpace": "OKLCH"
        })),
        node((1, 33), (1, 31), "KEYFRAME", "TransEnd", json!({
            "timelinePosition": 1000000,
            "keyframeValue": {"value": {"vectorValue": {"x": 100.0, "y": 50.0}}, "valueType": "VECTOR"},
            "interpolationType": "SPRING",
            "bezierHandles": {"p1x": -0.25, "p1y": 0.0, "p2x": 0.25, "p2y": 1.0},
            "easingData": {
                "easingType": "CUSTOM_SPRING",
                "easingValue": {"springEasing": {"stiffness": 300.0, "damping": 20.0, "mass": 1.0, "springType": "PHYSICAL"}}
            },
            "interpolationColorSpace": "OKLCH"
        }))
    ]})
}

#[test]
fn synthetic_motion_message_round_trips_field_for_field() {
    let doc1 = import_fig_node_changes(&synthetic_motion_message(), "Motion").expect("import");
    let hero1 = doc1.nodes.values().find(|n| n.name == "Hero").expect("hero");
    let motion1 = hero1.motion.as_deref().expect("hero motion");

    assert!(
        !hero1.fig_metadata.contains_key("timelineDefinitions")
            && !hero1.fig_metadata.contains_key("timelineAssignments")
            && !hero1.fig_metadata.contains_key("parameterConsumptionMap"),
        "modelled motion fields are dropped from fig_metadata: {:?}",
        hero1.fig_metadata
    );
    assert!(
        !doc1.nodes.values().any(|n| matches!(&n.kind, NodeKind::Unknown { type_name: t } if t == "KEYFRAME_TRACK" || t == "KEYFRAME")),
        "modelled KEYFRAME_TRACK and KEYFRAME nodes are dropped from the node tree"
    );

    let mut want_timeline = Timeline::new("3:1", 1_000_000);
    want_timeline.is_default = true;
    want_timeline.playback = PlaybackStyle::Loop;
    want_timeline.autoplay = true;
    want_timeline.snap_fps = Some(FrameRate::new(60, 1));
    assert_eq!(motion1.timelines, vec![want_timeline]);
    assert_eq!(
        motion1.assignments,
        vec![TimelineAssignment { timeline: "3:1".into(), containing: "3:1".into(), offset_us: 0, disabled: false, autoplay: true }]
    );

    assert_eq!(motion1.tracks.len(), 2);
    let opacity_track = &motion1.tracks[0];
    assert_eq!(opacity_track.id, "1:21");
    assert_eq!(opacity_track.timeline, "3:1");
    assert_eq!(opacity_track.property, TrackProperty::Opacity);
    assert_eq!(opacity_track.operation, KeyframeOperation::Set);
    assert_eq!(opacity_track.loop_mode, TrackLoop::None);
    assert_eq!(opacity_track.keyframes.len(), 2);
    assert_eq!(opacity_track.keyframes[0].time_us, 0);
    assert_eq!(opacity_track.keyframes[0].value, KeyValue::Float(0.2));
    assert_eq!(opacity_track.keyframes[0].interpolation, InterpolationType::Hold);
    assert_eq!(opacity_track.keyframes[0].color_space, ColorSpace::SrgbLinear);
    assert_eq!(opacity_track.keyframes[1].time_us, 1_000_000);
    assert_eq!(opacity_track.keyframes[1].value, KeyValue::Float(0.8));
    assert_eq!(opacity_track.keyframes[1].interpolation, InterpolationType::Bezier);
    assert_eq!(opacity_track.keyframes[1].color_space, ColorSpace::SrgbLinear);
    assert_eq!(opacity_track.keyframes[1].bezier_handles, Some([0.42, 0.0, 0.58, 1.0]));
    assert_eq!(opacity_track.keyframes[1].easing, Some(KeyEasing { easing_type: EasingType::CustomCubic, bezier: Some([0.42, 0.0, 0.58, 1.0]), spring: None }));

    let trans_track = &motion1.tracks[1];
    assert_eq!(trans_track.id, "1:31");
    assert_eq!(trans_track.timeline, "3:1");
    assert_eq!(trans_track.property, TrackProperty::TranslationXy);
    assert_eq!(trans_track.operation, KeyframeOperation::Offset);
    assert_eq!(trans_track.loop_mode, TrackLoop::Loop);
    assert_eq!(trans_track.keyframes.len(), 2);
    assert_eq!(trans_track.keyframes[0].time_us, 0);
    assert_eq!(trans_track.keyframes[0].value, KeyValue::Vector(Vec2::new(0.0, 0.0)));
    assert_eq!(trans_track.keyframes[0].color_space, ColorSpace::Oklch);
    assert_eq!(trans_track.keyframes[1].time_us, 1_000_000);
    assert_eq!(trans_track.keyframes[1].value, KeyValue::Vector(Vec2::new(100.0, 50.0)));
    assert_eq!(trans_track.keyframes[1].interpolation, InterpolationType::Spring);
    assert_eq!(trans_track.keyframes[1].bezier_handles, Some([-0.25, 0.0, 0.25, 1.0]));
    assert_eq!(trans_track.keyframes[1].color_space, ColorSpace::Oklch);
    assert_eq!(
        trans_track.keyframes[1].easing,
        Some(KeyEasing {
            easing_type: EasingType::CustomSpring,
            bezier: None,
            spring: Some(SpringParams { stiffness: 300.0, damping: 20.0, mass: 1.0, normalized: false }),
        })
    );

    let bytes1 = export_fig(&doc1).expect("export 1");
    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let hero2 = doc2.nodes.values().find(|n| n.name == "Hero").expect("hero 2");
    assert_eq!(hero2.motion, hero1.motion, "import -> export -> import is identical field-for-field");

    let bytes2 = export_fig(&doc2).expect("export 2");
    let doc3 = import_fig(&bytes2).expect("re-import 2");
    let hero3 = doc3.nodes.values().find(|n| n.name == "Hero").expect("hero 3");
    assert_eq!(hero3.motion, hero1.motion, "second round-trip remains identical");
}

#[test]
fn evaluate_motion_on_imported_doc_gives_expected_mid_time_values() {
    let bytes = export_fig(&import_fig_node_changes(&synthetic_motion_message(), "Motion").expect("seed")).expect("export");
    let doc = import_fig(&bytes).expect("import .fig");
    let hero = doc.nodes.values().find(|n| n.name == "Hero").expect("hero");

    let overrides = evaluate_motion(&doc, 500_000);
    assert_eq!(overrides.len(), 2);

    let opacity = overrides.iter().find(|o| o.property == TrackProperty::Opacity).expect("opacity override");
    assert_eq!(opacity.node, hero.id);
    assert_eq!(opacity.timeline, "3:1");
    assert_eq!(opacity.operation, KeyframeOperation::Set);
    match (opacity.sampled.clone(), opacity.value.clone()) {
        (KeyValue::Float(sampled), KeyValue::Float(value)) => {
            assert!((sampled - 0.5).abs() < 1e-4, "expected mid opacity 0.5 on symmetric cubic bezier, got {sampled}");
            assert!((value - 0.5).abs() < 1e-4, "expected Set opacity 0.5, got {value}");
        }
        other => panic!("expected Float override, got {other:?}"),
    }

    let trans = overrides.iter().find(|o| o.property == TrackProperty::TranslationXy).expect("translation override");
    assert_eq!(trans.node, hero.id);
    assert_eq!(trans.timeline, "3:1");
    assert_eq!(trans.operation, KeyframeOperation::Offset);
    match (&trans.sampled, &trans.value) {
        (KeyValue::Vector(sampled), KeyValue::Vector(value)) => {
            assert!(sampled.x > 80.0 && sampled.x < 105.0, "expected spring x near settled target at 0.5s, got {}", sampled.x);
            assert!(sampled.y > 40.0 && sampled.y < 52.5, "expected spring y near settled target at 0.5s, got {}", sampled.y);
            assert!((value.x - (10.0 + sampled.x)).abs() < 1e-4, "Offset adds base tx=10.0");
            assert!((value.y - (20.0 + sampled.y)).abs() < 1e-4, "Offset adds base ty=20.0");
        }
        other => panic!("expected Vector override, got {other:?}"),
    }
}

#[test]
fn unknown_track_fields_and_unknown_track_properties_survive_the_round_trip() {
    let mut msg = synthetic_motion_message();
    let changes = msg["nodeChanges"].as_array_mut().unwrap();
    // Add an unmodelled field (`timelineDuration` + `timelineOffset`) on the modelled KEYFRAME_TRACK node,
    // and add a third track bound to an unknown VariableField property with its own unmodelled field.
    let track = changes.iter_mut().find(|n| n["name"] == "OpacityTrack").unwrap();
    track["timelineDuration"] = json!(750000);
    track["timelineOffset"] = json!(-50000);

    let hero = changes.iter_mut().find(|n| n["name"] == "Hero").unwrap();
    hero["parameterConsumptionMap"]["entries"].as_array_mut().unwrap().push(json!({
        "nodeField": 99,
        "variableField": "DISSOLVE_PROGRESS",
        "variableData": {
            "value": {
                "keyframeTrackParameterValue": {
                    "parameters": [{
                        "type": "MANUAL",
                        "value": {
                            "manual": {
                                "keyframeTrackId": {"guid": g(1, 41)},
                                "timelineDefId": g(3, 1)
                            }
                        }
                    }]
                }
            },
            "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
        }
    }));
    changes.push(node(
        (1, 41),
        (1, 20),
        "KEYFRAME_TRACK",
        "FutureTrack",
        json!({
            "keyframeOperation": "SCALE",
            "trackLoopMode": "NONE",
            "timelineDuration": 333000
        }),
    ));
    changes.push(node(
        (1, 42),
        (1, 41),
        "KEYFRAME",
        "FutureKey",
        json!({
            "timelinePosition": 250000,
            "keyframeValue": {"value": {"floatValue": 2.5}, "valueType": "FLOAT"},
            "interpolationType": "HOLD",
            "interpolationColorSpace": "SRGB_LINEAR"
        }),
    ));

    let doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let hero1 = doc1.nodes.values().find(|n| n.name == "Hero").expect("hero 1");
    let motion1 = hero1.motion.as_deref().expect("motion 1");
    assert_eq!(motion1.tracks.len(), 3);
    assert_eq!(motion1.tracks[2].property, TrackProperty::Other("DISSOLVE_PROGRESS".into()));

    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");
    let opacity_nc = changes1.iter().find(|n| n["guid"] == g(1, 21)).expect("OpacityTrack NodeChange in exported .fig");
    assert_eq!(opacity_nc["timelineDuration"], json!(750000));
    assert_eq!(opacity_nc["timelineOffset"], json!(-50000));
    let future_nc = changes1.iter().find(|n| n["guid"] == g(1, 41)).expect("FutureTrack NodeChange in exported .fig");
    assert_eq!(future_nc["timelineDuration"], json!(333000));

    let doc2 = import_fig(&bytes1).expect("import 2");
    let bytes2 = export_fig(&doc2).expect("export 2");
    let decoded2 = decode_fig_to_json(&bytes2).expect("decode 2");
    let changes2 = decoded2["message"]["nodeChanges"].as_array().expect("nodeChanges 2");
    let opacity_nc2 = changes2.iter().find(|n| n["guid"] == g(1, 21)).expect("OpacityTrack NodeChange after second export");
    assert_eq!(opacity_nc2["timelineDuration"], json!(750000));
    assert_eq!(opacity_nc2["timelineOffset"], json!(-50000));
    let future_nc2 = changes2.iter().find(|n| n["guid"] == g(1, 41)).expect("FutureTrack NodeChange after second export");
    assert_eq!(future_nc2["timelineDuration"], json!(333000));
}

#[test]
fn renamed_motion_ids_map_to_unique_kiwi_guids_and_stay_stable_across_save_and_reopen() {
    let mut doc = import_fig_node_changes(&synthetic_motion_message(), "Motion").expect("import");
    let hero_id = doc.nodes.values().find(|n| n.name == "Hero").unwrap().id;

    // Simulate paste/duplicate suffixes (`3:1~1`, `3:1~2`, `1:21~1`, `1:21~2`) that are not `<u32>:<u32>`.
    let mut tl1 = Timeline::new("3:1~1", 1_000_000);
    tl1.is_default = true;
    let mut tl2 = Timeline::new("3:1~2", 500_000);
    tl2.parent = Some("3:1~1".into());

    let kiwi_tls = timelines_from_kiwi(Some(&timelines_to_kiwi(&[tl1.clone(), tl2.clone()])));
    assert_eq!(kiwi_tls.len(), 2);
    assert_ne!(kiwi_tls[0].id, "0:0");
    assert_ne!(kiwi_tls[1].id, "0:0");
    assert_ne!(kiwi_tls[0].id, kiwi_tls[1].id);
    assert_eq!(kiwi_tls[1].parent.as_deref(), Some(kiwi_tls[0].id.as_str()));

    let mut tr1 = Track::new("1:21~1", "3:1~1", TrackProperty::Opacity);
    tr1.keyframes.push(Keyframe::new(0, KeyValue::Float(0.1)));
    let mut tr2 = Track::new("1:21~2", "3:1~2", TrackProperty::Rotation);
    tr2.keyframes.push(Keyframe::new(100_000, KeyValue::Float(45.0)));

    doc.nodes.get_mut(&hero_id).unwrap().motion = Some(Box::new(NodeMotion {
        timelines: vec![tl1, tl2],
        assignments: vec![
            TimelineAssignment { timeline: "3:1~1".into(), containing: "3:1~1".into(), offset_us: 0, disabled: false, autoplay: true },
            TimelineAssignment { timeline: "3:1~2".into(), containing: "3:1~1".into(), offset_us: 100_000, disabled: false, autoplay: false },
        ],
        tracks: vec![tr1, tr2],
    }));

    let bytes1 = export_fig(&doc).expect("export 1");
    let reopened1 = import_fig(&bytes1).expect("reopen 1");
    let motion1 = reopened1.nodes.values().find(|n| n.name == "Hero").unwrap().motion.as_deref().unwrap();

    assert_eq!(motion1.timelines.len(), 2);
    assert_ne!(motion1.timelines[0].id, "0:0", "renamed timeline id must not collapse to 0:0");
    assert_ne!(motion1.timelines[1].id, "0:0", "second renamed timeline id must not collapse to 0:0");
    assert_ne!(motion1.timelines[0].id, motion1.timelines[1].id, "renamed timeline ids must get distinct GUIDs");
    assert_eq!(motion1.timelines[1].parent.as_deref(), Some(motion1.timelines[0].id.as_str()));

    assert_eq!(motion1.tracks.len(), 2);
    assert_ne!(motion1.tracks[0].id, "0:0");
    assert_ne!(motion1.tracks[1].id, "0:0");
    assert_ne!(motion1.tracks[0].id, motion1.tracks[1].id, "renamed track ids must get distinct GUIDs");
    assert_eq!(motion1.tracks[0].timeline, motion1.timelines[0].id);
    assert_eq!(motion1.tracks[1].timeline, motion1.timelines[1].id);

    // Save and reopen again: the allocated GUIDs are now canonical `<session>:<local>` strings and must stay identical.
    let bytes2 = export_fig(&reopened1).expect("export 2");
    let reopened2 = import_fig(&bytes2).expect("reopen 2");
    let motion2 = reopened2.nodes.values().find(|n| n.name == "Hero").unwrap().motion.as_deref().unwrap();
    assert_eq!(motion2, motion1, "GUID mapping stays stable through save and reopen");
}

#[test]
fn fig_without_motion_is_byte_identical_across_export() {
    let raw = testkit::sample_fig(designos_fig::container::Compression::Deflate, false);
    let doc = import_fig(&raw).expect("import sample_fig");
    assert!(doc.nodes.values().all(|n| n.motion.is_none()), "sample_fig has no motion");

    let bytes1 = export_fig(&doc).expect("export 1");
    let doc2 = import_fig(&bytes1).expect("re-import");
    let bytes2 = export_fig(&doc2).expect("export 2");
    assert_eq!(bytes1, bytes2, "a .fig without Motion exports byte-identically");

    let decoded: Value = decode_fig_to_json(&bytes1).expect("decode");
    for nc in decoded["message"]["nodeChanges"].as_array().unwrap() {
        assert!(nc.get("timelineDefinitions").is_none());
        assert!(nc.get("timelineAssignments").is_none());
        assert!(nc.get("parameterConsumptionMap").is_none());
        assert_ne!(nc["type"], "KEYFRAME_TRACK");
        assert_ne!(nc["type"], "KEYFRAME");
    }
}

#[test]
fn unmodelled_children_of_bound_keyframe_track_survive_round_trip() {
    let msg = json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((1, 20), (0, 1), "FRAME", "Hero", json!({
            "timelineDefinitions": {"entries": [
                {"id": g(3, 1), "data": {"durationUs": 1000000, "defaultTimeline": true}}
            ]},
            "parameterConsumptionMap": {"entries": [
                {
                    "nodeField": 18,
                    "variableField": "OPACITY",
                    "variableData": {
                        "value": {
                            "keyframeTrackParameterValue": {
                                "parameters": [{
                                    "type": "MANUAL",
                                    "value": {
                                        "manual": {
                                            "keyframeTrackId": {"guid": g(1, 21)},
                                            "timelineDefId": g(3, 1)
                                        }
                                    }
                                }]
                            }
                        },
                        "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
                        "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
                    }
                }
            ]}
        })),
        node((1, 21), (1, 20), "KEYFRAME_TRACK", "OpacityTrack", json!({
            "keyframeOperation": "SET",
            "trackLoopMode": "NONE"
        })),
        node((1, 22), (1, 21), "KEYFRAME", "ValuelessKey", json!({
            "timelinePosition": 0,
            "pluginData": [{"pluginID": "p1", "key": "k1", "value": "v1"}]
        })),
        node((1, 23), (1, 21), "ANIMATION_PRESET_INSTANCE", "TrackPresetChild", json!({
            "timelineDuration": 400000,
            "pluginData": [{"pluginID": "p2", "key": "k2", "value": "v2"}]
        })),
        node((1, 24), (1, 21), "KEYFRAME", "ValidKey", json!({
            "timelinePosition": 500000,
            "keyframeValue": {"value": {"floatValue": 0.5}, "valueType": "FLOAT"},
            "interpolationType": "HOLD"
        }))
    ]});

    let doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");

    let kf_valueless1 = changes1.iter().find(|n| n["guid"] == g(1, 22)).expect("valueless KEYFRAME 1:22 must survive export");
    assert_eq!(kf_valueless1["type"], "KEYFRAME");
    assert_eq!(kf_valueless1["name"], "ValuelessKey");
    assert_eq!(kf_valueless1["parentIndex"]["guid"], g(1, 21));
    assert_eq!(kf_valueless1["timelinePosition"], json!(0));
    assert!(kf_valueless1.get("keyframeValue").is_none());
    assert_eq!(kf_valueless1["pluginData"], json!([{"pluginID": "p1", "key": "k1", "value": "v1"}]));

    let preset_child1 = changes1.iter().find(|n| n["guid"] == g(1, 23)).expect("ANIMATION_PRESET_INSTANCE 1:23 must survive export");
    assert_eq!(preset_child1["type"], "ANIMATION_PRESET_INSTANCE");
    assert_eq!(preset_child1["name"], "TrackPresetChild");
    assert_eq!(preset_child1["parentIndex"]["guid"], g(1, 21));
    assert_eq!(preset_child1["timelineDuration"], json!(400000));
    assert_eq!(preset_child1["pluginData"], json!([{"pluginID": "p2", "key": "k2", "value": "v2"}]));

    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let hero1 = doc1.nodes.values().find(|n| n.name == "Hero").unwrap();
    let hero2 = doc2.nodes.values().find(|n| n.name == "Hero").unwrap();
    assert_eq!(hero2.motion, hero1.motion);
    assert_eq!(hero2.fig_metadata, hero1.fig_metadata);

    let bytes2 = export_fig(&doc2).expect("export 2");
    let decoded2 = decode_fig_to_json(&bytes2).expect("decode 2");
    let changes2 = decoded2["message"]["nodeChanges"].as_array().expect("nodeChanges 2");
    assert_eq!(changes2.iter().find(|n| n["guid"] == g(1, 22)), Some(kf_valueless1));
    assert_eq!(changes2.iter().find(|n| n["guid"] == g(1, 23)), Some(preset_child1));
}

#[test]
fn track_binding_without_timeline_def_id_keeps_empty_timeline_and_shape() {
    let binding_entry = json!({
        "nodeField": 18,
        "variableField": "OPACITY",
        "variableData": {
            "dataType": "KEYFRAME_TRACK_ID",
            "resolvedDataType": "KEYFRAME_TRACK_ID",
            "value": {
                "keyframeTrackIdValue": {
                    "guid": {"sessionID": 1, "localID": 21}
                }
            }
        }
    });
    let msg = json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((1, 10), (0, 1), "FRAME", "Earlier", json!({
            "timelineDefinitions": {"entries": [
                {"id": g(5, 9), "data": {"durationUs": 900000, "defaultTimeline": true}}
            ]}
        })),
        node((3, 1), (0, 1), "ROUNDED_RECTANGLE", "ExistingThreeOne", json!({})),
        node((1, 20), (0, 1), "FRAME", "Hero", json!({
            "parameterConsumptionMap": {"entries": [binding_entry.clone()]}
        })),
        node((1, 21), (1, 20), "KEYFRAME_TRACK", "OpacityTrack", json!({
            "keyframeOperation": "SET",
            "trackLoopMode": "NONE"
        })),
        node((1, 22), (1, 21), "KEYFRAME", "OpacityKey", json!({
            "timelinePosition": 0,
            "keyframeValue": {"value": {"floatValue": 0.4}, "valueType": "FLOAT"},
            "interpolationType": "HOLD"
        }))
    ]});

    let doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let hero1 = doc1.nodes.values().find(|n| n.name == "Hero").expect("hero 1");
    let motion1 = hero1.motion.as_deref().expect("hero motion 1");
    assert_eq!(motion1.tracks.len(), 1);
    assert_eq!(motion1.tracks[0].timeline, "", "binding without timelineDefId must keep an empty timeline id");

    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");
    let hero_nc1 = changes1.iter().find(|n| n["guid"] == g(1, 20)).expect("Hero NodeChange");
    assert_eq!(hero_nc1["parameterConsumptionMap"]["entries"], json!([binding_entry]), "export must keep KEYFRAME_TRACK_ID shape and omit timelineDefId");

    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let hero2 = doc2.nodes.values().find(|n| n.name == "Hero").expect("hero 2");
    assert_eq!(hero2.motion, hero1.motion);
    assert_eq!(hero2.fig_metadata, hero1.fig_metadata);

    // Also verify a non-empty non-canonical timeline id when guid 3:1 is already in used_guids never collides with 3:1.
    let mut doc_collide = doc2.clone();
    let hero_id = hero2.id;
    doc_collide.nodes.get_mut(&hero_id).unwrap().motion.as_mut().unwrap().tracks[0].timeline = "3:1~dup".into();
    doc_collide.nodes.get_mut(&hero_id).unwrap().fig_metadata.remove("_motionTrackMeta");
    let bytes_collide = export_fig(&doc_collide).expect("export collide");
    let decoded_collide = decode_fig_to_json(&bytes_collide).expect("decode collide");
    let hero_collide = decoded_collide["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["guid"] == g(1, 20)).unwrap();
    let written_tl_guid = &hero_collide["parameterConsumptionMap"]["entries"][0]["variableData"]["value"]["keyframeTrackParameterValue"]["parameters"][0]
        ["value"]["manual"]["timelineDefId"];
    assert_ne!(written_tl_guid, &g(3, 1), "timeline_guid must never collide with an existing 3:1 guid in used_guids");
}

#[test]
fn mixed_variable_and_preset_parameter_consumption_map_entries_survive_round_trip() {
    let alias_entry = json!({
        "nodeField": 18,
        "variableField": "OPACITY",
        "variableData": {
            "dataType": "ALIAS",
            "resolvedDataType": "FLOAT",
            "value": {
                "alias": {"guid": g(1, 10)}
            }
        }
    });
    let preset_entry = json!({
        "nodeField": 21,
        "variableField": "MOTION_TRANSLATION_XY",
        "variableData": {
            "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "value": {
                "keyframeTrackParameterValue": {
                    "parameters": [{
                        "type": "ANIMATION_PRESET",
                        "value": {
                            "animationPreset": {
                                "animationPresetId": {"guid": g(3, 99)},
                                "timelineDefId": g(3, 1)
                            }
                        }
                    }]
                }
            }
        }
    });
    let manual_entry = json!({
        "nodeField": 25,
        "variableField": "CORNER_RADIUS",
        "variableData": {
            "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "value": {
                "keyframeTrackParameterValue": {
                    "parameters": [{
                        "type": "MANUAL",
                        "value": {
                            "manual": {
                                "keyframeTrackId": {"guid": g(1, 21)},
                                "timelineDefId": g(3, 1)
                            }
                        }
                    }]
                }
            }
        }
    });

    let msg = json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((0, 999_999), (0, 0), "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        node((1, 8), (0, 999_999), "VARIABLE_SET", "Tokens", json!({
            "variableSetModes": [{"id": g(1, 9), "name": "Mode 1", "sortPosition": "!"}]
        })),
        node((1, 10), (0, 999_999), "VARIABLE", "opacity-token", json!({
            "variableSetID": {"guid": g(1, 8)},
            "variableResolvedType": "FLOAT",
            "variableDataValues": {"entries": [
                {"modeID": g(1, 9), "variableData": {"dataType": "FLOAT", "value": {"floatValue": 0.75}}}
            ]}
        })),
        node((1, 20), (0, 1), "FRAME", "Hero", json!({
            "parameterConsumptionMap": {"entries": [
                alias_entry.clone(),
                preset_entry.clone(),
                manual_entry.clone()
            ]}
        })),
        node((1, 21), (1, 20), "KEYFRAME_TRACK", "RadiusTrack", json!({
            "keyframeOperation": "SET",
            "trackLoopMode": "NONE"
        })),
        node((1, 22), (1, 21), "KEYFRAME", "RadiusKey", json!({
            "timelinePosition": 0,
            "keyframeValue": {"value": {"floatValue": 8.0}, "valueType": "FLOAT"},
            "interpolationType": "HOLD"
        }))
    ]});

    let doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let hero1 = doc1.nodes.values().find(|n| n.name == "Hero").unwrap();
    assert_eq!(hero1.variables.bindings.get("opacity").map(String::as_str), Some("VariableID:1:10"));

    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");
    let hero_nc1 = changes1.iter().find(|n| n["guid"] == g(1, 20)).expect("Hero NodeChange");
    assert!(hero_nc1.get("variableConsumptionMap").is_none(), "OPACITY from parameterConsumptionMap must not duplicate into variableConsumptionMap");
    let entries1 = hero_nc1["parameterConsumptionMap"]["entries"].as_array().expect("parameterConsumptionMap entries");
    assert!(entries1.contains(&alias_entry), "ALIAS entry must survive in parameterConsumptionMap: {entries1:?}");
    assert!(entries1.contains(&preset_entry), "ANIMATION_PRESET entry must survive in parameterConsumptionMap: {entries1:?}");
    assert!(entries1.contains(&manual_entry), "MANUAL track entry must survive in parameterConsumptionMap: {entries1:?}");

    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let hero2 = doc2.nodes.values().find(|n| n.name == "Hero").unwrap();
    assert_eq!(hero2.variables, hero1.variables);
    assert_eq!(hero2.motion, hero1.motion);
    assert_eq!(hero2.fig_metadata, hero1.fig_metadata);
    assert_eq!(hero2, hero1);
    assert_eq!(doc2.variables, doc1.variables);
    assert_eq!(doc2.variable_collections, doc1.variable_collections);
}

#[test]
fn bound_track_and_keyframe_preserve_names_guids_and_plugin_data() {
    let mut msg = synthetic_motion_message();
    let changes = msg["nodeChanges"].as_array_mut().unwrap();
    let track = changes.iter_mut().find(|n| n["name"] == "OpacityTrack").unwrap();
    track["pluginData"] = json!([{"pluginID": "plug_tr", "key": "tk", "value": "tv"}]);
    let kf = changes.iter_mut().find(|n| n["name"] == "OpacityStart").unwrap();
    kf["pluginData"] = json!([{"pluginID": "plug_kf", "key": "kk", "value": "kv"}]);

    let doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");

    let track_nc = changes1.iter().find(|n| n["guid"] == g(1, 21)).expect("track 1:21");
    assert_eq!(track_nc["name"], "OpacityTrack");
    assert_eq!(track_nc["pluginData"], json!([{"pluginID": "plug_tr", "key": "tk", "value": "tv"}]));

    let kf_start_nc = changes1.iter().find(|n| n["guid"] == g(1, 22)).expect("keyframe 1:22");
    assert_eq!(kf_start_nc["name"], "OpacityStart");
    assert_eq!(kf_start_nc["pluginData"], json!([{"pluginID": "plug_kf", "key": "kk", "value": "kv"}]));

    let kf_end_nc = changes1.iter().find(|n| n["guid"] == g(1, 23)).expect("keyframe 1:23");
    assert_eq!(kf_end_nc["name"], "OpacityEnd");

    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let hero1 = doc1.nodes.values().find(|n| n.name == "Hero").unwrap();
    let hero2 = doc2.nodes.values().find(|n| n.name == "Hero").unwrap();
    assert_eq!(hero2.motion, hero1.motion);
    assert_eq!(hero2.fig_metadata, hero1.fig_metadata);
}

#[test]
fn unmodelled_timeline_and_assignment_entries_without_guids_survive_round_trip() {
    let valid_tl = json!({"id": g(3, 1), "data": {"durationUs": 1000000, "defaultTimeline": true}});
    let nameless_tl = json!({"data": {"durationUs": 500000}});
    let valid_assign = json!({
        "key": {"assignedTimelineId": g(3, 1), "containingTimelineId": g(3, 1)},
        "value": {"offsetUs": 0, "disabled": false}
    });
    let incomplete_assign = json!({
        "key": {"assignedTimelineId": g(3, 1)},
        "value": {"offsetUs": 250000, "disabled": true}
    });

    let msg = json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((1, 20), (0, 1), "FRAME", "Hero", json!({
            "timelineDefinitions": {"entries": [valid_tl.clone(), nameless_tl.clone()]},
            "timelineAssignments": {"entries": [valid_assign.clone(), incomplete_assign.clone()]}
        }))
    ]});

    let doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");
    let hero_nc1 = changes1.iter().find(|n| n["guid"] == g(1, 20)).expect("Hero NodeChange");

    assert_eq!(hero_nc1["timelineDefinitions"]["entries"], json!([valid_tl, nameless_tl]));
    assert_eq!(hero_nc1["timelineAssignments"]["entries"], json!([valid_assign, incomplete_assign]));

    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let hero1 = doc1.nodes.values().find(|n| n.name == "Hero").unwrap();
    let hero2 = doc2.nodes.values().find(|n| n.name == "Hero").unwrap();
    assert_eq!(hero2.motion, hero1.motion);
    assert_eq!(hero2.fig_metadata, hero1.fig_metadata);
}

#[test]
fn two_bindings_to_one_track_share_single_exported_track_node() {
    let msg = json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((1, 20), (0, 1), "FRAME", "Hero", json!({
            "timelineDefinitions": {"entries": [
                {"id": g(3, 1), "data": {"durationUs": 1000000, "defaultTimeline": true}}
            ]},
            "parameterConsumptionMap": {"entries": [
                {
                    "nodeField": 16,
                    "variableField": "WIDTH",
                    "variableData": {
                        "value": {
                            "keyframeTrackParameterValue": {
                                "parameters": [{
                                    "type": "MANUAL",
                                    "value": {
                                        "manual": {
                                            "keyframeTrackId": {"guid": g(1, 21)},
                                            "timelineDefId": g(3, 1)
                                        }
                                    }
                                }]
                            }
                        },
                        "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
                        "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
                    }
                },
                {
                    "nodeField": 16,
                    "variableField": "HEIGHT",
                    "variableData": {
                        "value": {
                            "keyframeTrackParameterValue": {
                                "parameters": [{
                                    "type": "MANUAL",
                                    "value": {
                                        "manual": {
                                            "keyframeTrackId": {"guid": g(1, 21)},
                                            "timelineDefId": g(3, 1)
                                        }
                                    }
                                }]
                            }
                        },
                        "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
                        "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
                    }
                }
            ]}
        })),
        node((1, 21), (1, 20), "KEYFRAME_TRACK", "SharedTrack", json!({
            "keyframeOperation": "SET",
            "trackLoopMode": "NONE"
        })),
        node((1, 22), (1, 21), "KEYFRAME", "SharedKey", json!({
            "timelinePosition": 0,
            "keyframeValue": {"value": {"floatValue": 120.0}, "valueType": "FLOAT"},
            "interpolationType": "HOLD"
        }))
    ]});

    let doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");

    let tracks: Vec<&Value> = changes1.iter().filter(|n| n["type"] == "KEYFRAME_TRACK").collect();
    assert_eq!(tracks.len(), 1, "shared track 1:21 must be emitted only once");
    assert_eq!(tracks[0]["guid"], g(1, 21));
    let kfs: Vec<&Value> = changes1.iter().filter(|n| n["type"] == "KEYFRAME").collect();
    assert_eq!(kfs.len(), 1, "shared track keyframes must be emitted only once");

    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let hero1 = doc1.nodes.values().find(|n| n.name == "Hero").unwrap();
    let hero2 = doc2.nodes.values().find(|n| n.name == "Hero").unwrap();
    assert_eq!(hero2.motion, hero1.motion);
}

#[test]
fn shared_track_passthrough_survives_when_file_order_differs_from_tree_order() {
    let manual_binding = json!({
        "nodeField": 18,
        "variableField": "OPACITY",
        "variableData": {
            "value": {
                "keyframeTrackParameterValue": {
                    "parameters": [{
                        "type": "MANUAL",
                        "value": {
                            "manual": {
                                "keyframeTrackId": {"guid": g(1, 21)},
                                "timelineDefId": g(3, 1)
                            }
                        }
                    }]
                }
            },
            "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
        }
    });

    // In nodeChanges, Beta (1:20, position "\"") comes before Alpha (1:10, position "!").
    // In tree order, Alpha comes before Beta. Both bind KEYFRAME_TRACK 1:21 (parent Beta),
    // which has child ANIMATION_PRESET_INSTANCE 1:23.
    let msg = json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((1, 20), (0, 1), "FRAME", "Beta", json!({
            "parentIndex": {"guid": g(0, 1), "position": "\""},
            "timelineDefinitions": {"entries": [
                {"id": g(3, 1), "data": {"durationUs": 1000000, "defaultTimeline": true}}
            ]},
            "parameterConsumptionMap": {"entries": [manual_binding.clone()]}
        })),
        node((1, 10), (0, 1), "FRAME", "Alpha", json!({
            "parentIndex": {"guid": g(0, 1), "position": "!"},
            "parameterConsumptionMap": {"entries": [manual_binding]}
        })),
        node((1, 21), (1, 20), "KEYFRAME_TRACK", "OpacityTrack", json!({
            "keyframeOperation": "SET",
            "trackLoopMode": "NONE"
        })),
        node((1, 23), (1, 21), "ANIMATION_PRESET_INSTANCE", "TrackPresetChild", json!({
            "timelineDuration": 400000,
            "pluginData": [{"pluginID": "p2", "key": "k2", "value": "v2"}]
        })),
        node((1, 24), (1, 21), "KEYFRAME", "ValidKey", json!({
            "timelinePosition": 500000,
            "keyframeValue": {"value": {"floatValue": 0.5}, "valueType": "FLOAT"},
            "interpolationType": "HOLD"
        }))
    ]});

    let doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");

    let preset_child1 =
        changes1.iter().find(|n| n["guid"] == g(1, 23)).expect("ANIMATION_PRESET_INSTANCE 1:23 must survive export when file order differs from tree order");
    assert_eq!(preset_child1["type"], "ANIMATION_PRESET_INSTANCE");
    assert_eq!(preset_child1["name"], "TrackPresetChild");
    assert_eq!(preset_child1["parentIndex"]["guid"], g(1, 21));
    assert_eq!(preset_child1["timelineDuration"], json!(400000));
    assert_eq!(preset_child1["pluginData"], json!([{"pluginID": "p2", "key": "k2", "value": "v2"}]));

    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let alpha1 = doc1.nodes.values().find(|n| n.name == "Alpha").unwrap();
    let alpha2 = doc2.nodes.values().find(|n| n.name == "Alpha").unwrap();
    let beta1 = doc1.nodes.values().find(|n| n.name == "Beta").unwrap();
    let beta2 = doc2.nodes.values().find(|n| n.name == "Beta").unwrap();
    assert_eq!(alpha2.motion, alpha1.motion);
    assert_eq!(beta2.motion, beta1.motion);
    assert_eq!(alpha2.fig_metadata, alpha1.fig_metadata);
    assert_eq!(beta2.fig_metadata, beta1.fig_metadata);

    let bytes2 = export_fig(&doc2).expect("export 2");
    let decoded2 = decode_fig_to_json(&bytes2).expect("decode 2");
    let changes2 = decoded2["message"]["nodeChanges"].as_array().expect("nodeChanges 2");
    assert_eq!(changes2.iter().find(|n| n["guid"] == g(1, 23)), Some(preset_child1));
}

#[test]
fn edited_variable_binding_from_parameter_consumption_map_survives_round_trip() {
    let alias_entry = json!({
        "nodeField": 18,
        "variableField": "OPACITY",
        "variableData": {
            "dataType": "ALIAS",
            "resolvedDataType": "FLOAT",
            "value": {
                "alias": {"guid": g(1, 10)}
            }
        }
    });
    let font_style_entry = json!({
        "nodeField": 42,
        "variableField": "FONT_STYLE",
        "variableData": {
            "dataType": "FONT_STYLE",
            "resolvedDataType": "FONT_STYLE",
            "value": {
                "fontStyleValue": {
                    "asString": {
                        "dataType": "ALIAS",
                        "resolvedDataType": "STRING",
                        "value": {"alias": {"guid": g(1, 14)}}
                    }
                }
            }
        }
    });
    let preset_entry = json!({
        "nodeField": 21,
        "variableField": "MOTION_TRANSLATION_XY",
        "variableData": {
            "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "value": {
                "keyframeTrackParameterValue": {
                    "parameters": [{
                        "type": "ANIMATION_PRESET",
                        "value": {
                            "animationPreset": {
                                "animationPresetId": {"guid": g(3, 99)},
                                "timelineDefId": g(3, 1)
                            }
                        }
                    }]
                }
            }
        }
    });
    let manual_entry = json!({
        "nodeField": 25,
        "variableField": "CORNER_RADIUS",
        "variableData": {
            "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA",
            "value": {
                "keyframeTrackParameterValue": {
                    "parameters": [{
                        "type": "MANUAL",
                        "value": {
                            "manual": {
                                "keyframeTrackId": {"guid": g(1, 21)},
                                "timelineDefId": g(3, 1)
                            }
                        }
                    }]
                }
            }
        }
    });

    let msg = json!({"nodeChanges": [
        node((0, 0), (0, 0), "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page", json!({})),
        node((0, 999_999), (0, 0), "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        node((1, 8), (0, 999_999), "VARIABLE_SET", "Tokens", json!({
            "variableSetModes": [{"id": g(1, 9), "name": "Mode 1", "sortPosition": "!"}]
        })),
        node((1, 10), (0, 999_999), "VARIABLE", "opacity-token", json!({
            "variableSetID": {"guid": g(1, 8)},
            "variableResolvedType": "FLOAT",
            "variableDataValues": {"entries": [
                {"modeID": g(1, 9), "variableData": {"dataType": "FLOAT", "value": {"floatValue": 0.75}}}
            ]}
        })),
        node((1, 99), (0, 999_999), "VARIABLE", "opacity-token-2", json!({
            "variableSetID": {"guid": g(1, 8)},
            "variableResolvedType": "FLOAT",
            "variableDataValues": {"entries": [
                {"modeID": g(1, 9), "variableData": {"dataType": "FLOAT", "value": {"floatValue": 0.25}}}
            ]}
        })),
        node((1, 20), (0, 1), "FRAME", "Hero", json!({
            "parameterConsumptionMap": {"entries": [
                alias_entry,
                font_style_entry,
                preset_entry.clone(),
                manual_entry.clone()
            ]}
        })),
        node((1, 21), (1, 20), "KEYFRAME_TRACK", "RadiusTrack", json!({
            "keyframeOperation": "SET",
            "trackLoopMode": "NONE"
        })),
        node((1, 22), (1, 21), "KEYFRAME", "RadiusKey", json!({
            "timelinePosition": 0,
            "keyframeValue": {"value": {"floatValue": 8.0}, "valueType": "FLOAT"},
            "interpolationType": "HOLD"
        }))
    ]});

    let mut doc1 = import_fig_node_changes(&msg, "Motion").expect("import 1");
    let hero_id = doc1.nodes.values().find(|n| n.name == "Hero").unwrap().id;
    assert_eq!(doc1.nodes[&hero_id].variables.bindings.get("opacity").map(String::as_str), Some("VariableID:1:10"));
    assert_eq!(doc1.nodes[&hero_id].variables.bindings.get("fontStyle").map(String::as_str), Some("VariableID:1:14"));
    if let Some(raw_pcm) = doc1.nodes[&hero_id].fig_metadata.get("parameterConsumptionMap") {
        let pcm_val: Value = serde_json::from_str(raw_pcm).expect("parse fig_metadata parameterConsumptionMap");
        assert_eq!(pcm_val["entries"], json!([preset_entry]), "fig_metadata parameterConsumptionMap must only keep entries the model does not hold");
    }

    doc1.nodes.get_mut(&hero_id).unwrap().variables.bindings.insert("opacity".into(), "VariableID:1:99".into());
    doc1.nodes.get_mut(&hero_id).unwrap().variables.bindings.insert("fontStyle".into(), "VariableID:1:98".into());

    let bytes1 = export_fig(&doc1).expect("export 1");
    let decoded1 = decode_fig_to_json(&bytes1).expect("decode 1");
    let changes1 = decoded1["message"]["nodeChanges"].as_array().expect("nodeChanges 1");
    let hero_nc1 = changes1.iter().find(|n| n["guid"] == g(1, 20)).expect("Hero NodeChange");
    assert!(hero_nc1.get("variableConsumptionMap").is_none(), "edited parameterConsumptionMap bindings must not duplicate into variableConsumptionMap");

    let expected_alias_99 = json!({
        "nodeField": 18,
        "variableField": "OPACITY",
        "variableData": {
            "dataType": "ALIAS",
            "resolvedDataType": "FLOAT",
            "value": {
                "alias": {"guid": g(1, 99)}
            }
        }
    });
    let expected_font_style_98 = json!({
        "nodeField": 42,
        "variableField": "FONT_STYLE",
        "variableData": {
            "dataType": "FONT_STYLE",
            "resolvedDataType": "FONT_STYLE",
            "value": {
                "fontStyleValue": {
                    "asString": {
                        "dataType": "ALIAS",
                        "resolvedDataType": "STRING",
                        "value": {"alias": {"guid": g(1, 98)}}
                    }
                }
            }
        }
    });
    let entries1 = hero_nc1["parameterConsumptionMap"]["entries"].as_array().expect("parameterConsumptionMap entries");
    assert!(entries1.contains(&expected_alias_99), "updated ALIAS entry 1:99 must be emitted in parameterConsumptionMap: {entries1:?}");
    assert!(entries1.contains(&expected_font_style_98), "updated FONT_STYLE entry 1:98 must be emitted in parameterConsumptionMap: {entries1:?}");
    assert!(entries1.contains(&preset_entry), "ANIMATION_PRESET entry must survive in parameterConsumptionMap: {entries1:?}");
    assert!(entries1.contains(&manual_entry), "MANUAL track entry must survive in parameterConsumptionMap: {entries1:?}");

    let doc2 = import_fig(&bytes1).expect("re-import 1");
    let hero2 = doc2.nodes.values().find(|n| n.name == "Hero").unwrap();
    assert_eq!(hero2.variables.bindings.get("opacity").map(String::as_str), Some("VariableID:1:99"));
    assert_eq!(hero2.variables.bindings.get("fontStyle").map(String::as_str), Some("VariableID:1:98"));
    assert_eq!(hero2.variables, doc1.nodes[&hero_id].variables);
    assert_eq!(hero2.motion, doc1.nodes[&hero_id].motion);
    assert_eq!(hero2.fig_metadata, doc1.nodes[&hero_id].fig_metadata);
}
