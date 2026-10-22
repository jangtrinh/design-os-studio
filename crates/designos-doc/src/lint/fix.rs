//! Deterministic auto-fixes for lint rules, routed through [`Command`] / [`History`].

use super::config::LintConfig;
use super::registry::{resolve_rule, LintPreset};
use super::rules::{is_default_layer_name, nearest_on_scale, SPACING_SCALE};
use super::token_match::{number_inherited, Index as TokenIndex, Match, Slot, PADDING_KEYS, RADIUS_KEYS};
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

// ============================================================================
// E15-13 (#598): Engine safe-tier auto-fix planner & diff report
// ============================================================================

use super::finding::{Finding, FixSuggestion};
use super::token_match::{bind_is_current, keys_to_bind, same_colour, style_is_current};
use crate::{AutoFixMode, AutoFixSettings, Color, Effect, Node, Sizing, VariableValue, SAFE_AUTO_FIX_RULES};
use serde::{Deserialize, Serialize};

/// Default undo label for the engine auto-fix history entry (`E15-13` §3.3, §5).
pub const AUTO_FIX_UNDO_LABEL: &str = "Auto-fix";

/// Maximum number of auto-fix passes per mutating call (`E15-13` §3.3).
pub const MAX_AUTO_FIX_PASSES: usize = 2;

/// One planned safe-tier fix (`E15-13` §4, §5).
#[derive(Clone, Debug)]
pub struct PlannedFix {
    pub node: NodeId,
    pub node_name: String,
    pub property: String,
    pub before: Value,
    pub after: Value,
    pub rule: String,
    pub commands: Vec<Command>,
}

/// Entry in `auto_fix.applied` or `auto_fix.would_apply` (`E15-13` §5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoFixDiffEntry {
    pub node_id: u64,
    pub node_name: String,
    pub property: String,
    pub before: Value,
    pub after: Value,
    pub rule: String,
}

impl From<&PlannedFix> for AutoFixDiffEntry {
    fn from(p: &PlannedFix) -> Self {
        Self {
            node_id: p.node.0,
            node_name: p.node_name.clone(),
            property: p.property.clone(),
            before: p.before.clone(),
            after: p.after.clone(),
            rule: p.rule.clone(),
        }
    }
}

/// Entry in `auto_fix.suggested` (`E15-13` §5, §8.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoFixSuggestedEntry {
    pub node_id: u64,
    pub node_name: String,
    pub property: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested: Option<Value>,
    pub rule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixSuggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_by: Option<String>,
}

/// The `auto_fix` report block attached to mutating MCP replies (`E15-13` §5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoFixReport {
    pub mode: AutoFixMode,
    pub undo_label: String,
    pub passes: usize,
    pub applied: Vec<AutoFixDiffEntry>,
    pub would_apply: Vec<AutoFixDiffEntry>,
    pub suggested: Vec<AutoFixSuggestedEntry>,
    pub error: Option<String>,
}

impl Default for AutoFixReport {
    fn default() -> Self {
        Self {
            mode: AutoFixMode::On,
            undo_label: AUTO_FIX_UNDO_LABEL.to_string(),
            passes: 0,
            applied: Vec::new(),
            would_apply: Vec::new(),
            suggested: Vec::new(),
            error: None,
        }
    }
}

/// Resolved 3-level auto-fix configuration (session > file > user > default) (`E15-13` §3.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedAutoFixConfig {
    pub mode: AutoFixMode,
    /// Map of safe-tier rule ID -> `None` when enabled, or `Some("session" | "file" | "user")` when disabled.
    pub disabled_by: BTreeMap<String, Option<&'static str>>,
}

impl Default for ResolvedAutoFixConfig {
    fn default() -> Self {
        Self::resolve(None, None, None)
    }
}

impl ResolvedAutoFixConfig {
    /// Resolves effective mode and per-rule enabled status using precedence:
    /// session (`designos_configure_session`) > file (`Document.auto_fix`) > user (`settings.json`).
    pub fn resolve(session: Option<&AutoFixSettings>, file: Option<&AutoFixSettings>, user: Option<&AutoFixSettings>) -> Self {
        let mode = session.and_then(|s| s.mode).or_else(|| file.and_then(|f| f.mode)).or_else(|| user.and_then(|u| u.mode)).unwrap_or(AutoFixMode::On);

        let mut disabled_by = BTreeMap::new();
        for &rule in SAFE_AUTO_FIX_RULES {
            let status = if let Some(en) = session.and_then(|s| s.rules.get(rule).copied()) {
                if en {
                    None
                } else {
                    Some("session")
                }
            } else if let Some(en) = file.and_then(|f| f.rules.get(rule).copied()) {
                if en {
                    None
                } else {
                    Some("file")
                }
            } else if let Some(en) = user.and_then(|u| u.rules.get(rule).copied()) {
                if en {
                    None
                } else {
                    Some("user")
                }
            } else {
                None
            };
            disabled_by.insert(rule.to_string(), status);
        }

        Self { mode, disabled_by }
    }

    pub fn rule_disabled_by(&self, rule_id: &str) -> Option<&'static str> {
        self.disabled_by.get(rule_id).copied().flatten()
    }
}

fn format_fix_target_value(v: &Value) -> Value {
    if let Some(s) = v.as_str() {
        if let Some(stripped) = s.strip_prefix('$') {
            if stripped.starts_with("var:") || stripped.starts_with("style:") {
                return json!(stripped);
            }
        }
    }
    v.clone()
}

fn resolved_ancestor_bg_color(doc: &Document, node_id: NodeId) -> Color {
    let mut cur = doc.get(node_id).and_then(|n| n.parent);
    while let Some(pid) = cur {
        let Some(p) = doc.get(pid) else { break };
        if let NodeKind::Page { background } = p.kind {
            return background;
        }
        if p.visible && p.opacity > 0.0 {
            for fill in p.fills.iter().rev() {
                if fill.visible && fill.opacity > 0.0 {
                    if let PaintKind::Solid(c) = fill.kind {
                        if c.a > 0.0 {
                            return c;
                        }
                    }
                }
            }
        }
        cur = p.parent;
    }
    Color::WHITE
}

fn srgb_channel_lum(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(c: Color) -> f32 {
    0.2126 * srgb_channel_lum(c.r) + 0.7152 * srgb_channel_lum(c.g) + 0.0722 * srgb_channel_lum(c.b)
}

/// Returns `"#111111"` on light backgrounds or `"#FFFFFF"` on dark backgrounds for `text-missing-fill` suggestions (`E15-13` §3.1).
pub fn contrast_aware_text_fill_hex(doc: &Document, node_id: NodeId) -> &'static str {
    let bg = resolved_ancestor_bg_color(doc, node_id);
    let bg_lum = relative_luminance(bg);
    // Contrast against #111111 (lum ~0.0056) vs #FFFFFF (lum = 1.0).
    let dark_lum = relative_luminance(Color::rgb(0x11, 0x11, 0x11));
    let ratio_dark = (bg_lum.max(dark_lum) + 0.05) / (bg_lum.min(dark_lum) + 0.05);
    let ratio_white = (1.0 + 0.05) / (bg_lum + 0.05);
    if ratio_white > ratio_dark {
        "#FFFFFF"
    } else {
        "#111111"
    }
}

fn is_property_already_bound(node: &Node, prop: &str) -> bool {
    if node.variables.bindings.contains_key(prop) {
        return true;
    }
    if prop.starts_with("fills/") && node.style_refs.contains_key("fill") {
        return true;
    }
    if prop.starts_with("strokes/") && node.style_refs.contains_key("stroke") {
        return true;
    }
    if matches!(prop, "fontSize" | "lineHeight" | "letterSpacing" | "paragraphSpacing") && node.style_refs.contains_key("text") {
        return true;
    }
    match prop {
        "padding" => PADDING_KEYS.iter().any(|k| node.variables.bindings.contains_key(*k)),
        "cornerRadius" => RADIUS_KEYS.iter().any(|k| node.variables.bindings.contains_key(*k)),
        _ => false,
    }
}

fn find_safe_variable_id_for_binding(
    doc: &Document,
    token_index: &TokenIndex,
    node: &Node,
    prop: &str,
    expected_from: &Value,
    token_name: &str,
) -> Option<(String, Vec<String>)> {
    if is_property_already_bound(node, prop) {
        return None;
    }

    // 1. Check standard property bindings via `keys_to_bind` / `bind_is_current`.
    for v in doc.variables.values() {
        if v.collection == crate::EASEUI_DEPRECATED_COLLECTION_ID {
            continue;
        }
        let coll_name = doc.variable_collections.get(&v.collection).map(|c| c.name.as_str()).unwrap_or("");
        let entry_token_name = if v.name.contains('/') {
            v.name.clone()
        } else if !coll_name.is_empty() && !matches!(coll_name, "Default" | "Tokens" | "Variables" | "Collection" | "Theme" | "Modes") {
            format!("{coll_name}/{}", v.name)
        } else {
            v.name.clone()
        };
        if entry_token_name != token_name && v.name != token_name && v.id != token_name {
            continue;
        }
        // Check if variable is constant across all modes in its collection.
        let is_constant = doc
            .variable_collections
            .get(&v.collection)
            .map(|coll| {
                if coll.modes.len() <= 1 {
                    return true;
                }
                let mut first_val: Option<VariableValue> = None;
                for m in &coll.modes {
                    let resolved = v.values.get(&m.id).cloned();
                    match (&first_val, resolved) {
                        (None, Some(rv)) => first_val = Some(rv),
                        (Some(a), Some(b)) if a == &b => {}
                        _ => return false,
                    }
                }
                true
            })
            .unwrap_or(true);
        if !is_constant {
            continue;
        }

        // Standard properties supported by `keys_to_bind`
        let bind_keys = keys_to_bind(doc, node.id, prop, &v.id);
        if !bind_keys.is_empty() && bind_is_current(doc, node.id, prop, &v.id) {
            return Some((v.id.clone(), bind_keys.into_iter().map(str::to_string).collect()));
        }

        // Special properties: `runs/{run_idx}/fills/{i}` and `effects/{i}/color`
        if let Some(rest) = prop.strip_prefix("runs/") {
            if let Some((run_str, fill_rest)) = rest.split_once("/fills/") {
                if let (Ok(run_idx), Ok(fill_idx), NodeKind::Text(tp)) = (run_str.parse::<usize>(), fill_rest.parse::<usize>(), &node.kind) {
                    if let Some(p) = tp.runs.get(run_idx).and_then(|r| r.style.fills.get(fill_idx)) {
                        if let PaintKind::Solid(c) = &p.kind {
                            let hex = format!("#{}", c.to_hex());
                            if p.visible
                                && expected_from.as_str() == Some(hex.as_str())
                                && matches!(doc.resolve_variable(node.id, &v.id), Some(VariableValue::Color(vc)) if same_colour(&vc, c))
                            {
                                return Some((v.id.clone(), vec![prop.to_string()]));
                            }
                        }
                    }
                }
            }
        } else if let Some(rest) = prop.strip_prefix("effects/") {
            if let Some(idx_str) = rest.strip_suffix("/color") {
                if let Ok(eff_idx) = idx_str.parse::<usize>() {
                    if let Some(Effect::DropShadow { color: c, visible: true, .. } | Effect::InnerShadow { color: c, visible: true, .. }) =
                        node.effects.get(eff_idx)
                    {
                        let hex = format!("#{}", c.to_hex());
                        if expected_from.as_str() == Some(hex.as_str())
                            && matches!(doc.resolve_variable(node.id, &v.id), Some(VariableValue::Color(vc)) if same_colour(&vc, c))
                        {
                            return Some((v.id.clone(), vec![prop.to_string()]));
                        }
                    }
                }
            }
        }
    }

    let _ = token_index;
    None
}

/// Plans safe-tier fixes and suggest-tier entries from `findings` (`E15-13` §3, §5).
///
/// Safe-tier fixes are ordered by rule priority (`sizing-conflict` -> `hardcoded-value`) and
/// re-validate the live node state (existence, unlocked, non-instance-mirror, within owned frames,
/// unbound property, and unchanged value).
pub fn plan_fixes(
    doc: &Document,
    findings: &[Finding],
    config: &ResolvedAutoFixConfig,
    owned_frames: Option<&[NodeId]>,
) -> (Vec<PlannedFix>, Vec<AutoFixSuggestedEntry>) {
    let token_index = TokenIndex::new(doc);
    let mut working_nodes: BTreeMap<NodeId, Node> = BTreeMap::new();
    let mut planned: Vec<PlannedFix> = Vec::new();
    let mut suggested: Vec<AutoFixSuggestedEntry> = Vec::new();

    // Separate safe-tier candidates (ordered `sizing-conflict` -> `hardcoded-value`) from suggest-tier findings.
    let mut safe_candidates: Vec<&Finding> = Vec::new();
    let mut suggest_candidates: Vec<&Finding> = Vec::new();

    for f in findings {
        if f.ignored {
            continue;
        }
        if f.safe && (f.rule == "sizing-conflict" || f.rule == "hardcoded-value" || f.rule.starts_with("test-")) {
            safe_candidates.push(f);
        } else {
            suggest_candidates.push(f);
        }
    }

    safe_candidates.sort_by_key(|f| match f.rule {
        "sizing-conflict" => 0u8,
        "hardcoded-value" => 1u8,
        _ => 2u8,
    });

    for f in safe_candidates {
        let Some(orig_node) = doc.get(f.node) else {
            continue;
        };
        // Common safety conditions (E15-13 §3.2):
        // 1. Not locked
        // 2. Not an instance mirror (`component.source.is_some()`)
        // 3. Inside owned frames (if leased)
        if orig_node.locked || orig_node.component.source.is_some() {
            continue;
        }
        if let Some(frames) = owned_frames {
            if !frames.iter().any(|&fid| fid == f.node || doc.is_ancestor(fid, f.node)) {
                continue;
            }
        }

        let cur_node = working_nodes.get(&f.node).cloned().unwrap_or_else(|| orig_node.clone());

        match f.rule {
            "sizing-conflict" => {
                let Some(pid) = cur_node.parent else { continue };
                let Some(parent) = doc.get(pid) else { continue };
                if matches!(parent.kind, NodeKind::Page { .. }) {
                    continue;
                }
                let parent_al = parent.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref());
                match parent_al {
                    None => {
                        let mut node_after = cur_node.clone();
                        for (prop, axis_sizing, is_horiz) in [
                            ("layoutSizingHorizontal", cur_node.layout_child.horizontal, true),
                            ("layoutSizingVertical", cur_node.layout_child.vertical, false),
                        ] {
                            if matches!(axis_sizing, Sizing::Fill | Sizing::Hug) {
                                let before_val = json!(format!("{:?}", axis_sizing).to_ascii_uppercase());
                                let after_val = json!("FIXED");
                                if let Some(disabled_by) = config.rule_disabled_by("sizing-conflict") {
                                    suggested.push(AutoFixSuggestedEntry {
                                        node_id: f.node.0,
                                        node_name: f.node_name.clone(),
                                        property: prop.to_string(),
                                        current: Some(before_val.clone()),
                                        suggested: Some(after_val.clone()),
                                        rule: "sizing-conflict".into(),
                                        fix: Some(FixSuggestion { kind: "set".into(), property: prop.to_string(), from: before_val, to: after_val }),
                                        reason: None,
                                        disabled_by: Some(disabled_by.to_string()),
                                    });
                                } else {
                                    if is_horiz {
                                        node_after.layout_child.horizontal = Sizing::Fixed;
                                    } else {
                                        node_after.layout_child.vertical = Sizing::Fixed;
                                    }
                                    working_nodes.insert(f.node, node_after.clone());
                                    planned.push(PlannedFix {
                                        node: f.node,
                                        node_name: f.node_name.clone(),
                                        property: prop.to_string(),
                                        before: before_val,
                                        after: after_val,
                                        rule: "sizing-conflict".into(),
                                        commands: vec![Command::Update { node: Box::new(node_after.clone()) }],
                                    });
                                }
                            }
                        }
                    }
                    Some(al) if !cur_node.layout_child.absolute => {
                        let is_horiz = al.primary_is_horizontal();
                        let child_primary = if is_horiz { cur_node.layout_child.horizontal } else { cur_node.layout_child.vertical };
                        if al.primary_sizing == Sizing::Hug && child_primary == Sizing::Fill {
                            let prop = if is_horiz { "layoutSizingHorizontal" } else { "layoutSizingVertical" };
                            let mut node_after = cur_node.clone();
                            if is_horiz {
                                node_after.layout_child.horizontal = Sizing::Fixed;
                            } else {
                                node_after.layout_child.vertical = Sizing::Fixed;
                            }
                            // Verify resolved size is unchanged after relayout (E15-13 §3.1).
                            let mut scratch = doc.clone();
                            if let Some(sn) = scratch.get_mut(f.node) {
                                *sn = node_after.clone();
                            }
                            scratch.relayout_subtree(pid);
                            let size_unchanged = scratch
                                .get(f.node)
                                .is_some_and(|sn| (sn.size.x - cur_node.size.x).abs() <= 0.05 && (sn.size.y - cur_node.size.y).abs() <= 0.05);
                            if !size_unchanged {
                                continue;
                            }

                            if let Some(disabled_by) = config.rule_disabled_by("sizing-conflict") {
                                suggested.push(AutoFixSuggestedEntry {
                                    node_id: f.node.0,
                                    node_name: f.node_name.clone(),
                                    property: prop.to_string(),
                                    current: Some(json!("FILL")),
                                    suggested: Some(json!("FIXED")),
                                    rule: "sizing-conflict".into(),
                                    fix: Some(FixSuggestion { kind: "set".into(), property: prop.to_string(), from: json!("FILL"), to: json!("FIXED") }),
                                    reason: None,
                                    disabled_by: Some(disabled_by.to_string()),
                                });
                            } else {
                                working_nodes.insert(f.node, node_after.clone());
                                planned.push(PlannedFix {
                                    node: f.node,
                                    node_name: f.node_name.clone(),
                                    property: prop.to_string(),
                                    before: json!("FILL"),
                                    after: json!("FIXED"),
                                    rule: "sizing-conflict".into(),
                                    commands: vec![Command::Update { node: Box::new(node_after) }],
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            "hardcoded-value" => {
                let Some(sugg) = &f.suggested_fix else { continue };
                let prop = &sugg.property;
                let to_str = sugg.to.as_str().unwrap_or("");
                let after_val = format_fix_target_value(&sugg.to);

                if let Some(token_name) = to_str.strip_prefix("var:").or_else(|| to_str.strip_prefix("$var:")) {
                    let Some((var_id, bind_keys)) = find_safe_variable_id_for_binding(doc, &token_index, &cur_node, prop, &sugg.from, token_name) else {
                        continue;
                    };
                    if let Some(disabled_by) = config.rule_disabled_by("hardcoded-value") {
                        suggested.push(AutoFixSuggestedEntry {
                            node_id: f.node.0,
                            node_name: f.node_name.clone(),
                            property: prop.clone(),
                            current: Some(sugg.from.clone()),
                            suggested: Some(after_val.clone()),
                            rule: "hardcoded-value".into(),
                            fix: Some(FixSuggestion { kind: sugg.kind.clone(), property: prop.clone(), from: sugg.from.clone(), to: after_val }),
                            reason: None,
                            disabled_by: Some(disabled_by.to_string()),
                        });
                    } else {
                        let mut node_after = cur_node.clone();
                        for k in bind_keys {
                            node_after.variables.bindings.insert(k, var_id.clone());
                        }
                        working_nodes.insert(f.node, node_after.clone());
                        planned.push(PlannedFix {
                            node: f.node,
                            node_name: f.node_name.clone(),
                            property: prop.clone(),
                            before: sugg.from.clone(),
                            after: after_val,
                            rule: "hardcoded-value".into(),
                            commands: vec![Command::Update { node: Box::new(node_after) }],
                        });
                    }
                } else if let Some(style_name) = to_str.strip_prefix("style:").or_else(|| to_str.strip_prefix("$style:")) {
                    if cur_node.style_refs.contains_key(prop) {
                        continue;
                    }
                    let Some(style_entry) = token_index.styles.iter().find(|s| {
                        (s.name == style_name || s.fix_key == style_name)
                            && style_is_current(doc, cur_node.id, &s.fix_key)
                            && super::token_match::style_matches_node(doc, &cur_node, &s.style, s.property)
                    }) else {
                        continue;
                    };
                    if let Some(disabled_by) = config.rule_disabled_by("hardcoded-value") {
                        suggested.push(AutoFixSuggestedEntry {
                            node_id: f.node.0,
                            node_name: f.node_name.clone(),
                            property: prop.clone(),
                            current: Some(sugg.from.clone()),
                            suggested: Some(after_val.clone()),
                            rule: "hardcoded-value".into(),
                            fix: Some(FixSuggestion { kind: sugg.kind.clone(), property: prop.clone(), from: sugg.from.clone(), to: after_val }),
                            reason: None,
                            disabled_by: Some(disabled_by.to_string()),
                        });
                    } else {
                        let mut node_after = cur_node.clone();
                        node_after.style_refs.insert(prop.clone(), style_entry.fix_key.clone());
                        working_nodes.insert(f.node, node_after.clone());
                        planned.push(PlannedFix {
                            node: f.node,
                            node_name: f.node_name.clone(),
                            property: prop.clone(),
                            before: sugg.from.clone(),
                            after: after_val,
                            rule: "hardcoded-value".into(),
                            commands: vec![Command::Update { node: Box::new(node_after) }],
                        });
                    }
                }
            }
            other if other.starts_with("test-") => {
                // Injected test rule used by `two_pass_limit` (E15-13 §10).
                let Some(sugg) = &f.suggested_fix else { continue };
                let mut node_after = cur_node.clone();
                if sugg.property == "name" {
                    if let Some(new_name) = sugg.to.as_str() {
                        if node_after.name == new_name {
                            continue;
                        }
                        node_after.name = new_name.to_string();
                    }
                }
                working_nodes.insert(f.node, node_after.clone());
                planned.push(PlannedFix {
                    node: f.node,
                    node_name: f.node_name.clone(),
                    property: sugg.property.clone(),
                    before: sugg.from.clone(),
                    after: sugg.to.clone(),
                    rule: other.to_string(),
                    commands: vec![Command::Update { node: Box::new(node_after) }],
                });
            }
            _ => {}
        }
    }

    // Populate suggest-tier findings (`text-missing-fill`, `absolute-in-auto-layout`, `off-scale`, `contrast`, non-safe `hardcoded-value`, etc.).
    for f in suggest_candidates {
        match f.rule {
            "empty-container" => {
                // Never fixed or suggested for deletion by the engine (E15-13 §3.1).
            }
            "text-missing-fill" => {
                let suggested_hex = contrast_aware_text_fill_hex(doc, f.node);
                suggested.push(AutoFixSuggestedEntry {
                    node_id: f.node.0,
                    node_name: f.node_name.clone(),
                    property: "fills".into(),
                    current: Some(json!([])),
                    suggested: Some(json!(suggested_hex)),
                    rule: "text-missing-fill".into(),
                    fix: Some(FixSuggestion { kind: "set".into(), property: "fills".into(), from: json!([]), to: json!(suggested_hex) }),
                    reason: Some(f.message.clone()),
                    disabled_by: None,
                });
            }
            "absolute-in-auto-layout" => {
                suggested.push(AutoFixSuggestedEntry {
                    node_id: f.node.0,
                    node_name: f.node_name.clone(),
                    property: "layout_child.absolute".into(),
                    current: Some(json!(true)),
                    suggested: Some(json!(false)),
                    rule: "absolute-in-auto-layout".into(),
                    fix: Some(FixSuggestion { kind: "set".into(), property: "layout_child.absolute".into(), from: json!(true), to: json!(false) }),
                    reason: Some(f.message.clone()),
                    disabled_by: None,
                });
            }
            "off-scale" => {
                let prop = f.property.clone().or_else(|| f.suggested_fix.as_ref().map(|s| s.property.clone())).unwrap_or_default();
                let current = f.value.clone().or_else(|| f.suggested_fix.as_ref().map(|s| s.from.clone()));
                let sugg_val = f.suggested_fix.as_ref().map(|s| s.to.clone()).or_else(|| f.nearest.clone());
                let fix = f.suggested_fix.clone().or_else(|| {
                    Some(FixSuggestion {
                        kind: "set".into(),
                        property: prop.clone(),
                        from: current.clone().unwrap_or(Value::Null),
                        to: sugg_val.clone().unwrap_or(Value::Null),
                    })
                });
                suggested.push(AutoFixSuggestedEntry {
                    node_id: f.node.0,
                    node_name: f.node_name.clone(),
                    property: prop,
                    current,
                    suggested: sugg_val,
                    rule: "off-scale".into(),
                    fix,
                    reason: Some(f.message.clone()),
                    disabled_by: None,
                });
            }
            "contrast" => {
                let prop = f.suggested_fix.as_ref().map(|s| s.property.clone()).unwrap_or_else(|| "fills/0".into());
                let current = f.foreground_hex.as_ref().map(|h| json!(h)).or_else(|| f.suggested_fix.as_ref().map(|s| s.from.clone()));
                let sugg_val = f.suggested_fix.as_ref().map(|s| s.to.clone());
                suggested.push(AutoFixSuggestedEntry {
                    node_id: f.node.0,
                    node_name: f.node_name.clone(),
                    property: prop,
                    current,
                    suggested: sugg_val,
                    rule: "contrast".into(),
                    fix: f.suggested_fix.clone(),
                    reason: Some(f.message.clone()),
                    disabled_by: None,
                });
            }
            "hardcoded-value" => {
                let prop = f.property.clone().or_else(|| f.suggested_fix.as_ref().map(|s| s.property.clone())).unwrap_or_default();
                let current = f.value.clone().or_else(|| f.suggested_fix.as_ref().map(|s| s.from.clone()));
                let sugg_val = f.suggested_fix.as_ref().map(|s| format_fix_target_value(&s.to)).or_else(|| f.candidates.as_ref().map(|c| json!(c)));
                suggested.push(AutoFixSuggestedEntry {
                    node_id: f.node.0,
                    node_name: f.node_name.clone(),
                    property: prop,
                    current,
                    suggested: sugg_val,
                    rule: "hardcoded-value".into(),
                    fix: f.suggested_fix.clone(),
                    reason: Some(f.message.clone()),
                    disabled_by: None,
                });
            }
            _ => {
                if let Some(s) = &f.suggested_fix {
                    suggested.push(AutoFixSuggestedEntry {
                        node_id: f.node.0,
                        node_name: f.node_name.clone(),
                        property: s.property.clone(),
                        current: Some(s.from.clone()),
                        suggested: Some(s.to.clone()),
                        rule: f.rule.to_string(),
                        fix: Some(s.clone()),
                        reason: Some(f.message.clone()),
                        disabled_by: None,
                    });
                }
            }
        }
    }

    (planned, suggested)
}
