//! Conversions between the Motion records of a decoded `.fig` and the [`designos_doc`] Motion model.
//!
//! Every function reads or writes the JSON shape the kiwi decoder produces for one message of the schema:
//! enums are their schema names, a GUID is `{"sessionID", "localID"}`, `Int64` is a number. Reading never
//! fails: a field that is missing or of the wrong type takes the model default, so a damaged record still opens.
//!
//! What is here: the timeline definitions (field 552), the timeline assignments (553), the operation and loop
//! mode of a track node (509, 591), and a keyframe node (506, 507, 505, 508, 535, 599). What is not: the join
//! that says which layer and property a track animates (the `parameterConsumptionMap`, 445, and the node tree of
//! `KEYFRAME_TRACK` / `KEYFRAME`). Until that join is wired into the importer, those records travel through the
//! typed passthrough unchanged (`fig_metadata`), which is lossless.

use designos_doc::{
    Color, ColorSpace, EasingType, FrameRate, InterpolationType, KeyEasing, KeyValue, Keyframe, KeyframeOperation, PlaybackStyle, SpringParams, Timeline,
    TimelineAssignment, TrackLoop, Vec2,
};
use serde_json::{json, Map, Value};

fn guid_text(g: &Value) -> Option<String> {
    Some(format!("{}:{}", g.get("sessionID")?.as_u64()?, g.get("localID")?.as_u64()?))
}

fn guid_json(text: &str) -> Value {
    let (s, l) = text.split_once(':').unwrap_or((text, "0"));
    json!({"sessionID": s.parse::<u64>().unwrap_or(0), "localID": l.parse::<u64>().unwrap_or(0)})
}

fn f(v: &Value, k: &str) -> f32 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32
}

fn entries(v: Option<&Value>) -> &[Value] {
    v.and_then(|m| m.get("entries")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn name_of<T: Copy>(v: Option<&Value>, table: &[(&str, T)]) -> Option<T> {
    let n = v?.as_str()?;
    table.iter().find(|(k, _)| *k == n).map(|(_, t)| *t)
}

fn name_for<T: Copy + PartialEq>(t: T, table: &[(&'static str, T)]) -> &'static str {
    table.iter().find(|(_, x)| *x == t).map(|(k, _)| *k).unwrap_or(table[0].0)
}

const PLAYBACK: [(&str, PlaybackStyle); 3] = [("ONCE", PlaybackStyle::Once), ("LOOP", PlaybackStyle::Loop), ("BOOMERANG", PlaybackStyle::Boomerang)];
const OPERATION: [(&str, KeyframeOperation); 3] = [("SET", KeyframeOperation::Set), ("SCALE", KeyframeOperation::Scale), ("OFFSET", KeyframeOperation::Offset)];
const TRACK_LOOP: [(&str, TrackLoop); 2] = [("NONE", TrackLoop::None), ("LOOP", TrackLoop::Loop)];
const INTERPOLATION: [(&str, InterpolationType); 3] =
    [("BEZIER", InterpolationType::Bezier), ("HOLD", InterpolationType::Hold), ("SPRING", InterpolationType::Spring)];
const COLOR_SPACE: [(&str, ColorSpace); 5] = [
    ("DEFAULT", ColorSpace::Default),
    ("OKLAB", ColorSpace::Oklab),
    ("OKLCH", ColorSpace::Oklch),
    ("HSL", ColorSpace::Hsl),
    ("SRGB_LINEAR", ColorSpace::SrgbLinear),
];

/// The kiwi tag of `EasingType.EASE_IN`, which [`EasingType`] does not have yet.
const EASE_IN_TAG: u32 = 15;

/// Timeline definitions (field 552) as the model's timelines, in file order.
pub fn timelines_from_kiwi(map: Option<&Value>) -> Vec<Timeline> {
    entries(map)
        .iter()
        .filter_map(|e| {
            let id = e.get("id").and_then(guid_text)?;
            let d = e.get("data").unwrap_or(&Value::Null);
            let mut t = Timeline::new(id, d.get("durationUs").and_then(Value::as_u64).unwrap_or(0));
            t.is_default = d.get("defaultTimeline").and_then(Value::as_bool).unwrap_or(false);
            t.parent = d.get("parentTimelineDefId").and_then(guid_text);
            t.playback = name_of(d.get("playbackStyle"), &PLAYBACK).unwrap_or_default();
            t.autoplay = d.get("autoplay").and_then(Value::as_bool).unwrap_or(false);
            t.snap_fps = d.get("keyframeSnappingFps").and_then(|r| {
                let (n, den) = (r.get("numerator")?.as_u64()?, r.get("denominator")?.as_u64()?);
                Some(FrameRate::new(u32::try_from(n).ok()?, u32::try_from(den).ok()?))
            });
            t.quantize_playback = d.get("quantizePlayback").and_then(Value::as_bool).unwrap_or(false);
            Some(t)
        })
        .collect()
}

/// The model's timelines as a `TimelineDefinitionsMap`. Fields at their default are left out, as the file does.
pub fn timelines_to_kiwi(timelines: &[Timeline]) -> Value {
    let list: Vec<Value> = timelines
        .iter()
        .map(|t| {
            let mut d = Map::new();
            d.insert("durationUs".into(), json!(t.duration_us));
            if t.is_default {
                d.insert("defaultTimeline".into(), json!(true));
            }
            if let Some(p) = &t.parent {
                d.insert("parentTimelineDefId".into(), guid_json(p));
            }
            if t.playback != PlaybackStyle::Once {
                d.insert("playbackStyle".into(), json!(name_for(t.playback, &PLAYBACK)));
            }
            if t.autoplay {
                d.insert("autoplay".into(), json!(true));
            }
            if let Some(r) = t.snap_fps {
                d.insert("keyframeSnappingFps".into(), json!({"numerator": r.numerator, "denominator": r.denominator}));
            }
            if t.quantize_playback {
                d.insert("quantizePlayback".into(), json!(true));
            }
            json!({"id": guid_json(&t.id), "data": Value::Object(d)})
        })
        .collect();
    json!({"entries": list})
}

/// Timeline assignments (field 553) as the model's assignments, in file order.
pub fn assignments_from_kiwi(map: Option<&Value>) -> Vec<TimelineAssignment> {
    entries(map)
        .iter()
        .filter_map(|e| {
            let key = e.get("key")?;
            let v = e.get("value").unwrap_or(&Value::Null);
            Some(TimelineAssignment {
                timeline: key.get("assignedTimelineId").and_then(guid_text)?,
                containing: key.get("containingTimelineId").and_then(guid_text)?,
                offset_us: v.get("offsetUs").and_then(Value::as_i64).unwrap_or(0),
                disabled: v.get("disabled").and_then(Value::as_bool).unwrap_or(false),
                autoplay: v.get("autoplay").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// The model's assignments as a `TimelineAssignmentsMap`.
pub fn assignments_to_kiwi(assignments: &[TimelineAssignment]) -> Value {
    let list: Vec<Value> = assignments
        .iter()
        .map(|a| {
            let mut v = Map::new();
            v.insert("offsetUs".into(), json!(a.offset_us));
            v.insert("disabled".into(), json!(a.disabled));
            if a.autoplay {
                v.insert("autoplay".into(), json!(true));
            }
            json!({"key": {"assignedTimelineId": guid_json(&a.timeline), "containingTimelineId": guid_json(&a.containing)}, "value": Value::Object(v)})
        })
        .collect();
    json!({"entries": list})
}

/// The operation (509) and loop mode (591) of a `KEYFRAME_TRACK` node.
pub fn track_modes_from_kiwi(node: &Value) -> (KeyframeOperation, TrackLoop) {
    (name_of(node.get("keyframeOperation"), &OPERATION).unwrap_or_default(), name_of(node.get("trackLoopMode"), &TRACK_LOOP).unwrap_or_default())
}

/// Writes the operation and loop mode of a track into the fields of a `KEYFRAME_TRACK` node.
pub fn track_modes_to_kiwi(op: KeyframeOperation, loop_mode: TrackLoop, node: &mut Map<String, Value>) {
    node.insert("keyframeOperation".into(), json!(name_for(op, &OPERATION)));
    node.insert("trackLoopMode".into(), json!(name_for(loop_mode, &TRACK_LOOP)));
}

fn vec2(v: Option<&Value>) -> Vec2 {
    v.map(|v| Vec2::new(f(v, "x"), f(v, "y"))).unwrap_or_default()
}

fn vec2_json(v: Vec2) -> Value {
    json!({"x": v.x, "y": v.y})
}

/// A `KeyframeValueData` as a model value. A kind the model does not blend is kept as `Raw`.
pub fn key_value_from_kiwi(data: &Value) -> KeyValue {
    let kind = data.get("valueType").and_then(Value::as_str).unwrap_or("FLOAT");
    let value = data.get("value").unwrap_or(&Value::Null);
    let field = |k: &str| value.get(k);
    let typed = match kind {
        "FLOAT" => field("floatValue").and_then(Value::as_f64).map(|x| KeyValue::Float(x as f32)),
        "BOOL" => field("boolValue").and_then(Value::as_bool).map(KeyValue::Bool),
        "VECTOR" => field("vectorValue").map(|v| KeyValue::Vector(vec2(Some(v)))),
        "COLOR" => field("colorValue")
            .map(|c| KeyValue::Color(Color { r: f(c, "r"), g: f(c, "g"), b: f(c, "b"), a: c.get("a").and_then(Value::as_f64).unwrap_or(1.0) as f32 })),
        "CIRCLE" => field("circleValue").map(|c| KeyValue::Circle { center: vec2(c.get("center")), radius: f(c, "radius") }),
        "LINE" => field("lineValue").map(|l| KeyValue::Line { a: vec2(l.get("a")), b: vec2(l.get("b")) }),
        "CIRCLE_POINT" => {
            field("circlePointValue").map(|c| KeyValue::CirclePoint { center: vec2(c.get("center")), radius: f(c, "radius"), angle: f(c, "angle") })
        }
        "ROTATION_3D" => field("rotation3DValue").map(|r| KeyValue::Rotation3D { x: f(r, "x"), y: f(r, "y"), z: f(r, "z"), translate_z: f(r, "translateZ") }),
        _ => None,
    };
    typed.unwrap_or_else(|| KeyValue::Raw { kind: kind.to_string(), json: value.clone() })
}

/// The model value as a `KeyframeValueData`.
pub fn key_value_to_kiwi(v: &KeyValue) -> Value {
    let (kind, value) = match v {
        KeyValue::Float(x) => ("FLOAT", json!({"floatValue": x})),
        KeyValue::Bool(b) => ("BOOL", json!({"boolValue": b})),
        KeyValue::Vector(p) => ("VECTOR", json!({"vectorValue": vec2_json(*p)})),
        KeyValue::Color(c) => ("COLOR", json!({"colorValue": {"r": c.r, "g": c.g, "b": c.b, "a": c.a}})),
        KeyValue::Circle { center, radius } => ("CIRCLE", json!({"circleValue": {"center": vec2_json(*center), "radius": radius}})),
        KeyValue::Line { a, b } => ("LINE", json!({"lineValue": {"a": vec2_json(*a), "b": vec2_json(*b)}})),
        KeyValue::CirclePoint { center, radius, angle } => {
            ("CIRCLE_POINT", json!({"circlePointValue": {"center": vec2_json(*center), "radius": radius, "angle": angle}}))
        }
        KeyValue::Rotation3D { x, y, z, translate_z } => ("ROTATION_3D", json!({"rotation3DValue": {"x": x, "y": y, "z": z, "translateZ": translate_z}})),
        KeyValue::Raw { kind, json } => return json!({"value": json, "valueType": kind}),
    };
    json!({"value": value, "valueType": kind})
}

fn handles(v: &Value) -> [f32; 4] {
    [f(v, "p1x"), f(v, "p1y"), f(v, "p2x"), f(v, "p2y")]
}

fn handles_json(h: [f32; 4]) -> Value {
    json!({"p1x": h[0], "p1y": h[1], "p2x": h[2], "p2y": h[3]})
}

/// An `EasingData` as a model easing.
pub fn easing_from_kiwi(data: &Value) -> KeyEasing {
    let name = data.get("easingType").and_then(Value::as_str).unwrap_or("");
    let mut e = match EasingType::from_kiwi_name(name) {
        Some(t) => KeyEasing::new(t),
        None if name == "EASE_IN" => {
            let mut e = KeyEasing::new(EasingType::InCubic);
            e.unmapped_tag = Some(EASE_IN_TAG);
            e
        }
        None => KeyEasing::default(),
    };
    let value = data.get("easingValue").unwrap_or(&Value::Null);
    e.bezier = value.get("bezierEasing").map(handles);
    e.spring = value.get("springEasing").map(|s| SpringParams {
        stiffness: f(s, "stiffness"),
        damping: f(s, "damping"),
        mass: s.get("mass").and_then(Value::as_f64).unwrap_or(1.0) as f32,
        normalized: s.get("springType").and_then(Value::as_str) == Some("NORMALIZED"),
    });
    e
}

/// The model easing as an `EasingData`.
pub fn easing_to_kiwi(e: &KeyEasing) -> Value {
    let name = if e.unmapped_tag == Some(EASE_IN_TAG) { "EASE_IN" } else { e.easing_type.kiwi_name() };
    let mut out = json!({"easingType": name});
    if let Some(b) = e.bezier {
        out["easingValue"] = json!({"bezierEasing": handles_json(b)});
    } else if let Some(s) = e.spring {
        let kind = if s.normalized { "NORMALIZED" } else { "PHYSICAL" };
        out["easingValue"] = json!({"springEasing": {"stiffness": s.stiffness, "damping": s.damping, "mass": s.mass, "springType": kind}});
    }
    out
}

/// A `KEYFRAME` node (position 506, value 507, interpolation 505, handles 508, easing 535, colour space 599).
/// `None` when the node has no position or value, since there is nothing to place.
pub fn keyframe_from_kiwi(node: &Value) -> Option<Keyframe> {
    let time_us = node.get("timelinePosition").and_then(Value::as_i64)?;
    let mut k = Keyframe::new(time_us, key_value_from_kiwi(node.get("keyframeValue")?));
    k.interpolation = name_of(node.get("interpolationType"), &INTERPOLATION).unwrap_or_default();
    k.bezier_handles = node.get("bezierHandles").map(handles);
    k.easing = node.get("easingData").map(easing_from_kiwi);
    k.color_space = name_of(node.get("interpolationColorSpace"), &COLOR_SPACE).unwrap_or_default();
    Some(k)
}

/// Writes a keyframe into the fields of a `KEYFRAME` node. Fields at their default are left out.
pub fn keyframe_to_kiwi(k: &Keyframe, node: &mut Map<String, Value>) {
    node.insert("timelinePosition".into(), json!(k.time_us));
    node.insert("keyframeValue".into(), key_value_to_kiwi(&k.value));
    node.insert("interpolationType".into(), json!(name_for(k.interpolation, &INTERPOLATION)));
    if let Some(h) = k.bezier_handles {
        node.insert("bezierHandles".into(), handles_json(h));
    }
    if let Some(e) = &k.easing {
        node.insert("easingData".into(), easing_to_kiwi(e));
    }
    if k.color_space != ColorSpace::Default {
        node.insert("interpolationColorSpace".into(), json!(name_for(k.color_space, &COLOR_SPACE)));
    }
}
