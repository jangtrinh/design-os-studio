//! Deterministic auto-fixes for lint rules, routed through [`Command`] / [`History`].

use super::config::LintConfig;
use super::registry::{resolve_rule, LintPreset};
use super::rules::{is_default_layer_name, nearest_on_scale, SPACING_SCALE};
use super::token_match::{number_inherited, Index as TokenIndex, Match, Slot, PADDING_KEYS};
use crate::openpencil::{infer_role_hint, xpath_type_name};
use crate::{set_bound_property, Command, DocError, Document, History, LayoutDirection, NodeId, NodeKind, PaintKind};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Computes the [`Command`]s and `{ "fixed_count", "changes" }` payload for deterministic lint fixes
/// over `roots` using the enabled rules in `cfg`, without mutating `doc` directly.
pub fn lint_fix_commands(doc: &Document, roots: &[NodeId], cfg: &LintConfig) -> (Vec<Command>, Value) {
    let allow = |rule: &str, node_id: NodeId| -> bool { cfg.is_enabled(rule) && (cfg.include_ignored || !doc.is_lint_ignored(rule, node_id)) };

    let token_index = TokenIndex::new(doc);
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
    let mut warnings = Vec::new();
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
                    if !cfg.include_ignored && doc.is_lint_ignored("misaligned-siblings", children[j]) {
                        continue;
                    }
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
        let src_resolved = n.component.source.and_then(|s| crate::resolved_node(doc, s));
        let src = src_resolved.as_deref();

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

        if allow("tiny-text", id) && !number_inherited(&n, src, "fontSize") {
            let before = n.kind.any_text().filter(|t| t.style.font_size < 12.0 && n.style_refs.contains_key("text")).cloned();
            if let Some(t) = n.kind.any_text_mut() {
                if t.style.font_size < 12.0 {
                    t.style.font_size = 12.0;
                    if let Some(before) = before.as_ref() {
                        if crate::apply_text_link_rule(&mut n, before) == crate::LinkOutcome::Detached {
                            warnings.push(json!({
                                "code": "STYLE_DETACHED_BY_LINT_FIX",
                                "path": format!("nodes[{}].font_size", id.0),
                                "message": "A lint font-size fix detached the linked text style.",
                            }));
                        }
                    }
                    modified = true;
                    changes.push(json!({
                        "rule": "tiny-text",
                        "node_id": id.0,
                        "action": "bumped font_size to 12px",
                    }));
                }
            }
        }

        if allow("small-touch-target", id) && matches!(role, "button" | "input") && !matches!(n.kind, NodeKind::Connector(_)) {
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

        if allow("non-4px-grid", id) && !matches!(n.kind, NodeKind::Text(_) | NodeKind::Vector { .. } | NodeKind::Line | NodeKind::Connector(_)) {
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

        let padding_inherited = PADDING_KEYS.iter().all(|k| number_inherited(&n, src, k));
        if let Some(fp) = n.kind.frame_props_mut() {
            if let Some(al) = &mut fp.auto_layout {
                if allow("unbalanced-padding", id)
                    && !padding_inherited
                    && !PADDING_KEYS.iter().any(|k| n.variables.bindings.contains_key(*k))
                    && ((al.padding[0] - al.padding[2]).abs() > 0.1 || (al.padding[1] - al.padding[3]).abs() > 0.1)
                {
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

        let orphaned_mirror = n.component.source.is_some() && src.is_none();
        if allow("hardcoded-value", id) && !orphaned_mirror && !token_index.by_rgb.is_empty() && !n.style_refs.contains_key("fill") {
            let slot = Slot::Fill;
            let paints = slot.paints(&n);
            let slot_inherited = src.is_some_and(|s| {
                slot.paints(s) == paints
                    && s.style_refs.get(slot.ref_key()) == n.style_refs.get(slot.ref_key())
                    && (0..paints.len()).all(|i| {
                        let key = format!("{}/{i}", slot.prefix());
                        s.variables.bindings.get(&key) == n.variables.bindings.get(&key)
                    })
            });
            let mut to_bind = Vec::new();
            if !slot_inherited {
                for (i, p) in paints.iter().enumerate() {
                    let PaintKind::Solid(c) = &p.kind else { continue };
                    let key = format!("{}/{i}", slot.prefix());
                    if !p.visible || n.variables.bindings.contains_key(&key) || doc.is_lint_ignored_property("hardcoded-value", id, Some(&key)) {
                        continue;
                    }
                    if src.is_some_and(|s| slot.paints(s).get(i) == Some(p) && s.variables.bindings.get(&key) == n.variables.bindings.get(&key)) {
                        continue;
                    }
                    if let Match::One(entry) = token_index.find(doc, &n, &key, c) {
                        if entry.constant {
                            to_bind.push((key, entry.fix_id.clone(), entry.token_name()));
                        }
                    }
                }
            }
            for (key, fix_id, token_name) in to_bind {
                n.variables.bindings.insert(key.clone(), fix_id.clone());
                if let Some(val) = doc.resolve_variable(id, &fix_id) {
                    set_bound_property(&mut n, &key, &val);
                }
                modified = true;
                changes.push(json!({
                    "rule": "hardcoded-value",
                    "node_id": id.0,
                    "action": format!("bound {key} to variable '{token_name}'"),
                }));
            }
        }

        if modified || doc.get(id).is_some_and(|orig| &n != orig) {
            if n.component.source.is_some() {
                if let Some(orig) = doc.get(id) {
                    let ov = crate::record_override(orig, &n);
                    n.component.overrides.extend(ov);
                }
            }
            updated_nodes.insert(id, n);
        }
    }

    let mut touched_ids: Vec<NodeId> = updated_nodes.keys().copied().chain(to_remove.iter().copied()).collect();
    let mut cmds = Vec::new();
    for (_, node) in updated_nodes {
        cmds.push(Command::Update { node: Box::new(node) });
    }
    for rem_id in to_remove {
        cmds.push(Command::Remove { id: rem_id });
    }

    if !cmds.is_empty() {
        let mut queue: Vec<NodeId> = touched_ids.iter().filter_map(|&id| crate::enclosing_component(doc, id)).collect();
        if !queue.is_empty() {
            let mut scratch = doc.clone();
            if Command::Batch(cmds.clone()).apply(&mut scratch).is_ok() {
                queue.extend(touched_ids.drain(..).filter_map(|id| crate::enclosing_component(&scratch, id)));
                queue.sort();
                queue.dedup();
                let mut rounds = 0;
                while let Some(owner) = queue.pop() {
                    rounds += 1;
                    if rounds > 16 {
                        break;
                    }
                    let sync_cmd = crate::sync_instances(&mut scratch, owner);
                    if matches!(&sync_cmd, Command::Batch(v) if v.is_empty()) {
                        continue;
                    }
                    let _ = sync_cmd.clone().apply(&mut scratch);
                    match sync_cmd {
                        Command::Batch(sub) => cmds.extend(sub),
                        other => cmds.push(other),
                    }
                }
            }
        }
    }

    let summary = json!({
        "fixed_count": changes.len(),
        "changes": changes,
        "warnings": warnings,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Node, Rect, Style, StyleMeta, StyleProperty, TextStyle};

    #[test]
    fn tiny_text_fix_detaches_linked_text_style() {
        let mut doc = Document::new("lint text style");
        let page = doc.add_page("Page");
        let id = doc.alloc_id();
        let mut text = Node::text(id, Rect::new(0.0, 0.0, 100.0, 20.0), "Tiny", TextStyle { font_size: 9.0, ..Default::default() });
        text.style_refs.insert("text".into(), "Body".into());
        doc.insert(page, text).unwrap();
        doc.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 9.0, ..Default::default() }));
        doc.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), property: StyleProperty::Text, ..Default::default() });

        let cfg = LintConfig::from_rule_list(&["tiny-text".into()]).unwrap();
        let (cmds, _) = lint_fix_commands(&doc, &[page], &cfg);
        Command::Batch(cmds).apply(&mut doc).unwrap();

        let fixed = doc.get(id).unwrap();
        assert_eq!(fixed.kind.any_text().unwrap().style.font_size, 12.0);
        assert!(!fixed.style_refs.contains_key("text"));
    }

    #[test]
    fn tiny_text_fix_keeps_valid_links_and_handles_unlinked_text() {
        for (size, linked) in [(12.0, true), (16.0, true), (9.0, false)] {
            let mut doc = Document::new("lint controls");
            let page = doc.add_page("Page");
            let id = doc.alloc_id();
            let mut text = Node::text(id, Rect::new(0.0, 0.0, 100.0, 20.0), "Copy", TextStyle { font_size: size, ..Default::default() });
            if linked {
                text.style_refs.insert("text".into(), "Body".into());
            }
            doc.insert(page, text).unwrap();
            let cfg = LintConfig::from_rule_list(&["tiny-text".into()]).unwrap();
            let (cmds, report) = lint_fix_commands(&doc, &[page], &cfg);
            Command::Batch(cmds).apply(&mut doc).unwrap();
            assert!(report["warnings"].as_array().unwrap().is_empty());
            let node = doc.get(id).unwrap();
            assert_eq!(node.style_refs.contains_key("text"), linked);
            assert_eq!(node.kind.any_text().unwrap().content, "Copy");
            assert_eq!(node.kind.any_text().unwrap().style.font_size, size.max(12.0));
        }
    }
}

#[cfg(test)]
mod connector_grid_fix_tests {
    use super::*;
    use crate::{ConnectorEnd, ConnectorProps, Document, Magnet, Node, Rect, Vec2};

    #[test]
    fn non_4px_grid_leaves_connector_geometry_unchanged() {
        let mut doc = Document::new("connector lint");
        let page = doc.add_page("Page");
        let start_id = doc.alloc_id();
        let start = doc.insert(page, Node::rectangle(start_id, Rect::new(0.0, 0.0, 41.0, 43.0))).unwrap();
        let end_id = doc.alloc_id();
        let end = doc.insert(page, Node::rectangle(end_id, Rect::new(160.0, 0.0, 40.0, 40.0))).unwrap();
        let connector = doc.alloc_id();
        let props =
            ConnectorProps { start: ConnectorEnd::attached(start, Magnet::Right), end: ConnectorEnd::attached(end, Magnet::Left), ..Default::default() };
        let connector = doc.insert(page, Node::connector(connector, props)).unwrap();
        doc.get_mut(connector).unwrap().size = Vec2::new(127.0, 3.0);
        let connector_before = doc.get(connector).unwrap().clone();

        let cfg = LintConfig::from_rule_list(&["non-4px-grid".into(), "small-touch-target".into()]).unwrap();
        let (cmds, report) = lint_fix_commands(&doc, &[page], &cfg);
        assert!(report["changes"].as_array().unwrap().iter().all(|change| change["node_id"] != connector.0));
        assert!(cmds.iter().all(|cmd| !matches!(cmd, Command::Update { node } if node.id == connector)));

        let mut history = History::default();
        lint_fix_with_history(&mut doc, &mut history, Some(page), Some(&["non-4px-grid".into(), "small-touch-target".into()])).unwrap();
        assert_eq!(doc.get(connector), Some(&connector_before));
        assert!(history.undo(&mut doc).unwrap());
        assert_eq!(doc.get(connector), Some(&connector_before));
    }

    /// A connector's name can read as a control ("Button link"); its box is still derived from its
    /// ends, so neither the finding nor the fix may treat it as a touch target or a grid item.
    fn named_connector_doc(name: &str) -> (Document, NodeId, NodeId) {
        let mut doc = Document::new("named connector");
        let page = doc.add_page("Page");
        let start_id = doc.alloc_id();
        let start = doc.insert(page, Node::rectangle(start_id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        let end_id = doc.alloc_id();
        let end = doc.insert(page, Node::rectangle(end_id, Rect::new(160.0, 0.0, 40.0, 40.0))).unwrap();
        let id = doc.alloc_id();
        let props =
            ConnectorProps { start: ConnectorEnd::attached(start, Magnet::Right), end: ConnectorEnd::attached(end, Magnet::Left), ..Default::default() };
        let connector = doc.insert(page, Node::connector(id, props)).unwrap();
        let node = doc.get_mut(connector).unwrap();
        node.name = name.into();
        node.size = Vec2::new(127.0, 3.0);
        (doc, page, connector)
    }

    #[test]
    fn a_connector_named_like_a_control_is_not_a_touch_target_finding() {
        let (doc, _, connector) = named_connector_doc("Button link");
        let cfg = LintConfig::from_rule_list(&["small-touch-target".into(), "non-4px-grid".into()]).unwrap();

        let report = crate::lint::lint(&doc, &cfg, &crate::lint::NoLintEnv);

        let on_connector: Vec<&str> = report.findings.iter().filter(|f| f.node == connector).map(|f| f.rule).collect();
        assert!(on_connector.is_empty(), "connector findings: {on_connector:?}");
    }

    #[test]
    fn a_connector_named_like_a_control_is_not_resized_by_the_touch_target_fix() {
        let (mut doc, page, connector) = named_connector_doc("Input to button");
        let before = doc.get(connector).unwrap().clone();
        let rules = ["small-touch-target".to_string()];

        let cfg = LintConfig::from_rule_list(&rules).unwrap();
        let (cmds, report) = lint_fix_commands(&doc, &[page], &cfg);
        assert!(report["changes"].as_array().unwrap().iter().all(|change| change["node_id"] != connector.0), "report: {report}");
        assert!(cmds.iter().all(|cmd| !matches!(cmd, Command::Update { node } if node.id == connector)));

        let mut history = History::default();
        lint_fix_with_history(&mut doc, &mut history, Some(page), Some(&rules)).unwrap();
        assert_eq!(doc.get(connector), Some(&before));
    }
}
