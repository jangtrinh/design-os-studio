//! Paint, gradient, image, stroke, effect, and corner-radius parsing and serialization for `.pen` 2.21.

use super::variables::{resolve_doc_var, resolve_doc_var_color};
use super::PenWarning;
use designos_doc::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Component, Path};

pub const MAX_SINGLE_IMAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TOTAL_IMAGE_BYTES: usize = 256 * 1024 * 1024;

pub fn parse_blend_mode(s: &str) -> BlendMode {
    match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "normal" => BlendMode::Normal,
        "pass-through" | "passthrough" => BlendMode::PassThrough,
        "darken" => BlendMode::Darken,
        "multiply" => BlendMode::Multiply,
        "color-burn" | "colorburn" => BlendMode::ColorBurn,
        "linear-burn" | "linearburn" => BlendMode::LinearBurn,
        "lighten" => BlendMode::Lighten,
        "screen" => BlendMode::Screen,
        "color-dodge" | "colordodge" => BlendMode::ColorDodge,
        "linear-dodge" | "lineardodge" | "plus-lighter" => BlendMode::LinearDodge,
        "overlay" => BlendMode::Overlay,
        "soft-light" | "softlight" => BlendMode::SoftLight,
        "hard-light" | "hardlight" => BlendMode::HardLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        "hue" => BlendMode::Hue,
        "saturation" => BlendMode::Saturation,
        "color" => BlendMode::Color,
        "luminosity" => BlendMode::Luminosity,
        _ => BlendMode::Normal,
    }
}

pub fn blend_mode_to_pen_str(b: BlendMode) -> &'static str {
    match b {
        BlendMode::Normal => "normal",
        BlendMode::PassThrough => "pass-through",
        BlendMode::Darken => "darken",
        BlendMode::Multiply => "multiply",
        BlendMode::ColorBurn => "color-burn",
        BlendMode::LinearBurn => "linear-burn",
        BlendMode::Lighten => "lighten",
        BlendMode::Screen => "screen",
        BlendMode::ColorDodge => "color-dodge",
        BlendMode::LinearDodge => "linear-dodge",
        BlendMode::Overlay => "overlay",
        BlendMode::SoftLight => "soft-light",
        BlendMode::HardLight => "hard-light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "color",
        BlendMode::Luminosity => "luminosity",
    }
}

pub fn parse_image_scale_mode(s: &str) -> ImageScaleMode {
    match s.trim().to_ascii_lowercase().as_str() {
        "fit" | "contain" => ImageScaleMode::Fit,
        "crop" => ImageScaleMode::Crop,
        "tile" | "repeat" => ImageScaleMode::Tile,
        _ => ImageScaleMode::Fill,
    }
}

pub fn image_scale_mode_to_pen_str(m: ImageScaleMode) -> &'static str {
    match m {
        ImageScaleMode::Fill => "fill",
        ImageScaleMode::Fit => "fit",
        ImageScaleMode::Crop => "crop",
        ImageScaleMode::Tile => "tile",
    }
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn resolve_color_or_var(s: &str, doc: &Document) -> (Option<Color>, Option<String>) {
    if let Some(var_name) = s.strip_prefix('$') {
        let resolved = resolve_doc_var_color(doc, var_name).unwrap_or(Color::BLACK);
        (Some(resolved), Some(var_name.to_string()))
    } else {
        (Color::from_hex(s), None)
    }
}

pub fn parse_paints(
    fill_val: Option<&Value>,
    doc: &mut Document,
    node_id: Option<&str>,
    base_dir: Option<&Path>,
    total_image_bytes: &mut usize,
    plugin_data: &mut BTreeMap<String, String>,
    warnings: &mut Vec<PenWarning>,
) -> (Vec<Paint>, Option<String>) {
    let Some(v) = fill_val else {
        return (Vec::new(), None);
    };

    if let Some(s) = v.as_str() {
        let (col, var) = resolve_color_or_var(s, doc);
        if let Some(c) = col {
            return (vec![Paint::solid(c)], var);
        }
        return (Vec::new(), var);
    }

    if let Some(obj) = v.as_object() {
        let (p_opt, var) = parse_single_paint_obj(obj, doc, node_id, base_dir, total_image_bytes, plugin_data, warnings);
        return (p_opt.into_iter().collect(), var);
    }

    if let Some(arr) = v.as_array() {
        let mut out = Vec::new();
        let mut first_var = None;
        for item in arr {
            if let Some(s) = item.as_str() {
                let (col, var) = resolve_color_or_var(s, doc);
                if first_var.is_none() {
                    first_var = var;
                }
                if let Some(c) = col {
                    out.push(Paint::solid(c));
                }
            } else if let Some(obj) = item.as_object() {
                let (p_opt, var) = parse_single_paint_obj(obj, doc, node_id, base_dir, total_image_bytes, plugin_data, warnings);
                if first_var.is_none() {
                    first_var = var;
                }
                if let Some(p) = p_opt {
                    out.push(p);
                }
            }
        }
        return (out, first_var);
    }

    (Vec::new(), None)
}

fn parse_single_paint_obj(
    obj: &Map<String, Value>,
    doc: &mut Document,
    node_id: Option<&str>,
    base_dir: Option<&Path>,
    total_image_bytes: &mut usize,
    plugin_data: &mut BTreeMap<String, String>,
    warnings: &mut Vec<PenWarning>,
) -> (Option<Paint>, Option<String>) {
    let enabled = obj.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let opacity = obj.get("opacity").and_then(Value::as_f64).map(|n| (n as f32).clamp(0.0, 1.0)).unwrap_or(1.0);
    let blend = obj.get("blendMode").and_then(Value::as_str).map(parse_blend_mode).unwrap_or(BlendMode::Normal);
    let ptype = obj.get("type").and_then(Value::as_str).unwrap_or("color");

    match ptype {
        "gradient" | "linear-gradient" | "linear_gradient" | "radial-gradient" | "radial_gradient" | "angular-gradient" | "angular_gradient"
        | "diamond-gradient" | "diamond_gradient" => {
            let gtype = obj.get("gradientType").and_then(Value::as_str).unwrap_or(match ptype {
                "radial-gradient" | "radial_gradient" => "radial",
                "angular-gradient" | "angular_gradient" => "angular",
                "diamond-gradient" | "diamond_gradient" => "diamond",
                _ => "linear",
            });

            let stops_arr = obj.get("colors").or_else(|| obj.get("stops")).and_then(Value::as_array);
            let mut stops = Vec::new();
            if let Some(arr) = stops_arr {
                let len = arr.len();
                for (idx, stop_v) in arr.iter().enumerate() {
                    let default_pos = if len > 1 { idx as f32 / (len - 1) as f32 } else { 0.0 };
                    let pos = stop_v
                        .get("position")
                        .or_else(|| stop_v.get("offset"))
                        .or_else(|| stop_v.get("pos"))
                        .and_then(Value::as_f64)
                        .map(|n| (n as f32).clamp(0.0, 1.0))
                        .unwrap_or(default_pos);
                    let col_str = stop_v.get("color").and_then(Value::as_str).unwrap_or("#000000");
                    let mut color = resolve_color_or_var(col_str, doc).0.unwrap_or(Color::BLACK);
                    if let Some(stop_op) = stop_v.get("opacity").and_then(Value::as_f64) {
                        color.a *= (stop_op as f32).clamp(0.0, 1.0);
                    }
                    stops.push(GradientStop { pos, color });
                }
            }
            if stops.is_empty() {
                stops.push(GradientStop { pos: 0.0, color: Color::BLACK });
                stops.push(GradientStop { pos: 1.0, color: Color::WHITE });
            }

            let deg = obj.get("rotation").or_else(|| obj.get("angle")).and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let handles = parse_gradient_handles(obj, gtype, deg);

            let kind = match gtype.to_ascii_lowercase().as_str() {
                "radial" => PaintKind::RadialGradient { handles, stops },
                "angular" | "conic" => PaintKind::AngularGradient { handles, stops },
                "diamond" => PaintKind::DiamondGradient { handles, stops },
                _ => PaintKind::LinearGradient { handles, stops },
            };

            (Some(Paint { kind, opacity, visible: enabled, blend }), None)
        }
        "image" => {
            let url = obj.get("url").or_else(|| obj.get("src")).and_then(Value::as_str).unwrap_or("");
            let mode_str = obj.get("mode").or_else(|| obj.get("scaleMode")).and_then(Value::as_str).unwrap_or("fill");
            let scale_mode = parse_image_scale_mode(mode_str);
            plugin_data.insert("pen:image-url".into(), url.to_string());
            plugin_data.insert("pen:image-mode".into(), image_scale_mode_to_pen_str(scale_mode).to_string());

            let image_ref = resolve_image_url(url, doc, node_id, base_dir, total_image_bytes, warnings);
            (Some(Paint { kind: PaintKind::Image { image_ref, scale_mode }, opacity, visible: enabled, blend }), None)
        }
        _ => {
            if let Some(c_str) = obj.get("color").or_else(|| obj.get("fill")).and_then(Value::as_str) {
                let (col, var) = resolve_color_or_var(c_str, doc);
                if let Some(mut c) = col {
                    c.a *= opacity;
                    return (Some(Paint { kind: PaintKind::Solid(c), opacity: 1.0, visible: enabled, blend }), var);
                }
            }
            (None, None)
        }
    }
}

fn parse_vec2_val(v: &Value) -> Option<Vec2> {
    if let Some(arr) = v.as_array() {
        if arr.len() >= 2 {
            return Some(Vec2::new(arr[0].as_f64()? as f32, arr[1].as_f64()? as f32));
        }
    }
    if let Some(obj) = v.as_object() {
        return Some(Vec2::new(obj.get("x")?.as_f64()? as f32, obj.get("y")?.as_f64()? as f32));
    }
    None
}

fn parse_gradient_handles(obj: &Map<String, Value>, gtype: &str, deg: f32) -> [Vec2; 3] {
    if let Some(h_arr) = obj.get("handles").and_then(Value::as_array) {
        if h_arr.len() >= 3 {
            if let (Some(h0), Some(h1), Some(h2)) = (parse_vec2_val(&h_arr[0]), parse_vec2_val(&h_arr[1]), parse_vec2_val(&h_arr[2])) {
                return [h0, h1, h2];
            }
        }
    }
    if let (Some(start), Some(end)) = (obj.get("start").and_then(parse_vec2_val), obj.get("end").and_then(parse_vec2_val)) {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let perp = Vec2::new(start.x - dy, start.y + dx);
        return [start, end, perp];
    }

    let center = obj.get("center").and_then(parse_vec2_val).unwrap_or(Vec2::new(0.5, 0.5));
    let radius = obj.get("radius").and_then(Value::as_f64).map(|r| r as f32).unwrap_or(0.5);
    let rad = deg.to_radians();
    let d = Vec2::new(rad.cos() * radius, rad.sin() * radius);

    match gtype.to_ascii_lowercase().as_str() {
        "radial" | "angular" | "conic" | "diamond" => [center, Vec2::new(center.x + d.x, center.y + d.y), Vec2::new(center.x - d.y, center.y + d.x)],
        _ => {
            let start = Vec2::new(center.x - d.x, center.y - d.y);
            let end = Vec2::new(center.x + d.x, center.y + d.y);
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            [start, end, Vec2::new(start.x - dy, start.y + dx)]
        }
    }
}

fn resolve_image_url(
    url: &str,
    doc: &mut Document,
    node_id: Option<&str>,
    base_dir: Option<&Path>,
    total_image_bytes: &mut usize,
    warnings: &mut Vec<PenWarning>,
) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        warnings.push(PenWarning { code: "empty_image_url".into(), node_id: node_id.map(String::from), message: "Image fill has an empty URL".into() });
        return String::new();
    }

    if let Some(hash) = trimmed.strip_prefix("pen:image:") {
        return hash.to_string();
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("data:") {
        warnings.push(PenWarning {
            code: "external_image_url".into(),
            node_id: node_id.map(String::from),
            message: format!("External image URL '{trimmed}' is not fetched during offline import; preserved in plugin_data['pen:image-url']"),
        });
        return trimmed.to_string();
    }

    let Some(base) = base_dir else {
        warnings.push(PenWarning {
            code: "unresolved_image_path".into(),
            node_id: node_id.map(String::from),
            message: format!("Relative image path '{trimmed}' cannot be loaded without PenImportOptions::base_dir"),
        });
        return trimmed.to_string();
    };

    let rel_path = Path::new(trimmed);
    if rel_path.is_absolute() || rel_path.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
        warnings.push(PenWarning {
            code: "image_path_refused".into(),
            node_id: node_id.map(String::from),
            message: format!("Refused image path '{trimmed}' because it escapes base_dir"),
        });
        return trimmed.to_string();
    }

    let Ok(canon_base) = base.canonicalize() else {
        warnings.push(PenWarning {
            code: "image_load_failed".into(),
            node_id: node_id.map(String::from),
            message: format!("Base directory '{}' could not be canonicalized", base.display()),
        });
        return trimmed.to_string();
    };

    let candidate = canon_base.join(rel_path);
    let Ok(canon_target) = candidate.canonicalize() else {
        warnings.push(PenWarning {
            code: "image_load_failed".into(),
            node_id: node_id.map(String::from),
            message: format!("Image file '{trimmed}' does not exist inside base_dir"),
        });
        return trimmed.to_string();
    };

    if !canon_target.starts_with(&canon_base) {
        warnings.push(PenWarning {
            code: "image_path_refused".into(),
            node_id: node_id.map(String::from),
            message: format!("Refused image path '{trimmed}' because its canonical path escapes base_dir"),
        });
        return trimmed.to_string();
    }

    let Ok(meta) = std::fs::metadata(&canon_target) else {
        warnings.push(PenWarning {
            code: "image_load_failed".into(),
            node_id: node_id.map(String::from),
            message: format!("Cannot read metadata for image '{trimmed}'"),
        });
        return trimmed.to_string();
    };

    let file_len = meta.len() as usize;
    if file_len > MAX_SINGLE_IMAGE_BYTES {
        warnings.push(PenWarning {
            code: "image_size_exceeded".into(),
            node_id: node_id.map(String::from),
            message: format!("Image '{trimmed}' ({file_len} bytes) exceeds the 64 MiB per-image limit"),
        });
        return trimmed.to_string();
    }
    if total_image_bytes.saturating_add(file_len) > MAX_TOTAL_IMAGE_BYTES {
        warnings.push(PenWarning {
            code: "image_budget_exceeded".into(),
            node_id: node_id.map(String::from),
            message: format!("Loading image '{trimmed}' ({file_len} bytes) would exceed the 256 MiB total image budget"),
        });
        return trimmed.to_string();
    }

    match std::fs::read(&canon_target) {
        Ok(bytes) => {
            if bytes.len() > MAX_SINGLE_IMAGE_BYTES || total_image_bytes.saturating_add(bytes.len()) > MAX_TOTAL_IMAGE_BYTES {
                warnings.push(PenWarning {
                    code: "image_size_exceeded".into(),
                    node_id: node_id.map(String::from),
                    message: format!("Image '{trimmed}' exceeds image byte limits"),
                });
                return trimmed.to_string();
            }
            *total_image_bytes += bytes.len();
            let hash_key = format!("pen_img_{:016x}", fnv1a_64(&bytes));
            doc.images.insert(hash_key.clone(), bytes);
            hash_key
        }
        Err(e) => {
            warnings.push(PenWarning {
                code: "image_load_failed".into(),
                node_id: node_id.map(String::from),
                message: format!("Failed to read image '{trimmed}': {e}"),
            });
            trimmed.to_string()
        }
    }
}

pub fn parse_stroke(
    v: &Value,
    doc: &mut Document,
    node_id: Option<&str>,
    base_dir: Option<&Path>,
    total_image_bytes: &mut usize,
    plugin_data: &mut BTreeMap<String, String>,
    warnings: &mut Vec<PenWarning>,
) -> (Stroke, Option<String>, Option<String>) {
    let mut s = Stroke { weight: 1.0, miter_limit: 4.0, align: StrokeAlign::Inside, ..Default::default() };
    let mut color_var = None;
    let mut weight_var = None;

    let stroke_val = v.get("stroke");

    if let Some(sv) = stroke_val {
        if let Some(c_str) = sv.as_str() {
            let (paints, cvar) = parse_paints(Some(sv), doc, node_id, base_dir, total_image_bytes, plugin_data, warnings);
            s.paints = paints;
            color_var = cvar;
            let _ = c_str;
        } else if let Some(obj) = sv.as_object() {
            let fill_field = obj.get("fill").or_else(|| obj.get("fills")).or_else(|| obj.get("color"));
            if fill_field.is_some() {
                let (paints, cvar) = parse_paints(fill_field, doc, node_id, base_dir, total_image_bytes, plugin_data, warnings);
                s.paints = paints;
                color_var = cvar;
            }
            if let Some(t_val) = obj.get("thickness").or_else(|| obj.get("strokeWidth")).or_else(|| obj.get("width")) {
                apply_stroke_width_value(&mut s, t_val, doc, &mut weight_var);
            }
            if let Some(align_s) = obj.get("align").or_else(|| obj.get("strokeAlignment")).and_then(Value::as_str) {
                s.align = parse_stroke_align(align_s);
            }
            if let Some(cap_s) = obj.get("cap").or_else(|| obj.get("strokeCap")).and_then(Value::as_str) {
                s.cap = parse_stroke_cap(cap_s);
            }
            if let Some(join_s) = obj.get("join").or_else(|| obj.get("strokeJoin")).and_then(Value::as_str) {
                s.join = parse_stroke_join(join_s);
            }
            if let Some(arr) = obj.get("dashPattern").and_then(Value::as_array) {
                s.dashes = arr.iter().filter_map(Value::as_f64).map(|n| n as f32).collect();
            }
        }
    }

    // Also allow node-level strokeWidth / strokeAlignment / strokeCap / strokeJoin overrides.
    if let Some(t_val) = v.get("strokeWidth") {
        apply_stroke_width_value(&mut s, t_val, doc, &mut weight_var);
    }
    if let Some(align_s) = v.get("strokeAlignment").and_then(Value::as_str) {
        s.align = parse_stroke_align(align_s);
    }
    if let Some(cap_s) = v.get("strokeCap").and_then(Value::as_str) {
        s.cap = parse_stroke_cap(cap_s);
    }
    if let Some(join_s) = v.get("strokeJoin").and_then(Value::as_str) {
        s.join = parse_stroke_join(join_s);
    }

    (s, color_var, weight_var)
}

fn apply_stroke_width_value(s: &mut Stroke, t_val: &Value, doc: &Document, weight_var: &mut Option<String>) {
    if let Some(n) = t_val.as_f64() {
        s.weight = (n as f32).max(0.0);
        s.side_weights = None;
    } else if let Some(ts) = t_val.as_str() {
        if let Some(vn) = ts.strip_prefix('$') {
            *weight_var = Some(vn.to_string());
            s.weight = resolve_doc_var(doc, vn).unwrap_or(1.0).max(0.0);
            s.side_weights = None;
        }
    } else if let Some(tobj) = t_val.as_object() {
        let top = tobj.get("top").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0).max(0.0);
        let right = tobj.get("right").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0).max(0.0);
        let bottom = tobj.get("bottom").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0).max(0.0);
        let left = tobj.get("left").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0).max(0.0);
        s.side_weights = Some([top, right, bottom, left]);
        s.weight = top.max(right).max(bottom).max(left);
    } else if let Some(tarr) = t_val.as_array() {
        let sides = match tarr.len() {
            1 => {
                let v0 = resolve_num_or_var(&tarr[0], doc).unwrap_or(0.0).max(0.0);
                [v0, v0, v0, v0]
            }
            2 => {
                let v0 = resolve_num_or_var(&tarr[0], doc).unwrap_or(0.0).max(0.0);
                let v1 = resolve_num_or_var(&tarr[1], doc).unwrap_or(0.0).max(0.0);
                [v0, v1, v0, v1]
            }
            4 => [
                resolve_num_or_var(&tarr[0], doc).unwrap_or(0.0).max(0.0),
                resolve_num_or_var(&tarr[1], doc).unwrap_or(0.0).max(0.0),
                resolve_num_or_var(&tarr[2], doc).unwrap_or(0.0).max(0.0),
                resolve_num_or_var(&tarr[3], doc).unwrap_or(0.0).max(0.0),
            ],
            _ => [1.0; 4],
        };
        s.side_weights = Some(sides);
        s.weight = sides[0].max(sides[1]).max(sides[2]).max(sides[3]);
    }
}

pub fn parse_stroke_align(s: &str) -> StrokeAlign {
    match s.trim().to_ascii_lowercase().as_str() {
        "center" => StrokeAlign::Center,
        "outside" | "outer" => StrokeAlign::Outside,
        _ => StrokeAlign::Inside,
    }
}

pub fn parse_stroke_cap(s: &str) -> StrokeCap {
    match s.trim().to_ascii_lowercase().as_str() {
        "round" => StrokeCap::Round,
        "square" => StrokeCap::Square,
        _ => StrokeCap::None,
    }
}

pub fn parse_stroke_join(s: &str) -> StrokeJoin {
    match s.trim().to_ascii_lowercase().as_str() {
        "bevel" => StrokeJoin::Bevel,
        "round" => StrokeJoin::Round,
        _ => StrokeJoin::Miter,
    }
}

pub fn resolve_num_or_var(v: &Value, doc: &Document) -> Option<f32> {
    if let Some(n) = v.as_f64() {
        return Some(n as f32);
    }
    if let Some(s) = v.as_str() {
        if s.starts_with('$') {
            return resolve_doc_var(doc, s);
        }
        return s.parse::<f32>().ok();
    }
    None
}

pub fn parse_effects(effect_val: Option<&Value>, doc: &Document) -> Vec<Effect> {
    let Some(v) = effect_val else { return Vec::new() };
    let mut out = Vec::new();
    let items: Vec<&Value> = if let Some(arr) = v.as_array() {
        arr.iter().collect()
    } else if v.is_object() {
        vec![v]
    } else {
        return out;
    };

    for item in items {
        let Some(obj) = item.as_object() else { continue };
        let visible = obj.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let etype = obj.get("type").and_then(Value::as_str).unwrap_or("shadow");
        match etype {
            "shadow" | "drop-shadow" | "drop_shadow" | "inner-shadow" | "inner_shadow" => {
                let stype = obj.get("shadowType").and_then(Value::as_str).unwrap_or(if etype.starts_with("inner") { "inner" } else { "outer" });
                let color = obj.get("color").and_then(Value::as_str).and_then(|s| resolve_color_or_var(s, doc).0).unwrap_or_else(|| Color::rgba(0, 0, 0, 64));
                let ox = obj
                    .get("offset")
                    .and_then(|o| o.get("x"))
                    .or_else(|| obj.get("offsetX"))
                    .or_else(|| obj.get("x"))
                    .and_then(|x| resolve_num_or_var(x, doc))
                    .unwrap_or(0.0);
                let oy = obj
                    .get("offset")
                    .and_then(|o| o.get("y"))
                    .or_else(|| obj.get("offsetY"))
                    .or_else(|| obj.get("y"))
                    .and_then(|y| resolve_num_or_var(y, doc))
                    .unwrap_or(4.0);
                let radius = obj.get("blur").or_else(|| obj.get("radius")).and_then(|b| resolve_num_or_var(b, doc)).unwrap_or(8.0);
                let spread = obj.get("spread").and_then(|s| resolve_num_or_var(s, doc)).unwrap_or(0.0);
                if stype == "inner" {
                    out.push(Effect::InnerShadow { color, offset: Vec2::new(ox, oy), radius, spread, visible });
                } else {
                    out.push(Effect::DropShadow { color, offset: Vec2::new(ox, oy), radius, spread, visible, show_behind: false });
                }
            }
            "blur" | "layer-blur" | "layer_blur" => {
                let radius = obj.get("radius").or_else(|| obj.get("blur")).and_then(|r| resolve_num_or_var(r, doc)).unwrap_or(4.0);
                out.push(Effect::LayerBlur { radius, visible });
            }
            "background_blur" | "background-blur" | "backdrop-blur" => {
                let radius = obj.get("radius").or_else(|| obj.get("blur")).and_then(|r| resolve_num_or_var(r, doc)).unwrap_or(8.0);
                out.push(Effect::BackgroundBlur { radius, visible });
            }
            _ => {}
        }
    }
    out
}

pub fn parse_corner_radius(r_val: Option<&Value>, doc: &Document) -> ([f32; 4], Option<String>) {
    let Some(v) = r_val else {
        return ([0.0; 4], None);
    };
    if let Some(n) = v.as_f64() {
        let r = (n as f32).max(0.0);
        return ([r; 4], None);
    }
    if let Some(s) = v.as_str() {
        if let Some(var_name) = s.strip_prefix('$') {
            let r = resolve_doc_var(doc, var_name).unwrap_or(0.0).max(0.0);
            return ([r; 4], Some(var_name.to_string()));
        }
    }
    if let Some(arr) = v.as_array() {
        let radii = match arr.len() {
            1 => {
                let r = resolve_num_or_var(&arr[0], doc).unwrap_or(0.0).max(0.0);
                [r; 4]
            }
            4 => [
                resolve_num_or_var(&arr[0], doc).unwrap_or(0.0).max(0.0),
                resolve_num_or_var(&arr[1], doc).unwrap_or(0.0).max(0.0),
                resolve_num_or_var(&arr[2], doc).unwrap_or(0.0).max(0.0),
                resolve_num_or_var(&arr[3], doc).unwrap_or(0.0).max(0.0),
            ],
            _ => [0.0; 4],
        };
        return (radii, None);
    }
    ([0.0; 4], None)
}

pub fn format_pen_color(c: Color) -> String {
    if c.a < 1.0 - 1e-4 {
        let [r, g, b, a] = c.to_u8();
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    } else {
        format!("#{}", c.to_hex())
    }
}

pub fn export_single_paint(p: &Paint, plugin_data: &BTreeMap<String, String>) -> Value {
    match &p.kind {
        PaintKind::Solid(c) => {
            let mut obj = Map::new();
            obj.insert("type".into(), json!("color"));
            obj.insert("color".into(), json!(format_pen_color(*c)));
            if (p.opacity - 1.0).abs() > 1e-4 {
                obj.insert("opacity".into(), json!(p.opacity));
            }
            if !p.visible {
                obj.insert("enabled".into(), json!(false));
            }
            if p.blend != BlendMode::Normal && p.blend != BlendMode::PassThrough {
                obj.insert("blendMode".into(), json!(blend_mode_to_pen_str(p.blend)));
            }
            Value::Object(obj)
        }
        PaintKind::LinearGradient { handles, stops }
        | PaintKind::RadialGradient { handles, stops }
        | PaintKind::AngularGradient { handles, stops }
        | PaintKind::DiamondGradient { handles, stops } => {
            let gtype = match &p.kind {
                PaintKind::RadialGradient { .. } => "radial",
                PaintKind::AngularGradient { .. } => "angular",
                PaintKind::DiamondGradient { .. } => "diamond",
                _ => "linear",
            };
            let dx = handles[1].x - handles[0].x;
            let dy = handles[1].y - handles[0].y;
            let deg = ((dy.atan2(dx).to_degrees() * 1000.0).round()) / 1000.0;
            let colors: Vec<Value> = stops
                .iter()
                .map(|st| {
                    json!({
                        "color": format_pen_color(st.color),
                        "position": ((st.pos * 10000.0).round()) / 10000.0,
                    })
                })
                .collect();
            let mut obj = Map::new();
            obj.insert("type".into(), json!("gradient"));
            obj.insert("gradientType".into(), json!(gtype));
            obj.insert("rotation".into(), json!(deg));
            obj.insert("colors".into(), Value::Array(colors));
            if (p.opacity - 1.0).abs() > 1e-4 {
                obj.insert("opacity".into(), json!(p.opacity));
            }
            if !p.visible {
                obj.insert("enabled".into(), json!(false));
            }
            Value::Object(obj)
        }
        PaintKind::Image { image_ref, scale_mode } => {
            let url = plugin_data.get("pen:image-url").cloned().unwrap_or_else(|| format!("pen:image:{image_ref}"));
            let mut obj = Map::new();
            obj.insert("type".into(), json!("image"));
            obj.insert("url".into(), json!(url));
            obj.insert("mode".into(), json!(image_scale_mode_to_pen_str(*scale_mode)));
            if (p.opacity - 1.0).abs() > 1e-4 {
                obj.insert("opacity".into(), json!(p.opacity));
            }
            if !p.visible {
                obj.insert("enabled".into(), json!(false));
            }
            Value::Object(obj)
        }
    }
}

pub fn export_paints_value(paints: &[Paint], bound_var: Option<&String>, plugin_data: &BTreeMap<String, String>) -> Option<Value> {
    if let Some(fv) = bound_var {
        if paints.len() <= 1 {
            return Some(json!(format!("${fv}")));
        }
    }
    if paints.is_empty() {
        return None;
    }
    if paints.len() == 1 {
        let p = &paints[0];
        if let PaintKind::Solid(c) = p.kind {
            if p.visible && (p.opacity - 1.0).abs() <= 1e-4 && matches!(p.blend, BlendMode::Normal | BlendMode::PassThrough) {
                return Some(json!(format_pen_color(c)));
            }
        }
        return Some(export_single_paint(p, plugin_data));
    }
    let arr: Vec<Value> = paints.iter().map(|p| export_single_paint(p, plugin_data)).collect();
    Some(Value::Array(arr))
}

pub fn export_stroke_value(stroke: &Stroke, variables: &NodeVariables, plugin_data: &BTreeMap<String, String>) -> Option<Value> {
    if stroke.paints.is_empty() {
        return None;
    }
    let mut sobj = Map::new();
    if let Some(fill_val) = export_paints_value(&stroke.paints, variables.bindings.get("strokes/0"), plugin_data) {
        sobj.insert("fill".into(), fill_val);
    }
    if let Some([t, r, b, l]) = stroke.side_weights {
        sobj.insert(
            "thickness".into(),
            json!({
                "top": t,
                "right": r,
                "bottom": b,
                "left": l,
            }),
        );
    } else if let Some(wv) = variables.bindings.get("strokeWeight") {
        sobj.insert("thickness".into(), json!(format!("${wv}")));
    } else {
        sobj.insert("thickness".into(), json!(stroke.weight));
    }
    if stroke.align != StrokeAlign::Inside {
        let align_str = match stroke.align {
            StrokeAlign::Inside => "inside",
            StrokeAlign::Center => "center",
            StrokeAlign::Outside => "outside",
        };
        sobj.insert("align".into(), json!(align_str));
        sobj.insert("strokeAlignment".into(), json!(align_str));
    }
    if stroke.cap != StrokeCap::None {
        let cap_str = match stroke.cap {
            StrokeCap::None => "none",
            StrokeCap::Round => "round",
            StrokeCap::Square => "square",
        };
        sobj.insert("cap".into(), json!(cap_str));
    }
    if stroke.join != StrokeJoin::Miter {
        let join_str = match stroke.join {
            StrokeJoin::Miter => "miter",
            StrokeJoin::Bevel => "bevel",
            StrokeJoin::Round => "round",
        };
        sobj.insert("join".into(), json!(join_str));
    }
    if !stroke.dashes.is_empty() {
        sobj.insert("dashPattern".into(), json!(stroke.dashes));
    }
    Some(Value::Object(sobj))
}
