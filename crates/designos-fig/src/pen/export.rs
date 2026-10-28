//! Exporter from [`Document`] to `.pen` schema `2.21` JSON.

use super::migrate::CURRENT_PEN_VERSION;
use super::paint::{blend_mode_to_pen_str, export_paints_value, export_stroke_value_filtered, format_pen_color};
use super::variables::{export_themes_and_variables, splice_ordered_themes_and_variables};
use crate::svgpath::write_path;
use designos_doc::*;
use serde_json::{json, Map, Value};

pub fn node_pen_id(n: &Node) -> String {
    n.source_id.clone().unwrap_or_else(|| format!("n{}", n.id.0))
}

/// Summary of `.pen` scene format export side-effects (such as omitted transient `in_progress` flags, E15-7 §6).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PenExportReport {
    /// Active `in_progress` root IDs left out of the `.pen` scene format export.
    pub omitted_in_progress_ids: Vec<NodeId>,
    /// Number of active `in_progress` roots left out of the `.pen` scene format export.
    pub omitted_in_progress_count: usize,
}

impl PenExportReport {
    /// Human-readable summary note when transient `in_progress` flags were omitted from `.pen` export.
    pub fn summary_note(&self) -> Option<String> {
        (self.omitted_in_progress_count > 0)
            .then(|| format!("Omitted {} in_progress flag(s) from scene format export ({:?})", self.omitted_in_progress_count, self.omitted_in_progress_ids))
    }
}

pub fn export_pen_json(doc: &Document) -> Result<String, String> {
    export_pen_json_with_report(doc).map(|(json, _)| json)
}

/// Like [`export_pen_json`], also returning a [`PenExportReport`] listing any active `in_progress`
/// roots omitted because `.pen` does not carry agent work-in-progress state (`placeholder` has a
/// separate component meaning and is never written from `in_progress`, E15-7 §6).
pub fn export_pen_json_with_report(doc: &Document) -> Result<(String, PenExportReport), String> {
    let mut root = Map::new();
    root.insert("version".into(), Value::String(CURRENT_PEN_VERSION.into()));
    if !doc.name.is_empty() {
        root.insert("name".into(), Value::String(doc.name.clone()));
    }

    if let Some(&first_page) = doc.pages.first() {
        if let Some(raw_imports) = doc.get(first_page).and_then(|p| p.plugin_data.get("pen:imports")) {
            if let Ok(imports_val) = serde_json::from_str::<Value>(raw_imports) {
                root.insert("imports".into(), imports_val);
            }
        }
    }

    export_themes_and_variables(doc, &mut root);

    let first_page_children: Vec<Value> =
        doc.pages.first().and_then(|&pid| doc.get(pid)).map(|p| p.children.iter().filter_map(|&cid| export_pen_node(doc, cid)).collect()).unwrap_or_default();
    root.insert("children".into(), Value::Array(first_page_children));

    if doc.pages.len() > 1 {
        let mut pages_arr = Vec::with_capacity(doc.pages.len());
        for &page_id in &doc.pages {
            if let Some(page) = doc.get(page_id) {
                let mut pobj = Map::new();
                pobj.insert("id".into(), Value::String(node_pen_id(page)));
                pobj.insert("name".into(), Value::String(page.name.clone()));
                if let NodeKind::Page { background } = page.kind {
                    if background != Color::rgb(0xF5, 0xF5, 0xF5) {
                        pobj.insert("background".into(), Value::String(format_pen_color(background)));
                    }
                }
                let pchildren: Vec<Value> = page.children.iter().filter_map(|&cid| export_pen_node(doc, cid)).collect();
                pobj.insert("children".into(), Value::Array(pchildren));
                pages_arr.push(Value::Object(pobj));
            }
        }
        root.insert("pages".into(), Value::Array(pages_arr));
    }

    let omitted_in_progress_ids: Vec<NodeId> = doc.in_progress.keys().copied().filter(|&id| doc.get_in_progress(id).is_some()).collect();
    let report = PenExportReport { omitted_in_progress_count: omitted_in_progress_ids.len(), omitted_in_progress_ids };
    let pretty = serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| format!("Failed to serialize .pen JSON: {e}"))?;
    Ok((splice_ordered_themes_and_variables(doc, pretty), report))
}

pub fn export_pen_node(doc: &Document, id: NodeId) -> Option<Value> {
    let n = doc.get(id)?;

    // Spec §9: Unknown node round-trips its original raw JSON payload verbatim (plus updated children).
    if let NodeKind::Unknown { type_name } = &n.kind {
        let mut obj =
            n.plugin_data.get("pen:raw").and_then(|s| serde_json::from_str::<Value>(s).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_else(|| {
                let mut m = Map::new();
                m.insert("id".into(), Value::String(node_pen_id(n)));
                m.insert("type".into(), Value::String(type_name.clone()));
                m.insert("name".into(), Value::String(n.name.clone()));
                m
            });
        obj.insert("id".into(), Value::String(node_pen_id(n)));
        if !n.children.is_empty() {
            let children: Vec<Value> = n.children.iter().filter_map(|&cid| export_pen_node(doc, cid)).collect();
            obj.insert("children".into(), Value::Array(children));
        }
        return Some(Value::Object(obj));
    }

    // Spec §5: Unwrap a reusable non-frame wrapper back into its primitive node with `reusable: true`.
    if matches!(n.kind, NodeKind::Component(_)) && n.plugin_data.get("pen:reusable_wrapper").map(String::as_str) == Some("true") && n.children.len() == 1 {
        let inner_id = n.children[0];
        if let Some(Value::Object(mut inner_obj)) = export_pen_node(doc, inner_id) {
            inner_obj.insert("id".into(), Value::String(node_pen_id(n)));
            inner_obj.insert("name".into(), Value::String(n.name.clone()));
            inner_obj.insert("reusable".into(), Value::Bool(true));
            if n.transform.tx.abs() > 1e-4 {
                inner_obj.insert("x".into(), json!(n.transform.tx));
            } else {
                inner_obj.remove("x");
            }
            if n.transform.ty.abs() > 1e-4 {
                inner_obj.insert("y".into(), json!(n.transform.ty));
            } else {
                inner_obj.remove("y");
            }
            return Some(Value::Object(inner_obj));
        }
    }

    let mut obj = Map::new();
    obj.insert("id".into(), Value::String(node_pen_id(n)));

    let is_icon_node = n.plugin_data.contains_key("pen:icon");
    let is_instance = matches!(n.kind, NodeKind::Instance { .. });
    let is_frame_like = matches!(n.kind, NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_)) && !is_icon_node;

    let (pen_type, is_reusable) = if is_icon_node {
        ("icon_font", matches!(n.kind, NodeKind::Component(_)))
    } else {
        match &n.kind {
            NodeKind::Frame(_) => ("frame", false),
            NodeKind::Component(_) | NodeKind::ComponentSet(_) => ("frame", true),
            NodeKind::Group => ("group", false),
            NodeKind::Section => ("section", false),
            NodeKind::Rectangle => ("rectangle", false),
            NodeKind::Ellipse(_) => ("ellipse", false),
            NodeKind::Line => ("line", false),
            NodeKind::Polygon { .. } | NodeKind::Star { .. } => ("polygon", false),
            NodeKind::Vector { .. } => ("path", false),
            NodeKind::Text(_) => ("text", false),
            NodeKind::Sticky(_) => ("note", false),
            NodeKind::Instance { .. } => ("ref", false),
            NodeKind::Connector(_) => ("connection", false),
            NodeKind::BooleanOp(_) => ("boolean", false),
            NodeKind::Page { .. } | NodeKind::ShapeWithText(_) | NodeKind::Stamp { .. } => ("frame", false),
            NodeKind::Unknown { .. } => unreachable!(),
        }
    };

    obj.insert("type".into(), Value::String(pen_type.into()));
    if is_reusable {
        obj.insert("reusable".into(), Value::Bool(true));
    }
    if !n.name.is_empty() {
        obj.insert("name".into(), Value::String(n.name.clone()));
    }
    if !n.visible {
        if let Some(vis_var) = n.variables.bindings.get("visible") {
            obj.insert("enabled".into(), json!(format!("${vis_var}")));
        } else {
            obj.insert("enabled".into(), Value::Bool(false));
        }
    }
    if let Some(op_var) = n.variables.bindings.get("opacity") {
        obj.insert("opacity".into(), json!(format!("${op_var}")));
    } else if (n.opacity - 1.0).abs() > 1e-4 {
        obj.insert("opacity".into(), json!(n.opacity));
    }
    if n.blend != BlendMode::PassThrough {
        obj.insert("blendMode".into(), json!(blend_mode_to_pen_str(n.blend)));
    }
    if let Some(href) = &n.hyperlink {
        obj.insert("href".into(), json!(href));
    }
    if n.layout_child.absolute {
        obj.insert("layoutPosition".into(), json!("absolute"));
    }
    if n.plugin_data.get("pen:placeholder").map(String::as_str) == Some("true") {
        obj.insert("placeholder".into(), Value::Bool(true));
    }
    if n.component.is_slot {
        if let Some(raw_slot) = n.plugin_data.get("pen:slot") {
            if let Ok(slot_val) = serde_json::from_str::<Value>(raw_slot) {
                obj.insert("slot".into(), slot_val);
            }
        } else {
            obj.insert("slot".into(), Value::Array(Vec::new()));
        }
    }

    // Position, rotation, flipX, flipY
    if n.transform.tx.abs() > 1e-4 {
        obj.insert("x".into(), json!(n.transform.tx));
    }
    if n.transform.ty.abs() > 1e-4 {
        obj.insert("y".into(), json!(n.transform.ty));
    }
    if let Some(flip_raw) = n.plugin_data.get("pen:flip") {
        if let Ok(flip_obj) = serde_json::from_str::<Value>(flip_raw) {
            if flip_obj.get("flipX").and_then(Value::as_bool) == Some(true) {
                obj.insert("flipX".into(), Value::Bool(true));
            }
            if flip_obj.get("flipY").and_then(Value::as_bool) == Some(true) {
                obj.insert("flipY".into(), Value::Bool(true));
            }
            if let Some(rot) = flip_obj.get("rotation").and_then(Value::as_f64) {
                if rot.abs() > 1e-3 {
                    obj.insert("rotation".into(), json!(rot));
                }
            }
        }
    } else {
        let flip_x = n.transform.a < 0.0 && n.transform.b.abs() < 1e-4;
        let flip_y = n.transform.d < 0.0 && n.transform.c.abs() < 1e-4;
        if flip_x {
            obj.insert("flipX".into(), Value::Bool(true));
        }
        if flip_y {
            obj.insert("flipY".into(), Value::Bool(true));
        }
        if !flip_x && !flip_y {
            let rot_deg = (-n.transform.rotation_rad()).to_degrees();
            if rot_deg.abs() > 1e-3 {
                obj.insert("rotation".into(), json!(((rot_deg * 1000.0).round()) / 1000.0));
            }
        }
    }

    // Sizing / dimensions (skip default dimensions on Connector nodes)
    let is_connector = matches!(n.kind, NodeKind::Connector(_));
    let should_write_size = !is_connector && (!is_instance || n.component.overrides.contains(&OverrideField::Size));
    if should_write_size {
        let has_explicit_w = n.plugin_data.get("pen:explicit_width").map(String::as_str) == Some("true");
        let has_explicit_h = n.plugin_data.get("pen:explicit_height").map(String::as_str) == Some("true");
        let (write_w, write_h) = if !is_instance {
            (true, true)
        } else if has_explicit_w || has_explicit_h {
            (has_explicit_w, has_explicit_h)
        } else {
            (true, true)
        };
        write_node_dimensions(n, is_frame_like, &mut obj, write_w, write_h);
    }

    // Node-level theme override (`theme: { axis: val }`)
    if !n.variables.modes.is_empty() {
        let mut theme_obj = Map::new();
        for (col_id, mode_id) in &n.variables.modes {
            if let Some((axis, val)) = mode_id.split_once(':') {
                theme_obj.insert(axis.to_string(), Value::String(val.to_string()));
            } else if let Some(col) = doc.variable_collections.get(col_id) {
                let mname = col.modes.iter().find(|m| m.id == *mode_id).map(|m| m.name.clone()).unwrap_or_else(|| mode_id.clone());
                theme_obj.insert(col.name.clone(), Value::String(mname));
            }
        }
        if !theme_obj.is_empty() {
            obj.insert("theme".into(), Value::Object(theme_obj));
        }
    }

    // AutoLayout for Frame / Component (or overridden Instance)
    let should_write_layout = is_frame_like || (is_instance && n.component.overrides.contains(&OverrideField::Layout));
    if should_write_layout {
        if let Some(fp) = n.kind.frame_props() {
            if fp.clips_content {
                obj.insert("clip".into(), Value::Bool(true));
            }
            if let Some(al) = &fp.auto_layout {
                let dir_str = match al.direction {
                    LayoutDirection::Horizontal => "horizontal",
                    LayoutDirection::Vertical => "vertical",
                    LayoutDirection::Grid => "grid",
                };
                obj.insert("layout".into(), json!(dir_str));
                if al.wrap {
                    obj.insert("wrap".into(), Value::Bool(true));
                }
                if let Some(g_var) = n.variables.bindings.get("itemSpacing") {
                    obj.insert("gap".into(), json!(format!("${g_var}")));
                } else if al.item_spacing > 0.0 {
                    obj.insert("gap".into(), json!(al.item_spacing));
                }
                if let Some(pad_val) = export_padding_value(&al.padding, &n.variables) {
                    obj.insert("padding".into(), pad_val);
                }
                if al.primary_align != LayoutAlign::Start {
                    obj.insert("justifyContent".into(), json!(align_to_pen_str(al.primary_align)));
                }
                if al.counter_align != LayoutAlign::Start {
                    obj.insert("alignItems".into(), json!(align_to_pen_str(al.counter_align)));
                }
            } else {
                obj.insert("layout".into(), json!("none"));
            }
        }
    }

    let icon_stroke_from_fill = is_icon_node && n.plugin_data.get("pen:icon_stroke_from_fill").map(String::as_str) == Some("true");

    // Fills
    let should_write_fill = !is_instance || n.component.overrides.contains(&OverrideField::Fills);
    if should_write_fill {
        let (active_fills, fill_binding) = if let NodeKind::Text(t) = &n.kind {
            (&t.style.fills, n.variables.bindings.get("fills/0"))
        } else if icon_stroke_from_fill && n.fills.is_empty() {
            (&n.stroke.paints, n.variables.bindings.get("strokes/0").or_else(|| n.variables.bindings.get("fills/0")))
        } else {
            (&n.fills, n.variables.bindings.get("fills/0"))
        };
        if let Some(fill_val) = export_paints_value(active_fills, fill_binding, &n.plugin_data) {
            obj.insert("fill".into(), fill_val);
        }
    }

    // Stroke
    let should_write_stroke = !icon_stroke_from_fill && (!is_instance || n.component.overrides.contains(&OverrideField::Stroke));
    if should_write_stroke {
        if let Some(stroke_val) = export_stroke_value_filtered(&n.stroke, &n.variables, &n.plugin_data, is_instance) {
            obj.insert("stroke".into(), stroke_val);
        }
    }

    // Corner radius
    let should_write_radius = !is_instance || n.component.overrides.contains(&OverrideField::CornerRadii);
    if should_write_radius {
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
    }

    // Effects
    if !n.effects.is_empty() && (!is_instance || n.component.overrides.contains(&OverrideField::Effects)) {
        let eff_arr: Vec<Value> = n.effects.iter().filter_map(export_effect).collect();
        if eff_arr.len() == 1 {
            obj.insert("effect".into(), eff_arr.into_iter().next().unwrap());
        } else if !eff_arr.is_empty() {
            obj.insert("effect".into(), Value::Array(eff_arr));
        }
    }

    // Icon metadata
    if let Some(icon_raw) = n.plugin_data.get("pen:icon") {
        if let Ok(icon_meta) = serde_json::from_str::<Value>(icon_raw) {
            if let Some(fam) = icon_meta.get("family").and_then(Value::as_str) {
                obj.insert("iconFontFamily".into(), json!(fam));
            }
            if let Some(iname) = icon_meta.get("name").and_then(Value::as_str) {
                obj.insert("iconFontName".into(), json!(iname));
            }
            if let Some(w) = icon_meta.get("weight").and_then(Value::as_u64) {
                obj.insert("weight".into(), json!(w));
            }
        }
    }

    // Kind-specific properties
    if !is_icon_node {
        match &n.kind {
            NodeKind::Ellipse(arc) => {
                if *arc != ArcData::default() {
                    let start_deg = ((arc.start.to_degrees() * 1000.0).round()) / 1000.0;
                    let sweep_deg = (((arc.end - arc.start).to_degrees() * 1000.0).round()) / 1000.0;
                    if start_deg.abs() > 1e-3 {
                        obj.insert("startAngle".into(), json!(start_deg));
                    }
                    if (sweep_deg - 360.0).abs() > 1e-3 {
                        obj.insert("sweepAngle".into(), json!(sweep_deg));
                    }
                    if arc.inner_radius > 1e-4 {
                        obj.insert("innerRadius".into(), json!(arc.inner_radius));
                    }
                }
            }
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
                    if first.fill_rule == FillRule::EvenOdd {
                        obj.insert("fillRule".into(), json!("evenodd"));
                    }
                }
            }
            NodeKind::Text(t) => {
                export_text_fields(n, t, &mut obj);
            }
            NodeKind::Sticky(s) => {
                obj.insert("content".into(), json!(s.text.content));
            }
            NodeKind::Connector(conn) => {
                obj.insert("from".into(), export_connector_end(doc, &conn.start));
                obj.insert("to".into(), export_connector_end(doc, &conn.end));
                if let Some(lbl) = &conn.label {
                    if !lbl.content.is_empty() {
                        obj.insert("content".into(), json!(lbl.content));
                    }
                }
            }
            NodeKind::Instance { component, component_key, .. } => {
                let ref_id = component_key.clone().or_else(|| component.and_then(|cid| doc.get(cid)).map(node_pen_id)).unwrap_or_default();
                obj.insert("ref".into(), json!(ref_id));
                let mut desc_map = Map::new();
                collect_instance_descendant_overrides(doc, n, &mut desc_map);
                if !desc_map.is_empty() {
                    obj.insert("descendants".into(), Value::Object(desc_map));
                }
            }
            _ => {}
        }
    }

    // Export children (skip cloned instance children if `ref` handles them via `descendants`)
    if !matches!(n.kind, NodeKind::Instance { component: Some(_), .. }) && !n.children.is_empty() {
        let children: Vec<Value> = n.children.iter().filter_map(|&cid| export_pen_node(doc, cid)).collect();
        obj.insert("children".into(), Value::Array(children));
    }

    Some(Value::Object(obj))
}

fn export_padding_value(padding: &[f32; 4], nv: &NodeVariables) -> Option<Value> {
    let pt = nv.bindings.get("paddingTop");
    let pr = nv.bindings.get("paddingRight");
    let pb = nv.bindings.get("paddingBottom");
    let pl = nv.bindings.get("paddingLeft");
    if let (Some(t), Some(r), Some(b), Some(l)) = (pt, pr, pb, pl) {
        if t == r && r == b && b == l {
            return Some(json!(format!("${t}")));
        }
    }
    if *padding != [0.0; 4] {
        if padding[0] == padding[1] && padding[1] == padding[2] && padding[2] == padding[3] {
            return Some(json!(padding[0]));
        }
        return Some(json!(padding));
    }
    None
}

fn export_connector_end(doc: &Document, end: &ConnectorEnd) -> Value {
    if let Some(target_id) = end.node {
        let sid = doc.get(target_id).map(node_pen_id).unwrap_or_else(|| format!("n{}", target_id.0));
        match end.magnet {
            Magnet::Auto => Value::String(sid),
            m => {
                let m_str = match m {
                    Magnet::Top => "top",
                    Magnet::Right => "right",
                    Magnet::Bottom => "bottom",
                    Magnet::Left => "left",
                    Magnet::Center => "center",
                    Magnet::Auto => "auto",
                };
                json!({ "id": sid, "magnet": m_str })
            }
        }
    } else {
        json!({ "x": end.point.x, "y": end.point.y })
    }
}

fn write_node_dimensions(n: &Node, is_frame_like: bool, obj: &mut Map<String, Value>, write_w: bool, write_h: bool) {
    let is_text = matches!(n.kind, NodeKind::Text(_));
    let mut h_sizing = n.layout_child.horizontal;
    let mut v_sizing = n.layout_child.vertical;

    if let Some(al) = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) {
        if h_sizing == Sizing::Fixed && al.sizing_along(true) == Sizing::Hug {
            h_sizing = Sizing::Hug;
        }
        if v_sizing == Sizing::Fixed && al.sizing_along(false) == Sizing::Hug {
            v_sizing = Sizing::Hug;
        }
    }

    if write_w {
        if let Some(w_var) = n.variables.bindings.get("width") {
            obj.insert("width".into(), json!(format!("${w_var}")));
        } else {
            match h_sizing {
                Sizing::Fill => {
                    obj.insert("width".into(), json!("fill_container"));
                }
                Sizing::Hug => {
                    if !is_text || is_frame_like {
                        obj.insert("width".into(), json!("fit_content"));
                    }
                }
                Sizing::Fixed => {
                    if (!is_text || is_frame_like) && n.size.x > 0.0 {
                        obj.insert("width".into(), json!(n.size.x));
                    }
                }
            }
        }
    }

    if write_h {
        if let Some(h_var) = n.variables.bindings.get("height") {
            obj.insert("height".into(), json!(format!("${h_var}")));
        } else {
            match v_sizing {
                Sizing::Fill => {
                    obj.insert("height".into(), json!("fill_container"));
                }
                Sizing::Hug => {
                    if !is_text || is_frame_like {
                        obj.insert("height".into(), json!("fit_content"));
                    }
                }
                Sizing::Fixed => {
                    if (!is_text || is_frame_like) && n.size.y > 0.0 {
                        obj.insert("height".into(), json!(n.size.y));
                    }
                }
            }
        }
    }
}

fn export_text_fields(n: &Node, t: &TextProps, obj: &mut Map<String, Value>) {
    if let Some(c_var) = n.variables.bindings.get("characters") {
        obj.insert("content".into(), json!(format!("${c_var}")));
    } else {
        obj.insert("content".into(), json!(t.content));
    }

    let bound_ff = n
        .plugin_data
        .get("pen:bindings")
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| v.get("fontFamily").and_then(Value::as_str).map(String::from));
    if let Some(ff_var) = bound_ff {
        obj.insert("fontFamily".into(), json!(ff_var));
    } else {
        obj.insert("fontFamily".into(), json!(t.style.font_family));
    }

    if let Some(fs_var) = n.variables.bindings.get("fontSize") {
        obj.insert("fontSize".into(), json!(format!("${fs_var}")));
    } else {
        obj.insert("fontSize".into(), json!(t.style.font_size));
    }

    if let Some(fw_var) = n.variables.bindings.get("fontWeight") {
        obj.insert("fontWeight".into(), json!(format!("${fw_var}")));
    } else {
        obj.insert("fontWeight".into(), json!(t.style.font_weight));
    }

    if t.style.italic {
        obj.insert("fontStyle".into(), json!("italic"));
    }

    if let Some(ls_var) = n.variables.bindings.get("letterSpacing") {
        obj.insert("letterSpacing".into(), json!(format!("${ls_var}")));
    } else if t.style.letter_spacing.abs() > 1e-4 {
        obj.insert("letterSpacing".into(), json!(t.style.letter_spacing));
    }

    if let Some(lh_var) = n.variables.bindings.get("lineHeight") {
        obj.insert("lineHeight".into(), json!(format!("${lh_var}")));
    } else {
        match t.style.line_height {
            LineHeight::Pixels(px) => {
                obj.insert("lineHeight".into(), json!(px));
            }
            LineHeight::Percent(pct) => {
                obj.insert("lineHeight".into(), json!(pct / 100.0));
            }
            LineHeight::Auto => {}
        }
    }

    if t.style.align_h != TextAlignH::Left {
        let ta = match t.style.align_h {
            TextAlignH::Left => "left",
            TextAlignH::Center => "center",
            TextAlignH::Right => "right",
            TextAlignH::Justified => "justify",
        };
        obj.insert("textAlign".into(), json!(ta));
    }

    match t.style.decoration {
        TextDecoration::Underline => {
            obj.insert("underline".into(), Value::Bool(true));
            obj.insert("textDecoration".into(), json!("underline"));
        }
        TextDecoration::Strikethrough => {
            obj.insert("strikethrough".into(), Value::Bool(true));
            obj.insert("textDecoration".into(), json!("strikethrough"));
        }
        TextDecoration::None => {}
    }
}

fn align_to_pen_str(a: LayoutAlign) -> &'static str {
    match a {
        LayoutAlign::Start => "start",
        LayoutAlign::Center => "center",
        LayoutAlign::End => "end",
        LayoutAlign::SpaceBetween => "space_between",
        LayoutAlign::SpaceAround => "space_around",
        LayoutAlign::SpaceEvenly => "space_evenly",
        LayoutAlign::Baseline => "baseline",
    }
}

fn export_effect(eff: &Effect) -> Option<Value> {
    match eff {
        Effect::DropShadow { color, offset, radius, spread, visible, .. } => Some(json!({
            "type": "shadow",
            "shadowType": "outer",
            "color": format_pen_color(*color),
            "offset": { "x": offset.x, "y": offset.y },
            "blur": radius,
            "spread": spread,
            "enabled": visible,
        })),
        Effect::InnerShadow { color, offset, radius, spread, visible, .. } => Some(json!({
            "type": "shadow",
            "shadowType": "inner",
            "color": format_pen_color(*color),
            "offset": { "x": offset.x, "y": offset.y },
            "blur": radius,
            "spread": spread,
            "enabled": visible,
        })),
        Effect::LayerBlur { radius, visible } => Some(json!({
            "type": "blur",
            "radius": radius,
            "enabled": visible,
        })),
        Effect::BackgroundBlur { radius, visible } => Some(json!({
            "type": "background_blur",
            "radius": radius,
            "enabled": visible,
        })),
        _ => None,
    }
}

fn collect_instance_descendant_overrides(doc: &Document, inst_node: &Node, out: &mut Map<String, Value>) {
    for &cid in &inst_node.children {
        let Some(cn) = doc.get(cid) else { continue };
        let has_slot_override = cn.component.overrides.contains(&OverrideField::SlotContent);
        if !cn.component.overrides.is_empty() {
            if let Some(src_id) = cn.component.source.and_then(|s| doc.get(s)).map(node_pen_id) {
                let mut ov = Map::new();
                if cn.component.overrides.contains(&OverrideField::Characters) {
                    if cn.plugin_data.get("pen:icon_overridden").map(String::as_str) == Some("true") {
                        if let Some(icon_raw) = cn.plugin_data.get("pen:icon") {
                            if let Ok(icon_meta) = serde_json::from_str::<Value>(icon_raw) {
                                if let Some(fam) = icon_meta.get("family").and_then(Value::as_str) {
                                    ov.insert("iconFontFamily".into(), json!(fam));
                                }
                                if let Some(iname) = icon_meta.get("name").and_then(Value::as_str) {
                                    ov.insert("iconFontName".into(), json!(iname));
                                }
                            }
                        }
                    } else if let Some(t) = cn.kind.any_text() {
                        ov.insert("content".into(), json!(t.content));
                    }
                }
                if cn.component.overrides.contains(&OverrideField::TextStyle) {
                    if let Some(t) = cn.kind.any_text() {
                        ov.insert("fontFamily".into(), json!(t.style.font_family));
                        ov.insert("fontSize".into(), json!(t.style.font_size));
                        ov.insert("fontWeight".into(), json!(t.style.font_weight));
                        if t.style.italic {
                            ov.insert("fontStyle".into(), json!("italic"));
                        }
                        if t.style.decoration == TextDecoration::Underline {
                            ov.insert("underline".into(), Value::Bool(true));
                        } else if t.style.decoration == TextDecoration::Strikethrough {
                            ov.insert("strikethrough".into(), Value::Bool(true));
                        }
                    }
                }
                if cn.component.overrides.contains(&OverrideField::Fills) {
                    let icon_stroke = cn.plugin_data.get("pen:icon_stroke_from_fill").map(String::as_str) == Some("true");
                    let (active_fills, fbind) = if let NodeKind::Text(t) = &cn.kind {
                        (&t.style.fills, cn.variables.bindings.get("fills/0"))
                    } else if icon_stroke && cn.fills.is_empty() {
                        (&cn.stroke.paints, cn.variables.bindings.get("strokes/0").or_else(|| cn.variables.bindings.get("fills/0")))
                    } else {
                        (&cn.fills, cn.variables.bindings.get("fills/0"))
                    };
                    if let Some(fval) = export_paints_value(active_fills, fbind, &cn.plugin_data) {
                        ov.insert("fill".into(), fval);
                    }
                }
                if cn.component.overrides.contains(&OverrideField::Stroke) {
                    if let Some(sval) = export_stroke_value_filtered(&cn.stroke, &cn.variables, &cn.plugin_data, true) {
                        ov.insert("stroke".into(), sval);
                    }
                }
                if cn.component.overrides.contains(&OverrideField::Size) {
                    let has_explicit_w = cn.plugin_data.get("pen:explicit_width").map(String::as_str) == Some("true");
                    let has_explicit_h = cn.plugin_data.get("pen:explicit_height").map(String::as_str) == Some("true");
                    let (write_w, write_h) = if has_explicit_w || has_explicit_h { (has_explicit_w, has_explicit_h) } else { (true, true) };
                    write_node_dimensions(cn, cn.kind.frame_props().is_some(), &mut ov, write_w, write_h);
                }
                if cn.component.overrides.contains(&OverrideField::Visible) {
                    ov.insert("enabled".into(), json!(cn.visible));
                }
                if cn.component.overrides.contains(&OverrideField::CornerRadii) {
                    let r0 = cn.corner_radii[0];
                    if cn.corner_radii.iter().all(|&r| (r - r0).abs() < 1e-4) {
                        ov.insert("cornerRadius".into(), json!(r0));
                    } else {
                        ov.insert("cornerRadius".into(), json!(cn.corner_radii));
                    }
                }
                if cn.component.overrides.contains(&OverrideField::Layout) {
                    if let Some(fp) = cn.kind.frame_props() {
                        if let Some(al) = &fp.auto_layout {
                            if al.padding[0] == al.padding[1] && al.padding[1] == al.padding[2] && al.padding[2] == al.padding[3] {
                                ov.insert("padding".into(), json!(al.padding[0]));
                            } else {
                                ov.insert("padding".into(), json!(al.padding));
                            }
                            if al.item_spacing > 0.0 {
                                ov.insert("gap".into(), json!(al.item_spacing));
                            }
                        }
                    }
                }
                if has_slot_override {
                    let slot_children: Vec<Value> = cn.children.iter().filter_map(|&scid| export_pen_node(doc, scid)).collect();
                    ov.insert("children".into(), Value::Array(slot_children));
                }
                if !ov.is_empty() {
                    out.insert(src_id, Value::Object(ov));
                }
            }
        }
        if !has_slot_override {
            collect_instance_descendant_overrides(doc, cn, out);
        }
    }
}
