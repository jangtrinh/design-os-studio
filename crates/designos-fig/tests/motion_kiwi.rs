//! Motion records: the converters read what the importer keeps, and write it back in the same shape.

use designos_doc::{
    ColorSpace, EasingType, FrameRate, InterpolationType, KeyEasing, KeyValue, Keyframe, KeyframeOperation, PlaybackStyle, SpringParams, Timeline,
    TimelineAssignment, TrackLoop,
};
use designos_fig::motion::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes};
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

/// What the importer keeps of a motion field, parsed back into the JSON the converters read.
fn kept(meta: &std::collections::BTreeMap<String, String>, key: &str) -> Value {
    serde_json::from_str(meta.get(key).unwrap_or_else(|| panic!("importer kept no {key}"))).expect("kept value is JSON")
}

/// All kept fields of a node as one object: JSON where the importer wrote JSON, the plain text (an enum name) otherwise.
fn kept_node(meta: &std::collections::BTreeMap<String, String>) -> Value {
    Value::Object(meta.iter().map(|(k, v)| (k.clone(), serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.clone())))).collect::<Map<_, _>>())
}

#[test]
fn timelines_and_assignments_read_what_the_importer_keeps_and_write_it_back() {
    let doc = import_fig_node_changes(&motion_file(), "Motion").expect("import");
    let stage = doc.nodes.values().find(|n| n.name == "Stage").expect("stage");

    let defs = kept(&stage.fig_metadata, "timelineDefinitions");
    let timelines = timelines_from_kiwi(Some(&defs));
    let mut outer = Timeline::new("3:10", 2_000_000);
    outer.is_default = true;
    outer.playback = PlaybackStyle::Boomerang;
    outer.autoplay = true;
    let mut inner = Timeline::new("3:11", 500_000);
    inner.parent = Some("3:10".into());
    inner.snap_fps = Some(FrameRate::new(30, 1));
    inner.quantize_playback = true;
    assert_eq!(timelines, vec![outer, inner]);
    assert_eq!(timelines_to_kiwi(&timelines), defs, "writing the model gives the recorded definitions back");

    let map = kept(&stage.fig_metadata, "timelineAssignments");
    let assignments = assignments_from_kiwi(Some(&map));
    assert_eq!(
        assignments,
        vec![TimelineAssignment { timeline: "3:11".into(), containing: "3:10".into(), offset_us: -250_000, disabled: true, autoplay: true }]
    );
    let again = assignments_to_kiwi(&assignments);
    assert_eq!(assignments_from_kiwi(Some(&again)), assignments);
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
        unmapped_tag: None,
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
    assert_eq!(ease_in.unmapped_tag, Some(15));
    assert_eq!(easing_to_kiwi(&ease_in)["easingType"], "EASE_IN", "the tag the model does not know is written back as read");
    let cubic = easing_from_kiwi(&json!({"easingType": "CUSTOM_CUBIC", "easingValue": {"bezierEasing": {"p1x": 0.5, "p1y": 0.0, "p2x": 0.25, "p2y": 1.0}}}));
    assert_eq!((cubic.easing_type, cubic.bezier), (EasingType::CustomCubic, Some([0.5, 0.0, 0.25, 1.0])));
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
        assert_eq!(after.fig_metadata, before.fig_metadata, "{name}");
    }
}
