//! Node tree parsing, frame defaults, icon_font handling, geometry/transforms, and unknown-node preservation for `.pen` 2.21.

use super::paint::{parse_blend_mode, parse_corner_radius, parse_effects, parse_paints, parse_stroke, resolve_num_or_var};
use super::variables::{resolve_doc_var, resolve_doc_var_bool, resolve_doc_var_color, resolve_doc_var_string};
use super::PenWarning;
use crate::svgpath::parse_path;
use designos_doc::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct PendingInstance {
    pub inst_id: NodeId,
    pub ref_source_id: String,
    pub descendants: Option<Value>,
    pub explicit_width: bool,
    pub explicit_height: bool,
    pub explicit_fill: bool,
    pub explicit_stroke: bool,
    pub explicit_corner_radius: bool,
    pub explicit_layout: bool,
    pub explicit_content: Option<String>,
}

pub struct PenImportCtx {
    pub base_dir: Option<PathBuf>,
    pub total_image_bytes: usize,
    pub components_by_source_id: HashMap<String, NodeId>,
    pub pending_instances: Vec<PendingInstance>,
    pub pending_connections: Vec<(NodeId, Value)>,
    pub warnings: Vec<PenWarning>,
}

impl PenImportCtx {
    pub fn new(base_dir: Option<PathBuf>, warnings: Vec<PenWarning>) -> Self {
        Self {
            base_dir,
            total_image_bytes: 0,
            components_by_source_id: HashMap::new(),
            pending_instances: Vec::new(),
            pending_connections: Vec::new(),
            warnings,
        }
    }
}

enum ImportWork<'a> {
    Node { v: &'a Value, parent_id: NodeId },
    FinishComponent { comp_id: NodeId },
}

pub fn import_pen_node(v: &Value, doc: &mut Document, parent_id: NodeId, ctx: &mut PenImportCtx) -> Option<NodeId> {
    let (root_id, is_comp, skip_children) = import_single_pen_node(v, doc, parent_id, ctx)?;
    if skip_children {
        return Some(root_id);
    }

    let mut stack: Vec<ImportWork<'_>> = Vec::new();
    if is_comp {
        stack.push(ImportWork::FinishComponent { comp_id: root_id });
    }
    if let Some(children) = v.get("children").and_then(Value::as_array) {
        for ch in children.iter().rev() {
            stack.push(ImportWork::Node { v: ch, parent_id: root_id });
        }
    }

    while let Some(work) = stack.pop() {
        match work {
            ImportWork::Node { v: cur_v, parent_id: cur_parent } => {
                if let Some((nid, cur_comp, cur_skip)) = import_single_pen_node(cur_v, doc, cur_parent, ctx) {
                    if cur_skip {
                        continue;
                    }
                    if cur_comp {
                        stack.push(ImportWork::FinishComponent { comp_id: nid });
                    }
                    if let Some(children) = cur_v.get("children").and_then(Value::as_array) {
                        for ch in children.iter().rev() {
                            stack.push(ImportWork::Node { v: ch, parent_id: nid });
                        }
                    }
                }
            }
            ImportWork::FinishComponent { comp_id } => {
                register_component_slots_recursive(doc, comp_id, comp_id);
            }
        }
    }

    Some(root_id)
}

fn import_single_pen_node(v: &Value, doc: &mut Document, parent_id: NodeId, ctx: &mut PenImportCtx) -> Option<(NodeId, bool, bool)> {
    let raw_type = v.get("type").and_then(Value::as_str).unwrap_or("frame");
    let is_reusable = v.get("reusable").and_then(Value::as_bool).unwrap_or(false);
    let pen_id = v.get("id").and_then(Value::as_str).map(String::from);

    // Spec §5: reusable on non-frame nodes wraps the primitive in a Component frame with
    // `plugin_data["pen:reusable_wrapper"] = "true"` so `create_instance` / `ref` and export round-trip cleanly.
    if is_reusable && !matches!(raw_type, "frame" | "component" | "ref" | "instance") {
        let wid = import_reusable_non_frame_node(v, doc, parent_id, ctx)?;
        return Some((wid, false, true));
    }

    let node_id = doc.alloc_id();
    let x = v.get("x").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0);
    let y = v.get("y").and_then(|y| resolve_num_or_var(y, doc)).unwrap_or(0.0);

    let is_frame_like = matches!(raw_type, "frame" | "component");
    let is_icon_like = matches!(raw_type, "icon_font" | "icon");

    // Spec §4: when width/height are omitted on a frame, default is `fit_content` (Sizing::Hug).
    let (mut w, mut w_sizing, w_var) = match v.get("width") {
        Some(wv) => parse_pen_sizing(Some(wv), doc, if is_icon_like { 24.0 } else { 100.0 }),
        None if is_frame_like => (100.0, Sizing::Hug, None),
        None if is_icon_like => (24.0, Sizing::Fixed, None),
        None => (100.0, Sizing::Fixed, None),
    };
    let (mut h, mut h_sizing, h_var) = match v.get("height") {
        Some(hv) => parse_pen_sizing(Some(hv), doc, if is_icon_like { 24.0 } else { 100.0 }),
        None if is_frame_like => (100.0, Sizing::Hug, None),
        None if is_icon_like => (24.0, Sizing::Fixed, None),
        None => (100.0, Sizing::Fixed, None),
    };

    let mut plugin_data = std::collections::BTreeMap::new();
    let base_dir_ref = ctx.base_dir.clone();

    let (fills, fill_var) =
        parse_paints(v.get("fill"), doc, pen_id.as_deref(), base_dir_ref.as_deref(), &mut ctx.total_image_bytes, &mut plugin_data, &mut ctx.warnings);
    let (stroke, stroke_color_var, stroke_weight_var) =
        parse_stroke(v, doc, pen_id.as_deref(), base_dir_ref.as_deref(), &mut ctx.total_image_bytes, &mut plugin_data, &mut ctx.warnings);
    let effects = parse_effects(v.get("effect").or_else(|| v.get("effects")), doc);
    let (corner_radii, radius_var) = parse_corner_radius(v.get("cornerRadius"), doc);

    let kind = match raw_type {
        "frame" | "component" => {
            let fp = parse_frame_props(v, doc, w_sizing, h_sizing, true);
            if is_reusable || raw_type == "component" {
                NodeKind::Component(fp)
            } else {
                NodeKind::Frame(fp)
            }
        }
        "group" => NodeKind::Group,
        "section" => NodeKind::Section,
        "rectangle" => NodeKind::Rectangle,
        "ellipse" => {
            let start_deg = v.get("startAngle").and_then(|a| resolve_num_or_var(a, doc)).unwrap_or(0.0);
            let sweep_deg = v
                .get("sweepAngle")
                .and_then(|a| resolve_num_or_var(a, doc))
                .or_else(|| v.get("endAngle").and_then(|a| resolve_num_or_var(a, doc)).map(|end| end - start_deg))
                .unwrap_or(360.0);
            let inner_radius = v.get("innerRadius").and_then(|r| resolve_num_or_var(r, doc)).unwrap_or(0.0).clamp(0.0, 1.0);
            let start = start_deg.to_radians();
            let end = start + sweep_deg.to_radians();
            NodeKind::Ellipse(ArcData { start, end, inner_radius })
        }
        "line" => NodeKind::Line,
        "polygon" | "star" => {
            let count = v.get("polygonCount").or_else(|| v.get("count")).and_then(Value::as_u64).unwrap_or(3).max(3) as u32;
            if let Some(inner) = v.get("innerRadius").and_then(|r| resolve_num_or_var(r, doc)) {
                NodeKind::Star { count, inner_ratio: inner.clamp(0.0, 1.0) }
            } else if raw_type == "star" {
                NodeKind::Star { count, inner_ratio: 0.5 }
            } else {
                NodeKind::Polygon { count }
            }
        }
        "path" | "svg" => {
            let geom = v.get("geometry").or_else(|| v.get("d")).and_then(Value::as_str).unwrap_or("");
            let fill_rule = match v.get("fillRule").and_then(Value::as_str) {
                Some(rule) if rule.eq_ignore_ascii_case("evenodd") => FillRule::EvenOdd,
                _ => FillRule::NonZero,
            };
            let mut path = parse_path(geom, fill_rule);
            if let Some((vb_x, vb_y, vb_w, vb_h)) = parse_view_box(v.get("viewBox")) {
                if v.get("width").is_none() && vb_w > 0.0 {
                    w = vb_w;
                }
                if v.get("height").is_none() && vb_h > 0.0 {
                    h = vb_h;
                }
                if vb_w > 0.0 && vb_h > 0.0 && w > 0.0 && h > 0.0 {
                    scale_path(&mut path, vb_x, vb_y, w / vb_w, h / vb_h);
                }
            }
            let paths = if path.cmds.is_empty() { Vec::new() } else { vec![path] };
            NodeKind::Vector { paths, network: None }
        }
        "text" => {
            let (tp, text_w, text_h) = parse_text_props(v, doc, &fills, w_sizing, h_sizing, (w, h), pen_id.as_deref(), &mut plugin_data, &mut ctx.warnings);
            if v.get("width").is_none() {
                w = text_w;
                w_sizing = Sizing::Hug;
            }
            if v.get("height").is_none() {
                h = text_h;
                h_sizing = Sizing::Hug;
            }
            NodeKind::Text(tp)
        }
        "icon_font" | "icon" => {
            let (family, icon_name) = extract_icon_family_and_name(v);
            let mut icon_meta = serde_json::Map::new();
            icon_meta.insert("family".into(), json!(family));
            icon_meta.insert("name".into(), json!(icon_name));
            if let Some(fw) = v.get("weight").or_else(|| v.get("fontWeight")).and_then(Value::as_u64) {
                icon_meta.insert("weight".into(), json!(fw));
            }
            plugin_data.insert("pen:icon".into(), Value::Object(icon_meta).to_string());

            let explicit_geom = v.get("geometry").and_then(Value::as_str);
            let svg_d = explicit_geom.or_else(|| builtin_icon_svg_path(&family, &icon_name));
            if let Some(d_str) = svg_d {
                let mut path = parse_path(d_str, FillRule::NonZero);
                let (vb_x, vb_y, vb_w, vb_h) = parse_view_box(v.get("viewBox")).unwrap_or((0.0, 0.0, 24.0, 24.0));
                if vb_w > 0.0 && vb_h > 0.0 && w > 0.0 && h > 0.0 {
                    scale_path(&mut path, vb_x, vb_y, w / vb_w, h / vb_h);
                }
                NodeKind::Vector { paths: if path.cmds.is_empty() { Vec::new() } else { vec![path] }, network: None }
            } else {
                ctx.warnings.push(PenWarning {
                    code: "unresolved_icon_glyph".into(),
                    node_id: pen_id.clone(),
                    message: format!("Icon glyph '{family}:{icon_name}' could not be resolved to local vector geometry; using placeholder frame"),
                });
                NodeKind::Frame(FrameProps::default())
            }
        }
        "note" | "sticky" => {
            let content = v.get("content").and_then(Value::as_str).unwrap_or("").to_string();
            let text = TextProps { content, ..Default::default() };
            NodeKind::Sticky(StickyProps { text, author: v.get("author").and_then(Value::as_str).map(String::from), show_author: true, ..Default::default() })
        }
        "ref" | "instance" => {
            let ref_id = v.get("ref").or_else(|| v.get("componentId")).and_then(Value::as_str).unwrap_or("").to_string();
            let fp = parse_frame_props(v, doc, w_sizing, h_sizing, v.get("layout").is_some());
            ctx.pending_instances.push(PendingInstance {
                inst_id: node_id,
                ref_source_id: ref_id.clone(),
                descendants: v.get("descendants").cloned(),
                explicit_width: v.get("width").is_some(),
                explicit_height: v.get("height").is_some(),
                explicit_fill: v.get("fill").is_some(),
                explicit_stroke: v.get("stroke").is_some() || v.get("strokeWidth").is_some(),
                explicit_corner_radius: v.get("cornerRadius").is_some(),
                explicit_layout: v.get("layout").is_some()
                    || v.get("gap").is_some()
                    || v.get("padding").is_some()
                    || v.get("justifyContent").is_some()
                    || v.get("alignItems").is_some()
                    || v.get("clip").is_some(),
                explicit_content: v.get("content").and_then(Value::as_str).map(String::from),
            });
            NodeKind::Instance { frame: fp, component: None, component_key: if ref_id.is_empty() { None } else { Some(ref_id) } }
        }
        "connection" | "connector" => {
            ctx.pending_connections.push((node_id, v.clone()));
            NodeKind::Connector(ConnectorProps::default())
        }
        "boolean" | "union" | "subtract" | "intersect" | "exclude" => {
            let op_str = v.get("operation").and_then(Value::as_str).unwrap_or(raw_type);
            let op = match op_str {
                "subtract" => BooleanOp::Subtract,
                "intersect" => BooleanOp::Intersect,
                "exclude" => BooleanOp::Exclude,
                _ => BooleanOp::Union,
            };
            NodeKind::BooleanOp(op)
        }
        unknown_type => {
            let mut raw_copy = v.as_object().cloned().unwrap_or_default();
            raw_copy.remove("children");
            if let Ok(serialized) = serde_json::to_string(&Value::Object(raw_copy)) {
                plugin_data.insert("pen:raw".into(), serialized);
            }
            ctx.warnings.push(PenWarning {
                code: "unknown_node_type".into(),
                node_id: pen_id.clone(),
                message: format!("Unrecognized .pen node type '{unknown_type}' preserved as NodeKind::Unknown"),
            });
            NodeKind::Unknown { type_name: unknown_type.to_string() }
        }
    };

    let is_comp = matches!(kind, NodeKind::Component(_));
    let mut node = Node::new(node_id, kind);
    node.source_id = pen_id.clone();
    node.name = v.get("name").and_then(Value::as_str).map(String::from).or_else(|| pen_id.clone()).unwrap_or_else(|| node.kind.default_name().into());

    // Transform: translation + rotation + flipX/flipY
    let rot_deg = v.get("rotation").and_then(|r| resolve_num_or_var(r, doc)).unwrap_or(0.0);
    let flip_x = v.get("flipX").or_else(|| v.get("horizontalFlip")).and_then(Value::as_bool).unwrap_or(false);
    let flip_y = v.get("flipY").or_else(|| v.get("verticalFlip")).and_then(Value::as_bool).unwrap_or(false);
    let sx = if flip_x { -1.0 } else { 1.0 };
    let sy = if flip_y { -1.0 } else { 1.0 };
    node.transform = Affine::translate(x, y) * Affine::rotate(-rot_deg.to_radians()) * Affine::scale(sx, sy);
    if flip_x || flip_y {
        plugin_data.insert("pen:flip".into(), json!({ "flipX": flip_x, "flipY": flip_y, "rotation": rot_deg }).to_string());
    }

    node.size = Vec2::new(w.max(0.0), h.max(0.0));
    node.layout_child.horizontal = w_sizing;
    node.layout_child.vertical = h_sizing;
    if v.get("layoutPosition").and_then(Value::as_str) == Some("absolute") {
        node.layout_child.absolute = true;
    }

    if let Some(en) = v.get("enabled") {
        if let Some(b) = en.as_bool() {
            node.visible = b;
        } else if let Some(s) = en.as_str() {
            if let Some(vn) = s.strip_prefix('$') {
                node.visible = resolve_doc_var_bool(doc, vn).unwrap_or(true);
                node.variables.bindings.insert("visible".into(), vn.to_string());
            }
        }
    }
    if let Some(op) = v.get("opacity") {
        if let Some(n) = resolve_num_or_var(op, doc) {
            node.opacity = n.clamp(0.0, 1.0);
        }
        if let Some(s) = op.as_str() {
            if let Some(vn) = s.strip_prefix('$') {
                node.variables.bindings.insert("opacity".into(), vn.to_string());
            }
        }
    }
    if let Some(bm) = v.get("blendMode").and_then(Value::as_str) {
        node.blend = parse_blend_mode(bm);
    }
    if let Some(href) = v.get("href").and_then(Value::as_str) {
        if !href.is_empty() {
            node.hyperlink = Some(href.to_string());
        }
    }

    if !matches!(node.kind, NodeKind::Text(_) | NodeKind::Unknown { .. }) {
        if is_icon_like && fills.is_empty() && stroke.paints.is_empty() && matches!(node.kind, NodeKind::Vector { .. }) {
            node.fills = vec![Paint::solid(Color::BLACK)];
        } else {
            node.fills = fills;
        }
    } else if matches!(node.kind, NodeKind::Unknown { .. }) {
        node.fills.clear();
    }

    node.stroke = stroke;
    node.effects = effects;
    node.corner_radii = corner_radii;

    // Bindings for text properties
    if matches!(node.kind, NodeKind::Text(_)) {
        bind_text_variables(v, &mut node);
    }

    if let Some(fv) = fill_var {
        node.variables.bindings.insert("fills/0".into(), fv);
    }
    if let Some(scv) = stroke_color_var {
        node.variables.bindings.insert("strokes/0".into(), scv);
    }
    if let Some(swv) = stroke_weight_var {
        node.variables.bindings.insert("strokeWeight".into(), swv);
    }
    if let Some(rv) = radius_var {
        node.variables.bindings.insert("cornerRadius".into(), rv);
    }
    if let Some(wv) = w_var {
        node.variables.bindings.insert("width".into(), wv);
    }
    if let Some(hv) = h_var {
        node.variables.bindings.insert("height".into(), hv);
    }

    // Spec §5: placeholder and slot flags
    if v.get("placeholder").and_then(Value::as_bool).unwrap_or(false) {
        plugin_data.insert("pen:placeholder".into(), "true".into());
    }
    if let Some(slot_arr) = v.get("slot").and_then(Value::as_array) {
        node.component.is_slot = true;
        let slot_prop_name = node.name.clone();
        node.component.bindings.insert(OverrideField::SlotContent, slot_prop_name);
        if let Ok(s) = serde_json::to_string(slot_arr) {
            plugin_data.insert("pen:slot".into(), s);
        }
    }

    // Theme override on node (`theme: { axis: val }`)
    if let Some(theme_obj) = v.get("theme").and_then(Value::as_object) {
        for (axis, tval) in theme_obj {
            if let Some(tstr) = tval.as_str() {
                let mode_id = format!("{axis}:{tstr}");
                let col_id =
                    doc.variable_collections.values().find(|c| c.name == *axis).map(|c| c.id.clone()).unwrap_or_else(|| format!("pen:axis:0000:{axis}"));
                node.variables.modes.insert(col_id, mode_id);
            }
        }
    }

    node.plugin_data = plugin_data;

    if is_reusable {
        if let Some(sid) = &pen_id {
            ctx.components_by_source_id.insert(sid.clone(), node_id);
        }
    }

    doc.insert(parent_id, node).ok()?;

    Some((node_id, is_comp, false))
}

fn import_reusable_non_frame_node(v: &Value, doc: &mut Document, parent_id: NodeId, ctx: &mut PenImportCtx) -> Option<NodeId> {
    let pen_id = v.get("id").and_then(Value::as_str).map(String::from);
    let x = v.get("x").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0);
    let y = v.get("y").and_then(|y| resolve_num_or_var(y, doc)).unwrap_or(0.0);

    let wrapper_id = doc.alloc_id();
    let mut wrapper = Node::new(wrapper_id, NodeKind::Component(FrameProps::default()));
    wrapper.source_id = pen_id.clone();
    wrapper.name = v.get("name").and_then(Value::as_str).map(String::from).or_else(|| pen_id.clone()).unwrap_or_else(|| "Component".into());
    wrapper.transform = Affine::translate(x, y);
    wrapper.fills.clear();
    wrapper.plugin_data.insert("pen:reusable_wrapper".into(), "true".into());
    doc.insert(parent_id, wrapper).ok()?;

    let mut inner_json = v.clone();
    if let Some(obj) = inner_json.as_object_mut() {
        obj.remove("reusable");
        obj.insert("x".into(), json!(0.0));
        obj.insert("y".into(), json!(0.0));
        if let Some(sid) = &pen_id {
            obj.insert("id".into(), json!(format!("{sid}:inner")));
        }
    }
    if let Some((inner_id, _, _)) = import_single_pen_node(&inner_json, doc, wrapper_id, ctx) {
        if let Some(inner_node) = doc.get(inner_id).cloned() {
            if let Some(w_node) = doc.get_mut(wrapper_id) {
                w_node.size = inner_node.size;
                w_node.layout_child = inner_node.layout_child;
            }
        }
    }

    if let Some(sid) = pen_id {
        ctx.components_by_source_id.insert(sid, wrapper_id);
    }

    Some(wrapper_id)
}

pub fn register_component_slots_recursive(doc: &mut Document, comp_id: NodeId, cur_id: NodeId) {
    let mut stack: Vec<NodeId> = doc.get(cur_id).map(|n| n.children.iter().copied().rev().collect()).unwrap_or_default();
    while let Some(cid) = stack.pop() {
        let (is_slot, children) = doc.get(cid).map(|n| (n.component.is_slot, n.children.clone())).unwrap_or((false, Vec::new()));
        if is_slot {
            let slot_name = doc
                .get(cid)
                .and_then(|n| n.component.bindings.get(&OverrideField::SlotContent).cloned().or_else(|| Some(n.name.clone())))
                .unwrap_or_else(|| "Slot".into());
            if let Some(comp) = doc.get_mut(comp_id) {
                if !comp.component.props.iter().any(|p| p.name == slot_name) {
                    comp.component.props.push(ComponentPropDef { name: slot_name.clone(), kind: PropKind::Slot, default: PropValue::Slot(Vec::new()) });
                }
                comp.component.slot_configs.entry(slot_name).or_default();
            }
        }
        stack.extend(children.into_iter().rev());
    }
}

pub fn parse_pen_sizing(v: Option<&Value>, doc: &Document, default_px: f32) -> (f32, Sizing, Option<String>) {
    let Some(val) = v else {
        return (default_px, Sizing::Fixed, None);
    };
    if let Some(n) = val.as_f64() {
        return (n as f32, Sizing::Fixed, None);
    }
    if let Some(s) = val.as_str() {
        let s = s.trim();
        if s == "fit_content" {
            return (default_px, Sizing::Hug, None);
        }
        if let Some(inner) = s.strip_prefix("fit_content(").and_then(|x| x.strip_suffix(')')) {
            let fallback = inner.trim().parse::<f32>().unwrap_or(default_px);
            return (fallback, Sizing::Hug, None);
        }
        if s == "fill_container" {
            return (default_px, Sizing::Fill, None);
        }
        if let Some(inner) = s.strip_prefix("fill_container(").and_then(|x| x.strip_suffix(')')) {
            let fallback = inner.trim().parse::<f32>().unwrap_or(default_px);
            return (fallback, Sizing::Fill, None);
        }
        if let Some(var_name) = s.strip_prefix('$') {
            let resolved = resolve_doc_var(doc, var_name).unwrap_or(default_px);
            return (resolved, Sizing::Fixed, Some(var_name.to_string()));
        }
        if let Ok(n) = s.parse::<f32>() {
            return (n, Sizing::Fixed, None);
        }
    }
    (default_px, Sizing::Fixed, None)
}

pub fn parse_frame_props(v: &Value, doc: &Document, w_sizing: Sizing, h_sizing: Sizing, default_horizontal: bool) -> FrameProps {
    let clips_content = match v.get("clip") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) if s.starts_with('$') => resolve_doc_var_bool(doc, s).unwrap_or(false),
        _ => false,
    };

    let layout_str = v.get("layout").and_then(Value::as_str);
    if layout_str == Some("none") || (!default_horizontal && layout_str.is_none()) {
        return FrameProps { clips_content, auto_layout: None, grids: Vec::new() };
    }

    let direction = match layout_str {
        Some("vertical") => LayoutDirection::Vertical,
        Some("grid") => LayoutDirection::Grid,
        _ => LayoutDirection::Horizontal,
    };

    let gap = v.get("gap").and_then(|g| resolve_num_or_var(g, doc)).unwrap_or(0.0);
    let padding = parse_pen_padding(v.get("padding"), doc);
    let primary_align = parse_pen_align(v.get("justifyContent").and_then(Value::as_str));
    let counter_align = parse_pen_align(v.get("alignItems").and_then(Value::as_str));
    let wrap = v.get("wrap").and_then(Value::as_bool).unwrap_or(false);

    let (primary_sizing, counter_sizing) = match direction {
        LayoutDirection::Horizontal | LayoutDirection::Grid => {
            (if w_sizing == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed }, if h_sizing == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed })
        }
        LayoutDirection::Vertical => {
            (if h_sizing == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed }, if w_sizing == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed })
        }
    };

    let mut al = AutoLayout::v1();
    al.direction = direction;
    al.wrap = wrap;
    al.item_spacing = gap;
    al.counter_spacing = gap;
    al.padding = padding;
    al.primary_align = primary_align;
    al.counter_align = counter_align;
    al.primary_sizing = primary_sizing;
    al.counter_sizing = counter_sizing;

    FrameProps { clips_content, auto_layout: Some(al), grids: Vec::new() }
}

pub fn parse_pen_padding(v: Option<&Value>, doc: &Document) -> [f32; 4] {
    let Some(val) = v else { return [0.0; 4] };
    if let Some(n) = resolve_num_or_var(val, doc) {
        return [n, n, n, n];
    }
    if let Some(arr) = val.as_array() {
        match arr.len() {
            1 => {
                let p = resolve_num_or_var(&arr[0], doc).unwrap_or(0.0);
                return [p, p, p, p];
            }
            2 => {
                let pv = resolve_num_or_var(&arr[0], doc).unwrap_or(0.0);
                let ph = resolve_num_or_var(&arr[1], doc).unwrap_or(0.0);
                return [pv, ph, pv, ph];
            }
            4 => {
                return [
                    resolve_num_or_var(&arr[0], doc).unwrap_or(0.0),
                    resolve_num_or_var(&arr[1], doc).unwrap_or(0.0),
                    resolve_num_or_var(&arr[2], doc).unwrap_or(0.0),
                    resolve_num_or_var(&arr[3], doc).unwrap_or(0.0),
                ];
            }
            _ => {}
        }
    }
    if let Some(obj) = val.as_object() {
        let top = obj.get("top").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0);
        let right = obj.get("right").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0);
        let bottom = obj.get("bottom").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0);
        let left = obj.get("left").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0);
        return [top, right, bottom, left];
    }
    [0.0; 4]
}

pub fn parse_pen_align(s: Option<&str>) -> LayoutAlign {
    match s {
        Some("center") | Some("middle") => LayoutAlign::Center,
        Some("end") | Some("flex-end") | Some("bottom") | Some("right") => LayoutAlign::End,
        Some("space_between") | Some("space-between") => LayoutAlign::SpaceBetween,
        Some("space_around") | Some("space-around") => LayoutAlign::SpaceAround,
        Some("space_evenly") | Some("space-evenly") => LayoutAlign::SpaceEvenly,
        Some("baseline") => LayoutAlign::Baseline,
        _ => LayoutAlign::Start,
    }
}

fn parse_view_box(v: Option<&Value>) -> Option<(f32, f32, f32, f32)> {
    let val = v?;
    if let Some(s) = val.as_str() {
        let nums: Vec<f32> =
            s.split(|c: char| c.is_ascii_whitespace() || c == ',').filter(|part| !part.is_empty()).filter_map(|part| part.parse::<f32>().ok()).collect();
        if nums.len() == 4 {
            return Some((nums[0], nums[1], nums[2], nums[3]));
        }
    } else if let Some(arr) = val.as_array() {
        if arr.len() == 4 {
            return Some((arr[0].as_f64()? as f32, arr[1].as_f64()? as f32, arr[2].as_f64()? as f32, arr[3].as_f64()? as f32));
        }
    } else if let Some(obj) = val.as_object() {
        let x = obj.get("x").or_else(|| obj.get("minX")).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let y = obj.get("y").or_else(|| obj.get("minY")).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let w = obj.get("width")?.as_f64()? as f32;
        let h = obj.get("height")?.as_f64()? as f32;
        return Some((x, y, w, h));
    }
    None
}

fn scale_path(path: &mut Path, min_x: f32, min_y: f32, sx: f32, sy: f32) {
    let tx = |p: Vec2| Vec2::new((p.x - min_x) * sx, (p.y - min_y) * sy);
    for cmd in &mut path.cmds {
        match cmd {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => *p = tx(*p),
            PathCmd::QuadTo(p1, p2) => {
                *p1 = tx(*p1);
                *p2 = tx(*p2);
            }
            PathCmd::CubicTo(p1, p2, p3) => {
                *p1 = tx(*p1);
                *p2 = tx(*p2);
                *p3 = tx(*p3);
            }
            PathCmd::Close => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_text_props(
    v: &Value,
    doc: &Document,
    fills: &[Paint],
    w_sizing: Sizing,
    h_sizing: Sizing,
    size_hint: (f32, f32),
    node_id: Option<&str>,
    plugin_data: &mut std::collections::BTreeMap<String, String>,
    warnings: &mut Vec<PenWarning>,
) -> (TextProps, f32, f32) {
    let mut style = TextStyle::default();
    if let Some(ff) = v.get("fontFamily").and_then(Value::as_str) {
        if let Some(var_name) = ff.strip_prefix('$') {
            style.font_family = resolve_doc_var_string(doc, var_name).unwrap_or_else(|| "Inter".into());
            plugin_data.insert("pen:bindings".into(), json!({ "fontFamily": format!("${var_name}") }).to_string());
            warnings.push(PenWarning {
                code: "unsupported_font_family_variable".into(),
                node_id: node_id.map(String::from),
                message: format!("Font family variable '${var_name}' resolved to '{}' and stored in plugin_data['pen:bindings']", style.font_family),
            });
        } else {
            style.font_family = ff.to_string();
        }
    }
    if let Some(fs) = v.get("fontSize").and_then(|x| resolve_num_or_var(x, doc)) {
        style.font_size = fs.max(1.0);
    }
    if let Some(fw) = v.get("fontWeight") {
        style.font_weight = parse_font_weight(fw, doc);
    }
    if v.get("fontStyle").and_then(Value::as_str) == Some("italic") {
        style.italic = true;
    }
    if let Some(ls) = v.get("letterSpacing").and_then(|x| resolve_num_or_var(x, doc)) {
        style.letter_spacing = ls;
    }
    if let Some(lh) = v.get("lineHeight").and_then(|x| resolve_num_or_var(x, doc)) {
        style.line_height = if lh <= 4.0 { LineHeight::Percent(lh * 100.0) } else { LineHeight::Pixels(lh) };
    }
    if let Some(ta) = v.get("textAlign").and_then(Value::as_str) {
        style.align_h = match ta {
            "center" => TextAlignH::Center,
            "right" | "end" => TextAlignH::Right,
            "justify" | "justified" => TextAlignH::Justified,
            _ => TextAlignH::Left,
        };
    }
    if let Some(tav) = v.get("textAlignVertical").and_then(Value::as_str) {
        style.align_v = match tav {
            "middle" | "center" => TextAlignV::Center,
            "bottom" | "end" => TextAlignV::Bottom,
            _ => TextAlignV::Top,
        };
    }

    if v.get("underline").and_then(Value::as_bool) == Some(true) {
        style.decoration = TextDecoration::Underline;
    } else if v.get("strikethrough").or_else(|| v.get("strike")).and_then(Value::as_bool) == Some(true) {
        style.decoration = TextDecoration::Strikethrough;
    } else if let Some(td) = v.get("textDecoration").and_then(Value::as_str) {
        style.decoration = match td.to_ascii_lowercase().as_str() {
            "underline" => TextDecoration::Underline,
            "strikethrough" | "line-through" => TextDecoration::Strikethrough,
            _ => TextDecoration::None,
        };
    }

    if !fills.is_empty() {
        style.fills = fills.to_vec();
    }

    let mut content = String::new();
    let mut runs = Vec::new();
    match v.get("content") {
        Some(Value::String(s)) => {
            if let Some(var_name) = s.strip_prefix('$') {
                content = resolve_doc_var_string(doc, var_name).unwrap_or_else(|| s.clone());
            } else {
                content = s.clone();
            }
        }
        Some(Value::Array(arr)) => {
            for span in arr {
                let span_text = span.get("content").or_else(|| span.get("text")).and_then(Value::as_str).unwrap_or("");
                if span_text.is_empty() {
                    continue;
                }
                let start = content.len();
                content.push_str(span_text);
                let end = content.len();
                let mut rstyle = style.clone();
                if let Some(ff) = span.get("fontFamily").and_then(Value::as_str) {
                    rstyle.font_family = ff.to_string();
                }
                if let Some(fs) = span.get("fontSize").and_then(|x| resolve_num_or_var(x, doc)) {
                    rstyle.font_size = fs.max(1.0);
                }
                if let Some(fw) = span.get("fontWeight") {
                    rstyle.font_weight = parse_font_weight(fw, doc);
                }
                if span.get("fontStyle").and_then(Value::as_str) == Some("italic") {
                    rstyle.italic = true;
                }
                if span.get("underline").and_then(Value::as_bool) == Some(true) {
                    rstyle.decoration = TextDecoration::Underline;
                } else if span.get("strikethrough").and_then(Value::as_bool) == Some(true) {
                    rstyle.decoration = TextDecoration::Strikethrough;
                }
                if let Some(fill_s) = span.get("fill").and_then(Value::as_str) {
                    let c = if let Some(vn) = fill_s.strip_prefix('$') { resolve_doc_var_color(doc, vn) } else { Color::from_hex(fill_s) };
                    if let Some(col) = c {
                        rstyle.fills = vec![Paint::solid(col)];
                    }
                }
                runs.push(TextRun { start, end, style: rstyle });
            }
        }
        _ => {}
    }

    let auto_resize = match v.get("textGrowth").and_then(Value::as_str) {
        Some("fixed-width") => TextAutoResize::Height,
        Some("fixed-width-height") => TextAutoResize::None,
        _ => {
            if w_sizing == Sizing::Fixed && v.get("width").is_some() && h_sizing != Sizing::Fixed {
                TextAutoResize::Height
            } else if w_sizing == Sizing::Fixed && h_sizing == Sizing::Fixed && v.get("width").is_some() && v.get("height").is_some() {
                TextAutoResize::None
            } else {
                TextAutoResize::WidthAndHeight
            }
        }
    };

    let char_count = content.chars().count().max(1) as f32;
    let approx_w = (char_count * style.font_size * 0.58).ceil().max(style.font_size);
    let approx_h = match style.line_height {
        LineHeight::Pixels(px) => px,
        LineHeight::Percent(pct) => (style.font_size * pct / 100.0).ceil(),
        LineHeight::Auto => (style.font_size * 1.25).ceil(),
    };

    let tp = TextProps { content, style, runs, auto_resize, ..Default::default() };

    let out_w = if v.get("width").is_some() { size_hint.0 } else { approx_w };
    let out_h = if v.get("height").is_some() { size_hint.1 } else { approx_h };
    (tp, out_w, out_h)
}

pub fn parse_font_weight(v: &Value, doc: &Document) -> u16 {
    if let Some(n) = v.as_u64() {
        return (n as u16).clamp(100, 900);
    }
    if let Some(f) = v.as_f64() {
        return (f.round() as u16).clamp(100, 900);
    }
    if let Some(s) = v.as_str() {
        if let Some(vn) = s.strip_prefix('$') {
            if let Some(n) = resolve_doc_var(doc, vn) {
                return (n.round() as u16).clamp(100, 900);
            }
        }
        if let Ok(n) = s.parse::<u16>() {
            return n.clamp(100, 900);
        }
        return match s.to_ascii_lowercase().as_str() {
            "thin" | "hairline" => 100,
            "extralight" | "ultralight" => 200,
            "light" => 300,
            "normal" | "regular" => 400,
            "medium" => 500,
            "semibold" | "demibold" => 600,
            "bold" => 700,
            "extrabold" | "ultrabold" => 800,
            "black" | "heavy" => 900,
            _ => 400,
        };
    }
    400
}

fn bind_text_variables(v: &Value, node: &mut Node) {
    for key in ["fontSize", "fontWeight", "lineHeight", "letterSpacing"] {
        if let Some(s) = v.get(key).and_then(Value::as_str) {
            if let Some(var_name) = s.strip_prefix('$') {
                node.variables.bindings.insert(key.to_string(), var_name.to_string());
            }
        }
    }
    if let Some(s) = v.get("content").and_then(Value::as_str) {
        if let Some(var_name) = s.strip_prefix('$') {
            node.variables.bindings.insert("characters".into(), var_name.to_string());
        }
    }
}

pub fn extract_icon_family_and_name(v: &Value) -> (String, String) {
    let explicit_family = v.get("iconFontFamily").or_else(|| v.get("family")).or_else(|| v.get("fontFamily")).and_then(Value::as_str);
    let raw_name =
        v.get("iconFontName").or_else(|| v.get("icon")).or_else(|| v.get("content")).or_else(|| v.get("name")).and_then(Value::as_str).unwrap_or("icon");

    if let Some((prefix, rest)) = raw_name.split_once(':') {
        let fam = explicit_family.unwrap_or(prefix).to_string();
        (fam, rest.to_string())
    } else {
        let fam = explicit_family.unwrap_or("lucide").to_string();
        (fam, raw_name.to_string())
    }
}

pub fn builtin_icon_svg_path(_family: &str, name: &str) -> Option<&'static str> {
    match name.trim().to_ascii_lowercase().as_str() {
        "search" => Some("M 11 3 C 6.58 3 3 6.58 3 11 C 3 15.42 6.58 19 11 19 C 13.21 19 15.21 18.1 16.66 16.66 L 21 21 L 16.66 16.66 C 18.1 15.21 19 13.21 19 11 C 19 6.58 15.42 3 11 3 Z"),
        "check" => Some("M 20 6 L 9 17 L 4 12 L 9 17 Z"),
        "plus" | "add" => Some("M 12 5 L 12 19 M 5 12 L 19 12"),
        "x" | "close" => Some("M 18 6 L 6 18 M 6 6 L 18 18"),
        "arrow-right" | "arrow_forward" => Some("M 5 12 L 19 12 M 12 5 L 19 12 L 12 19"),
        "arrow-left" | "arrow_back" => Some("M 19 12 L 5 12 M 12 19 L 5 12 L 12 5"),
        "chevron-down" | "expand_more" => Some("M 6 9 L 12 15 L 18 9"),
        "chevron-right" | "chevron_right" => Some("M 9 18 L 15 12 L 9 6"),
        "chevron-left" | "chevron_left" => Some("M 15 18 L 9 12 L 15 6"),
        "chevron-up" | "expand_less" => Some("M 18 15 L 12 9 L 6 15"),
        "star" => Some("M 12 2 L 15.09 8.26 L 22 9.27 L 17 14.14 L 18.18 21.02 L 12 17.77 L 5.82 21.02 L 7 14.14 L 2 9.27 L 8.91 8.26 Z"),
        "heart" => Some("M 12 21.35 L 10.55 20.03 C 5.4 15.36 2 12.28 2 8.5 C 2 5.42 4.42 3 7.5 3 C 9.24 3 10.91 3.81 12 5.09 C 13.09 3.81 14.76 3 16.5 3 C 19.58 3 22 5.42 22 8.5 C 22 12.28 18.6 15.36 13.45 20.04 Z"),
        "home" => Some("M 3 10 L 12 3 L 21 10 L 21 21 L 15 21 L 15 14 L 9 14 L 9 21 L 3 21 Z"),
        "user" | "person" => Some("M 12 4 C 9.79 4 8 5.79 8 8 C 8 10.21 9.79 12 12 12 C 14.21 12 16 10.21 16 8 C 16 5.79 14.21 4 12 4 Z M 4 20 C 4 16.69 7.58 14 12 14 C 16.42 14 20 16.69 20 20 Z"),
        "settings" | "cog" => Some("M 12 8 C 9.79 8 8 9.79 8 12 C 8 14.21 9.79 16 12 16 C 14.21 16 16 14.21 16 12 C 16 9.79 14.21 8 12 8 Z"),
        "menu" => Some("M 4 6 L 20 6 M 4 12 L 20 12 M 4 18 L 20 18"),
        "bell" | "notifications" => Some("M 18 8 C 18 4.69 15.31 2 12 2 C 8.69 2 6 4.69 6 8 L 6 15 L 4 17 L 20 17 L 18 15 Z"),
        "info" => Some("M 12 2 C 6.48 2 2 6.48 2 12 C 2 17.52 6.48 22 12 22 C 17.52 22 22 17.52 22 12 C 22 6.48 17.52 2 12 2 Z"),
        "sparkles" => Some("M 12 3 L 14.5 9.5 L 21 12 L 14.5 14.5 L 12 21 L 9.5 14.5 L 3 12 L 9.5 9.5 Z"),
        _ => None,
    }
}
