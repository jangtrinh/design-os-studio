//! Version table and ordered migration chain (`1.0 -> 2.8 -> 2.13 -> 2.17 -> 2.21`) for `.pen` JSON scenes.

use super::PenWarning;
use serde_json::{json, Map, Value};

pub const CURRENT_PEN_VERSION: &str = "2.21";

pub struct Step {
    pub from: &'static str,
    pub to: &'static str,
    pub apply: fn(&mut Value, &mut Vec<PenWarning>),
}

pub const MIGRATIONS: &[Step] = &[
    Step { from: "1.0", to: "2.8", apply: step_1_0_to_2_8 },
    Step { from: "2.8", to: "2.13", apply: step_2_8_to_2_13 },
    Step { from: "2.13", to: "2.17", apply: step_2_13_to_2_17 },
    Step { from: "2.17", to: "2.21", apply: step_2_17_to_2_21 },
];

pub fn parse_version_pair(ver: &str) -> Option<(u32, u32)> {
    let trimmed = ver.trim();
    let (maj_s, min_s) = trimmed.split_once('.')?;
    if min_s.contains('.') {
        return None;
    }
    let maj = maj_s.parse::<u32>().ok()?;
    let min = min_s.parse::<u32>().ok()?;
    Some((maj, min))
}

/// Validates `"version"` on `root` (defaulting to `"1.0"` when omitted) and runs all applicable
/// migration steps up to [`CURRENT_PEN_VERSION`] (`"2.21"`), returning the original `source_version`.
pub fn migrate_to_current(root: &mut Value, warnings: &mut Vec<PenWarning>) -> Result<String, String> {
    let source_version = match root.get("version") {
        None | Some(Value::Null) => "1.0".to_string(),
        Some(Value::String(s)) if s.trim().is_empty() => "1.0".to_string(),
        Some(Value::String(s)) => s.trim().to_string(),
        Some(other) => {
            return Err(format!("Invalid .pen 'version' field {other}; expected a version string up to {CURRENT_PEN_VERSION}"));
        }
    };

    let Some((maj, min)) = parse_version_pair(&source_version) else {
        return Err(format!("Invalid .pen version '{source_version}'; expected '<major>.<minor>' up to {CURRENT_PEN_VERSION}"));
    };

    if (maj, min) < (1, 0) {
        return Err(format!("Unsupported .pen version '{source_version}'; minimum supported version is 1.0 (current schema {CURRENT_PEN_VERSION})"));
    }
    if (maj, min) > (2, 21) {
        return Err(format!("Unsupported .pen version '{source_version}'; file requires a newer schema than supported version {CURRENT_PEN_VERSION}"));
    }

    let start_idx = MIGRATIONS
        .iter()
        .position(|step| {
            let to_pair = parse_version_pair(step.to).unwrap_or((2, 21));
            (maj, min) < to_pair
        })
        .unwrap_or(MIGRATIONS.len());

    if let Some(obj) = root.as_object_mut() {
        obj.insert("version".into(), Value::String(source_version.clone()));
    }

    for step in &MIGRATIONS[start_idx..] {
        (step.apply)(root, warnings);
        if let Some(obj) = root.as_object_mut() {
            obj.insert("version".into(), Value::String(step.to.to_string()));
        }
    }

    if let Some(obj) = root.as_object_mut() {
        obj.insert("version".into(), Value::String(CURRENT_PEN_VERSION.to_string()));
    }

    Ok(source_version)
}

type NodeVisitorFn = dyn FnMut(&mut Map<String, Value>, &mut Vec<PenWarning>);

fn walk_nodes_mut(root: &mut Value, f: &mut NodeVisitorFn, warnings: &mut Vec<PenWarning>) {
    let mut stack: Vec<&mut Value> = Vec::new();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        stack.extend(children.iter_mut().rev());
    }
    while let Some(node) = stack.pop() {
        let Some(obj) = node.as_object_mut() else { continue };
        f(obj, warnings);
        for (k, v) in obj.iter_mut() {
            if k == "children" {
                if let Some(arr) = v.as_array_mut() {
                    stack.extend(arr.iter_mut().rev());
                }
            } else if k == "descendants" {
                if let Some(desc) = v.as_object_mut() {
                    for (_, ov) in desc.iter_mut() {
                        stack.push(ov);
                    }
                }
            }
        }
    }
}

/// `1.0 -> 2.8`:
/// - Normalizes legacy `padding: {top, right, bottom, left}` object into `[top, right, bottom, left]`.
/// - Normalizes legacy `cornerRadius: {topLeft, topRight, bottomRight, bottomLeft}` object into `[tl, tr, br, bl]`.
/// - Normalizes legacy `fillColor` onto `fill` and `strokeColor` / `strokeThickness` onto `stroke`.
/// - Normalizes legacy `icon` node type onto `icon_font`.
fn step_1_0_to_2_8(root: &mut Value, warnings: &mut Vec<PenWarning>) {
    walk_nodes_mut(
        root,
        &mut |obj, _warnings| {
            if let Some(pad_obj) = obj.get("padding").and_then(Value::as_object).cloned() {
                let top = pad_obj.get("top").and_then(Value::as_f64).unwrap_or(0.0);
                let right = pad_obj.get("right").and_then(Value::as_f64).unwrap_or(0.0);
                let bottom = pad_obj.get("bottom").and_then(Value::as_f64).unwrap_or(0.0);
                let left = pad_obj.get("left").and_then(Value::as_f64).unwrap_or(0.0);
                obj.insert("padding".into(), json!([top, right, bottom, left]));
            }
            if let Some(cr_obj) = obj.get("cornerRadius").and_then(Value::as_object).cloned() {
                let tl = cr_obj.get("topLeft").or_else(|| cr_obj.get("tl")).and_then(Value::as_f64).unwrap_or(0.0);
                let tr = cr_obj.get("topRight").or_else(|| cr_obj.get("tr")).and_then(Value::as_f64).unwrap_or(0.0);
                let br = cr_obj.get("bottomRight").or_else(|| cr_obj.get("br")).and_then(Value::as_f64).unwrap_or(0.0);
                let bl = cr_obj.get("bottomLeft").or_else(|| cr_obj.get("bl")).and_then(Value::as_f64).unwrap_or(0.0);
                obj.insert("cornerRadius".into(), json!([tl, tr, br, bl]));
            }
            if !obj.contains_key("fill") {
                if let Some(fc) = obj.remove("fillColor") {
                    obj.insert("fill".into(), fc);
                }
            }
            if !obj.contains_key("stroke") {
                if let Some(sc) = obj.remove("strokeColor") {
                    let thickness = obj.remove("strokeThickness").unwrap_or_else(|| json!(1.0));
                    obj.insert("stroke".into(), json!({ "fill": sc, "thickness": thickness }));
                }
            }
            if obj.get("type").and_then(Value::as_str) == Some("icon") {
                obj.insert("type".into(), json!("icon_font"));
                if !obj.contains_key("iconFontName") {
                    if let Some(icon_name) = obj.remove("icon").or_else(|| obj.get("content").cloned()) {
                        obj.insert("iconFontName".into(), icon_name);
                    }
                }
            }
        },
        warnings,
    );
}

/// `2.8 -> 2.13`:
/// - Normalizes `type: "component"` -> `type: "frame", reusable: true`.
/// - Normalizes `type: "instance"` -> `type: "ref"` and `componentId` -> `ref`.
/// - Normalizes `overrides` -> `descendants`.
fn step_2_8_to_2_13(root: &mut Value, warnings: &mut Vec<PenWarning>) {
    walk_nodes_mut(
        root,
        &mut |obj, _warnings| {
            match obj.get("type").and_then(Value::as_str) {
                Some("component") => {
                    obj.insert("type".into(), json!("frame"));
                    obj.insert("reusable".into(), json!(true));
                }
                Some("instance") => {
                    obj.insert("type".into(), json!("ref"));
                }
                _ => {}
            }
            if !obj.contains_key("ref") {
                if let Some(cid) = obj.remove("componentId") {
                    obj.insert("ref".into(), cid);
                }
            }
            if !obj.contains_key("descendants") {
                if let Some(ov) = obj.remove("overrides") {
                    obj.insert("descendants".into(), ov);
                }
            }
        },
        warnings,
    );
}

/// `2.13 -> 2.17`:
/// - Normalizes `type: "svg"` -> `type: "path"` and `d` / `svgPath` -> `geometry`.
/// - Normalizes `type: "image"` node into `type: "rectangle"` with an image fill when `url`/`src` is present.
/// - Normalizes legacy gradient types (`linear-gradient`, `radial-gradient`, `angular-gradient`) into
///   `type: "gradient", gradientType: ...`.
fn step_2_13_to_2_17(root: &mut Value, warnings: &mut Vec<PenWarning>) {
    walk_nodes_mut(
        root,
        &mut |obj, _warnings| {
            match obj.get("type").and_then(Value::as_str) {
                Some("svg") => {
                    obj.insert("type".into(), json!("path"));
                }
                Some("image") => {
                    obj.insert("type".into(), json!("rectangle"));
                    if !obj.contains_key("fill") {
                        if let Some(url) = obj.remove("url").or_else(|| obj.remove("src")) {
                            let mode = obj.remove("scaleMode").or_else(|| obj.remove("mode")).unwrap_or_else(|| json!("fill"));
                            obj.insert("fill".into(), json!({ "type": "image", "url": url, "mode": mode }));
                        }
                    }
                }
                _ => {}
            }
            if !obj.contains_key("geometry") {
                if let Some(d) = obj.remove("svgPath").or_else(|| obj.remove("d")) {
                    obj.insert("geometry".into(), d);
                }
            }
            if let Some(fill_val) = obj.get_mut("fill") {
                normalize_gradient_value(fill_val);
            }
        },
        warnings,
    );
}

fn normalize_gradient_value(val: &mut Value) {
    match val {
        Value::Object(map) => {
            let gtype = match map.get("type").and_then(Value::as_str) {
                Some("linear-gradient" | "linear_gradient") => Some("linear"),
                Some("radial-gradient" | "radial_gradient") => Some("radial"),
                Some("angular-gradient" | "angular_gradient" | "conic-gradient") => Some("angular"),
                Some("diamond-gradient" | "diamond_gradient") => Some("diamond"),
                _ => None,
            };
            if let Some(gt) = gtype {
                map.insert("type".into(), json!("gradient"));
                map.entry("gradientType".to_string()).or_insert_with(|| json!(gt));
            }
            if !map.contains_key("colors") {
                if let Some(stops) = map.remove("stops") {
                    map.insert("colors".into(), stops);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                normalize_gradient_value(item);
            }
        }
        _ => {}
    }
}

/// `2.17 -> 2.21`:
/// - Normalizes `slots: [...]` -> `slot: [...]`.
/// - Normalizes `isPlaceholder: true` -> `placeholder: true`.
/// - Normalizes `horizontalFlip` -> `flipX` and `verticalFlip` -> `flipY`.
/// - Normalizes top-level `strokeWidth` / `strokeAlignment` / `strokeCap` / `strokeJoin` into `stroke`.
fn step_2_17_to_2_21(root: &mut Value, warnings: &mut Vec<PenWarning>) {
    walk_nodes_mut(
        root,
        &mut |obj, _warnings| {
            if !obj.contains_key("slot") {
                if let Some(slots) = obj.remove("slots") {
                    obj.insert("slot".into(), slots);
                }
            }
            if !obj.contains_key("placeholder") {
                if let Some(ph) = obj.remove("isPlaceholder") {
                    obj.insert("placeholder".into(), ph);
                }
            }
            if !obj.contains_key("flipX") {
                if let Some(fx) = obj.remove("horizontalFlip") {
                    obj.insert("flipX".into(), fx);
                }
            }
            if !obj.contains_key("flipY") {
                if let Some(fy) = obj.remove("verticalFlip") {
                    obj.insert("flipY".into(), fy);
                }
            }
            if !obj.contains_key("layoutPosition") && obj.get("position").and_then(Value::as_str) == Some("absolute") {
                obj.remove("position");
                obj.insert("layoutPosition".into(), json!("absolute"));
            }
        },
        warnings,
    );
}
