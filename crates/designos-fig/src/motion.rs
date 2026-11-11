//! Conversions between the Motion records of a decoded `.fig` and the [`designos_doc`] Motion model.
//!
//! Every function reads or writes the JSON shape the kiwi decoder produces for one message of the schema:
//! enums are their schema names, a GUID is `{"sessionID", "localID"}`, `Int64` is a number. Reading never
//! fails: a field that is missing or of the wrong type takes the model default, so a damaged record still opens.
//!
//! Covers timeline definitions (field 552), timeline assignments (553), track bindings in
//! `parameterConsumptionMap` (445), the operation and loop mode of a `KEYFRAME_TRACK` node (509, 591),
//! and a `KEYFRAME` node (506, 507, 505, 508, 535, 599).

use std::collections::{BTreeMap, BTreeSet};

use designos_doc::{
    Color, ColorSpace, EasingType, FrameRate, InterpolationType, KeyEasing, KeyValue, Keyframe, KeyframeOperation, PlaybackStyle, SpringParams, Timeline,
    TimelineAssignment, Track, TrackLoop, TrackProperty, Vec2,
};
use serde_json::{json, Map, Value};

/// Formats a kiwi GUID JSON object (`{"sessionID", "localID"}`) as `"session:local"`.
pub fn guid_text(g: &Value) -> Option<String> {
    Some(format!("{}:{}", g.get("sessionID")?.as_u64()?, g.get("localID")?.as_u64()?))
}

/// Parses a canonical `"session:local"` string into a non-zero `(session, local)` pair.
pub fn parse_guid_text(text: &str) -> Option<(u32, u32)> {
    let (s, l) = text.split_once(':')?;
    let (s, l) = (s.parse::<u32>().ok()?, l.parse::<u32>().ok()?);
    if s == 0 && l == 0 {
        return None;
    }
    Some((s, l))
}

/// Extracts the leading session number from `text` if positive, or returns `default_session`.
pub fn guid_session_hint(text: &str, default_session: u32) -> u32 {
    text.split_once(':').and_then(|(s, _)| s.parse::<u32>().ok()).filter(|&s| s > 0).unwrap_or(default_session)
}

/// Formats a `(session, local)` GUID pair as the kiwi JSON object `{"sessionID", "localID"}`.
pub fn guid_pair_json((s, l): (u32, u32)) -> Value {
    json!({"sessionID": u64::from(s), "localID": u64::from(l)})
}

/// Deterministic allocator that maps Motion IDs (including renamed paste/duplicate IDs like `"3:1~1"`)
/// to unique non-zero kiwi GUID pairs `(u32, u32)`.
#[derive(Default)]
pub struct MotionGuids {
    used: BTreeSet<(u32, u32)>,
    next_local: BTreeMap<u32, u32>,
    mapped: BTreeMap<String, (u32, u32)>,
}

impl MotionGuids {
    pub fn new() -> Self {
        Self::default()
    }

    fn bump(&mut self, (s, l): (u32, u32)) {
        let e = self.next_local.entry(s).or_insert(1);
        if l >= *e {
            *e = l.saturating_add(1);
        }
    }

    /// Marks an existing `(session, local)` GUID as in use so fresh allocations never collide with it.
    pub fn mark_used(&mut self, g: (u32, u32)) {
        self.used.insert(g);
        self.bump(g);
    }

    /// First pass: reserves every ID that is already a canonical `<u32>:<u32>` pair.
    pub fn reserve_canonical<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        for id in ids {
            if id.is_empty() || self.mapped.contains_key(id) {
                continue;
            }
            if let Some(g) = parse_guid_text(id) {
                if self.used.insert(g) {
                    self.bump(g);
                    self.mapped.insert(id.to_string(), g);
                }
            }
        }
    }

    /// Resolves `id` to its unique `(session, local)` GUID, allocating a fresh one in `default_session`
    /// (or the session prefix of `id`) if `id` is not a free `<u32>:<u32>` pair.
    pub fn resolve(&mut self, id: &str, default_session: u32) -> (u32, u32) {
        if let Some(&g) = self.mapped.get(id) {
            return g;
        }
        if let Some(g) = parse_guid_text(id) {
            if self.used.insert(g) {
                self.bump(g);
                self.mapped.insert(id.to_string(), g);
                return g;
            }
        }
        let s = guid_session_hint(id, default_session).max(1);
        let entry = self.next_local.entry(s).or_insert(1);
        loop {
            let l = *entry;
            *entry = entry.saturating_add(1);
            if l > 0 && self.used.insert((s, l)) {
                self.mapped.insert(id.to_string(), (s, l));
                return (s, l);
            }
        }
    }
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

/// Timeline definitions (field 552) as the model's timelines, in file order.
pub fn timelines_from_kiwi(map: Option<&Value>) -> Vec<Timeline> {
    split_timelines_from_kiwi(map).0
}

/// Splits timeline definitions (field 552) into modelled timelines and any entries that lacked a valid GUID.
pub fn split_timelines_from_kiwi(map: Option<&Value>) -> (Vec<Timeline>, Option<Value>) {
    let mut timelines = Vec::new();
    let mut skipped = Vec::new();
    for e in entries(map) {
        let Some(id) = e.get("id").and_then(guid_text) else {
            skipped.push(e.clone());
            continue;
        };
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
        timelines.push(t);
    }
    let rem = if skipped.is_empty() { None } else { Some(json!({"entries": skipped})) };
    (timelines, rem)
}

/// The model's timelines as a `TimelineDefinitionsMap`. Fields at their default are left out, as the file does.
pub fn timelines_to_kiwi(timelines: &[Timeline]) -> Value {
    let mut guids = MotionGuids::new();
    guids.reserve_canonical(timelines.iter().flat_map(|t| std::iter::once(t.id.as_str()).chain(t.parent.as_deref())));
    timelines_to_kiwi_with(timelines, |id| guids.resolve(id, 3))
}

/// Like [`timelines_to_kiwi`], resolving each timeline ID via `guid_of` so document-wide GUID allocations stay consistent.
pub fn timelines_to_kiwi_with(timelines: &[Timeline], mut guid_of: impl FnMut(&str) -> (u32, u32)) -> Value {
    let list: Vec<Value> = timelines
        .iter()
        .map(|t| {
            let mut d = Map::new();
            d.insert("durationUs".into(), json!(t.duration_us));
            if t.is_default {
                d.insert("defaultTimeline".into(), json!(true));
            }
            if let Some(p) = &t.parent {
                d.insert("parentTimelineDefId".into(), guid_pair_json(guid_of(p)));
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
            json!({"id": guid_pair_json(guid_of(&t.id)), "data": Value::Object(d)})
        })
        .collect();
    json!({"entries": list})
}

/// Timeline assignments (field 553) as the model's assignments, in file order.
pub fn assignments_from_kiwi(map: Option<&Value>) -> Vec<TimelineAssignment> {
    split_assignments_from_kiwi(map).0
}

/// Splits timeline assignments (field 553) into modelled assignments and any entries that lacked valid GUIDs.
pub fn split_assignments_from_kiwi(map: Option<&Value>) -> (Vec<TimelineAssignment>, Option<Value>) {
    let mut assignments = Vec::new();
    let mut skipped = Vec::new();
    for e in entries(map) {
        let Some(key) = e.get("key") else {
            skipped.push(e.clone());
            continue;
        };
        let (Some(timeline), Some(containing)) = (key.get("assignedTimelineId").and_then(guid_text), key.get("containingTimelineId").and_then(guid_text))
        else {
            skipped.push(e.clone());
            continue;
        };
        let v = e.get("value").unwrap_or(&Value::Null);
        assignments.push(TimelineAssignment {
            timeline,
            containing,
            offset_us: v.get("offsetUs").and_then(Value::as_i64).unwrap_or(0),
            disabled: v.get("disabled").and_then(Value::as_bool).unwrap_or(false),
            autoplay: v.get("autoplay").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    let rem = if skipped.is_empty() { None } else { Some(json!({"entries": skipped})) };
    (assignments, rem)
}

/// The model's assignments as a `TimelineAssignmentsMap`.
pub fn assignments_to_kiwi(assignments: &[TimelineAssignment]) -> Value {
    let mut guids = MotionGuids::new();
    guids.reserve_canonical(assignments.iter().flat_map(|a| [a.timeline.as_str(), a.containing.as_str()]));
    assignments_to_kiwi_with(assignments, |id| guids.resolve(id, 3))
}

/// Like [`assignments_to_kiwi`], resolving each timeline ID via `guid_of` so document-wide GUID allocations stay consistent.
pub fn assignments_to_kiwi_with(assignments: &[TimelineAssignment], mut guid_of: impl FnMut(&str) -> (u32, u32)) -> Value {
    let list: Vec<Value> = assignments
        .iter()
        .map(|a| {
            let mut v = Map::new();
            v.insert("offsetUs".into(), json!(a.offset_us));
            v.insert("disabled".into(), json!(a.disabled));
            if a.autoplay {
                v.insert("autoplay".into(), json!(true));
            }
            json!({
                "key": {
                    "assignedTimelineId": guid_pair_json(guid_of(&a.timeline)),
                    "containingTimelineId": guid_pair_json(guid_of(&a.containing))
                },
                "value": Value::Object(v)
            })
        })
        .collect();
    json!({"entries": list})
}

/// One manual keyframe-track binding extracted from a layer's `parameterConsumptionMap` (field 445).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackBinding {
    pub property: TrackProperty,
    pub track_guid: String,
    pub timeline_id: Option<String>,
    pub node_field: Option<u64>,
    pub is_track_id_shape: bool,
}

/// Document `NodeVariables::bindings` keys populated by `bound_variables` for a `VariableField` name.
pub fn pcm_variable_keys_for_field(field: &str) -> &'static [&'static str] {
    match field {
        "STACK_PADDING_LEFT" => &["paddingLeft"],
        "STACK_PADDING_RIGHT" => &["paddingRight"],
        "STACK_PADDING_TOP" => &["paddingTop"],
        "STACK_PADDING_BOTTOM" => &["paddingBottom"],
        "STACK_HORIZONTAL_PADDING" => &["paddingLeft", "paddingRight"],
        "STACK_VERTICAL_PADDING" => &["paddingTop", "paddingBottom"],
        "STACK_PADDING" => &["paddingLeft", "paddingRight", "paddingTop", "paddingBottom"],
        "STACK_SPACING" | "GRID_COLUMN_GAP" => &["itemSpacing"],
        "STACK_COUNTER_SPACING" | "GRID_ROW_GAP" => &["counterAxisSpacing"],
        "RECTANGLE_TOP_LEFT_CORNER_RADIUS" => &["topLeftRadius"],
        "RECTANGLE_TOP_RIGHT_CORNER_RADIUS" => &["topRightRadius"],
        "RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS" => &["bottomLeftRadius"],
        "RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS" => &["bottomRightRadius"],
        "CORNER_RADIUS" => &["topLeftRadius", "topRightRadius", "bottomLeftRadius", "bottomRightRadius"],
        "WIDTH" => &["width"],
        "HEIGHT" => &["height"],
        "MIN_WIDTH" => &["minWidth"],
        "MAX_WIDTH" => &["maxWidth"],
        "MIN_HEIGHT" => &["minHeight"],
        "MAX_HEIGHT" => &["maxHeight"],
        "OPACITY" => &["opacity"],
        "VISIBLE" => &["visible"],
        "TEXT_DATA" => &["characters"],
        "OVERRIDDEN_SYMBOL_ID" => &["component"],
        "STROKE_WEIGHT" => &["strokeWeight"],
        "BORDER_TOP_WEIGHT" => &["strokeTopWeight"],
        "BORDER_RIGHT_WEIGHT" => &["strokeRightWeight"],
        "BORDER_BOTTOM_WEIGHT" => &["strokeBottomWeight"],
        "BORDER_LEFT_WEIGHT" => &["strokeLeftWeight"],
        "FONT_SIZE" => &["fontSize"],
        "FONT_FAMILY" => &["fontFamily"],
        "FONT_WEIGHT" | "FONT_STYLE" => &["fontWeight"],
        "LINE_HEIGHT" => &["lineHeight"],
        "LETTER_SPACING" => &["letterSpacing"],
        "PARAGRAPH_SPACING" => &["paragraphSpacing"],
        "PARAGRAPH_INDENT" => &["paragraphIndent"],
        "EASING_DATA" => &["easingData"],
        "TIMELINE_DURATION" => &["timelineDuration"],
        _ => &[],
    }
}

/// If `entry` is a `parameterConsumptionMap` row that `bound_variables` models into `NodeVariables::bindings`
/// (an `ALIAS` or `FONT_STYLE` entry whose alias target is a local `guid`), returns a template clone with the
/// bound variable `guid` removed so export can re-emit the row from the current `node.variables.bindings`.
pub fn strip_modelled_pcm_variable_entry(entry: &Value) -> Option<Value> {
    let field = entry.get("variableField").and_then(Value::as_str)?;
    let vdata = entry.get("variableData")?.as_object()?;
    if field == "FONT_STYLE" {
        if let Some(fs) = vdata.get("value").and_then(|v| v.get("fontStyleValue")).and_then(Value::as_object) {
            if fs.is_empty() {
                return None;
            }
            for (k, inner) in fs {
                if !matches!(k.as_str(), "asFloat" | "asString") {
                    return None;
                }
                let alias = inner.get("value")?.get("alias")?;
                if alias.get("assetRef").is_some() || alias.get("guid").and_then(guid_text).is_none() {
                    return None;
                }
            }
            let mut stripped = entry.clone();
            if let Some(fs_mut) =
                stripped.get_mut("variableData").and_then(|d| d.get_mut("value")).and_then(|v| v.get_mut("fontStyleValue")).and_then(Value::as_object_mut)
            {
                for inner in fs_mut.values_mut() {
                    if let Some(obj) = inner.as_object_mut() {
                        obj.remove("value");
                    }
                }
            }
            return Some(stripped);
        }
    }
    if pcm_variable_keys_for_field(field).is_empty() {
        return None;
    }
    let val_obj = vdata.get("value")?.as_object()?;
    if val_obj.len() != 1 {
        return None;
    }
    let alias = val_obj.get("alias")?;
    if alias.get("assetRef").is_some() || alias.get("guid").and_then(guid_text).is_none() {
        return None;
    }
    let mut stripped = entry.clone();
    if let Some(d_mut) = stripped.get_mut("variableData").and_then(Value::as_object_mut) {
        d_mut.remove("value");
    }
    Some(stripped)
}

/// Reconstructs `ALIAS` and `FONT_STYLE` `parameterConsumptionMap` entries from the stripped `_pcmVarFields`
/// templates and the node's current `bindings`, returning the reconstructed entries and the set of
/// `VariableField` names that `variableConsumptionMap` should skip.
pub fn reconstruct_pcm_variable_entries(
    pcm_var_fields_json: Option<&str>,
    bindings: &std::collections::BTreeMap<String, String>,
    mut resolve_var_guid: impl FnMut(&str) -> Option<(u32, u32)>,
) -> (Vec<Value>, std::collections::HashSet<String>) {
    let mut out_entries = Vec::new();
    let mut skip_fields = std::collections::HashSet::new();
    let templates: Vec<Value> = pcm_var_fields_json
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|val| val.get("entries").and_then(Value::as_array).cloned())
        .unwrap_or_default();

    let mut lookup_binding = |key: &str| -> Option<(u32, u32)> {
        let vid = bindings.get(key).or_else(|| match key {
            "width" => bindings.get("size/x"),
            "height" => bindings.get("size/y"),
            _ => None,
        })?;
        resolve_var_guid(vid)
    };

    for tmpl in templates {
        let Some(field) = tmpl.get("variableField").and_then(Value::as_str) else { continue };
        let Some(vdata) = tmpl.get("variableData").and_then(Value::as_object) else { continue };

        if field == "FONT_STYLE" && vdata.get("value").and_then(|v| v.get("fontStyleValue")).is_some() {
            let fs_tmpl = vdata.get("value").and_then(|v| v.get("fontStyleValue")).and_then(Value::as_object);
            let mut fs_out = Map::new();
            if fs_tmpl.is_some_and(|m| m.contains_key("asFloat")) {
                if let Some(vg) = lookup_binding("fontWeight") {
                    let mut inner = fs_tmpl.and_then(|m| m.get("asFloat")).and_then(Value::as_object).cloned().unwrap_or_else(|| {
                        let mut m = Map::new();
                        m.insert("dataType".into(), json!("ALIAS"));
                        m
                    });
                    inner.insert("value".into(), json!({"alias": {"guid": guid_pair_json(vg)}}));
                    fs_out.insert("asFloat".into(), Value::Object(inner));
                    skip_fields.insert("FONT_STYLE:asFloat".into());
                }
            }
            if fs_tmpl.is_some_and(|m| m.contains_key("asString")) {
                if let Some(vg) = lookup_binding("fontStyle") {
                    let mut inner = fs_tmpl.and_then(|m| m.get("asString")).and_then(Value::as_object).cloned().unwrap_or_else(|| {
                        let mut m = Map::new();
                        m.insert("dataType".into(), json!("ALIAS"));
                        m
                    });
                    inner.insert("value".into(), json!({"alias": {"guid": guid_pair_json(vg)}}));
                    fs_out.insert("asString".into(), Value::Object(inner));
                    skip_fields.insert("FONT_STYLE:asString".into());
                }
            }
            if !fs_out.is_empty() {
                let mut entry = tmpl.clone();
                entry["variableData"]["value"]["fontStyleValue"] = Value::Object(fs_out);
                out_entries.push(entry);
            }
            continue;
        }

        let keys = pcm_variable_keys_for_field(field);
        if keys.is_empty() {
            continue;
        }
        let mut resolved_guid: Option<(u32, u32)> = None;
        let mut all_match = true;
        for &k in keys {
            match lookup_binding(k) {
                Some(vg) if resolved_guid.is_none_or(|prev| prev == vg) => {
                    resolved_guid = Some(vg);
                }
                _ => {
                    all_match = false;
                    break;
                }
            }
        }
        if all_match {
            if let Some(vg) = resolved_guid {
                let mut entry = tmpl.clone();
                if let Some(d_mut) = entry.get_mut("variableData").and_then(Value::as_object_mut) {
                    d_mut.insert("value".into(), json!({"alias": {"guid": guid_pair_json(vg)}}));
                }
                out_entries.push(entry);
                skip_fields.insert(field.to_string());
                match field {
                    "FONT_WEIGHT" | "FONT_STYLE" => {
                        skip_fields.insert("FONT_STYLE:asFloat".into());
                    }
                    "GRID_COLUMN_GAP" => {
                        skip_fields.insert("STACK_SPACING".into());
                    }
                    "GRID_ROW_GAP" => {
                        skip_fields.insert("STACK_COUNTER_SPACING".into());
                    }
                    "STACK_HORIZONTAL_PADDING" => {
                        skip_fields.insert("STACK_PADDING_LEFT".into());
                        skip_fields.insert("STACK_PADDING_RIGHT".into());
                    }
                    "STACK_VERTICAL_PADDING" => {
                        skip_fields.insert("STACK_PADDING_TOP".into());
                        skip_fields.insert("STACK_PADDING_BOTTOM".into());
                    }
                    "STACK_PADDING" => {
                        skip_fields.insert("STACK_PADDING_LEFT".into());
                        skip_fields.insert("STACK_PADDING_RIGHT".into());
                        skip_fields.insert("STACK_PADDING_TOP".into());
                        skip_fields.insert("STACK_PADDING_BOTTOM".into());
                    }
                    "CORNER_RADIUS" => {
                        skip_fields.insert("RECTANGLE_TOP_LEFT_CORNER_RADIUS".into());
                        skip_fields.insert("RECTANGLE_TOP_RIGHT_CORNER_RADIUS".into());
                        skip_fields.insert("RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS".into());
                        skip_fields.insert("RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS".into());
                    }
                    _ => {}
                }
            }
        }
    }

    if !skip_fields.contains("EASING_DATA") {
        let ease_vid = bindings.get("easingData").or_else(|| bindings.get("transition/easing")).or_else(|| bindings.get("easing"));
        if let Some(vid) = ease_vid {
            if let Some(vg) = resolve_var_guid(vid) {
                out_entries.push(json!({
                    "variableField": "EASING_DATA",
                    "variableData": {
                        "dataType": "ALIAS",
                        "resolvedDataType": "STRING",
                        "value": {
                            "alias": {
                                "guid": guid_pair_json(vg)
                            }
                        }
                    }
                }));
                skip_fields.insert("EASING_DATA".into());
            }
        }
    }

    if !skip_fields.contains("TIMELINE_DURATION") {
        let dur_vid = bindings.get("timelineDuration").or_else(|| bindings.get("transition/duration")).or_else(|| bindings.get("duration"));
        if let Some(vid) = dur_vid {
            if let Some(vg) = resolve_var_guid(vid) {
                out_entries.push(json!({
                    "variableField": "TIMELINE_DURATION",
                    "variableData": {
                        "dataType": "ALIAS",
                        "resolvedDataType": "FLOAT",
                        "value": {
                            "alias": {
                                "guid": guid_pair_json(vg)
                            }
                        }
                    }
                }));
                skip_fields.insert("TIMELINE_DURATION".into());
            }
        }
    }

    (out_entries, skip_fields)
}

/// Splits a layer's `parameterConsumptionMap` (field 445) into:
/// 1. Manual keyframe-track bindings whose `keyframeTrackId.guid` resolves to a known `KEYFRAME_TRACK` node in `valid_track_guids`.
/// 2. Remaining `parameterConsumptionMap` JSON for `bound_variables` (`ALIAS` / `FONT_STYLE` entries), or `None` when none remain.
/// 3. Unmodelled `parameterConsumptionMap` JSON (preset entries and any other rows `bound_variables` does not model) to keep in `fig_metadata`, or `None`.
pub fn extract_track_bindings(param_map: Option<&Value>, valid_track_guids: &BTreeSet<String>) -> (Vec<TrackBinding>, Option<Value>, Option<Value>) {
    let Some(map) = param_map else { return (Vec::new(), None, None) };
    let list = entries(Some(map));
    if list.is_empty() {
        return (Vec::new(), Some(map.clone()), None);
    }
    let mut bindings = Vec::new();
    let mut kept_for_vars = Vec::new();
    let mut kept_for_meta = Vec::new();

    for entry in list {
        let var_field =
            entry.get("variableField").and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|u| u.to_string()))).filter(|s| !s.is_empty());
        let node_field = entry.get("nodeField").and_then(Value::as_u64);
        let vdata = entry.get("variableData");
        let val = vdata.and_then(|d| d.get("value"));
        let is_motion_entry =
            vdata.and_then(|d| d.get("dataType")).and_then(Value::as_str).is_some_and(|dt| matches!(dt, "KEYFRAME_TRACK_PARAMETER_DATA" | "KEYFRAME_TRACK_ID"))
                || val.is_some_and(|v| v.get("keyframeTrackParameterValue").is_some() || v.get("keyframeTrackIdValue").is_some());

        if let (Some(field_name), Some(val_obj)) = (var_field, val) {
            if let Some(params) = val_obj.get("keyframeTrackParameterValue").and_then(|k| k.get("parameters")).and_then(Value::as_array) {
                if !params.is_empty() {
                    let mut kept_params = Vec::new();
                    for param in params {
                        let ptype = param.get("type").and_then(Value::as_str).unwrap_or("MANUAL");
                        let pval = param.get("value");
                        let manual = pval.and_then(|v| v.get("manual"));
                        let has_other = pval.is_some_and(|v| v.get("animationPreset").is_some() || v.get("animationStyleBindingId").is_some());
                        let ktid = manual.and_then(|m| m.get("keyframeTrackId"));
                        let track_guid = ktid.and_then(|k| k.get("guid")).and_then(guid_text);
                        let has_asset_ref = ktid.is_some_and(|k| k.get("assetRef").is_some());

                        if ptype == "MANUAL" && !has_other && !has_asset_ref {
                            if let Some(tg) = track_guid.filter(|tg| valid_track_guids.contains(tg)) {
                                let tl_id = manual.and_then(|m| m.get("timelineDefId")).and_then(guid_text);
                                bindings.push(TrackBinding {
                                    property: TrackProperty::from_kiwi_name(&field_name),
                                    track_guid: tg,
                                    timeline_id: tl_id,
                                    node_field,
                                    is_track_id_shape: false,
                                });
                                continue;
                            }
                        }
                        kept_params.push(param.clone());
                    }
                    if !kept_params.is_empty() {
                        let mut partial = entry.clone();
                        partial["variableData"]["value"]["keyframeTrackParameterValue"]["parameters"] = Value::Array(kept_params);
                        kept_for_meta.push(partial);
                    }
                    continue;
                }
            } else if let Some(ktid) = val_obj.get("keyframeTrackIdValue") {
                let track_guid = ktid.get("guid").and_then(guid_text);
                let has_asset_ref = ktid.get("assetRef").is_some();
                if !has_asset_ref {
                    if let Some(tg) = track_guid.filter(|tg| valid_track_guids.contains(tg)) {
                        bindings.push(TrackBinding {
                            property: TrackProperty::from_kiwi_name(&field_name),
                            track_guid: tg,
                            timeline_id: None,
                            node_field,
                            is_track_id_shape: true,
                        });
                        continue;
                    }
                }
            }
        }
        if !is_motion_entry {
            kept_for_vars.push(entry.clone());
            if strip_modelled_pcm_variable_entry(entry).is_some() {
                continue;
            }
        }
        kept_for_meta.push(entry.clone());
    }

    let rem_vars = if kept_for_vars.is_empty() { None } else { Some(json!({"entries": kept_for_vars})) };
    let rem_meta = if kept_for_meta.is_empty() { None } else { Some(json!({"entries": kept_for_meta})) };
    (bindings, rem_vars, rem_meta)
}

/// Default `NodeChange` field number (`nodeField`) for a [`TrackProperty`] when none was recorded on import.
pub fn default_node_field_for_property(prop: &TrackProperty) -> u64 {
    match prop {
        TrackProperty::Opacity => 18,
        TrackProperty::Width | TrackProperty::Height => 16,
        TrackProperty::CornerRadius => 25,
        TrackProperty::StrokeWeight => 29,
        TrackProperty::TextData => 42,
        TrackProperty::TranslationX
        | TrackProperty::TranslationY
        | TrackProperty::TranslationXy
        | TrackProperty::Rotation
        | TrackProperty::ScaleX
        | TrackProperty::ScaleY
        | TrackProperty::ScaleXy
        | TrackProperty::Shear
        | TrackProperty::TransformOriginX
        | TrackProperty::TransformOriginY
        | TrackProperty::TransformOriginXy => 21,
        TrackProperty::PathTrimStart | TrackProperty::PathTrimEnd | TrackProperty::Other(_) => 0,
    }
}

/// Builds the `VariableDataMapEntry` JSON items for `tracks` to be written into `parameterConsumptionMap.entries`.
pub fn tracks_to_parameter_consumption_entries(
    tracks: &[Track],
    track_guid_of: impl FnMut(usize, &Track) -> (u32, u32),
    mut timeline_guid_of: impl FnMut(&str) -> (u32, u32),
    node_field_of: impl FnMut(usize, &TrackProperty) -> Option<u64>,
) -> Vec<Value> {
    tracks_to_parameter_consumption_entries_with_shape(tracks, track_guid_of, |tid| (!tid.is_empty()).then(|| timeline_guid_of(tid)), node_field_of, |_| false)
}

/// Like [`tracks_to_parameter_consumption_entries`], omitting `timelineDefId` when `timeline_guid_of` returns `None`
/// and keeping the `KEYFRAME_TRACK_ID` shape when `is_track_id_shape` returns `true` for a timeless track.
pub fn tracks_to_parameter_consumption_entries_with_shape(
    tracks: &[Track],
    mut track_guid_of: impl FnMut(usize, &Track) -> (u32, u32),
    mut timeline_guid_of: impl FnMut(&str) -> Option<(u32, u32)>,
    mut node_field_of: impl FnMut(usize, &TrackProperty) -> Option<u64>,
    mut is_track_id_shape: impl FnMut(usize) -> bool,
) -> Vec<Value> {
    enum GroupItem {
        ParamGroup(TrackProperty, u64, Vec<Value>),
        TrackIdEntry(Value),
    }

    let mut items: Vec<GroupItem> = Vec::new();
    for (idx, track) in tracks.iter().enumerate() {
        if track.property.kiwi_name().is_empty() {
            continue;
        }
        let nf = node_field_of(idx, &track.property).unwrap_or_else(|| default_node_field_for_property(&track.property));
        let tg = track_guid_of(idx, track);
        let tlg = timeline_guid_of(&track.timeline);
        if tlg.is_none() && is_track_id_shape(idx) {
            let vf_name = track.property.kiwi_name();
            let vf_val = vf_name.parse::<u64>().map(Value::from).unwrap_or_else(|_| Value::String(vf_name.to_string()));
            items.push(GroupItem::TrackIdEntry(json!({
                "nodeField": nf,
                "variableField": vf_val,
                "variableData": {
                    "dataType": "KEYFRAME_TRACK_ID",
                    "resolvedDataType": "KEYFRAME_TRACK_ID",
                    "value": {
                        "keyframeTrackIdValue": {
                            "guid": guid_pair_json(tg)
                        }
                    }
                }
            })));
            continue;
        }
        let mut manual = Map::new();
        manual.insert("keyframeTrackId".into(), json!({"guid": guid_pair_json(tg)}));
        if let Some(tlg) = tlg {
            manual.insert("timelineDefId".into(), guid_pair_json(tlg));
        }
        let param = json!({
            "type": "MANUAL",
            "value": {
                "manual": Value::Object(manual)
            }
        });
        if let Some(GroupItem::ParamGroup(_, _, params)) =
            items.iter_mut().find(|it| matches!(it, GroupItem::ParamGroup(p, existing_nf, _) if *p == track.property && *existing_nf == nf))
        {
            params.push(param);
        } else {
            items.push(GroupItem::ParamGroup(track.property.clone(), nf, vec![param]));
        }
    }

    items
        .into_iter()
        .map(|item| match item {
            GroupItem::TrackIdEntry(v) => v,
            GroupItem::ParamGroup(prop, nf, params) => {
                let vf_name = prop.kiwi_name();
                let vf_val = vf_name.parse::<u64>().map(Value::from).unwrap_or_else(|_| Value::String(vf_name.to_string()));
                json!({
                    "nodeField": nf,
                    "variableField": vf_val,
                    "variableData": {
                        "value": {
                            "keyframeTrackParameterValue": {
                                "parameters": params
                            }
                        },
                        "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
                        "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
                    }
                })
            }
        })
        .collect()
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
    let mut e = EasingType::from_kiwi_name(name).map(KeyEasing::new).unwrap_or_default();
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
    let mut out = json!({"easingType": e.easing_type.kiwi_name()});
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
    // Kiwi tag 0 is HOLD: a record without the field (or with a name this model lacks) is a hold, not the
    // Bezier a keyframe made in the editor starts with.
    k.interpolation = name_of(node.get("interpolationType"), &INTERPOLATION).unwrap_or(InterpolationType::Hold);
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
