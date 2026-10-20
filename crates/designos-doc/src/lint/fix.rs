//! Deterministic auto-fixes for lint rules, routed through [`Command`] / [`History`].

use super::config::LintConfig;
use super::registry::{resolve_rule, LintPreset};
use super::rules::{is_default_layer_name, nearest_on_scale, on_scale, FONT_SCALE, RADIUS_SCALE, SPACING_SCALE};
use crate::openpencil::{infer_role_hint, xpath_type_name};
use crate::{Command, DocError, Document, History, LayoutDirection, NodeId, NodeKind};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Computes the [`Command`]s and `{ "fixed_count", "changes" }` payload for deterministic lint fixes
/// over `roots` using the enabled rules in `cfg`, without mutating `doc` directly.
pub fn lint_fix_commands(doc: &Document, roots: &[NodeId], cfg: &LintConfig) -> (Vec<Command>, Value) {
    let allow = |rule: &str, node_id: NodeId| -> bool { cfg.is_enabled(rule) && (cfg.include_ignored || !doc.is_lint_ignored(rule, node_id)) };

    let mut visit_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for &r in roots {
        doc.walk(r, |n, _| {
            if !n.visible || matches!(n.kind, NodeKind::Page { .. }) {
                return;
            }
            if seen.insert(n.id) {
                visit_ids.push(n.id);
            }
        });
    }

    let mut changes = Vec::new();
    let mut to_remove = Vec::new();
    let mut updated_nodes: BTreeMap<NodeId, crate::Node> = BTreeMap::new();

    for id in visit_ids {
        let Some(orig_node) = doc.get(id) else {
            continue;
        };
        let role = infer_role_hint(doc, orig_node);
        let is_empty_leaf = matches!(orig_node.kind, NodeKind::Frame(_) | NodeKind::Group)
            && orig_node.children.is_empty()
            && orig_node.fills.iter().all(|p| !p.visible)
            && orig_node.stroke.paints.iter().all(|p| !p.visible)
            && orig_node.effects.iter().all(|e| !e.visible());

        if allow("empty-container", id) && is_empty_leaf {
            to_remove.push(id);
            changes.push(json!({
                "rule": "empty-container",
                "node_id": id.0,
                "action": "removed empty container",
            }));
            continue;
        }

        if allow("misaligned-siblings", id) {
            let children = orig_node.kind.frame_props().filter(|f| f.auto_layout.is_none()).map(|_| orig_node.children.clone()).unwrap_or_default();
            for i in 0..children.len() {
                for j in (i + 1)..children.len() {
                    let a_tx_ty = updated_nodes.get(&children[i]).or_else(|| doc.get(children[i])).map(|n| (n.transform.tx, n.transform.ty));
                    let b_node = updated_nodes.get(&children[j]).cloned().or_else(|| doc.get(children[j]).cloned());
                    let (Some((atx, aty)), Some(mut bm)) = (a_tx_ty, b_node) else {
                        continue;
                    };
                    let dx = (atx - bm.transform.tx).abs();
                    let dy = (aty - bm.transform.ty).abs();
                    let mut touched = false;
                    if dx > 0.1 && dx <= 3.0 {
                        bm.transform.tx = atx;
                        touched = true;
                        changes.push(json!({
                            "rule": "misaligned-siblings",
                            "node_id": children[j].0,
                            "action": format!("snapped x to {atx}"),
                        }));
                    }
                    if dy > 0.1 && dy <= 3.0 {
                        bm.transform.ty = aty;
                        touched = true;
                        changes.push(json!({
                            "rule": "misaligned-siblings",
                            "node_id": children[j].0,
                            "action": format!("snapped y to {aty}"),
                        }));
                    }
                    if touched {
                        updated_nodes.insert(children[j], bm);
                    }
                }
            }
        }

        let mut n = updated_nodes.remove(&id).unwrap_or_else(|| orig_node.clone());
        let mut modified = false;

        if allow("default-layer-name", id) && is_default_layer_name(&n.name) {
            let new_name = match &n.kind {
                NodeKind::Text(t) => {
                    let snippet: String = t.content.trim().chars().take(24).collect();
                    if snippet.is_empty() {
                        "Text Label".to_string()
                    } else {
                        snippet
                    }
                }
                _ => match role {
                    "button" => "Button".to_string(),
                    "input" => "Input Field".to_string(),
                    "card" => "Card".to_string(),
                    "navbar" => "Navigation Bar".to_string(),
                    "badge" => "Badge".to_string(),
                    "icon" => "Icon".to_string(),
                    _ => match n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
                        Some(al) => match al.direction {
                            LayoutDirection::Horizontal => "Row".to_string(),
                            LayoutDirection::Vertical => "Column".to_string(),
                            LayoutDirection::Grid => "Grid".to_string(),
                        },
                        None => format!("{} Layer", xpath_type_name(&n.kind).to_ascii_lowercase()),
                    },
                },
            };
            n.name = new_name.clone();
            modified = true;
            changes.push(json!({
                "rule": "default-layer-name",
                "node_id": id.0,
                "action": format!("renamed to '{new_name}'"),
            }));
        }

        if allow("tiny-text", id) {
            if let Some(t) = n.kind.any_text_mut() {
                if t.style.font_size < 12.0 {
                    t.style.font_size = 12.0;
                    modified = true;
                    changes.push(json!({
                        "rule": "tiny-text",
                        "node_id": id.0,
                        "action": "bumped font_size to 12px",
                    }));
                }
            }
        }

        if allow("non-scale-font-size", id) {
            if let Some(t) = n.kind.any_text_mut() {
                if !on_scale(t.style.font_size, &FONT_SCALE) {
                    let snapped = nearest_on_scale(t.style.font_size, &FONT_SCALE);
                    t.style.font_size = snapped;
                    modified = true;
                    changes.push(json!({
                        "rule": "non-scale-font-size",
                        "node_id": id.0,
                        "action": format!("snapped font_size to {snapped}px"),
                    }));
                }
            }
        }

        if allow("small-touch-target", id) && matches!(role, "button" | "input") {
            if n.size.x < 44.0 || n.size.y < 44.0 {
                n.size.x = n.size.x.max(44.0);
                n.size.y = n.size.y.max(44.0);
                modified = true;
                changes.push(json!({
                    "rule": "small-touch-target",
                    "node_id": id.0,
                    "action": format!("expanded touch target to {}x{}", n.size.x, n.size.y),
                }));
            }
        }

        if allow("non-4px-grid", id) && !matches!(n.kind, NodeKind::Text(_) | NodeKind::Vector { .. } | NodeKind::Line) {
            let sx = (n.size.x / 4.0).round().max(1.0) * 4.0;
            let sy = (n.size.y / 4.0).round().max(1.0) * 4.0;
            if (sx - n.size.x).abs() >= 0.05 || (sy - n.size.y).abs() >= 0.05 {
                n.size.x = sx;
                n.size.y = sy;
                modified = true;
                changes.push(json!({
                    "rule": "non-4px-grid",
                    "node_id": id.0,
                    "action": format!("snapped size to {sx}x{sy}"),
                }));
            }
        }

        if allow("inconsistent-radius", id) && n.corner_radii != [0.0; 4] {
            let r0 = nearest_on_scale(n.corner_radii[0], &RADIUS_SCALE);
            if n.corner_radii != [r0; 4] {
                n.corner_radii = [r0; 4];
                modified = true;
                changes.push(json!({
                    "rule": "inconsistent-radius",
                    "node_id": id.0,
                    "action": format!("normalized corner_radii to {r0}px"),
                }));
            }
        }

        if let Some(fp) = n.kind.frame_props_mut() {
            if let Some(al) = &mut fp.auto_layout {
                if allow("inconsistent-spacing", id) && !on_scale(al.item_spacing, &SPACING_SCALE) {
                    let snapped = nearest_on_scale(al.item_spacing, &SPACING_SCALE);
                    al.item_spacing = snapped;
                    modified = true;
                    changes.push(json!({
                        "rule": "inconsistent-spacing",
                        "node_id": id.0,
                        "action": format!("snapped item_spacing to {snapped}px"),
                    }));
                }
                if allow("unbalanced-padding", id) && ((al.padding[0] - al.padding[2]).abs() > 0.1 || (al.padding[1] - al.padding[3]).abs() > 0.1) {
                    let v = nearest_on_scale((al.padding[0] + al.padding[2]) * 0.5, &SPACING_SCALE);
                    let h = nearest_on_scale((al.padding[1] + al.padding[3]) * 0.5, &SPACING_SCALE);
                    al.padding = [v, h, v, h];
                    modified = true;
                    changes.push(json!({
                        "rule": "unbalanced-padding",
                        "node_id": id.0,
                        "action": format!("balanced padding to [{v}, {h}, {v}, {h}]"),
                    }));
                }
            }
        }

        if modified || doc.get(id).is_some_and(|orig| &n != orig) {
            updated_nodes.insert(id, n);
        }
    }

    let mut cmds = Vec::new();
    for (_, node) in updated_nodes {
        cmds.push(Command::Update { node: Box::new(node) });
    }
    for rem_id in to_remove {
        cmds.push(Command::Remove { id: rem_id });
    }

    let summary = json!({
        "fixed_count": changes.len(),
        "changes": changes,
    });
    (cmds, summary)
}

fn legacy_fix_scope_and_config(doc: &Document, page: Option<NodeId>, rules: Option<&[String]>) -> (Vec<NodeId>, LintConfig) {
    let roots: Vec<NodeId> = match page {
        Some(p) => vec![p],
        None => doc.pages.clone(),
    };
    let mut cfg = LintConfig::from_preset(LintPreset::Strict);
    if let Some(list) = rules {
        if !list.is_empty() {
            cfg.enabled_rules.clear();
            for name in list {
                if let Some(spec) = resolve_rule(name) {
                    cfg.enabled_rules.insert(spec.id, spec.default_severity);
                }
            }
        }
    }
    (roots, cfg)
}

/// Applies deterministic auto-fixes as a single undoable [`Command::Batch`] in `history`.
pub fn lint_fix_with_history(doc: &mut Document, history: &mut History, page: Option<NodeId>, rules: Option<&[String]>) -> Result<Value, DocError> {
    let (roots, cfg) = legacy_fix_scope_and_config(doc, page, rules);
    let (cmds, summary) = lint_fix_commands(doc, &roots, &cfg);
    if !cmds.is_empty() {
        history.exec(doc, Command::Batch(cmds))?;
    }
    Ok(summary)
}

/// Applies deterministic auto-fixes for enabled lint rules and returns `{ "fixed_count", "changes" }`.
pub fn lint_fix_document(doc: &mut Document, page: Option<NodeId>, rules: Option<&[String]>) -> Value {
    let (roots, cfg) = legacy_fix_scope_and_config(doc, page, rules);
    let (cmds, summary) = lint_fix_commands(doc, &roots, &cfg);
    if !cmds.is_empty() {
        let _ = Command::Batch(cmds).apply(doc);
    }
    summary
}
