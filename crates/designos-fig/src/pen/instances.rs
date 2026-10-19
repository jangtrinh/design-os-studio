//! Component instance (`ref`) resolution, property inheritance, descendant & slot overrides,
//! and external `.pen` library `imports` loading.

use super::migrate::migrate_to_current;
use super::node::{
    builtin_icon_svg_path, extract_icon_family_and_name, import_pen_node, parse_font_weight, parse_frame_props, parse_pen_align, parse_pen_padding,
    parse_pen_sizing, PenImportCtx, PendingInstance,
};
use super::paint::{parse_corner_radius, parse_paints, parse_stroke, resolve_num_or_var};
use super::variables::{import_pen_variables, resolve_doc_var_string};
use super::PenWarning;
use crate::svgpath::parse_path;
use designos_doc::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

pub const MAX_LIBRARY_IMPORT_DEPTH: usize = 8;

/// Resolves top-level `"imports": { "<alias>": "<relative-path>.pen" }` inside `ctx.base_dir`.
pub fn resolve_pen_library_imports(root: &Value, doc: &mut Document, page_id: NodeId, ctx: &mut PenImportCtx, visited_paths: &mut Vec<PathBuf>, depth: usize) {
    let Some(imports_obj) = root.get("imports").and_then(Value::as_object) else {
        return;
    };
    if imports_obj.is_empty() {
        return;
    }

    if depth == 0 {
        if let Ok(serialized) = serde_json::to_string(imports_obj) {
            if let Some(page) = doc.get_mut(page_id) {
                page.plugin_data.insert("pen:imports".into(), serialized);
            }
        }
    }

    let Some(base_dir) = ctx.base_dir.clone() else {
        for (alias, rel_v) in imports_obj {
            let rel = rel_v.as_str().unwrap_or("");
            ctx.warnings.push(PenWarning {
                code: "unresolved_import".into(),
                node_id: None,
                message: format!("Cannot resolve library import '{alias}' ('{rel}') without PenImportOptions::base_dir"),
            });
        }
        return;
    };

    if depth >= MAX_LIBRARY_IMPORT_DEPTH {
        ctx.warnings.push(PenWarning {
            code: "import_depth_exceeded".into(),
            node_id: None,
            message: format!("Library import depth exceeded maximum of {MAX_LIBRARY_IMPORT_DEPTH}"),
        });
        return;
    }

    let Ok(canon_base) = base_dir.canonicalize() else {
        ctx.warnings.push(PenWarning {
            code: "import_load_failed".into(),
            node_id: None,
            message: format!("Cannot canonicalize base_dir '{}'", base_dir.display()),
        });
        return;
    };

    for (alias, rel_v) in imports_obj {
        let Some(rel_str) = rel_v.as_str() else { continue };
        let rel_path = Path::new(rel_str);
        if rel_path.is_absolute() || rel_path.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
            ctx.warnings.push(PenWarning {
                code: "import_path_refused".into(),
                node_id: None,
                message: format!("Refused library import '{alias}' ('{rel_str}') because it escapes base_dir"),
            });
            continue;
        }

        let candidate = canon_base.join(rel_path);
        let Ok(canon_target) = candidate.canonicalize() else {
            ctx.warnings.push(PenWarning {
                code: "import_load_failed".into(),
                node_id: None,
                message: format!("Library import '{alias}' ('{rel_str}') could not be found inside base_dir"),
            });
            continue;
        };

        if !canon_target.starts_with(&canon_base) {
            ctx.warnings.push(PenWarning {
                code: "import_path_refused".into(),
                node_id: None,
                message: format!("Refused library import '{alias}' ('{rel_str}') because its canonical path escapes base_dir"),
            });
            continue;
        }

        if visited_paths.contains(&canon_target) {
            ctx.warnings.push(PenWarning {
                code: "import_cycle".into(),
                node_id: None,
                message: format!("Cyclic library import detected for '{rel_str}'; skipping"),
            });
            continue;
        }

        let Ok(lib_raw) = std::fs::read_to_string(&canon_target) else {
            ctx.warnings.push(PenWarning { code: "import_load_failed".into(), node_id: None, message: format!("Failed to read library file '{rel_str}'") });
            continue;
        };

        let Ok(mut lib_root) = serde_json::from_str::<Value>(&lib_raw) else {
            ctx.warnings.push(PenWarning { code: "import_load_failed".into(), node_id: None, message: format!("Invalid JSON in library file '{rel_str}'") });
            continue;
        };

        if let Err(err) = migrate_to_current(&mut lib_root, &mut ctx.warnings) {
            ctx.warnings.push(PenWarning {
                code: "import_load_failed".into(),
                node_id: None,
                message: format!("Cannot migrate library file '{rel_str}': {err}"),
            });
            continue;
        }

        // Import library variables that do not collide with existing variables.
        let mut temp_doc = Document::new("lib");
        import_pen_variables(&lib_raw, &lib_root, &mut temp_doc);
        for (cid, col) in temp_doc.variable_collections {
            doc.variable_collections.entry(cid).or_insert(col);
        }
        for (vid, var) in temp_doc.variables {
            doc.variables.entry(vid).or_insert(var);
        }

        let lib_page = ensure_library_page(doc);
        let saved_base_dir = ctx.base_dir.clone();
        ctx.base_dir = canon_target.parent().map(Path::to_path_buf);
        visited_paths.push(canon_target);

        resolve_pen_library_imports(&lib_root, doc, lib_page, ctx, visited_paths, depth + 1);

        if let Some(children) = lib_root.get("children").and_then(Value::as_array) {
            for ch in children {
                let before_id = ch.get("id").and_then(Value::as_str).map(String::from);
                if let Some(imported_id) = import_pen_node(ch, doc, lib_page, ctx) {
                    if let Some(raw_id) = before_id {
                        let is_comp = matches!(doc.get(imported_id).map(|n| &n.kind), Some(NodeKind::Component(_)));
                        if is_comp {
                            ctx.components_by_source_id.insert(format!("{alias}:{raw_id}"), imported_id);
                            ctx.components_by_source_id.entry(raw_id).or_insert(imported_id);
                        }
                    }
                }
            }
        }

        visited_paths.pop();
        ctx.base_dir = saved_base_dir;
    }
}

fn ensure_library_page(doc: &mut Document) -> NodeId {
    if let Some(lp) = doc.library_page {
        if doc.get(lp).is_some() {
            return lp;
        }
    }
    let id = doc.alloc_id();
    let mut page = Node::new(id, NodeKind::Page { background: Color::rgb(0xF5, 0xF5, 0xF5) });
    page.name = "Library".into();
    doc.nodes.insert(id, page);
    doc.library_page = Some(id);
    id
}

/// Resolves slot `preferred` component IDs from `plugin_data["pen:slot"]` onto the enclosing
/// component's `slot_configs`.
pub fn resolve_slot_preferred_components(doc: &mut Document, components_by_source_id: &HashMap<String, NodeId>) {
    let slot_nodes: Vec<(NodeId, String, Vec<String>)> = doc
        .nodes
        .values()
        .filter_map(|n| {
            if !n.component.is_slot {
                return None;
            }
            let raw = n.plugin_data.get("pen:slot")?;
            let ids: Vec<String> = serde_json::from_str(raw).ok()?;
            let prop_name = n.component.bindings.get(&OverrideField::SlotContent).cloned().unwrap_or_else(|| n.name.clone());
            Some((n.id, prop_name, ids))
        })
        .collect();

    for (slot_id, prop_name, pref_strs) in slot_nodes {
        let preferred: Vec<NodeId> = pref_strs.iter().filter_map(|s| components_by_source_id.get(s).copied()).collect();
        if let Some(comp_id) = find_enclosing_component(doc, slot_id) {
            if let Some(comp) = doc.get_mut(comp_id) {
                let cfg = comp.component.slot_configs.entry(prop_name).or_default();
                cfg.preferred = preferred;
            }
        }
    }
}

fn find_enclosing_component(doc: &Document, start: NodeId) -> Option<NodeId> {
    let mut cur = doc.get(start)?.parent;
    while let Some(id) = cur {
        let n = doc.get(id)?;
        if matches!(n.kind, NodeKind::Component(_)) {
            return Some(id);
        }
        cur = n.parent;
    }
    None
}

pub fn subtree_len_up_to(doc: &Document, roots: &[NodeId], limit: usize) -> usize {
    let mut count = 0usize;
    let mut stack: Vec<NodeId> = roots.to_vec();
    while let Some(id) = stack.pop() {
        count += 1;
        if count > limit {
            return count;
        }
        if let Some(n) = doc.get(id) {
            stack.extend(n.children.iter().copied());
        }
    }
    count
}

pub fn resolve_pen_instances(doc: &mut Document, ctx: &mut PenImportCtx, clone_budget: usize) -> Result<(), String> {
    resolve_slot_preferred_components(doc, &ctx.components_by_source_id);

    let mut remaining_clones = clone_budget;
    let mut skipped_instances = 0usize;

    let initial_pending = std::mem::take(&mut ctx.pending_instances);
    expand_pending_list(doc, ctx, initial_pending, &mut remaining_clones, &mut skipped_instances)?;

    if skipped_instances > 0 {
        ctx.warnings.push(PenWarning {
            code: "clone_budget_exhausted".into(),
            node_id: None,
            message: format!(
                ".pen import skipped expanding {skipped_instances} instance(s) because the component clone tree would expand past {clone_budget} nodes"
            ),
        });
    }

    Ok(())
}

fn expand_pending_list(
    doc: &mut Document,
    ctx: &mut PenImportCtx,
    pending_list: Vec<PendingInstance>,
    remaining_clones: &mut usize,
    skipped_instances: &mut usize,
) -> Result<(), String> {
    for pending in pending_list {
        let inst_id = pending.inst_id;
        let ref_source_id = &pending.ref_source_id;

        let resolved_comp_id = ctx
            .components_by_source_id
            .get(ref_source_id)
            .copied()
            .or_else(|| ref_source_id.split_once(':').and_then(|(_, bare)| ctx.components_by_source_id.get(bare).copied()));

        let Some(comp_id) = resolved_comp_id else {
            if doc.nodes.values().any(|n| n.id == inst_id && n.source_id.as_deref() == Some(ref_source_id.as_str())) {
                return Err(format!("Component '{ref_source_id}' cannot be referenced from inside itself"));
            }
            ctx.warnings.push(PenWarning {
                code: "unresolved_ref".into(),
                node_id: doc.get(inst_id).and_then(|n| n.source_id.clone()),
                message: format!("Instance references unknown component '{ref_source_id}'"),
            });
            continue;
        };

        if inst_id == comp_id || doc.is_ancestor(comp_id, inst_id) {
            return Err(format!("Component '{ref_source_id}' cannot be referenced from inside itself"));
        }

        // Spec §3.1: inherit unset properties from the target component and record OverrideField
        // only for properties explicitly overridden on the `ref` node.
        apply_instance_inheritance(doc, inst_id, comp_id, &pending);

        if *remaining_clones == 0 {
            if doc.get(comp_id).is_some_and(|c| !c.children.is_empty()) {
                *skipped_instances += 1;
            }
            continue;
        }

        let comp_children = doc.get(comp_id).map(|c| c.children.clone()).unwrap_or_default();
        if !comp_children.is_empty() {
            let needed = subtree_len_up_to(doc, &comp_children, *remaining_clones);
            if needed > *remaining_clones {
                *remaining_clones = 0;
                *skipped_instances += 1;
                continue;
            }
            *remaining_clones -= needed;
        }

        clone_component_children_into_instance(doc, comp_id, inst_id);

        // If the component is a reusable wrapper around a non-frame primitive, forward direct
        // overrides on the `ref` (e.g., `content`, `fill`, `width`/`height`) onto its inner child.
        let is_reusable_wrapper = doc.get(comp_id).and_then(|c| c.plugin_data.get("pen:reusable_wrapper")).is_some_and(|v| v == "true");
        if is_reusable_wrapper {
            if let Some(&inner_child_id) = doc.get(inst_id).and_then(|n| n.children.first()) {
                let inst_snapshot = doc.get(inst_id).cloned();
                if let (Some(inst_n), Some(inner_n)) = (inst_snapshot, doc.get_mut(inner_child_id)) {
                    if pending.explicit_width || pending.explicit_height {
                        inner_n.size = inst_n.size;
                        inner_n.component.overrides.insert(OverrideField::Size);
                    }
                    if pending.explicit_fill {
                        if let Some(tp) = inner_n.kind.any_text_mut() {
                            tp.style.fills = inst_n.fills.clone();
                        } else {
                            inner_n.fills = inst_n.fills.clone();
                        }
                        inner_n.component.overrides.insert(OverrideField::Fills);
                    }
                    if let Some(text_content) = &pending.explicit_content {
                        if let Some(tp) = inner_n.kind.any_text_mut() {
                            tp.content = text_content.clone();
                        }
                        inner_n.component.overrides.insert(OverrideField::Characters);
                    }
                }
            }
        }

        // Apply `descendants` overrides (including slot `children` replacement).
        if let Some(desc_obj) = pending.descendants.as_ref().and_then(Value::as_object) {
            for (desc_path, ov_val) in desc_obj {
                let Some(ov_map) = ov_val.as_object() else { continue };
                if let Some(target_id) = find_cloned_descendant_by_path(doc, inst_id, desc_path) {
                    apply_descendant_overrides(doc, target_id, ov_map, ctx, remaining_clones, skipped_instances)?;
                }
            }
        }
    }
    Ok(())
}

fn apply_instance_inheritance(doc: &mut Document, inst_id: NodeId, comp_id: NodeId, pending: &PendingInstance) {
    let Some(comp) = doc.get(comp_id).cloned() else { return };
    let Some(inst) = doc.get_mut(inst_id) else { return };

    if !pending.explicit_width {
        inst.size.x = comp.size.x;
        inst.layout_child.horizontal = comp.layout_child.horizontal;
        inst.layout_child.min_w = comp.layout_child.min_w;
        inst.layout_child.max_w = comp.layout_child.max_w;
        if let Some(wv) = comp.variables.bindings.get("width") {
            inst.variables.bindings.insert("width".into(), wv.clone());
        }
    }
    if !pending.explicit_height {
        inst.size.y = comp.size.y;
        inst.layout_child.vertical = comp.layout_child.vertical;
        inst.layout_child.min_h = comp.layout_child.min_h;
        inst.layout_child.max_h = comp.layout_child.max_h;
        if let Some(hv) = comp.variables.bindings.get("height") {
            inst.variables.bindings.insert("height".into(), hv.clone());
        }
    }
    if pending.explicit_width || pending.explicit_height {
        inst.component.overrides.insert(OverrideField::Size);
    }

    if !pending.explicit_fill {
        inst.fills = comp.fills.clone();
        if let Some(fv) = comp.variables.bindings.get("fills/0") {
            inst.variables.bindings.insert("fills/0".into(), fv.clone());
        }
    } else {
        inst.component.overrides.insert(OverrideField::Fills);
    }

    if !pending.explicit_stroke {
        inst.stroke = comp.stroke.clone();
        if let Some(sv) = comp.variables.bindings.get("strokes/0") {
            inst.variables.bindings.insert("strokes/0".into(), sv.clone());
        }
        if let Some(swv) = comp.variables.bindings.get("strokeWeight") {
            inst.variables.bindings.insert("strokeWeight".into(), swv.clone());
        }
    } else {
        inst.component.overrides.insert(OverrideField::Stroke);
    }

    if !pending.explicit_corner_radius {
        inst.corner_radii = comp.corner_radii;
        if let Some(rv) = comp.variables.bindings.get("cornerRadius") {
            inst.variables.bindings.insert("cornerRadius".into(), rv.clone());
        }
    } else {
        inst.component.overrides.insert(OverrideField::CornerRadii);
    }

    let comp_fp = comp.kind.frame_props().cloned().unwrap_or_default();
    let w_sizing = inst.layout_child.horizontal;
    let h_sizing = inst.layout_child.vertical;

    if let NodeKind::Instance { frame, component, .. } = &mut inst.kind {
        *component = Some(comp_id);
        if !pending.explicit_layout {
            *frame = comp_fp;
        } else {
            inst.component.overrides.insert(OverrideField::Layout);
        }
        if let Some(al) = &mut frame.auto_layout {
            match al.direction {
                LayoutDirection::Horizontal | LayoutDirection::Grid => {
                    al.primary_sizing = if w_sizing == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                    al.counter_sizing = if h_sizing == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                }
                LayoutDirection::Vertical => {
                    al.primary_sizing = if h_sizing == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                    al.counter_sizing = if w_sizing == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                }
            }
        }
    }
}

pub fn clone_component_children_into_instance(doc: &mut Document, comp_id: NodeId, inst_id: NodeId) {
    let Some(comp_children) = doc.get(comp_id).map(|c| c.children.clone()) else { return };
    let mut stack: Vec<(NodeId, NodeId)> = comp_children.into_iter().rev().map(|cid| (cid, inst_id)).collect();
    while let Some((src_id, dst_parent_id)) = stack.pop() {
        let Some(orig) = doc.get(src_id).cloned() else { continue };
        let new_id = doc.alloc_id();
        let mut n = orig.clone();
        n.id = new_id;
        n.parent = Some(dst_parent_id);
        n.children.clear();
        n.component.source = Some(src_id);
        n.component.overrides.clear();
        n.component.props.clear();
        n.component.slot_configs.clear();
        let _ = doc.insert(dst_parent_id, n);
        for &ch_id in orig.children.iter().rev() {
            stack.push((ch_id, new_id));
        }
    }
}

pub fn find_cloned_descendant_by_path(doc: &Document, root_id: NodeId, path: &str) -> Option<NodeId> {
    if let Some(direct) = find_cloned_descendant_by_source_id(doc, root_id, path) {
        return Some(direct);
    }
    let mut cur = root_id;
    for part in path.split('/').filter(|s| !s.is_empty()) {
        cur = find_cloned_descendant_by_source_id(doc, cur, part)?;
    }
    if cur == root_id {
        None
    } else {
        Some(cur)
    }
}

pub fn find_cloned_descendant_by_source_id(doc: &Document, root_id: NodeId, target_source_id: &str) -> Option<NodeId> {
    let mut stack: Vec<NodeId> = doc.get(root_id)?.children.iter().copied().rev().collect();
    while let Some(cid) = stack.pop() {
        let Some(cn) = doc.get(cid) else { continue };
        let matches_source = cn.component.source.and_then(|s| doc.get(s)).and_then(|sn| sn.source_id.as_deref()).is_some_and(|sid| sid == target_source_id)
            || cn.source_id.as_deref() == Some(target_source_id);
        if matches_source {
            return Some(cid);
        }
        stack.extend(cn.children.iter().copied().rev());
    }
    None
}

fn remove_subtree(doc: &mut Document, root_id: NodeId) {
    let mut stack = vec![root_id];
    let mut post_order = Vec::new();
    while let Some(id) = stack.pop() {
        post_order.push(id);
        if let Some(n) = doc.get(id) {
            stack.extend(n.children.iter().copied());
        }
    }
    for id in post_order.into_iter().rev() {
        let _ = doc.remove(id);
    }
}

fn apply_descendant_overrides(
    doc: &mut Document,
    target_id: NodeId,
    ov_map: &serde_json::Map<String, Value>,
    ctx: &mut PenImportCtx,
    remaining_clones: &mut usize,
    skipped_instances: &mut usize,
) -> Result<(), String> {
    let ov_val = Value::Object(ov_map.clone());
    let target_source_id = doc.get(target_id).and_then(|n| n.source_id.clone());
    let base_dir_ref = ctx.base_dir.clone();

    // 1. Text content override
    if let Some(content_val) = ov_map.get("content") {
        if let Some(s) = content_val.as_str() {
            let resolved = if let Some(vn) = s.strip_prefix('$') { resolve_doc_var_string(doc, vn).unwrap_or_else(|| s.to_string()) } else { s.to_string() };
            if let Some(n) = doc.get_mut(target_id) {
                if let Some(tp) = n.kind.any_text_mut() {
                    tp.content = resolved;
                    n.component.overrides.insert(OverrideField::Characters);
                }
            }
        }
    }

    // 2. Fill override
    if let Some(fill_v) = ov_map.get("fill") {
        let mut pdata = doc.get(target_id).map(|n| n.plugin_data.clone()).unwrap_or_default();
        let (paints, fvar) =
            parse_paints(Some(fill_v), doc, target_source_id.as_deref(), base_dir_ref.as_deref(), &mut ctx.total_image_bytes, &mut pdata, &mut ctx.warnings);
        if let Some(n) = doc.get_mut(target_id) {
            n.plugin_data = pdata;
            if let Some(tp) = n.kind.any_text_mut() {
                tp.style.fills = paints;
            } else {
                n.fills = paints;
            }
            if let Some(fv) = fvar {
                n.variables.bindings.insert("fills/0".into(), fv);
            } else {
                n.variables.bindings.remove("fills/0");
            }
            n.component.overrides.insert(OverrideField::Fills);
        }
    }

    // 3. Stroke override
    if ov_map.contains_key("stroke")
        || ov_map.contains_key("strokeWidth")
        || ov_map.contains_key("strokeAlignment")
        || ov_map.contains_key("strokeCap")
        || ov_map.contains_key("strokeJoin")
    {
        let mut pdata = doc.get(target_id).map(|n| n.plugin_data.clone()).unwrap_or_default();
        let (stroke, scv, swv) =
            parse_stroke(&ov_val, doc, target_source_id.as_deref(), base_dir_ref.as_deref(), &mut ctx.total_image_bytes, &mut pdata, &mut ctx.warnings);
        if let Some(n) = doc.get_mut(target_id) {
            n.plugin_data = pdata;
            if stroke.paints.is_empty() && !ov_map.contains_key("stroke") {
                // Only width/align/cap/join were overridden; keep existing stroke paints.
                let existing_paints = n.stroke.paints.clone();
                n.stroke = stroke;
                n.stroke.paints = existing_paints;
            } else {
                n.stroke = stroke;
            }
            if let Some(cv) = scv {
                n.variables.bindings.insert("strokes/0".into(), cv);
            }
            if let Some(wv) = swv {
                n.variables.bindings.insert("strokeWeight".into(), wv);
            }
            n.component.overrides.insert(OverrideField::Stroke);
        }
    }

    // 4. Size / width / height override
    if ov_map.contains_key("width") || ov_map.contains_key("height") {
        let cur_size = doc.get(target_id).map(|n| n.size).unwrap_or(Vec2::new(100.0, 100.0));
        let w_parsed = ov_map.get("width").map(|wv| parse_pen_sizing(Some(wv), doc, cur_size.x));
        let h_parsed = ov_map.get("height").map(|hv| parse_pen_sizing(Some(hv), doc, cur_size.y));
        if let Some(n) = doc.get_mut(target_id) {
            if let Some((w, ws, wv)) = w_parsed {
                n.size.x = w;
                n.layout_child.horizontal = ws;
                if let Some(v) = wv {
                    n.variables.bindings.insert("width".into(), v);
                }
            }
            if let Some((h, hs, hv)) = h_parsed {
                n.size.y = h;
                n.layout_child.vertical = hs;
                if let Some(v) = hv {
                    n.variables.bindings.insert("height".into(), v);
                }
            }
            n.component.overrides.insert(OverrideField::Size);
        }
    }

    // 5. Enabled / visible override
    if let Some(en) = ov_map.get("enabled").and_then(Value::as_bool) {
        if let Some(n) = doc.get_mut(target_id) {
            n.visible = en;
            n.component.overrides.insert(OverrideField::Visible);
        }
    }

    // 6. Corner radius override
    if let Some(cr_val) = ov_map.get("cornerRadius") {
        let (radii, rvar) = parse_corner_radius(Some(cr_val), doc);
        if let Some(n) = doc.get_mut(target_id) {
            n.corner_radii = radii;
            if let Some(rv) = rvar {
                n.variables.bindings.insert("cornerRadius".into(), rv);
            }
            n.component.overrides.insert(OverrideField::CornerRadii);
        }
    }

    // 7. Layout / padding / gap overrides
    if ov_map.contains_key("padding")
        || ov_map.contains_key("gap")
        || ov_map.contains_key("layout")
        || ov_map.contains_key("justifyContent")
        || ov_map.contains_key("alignItems")
        || ov_map.contains_key("clip")
    {
        let pad_opt = ov_map.get("padding").map(|p| parse_pen_padding(Some(p), doc));
        let gap_opt = ov_map.get("gap").and_then(|g| resolve_num_or_var(g, doc));
        let jc_opt = ov_map.get("justifyContent").and_then(Value::as_str).map(|s| parse_pen_align(Some(s)));
        let ai_opt = ov_map.get("alignItems").and_then(Value::as_str).map(|s| parse_pen_align(Some(s)));
        let clip_opt = ov_map.get("clip").and_then(Value::as_bool);
        let layout_opt = ov_map.get("layout").and_then(Value::as_str);

        let (ws, hs) = doc.get(target_id).map(|n| (n.layout_child.horizontal, n.layout_child.vertical)).unwrap_or((Sizing::Fixed, Sizing::Fixed));
        let fallback_fp = parse_frame_props(&ov_val, doc, ws, hs, true);

        if let Some(n) = doc.get_mut(target_id) {
            if let Some(fp) = n.kind.frame_props_mut() {
                if let Some(c) = clip_opt {
                    fp.clips_content = c;
                }
                if layout_opt == Some("none") {
                    fp.auto_layout = None;
                } else {
                    let al = fp.auto_layout.get_or_insert_with(AutoLayout::v1);
                    if let Some(lstr) = layout_opt {
                        al.direction = match lstr {
                            "vertical" => LayoutDirection::Vertical,
                            "grid" => LayoutDirection::Grid,
                            _ => LayoutDirection::Horizontal,
                        };
                    }
                    if let Some(pad) = pad_opt {
                        al.padding = pad;
                    }
                    if let Some(gap) = gap_opt {
                        al.item_spacing = gap;
                        al.counter_spacing = gap;
                    }
                    if let Some(jc) = jc_opt {
                        al.primary_align = jc;
                    }
                    if let Some(ai) = ai_opt {
                        al.counter_align = ai;
                    }
                }
            } else if layout_opt.is_some() || pad_opt.is_some() || gap_opt.is_some() {
                n.kind = NodeKind::Frame(fallback_fp);
            }
            n.component.overrides.insert(OverrideField::Layout);
        }
    }

    // 8. Font / text style overrides
    if ov_map.contains_key("fontFamily")
        || ov_map.contains_key("fontSize")
        || ov_map.contains_key("fontWeight")
        || ov_map.contains_key("fontStyle")
        || ov_map.contains_key("letterSpacing")
        || ov_map.contains_key("lineHeight")
        || ov_map.contains_key("textAlign")
        || ov_map.contains_key("underline")
        || ov_map.contains_key("strikethrough")
        || ov_map.contains_key("textDecoration")
    {
        let ff_opt = ov_map.get("fontFamily").and_then(Value::as_str).map(|s| {
            if let Some(vn) = s.strip_prefix('$') {
                resolve_doc_var_string(doc, vn).unwrap_or_else(|| "Inter".into())
            } else {
                s.to_string()
            }
        });
        let fs_opt = ov_map.get("fontSize").and_then(|v| resolve_num_or_var(v, doc));
        let fw_opt = ov_map.get("fontWeight").map(|v| parse_font_weight(v, doc));
        let italic_opt = ov_map.get("fontStyle").and_then(Value::as_str).map(|s| s == "italic");
        let ls_opt = ov_map.get("letterSpacing").and_then(|v| resolve_num_or_var(v, doc));
        let lh_opt = ov_map.get("lineHeight").and_then(|v| resolve_num_or_var(v, doc));

        if let Some(n) = doc.get_mut(target_id) {
            if let Some(tp) = n.kind.any_text_mut() {
                if let Some(ff) = ff_opt {
                    tp.style.font_family = ff;
                }
                if let Some(fs) = fs_opt {
                    tp.style.font_size = fs.max(1.0);
                }
                if let Some(fw) = fw_opt {
                    tp.style.font_weight = fw;
                }
                if let Some(it) = italic_opt {
                    tp.style.italic = it;
                }
                if let Some(ls) = ls_opt {
                    tp.style.letter_spacing = ls;
                }
                if let Some(lh) = lh_opt {
                    tp.style.line_height = if lh <= 4.0 { LineHeight::Percent(lh * 100.0) } else { LineHeight::Pixels(lh) };
                }
                if ov_map.get("underline").and_then(Value::as_bool) == Some(true) {
                    tp.style.decoration = TextDecoration::Underline;
                } else if ov_map.get("strikethrough").and_then(Value::as_bool) == Some(true) {
                    tp.style.decoration = TextDecoration::Strikethrough;
                }
                n.component.overrides.insert(OverrideField::TextStyle);
            }
        }
    }

    // 9. Icon overrides (`iconFontName`, `iconFontFamily`, `icon`)
    if ov_map.contains_key("iconFontName") || ov_map.contains_key("iconFontFamily") || ov_map.contains_key("icon") {
        if let Some(n) = doc.get_mut(target_id) {
            let existing_meta: Value =
                n.plugin_data.get("pen:icon").and_then(|s| serde_json::from_str(s).ok()).unwrap_or_else(|| json!({ "family": "lucide", "name": "icon" }));
            let (mut fam, mut iname) = (
                existing_meta.get("family").and_then(Value::as_str).unwrap_or("lucide").to_string(),
                existing_meta.get("name").and_then(Value::as_str).unwrap_or("icon").to_string(),
            );
            let (parsed_fam, parsed_name) = extract_icon_family_and_name(&ov_val);
            if ov_map.contains_key("iconFontFamily") {
                fam = parsed_fam;
            }
            if ov_map.contains_key("iconFontName") || ov_map.contains_key("icon") {
                iname = parsed_name;
            }
            n.plugin_data.insert("pen:icon".into(), json!({ "family": fam, "name": iname }).to_string());
            n.plugin_data.insert("pen:icon_overridden".into(), "true".into());
            if let Some(d_str) = builtin_icon_svg_path(&fam, &iname) {
                let mut path = parse_path(d_str, FillRule::NonZero);
                let (w, h) = (n.size.x.max(1.0), n.size.y.max(1.0));
                for cmd in &mut path.cmds {
                    let scale = |p: Vec2| Vec2::new(p.x * (w / 24.0), p.y * (h / 24.0));
                    match cmd {
                        PathCmd::MoveTo(p) | PathCmd::LineTo(p) => *p = scale(*p),
                        PathCmd::QuadTo(p1, p2) => {
                            *p1 = scale(*p1);
                            *p2 = scale(*p2);
                        }
                        PathCmd::CubicTo(p1, p2, p3) => {
                            *p1 = scale(*p1);
                            *p2 = scale(*p2);
                            *p3 = scale(*p3);
                        }
                        PathCmd::Close => {}
                    }
                }
                n.kind = NodeKind::Vector { paths: vec![path], network: None };
            }
            n.component.overrides.insert(OverrideField::Characters);
        }
    }

    // 10. Spec §3.2: Slot content replacement (`descendants[..].children`)
    if let Some(children_arr) = ov_map.get("children").and_then(Value::as_array) {
        let old_children = doc.get(target_id).map(|n| n.children.clone()).unwrap_or_default();
        for old_cid in old_children {
            remove_subtree(doc, old_cid);
        }

        // Ensure target can hold children (if it wasn't already a frame/container).
        if let Some(tn) = doc.get_mut(target_id) {
            tn.component.is_slot = true;
            tn.component.overrides.insert(OverrideField::SlotContent);
        }

        let mut inserted_roots = Vec::new();
        for ch_json in children_arr {
            if let Some(cid) = import_pen_node(ch_json, doc, target_id, ctx) {
                inserted_roots.push(cid);
            }
        }

        // Expand any nested `ref` instances inside the newly inserted slot children.
        let nested_pending = std::mem::take(&mut ctx.pending_instances);
        if !nested_pending.is_empty() {
            expand_pending_list(doc, ctx, nested_pending, remaining_clones, skipped_instances)?;
        }

        for cid in inserted_roots {
            designos_doc::components::mark_slot_content_subtree(doc, cid);
        }
        if let Some(tn) = doc.get_mut(target_id) {
            tn.component.overrides.insert(OverrideField::SlotContent);
        }
    }

    Ok(())
}

pub fn resolve_pen_connections(doc: &mut Document, pending_connections: &[(NodeId, Value)]) {
    let mut by_source_id: HashMap<String, NodeId> = HashMap::new();
    for (&nid, node) in &doc.nodes {
        if let Some(sid) = &node.source_id {
            by_source_id.entry(sid.clone()).or_insert(nid);
        }
    }

    for (conn_id, v) in pending_connections {
        let start = parse_pen_endpoint(v.get("from").or_else(|| v.get("start")), doc, &by_source_id);
        let end = parse_pen_endpoint(v.get("to").or_else(|| v.get("end")), doc, &by_source_id);
        let label = v.get("content").or_else(|| v.get("label")).and_then(Value::as_str).map(|s| TextProps { content: s.to_string(), ..Default::default() });
        if let Some(node) = doc.get_mut(*conn_id) {
            node.kind = NodeKind::Connector(ConnectorProps {
                start,
                end,
                line: ConnectorLine::Elbowed,
                start_cap: ConnectorCap::None,
                end_cap: ConnectorCap::Arrow,
                label,
            });
        }
    }
}

fn parse_pen_endpoint(v: Option<&Value>, doc: &Document, by_source_id: &HashMap<String, NodeId>) -> ConnectorEnd {
    let Some(val) = v else {
        return ConnectorEnd::free(Vec2::ZERO);
    };
    if let Some(sid) = val.as_str() {
        if let Some(&target_id) = by_source_id.get(sid) {
            return ConnectorEnd::attached(target_id, Magnet::Auto);
        }
    }
    if let Some(obj) = val.as_object() {
        if let Some(sid) = obj.get("id").or_else(|| obj.get("node")).and_then(Value::as_str) {
            if let Some(&target_id) = by_source_id.get(sid) {
                let magnet = match obj.get("magnet").or_else(|| obj.get("side")).and_then(Value::as_str) {
                    Some("top") | Some("north") => Magnet::Top,
                    Some("right") | Some("east") => Magnet::Right,
                    Some("bottom") | Some("south") => Magnet::Bottom,
                    Some("left") | Some("west") => Magnet::Left,
                    Some("center") => Magnet::Center,
                    _ => Magnet::Auto,
                };
                return ConnectorEnd::attached(target_id, magnet);
            }
        }
        let x = obj.get("x").and_then(|x| resolve_num_or_var(x, doc)).unwrap_or(0.0);
        let y = obj.get("y").and_then(|y| resolve_num_or_var(y, doc)).unwrap_or(0.0);
        return ConnectorEnd::free(Vec2::new(x, y));
    }
    ConnectorEnd::free(Vec2::ZERO)
}
