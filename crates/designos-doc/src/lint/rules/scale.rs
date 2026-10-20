//! Scale resolution (`scales_for`) and token/scale lint rules (`off-scale`, `hardcoded-value`)
//! for Epic E15-11 (#596).

use super::super::config::LintConfig;
use super::super::finding::{Finding, FixSuggestion, ScaleSource};
use super::super::registry::{Category, Severity};
use super::super::token_match::{
    enabled_library_name, fmt_num, fmt_style_quoted, num_key, number_inherited, number_value_at, same_number, style_library_id, text_style_inherited,
    variable_library_id, Entry, Index, Match, Slot, PADDING_KEYS, RADIUS_KEYS,
};
use crate::{
    scope_allows, Document, Effect, FrameProps, LayoutAlign, Node, NodeKind, PaintKind, Rect, Style, TextProps, Variable, VariableKind, VariableStore,
    VariableValue,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::OnceCell;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

/// XPath of the node under evaluation, computed on first use: the path walk is `O(siblings x depth)`,
/// so it must not run for nodes that produce no finding.
pub struct LazyXPath<'a> {
    doc: &'a Document,
    id: crate::NodeId,
    cell: OnceCell<String>,
}

impl<'a> LazyXPath<'a> {
    pub fn new(doc: &'a Document, id: crate::NodeId) -> Self {
        Self { doc, id, cell: OnceCell::new() }
    }

    pub fn get(&self) -> &str {
        self.cell.get_or_init(|| crate::openpencil::node_to_xpath(self.doc, self.id))
    }
}

/// Default spacing scale steps (px).
pub const SPACING_SCALE: [f32; 15] = [0.0, 2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0, 40.0, 48.0, 64.0, 80.0, 96.0];
/// Default corner radius scale steps (px).
pub const RADIUS_SCALE: [f32; 12] = [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0, 32.0, 9999.0];
/// Default typographic font-size scale steps (px).
pub const FONT_SCALE: [f32; 22] =
    [10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0, 24.0, 28.0, 30.0, 32.0, 36.0, 40.0, 48.0, 56.0, 60.0, 64.0, 72.0, 80.0, 96.0];

/// Floating-point tolerance for scale membership (`0.05` px per E15-11 §3.2).
pub const SCALE_TOLERANCE: f32 = 0.05;

/// Optional archetype scales (from E15-4 / E15-18) that take highest priority when resolving scales.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeScales {
    #[serde(default)]
    pub spacing: Vec<f32>,
    #[serde(default)]
    pub radius: Vec<f32>,
    #[serde(default)]
    pub typography: Vec<f32>,
}

/// Maps `num_key(step)` to `Some((token_name, var_ref, var_id))` when the scale came from
/// document variables and a single variable holds that step value; `None` when multiple variables
/// share the step value.
pub type UniqueVarByStep = HashMap<i64, Option<(String, String, String)>>;

/// A single resolved scale (spacing, radius, or typography) for a lint run.
#[derive(Clone, Debug, PartialEq)]
pub struct Scale {
    /// Sorted distinct steps in pixels.
    pub steps: Vec<f32>,
    /// Which source defined this scale (`archetype`, `variables`, `styles`, or `default`).
    pub source: ScaleSource,
    /// Optional reason why a higher-priority source was skipped (e.g. `"fewer than 3 variables"`).
    pub fallback_reason: Option<String>,
    /// Maps `num_key(step)` to `Some((token_name, var_ref, var_id))` when the scale came from
    /// document variables and a single variable holds that step value; `None` when multiple variables
    /// share the step value.
    pub unique_var_by_step: UniqueVarByStep,
}

/// The three resolved scales for a lint run.
#[derive(Clone, Debug, PartialEq)]
pub struct Scales {
    pub spacing: Scale,
    pub radius: Scale,
    pub typography: Scale,
}

/// Outcome of checking a raw numeric value against a [`Scale`].
#[derive(Clone, Debug, PartialEq)]
pub enum ScaleMatch {
    OnScale,
    Nearest { step: f32, unique_var: Option<(String, String, String)> },
    Equidistant { low: f32, high: f32 },
}

impl Scale {
    /// Checks whether `val` is on this scale within [`SCALE_TOLERANCE`] (`0.05` px).
    /// Zero is always on scale; for `is_radius`, `>= 999.0` ("pill") is always on scale.
    pub fn check(&self, val: f32, is_radius: bool) -> ScaleMatch {
        if !val.is_finite() || val.abs() <= SCALE_TOLERANCE {
            return ScaleMatch::OnScale;
        }
        if is_radius && val >= 999.0 {
            return ScaleMatch::OnScale;
        }
        for &s in &self.steps {
            if (val - s).abs() <= SCALE_TOLERANCE {
                return ScaleMatch::OnScale;
            }
        }
        if self.steps.is_empty() {
            return ScaleMatch::OnScale;
        }
        let min_dist = self.steps.iter().map(|&s| (val - s).abs()).fold(f32::INFINITY, f32::min);
        let mut nearest: Vec<f32> = self.steps.iter().copied().filter(|&s| ((val - s).abs() - min_dist).abs() <= 1e-4).collect();
        nearest.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        nearest.dedup_by(|a, b| (*a - *b).abs() <= SCALE_TOLERANCE);
        match nearest.as_slice() {
            [] => ScaleMatch::OnScale,
            [step] => {
                let unique_var = self.unique_var_by_step.get(&num_key(*step)).and_then(|v| v.clone());
                ScaleMatch::Nearest { step: *step, unique_var }
            }
            [low, high, ..] => ScaleMatch::Equidistant { low: *low, high: *high },
        }
    }
}

fn dedup_sorted_positive(values: impl IntoIterator<Item = f32>) -> Vec<f32> {
    let mut pos: Vec<f32> = values.into_iter().filter(|v| v.is_finite() && *v > SCALE_TOLERANCE).collect();
    pos.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    pos.dedup_by(|a, b| (*a - *b).abs() <= SCALE_TOLERANCE);
    pos
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScaleKind {
    Spacing,
    Radius,
    Typography,
}

fn var_matches_scale_kind(v: &Variable, kind: ScaleKind) -> bool {
    if v.kind != VariableKind::Number {
        return false;
    }
    match kind {
        ScaleKind::Spacing => scope_allows(&v.scopes, "itemSpacing", &NodeKind::Frame(FrameProps::default())),
        ScaleKind::Radius => scope_allows(&v.scopes, "topLeftRadius", &NodeKind::Rectangle),
        ScaleKind::Typography => scope_allows(&v.scopes, "fontSize", &NodeKind::Text(TextProps::default())),
    }
}

fn collect_variable_steps(doc: &Document, store: &VariableStore, kind: ScaleKind) -> (Vec<f32>, UniqueVarByStep, usize) {
    let mut raw_steps = Vec::new();
    let mut by_step: HashMap<i64, Vec<(String, String, String)>> = HashMap::new();
    let mut var_count = 0usize;

    for v in doc.variables.values() {
        if let Some(lib_id) = variable_library_id(doc, v) {
            if enabled_library_name(doc, lib_id).is_none() {
                continue;
            }
        }
        let coll = doc.variable_collections.get(&v.collection);
        let coll_name = coll.map(|c| c.name.as_str()).unwrap_or("");
        if !var_matches_scale_kind(v, kind) {
            continue;
        }
        var_count += 1;

        let entry = Entry {
            var: Arc::new(v.clone()),
            collection_name: coll_name.to_string(),
            fix_id: v.id.clone(),
            library_name: None,
            constant: true,
            source: super::super::token_match::VarSource::Local,
        };
        let token_name = entry.token_name();
        let var_ref = entry.var_ref();

        let mode_values: Vec<f32> = if let Some(c) = coll.filter(|c| !c.modes.is_empty()) {
            c.modes
                .iter()
                .filter_map(|m| match store.resolve(&v.id, &m.id) {
                    Some(VariableValue::Number(x)) if x.is_finite() && x > SCALE_TOLERANCE => Some(x),
                    _ => None,
                })
                .collect()
        } else {
            v.values
                .values()
                .filter_map(|val| match val {
                    VariableValue::Number(x) if x.is_finite() && *x > SCALE_TOLERANCE => Some(*x),
                    _ => None,
                })
                .collect()
        };

        for x in mode_values {
            raw_steps.push(x);
            let k = num_key(x);
            let list = by_step.entry(k).or_default();
            if !list.iter().any(|(_, _, id)| id == &v.id) {
                list.push((token_name.clone(), var_ref.clone(), v.id.clone()));
            }
        }
    }

    let steps = dedup_sorted_positive(raw_steps);
    let mut unique_var_by_step = HashMap::new();
    for &s in &steps {
        let k = num_key(s);
        if let Some(list) = by_step.get(&k) {
            if list.len() == 1 {
                unique_var_by_step.insert(k, Some(list[0].clone()));
            } else {
                unique_var_by_step.insert(k, None);
            }
        }
    }
    (steps, unique_var_by_step, var_count)
}

/// Resolves the spacing, corner-radius, and typographic scales for `doc` (with optional `archetype` override).
///
/// Each scale is resolved in priority order (first source with `>= 3` distinct positive steps wins):
/// 1. `archetype` (`ScaleSource::Archetype`)
/// 2. Document variables / text styles (`ScaleSource::Variables` or `ScaleSource::Styles`)
/// 3. Built-in default (`ScaleSource::Default`)
pub fn scales_for(doc: &Document, archetype: Option<&ArchetypeScales>) -> Scales {
    let store = VariableStore::from_doc(doc);

    // 1. Spacing scale
    let spacing = if let Some(steps) = archetype.map(|a| dedup_sorted_positive(a.spacing.iter().copied())).filter(|s| s.len() >= 3) {
        Scale { steps, source: ScaleSource::Archetype, fallback_reason: None, unique_var_by_step: HashMap::new() }
    } else {
        let (var_steps, unique_var_by_step, var_count) = collect_variable_steps(doc, &store, ScaleKind::Spacing);
        if var_steps.len() >= 3 {
            Scale { steps: var_steps, source: ScaleSource::Variables, fallback_reason: None, unique_var_by_step }
        } else {
            let reason = (var_count > 0).then(|| format!("only {} distinct positive GAP variable step(s); minimum is 3", var_steps.len()));
            Scale { steps: SPACING_SCALE.to_vec(), source: ScaleSource::Default, fallback_reason: reason, unique_var_by_step: HashMap::new() }
        }
    };

    // 2. Radius scale
    let radius = if let Some(steps) = archetype.map(|a| dedup_sorted_positive(a.radius.iter().copied())).filter(|s| s.len() >= 3) {
        Scale { steps, source: ScaleSource::Archetype, fallback_reason: None, unique_var_by_step: HashMap::new() }
    } else {
        let (var_steps, unique_var_by_step, var_count) = collect_variable_steps(doc, &store, ScaleKind::Radius);
        if var_steps.len() >= 3 {
            Scale { steps: var_steps, source: ScaleSource::Variables, fallback_reason: None, unique_var_by_step }
        } else {
            let reason = (var_count > 0).then(|| format!("only {} distinct positive CORNER_RADIUS variable step(s); minimum is 3", var_steps.len()));
            Scale { steps: RADIUS_SCALE.to_vec(), source: ScaleSource::Default, fallback_reason: reason, unique_var_by_step: HashMap::new() }
        }
    };

    // 3. Typography scale (local text styles' `font_size` + `NUMBER` variables allowing `FONT_SIZE`)
    let typography = if let Some(steps) = archetype.map(|a| dedup_sorted_positive(a.typography.iter().copied())).filter(|s| s.len() >= 3) {
        Scale { steps, source: ScaleSource::Archetype, fallback_reason: None, unique_var_by_step: HashMap::new() }
    } else {
        let (mut var_steps, unique_var_by_step, var_count) = collect_variable_steps(doc, &store, ScaleKind::Typography);
        let mut style_steps = Vec::new();
        for (k, s) in &doc.styles {
            if let Some(lib_id) = style_library_id(doc, k) {
                if enabled_library_name(doc, lib_id).is_none() {
                    continue;
                }
            }
            if let Style::Text(ts) = s {
                if ts.font_size.is_finite() && ts.font_size > SCALE_TOLERANCE {
                    style_steps.push(ts.font_size);
                }
            }
        }
        let has_styles = !style_steps.is_empty();
        var_steps.extend(style_steps);
        let combined = dedup_sorted_positive(var_steps);
        if combined.len() >= 3 {
            let source = if has_styles { ScaleSource::Styles } else { ScaleSource::Variables };
            Scale { steps: combined, source, fallback_reason: None, unique_var_by_step }
        } else {
            let reason = (has_styles || var_count > 0).then(|| format!("only {} distinct positive typography step(s); minimum is 3", combined.len()));
            Scale { steps: FONT_SCALE.to_vec(), source: ScaleSource::Default, fallback_reason: reason, unique_var_by_step: HashMap::new() }
        }
    };

    Scales { spacing, radius, typography }
}

fn num_json(x: f32) -> Value {
    if (x - x.round()).abs() < 0.0005 {
        json!(x.round() as i64)
    } else {
        let r = (x as f64 * 1000.0).round() / 1000.0;
        json!(r)
    }
}

#[allow(clippy::too_many_arguments)]
fn check_scale_property(
    scale: &Scale,
    scale_label: &str,
    is_radius: bool,
    node: &Node,
    page_bounds: Rect,
    xpath: &LazyXPath,
    severity: Severity,
    property: &str,
    val: f32,
    suppressed_props: &BTreeSet<String>,
    out: &mut Vec<Finding>,
) {
    if suppressed_props.contains(property) {
        return;
    }
    match scale.check(val, is_radius) {
        ScaleMatch::OnScale => {}
        ScaleMatch::Nearest { step, unique_var } => {
            let val_json = num_json(val);
            let step_json = num_json(step);
            let (suggested_fix, mut message) = if let Some((token_name, var_ref, _)) = unique_var {
                (
                    Some(FixSuggestion { kind: "bind".into(), property: property.to_string(), from: val_json.clone(), to: json!(var_ref) }),
                    format!("{property} {} px is off the {scale_label} scale (nearest: {} px = {token_name}).", fmt_num(val), fmt_num(step)),
                )
            } else {
                (
                    Some(FixSuggestion { kind: "set".into(), property: property.to_string(), from: val_json.clone(), to: step_json.clone() }),
                    format!("{property} {} px is off the {scale_label} scale (nearest: {} px).", fmt_num(val), fmt_num(step)),
                )
            };
            if let Some(reason) = &scale.fallback_reason {
                message.push_str(&format!(" ({reason})"));
            }
            out.push(Finding {
                rule: "off-scale",
                category: Category::Tokens,
                severity,
                node: node.id,
                node_name: node.name.clone(),
                xpath: xpath.get().to_string(),
                bounds: page_bounds,
                message,
                suggested_fix,
                fixable: true,
                safe: false,
                property: Some(property.to_string()),
                value: Some(val_json),
                nearest: Some(step_json),
                scale_source: Some(scale.source),
                candidates: None,
                ignored: false,
            });
        }
        ScaleMatch::Equidistant { low, high } => {
            let val_json = num_json(val);
            let nearest_json = json!([num_json(low), num_json(high)]);
            let mut message = format!("{property} {} px is off the {scale_label} scale (nearest: {} or {}).", fmt_num(val), fmt_num(low), fmt_num(high));
            if let Some(reason) = &scale.fallback_reason {
                message.push_str(&format!(" ({reason})"));
            }
            out.push(Finding {
                rule: "off-scale",
                category: Category::Tokens,
                severity,
                node: node.id,
                node_name: node.name.clone(),
                xpath: xpath.get().to_string(),
                bounds: page_bounds,
                message,
                suggested_fix: None,
                fixable: false,
                safe: false,
                property: Some(property.to_string()),
                value: Some(val_json),
                nearest: Some(nearest_json),
                scale_source: Some(scale.source),
                candidates: None,
                ignored: false,
            });
        }
    }
}

/// Evaluates `off-scale` on `node` using the precomputed [`Scales`].
/// Any `(node, property)` already reported by `hardcoded-value` in `suppressed_props` is skipped (§8.6).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_off_scale(
    doc: &Document,
    scales: &Scales,
    config: &LintConfig,
    node: &Node,
    page_bounds: Rect,
    xpath: &LazyXPath,
    suppressed_props: &BTreeSet<String>,
    out: &mut Vec<Finding>,
) {
    let Some(severity) = config.rule_severity("off-scale") else { return };
    if matches!(node.kind, NodeKind::Unknown { .. } | NodeKind::Page { .. }) {
        return;
    }
    let src_resolved = node.component.source.and_then(|s| crate::resolved_node(doc, s));
    if node.component.source.is_some() && src_resolved.is_none() {
        return;
    }
    let src = src_resolved.as_deref();

    // 1. Auto-layout spacing (`itemSpacing`, `counterAxisSpacing` when wrap, and 4 paddings).
    if let Some(al) = node.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) {
        let is_space_between = matches!(al.primary_align, LayoutAlign::SpaceBetween | LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly);
        if al.item_spacing >= 0.0 && !is_space_between && !node.variables.bindings.contains_key("itemSpacing") && !number_inherited(node, src, "itemSpacing") {
            check_scale_property(&scales.spacing, "spacing", false, node, page_bounds, xpath, severity, "itemSpacing", al.item_spacing, suppressed_props, out);
        }

        if al.wrap
            && al.counter_spacing >= 0.0
            && !node.variables.bindings.contains_key("counterAxisSpacing")
            && !number_inherited(node, src, "counterAxisSpacing")
        {
            check_scale_property(
                &scales.spacing,
                "spacing",
                false,
                node,
                page_bounds,
                xpath,
                severity,
                "counterAxisSpacing",
                al.counter_spacing,
                suppressed_props,
                out,
            );
        }

        for (i, key) in PADDING_KEYS.iter().enumerate() {
            let val = al.padding[i];
            if val >= 0.0 && !node.variables.bindings.contains_key(*key) && !number_inherited(node, src, key) {
                check_scale_property(&scales.spacing, "spacing", false, node, page_bounds, xpath, severity, key, val, suppressed_props, out);
            }
        }
    }

    // 2. Corner radii (each corner).
    if !matches!(node.kind, NodeKind::Text(_) | NodeKind::Line | NodeKind::Group | NodeKind::Vector { .. } | NodeKind::Connector { .. }) {
        for (i, key) in RADIUS_KEYS.iter().enumerate() {
            let val = node.corner_radii[i];
            if (0.0..999.0).contains(&val) && !node.variables.bindings.contains_key(*key) && !number_inherited(node, src, key) {
                check_scale_property(&scales.radius, "radius", true, node, page_bounds, xpath, severity, key, val, suppressed_props, out);
            }
        }
    }

    // 3. Font size of text style and runs.
    if let NodeKind::Text(tp) = &node.kind {
        // §8.5: Text bound to a text style is skipped (the style defines the scale).
        let bound_to_text_style = node.style_refs.contains_key("text");
        if !bound_to_text_style && !node.variables.bindings.contains_key("fontSize") && !text_style_inherited(node, src) {
            check_scale_property(&scales.typography, "type", false, node, page_bounds, xpath, severity, "fontSize", tp.style.font_size, suppressed_props, out);
            for (run_idx, run) in tp.runs.iter().enumerate() {
                if !same_number(run.style.font_size, tp.style.font_size) {
                    let prop = format!("runs/{run_idx}/fontSize");
                    check_scale_property(
                        &scales.typography,
                        "type",
                        false,
                        node,
                        page_bounds,
                        xpath,
                        severity,
                        &prop,
                        run.style.font_size,
                        suppressed_props,
                        out,
                    );
                }
            }
        }
    }
}

fn collection_var_order(doc: &Document, v: &Variable) -> usize {
    doc.variable_collections.get(&v.collection).and_then(|c| c.variables.iter().position(|id| id == &v.id)).unwrap_or(usize::MAX)
}

fn ordered_candidates(doc: &Document, vars: &[&Entry]) -> Vec<String> {
    let mut sorted: Vec<&Entry> = vars.to_vec();
    sorted.sort_by(|a, b| {
        let ord_a = collection_var_order(doc, &a.var);
        let ord_b = collection_var_order(doc, &b.var);
        (ord_a, &a.var.name, &a.fix_id).cmp(&(ord_b, &b.var.name, &b.fix_id))
    });
    sorted.into_iter().map(|e| e.token_name()).collect()
}

#[allow(clippy::too_many_arguments)]
fn emit_number_match(
    doc: &Document,
    index: &Index,
    node: &Node,
    page_bounds: Rect,
    xpath: &LazyXPath,
    severity: Severity,
    prop_key: &str,
    keys: &[&str],
    label: &str,
    val: f32,
    require_explicit_scope: bool,
    emitted_props: &mut BTreeSet<String>,
    out: &mut Vec<Finding>,
) {
    let Some(&first_key) = keys.first() else { return };
    match index.find_number(doc, node, first_key, val) {
        Match::None => {}
        Match::One(e) => {
            if require_explicit_scope && e.var.scopes.is_empty() {
                return;
            }
            for &k in keys {
                emitted_props.insert(k.to_string());
            }
            emitted_props.insert(prop_key.to_string());
            let val_json = num_json(val);
            let var_ref = e.var_ref();
            let message = format!("{label} {} matches variable \"{}\"", fmt_num(val), e.token_name());
            out.push(Finding {
                rule: "hardcoded-value",
                category: Category::Tokens,
                severity,
                node: node.id,
                node_name: node.name.clone(),
                xpath: xpath.get().to_string(),
                bounds: page_bounds,
                message,
                suggested_fix: Some(FixSuggestion { kind: "bind".into(), property: prop_key.to_string(), from: val_json.clone(), to: json!(var_ref) }),
                fixable: true,
                safe: e.constant,
                property: Some(prop_key.to_string()),
                value: Some(val_json),
                nearest: None,
                scale_source: None,
                candidates: None,
                ignored: false,
            });
        }
        Match::Ambiguous(vars) => {
            let filtered: Vec<&Entry> = if require_explicit_scope { vars.into_iter().filter(|e| !e.var.scopes.is_empty()).collect() } else { vars };
            if filtered.is_empty() {
                return;
            }
            for &k in keys {
                emitted_props.insert(k.to_string());
            }
            emitted_props.insert(prop_key.to_string());
            let val_json = num_json(val);
            let cands = ordered_candidates(doc, &filtered);
            let message = format!("{label} {} matches {} variables ({}); pick one.", fmt_num(val), cands.len(), cands.join(", "));
            out.push(Finding {
                rule: "hardcoded-value",
                category: Category::Tokens,
                severity,
                node: node.id,
                node_name: node.name.clone(),
                xpath: xpath.get().to_string(),
                bounds: page_bounds,
                message,
                suggested_fix: None,
                fixable: false,
                safe: false,
                property: Some(prop_key.to_string()),
                value: Some(val_json),
                nearest: None,
                scale_source: None,
                candidates: Some(cands),
                ignored: false,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_single_number_lint(
    doc: &Document,
    index: &Index,
    node: &Node,
    src: Option<&Node>,
    page_bounds: Rect,
    xpath: &LazyXPath,
    severity: Severity,
    key: &str,
    label: &str,
    require_explicit_scope: bool,
    emitted_props: &mut BTreeSet<String>,
    out: &mut Vec<Finding>,
) {
    if node.variables.bindings.contains_key(key) || number_inherited(node, src, key) {
        return;
    }
    let Some(val) = number_value_at(node, key) else { return };
    emit_number_match(doc, index, node, page_bounds, xpath, severity, key, &[key], label, val, require_explicit_scope, emitted_props, out);
}

#[allow(clippy::too_many_arguments)]
fn check_quad_lint(
    doc: &Document,
    index: &Index,
    node: &Node,
    src: Option<&Node>,
    page_bounds: Rect,
    xpath: &LazyXPath,
    severity: Severity,
    uniform_key: &str,
    uniform_label: &str,
    sides: [(&str, &str); 4],
    emitted_props: &mut BTreeSet<String>,
    out: &mut Vec<Finding>,
) {
    if sides.iter().all(|(k, _)| number_inherited(node, src, k)) {
        return;
    }
    let all_unbound = sides.iter().all(|(k, _)| !node.variables.bindings.contains_key(*k));
    if all_unbound {
        if let Some(v0) = number_value_at(node, sides[0].0) {
            let all_equal = sides[1..].iter().all(|(k, _)| number_value_at(node, k).is_some_and(|v| same_number(v, v0)));
            if all_equal {
                let keys = [sides[0].0, sides[1].0, sides[2].0, sides[3].0];
                emit_number_match(doc, index, node, page_bounds, xpath, severity, uniform_key, &keys, uniform_label, v0, false, emitted_props, out);
                return;
            }
        }
    }
    for (k, label) in sides {
        check_single_number_lint(doc, index, node, src, page_bounds, xpath, severity, k, label, false, emitted_props, out);
    }
}

#[allow(clippy::too_many_arguments)]
fn check_paints_lint(
    doc: &Document,
    index: &Index,
    node: &Node,
    src: Option<&Node>,
    slot: Slot,
    page_bounds: Rect,
    xpath: &LazyXPath,
    severity: Severity,
    emitted_props: &mut BTreeSet<String>,
    out: &mut Vec<Finding>,
) {
    let paints = slot.paints(node);
    if paints.is_empty() || node.style_refs.contains_key(slot.ref_key()) {
        return;
    }
    if let Some(s) = src {
        let same_paints = slot.paints(s) == paints && s.style_refs.get(slot.ref_key()) == node.style_refs.get(slot.ref_key());
        let same_bindings = (0..paints.len()).all(|i| {
            let key = format!("{}/{i}", slot.prefix());
            s.variables.bindings.get(&key) == node.variables.bindings.get(&key)
        });
        if same_paints && same_bindings {
            return;
        }
    }
    let mut matched_variable = false;
    for (i, p) in paints.iter().enumerate() {
        let PaintKind::Solid(c) = &p.kind else { continue };
        let key = format!("{}/{i}", slot.prefix());
        if !p.visible || node.variables.bindings.contains_key(&key) {
            continue;
        }
        if src.is_some_and(|s| slot.paints(s).get(i) == Some(p) && s.variables.bindings.get(&key) == node.variables.bindings.get(&key)) {
            continue;
        }
        let hex = format!("#{}", c.to_hex());
        match index.find(doc, node, &key, c) {
            Match::None => {}
            Match::One(e) => {
                matched_variable = true;
                emitted_props.insert(key.clone());
                let var_ref = e.var_ref();
                let message = format!("{} {} matches variable \"{}\"", slot.label(), hex, e.token_name());
                out.push(Finding {
                    rule: "hardcoded-value",
                    category: Category::Tokens,
                    severity,
                    node: node.id,
                    node_name: node.name.clone(),
                    xpath: xpath.get().to_string(),
                    bounds: page_bounds,
                    message,
                    suggested_fix: Some(FixSuggestion { kind: "bind".into(), property: key.clone(), from: json!(hex), to: json!(var_ref) }),
                    fixable: true,
                    safe: e.constant,
                    property: Some(key),
                    value: Some(json!(hex)),
                    nearest: None,
                    scale_source: None,
                    candidates: None,
                    ignored: false,
                });
            }
            Match::Ambiguous(vars) => {
                matched_variable = true;
                emitted_props.insert(key.clone());
                let cands = ordered_candidates(doc, &vars);
                let message = format!("{} {} matches {} variables ({}); pick one.", slot.label(), hex, cands.len(), cands.join(", "));
                out.push(Finding {
                    rule: "hardcoded-value",
                    category: Category::Tokens,
                    severity,
                    node: node.id,
                    node_name: node.name.clone(),
                    xpath: xpath.get().to_string(),
                    bounds: page_bounds,
                    message,
                    suggested_fix: None,
                    fixable: false,
                    safe: false,
                    property: Some(key),
                    value: Some(json!(hex)),
                    nearest: None,
                    scale_source: None,
                    candidates: Some(cands),
                    ignored: false,
                });
            }
        }
    }
    let any_bound = node.variables.bindings.keys().any(|k| k.starts_with(slot.prefix()));
    if matched_variable || any_bound {
        return;
    }
    let style = index.styles.iter().find(|s| matches!(&s.style, Style::Fill(ps) if ps == paints) && s.property == slot.property());
    if let Some(s) = style {
        let prop = slot.ref_key().to_string();
        emitted_props.insert(prop.clone());
        let raw_val = if paints.len() == 1 {
            if let PaintKind::Solid(c) = &paints[0].kind {
                json!(format!("#{}", c.to_hex()))
            } else {
                json!(slot.ref_key())
            }
        } else {
            json!(slot.ref_key())
        };
        let style_ref = s.style_ref();
        out.push(Finding {
            rule: "hardcoded-value",
            category: Category::Tokens,
            severity,
            node: node.id,
            node_name: node.name.clone(),
            xpath: xpath.get().to_string(),
            bounds: page_bounds,
            message: format!("{} matches style \"{}\"", slot.label(), s.name),
            suggested_fix: Some(FixSuggestion { kind: "bind".into(), property: prop.clone(), from: raw_val.clone(), to: json!(style_ref) }),
            fixable: true,
            safe: true,
            property: Some(prop),
            value: Some(raw_val),
            nearest: None,
            scale_source: None,
            candidates: None,
            ignored: false,
        });
    }
}

/// Evaluates `hardcoded-value` on `node` using the shared [`Index`] and returns the set of
/// property keys emitted on `node` (so `off-scale` suppresses duplicates per §8.6).
pub fn evaluate_hardcoded_value(
    doc: &Document,
    index: &Index,
    config: &LintConfig,
    node: &Node,
    page_bounds: Rect,
    xpath: &LazyXPath,
    out: &mut Vec<Finding>,
) -> BTreeSet<String> {
    let mut emitted_props = BTreeSet::new();
    let Some(severity) = config.rule_severity("hardcoded-value") else {
        return emitted_props;
    };
    if matches!(node.kind, NodeKind::Unknown { .. } | NodeKind::Page { .. }) {
        return emitted_props;
    }
    let resolved = crate::resolved_node(doc, node.id);
    let rn = resolved.as_deref().unwrap_or(node);
    let src_resolved = node.component.source.and_then(|s| crate::resolved_node(doc, s));
    if node.component.source.is_some() && src_resolved.is_none() {
        return emitted_props;
    }
    let src = src_resolved.as_deref();

    // 1. Fill & stroke paints (variables first, then whole-list paint style).
    check_paints_lint(doc, index, rn, src, Slot::Fill, page_bounds, xpath, severity, &mut emitted_props, out);
    if rn.stroke.weight > 0.0 {
        check_paints_lint(doc, index, rn, src, Slot::Stroke, page_bounds, xpath, severity, &mut emitted_props, out);
    }

    // 2. Text run fills (when runs override the base text fill).
    if let NodeKind::Text(tp) = &rn.kind {
        if !rn.style_refs.contains_key("fill") {
            for (run_idx, run) in tp.runs.iter().enumerate() {
                if run.style.fills == tp.style.fills {
                    continue;
                }
                for (i, p) in run.style.fills.iter().enumerate() {
                    let PaintKind::Solid(c) = &p.kind else { continue };
                    let prop = format!("runs/{run_idx}/fills/{i}");
                    if !p.visible || rn.variables.bindings.contains_key(&prop) {
                        continue;
                    }
                    let hex = format!("#{}", c.to_hex());
                    match index.find(doc, rn, "fills/0", c) {
                        Match::None => {}
                        Match::One(e) => {
                            emitted_props.insert(prop.clone());
                            let var_ref = e.var_ref();
                            out.push(Finding {
                                rule: "hardcoded-value",
                                category: Category::Tokens,
                                severity,
                                node: rn.id,
                                node_name: rn.name.clone(),
                                xpath: xpath.get().to_string(),
                                bounds: page_bounds,
                                message: format!("Text run fill {hex} matches variable \"{}\"", e.token_name()),
                                suggested_fix: Some(FixSuggestion { kind: "bind".into(), property: prop.clone(), from: json!(hex), to: json!(var_ref) }),
                                fixable: true,
                                safe: e.constant,
                                property: Some(prop),
                                value: Some(json!(hex)),
                                nearest: None,
                                scale_source: None,
                                candidates: None,
                                ignored: false,
                            });
                        }
                        Match::Ambiguous(vars) => {
                            emitted_props.insert(prop.clone());
                            let cands = ordered_candidates(doc, &vars);
                            out.push(Finding {
                                rule: "hardcoded-value",
                                category: Category::Tokens,
                                severity,
                                node: rn.id,
                                node_name: rn.name.clone(),
                                xpath: xpath.get().to_string(),
                                bounds: page_bounds,
                                message: format!("Text run fill {hex} matches {} variables ({}); pick one.", cands.len(), cands.join(", ")),
                                suggested_fix: None,
                                fixable: false,
                                safe: false,
                                property: Some(prop),
                                value: Some(json!(hex)),
                                nearest: None,
                                scale_source: None,
                                candidates: Some(cands),
                                ignored: false,
                            });
                        }
                    }
                }
            }
        }
    }

    // 3. Number properties.
    if !index.by_num.is_empty() {
        check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "itemSpacing", "Gap", false, &mut emitted_props, out);
        check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "counterAxisSpacing", "Row gap", false, &mut emitted_props, out);
        check_quad_lint(
            doc,
            index,
            rn,
            src,
            page_bounds,
            xpath,
            severity,
            "padding",
            "Padding",
            [("paddingTop", "Top padding"), ("paddingRight", "Right padding"), ("paddingBottom", "Bottom padding"), ("paddingLeft", "Left padding")],
            &mut emitted_props,
            out,
        );
        check_quad_lint(
            doc,
            index,
            rn,
            src,
            page_bounds,
            xpath,
            severity,
            "cornerRadius",
            "Corner radius",
            [
                ("topLeftRadius", "Top-left radius"),
                ("topRightRadius", "Top-right radius"),
                ("bottomRightRadius", "Bottom-right radius"),
                ("bottomLeftRadius", "Bottom-left radius"),
            ],
            &mut emitted_props,
            out,
        );
        check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "strokeWeight", "Stroke weight", false, &mut emitted_props, out);
        for (k, label) in [
            ("strokeTopWeight", "Stroke top weight"),
            ("strokeRightWeight", "Stroke right weight"),
            ("strokeBottomWeight", "Stroke bottom weight"),
            ("strokeLeftWeight", "Stroke left weight"),
        ] {
            check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, k, label, false, &mut emitted_props, out);
        }
        check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "width", "Width", true, &mut emitted_props, out);
        check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "height", "Height", true, &mut emitted_props, out);
        check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "opacity", "Opacity", true, &mut emitted_props, out);

        if matches!(rn.kind, NodeKind::Text(_)) && !rn.style_refs.contains_key("text") && !text_style_inherited(rn, src) {
            check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "fontSize", "Font size", false, &mut emitted_props, out);
            check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "lineHeight", "Line height", false, &mut emitted_props, out);
            check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "letterSpacing", "Letter spacing", true, &mut emitted_props, out);
            check_single_number_lint(doc, index, rn, src, page_bounds, xpath, severity, "paragraphSpacing", "Paragraph spacing", true, &mut emitted_props, out);
        }
    }

    // 3b. Shared text style match (whole-style match on unlinked non-empty text).
    if let NodeKind::Text(tp) = &rn.kind {
        if !tp.content.trim().is_empty() && !rn.style_refs.contains_key("text") && !text_style_inherited(rn, src) {
            match index.matching_text_styles(tp).as_slice() {
                [] => {}
                [s] => {
                    emitted_props.insert("text".into());
                    emitted_props.insert("fontSize".into());
                    let style_ref = s.style_ref();
                    out.push(Finding {
                        rule: "hardcoded-value",
                        category: Category::Tokens,
                        severity,
                        node: rn.id,
                        node_name: rn.name.clone(),
                        xpath: xpath.get().to_string(),
                        bounds: page_bounds,
                        message: format!("Text matches style {}", fmt_style_quoted(s)),
                        suggested_fix: Some(FixSuggestion { kind: "bind".into(), property: "text".into(), from: json!("text"), to: json!(style_ref) }),
                        fixable: true,
                        safe: true,
                        property: Some("text".into()),
                        value: Some(json!("text")),
                        nearest: None,
                        scale_source: None,
                        candidates: None,
                        ignored: false,
                    });
                }
                many => {
                    emitted_props.insert("text".into());
                    emitted_props.insert("fontSize".into());
                    let cands: Vec<String> = many.iter().map(|s| s.name.clone()).collect();
                    out.push(Finding {
                        rule: "hardcoded-value",
                        category: Category::Tokens,
                        severity,
                        node: rn.id,
                        node_name: rn.name.clone(),
                        xpath: xpath.get().to_string(),
                        bounds: page_bounds,
                        message: format!("Text matches {} styles ({}); pick one.", cands.len(), cands.join(", ")),
                        suggested_fix: None,
                        fixable: false,
                        safe: false,
                        property: Some("text".into()),
                        value: Some(json!("text")),
                        nearest: None,
                        scale_source: None,
                        candidates: Some(cands),
                        ignored: false,
                    });
                }
            }
        }
    }

    // 4. Effects (whole-list effect style, or individual shadow colour variables).
    if !rn.effects.is_empty() && !rn.style_refs.contains_key("effect") && !src.is_some_and(|s| s.effects == rn.effects) {
        let style = index.styles.iter().find(|s| matches!(&s.style, Style::Effect(fx) if *fx == rn.effects));
        if let Some(s) = style {
            emitted_props.insert("effect".into());
            let style_ref = s.style_ref();
            out.push(Finding {
                rule: "hardcoded-value",
                category: Category::Tokens,
                severity,
                node: rn.id,
                node_name: rn.name.clone(),
                xpath: xpath.get().to_string(),
                bounds: page_bounds,
                message: format!("Effects match style \"{}\"", s.name),
                suggested_fix: Some(FixSuggestion { kind: "bind".into(), property: "effect".into(), from: json!("effect"), to: json!(style_ref) }),
                fixable: true,
                safe: true,
                property: Some("effect".into()),
                value: Some(json!("effect")),
                nearest: None,
                scale_source: None,
                candidates: None,
                ignored: false,
            });
        } else {
            for (i, eff) in rn.effects.iter().enumerate() {
                let (Effect::DropShadow { color: c, visible: true, .. } | Effect::InnerShadow { color: c, visible: true, .. }) = eff else {
                    continue;
                };
                let prop = format!("effects/{i}/color");
                if rn.variables.bindings.contains_key(&prop) {
                    continue;
                }
                let hex = format!("#{}", c.to_hex());
                match index.find(doc, rn, "fills/0", c) {
                    Match::None => {}
                    Match::One(e) => {
                        if !e.var.scopes.is_empty() && !e.var.scopes.iter().any(|s| s == "ALL_SCOPES" || s == "EFFECT_COLOR" || s == "ALL_FILLS") {
                            continue;
                        }
                        emitted_props.insert(prop.clone());
                        let var_ref = e.var_ref();
                        out.push(Finding {
                            rule: "hardcoded-value",
                            category: Category::Tokens,
                            severity,
                            node: rn.id,
                            node_name: rn.name.clone(),
                            xpath: xpath.get().to_string(),
                            bounds: page_bounds,
                            message: format!("Effect color {hex} matches variable \"{}\"", e.token_name()),
                            suggested_fix: Some(FixSuggestion { kind: "bind".into(), property: prop.clone(), from: json!(hex), to: json!(var_ref) }),
                            fixable: true,
                            safe: e.constant,
                            property: Some(prop),
                            value: Some(json!(hex)),
                            nearest: None,
                            scale_source: None,
                            candidates: None,
                            ignored: false,
                        });
                    }
                    Match::Ambiguous(vars) => {
                        let filtered: Vec<&Entry> = vars
                            .into_iter()
                            .filter(|e| e.var.scopes.is_empty() || e.var.scopes.iter().any(|s| s == "ALL_SCOPES" || s == "EFFECT_COLOR" || s == "ALL_FILLS"))
                            .collect();
                        if filtered.is_empty() {
                            continue;
                        }
                        emitted_props.insert(prop.clone());
                        let cands = ordered_candidates(doc, &filtered);
                        out.push(Finding {
                            rule: "hardcoded-value",
                            category: Category::Tokens,
                            severity,
                            node: rn.id,
                            node_name: rn.name.clone(),
                            xpath: xpath.get().to_string(),
                            bounds: page_bounds,
                            message: format!("Effect color {hex} matches {} variables ({}); pick one.", cands.len(), cands.join(", ")),
                            suggested_fix: None,
                            fixable: false,
                            safe: false,
                            property: Some(prop),
                            value: Some(json!(hex)),
                            nearest: None,
                            scale_source: None,
                            candidates: Some(cands),
                            ignored: false,
                        });
                    }
                }
            }
        }
    }

    emitted_props
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::{lint, NoLintEnv};
    use crate::{AutoLayout, Color, LayoutDirection, NodeId, Paint, StyleMeta, StyleProperty, TextStyle, VariableCollection, VariableMode, Vec2};

    fn new_doc() -> (Document, NodeId) {
        let mut doc = Document::new("E15-11 Test");
        let page = doc.add_page("Page 1");
        (doc, page)
    }

    fn ensure_collection(doc: &mut Document, id: &str, name: &str, modes: &[(&str, &str)]) {
        if !doc.variable_collections.contains_key(id) {
            let mode_vec: Vec<VariableMode> = modes.iter().map(|(mid, mname)| VariableMode { id: (*mid).into(), name: (*mname).into() }).collect();
            let default_mode = mode_vec.first().map(|m| m.id.clone()).unwrap_or_default();
            doc.variable_collections.insert(
                id.into(),
                VariableCollection {
                    id: id.into(),
                    name: name.into(),
                    modes: mode_vec,
                    default_mode,
                    variables: Vec::new(),
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
        }
    }

    fn add_number_var(doc: &mut Document, coll_id: &str, coll_name: &str, var_id: &str, var_name: &str, val: f32, scopes: &[&str]) {
        ensure_collection(doc, coll_id, coll_name, &[("default", "Default")]);
        doc.variable_collections.get_mut(coll_id).unwrap().variables.push(var_id.into());
        doc.variables.insert(
            var_id.into(),
            Variable {
                id: var_id.into(),
                name: var_name.into(),
                collection: coll_id.into(),
                kind: VariableKind::Number,
                values: [("default".to_string(), VariableValue::Number(val))].into_iter().collect(),
                description: String::new(),
                scopes: scopes.iter().map(|s| (*s).to_string()).collect(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
    }

    fn add_color_var(doc: &mut Document, coll_id: &str, coll_name: &str, var_id: &str, var_name: &str, modes: &[(&str, Color)]) {
        let mode_defs: Vec<(&str, &str)> = modes.iter().map(|(m, _)| (*m, *m)).collect();
        ensure_collection(doc, coll_id, coll_name, &mode_defs);
        doc.variable_collections.get_mut(coll_id).unwrap().variables.push(var_id.into());
        doc.variables.insert(
            var_id.into(),
            Variable {
                id: var_id.into(),
                name: var_name.into(),
                collection: coll_id.into(),
                kind: VariableKind::Color,
                values: modes.iter().map(|(m, c)| ((*m).to_string(), VariableValue::Color(*c))).collect(),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
    }

    #[test]
    fn gap_15_on_4_step_scale_suggests_16() {
        let (mut doc, page) = new_doc();
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 80.0));
        frame.name = "Feature row".into();
        frame.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 15.0, ..Default::default() }),
            ..Default::default()
        });
        doc.insert(page, frame).unwrap();

        let cfg = LintConfig::from_rule_list(&["off-scale".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report.findings.len(), 1);
        let f = &report.findings[0];
        assert_eq!(f.rule, "off-scale");
        assert_eq!(f.property.as_deref(), Some("itemSpacing"));
        assert_eq!(f.value, Some(json!(15)));
        assert_eq!(f.nearest, Some(json!(16)));
        assert_eq!(f.scale_source, Some(ScaleSource::Default));
        assert!(!f.safe);
        assert!(f.fixable);
        let fix = f.suggested_fix.as_ref().expect("suggested_fix");
        assert_eq!(fix.kind, "set");
        assert_eq!(fix.property, "itemSpacing");
        assert_eq!(fix.from, json!(15));
        assert_eq!(fix.to, json!(16));
    }

    #[test]
    fn variables_define_the_scale() {
        let (mut doc, page) = new_doc();
        for (i, step) in [4.0, 8.0, 12.0, 16.0, 24.0].into_iter().enumerate() {
            add_number_var(&mut doc, "spacing", "Spacing", &format!("sp_{i}"), &format!("Spacing/{}", step as i64), step, &["GAP"]);
        }
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 80.0));
        frame.name = "Row".into();
        frame.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 20.0, ..Default::default() }),
            ..Default::default()
        });
        doc.insert(page, frame).unwrap();

        let cfg = LintConfig::from_rule_list(&["off-scale".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report.findings.len(), 1);
        let f = &report.findings[0];
        assert_eq!(f.rule, "off-scale");
        assert_eq!(f.scale_source, Some(ScaleSource::Variables));
        assert_eq!(f.nearest, Some(json!([16, 24])));
        assert!(f.suggested_fix.is_none(), "equidistant 20 between 16 and 24 must have no fix");
        assert!(!f.fixable);
        assert!(!f.safe);
        assert!(f.message.contains("nearest: 16 or 24"), "message was: {}", f.message);
    }

    #[test]
    fn type_scale_from_text_styles() {
        let (mut doc, page) = new_doc();
        for (i, sz) in [12.0, 14.0, 16.0, 20.0].into_iter().enumerate() {
            let key = format!("ts_{i}");
            doc.styles.insert(key.clone(), Style::Text(TextStyle { font_size: sz, ..Default::default() }));
            doc.style_meta.insert(key, StyleMeta { name: format!("Type/{}", sz as i64), property: StyleProperty::Text, ..Default::default() });
        }
        let tid = doc.alloc_id();
        let txt = Node::text(tid, Rect::new(0.0, 0.0, 120.0, 24.0), "Caption", TextStyle { font_size: 13.0, ..Default::default() });
        doc.insert(page, txt).unwrap();

        let cfg = LintConfig::from_rule_list(&["off-scale".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report.findings.len(), 1);
        let f = &report.findings[0];
        assert_eq!(f.rule, "off-scale");
        assert_eq!(f.property.as_deref(), Some("fontSize"));
        assert_eq!(f.scale_source, Some(ScaleSource::Styles));
        assert_eq!(f.nearest, Some(json!([12, 14])));
        assert!(f.suggested_fix.is_none());
        assert!(!f.fixable);
        assert!(!f.safe);
    }

    #[test]
    fn colour_equal_to_primary_gets_bind() {
        let (mut doc, page) = new_doc();
        let blue = Color::rgb(0x25, 0x63, 0xEB);
        add_color_var(&mut doc, "brand", "Brand", "primary", "Color/Primary", &[("default", blue)]);

        let rid = doc.alloc_id();
        let mut rect = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
        rect.name = "CTA".into();
        rect.fills = vec![Paint::solid(blue)];
        doc.insert(page, rect).unwrap();

        let cfg = LintConfig::from_rule_list(&["hardcoded-value".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report.findings.len(), 1);
        let f = &report.findings[0];
        assert_eq!(f.rule, "hardcoded-value");
        assert_eq!(f.property.as_deref(), Some("fills/0"));
        assert_eq!(f.value, Some(json!("#2563EB")));
        assert!(f.safe);
        assert!(f.fixable);
        let fix = f.suggested_fix.as_ref().expect("suggested_fix");
        assert_eq!(fix.kind, "bind");
        assert_eq!(fix.property, "fills/0");
        assert_eq!(fix.to, json!("var:Color/Primary"));
    }

    #[test]
    fn two_tokens_same_colour_is_ambiguous() {
        let (mut doc, page) = new_doc();
        let blue = Color::rgb(0x25, 0x63, 0xEB);
        add_color_var(&mut doc, "brand", "Brand", "primary", "Color/Primary", &[("default", blue)]);
        add_color_var(&mut doc, "brand", "Brand", "blue", "Brand/Blue", &[("default", blue)]);

        let rid = doc.alloc_id();
        let mut rect = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
        rect.name = "Badge".into();
        rect.fills = vec![Paint::solid(blue)];
        doc.insert(page, rect).unwrap();

        let cfg = LintConfig::from_rule_list(&["hardcoded-value".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report.findings.len(), 1);
        let f = &report.findings[0];
        assert_eq!(f.rule, "hardcoded-value");
        assert_eq!(f.candidates, Some(vec!["Color/Primary".to_string(), "Brand/Blue".to_string()]));
        assert!(f.suggested_fix.is_none());
        assert!(!f.fixable);
        assert!(!f.safe);
    }

    #[test]
    fn mode_dependent_match_is_unsafe() {
        let (mut doc, page) = new_doc();
        let white = Color::rgb(0xFF, 0xFF, 0xFF);
        let slate = Color::rgb(0x0F, 0x17, 0x2A);
        add_color_var(&mut doc, "theme", "Theme", "surface", "Surface", &[("light", white), ("dark", slate)]);

        let rid = doc.alloc_id();
        let mut rect = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
        rect.name = "Card".into();
        rect.fills = vec![Paint::solid(white)];
        doc.insert(page, rect).unwrap();

        let cfg = LintConfig::from_rule_list(&["hardcoded-value".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report.findings.len(), 1);
        let f = &report.findings[0];
        assert_eq!(f.rule, "hardcoded-value");
        assert!(f.suggested_fix.is_some());
        assert!(f.fixable);
        assert!(!f.safe, "mode-dependent variable match must have safe: false");
    }

    #[test]
    fn check_designs_and_linter_identical() {
        let mut d = Document::new("Check designs");
        let page = d.add_page("Page 1");
        let primary = Color { r: 13.0 / 255.0, g: 153.0 / 255.0, b: 1.0, a: 1.0 };
        add_color_var(&mut d, "brand", "Brand", "primary", "Color/Primary", &[("default", primary)]);

        let shadow =
            Effect::DropShadow { color: Color::rgba(0, 0, 0, 56), offset: Vec2::new(0.0, 6.0), radius: 16.0, spread: 0.0, visible: true, show_behind: false };
        d.styles.insert("shadow".into(), Style::Effect(vec![shadow.clone()]));
        d.style_meta.insert("shadow".into(), StyleMeta { name: "Shadow/Card".into(), property: StyleProperty::Effect, ..Default::default() });
        d.styles.insert("surface".into(), Style::Fill(vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))]));
        d.style_meta.insert("surface".into(), StyleMeta { name: "Surface/Card".into(), property: StyleProperty::Fill, ..Default::default() });

        let heading_ts = TextStyle { font_size: 24.0, font_weight: 700, ..TextStyle::default() };
        d.styles.insert("heading".into(), Style::Text(heading_ts.clone()));
        d.style_meta.insert("heading".into(), StyleMeta { name: "Heading/H2".into(), property: StyleProperty::Text, ..Default::default() });

        let card_id = d.alloc_id();
        let mut card = Node::frame(card_id, Rect::new(80.0, 80.0, 320.0, 240.0));
        card.name = "Pricing card".into();
        card.fills = vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))];
        card.effects = vec![shadow];
        d.insert(page, card).unwrap();

        let plan_id = d.alloc_id();
        let mut plan = Node::text(
            plan_id,
            Rect::new(24.0, 24.0, 200.0, 32.0),
            "Pro plan",
            TextStyle { fills: vec![Paint::solid(Color::rgb(0x22, 0x22, 0x22))], ..heading_ts },
        );
        plan.name = "Plan name".into();
        d.insert(card_id, plan).unwrap();

        let btn_id = d.alloc_id();
        let mut button = Node::rectangle(btn_id, Rect::new(24.0, 168.0, 272.0, 48.0));
        button.name = "Upgrade button".into();
        button.fills = vec![Paint::solid(primary)];
        d.insert(card_id, button).unwrap();

        let cfg = LintConfig::from_rule_list(&["hardcoded-value".into()]).unwrap();
        let report = lint(&d, &cfg, &NoLintEnv);
        let summary: Vec<(NodeId, &str, &str)> = report
            .findings
            .iter()
            .map(|f| (f.node, f.property.as_deref().unwrap_or(""), f.suggested_fix.as_ref().and_then(|s| s.to.as_str()).unwrap_or("")))
            .collect();
        assert_eq!(
            summary,
            vec![
                (card_id, "fill", "style:Surface/Card"),
                (card_id, "effect", "style:Shadow/Card"),
                (plan_id, "text", "style:Heading/H2"),
                (btn_id, "fills/0", "var:Color/Primary"),
            ]
        );
    }

    #[test]
    fn automatic_fixer_does_not_snap_off_scale_values() {
        let (mut doc, page) = new_doc();
        for (i, step) in [4.0, 8.0, 12.0, 16.0, 24.0].into_iter().enumerate() {
            add_number_var(&mut doc, "spacing", "Spacing", &format!("sp_{i}"), &format!("Spacing/{}", step as i64), step, &["GAP"]);
        }
        let f_tie = doc.alloc_id();
        let mut frame_tie = Node::frame(f_tie, Rect::new(0.0, 0.0, 200.0, 80.0));
        frame_tie.name = "TieRow".into();
        frame_tie.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 20.0, ..Default::default() }),
            ..Default::default()
        });
        doc.insert(page, frame_tie).unwrap();

        let f_snap = doc.alloc_id();
        let mut frame_snap = Node::frame(f_snap, Rect::new(0.0, 100.0, 200.0, 80.0));
        frame_snap.name = "SnapRow".into();
        frame_snap.corner_radii = [7.0; 4];
        frame_snap.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 15.0, ..Default::default() }),
            ..Default::default()
        });
        doc.insert(page, frame_snap).unwrap();

        let tid = doc.alloc_id();
        let mut txt = Node::text(tid, Rect::new(0.0, 200.0, 120.0, 24.0), "Custom size", TextStyle { font_size: 15.0, ..Default::default() });
        txt.name = "BodyCopy".into();
        doc.insert(page, txt).unwrap();

        crate::lint::lint_fix_document(&mut doc, Some(page), None);
        crate::lint::lint_fix_document(
            &mut doc,
            Some(page),
            Some(&["off-scale".into(), "inconsistent-spacing".into(), "inconsistent-radius".into(), "non-scale-font-size".into()]),
        );

        assert_eq!(doc.get(f_tie).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap().item_spacing, 20.0);
        assert_eq!(doc.get(f_snap).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap().item_spacing, 15.0);
        assert_eq!(doc.get(f_snap).unwrap().corner_radii, [7.0; 4]);
        assert_eq!(doc.get(tid).unwrap().kind.any_text().unwrap().style.font_size, 15.0);
    }

    #[test]
    fn check_designs_ignore_keys_respect_rule_and_property_and_fixer_skips_ignored() {
        let (mut doc, page) = new_doc();
        let blue = Color::rgb(0x25, 0x63, 0xEB);
        add_color_var(&mut doc, "brand", "Brand", "primary", "Color/Primary", &[("default", blue)]);
        add_number_var(&mut doc, "radii", "Radius", "rad_lg", "radius/lg", 12.0, &["CORNER_RADIUS"]);

        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 80.0));
        frame.name = "Card".into();
        frame.fills = vec![Paint::solid(blue)];
        frame.corner_radii = [12.0; 4];
        frame.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 15.0, ..Default::default() }),
            ..Default::default()
        });
        doc.insert(page, frame).unwrap();

        // Ignore the cornerRadius hardcoded-value finding using Check designs' rule ID
        doc.check_designs.ignored.insert(crate::IgnoreKey { node: fid, rule: "REQUIRE_CORNER_RADIUS_VARIABLES".into(), property: "cornerRadius".into() });

        let cfg = LintConfig::from_rule_list(&["off-scale".into(), "hardcoded-value".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert!(
            report.findings.iter().any(|f| f.rule == "off-scale" && f.property.as_deref() == Some("itemSpacing")),
            "ignoring REQUIRE_CORNER_RADIUS_VARIABLES must not suppress off-scale itemSpacing: {:#?}",
            report.findings
        );
        assert!(
            !report.findings.iter().any(|f| f.rule == "hardcoded-value" && f.property.as_deref() == Some("cornerRadius")),
            "ignoring REQUIRE_CORNER_RADIUS_VARIABLES + cornerRadius must suppress hardcoded-value on cornerRadius: {:#?}",
            report.findings
        );
        assert!(
            report.findings.iter().any(|f| f.rule == "hardcoded-value" && f.property.as_deref() == Some("fills/0")),
            "ignoring cornerRadius must not suppress fills/0 on the same node: {:#?}",
            report.findings
        );

        // Now ignore fills/0 with Check designs' RECOMMEND_VARIABLES key
        doc.check_designs.ignored.insert(crate::IgnoreKey { node: fid, rule: "RECOMMEND_VARIABLES".into(), property: "fills/0".into() });
        let report2 = lint(&doc, &cfg, &NoLintEnv);
        assert!(
            !report2.findings.iter().any(|f| f.rule == "hardcoded-value" && f.property.as_deref() == Some("fills/0")),
            "ignoring RECOMMEND_VARIABLES + fills/0 must suppress hardcoded-value on fills/0: {:#?}",
            report2.findings
        );

        // Automatic fixer must also skip binding the ignored fills/0
        crate::lint::lint_fix_document(&mut doc, Some(page), None);
        assert!(!doc.get(fid).unwrap().variables.bindings.contains_key("fills/0"), "lint_fix_document must not bind an ignored property");
    }

    #[test]
    fn ambiguous_text_styles_list_candidates_without_fix_in_hardcoded_value() {
        let (mut doc, page) = new_doc();
        let ts = TextStyle { font_size: 32.0, font_weight: 700, ..TextStyle::default() };
        doc.styles.insert("h1".into(), Style::Text(ts.clone()));
        doc.style_meta.insert("h1".into(), StyleMeta { name: "Heading/H1".into(), property: StyleProperty::Text, ..Default::default() });
        doc.styles.insert("h1b".into(), Style::Text(ts.clone()));
        doc.style_meta.insert("h1b".into(), StyleMeta { name: "Heading/H1 copy".into(), property: StyleProperty::Text, ..Default::default() });

        let tid = doc.alloc_id();
        let mut txt = Node::text(tid, Rect::new(0.0, 0.0, 200.0, 40.0), "Hero title", ts);
        txt.name = "Hero".into();
        doc.insert(page, txt).unwrap();

        let cfg = LintConfig::from_rule_list(&["hardcoded-value".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        let tf = report.findings.iter().find(|f| f.property.as_deref() == Some("text")).expect("text style finding");
        assert_eq!(tf.candidates, Some(vec!["Heading/H1".to_string(), "Heading/H1 copy".to_string()]));
        assert!(tf.suggested_fix.is_none());
        assert!(!tf.fixable);
        assert!(!tf.safe);
    }

    #[test]
    fn unscoped_number_variables_join_spacing_scale_and_fallback_reason_is_in_message() {
        let (mut doc, page) = new_doc();
        // Three unscoped variables named radius/* have empty scopes ("all scopes") so they join the spacing scale.
        add_number_var(&mut doc, "tokens", "Tokens", "r_sm", "radius/sm", 4.0, &[]);
        add_number_var(&mut doc, "tokens", "Tokens", "r_md", "radius/md", 8.0, &[]);
        add_number_var(&mut doc, "tokens", "Tokens", "r_lg", "radius/lg", 12.0, &[]);

        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 80.0));
        frame.name = "Row".into();
        frame.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 15.0, ..Default::default() }),
            ..Default::default()
        });
        doc.insert(page, frame).unwrap();

        let cfg = LintConfig::from_rule_list(&["off-scale".into()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].scale_source, Some(ScaleSource::Variables));
        assert_eq!(report.findings[0].nearest, Some(json!(12)));

        // When fewer than 3 variables exist, fallback to default scale includes the fallback_reason in message.
        doc.variables.remove("r_lg");
        doc.variable_collections.get_mut("tokens").unwrap().variables.retain(|v| v != "r_lg");
        let report_fb = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report_fb.findings.len(), 1);
        assert_eq!(report_fb.findings[0].scale_source, Some(ScaleSource::Default));
        assert!(
            report_fb.findings[0].message.contains("only 2 distinct positive GAP variable step(s); minimum is 3"),
            "fallback message must include fallback_reason, got: {}",
            report_fb.findings[0].message
        );
    }

    #[test]
    fn empty_recommend_variables_ignore_or_contrast_ignore_does_not_suppress_hardcoded_value_properties() {
        let (mut doc, page) = new_doc();
        let muted = Color::rgb(0x94, 0xA3, 0xB8);
        add_color_var(&mut doc, "brand", "Brand", "muted", "Color/Muted", &[("default", muted)]);
        add_number_var(&mut doc, "type", "Type", "fs_body", "Font/Body", 14.0, &["FONT_SIZE"]);

        let ts = TextStyle { font_size: 14.0, font_weight: 400, ..TextStyle::default() };
        doc.styles.insert("body_ts".into(), Style::Text(ts.clone()));
        doc.style_meta.insert("body_ts".into(), StyleMeta { name: "Body/Regular".into(), property: StyleProperty::Text, ..Default::default() });

        let tid = doc.alloc_id();
        let mut txt = Node::text(tid, Rect::new(0.0, 0.0, 120.0, 24.0), "Low contrast", TextStyle { fills: vec![Paint::solid(muted)], ..ts });
        txt.name = "MutedLabel".into();
        doc.insert(page, txt).unwrap();

        // Both a current CONTRAST ignore and a legacy empty-property RECOMMEND_VARIABLES ignore
        // must not suppress hardcoded-value on fills/0, fontSize, or text.
        for rule in ["CONTRAST", "RECOMMEND_VARIABLES"] {
            doc.check_designs.ignored.clear();
            doc.check_designs.ignored.insert(crate::IgnoreKey { node: tid, rule: rule.into(), property: String::new() });

            let cfg = LintConfig::from_rule_list(&["hardcoded-value".into()]).unwrap();
            let report = lint(&doc, &cfg, &NoLintEnv);
            let props: Vec<&str> =
                report.findings.iter().filter(|f| f.node == tid && f.rule == "hardcoded-value").filter_map(|f| f.property.as_deref()).collect();
            assert!(props.contains(&"fills/0"), "rule={rule}: fills/0 must still be reported: {props:?}");
            assert!(props.contains(&"fontSize"), "rule={rule}: fontSize must still be reported: {props:?}");
            assert!(props.contains(&"text"), "rule={rule}: text style match must still be reported: {props:?}");
        }
    }

    #[test]
    fn lint_fix_skips_inherited_instance_mirrors_and_binds_text_style_fills() {
        let (mut doc, page) = new_doc();
        let blue = Color::rgb(0x25, 0x63, 0xEB);
        let coral = Color::rgb(0xF9, 0x73, 0x16);
        add_color_var(&mut doc, "brand", "Brand", "primary", "Color/Primary", &[("default", blue)]);
        add_color_var(&mut doc, "brand", "Brand", "accent", "Color/Accent", &[("default", coral)]);

        // Main component with a child rectangle (fill = blue) and a child text node (TextProps.style.fills = blue, Node.fills = empty).
        let comp_id = doc.alloc_id();
        let mut comp = Node::frame(comp_id, Rect::new(0.0, 0.0, 160.0, 60.0));
        comp.name = "Badge".into();
        comp.kind = NodeKind::Component(FrameProps::default());
        doc.insert(page, comp).unwrap();

        let comp_bg_id = doc.alloc_id();
        let mut comp_bg = Node::rectangle(comp_bg_id, Rect::new(0.0, 0.0, 160.0, 60.0));
        comp_bg.name = "Bg".into();
        comp_bg.fills = vec![Paint::solid(blue)];
        doc.insert(comp_id, comp_bg).unwrap();

        let comp_txt_id = doc.alloc_id();
        let mut comp_txt =
            Node::text(comp_txt_id, Rect::new(8.0, 8.0, 120.0, 24.0), "Label", TextStyle { fills: vec![Paint::solid(blue)], ..Default::default() });
        comp_txt.name = "Label".into();
        assert!(comp_txt.fills.is_empty(), "Node::text stores fills on TextProps.style.fills, leaving Node.fills empty");
        doc.insert(comp_id, comp_txt).unwrap();

        // Create two instances: inst1 leaves children inherited; inst2 overrides Bg fill to coral.
        let inst1_cmd = crate::create_instance(&mut doc, comp_id, page, Vec2::new(200.0, 0.0));
        inst1_cmd.apply(&mut doc).unwrap();
        let inst2_cmd = crate::create_instance(&mut doc, comp_id, page, Vec2::new(400.0, 0.0));
        inst2_cmd.apply(&mut doc).unwrap();

        let page_children = doc.get(page).unwrap().children.clone();
        let inst1_id = page_children[1];
        let inst2_id = page_children[2];
        let inst1_children = doc.get(inst1_id).unwrap().children.clone();
        let inst1_bg = inst1_children[0];
        let inst1_txt = inst1_children[1];
        let inst2_bg = doc.get(inst2_id).unwrap().children[0];
        {
            let n = doc.get_mut(inst2_bg).unwrap();
            n.fills = vec![Paint::solid(coral)];
            n.component.overrides.insert(crate::OverrideField::Fills);
        }

        let summary = crate::lint::lint_fix_document(&mut doc, Some(page), Some(&["hardcoded-value".into()]));
        assert_eq!(summary["fixed_count"], 3, "only main Bg, main Label, and overridden inst2 Bg should be bound directly: {summary}");

        // Main component Bg and Text are bound.
        assert_eq!(doc.get(comp_bg_id).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("primary"));
        assert_eq!(doc.get(comp_txt_id).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("primary"));

        // Un-overridden instance mirror nodes stay un-overridden and receive the synced binding from the main component,
        // so a subsequent lint pass produces 0 hardcoded-value findings.
        assert!(doc.get(inst1_bg).unwrap().component.overrides.is_empty(), "inherited instance mirror Bg must stay un-overridden");
        assert!(doc.get(inst1_txt).unwrap().component.overrides.is_empty(), "inherited instance mirror Label must stay un-overridden");
        assert_eq!(doc.get(inst1_bg).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("primary"));
        assert_eq!(doc.get(inst1_txt).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("primary"));

        // Overridden instance mirror node IS bound to its overridden colour's token.
        assert_eq!(doc.get(inst2_bg).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("accent"));

        let cfg = LintConfig::from_rule_list(&["hardcoded-value".into()]).unwrap();
        let after_report = crate::lint::lint(&doc, &cfg, &crate::lint::NoLintEnv);
        assert!(after_report.findings.is_empty(), "second audit after lint_fix must report 0 hardcoded-value findings: {:#?}", after_report.findings);
    }

    #[test]
    fn default_radius_and_font_scales_include_32_and_22_steps_through_96() {
        assert_eq!(RADIUS_SCALE.as_slice(), &[0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0, 32.0, 9999.0]);
        assert_eq!(FONT_SCALE.len(), 22);
        assert_eq!(*FONT_SCALE.last().unwrap(), 96.0);

        // A rectangle with corner radius 20 (or 10) in a file with no variables must produce no off-scale finding.
        let (mut doc, page) = new_doc();
        for r_val in [10.0, 20.0] {
            let rid = doc.alloc_id();
            let mut rect = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 60.0));
            rect.corner_radii = [r_val; 4];
            doc.insert(page, rect).unwrap();
        }
        let cfg = LintConfig::from_rule_list(&["off-scale".into()]).unwrap();
        let report = crate::lint::lint(&doc, &cfg, &crate::lint::NoLintEnv);
        assert!(report.findings.is_empty(), "radius 10 and 20 must be on default scale with no variables: {:#?}", report.findings);
    }
}
