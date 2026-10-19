//! Pencil.dev (`.pen`) JSON format importer and exporter.
//!
//! Supports `.pen` schema features:
//! - `version`, `name`, `themes`, `variables` (`color`, `number`, `string`, `boolean`, themed values)
//! - Single-page `children` or multi-page `pages`
//! - Node types: `frame`, `group`, `rectangle`, `ellipse`, `line`, `path`/`vector`,
//!   `polygon`, `star`, `text`, `icon_font`, `ref`/`instance`, `note`/`sticky`, `section`
//! - Auto-layout (`layout: "horizontal" | "vertical" | "none"`, `gap`, `padding`, `justifyContent`, `alignItems`, `clip`)
//! - Sizing (`width`/`height` numbers, `"fill_container"`, `"fit_content"`)
//! - Fills, strokes, effects, corner radii, and `$variable` bindings
//! - Components (`reusable: true`) and instances (`type: "ref"`, `ref: "<id>"`, `descendants` overrides)

use crate::svgpath::{parse_path, write_path};
use designos_doc::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// A `ref` instance waiting for its component: instance node, referenced component key, and the
/// per-descendant overrides to apply once the component subtree is cloned.
type PendingRef = (NodeId, String, Option<Map<String, Value>>);

const PEN_COLLECTION_ID: &str = "pen:tokens";

/// Parses a `.pen` (Pencil.dev) JSON string into a [`Document`].
///
/// Non-fatal import warnings (see [`import_pen_json_with_warnings`]) are written to stderr, the crate's only log sink.
pub fn import_pen_json(raw: &str) -> Result<Document, String> {
    let (doc, warning) = import_pen_json_with_warnings(raw)?;
    if let Some(w) = warning {
        eprintln!("designos-fig: .pen import warning: {w}");
    }
    Ok(doc)
}

/// Like [`import_pen_json`], but also returns the non-fatal warning, if any: the clone budget for `ref`
/// expansion ran out, so the remaining instances were left unexpanded.
pub fn import_pen_json_with_warnings(raw: &str) -> Result<(Document, Option<String>), String> {
    import_pen_json_budgeted(raw, MAX_PEN_REF_CLONES)
}

fn import_pen_json_budgeted(raw: &str, clone_budget: usize) -> Result<(Document, Option<String>), String> {
    let root: Value = serde_json::from_str(raw).map_err(|e| format!("invalid .pen JSON: {e}"))?;
    let obj = root.as_object().ok_or_else(|| "expected top-level JSON object in .pen file".to_string())?;

    let doc_name = obj.get("name").and_then(Value::as_str).unwrap_or("Pencil Document").to_string();
    let mut doc = Document::new(doc_name);

    // 1. Parse themes & variables
    import_pen_variables(&mut doc, obj);

    // 2. Parse pages or top-level children
    let mut id_map: BTreeMap<String, NodeId> = BTreeMap::new();
    let mut pending_refs: Vec<PendingRef> = Vec::new();

    if let Some(pages) = obj.get("pages").and_then(Value::as_array) {
        if pages.is_empty() {
            doc.add_page("Page 1");
        } else {
            for (idx, pval) in pages.iter().enumerate() {
                let pname = pval.get("name").and_then(Value::as_str).map(String::from).unwrap_or_else(|| format!("Page {}", idx + 1));
                let page_id = doc.add_page(pname);
                if let Some(bg_str) = pval.get("background").and_then(Value::as_str) {
                    if let Some(bg) = Color::from_hex(bg_str) {
                        if let Some(pnode) = doc.get_mut(page_id) {
                            pnode.kind = NodeKind::Page { background: bg };
                        }
                    }
                }
                if let Some(children) = pval.get("children").and_then(Value::as_array) {
                    for child_val in children {
                        import_pen_node(&mut doc, page_id, child_val, &mut id_map, &mut pending_refs)?;
                    }
                }
            }
        }
    } else {
        let page_id = doc.add_page("Page 1");
        if let Some(bg_str) = obj.get("background").and_then(Value::as_str) {
            if let Some(bg) = Color::from_hex(bg_str) {
                if let Some(pnode) = doc.get_mut(page_id) {
                    pnode.kind = NodeKind::Page { background: bg };
                }
            }
        }
        if let Some(children) = obj.get("children").and_then(Value::as_array) {
            for child_val in children {
                import_pen_node(&mut doc, page_id, child_val, &mut id_map, &mut pending_refs)?;
            }
        }
    }

    // 3. Resolve `type: "ref"` component links and clone/override descendants
    let warning = resolve_pen_refs(&mut doc, &id_map, pending_refs, clone_budget)?;

    doc.mark_full_dirty();
    Ok((doc, warning))
}

fn import_pen_variables(doc: &mut Document, obj: &Map<String, Value>) {
    let mut modes: Vec<VariableMode> = Vec::new();
    if let Some(themes) = obj.get("themes") {
        if let Some(arr) = themes.as_array() {
            for m in arr.iter().filter_map(Value::as_str) {
                modes.push(VariableMode { id: m.to_string(), name: m.to_string() });
            }
        } else if let Some(map) = themes.as_object() {
            for (_, v) in map {
                if let Some(arr) = v.as_array() {
                    for m in arr.iter().filter_map(Value::as_str) {
                        if !modes.iter().any(|existing| existing.id == m) {
                            modes.push(VariableMode { id: m.to_string(), name: m.to_string() });
                        }
                    }
                }
            }
        }
    }
    if modes.is_empty() {
        modes.push(VariableMode { id: "Default".into(), name: "Default".into() });
    }
    let default_mode = modes[0].id.clone();

    let Some(vars_obj) = obj.get("variables").and_then(Value::as_object) else {
        return;
    };
    if vars_obj.is_empty() {
        return;
    }

    let mut var_ids = Vec::new();
    for (raw_key, spec) in vars_obj {
        let var_id = raw_key.trim_start_matches('$').to_string();
        let (kind, values) = parse_pen_variable_spec(spec, &mut modes, &default_mode);
        var_ids.push(var_id.clone());
        doc.variables.insert(
            var_id.clone(),
            Variable {
                id: var_id.clone(),
                name: var_id,
                collection: PEN_COLLECTION_ID.into(),
                kind,
                values,
                description: spec.get("description").and_then(Value::as_str).unwrap_or_default().to_string(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,

                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
    }

    doc.variable_collections.insert(
        PEN_COLLECTION_ID.into(),
        VariableCollection {
            id: PEN_COLLECTION_ID.into(),
            name: "Tokens".into(),
            modes,
            default_mode,
            variables: var_ids,
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
}

fn parse_pen_variable_spec(spec: &Value, modes: &mut Vec<VariableMode>, default_mode: &str) -> (VariableKind, BTreeMap<String, VariableValue>) {
    let explicit_type = spec.get("type").and_then(Value::as_str).unwrap_or("");
    let raw_val = spec.get("value").unwrap_or(spec);

    let inferred_kind = match explicit_type.to_ascii_lowercase().as_str() {
        "color" => VariableKind::Color,
        "number" | "float" | "dimension" => VariableKind::Number,
        "boolean" | "bool" => VariableKind::Boolean,
        "string" | "text" => VariableKind::String,
        _ => infer_var_kind(raw_val),
    };

    let mut values = BTreeMap::new();
    if let Some(arr) = raw_val.as_array() {
        for item in arr {
            let mode_name = item
                .get("theme")
                .and_then(|t| t.as_str().map(String::from).or_else(|| t.as_object().and_then(|m| m.values().next()).and_then(Value::as_str).map(String::from)))
                .or_else(|| item.get("mode").and_then(Value::as_str).map(String::from))
                .unwrap_or_else(|| default_mode.to_string());
            if !modes.iter().any(|m| m.id == mode_name) {
                modes.push(VariableMode { id: mode_name.clone(), name: mode_name.clone() });
            }
            if let Some(v) = item.get("value") {
                if let Some(parsed) = parse_var_value(inferred_kind, v) {
                    values.insert(mode_name, parsed);
                }
            }
        }
    } else if let Some(map) = raw_val.as_object() {
        for (k, v) in map {
            if !modes.iter().any(|m| m.id == *k) {
                modes.push(VariableMode { id: k.clone(), name: k.clone() });
            }
            if let Some(parsed) = parse_var_value(inferred_kind, v) {
                values.insert(k.clone(), parsed);
            }
        }
    } else if let Some(parsed) = parse_var_value(inferred_kind, raw_val) {
        values.insert(default_mode.to_string(), parsed);
    }

    if values.is_empty() {
        let fallback = match inferred_kind {
            VariableKind::Color => VariableValue::Color(Color::BLACK),
            VariableKind::Number => VariableValue::Number(0.0),
            VariableKind::String => VariableValue::String(String::new()),
            VariableKind::Boolean => VariableValue::Boolean(false),
        };
        values.insert(default_mode.to_string(), fallback);
    }

    (inferred_kind, values)
}

fn infer_var_kind(v: &Value) -> VariableKind {
    match v {
        Value::Number(_) => VariableKind::Number,
        Value::Bool(_) => VariableKind::Boolean,
        Value::String(s) => {
            if s.starts_with('#') && Color::from_hex(s).is_some() {
                VariableKind::Color
            } else {
                VariableKind::String
            }
        }
        Value::Array(arr) => arr.first().and_then(|item| item.get("value")).map(infer_var_kind).unwrap_or(VariableKind::String),
        Value::Object(map) => map.values().next().map(infer_var_kind).unwrap_or(VariableKind::String),
        Value::Null => VariableKind::String,
    }
}

fn parse_var_value(kind: VariableKind, v: &Value) -> Option<VariableValue> {
    if let Some(s) = v.as_str() {
        if let Some(alias) = s.strip_prefix('$') {
            return Some(VariableValue::Alias(alias.to_string()));
        }
    }
    match kind {
        VariableKind::Color => v.as_str().and_then(Color::from_hex).map(VariableValue::Color),
        VariableKind::Number => {
            v.as_f64().map(|n| VariableValue::Number(n as f32)).or_else(|| v.as_str().and_then(|s| s.parse::<f32>().ok()).map(VariableValue::Number))
        }
        VariableKind::Boolean => v.as_bool().map(VariableValue::Boolean),
        VariableKind::String => v.as_str().map(|s| VariableValue::String(s.to_string())),
    }
}

/// Follows `$alias` links iteratively and returns the first value `pick` accepts. A cycle (or a dangling alias)
/// resolves to `None`, so callers use their fallback.
fn resolve_doc_var<T>(doc: &Document, var_name: &str, pick: impl Fn(&VariableValue) -> Option<T>) -> Option<T> {
    let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut key = var_name.trim_start_matches('$');
    loop {
        if !visited.insert(key) {
            return None;
        }
        let var = doc.variables.get(key)?;
        let mut next = None;
        for val in var.values.values() {
            if let Some(found) = pick(val) {
                return Some(found);
            }
            if let VariableValue::Alias(target) = val {
                next = Some(target.trim_start_matches('$'));
                break;
            }
        }
        key = next?;
    }
}

fn resolve_doc_var_color(doc: &Document, var_name: &str) -> Option<Color> {
    resolve_doc_var(doc, var_name, |v| match v {
        VariableValue::Color(c) => Some(*c),
        _ => None,
    })
}

fn resolve_doc_var_number(doc: &Document, var_name: &str) -> Option<f32> {
    resolve_doc_var(doc, var_name, |v| match v {
        VariableValue::Number(n) => Some(*n),
        _ => None,
    })
}

#[derive(Clone, Copy, Debug)]
enum ParsedDim {
    Fixed(f32),
    Fill(f32),
    Hug(f32),
}

fn parse_dimension(doc: &Document, v: Option<&Value>, default_px: f32) -> (ParsedDim, Option<String>) {
    let Some(val) = v else {
        return (ParsedDim::Fixed(default_px), None);
    };
    if let Some(n) = val.as_f64() {
        return (ParsedDim::Fixed(n as f32), None);
    }
    if let Some(s) = val.as_str() {
        let s = s.trim();
        if let Some(var_name) = s.strip_prefix('$') {
            let px = resolve_doc_var_number(doc, var_name).unwrap_or(default_px);
            return (ParsedDim::Fixed(px), Some(var_name.to_string()));
        }
        if s.starts_with("fill_container") || s.eq_ignore_ascii_case("fill") {
            let fallback = parse_paren_number(s).unwrap_or(default_px);
            return (ParsedDim::Fill(fallback), None);
        }
        if s.starts_with("fit_content") || s.eq_ignore_ascii_case("hug") {
            let fallback = parse_paren_number(s).unwrap_or(default_px);
            return (ParsedDim::Hug(fallback), None);
        }
        if let Ok(px) = s.trim_end_matches("px").parse::<f32>() {
            return (ParsedDim::Fixed(px), None);
        }
    }
    (ParsedDim::Fixed(default_px), None)
}

fn parse_paren_number(s: &str) -> Option<f32> {
    let start = s.find('(')? + 1;
    let end = s[start..].find(')')? + start;
    s[start..end].trim().parse::<f32>().ok()
}

fn parse_num_or_var(doc: &Document, v: Option<&Value>, default_val: f32) -> (f32, Option<String>) {
    let Some(val) = v else {
        return (default_val, None);
    };
    if let Some(n) = val.as_f64() {
        return (n as f32, None);
    }
    if let Some(s) = val.as_str() {
        let s = s.trim();
        if let Some(var_name) = s.strip_prefix('$') {
            let resolved = resolve_doc_var_number(doc, var_name).unwrap_or(default_val);
            return (resolved, Some(var_name.to_string()));
        }
        if let Ok(parsed) = s.parse::<f32>() {
            return (parsed, None);
        }
    }
    (default_val, None)
}

fn parse_padding(doc: &Document, v: Option<&Value>) -> ([f32; 4], Vec<(&'static str, String)>) {
    let mut bindings = Vec::new();
    let Some(val) = v else {
        return ([0.0; 4], bindings);
    };
    if let Some(arr) = val.as_array() {
        match arr.len() {
            1 => {
                let (p, b) = parse_num_or_var(doc, arr.first(), 0.0);
                if let Some(var) = b {
                    for k in ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"] {
                        bindings.push((k, var.clone()));
                    }
                }
                ([p, p, p, p], bindings)
            }
            2 => {
                let (pv, bv) = parse_num_or_var(doc, arr.first(), 0.0);
                let (ph, bh) = parse_num_or_var(doc, arr.get(1), 0.0);
                if let Some(var) = bv {
                    bindings.push(("paddingTop", var.clone()));
                    bindings.push(("paddingBottom", var));
                }
                if let Some(var) = bh {
                    bindings.push(("paddingRight", var.clone()));
                    bindings.push(("paddingLeft", var));
                }
                ([pv, ph, pv, ph], bindings)
            }
            4 => {
                let (pt, bt) = parse_num_or_var(doc, arr.first(), 0.0);
                let (pr, br) = parse_num_or_var(doc, arr.get(1), 0.0);
                let (pb, bb) = parse_num_or_var(doc, arr.get(2), 0.0);
                let (pl, bl) = parse_num_or_var(doc, arr.get(3), 0.0);
                if let Some(var) = bt {
                    bindings.push(("paddingTop", var));
                }
                if let Some(var) = br {
                    bindings.push(("paddingRight", var));
                }
                if let Some(var) = bb {
                    bindings.push(("paddingBottom", var));
                }
                if let Some(var) = bl {
                    bindings.push(("paddingLeft", var));
                }
                ([pt, pr, pb, pl], bindings)
            }
            _ => ([0.0; 4], bindings),
        }
    } else {
        let (p, b) = parse_num_or_var(doc, Some(val), 0.0);
        if let Some(var) = b {
            for k in ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"] {
                bindings.push((k, var.clone()));
            }
        }
        ([p, p, p, p], bindings)
    }
}

fn parse_layout_align(s: Option<&str>) -> LayoutAlign {
    match s.unwrap_or("start").to_ascii_lowercase().as_str() {
        "center" | "middle" => LayoutAlign::Center,
        "end" | "flex_end" | "flex-end" | "right" | "bottom" => LayoutAlign::End,
        "space_between" | "space-between" => LayoutAlign::SpaceBetween,
        "space_around" | "space-around" => LayoutAlign::SpaceAround,
        "space_evenly" | "space-evenly" => LayoutAlign::SpaceEvenly,
        "baseline" => LayoutAlign::Baseline,
        _ => LayoutAlign::Start,
    }
}

fn parse_paints(doc: &Document, val: Option<&Value>, binding_prefix: &str) -> (Vec<Paint>, Vec<(String, String)>) {
    let mut paints = Vec::new();
    let mut bindings = Vec::new();
    let Some(v) = val else {
        return (paints, bindings);
    };

    let items: Vec<&Value> = if let Some(arr) = v.as_array() { arr.iter().collect() } else { vec![v] };

    for (idx, item) in items.into_iter().enumerate() {
        if let Some(s) = item.as_str() {
            let s = s.trim();
            if let Some(var_name) = s.strip_prefix('$') {
                bindings.push((format!("{binding_prefix}/{idx}"), var_name.to_string()));
                let c = resolve_doc_var_color(doc, var_name).unwrap_or(Color::BLACK);
                paints.push(Paint::solid(c));
            } else if let Some(c) = Color::from_hex(s) {
                paints.push(Paint::solid(c));
            }
        } else if let Some(obj) = item.as_object() {
            let visible = obj.get("enabled").or_else(|| obj.get("visible")).and_then(Value::as_bool).unwrap_or(true);
            let opacity = obj.get("opacity").and_then(Value::as_f64).unwrap_or(1.0) as f32;
            let kind_str = obj.get("type").and_then(Value::as_str).unwrap_or("solid").to_ascii_lowercase();
            match kind_str.as_str() {
                "linear" | "linear_gradient" | "linear-gradient" => {
                    let stops = parse_gradient_stops(doc, obj.get("stops"));
                    paints.push(Paint {
                        kind: PaintKind::LinearGradient { handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)], stops },
                        opacity,
                        visible,
                        blend: BlendMode::Normal,
                    });
                }
                "radial" | "radial_gradient" | "radial-gradient" => {
                    let stops = parse_gradient_stops(doc, obj.get("stops"));
                    paints.push(Paint {
                        kind: PaintKind::RadialGradient { handles: [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)], stops },
                        opacity,
                        visible,
                        blend: BlendMode::Normal,
                    });
                }
                "image" => {
                    let image_ref = obj.get("url").or_else(|| obj.get("imageRef")).and_then(Value::as_str).unwrap_or("").to_string();
                    paints.push(Paint { kind: PaintKind::Image { image_ref, scale_mode: ImageScaleMode::Fill }, opacity, visible, blend: BlendMode::Normal });
                }
                _ => {
                    if let Some(color_str) = obj.get("color").or_else(|| obj.get("fill")).and_then(Value::as_str) {
                        let color_str = color_str.trim();
                        if let Some(var_name) = color_str.strip_prefix('$') {
                            bindings.push((format!("{binding_prefix}/{idx}"), var_name.to_string()));
                            let c = resolve_doc_var_color(doc, var_name).unwrap_or(Color::BLACK);
                            paints.push(Paint { kind: PaintKind::Solid(c), opacity, visible, blend: BlendMode::Normal });
                        } else if let Some(c) = Color::from_hex(color_str) {
                            paints.push(Paint { kind: PaintKind::Solid(c), opacity, visible, blend: BlendMode::Normal });
                        }
                    }
                }
            }
        }
    }

    (paints, bindings)
}

fn parse_gradient_stops(doc: &Document, stops_val: Option<&Value>) -> Vec<GradientStop> {
    let Some(arr) = stops_val.and_then(Value::as_array) else {
        return vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }];
    };
    let mut out = Vec::new();
    for s in arr {
        let pos = s.get("position").or_else(|| s.get("offset")).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let color_str = s.get("color").and_then(Value::as_str).unwrap_or("#000000");
        let color = if let Some(vn) = color_str.strip_prefix('$') {
            resolve_doc_var_color(doc, vn).unwrap_or(Color::BLACK)
        } else {
            Color::from_hex(color_str).unwrap_or(Color::BLACK)
        };
        out.push(GradientStop { pos, color });
    }
    out
}

fn parse_stroke(doc: &Document, val: Option<&Value>) -> (Stroke, Vec<(String, String)>) {
    let mut stroke = Stroke { weight: 1.0, miter_limit: 4.0, ..Default::default() };
    let mut bindings = Vec::new();
    let Some(v) = val else {
        return (stroke, bindings);
    };
    if v.is_string() {
        let (paints, b) = parse_paints(doc, Some(v), "strokes");
        stroke.paints = paints;
        return (stroke, b);
    }
    if let Some(obj) = v.as_object() {
        let paint_val = obj.get("fill").or_else(|| obj.get("color")).or_else(|| obj.get("paints"));
        let (paints, b) = parse_paints(doc, paint_val, "strokes");
        stroke.paints = paints;
        bindings.extend(b);
        if let Some(t) = obj.get("thickness").or_else(|| obj.get("weight")) {
            if let Some(w) = t.as_f64() {
                stroke.weight = w as f32;
            } else if let Some(tobj) = t.as_object() {
                let top = tobj.get("top").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let right = tobj.get("right").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let bottom = tobj.get("bottom").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let left = tobj.get("left").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                stroke.side_weights = Some([top, right, bottom, left]);
                stroke.weight = top.max(right).max(bottom).max(left).max(1.0);
            }
        }
        if let Some(align) = obj.get("align").and_then(Value::as_str) {
            stroke.align = match align.to_ascii_lowercase().as_str() {
                "center" => StrokeAlign::Center,
                "outside" => StrokeAlign::Outside,
                _ => StrokeAlign::Inside,
            };
        }
    }
    (stroke, bindings)
}

fn parse_effects(doc: &Document, val: Option<&Value>) -> Vec<Effect> {
    let Some(v) = val else {
        return Vec::new();
    };
    let items: Vec<&Value> = if let Some(arr) = v.as_array() { arr.iter().collect() } else { vec![v] };
    let mut out = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else { continue };
        let visible = obj.get("enabled").or_else(|| obj.get("visible")).and_then(Value::as_bool).unwrap_or(true);
        let ty = obj.get("type").and_then(Value::as_str).unwrap_or("shadow").to_ascii_lowercase();
        let radius = obj.get("blur").or_else(|| obj.get("radius")).and_then(Value::as_f64).unwrap_or(4.0) as f32;
        let spread = obj.get("spread").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let ox = obj.get("offset").and_then(|o| o.get("x")).or_else(|| obj.get("x")).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let oy = obj.get("offset").and_then(|o| o.get("y")).or_else(|| obj.get("y")).and_then(Value::as_f64).unwrap_or(2.0) as f32;
        let color_str = obj.get("color").and_then(Value::as_str).unwrap_or("#00000040");
        let color = if let Some(vn) = color_str.strip_prefix('$') {
            resolve_doc_var_color(doc, vn).unwrap_or(Color::rgba(0, 0, 0, 64))
        } else {
            Color::from_hex(color_str).unwrap_or(Color::rgba(0, 0, 0, 64))
        };
        match ty.as_str() {
            "inner_shadow" | "inner-shadow" => {
                out.push(Effect::InnerShadow { color, offset: Vec2::new(ox, oy), radius, spread, visible });
            }
            "blur" | "layer_blur" | "layer-blur" => {
                out.push(Effect::LayerBlur { radius, visible });
            }
            "background_blur" | "background-blur" => {
                out.push(Effect::BackgroundBlur { radius, visible });
            }
            _ => {
                let inner = obj.get("shadowType").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case("inner"));
                if inner {
                    out.push(Effect::InnerShadow { color, offset: Vec2::new(ox, oy), radius, spread, visible });
                } else {
                    out.push(Effect::DropShadow { color, offset: Vec2::new(ox, oy), radius, spread, visible, show_behind: false });
                }
            }
        }
    }
    out
}

fn parse_corner_radius(doc: &Document, val: Option<&Value>) -> ([f32; 4], Vec<(String, String)>) {
    let mut bindings = Vec::new();
    let Some(v) = val else {
        return ([0.0; 4], bindings);
    };
    if let Some(arr) = v.as_array() {
        let keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
        let mut radii = [0.0; 4];
        for i in 0..4 {
            let (r, b) = parse_num_or_var(doc, arr.get(i), 0.0);
            radii[i] = r;
            if let Some(var) = b {
                bindings.push((keys[i].to_string(), var));
            }
        }
        (radii, bindings)
    } else {
        let (r, b) = parse_num_or_var(doc, Some(v), 0.0);
        if let Some(var) = b {
            bindings.push(("cornerRadius".into(), var.clone()));
            bindings.push(("topLeftRadius".into(), var));
        }
        ([r; 4], bindings)
    }
}

fn parse_frame_props(doc: &Document, obj: &Map<String, Value>, w_dim: ParsedDim, h_dim: ParsedDim) -> (FrameProps, Vec<(String, String)>) {
    let mut bindings = Vec::new();
    let clips_content = obj.get("clip").or_else(|| obj.get("clipsContent")).and_then(Value::as_bool).unwrap_or(false);
    let layout_str = obj.get("layout").and_then(Value::as_str).map(|s| s.to_ascii_lowercase());
    let has_layout_props = obj.contains_key("gap") || obj.contains_key("padding") || obj.contains_key("justifyContent") || obj.contains_key("alignItems");

    let direction = match layout_str.as_deref() {
        Some("none") => None,
        Some("vertical") | Some("column") => Some(LayoutDirection::Vertical),
        Some("horizontal") | Some("row") => Some(LayoutDirection::Horizontal),
        Some("grid") => Some(LayoutDirection::Grid),
        None if has_layout_props => Some(LayoutDirection::Horizontal),
        _ => None,
    };

    let auto_layout = direction.map(|dir| {
        let (gap, gap_var) = parse_num_or_var(doc, obj.get("gap").or_else(|| obj.get("itemSpacing")), 0.0);
        if let Some(var) = gap_var {
            bindings.push(("itemSpacing".into(), var));
        }
        let (padding, pad_vars) = parse_padding(doc, obj.get("padding"));
        for (k, v) in pad_vars {
            bindings.push((k.to_string(), v));
        }
        let primary_align = parse_layout_align(obj.get("justifyContent").and_then(Value::as_str));
        let counter_align = parse_layout_align(obj.get("alignItems").and_then(Value::as_str));
        let map_sizing = |d: ParsedDim| match d {
            ParsedDim::Fixed(_) => Sizing::Fixed,
            ParsedDim::Fill(_) => Sizing::Fill,
            ParsedDim::Hug(_) => Sizing::Hug,
        };
        let (primary_sizing, counter_sizing) = match dir {
            LayoutDirection::Horizontal | LayoutDirection::Grid => (map_sizing(w_dim), map_sizing(h_dim)),
            LayoutDirection::Vertical => (map_sizing(h_dim), map_sizing(w_dim)),
        };
        AutoLayout {
            direction: dir,
            wrap: obj.get("wrap").and_then(Value::as_bool).unwrap_or(false),
            item_spacing: gap,
            counter_spacing: 0.0,
            padding,
            primary_align,
            counter_align,
            primary_sizing,
            counter_sizing,
            ..Default::default()
        }
    });

    (FrameProps { clips_content, auto_layout, grids: Vec::new() }, bindings)
}

fn import_pen_node(
    doc: &mut Document,
    parent: NodeId,
    val: &Value,
    id_map: &mut BTreeMap<String, NodeId>,
    pending_refs: &mut Vec<PendingRef>,
) -> Result<NodeId, String> {
    let obj = val.as_object().ok_or_else(|| "expected node object in .pen".to_string())?;
    let id = doc.alloc_id();
    let raw_type = obj.get("type").and_then(Value::as_str).unwrap_or("frame").to_ascii_lowercase();
    let reusable = obj.get("reusable").and_then(Value::as_bool).unwrap_or(false);

    let (w_dim, w_var) = parse_dimension(doc, obj.get("width"), 100.0);
    let (h_dim, h_var) = parse_dimension(doc, obj.get("height"), 100.0);
    let px_w = match w_dim {
        ParsedDim::Fixed(v) | ParsedDim::Fill(v) | ParsedDim::Hug(v) => v,
    };
    let px_h = match h_dim {
        ParsedDim::Fixed(v) | ParsedDim::Fill(v) | ParsedDim::Hug(v) => v,
    };

    let (frame_props, mut var_bindings) = parse_frame_props(doc, obj, w_dim, h_dim);
    if let Some(v) = w_var {
        var_bindings.push(("width".into(), v));
    }
    if let Some(v) = h_var {
        var_bindings.push(("height".into(), v));
    }

    let (fills, fill_vars) = parse_paints(doc, obj.get("fill").or_else(|| obj.get("fills")), "fills");
    var_bindings.extend(fill_vars);

    let kind = match raw_type.as_str() {
        "frame" => {
            if reusable {
                NodeKind::Component(frame_props)
            } else {
                NodeKind::Frame(frame_props)
            }
        }
        "component" => NodeKind::Component(frame_props),
        "section" => NodeKind::Section,
        "group" => NodeKind::Group,
        "rectangle" | "rect" => NodeKind::Rectangle,
        "ellipse" | "circle" => NodeKind::Ellipse(ArcData::default()),
        "line" => NodeKind::Line,
        "polygon" => {
            let count = obj.get("polygonCount").or_else(|| obj.get("sides")).and_then(Value::as_u64).unwrap_or(3) as u32;
            NodeKind::Polygon { count }
        }
        "star" => {
            let count = obj.get("polygonCount").or_else(|| obj.get("points")).and_then(Value::as_u64).unwrap_or(5) as u32;
            let inner_ratio = obj.get("innerRadius").and_then(Value::as_f64).unwrap_or(0.382) as f32;
            NodeKind::Star { count, inner_ratio }
        }
        "path" | "vector" => {
            let geom = obj.get("geometry").or_else(|| obj.get("d")).and_then(Value::as_str).unwrap_or("");
            let fill_rule = match obj.get("fillRule").and_then(Value::as_str) {
                Some("evenodd") => FillRule::EvenOdd,
                _ => FillRule::NonZero,
            };
            let paths = if geom.is_empty() { Vec::new() } else { vec![parse_path(geom, fill_rule)] };
            NodeKind::Vector { paths, network: None }
        }
        "text" | "icon_font" => {
            let content =
                obj.get("content").or_else(|| obj.get("characters")).or_else(|| obj.get("iconFontName")).and_then(Value::as_str).unwrap_or("").to_string();
            let font_family = obj.get("fontFamily").or_else(|| obj.get("iconFontFamily")).and_then(Value::as_str).unwrap_or("Inter").to_string();
            let (font_size, fs_var) = parse_num_or_var(doc, obj.get("fontSize"), 14.0);
            if let Some(v) = fs_var {
                var_bindings.push(("fontSize".into(), v));
            }
            let font_weight = match obj.get("fontWeight") {
                Some(Value::Number(n)) => n.as_u64().unwrap_or(400) as u16,
                Some(Value::String(s)) => match s.to_ascii_lowercase().as_str() {
                    "thin" => 100,
                    "extralight" | "ultralight" => 200,
                    "light" => 300,
                    "normal" | "regular" => 400,
                    "medium" => 500,
                    "semibold" | "demibold" => 600,
                    "bold" => 700,
                    "extrabold" | "ultrabold" => 800,
                    "black" | "heavy" => 900,
                    other => other.parse::<u16>().unwrap_or(400),
                },
                _ => 400,
            };
            let italic = obj.get("fontStyle").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case("italic"));
            let line_height = match obj.get("lineHeight") {
                Some(Value::Number(n)) => {
                    let v = n.as_f64().unwrap_or(1.2) as f32;
                    if v <= 4.0 {
                        LineHeight::Percent(v * 100.0)
                    } else {
                        LineHeight::Pixels(v)
                    }
                }
                _ => LineHeight::Auto,
            };
            let letter_spacing = obj.get("letterSpacing").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let align_h = match obj.get("textAlign").and_then(Value::as_str).unwrap_or("left") {
                "center" => TextAlignH::Center,
                "right" => TextAlignH::Right,
                "justify" | "justified" => TextAlignH::Justified,
                _ => TextAlignH::Left,
            };
            let align_v = match obj.get("textAlignVertical").and_then(Value::as_str).unwrap_or("top") {
                "middle" | "center" => TextAlignV::Center,
                "bottom" => TextAlignV::Bottom,
                _ => TextAlignV::Top,
            };
            let text_fills = if fills.is_empty() { vec![Paint::solid(Color::BLACK)] } else { fills.clone() };
            let auto_resize = match obj.get("textGrowth").and_then(Value::as_str) {
                Some("fixed-width") => TextAutoResize::Height,
                Some("fixed-width-height") => TextAutoResize::None,
                _ => TextAutoResize::WidthAndHeight,
            };
            NodeKind::Text(TextProps {
                content,
                style: TextStyle {
                    font_family,
                    font_weight,
                    italic,
                    font_size,
                    line_height,
                    letter_spacing,
                    align_h,
                    align_v,
                    fills: text_fills,
                    ..Default::default()
                },
                auto_resize,
                ..Default::default()
            })
        }
        "ref" | "instance" => {
            let ref_target = obj.get("ref").or_else(|| obj.get("componentId")).and_then(Value::as_str).unwrap_or("").to_string();
            let descendants = obj.get("descendants").and_then(Value::as_object).cloned();
            pending_refs.push((id, ref_target.clone(), descendants));
            NodeKind::Instance { frame: frame_props, component: None, component_key: if ref_target.is_empty() { None } else { Some(ref_target) } }
        }
        "note" | "sticky" => {
            let content = obj.get("content").or_else(|| obj.get("text")).and_then(Value::as_str).unwrap_or("").to_string();
            let mut sp = StickyProps::default();
            sp.text.content = content;
            if let Some(Paint { kind: PaintKind::Solid(c), .. }) = fills.first() {
                sp.color = *c;
            }
            NodeKind::Sticky(sp)
        }
        _ => {
            if reusable {
                NodeKind::Component(frame_props)
            } else {
                NodeKind::Frame(frame_props)
            }
        }
    };

    let mut node = Node::new(id, kind);
    if let Some(src_id) = obj.get("id").and_then(Value::as_str) {
        node.source_id = Some(src_id.to_string());
        id_map.insert(src_id.to_string(), id);
    }
    node.name = obj.get("name").and_then(Value::as_str).or_else(|| obj.get("id").and_then(Value::as_str)).unwrap_or(node.kind.default_name()).to_string();
    node.visible = obj.get("enabled").or_else(|| obj.get("visible")).and_then(Value::as_bool).unwrap_or(true);
    let (opacity, op_var) = parse_num_or_var(doc, obj.get("opacity"), 1.0);
    node.opacity = opacity;
    if let Some(v) = op_var {
        var_bindings.push(("opacity".into(), v));
    }

    let x = obj.get("x").and_then(Value::as_f64).unwrap_or(0.0) as f32;
    let y = obj.get("y").and_then(Value::as_f64).unwrap_or(0.0) as f32;
    let rot_deg = obj.get("rotation").and_then(Value::as_f64).unwrap_or(0.0) as f32;
    node.transform = if rot_deg.abs() > 1e-4 { Affine::translate(x, y) * Affine::rotate((-rot_deg).to_radians()) } else { Affine::translate(x, y) };
    node.size = Vec2::new(px_w, px_h);

    node.layout_child.horizontal = match w_dim {
        ParsedDim::Fixed(_) => Sizing::Fixed,
        ParsedDim::Fill(_) => Sizing::Fill,
        ParsedDim::Hug(_) => Sizing::Hug,
    };
    node.layout_child.vertical = match h_dim {
        ParsedDim::Fixed(_) => Sizing::Fixed,
        ParsedDim::Fill(_) => Sizing::Fill,
        ParsedDim::Hug(_) => Sizing::Hug,
    };

    if !matches!(node.kind, NodeKind::Text(_)) {
        node.fills = fills;
    }

    let (stroke, stroke_vars) = parse_stroke(doc, obj.get("stroke"));
    node.stroke = stroke;
    var_bindings.extend(stroke_vars);

    node.effects = parse_effects(doc, obj.get("effect").or_else(|| obj.get("effects")));

    let (radii, radii_vars) = parse_corner_radius(doc, obj.get("cornerRadius"));
    node.corner_radii = radii;
    var_bindings.extend(radii_vars);

    for (k, v) in var_bindings {
        node.variables.bindings.insert(k, v);
    }

    if let Some(theme_obj) = obj.get("theme").and_then(Value::as_object) {
        if let Some(mode_val) = theme_obj.values().next().and_then(Value::as_str) {
            node.variables.modes.insert(PEN_COLLECTION_ID.into(), mode_val.to_string());
        }
    }

    doc.insert(parent, node).map_err(|e| e.to_string())?;

    if let Some(children) = obj.get("children").and_then(Value::as_array) {
        for child_val in children {
            import_pen_node(doc, id, child_val, id_map, pending_refs)?;
        }
    }

    Ok(id)
}

/// Nodes the whole import may clone while expanding `ref`s; stops chained components from multiplying without bound.
const MAX_PEN_REF_CLONES: usize = 2_000_000;

/// Expands every `ref`. When expanding the next one would pass `clone_budget` nodes, that instance and all
/// later ones are left unexpanded (everything cloned so far stays) and a warning is returned.
/// Self and ancestor references are still hard errors, also after the budget ran out.
fn resolve_pen_refs(
    doc: &mut Document,
    id_map: &BTreeMap<String, NodeId>,
    pending_refs: Vec<PendingRef>,
    clone_budget: usize,
) -> Result<Option<String>, String> {
    let mut budget_left = clone_budget;
    let mut skipped = 0usize;
    for (inst_id, ref_key, descendants) in pending_refs {
        let Some(&comp_id) = id_map.get(&ref_key) else {
            continue;
        };
        if comp_id == inst_id || doc.is_ancestor(comp_id, inst_id) {
            return Err(format!("component \"{ref_key}\" is referenced from inside itself; remove the ref from its own contents"));
        }
        // Once one instance was left unexpanded, every later one is too: do not even size its component.
        if skipped > 0 {
            skipped += 1;
            continue;
        }
        let has_explicit_children = doc.get(inst_id).is_some_and(|n| !n.children.is_empty());
        // Clone component's children into instance if instance had no explicit children
        let comp_children = if has_explicit_children { Vec::new() } else { doc.get(comp_id).map(|c| c.children.clone()).unwrap_or_default() };
        let needed = subtree_len_up_to(doc, &comp_children, budget_left);
        if needed > budget_left {
            skipped += 1;
            continue;
        }
        budget_left -= needed;
        if let Some(inst_node) = doc.get_mut(inst_id) {
            if let NodeKind::Instance { component, .. } = &mut inst_node.kind {
                *component = Some(comp_id);
            }
        }
        let mut source_to_clone: BTreeMap<String, NodeId> = BTreeMap::new();
        for cid in comp_children {
            clone_component_subtree(doc, cid, inst_id, &mut source_to_clone);
        }
        // Apply `descendants` overrides
        if let Some(desc_map) = descendants {
            for (desc_path, ov_val) in desc_map {
                let target_key = desc_path.rsplit('/').next().unwrap_or(&desc_path);
                let target_id = source_to_clone.get(&desc_path).or_else(|| source_to_clone.get(target_key)).copied();
                let Some(tid) = target_id else { continue };
                let Some(ov_obj) = ov_val.as_object() else { continue };
                apply_descendant_override(doc, tid, ov_obj);
            }
        }
    }
    Ok((skipped > 0).then(|| {
        format!("component references expand past {clone_budget} cloned nodes; {skipped} instance(s) were left unexpanded (flatten nested components)")
    }))
}

/// Number of nodes in the subtrees rooted at `roots`, counting stops once it exceeds `limit` (the result is then
/// some value above `limit`, not the true size). Iterative, so depth cannot overflow the stack.
fn subtree_len_up_to(doc: &Document, roots: &[NodeId], limit: usize) -> usize {
    let mut stack: Vec<NodeId> = roots.to_vec();
    let mut count = 0usize;
    while let Some(id) = stack.pop() {
        if let Some(n) = doc.get(id) {
            count += 1;
            if count > limit {
                break;
            }
            stack.extend(n.children.iter().copied());
        }
    }
    count
}

/// Clones the subtree rooted at `src_id` under `new_parent`, parents before children and siblings in order.
/// Iterative (explicit stack), so a component nested thousands of levels deep cannot overflow the call stack.
fn clone_component_subtree(doc: &mut Document, src_id: NodeId, new_parent: NodeId, source_to_clone: &mut BTreeMap<String, NodeId>) {
    let mut stack: Vec<(NodeId, NodeId)> = vec![(src_id, new_parent)];
    while let Some((src_id, new_parent)) = stack.pop() {
        let Some(orig) = doc.get(src_id).cloned() else { continue };
        let new_id = doc.alloc_id();
        let mut copy = orig.clone();
        copy.id = new_id;
        copy.parent = Some(new_parent);
        copy.children = Vec::new();
        copy.component.source = Some(src_id);
        if let Some(sid) = &orig.source_id {
            source_to_clone.insert(sid.clone(), new_id);
        }
        let _ = doc.insert(new_parent, copy);
        // Reversed so the first child is popped (and so allocated and inserted) first.
        stack.extend(orig.children.into_iter().rev().map(|child| (child, new_id)));
    }
}

fn apply_descendant_override(doc: &mut Document, tid: NodeId, ov_obj: &Map<String, Value>) {
    let (fills, fill_vars) = parse_paints(doc, ov_obj.get("fill").or_else(|| ov_obj.get("fills")), "fills");
    let Some(n) = doc.get_mut(tid) else { return };
    if let Some(name) = ov_obj.get("name").and_then(Value::as_str) {
        n.name = name.to_string();
        n.component.overrides.insert(OverrideField::Name);
    }
    if let Some(en) = ov_obj.get("enabled").or_else(|| ov_obj.get("visible")).and_then(Value::as_bool) {
        n.visible = en;
        n.component.overrides.insert(OverrideField::Visible);
    }
    if let Some(w) = ov_obj.get("width").and_then(Value::as_f64) {
        n.size.x = w as f32;
        n.component.overrides.insert(OverrideField::Size);
    }
    if let Some(h) = ov_obj.get("height").and_then(Value::as_f64) {
        n.size.y = h as f32;
        n.component.overrides.insert(OverrideField::Size);
    }
    if let Some(content) = ov_obj.get("content").or_else(|| ov_obj.get("characters")).and_then(Value::as_str) {
        if let Some(t) = n.kind.any_text_mut() {
            t.content = content.to_string();
            n.component.overrides.insert(OverrideField::Characters);
        }
    }
    if !fills.is_empty() {
        if let Some(t) = n.kind.any_text_mut() {
            t.style.fills = fills;
            n.component.overrides.insert(OverrideField::TextStyle);
        } else {
            n.fills = fills;
            n.component.overrides.insert(OverrideField::Fills);
        }
        for (k, v) in fill_vars {
            n.variables.bindings.insert(k, v);
        }
    }
}

/// Serializes a [`Document`] into a Pencil.dev `.pen` JSON string.
pub fn export_pen_json(doc: &Document) -> Result<String, String> {
    let mut root = Map::new();
    root.insert("version".into(), json!("1.0"));
    root.insert("name".into(), json!(doc.name));

    // Export themes & variables if present
    if !doc.variables.is_empty() {
        let mut mode_names = Vec::new();
        for coll in doc.variable_collections.values() {
            for m in &coll.modes {
                if !mode_names.contains(&m.name) {
                    mode_names.push(m.name.clone());
                }
            }
        }
        if mode_names.len() > 1 {
            root.insert("themes".into(), json!({ "mode": mode_names }));
        }

        let mut vars_map = Map::new();
        for (id, var) in &doc.variables {
            let type_str = match var.kind {
                VariableKind::Color => "color",
                VariableKind::Number => "number",
                VariableKind::String => "string",
                VariableKind::Boolean => "boolean",
            };
            let val_json = if var.values.len() <= 1 {
                var.values.values().next().map(export_var_value).unwrap_or(Value::Null)
            } else {
                let arr: Vec<Value> = var
                    .values
                    .iter()
                    .map(|(mode_id, v)| {
                        json!({
                            "theme": { "mode": mode_id },
                            "value": export_var_value(v),
                        })
                    })
                    .collect();
                Value::Array(arr)
            };
            vars_map.insert(
                id.clone(),
                json!({
                    "type": type_str,
                    "value": val_json,
                }),
            );
        }
        root.insert("variables".into(), Value::Object(vars_map));
    }

    if let Some(&first_page) = doc.pages.first() {
        if let Some(NodeKind::Page { background }) = doc.get(first_page).map(|n| &n.kind) {
            root.insert("background".into(), json!(format!("#{}", background.to_hex())));
        }
        let children: Vec<Value> = doc.get(first_page).map(|p| p.children.iter().filter_map(|&cid| export_pen_node(doc, cid)).collect()).unwrap_or_default();
        root.insert("children".into(), Value::Array(children));
    } else {
        root.insert("children".into(), Value::Array(Vec::new()));
    }

    if doc.pages.len() > 1 {
        let pages_json: Vec<Value> = doc
            .pages
            .iter()
            .filter_map(|&pid| {
                let p = doc.get(pid)?;
                let bg = match &p.kind {
                    NodeKind::Page { background } => format!("#{}", background.to_hex()),
                    _ => "#5A5A5A".to_string(),
                };
                let children: Vec<Value> = p.children.iter().filter_map(|&cid| export_pen_node(doc, cid)).collect();
                Some(json!({
                    "id": node_pen_id(p),
                    "name": p.name,
                    "background": bg,
                    "children": children,
                }))
            })
            .collect();
        root.insert("pages".into(), Value::Array(pages_json));
    }

    serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())
}

fn export_var_value(v: &VariableValue) -> Value {
    match v {
        VariableValue::Color(c) => json!(format!("#{}", c.to_hex())),
        VariableValue::Number(n) => json!(n),
        VariableValue::String(s) => json!(s),
        VariableValue::Boolean(b) => json!(b),
        VariableValue::Alias(a) => json!(format!("${a}")),
    }
}

fn node_pen_id(n: &Node) -> String {
    n.source_id.clone().unwrap_or_else(|| format!("n{}", n.id.0))
}

fn export_pen_node(doc: &Document, id: NodeId) -> Option<Value> {
    let n = doc.get(id)?;
    let mut obj = Map::new();
    obj.insert("id".into(), json!(node_pen_id(n)));

    let (type_str, is_reusable) = match &n.kind {
        NodeKind::Frame(_) => ("frame", false),
        NodeKind::Component(_) => ("frame", true),
        NodeKind::ComponentSet(_) => ("frame", true),
        NodeKind::Instance { .. } => ("ref", false),
        NodeKind::Group => ("group", false),
        NodeKind::Section => ("section", false),
        NodeKind::Rectangle => ("rectangle", false),
        NodeKind::Ellipse(_) => ("ellipse", false),
        NodeKind::Line => ("line", false),
        NodeKind::Polygon { .. } => ("polygon", false),
        NodeKind::Star { .. } => ("star", false),
        NodeKind::Vector { .. } => ("path", false),
        NodeKind::Text(_) => ("text", false),
        NodeKind::Sticky(_) => ("note", false),
        _ => ("frame", false),
    };
    obj.insert("type".into(), json!(type_str));
    if is_reusable {
        obj.insert("reusable".into(), json!(true));
    }
    obj.insert("name".into(), json!(n.name));

    if !n.visible {
        obj.insert("enabled".into(), json!(false));
    }
    if (n.opacity - 1.0).abs() > 1e-4 {
        obj.insert("opacity".into(), json!(n.opacity));
    }

    obj.insert("x".into(), json!(n.transform.tx));
    obj.insert("y".into(), json!(n.transform.ty));

    let export_dim = |s: Sizing, px: f32, var_opt: Option<&String>| -> Value {
        if let Some(vn) = var_opt {
            return json!(format!("${vn}"));
        }
        match s {
            Sizing::Fill => json!(format!("fill_container({px})")),
            Sizing::Hug => json!(format!("fit_content({px})")),
            Sizing::Fixed => json!(px),
        }
    };
    obj.insert("width".into(), export_dim(n.layout_child.horizontal, n.size.x, n.variables.bindings.get("width")));
    obj.insert("height".into(), export_dim(n.layout_child.vertical, n.size.y, n.variables.bindings.get("height")));

    if let Some(fp) = n.kind.frame_props() {
        if fp.clips_content {
            obj.insert("clip".into(), json!(true));
        }
        if let Some(al) = &fp.auto_layout {
            let lstr = match al.direction {
                LayoutDirection::Horizontal => "horizontal",
                LayoutDirection::Vertical => "vertical",
                LayoutDirection::Grid => "grid",
            };
            obj.insert("layout".into(), json!(lstr));
            if let Some(gap_var) = n.variables.bindings.get("itemSpacing") {
                obj.insert("gap".into(), json!(format!("${gap_var}")));
            } else if al.item_spacing != 0.0 {
                obj.insert("gap".into(), json!(al.item_spacing));
            }
            if al.padding != [0.0; 4] {
                if al.padding[0] == al.padding[1] && al.padding[1] == al.padding[2] && al.padding[2] == al.padding[3] {
                    obj.insert("padding".into(), json!(al.padding[0]));
                } else {
                    obj.insert("padding".into(), json!(al.padding));
                }
            }
            let align_str = |a: LayoutAlign| match a {
                LayoutAlign::Start => "start",
                LayoutAlign::Center => "center",
                LayoutAlign::End => "end",
                LayoutAlign::SpaceBetween => "space_between",
                LayoutAlign::SpaceAround => "space_around",
                LayoutAlign::SpaceEvenly => "space_evenly",
                LayoutAlign::Baseline => "baseline",
            };
            if al.primary_align != LayoutAlign::Start {
                obj.insert("justifyContent".into(), json!(align_str(al.primary_align)));
            }
            if al.counter_align != LayoutAlign::Start {
                obj.insert("alignItems".into(), json!(align_str(al.counter_align)));
            }
        } else {
            obj.insert("layout".into(), json!("none"));
        }
    }

    // Fills
    let active_fills = if let NodeKind::Text(t) = &n.kind { &t.style.fills } else { &n.fills };
    if let Some(fill_var) = n.variables.bindings.get("fills/0") {
        obj.insert("fill".into(), json!(format!("${fill_var}")));
    } else if active_fills.len() == 1 {
        if let PaintKind::Solid(c) = active_fills[0].kind {
            obj.insert("fill".into(), json!(format!("#{}", c.to_hex())));
        }
    } else if active_fills.len() > 1 {
        let arr: Vec<Value> = active_fills
            .iter()
            .filter_map(|p| match &p.kind {
                PaintKind::Solid(c) => Some(json!({
                    "type": "color",
                    "color": format!("#{}", c.to_hex()),
                    "enabled": p.visible,
                })),
                _ => None,
            })
            .collect();
        obj.insert("fill".into(), Value::Array(arr));
    }

    // Stroke
    if !n.stroke.paints.is_empty() {
        if let Some(PaintKind::Solid(c)) = n.stroke.paints.first().map(|p| &p.kind) {
            let color_val = if let Some(sv) = n.variables.bindings.get("strokes/0") { format!("${sv}") } else { format!("#{}", c.to_hex()) };
            obj.insert(
                "stroke".into(),
                json!({
                    "fill": color_val,
                    "thickness": n.stroke.weight,
                }),
            );
        }
    }

    // Corner radius
    if let Some(r_var) = n.variables.bindings.get("cornerRadius") {
        obj.insert("cornerRadius".into(), json!(format!("${r_var}")));
    } else if n.corner_radii != [0.0; 4] {
        let r0 = n.corner_radii[0];
        if n.corner_radii.iter().all(|&r| (r - r0).abs() < 1e-4) {
            obj.insert("cornerRadius".into(), json!(r0));
        } else {
            obj.insert("cornerRadius".into(), json!(n.corner_radii));
        }
    }

    // Kind-specific properties
    match &n.kind {
        NodeKind::Polygon { count } => {
            obj.insert("polygonCount".into(), json!(count));
        }
        NodeKind::Star { count, inner_ratio } => {
            obj.insert("polygonCount".into(), json!(count));
            obj.insert("innerRadius".into(), json!(inner_ratio));
        }
        NodeKind::Vector { paths, .. } => {
            if let Some(first) = paths.first() {
                obj.insert("geometry".into(), json!(write_path(first)));
            }
        }
        NodeKind::Text(t) => {
            obj.insert("content".into(), json!(t.content));
            obj.insert("fontFamily".into(), json!(t.style.font_family));
            obj.insert("fontSize".into(), json!(t.style.font_size));
            obj.insert("fontWeight".into(), json!(t.style.font_weight));
            if t.style.italic {
                obj.insert("fontStyle".into(), json!("italic"));
            }
        }
        NodeKind::Sticky(s) => {
            obj.insert("content".into(), json!(s.text.content));
        }
        NodeKind::Instance { component, component_key, .. } => {
            let ref_id = component.and_then(|cid| doc.get(cid)).map(node_pen_id).or_else(|| component_key.clone()).unwrap_or_default();
            obj.insert("ref".into(), json!(ref_id));
            let mut desc_map = Map::new();
            collect_instance_descendant_overrides(doc, n, &mut desc_map);
            if !desc_map.is_empty() {
                obj.insert("descendants".into(), Value::Object(desc_map));
            }
        }
        _ => {}
    }

    // Export children (skip cloned instance children if `ref` handles them via `descendants`)
    if !matches!(n.kind, NodeKind::Instance { component: Some(_), .. }) && !n.children.is_empty() {
        let children: Vec<Value> = n.children.iter().filter_map(|&cid| export_pen_node(doc, cid)).collect();
        obj.insert("children".into(), Value::Array(children));
    }

    Some(Value::Object(obj))
}

fn collect_instance_descendant_overrides(doc: &Document, inst_node: &Node, out: &mut Map<String, Value>) {
    for &cid in &inst_node.children {
        let Some(cn) = doc.get(cid) else { continue };
        if !cn.component.overrides.is_empty() {
            if let Some(src_id) = cn.component.source.and_then(|s| doc.get(s)).map(node_pen_id) {
                let mut ov = Map::new();
                if cn.component.overrides.contains(&OverrideField::Characters) {
                    if let Some(t) = cn.kind.any_text() {
                        ov.insert("content".into(), json!(t.content));
                    }
                }
                if cn.component.overrides.contains(&OverrideField::Fills) {
                    if let Some(PaintKind::Solid(c)) = cn.fills.first().map(|p| &p.kind) {
                        ov.insert("fill".into(), json!(format!("#{}", c.to_hex())));
                    }
                }
                if cn.component.overrides.contains(&OverrideField::Size) {
                    ov.insert("width".into(), json!(cn.size.x));
                    ov.insert("height".into(), json!(cn.size.y));
                }
                if cn.component.overrides.contains(&OverrideField::Visible) {
                    ov.insert("enabled".into(), json!(cn.visible));
                }
                if !ov.is_empty() {
                    out.insert(src_id, Value::Object(ov));
                }
            }
        }
        collect_instance_descendant_overrides(doc, cn, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pen_import_and_export_roundtrip() {
        let sample_pen = r##"{
            "version": "1.0",
            "name": "Design System Pen",
            "themes": {
                "mode": ["Light", "Dark"]
            },
            "variables": {
                "primary": {
                    "type": "color",
                    "value": [
                        { "theme": { "mode": "Light" }, "value": "#3B82F6" },
                        { "theme": { "mode": "Dark" }, "value": "#60A5FA" }
                    ]
                },
                "radius-md": {
                    "type": "number",
                    "value": 12
                }
            },
            "children": [
                {
                    "id": "btn-comp",
                    "type": "frame",
                    "reusable": true,
                    "name": "Primary Button",
                    "x": 20,
                    "y": 40,
                    "width": "fit_content(160)",
                    "height": 48,
                    "layout": "horizontal",
                    "gap": 8,
                    "padding": [12, 24],
                    "justifyContent": "center",
                    "alignItems": "center",
                    "fill": "$primary",
                    "cornerRadius": "$radius-md",
                    "children": [
                        {
                            "id": "btn-label",
                            "type": "text",
                            "name": "Label",
                            "content": "Click Me",
                            "fontFamily": "Inter",
                            "fontSize": 16,
                            "fontWeight": 600,
                            "fill": "#FFFFFF"
                        }
                    ]
                },
                {
                    "id": "btn-inst",
                    "type": "ref",
                    "ref": "btn-comp",
                    "name": "CTA Instance",
                    "x": 220,
                    "y": 40,
                    "width": "fill_container(200)",
                    "height": 48,
                    "descendants": {
                        "btn-label": {
                            "content": "Get Started"
                        }
                    }
                }
            ]
        }"##;

        let doc = import_pen_json(sample_pen).expect("import .pen should succeed");
        assert_eq!(doc.name, "Design System Pen");
        assert!(doc.variables.contains_key("primary"));
        assert!(doc.variables.contains_key("radius-md"));

        let page = doc.pages[0];
        let page_node = doc.get(page).unwrap();
        assert_eq!(page_node.children.len(), 2);

        let comp_id = page_node.children[0];
        let comp_node = doc.get(comp_id).unwrap();
        assert!(matches!(comp_node.kind, NodeKind::Component(_)));
        assert_eq!(comp_node.variables.bindings.get("fills/0").map(String::as_str), Some("primary"));
        assert_eq!(comp_node.corner_radii, [12.0; 4]);

        let inst_id = page_node.children[1];
        let inst_node = doc.get(inst_id).unwrap();
        assert!(matches!(inst_node.kind, NodeKind::Instance { component: Some(cid), .. } if cid == comp_id));
        assert_eq!(inst_node.layout_child.horizontal, Sizing::Fill);
        assert_eq!(inst_node.children.len(), 1);
        let inst_label = doc.get(inst_node.children[0]).unwrap();
        assert_eq!(inst_label.kind.any_text().unwrap().content, "Get Started");

        // Round-trip through export_pen_json -> import_pen_json
        let exported = export_pen_json(&doc).expect("export .pen should succeed");
        let reimported = import_pen_json(&exported).expect("re-import .pen should succeed");
        assert_eq!(reimported.name, "Design System Pen");
        assert_eq!(reimported.variables.len(), 2);
        let rpage = reimported.pages[0];
        let rchildren = &reimported.get(rpage).unwrap().children;
        assert_eq!(rchildren.len(), 2);
        let rinst = reimported.get(rchildren[1]).unwrap();
        let rlabel = reimported.get(rinst.children[0]).unwrap();
        assert_eq!(rlabel.kind.any_text().unwrap().content, "Get Started");
    }

    #[test]
    fn ref_to_own_component_or_itself_is_rejected() {
        let inside_component = r##"{"version":"1.0","children":[
            {"id":"c","type":"frame","reusable":true,"children":[{"id":"i","type":"ref","ref":"c"}]}]}"##;
        let err = import_pen_json(inside_component).unwrap_err();
        assert!(err.contains("referenced from inside itself"), "{err}");

        let deep = r##"{"version":"1.0","children":[
            {"id":"c","type":"frame","reusable":true,"children":[{"id":"g","type":"frame","children":[{"id":"i","type":"ref","ref":"c"}]}]}]}"##;
        assert!(import_pen_json(deep).is_err());

        let self_ref = r##"{"version":"1.0","children":[{"id":"i","type":"ref","ref":"i"}]}"##;
        assert!(import_pen_json(self_ref).is_err());
    }

    #[test]
    fn cyclic_variable_aliases_fall_back_instead_of_recursing() {
        let json = r##"{"version":"1.0",
            "variables":{"a":{"type":"number","value":"$b"},"b":{"type":"number","value":"$a"},
                         "c":{"type":"color","value":"$d"},"d":{"type":"color","value":"$c"},
                         "self":{"type":"number","value":"$self"}},
            "children":[
                {"id":"n","type":"frame","width":"$a","height":"$self","fill":"$c"}]}"##;
        let doc = import_pen_json(json).expect("a variable cycle must not fail the import");
        let n = doc.get(doc.get(doc.pages[0]).unwrap().children[0]).unwrap();
        // Unresolved: both dimensions fall back to the default size, the fill to black.
        let (w, h) = (n.size.x, n.size.y);
        assert!(w.is_finite() && h.is_finite());
        assert_eq!(n.variables.bindings.get("fills/0").map(String::as_str), Some("c"));
    }

    #[test]
    fn long_acyclic_alias_chain_still_resolves() {
        let mut vars = vec![r#""v0":{"type":"number","value":42}"#.to_string()];
        vars.extend((1..2000).map(|i| format!(r#""v{i}":{{"type":"number","value":"$v{}"}}"#, i - 1)));
        let json = format!(r##"{{"version":"1.0","variables":{{{}}},"children":[{{"id":"n","type":"frame","width":"$v1999","height":10}}]}}"##, vars.join(","));
        let doc = import_pen_json(&json).unwrap();
        let n = doc.get(doc.get(doc.pages[0]).unwrap().children[0]).unwrap();
        assert_eq!(n.size.x, 42.0);
    }

    #[test]
    fn chained_component_refs_hit_the_clone_budget() {
        // c23 holds two refs to c24, c22 two refs to c23, ... declared deepest-first so every
        // level clones an already-expanded subtree: 2^24 nodes without a budget.
        let mut children = vec![r##"{"id":"c24","type":"frame","reusable":true,"children":[]}"##.to_string()];
        for i in (0..24).rev() {
            let next = i + 1;
            children.push(format!(
                r##"{{"id":"c{i}","type":"frame","reusable":true,"children":[{{"id":"a{i}","type":"ref","ref":"c{next}"}},{{"id":"b{i}","type":"ref","ref":"c{next}"}}]}}"##
            ));
        }
        let json = format!(r##"{{"version":"1.0","children":[{}]}}"##, children.join(","));
        let (doc, warning) = import_pen_json_budgeted(&json, 1_000).expect("an exhausted clone budget must not fail the import");
        let warning = warning.expect("budget exhaustion is surfaced as a warning");
        assert!(warning.contains("expand past 1000"), "{warning}");
        // The nodes imported before the budget ran out are all still there, and cloning stopped at the budget.
        assert!(doc.get(doc.pages[0]).unwrap().children.len() >= 25);
        assert!(doc.nodes.len() < 1_000 + 200, "got {} nodes", doc.nodes.len());
    }

    #[test]
    fn exhausted_clone_budget_keeps_earlier_instances_and_skips_later_ones() {
        let mut kids = vec![r##"{"id":"comp","type":"frame","reusable":true,"children":[
            {"id":"l1","type":"frame"},{"id":"l2","type":"frame"},{"id":"l3","type":"frame"}]}"##
            .to_string()];
        kids.extend((0..5).map(|i| format!(r##"{{"id":"i{i}","type":"ref","ref":"comp"}}"##)));
        let json = format!(r##"{{"version":"1.0","children":[{}]}}"##, kids.join(","));

        // Budget 7 fits two 3-node expansions; the third needs 3 but only 1 is left.
        let (doc, warning) = import_pen_json_budgeted(&json, 7).unwrap();
        let page = doc.get(doc.pages[0]).unwrap();
        let counts: Vec<usize> = page.children[1..].iter().map(|&i| doc.get(i).unwrap().children.len()).collect();
        assert_eq!(counts, [3, 3, 0, 0, 0]);
        let w = warning.unwrap();
        assert!(w.contains("3 instance(s)"), "{w}");

        // Enough budget: everything expands and there is no warning.
        let (doc, warning) = import_pen_json_budgeted(&json, 15).unwrap();
        assert!(warning.is_none());
        let page = doc.get(doc.pages[0]).unwrap();
        assert!(page.children[1..].iter().all(|&i| doc.get(i).unwrap().children.len() == 3));

        // Self references stay errors even after the budget ran out.
        let json = format!(r##"{{"version":"1.0","children":[{},{{"id":"s","type":"ref","ref":"s"}}]}}"##, kids.join(","));
        assert!(import_pen_json_budgeted(&json, 7).is_err());
    }

    #[test]
    fn public_warning_entry_point_reports_none_for_a_small_document() {
        let json = r##"{"version":"1.0","children":[{"id":"c","type":"frame","reusable":true},{"id":"i","type":"ref","ref":"c"}]}"##;
        assert!(import_pen_json_with_warnings(json).unwrap().1.is_none());
        assert!(import_pen_json(json).is_ok());
    }

    #[test]
    fn subtree_count_stops_once_it_passes_the_limit() {
        let kids: Vec<String> = (0..50).map(|i| format!(r##"{{"id":"k{i}","type":"frame"}}"##)).collect();
        let json = format!(r##"{{"version":"1.0","children":[{{"id":"c","type":"frame","reusable":true,"children":[{}]}}]}}"##, kids.join(","));
        let (doc, _) = import_pen_json_budgeted(&json, 1_000).unwrap();
        let comp = doc.get(doc.pages[0]).unwrap().children[0];
        let roots = doc.get(comp).unwrap().children.clone();
        assert_eq!(subtree_len_up_to(&doc, &roots, 1_000), 50);
        assert_eq!(subtree_len_up_to(&doc, &roots, 50), 50);
        assert_eq!(subtree_len_up_to(&doc, &roots, 10), 11, "the walk gives up one node past the limit");
        assert_eq!(subtree_len_up_to(&doc, &roots, 0), 1);
    }

    #[test]
    fn skipped_refs_do_not_re_walk_a_huge_component() {
        // 20,000-node component, 3,000 refs, budget 10: every ref is skipped. Sizing the component per skipped ref
        // costs 60M node visits (seconds in a debug build); skipping without sizing costs next to nothing.
        let kids: Vec<String> = (0..20_000).map(|i| format!(r##"{{"id":"k{i}","type":"frame"}}"##)).collect();
        let refs: Vec<String> = (0..3_000).map(|i| format!(r##"{{"id":"r{i}","type":"ref","ref":"comp"}}"##)).collect();
        let json =
            format!(r##"{{"version":"1.0","children":[{{"id":"comp","type":"frame","reusable":true,"children":[{}]}},{}]}}"##, kids.join(","), refs.join(","));
        let started = std::time::Instant::now();
        let (doc, warning) = import_pen_json_budgeted(&json, 10).unwrap();
        let elapsed = started.elapsed();
        assert!(warning.expect("budget exhaustion is surfaced as a warning").contains("3000 instance(s)"));
        assert_eq!(doc.nodes.len(), 1 + 1 + 20_000 + 3_000, "nothing was cloned");
        assert!(elapsed < std::time::Duration::from_millis(1_500), "importing took {elapsed:?}");
    }
}
