//! Hard-coded values rule: a raw fill, stroke, number property, text style or effect list that
//! equals an existing variable or shared style but is not linked to it.
//!
//! A variable wins over a style for paints and text (one finding per property). Styles are matched
//! on the whole list: a layer whose paints, text properties or effects equal a style's values
//! exactly.

use super::{Finding, Fix, Rule};
use crate::styles::rgb_key;
use designos_doc::{
    scope_allows, style_property, Color, Document, LayoutAlign, LineHeight, Node, NodeId, NodeKind, Paint, PaintKind, Style, StyleProperty, TextStyle,
    Variable, VariableKind, VariableStore, VariableValue,
};
use std::collections::HashMap;

/// Alpha tolerance when comparing colours: half an 8-bit step.
const ALPHA_EPS: f32 = 0.002;
/// Tolerance when comparing exact numbers (0.0005).
const NUM_EPS: f32 = 0.0005;

const PADDING_KEYS: [&str; 4] = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
const RADIUS_KEYS: [&str; 4] = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];

/// A variable and whether it resolves to one and the same value in every mode.
struct Entry<'a> {
    var: &'a Variable,
    constant: bool,
}

/// What [`Index::find`] or [`Index::find_number`] concluded for one property.
pub(super) enum Match<'a> {
    None,
    /// Exactly one variable is the right one.
    One(&'a Variable),
    /// Several variables fit equally well, so none may be picked for the user.
    Ambiguous(Vec<&'a Variable>),
}

/// Colour and number variables by the value they resolve to in any mode of their collection.
pub(super) struct Index<'a> {
    by_rgb: HashMap<u32, Vec<Entry<'a>>>,
    by_num: HashMap<i64, Vec<Entry<'a>>>,
}

impl<'a> Index<'a> {
    pub(super) fn new(doc: &'a Document) -> Self {
        let store = VariableStore::from_doc(doc);
        let mut by_rgb: HashMap<u32, Vec<Entry>> = HashMap::new();
        let mut by_num: HashMap<i64, Vec<Entry>> = HashMap::new();
        for v in doc.variables.values() {
            let Some(coll) = doc.variable_collections.get(&v.collection) else { continue };
            match v.kind {
                VariableKind::Color => {
                    let values: Vec<Option<Color>> = coll
                        .modes
                        .iter()
                        .map(|mode| match store.resolve(&v.id, &mode.id) {
                            Some(VariableValue::Color(c)) => Some(c),
                            _ => None,
                        })
                        .collect();
                    let constant = !values.is_empty() && values.iter().all(|c| matches!((c, &values[0]), (Some(a), Some(b)) if same_colour(a, b)));
                    let mut seen = Vec::new();
                    for c in values.iter().flatten() {
                        let key = rgb_key(c);
                        if !seen.contains(&key) {
                            seen.push(key);
                            by_rgb.entry(key).or_default().push(Entry { var: v, constant });
                        }
                    }
                }
                VariableKind::Number => {
                    let values: Vec<Option<f32>> = coll
                        .modes
                        .iter()
                        .map(|mode| match store.resolve(&v.id, &mode.id) {
                            Some(VariableValue::Number(x)) if x.is_finite() => Some(x),
                            _ => None,
                        })
                        .collect();
                    let constant = !values.is_empty() && values.iter().all(|x| matches!((x, &values[0]), (Some(a), Some(b)) if same_number(*a, *b)));
                    let mut seen = Vec::new();
                    for x in values.iter().flatten() {
                        let key = num_key(*x);
                        if !seen.contains(&key) {
                            seen.push(key);
                            by_num.entry(key).or_default().push(Entry { var: v, constant });
                        }
                    }
                }
                _ => {}
            }
        }
        for list in by_rgb.values_mut() {
            list.sort_by(|a, b| (&a.var.name, &a.var.id).cmp(&(&b.var.name, &b.var.id)));
        }
        for list in by_num.values_mut() {
            list.sort_by(|a, b| (&a.var.name, &a.var.id).cmp(&(&b.var.name, &b.var.id)));
        }
        Index { by_rgb, by_num }
    }

    /// The variable that holds `c` at `node` for property `key`. A variable with the same value in
    /// every mode is preferred over one that only matches in the mode in effect here, since the
    /// latter would change colour when the mode switches. Several equally good ones are ambiguous.
    fn find(&self, doc: &Document, node: &Node, key: &str, c: &Color) -> Match<'a> {
        let Some(list) = self.by_rgb.get(&rgb_key(c)) else { return Match::None };
        let holding: Vec<&Entry> = list.iter().filter(|e| variable_holds(doc, node, key, e.var, c)).collect();
        let steady: Vec<&Entry> = holding.iter().copied().filter(|e| e.constant).collect();
        let pool = if steady.is_empty() { holding } else { steady };
        match pool.as_slice() {
            [] => Match::None,
            [only] => Match::One(only.var),
            many => Match::Ambiguous(many.iter().map(|e| e.var).collect()),
        }
    }

    /// The number variable that holds `x` at `node` for property `key`.
    fn find_number(&self, doc: &Document, node: &Node, key: &str, x: f32) -> Match<'a> {
        let base = num_key(x);
        let mut seen_ids: Vec<&str> = Vec::new();
        let mut holding: Vec<&Entry> = Vec::new();
        for bucket in [base - 1, base, base + 1] {
            if let Some(list) = self.by_num.get(&bucket) {
                for e in list {
                    if !seen_ids.contains(&e.var.id.as_str()) && number_variable_holds(doc, node, key, e.var, x) {
                        seen_ids.push(e.var.id.as_str());
                        holding.push(e);
                    }
                }
            }
        }
        holding.sort_by(|a, b| (&a.var.name, &a.var.id).cmp(&(&b.var.name, &b.var.id)));
        let steady: Vec<&Entry> = holding.iter().copied().filter(|e| e.constant).collect();
        let pool = if steady.is_empty() { holding } else { steady };
        match pool.as_slice() {
            [] => Match::None,
            [only] => Match::One(only.var),
            many => Match::Ambiguous(many.iter().map(|e| e.var).collect()),
        }
    }
}

/// Same colour to the 8-bit step, and the same alpha within [`ALPHA_EPS`].
fn same_colour(a: &Color, b: &Color) -> bool {
    rgb_key(a) == rgb_key(b) && (a.a - b.a).abs() <= ALPHA_EPS
}

fn num_key(x: f32) -> i64 {
    (x as f64 * 1000.0).round() as i64
}

fn same_number(a: f32, b: f32) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= NUM_EPS
}

fn fmt_num(x: f32) -> String {
    if (x - x.round()).abs() < NUM_EPS {
        format!("{}", x.round() as i64)
    } else {
        let r = (x * 1000.0).round() / 1000.0;
        format!("{r}")
    }
}

fn variable_holds(doc: &Document, node: &Node, key: &str, v: &Variable, c: &Color) -> bool {
    scope_allows(&v.scopes, key, &node.kind) && matches!(doc.resolve_variable(node.id, &v.id), Some(VariableValue::Color(vc)) if same_colour(&vc, c))
}

fn number_variable_holds(doc: &Document, node: &Node, key: &str, v: &Variable, x: f32) -> bool {
    let scope_key = match key {
        "padding" => "paddingTop",
        "cornerRadius" => "topLeftRadius",
        other => other,
    };
    scope_allows(&v.scopes, scope_key, &node.kind) && matches!(doc.resolve_variable(node.id, &v.id), Some(VariableValue::Number(vx)) if same_number(vx, x))
}

/// The paint list a slot reads from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Fill,
    Stroke,
}

impl Slot {
    fn prefix(self) -> &'static str {
        match self {
            Slot::Fill => "fills",
            Slot::Stroke => "strokes",
        }
    }

    fn ref_key(self) -> &'static str {
        match self {
            Slot::Fill => "fill",
            Slot::Stroke => "stroke",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Slot::Fill => "Fill",
            Slot::Stroke => "Stroke",
        }
    }

    fn property(self) -> StyleProperty {
        match self {
            Slot::Fill => StyleProperty::Fill,
            Slot::Stroke => StyleProperty::Stroke,
        }
    }

    fn paints(self, n: &Node) -> &[Paint] {
        match (self, &n.kind) {
            (Slot::Fill, NodeKind::Text(t)) => &t.style.fills,
            (Slot::Fill, _) => &n.fills,
            (Slot::Stroke, _) => &n.stroke.paints,
        }
    }
}

fn finding(n: &Node, message: String, fix: Fix, fix_label: String) -> Finding {
    Finding { node: n.id, node_name: n.name.clone(), rule: Rule::HardCodedValue, message, fix: Some(fix), fix_label }
}

/// A finding that is reported but cannot be fixed without a choice only the user can make.
fn report_only(n: &Node, message: String) -> Finding {
    Finding { node: n.id, node_name: n.name.clone(), rule: Rule::HardCodedValue, message, fix: None, fix_label: String::new() }
}

/// `"A", "B"` for up to three variable names, then an ellipsis.
fn name_list(vars: &[&Variable]) -> String {
    let mut names: Vec<String> = vars.iter().take(3).map(|v| format!("\"{}\"", v.name)).collect();
    if vars.len() > 3 {
        names.push("…".to_string());
    }
    names.join(", ")
}

pub(super) fn check(doc: &Document, index: &Index, n: &Node, out: &mut Vec<Finding>) {
    if matches!(n.kind, NodeKind::Unknown { .. }) {
        return;
    }
    paints_rule(doc, index, n, Slot::Fill, out);
    if n.stroke.weight > 0.0 {
        paints_rule(doc, index, n, Slot::Stroke, out);
    }
    numbers_rule(doc, index, n, out);
    text_rule(doc, index, n, out);
    effects_rule(doc, n, out);
}

fn check_number_prop(doc: &Document, index: &Index, n: &Node, keys: &[&str], label: &str, val: f32, out: &mut Vec<Finding>) -> bool {
    let Some(&first_key) = keys.first() else { return false };
    match index.find_number(doc, n, first_key, val) {
        Match::None => false,
        Match::One(v) => {
            let message = format!("{label} {} matches variable \"{}\"", fmt_num(val), v.name);
            let fix = if keys.len() == 1 {
                Fix::BindVariable { key: first_key.into(), variable: v.id.clone() }
            } else {
                Fix::BindVariables { keys: keys.iter().map(|k| (*k).to_string()).collect(), variable: v.id.clone() }
            };
            out.push(finding(n, message, fix, format!("Bind to variable {}", v.name)));
            true
        }
        Match::Ambiguous(vars) => {
            let message = format!("{label} {} matches {} variables: {}", fmt_num(val), vars.len(), name_list(&vars));
            out.push(report_only(n, message));
            true
        }
    }
}

/// Raw positive numeric value on `n` for binding `key`, when that property applies to `n`.
fn number_value_at(n: &Node, key: &str) -> Option<f32> {
    let v = match key {
        "itemSpacing" => {
            let al = n.kind.frame_props()?.auto_layout.as_ref()?;
            if matches!(al.primary_align, LayoutAlign::SpaceBetween | LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly) {
                return None;
            }
            al.item_spacing
        }
        "counterAxisSpacing" => {
            let al = n.kind.frame_props()?.auto_layout.as_ref()?;
            al.counter_spacing
        }
        "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" => {
            let al = n.kind.frame_props()?.auto_layout.as_ref()?;
            let i = match key {
                "paddingTop" => 0,
                "paddingRight" => 1,
                "paddingBottom" => 2,
                _ => 3,
            };
            al.padding[i]
        }
        "topLeftRadius" | "topRightRadius" | "bottomRightRadius" | "bottomLeftRadius" => {
            if matches!(n.kind, NodeKind::Text(_) | NodeKind::Line | NodeKind::Group | NodeKind::Vector { .. } | NodeKind::Connector { .. }) {
                return None;
            }
            let i = match key {
                "topLeftRadius" => 0,
                "topRightRadius" => 1,
                "bottomRightRadius" => 2,
                _ => 3,
            };
            n.corner_radii[i]
        }
        "strokeWeight" => {
            if n.stroke.side_weights.is_some() || !n.stroke.paints.iter().any(|p| p.visible) {
                return None;
            }
            n.stroke.weight
        }
        "strokeTopWeight" | "strokeRightWeight" | "strokeBottomWeight" | "strokeLeftWeight" => {
            if !n.stroke.paints.iter().any(|p| p.visible) {
                return None;
            }
            let sides = n.stroke.side_weights?;
            let i = match key {
                "strokeTopWeight" => 0,
                "strokeRightWeight" => 1,
                "strokeBottomWeight" => 2,
                _ => 3,
            };
            sides[i]
        }
        "fontSize" => {
            let NodeKind::Text(t) = &n.kind else { return None };
            if n.style_refs.contains_key("text") || !t.runs.iter().all(|r| same_number(r.style.font_size, t.style.font_size)) {
                return None;
            }
            t.style.font_size
        }
        "lineHeight" => {
            let NodeKind::Text(t) = &n.kind else { return None };
            let LineHeight::Pixels(lh) = t.style.line_height else { return None };
            if n.style_refs.contains_key("text") || !t.runs.iter().all(|r| matches!(r.style.line_height, LineHeight::Pixels(rlh) if same_number(rlh, lh))) {
                return None;
            }
            lh
        }
        _ => return None,
    };
    (v.is_finite() && v > 0.0).then_some(v)
}

fn check_single_number(doc: &Document, index: &Index, n: &Node, key: &str, label: &str, out: &mut Vec<Finding>) -> bool {
    if n.variables.bindings.contains_key(key) {
        return false;
    }
    let Some(val) = number_value_at(n, key) else { return false };
    check_number_prop(doc, index, n, &[key], label, val, out)
}

fn check_quad(doc: &Document, index: &Index, n: &Node, uniform_label: &str, sides: [(&str, &str); 4], out: &mut Vec<Finding>) {
    let all_unbound = sides.iter().all(|(k, _)| !n.variables.bindings.contains_key(*k));
    if all_unbound {
        if let Some(v0) = number_value_at(n, sides[0].0) {
            if sides[1..].iter().all(|(k, _)| number_value_at(n, k).is_some_and(|v| same_number(v, v0))) {
                let keys = [sides[0].0, sides[1].0, sides[2].0, sides[3].0];
                check_number_prop(doc, index, n, &keys, uniform_label, v0, out);
                return;
            }
        }
    }
    for (k, label) in sides {
        check_single_number(doc, index, n, k, label, out);
    }
}

fn numbers_rule(doc: &Document, index: &Index, n: &Node, out: &mut Vec<Finding>) {
    if index.by_num.is_empty() {
        return;
    }
    check_single_number(doc, index, n, "itemSpacing", "Gap", out);
    check_single_number(doc, index, n, "counterAxisSpacing", "Row gap", out);
    check_quad(
        doc,
        index,
        n,
        "Padding",
        [("paddingTop", "Top padding"), ("paddingRight", "Right padding"), ("paddingBottom", "Bottom padding"), ("paddingLeft", "Left padding")],
        out,
    );
    check_quad(
        doc,
        index,
        n,
        "Corner radius",
        [
            ("topLeftRadius", "Top-left radius"),
            ("topRightRadius", "Top-right radius"),
            ("bottomRightRadius", "Bottom-right radius"),
            ("bottomLeftRadius", "Bottom-left radius"),
        ],
        out,
    );
    check_single_number(doc, index, n, "strokeWeight", "Stroke weight", out);
    for (k, label) in [
        ("strokeTopWeight", "Stroke top weight"),
        ("strokeRightWeight", "Stroke right weight"),
        ("strokeBottomWeight", "Stroke bottom weight"),
        ("strokeLeftWeight", "Stroke left weight"),
    ] {
        check_single_number(doc, index, n, k, label, out);
    }
}

fn paints_rule(doc: &Document, index: &Index, n: &Node, slot: Slot, out: &mut Vec<Finding>) {
    let paints = slot.paints(n);
    if paints.is_empty() || n.style_refs.contains_key(slot.ref_key()) {
        return;
    }
    let mut matched_variable = false;
    for (i, p) in paints.iter().enumerate() {
        let PaintKind::Solid(c) = &p.kind else { continue };
        let key = format!("{}/{i}", slot.prefix());
        if !p.visible || n.variables.bindings.contains_key(&key) {
            continue;
        }
        match index.find(doc, n, &key, c) {
            Match::None => {}
            Match::One(v) => {
                matched_variable = true;
                let message = format!("{} #{} matches variable \"{}\"", slot.label(), c.to_hex(), v.name);
                out.push(finding(n, message, Fix::BindVariable { key, variable: v.id.clone() }, format!("Bind to variable {}", v.name)));
            }
            Match::Ambiguous(vars) => {
                matched_variable = true;
                let message = format!("{} #{} matches {} variables: {}", slot.label(), c.to_hex(), vars.len(), name_list(&vars));
                out.push(report_only(n, message));
            }
        }
    }
    // A style replaces the whole list, so it is only suggested when no paint of the layer is bound.
    let any_bound = n.variables.bindings.keys().any(|k| k.starts_with(slot.prefix()));
    if matched_variable || any_bound {
        return;
    }
    let style = doc.styles.iter().find(|(k, s)| matches!(s, Style::Fill(ps) if ps == paints) && style_property(doc, k) == slot.property());
    if let Some((key, _)) = style {
        let name = doc.style_name(key);
        out.push(finding(n, format!("{} matches style \"{name}\"", slot.label()), Fix::ApplyStyle { style: key.clone() }, format!("Apply style {name}")));
    }
}

/// Text properties equal a text style's apart from the colour, which a text style does not carry.
fn same_text_style(own: &TextStyle, style: &TextStyle) -> bool {
    TextStyle { fills: own.fills.clone(), ..style.clone() } == *own
}

fn has_bound_text_number(n: &Node) -> bool {
    n.variables.bindings.keys().any(|k| matches!(k.as_str(), "fontSize" | "lineHeight" | "fontWeight" | "letterSpacing"))
}

fn text_rule(doc: &Document, index: &Index, n: &Node, out: &mut Vec<Finding>) {
    let NodeKind::Text(t) = &n.kind else { return };
    if n.style_refs.contains_key("text") {
        return;
    }
    let mut matched_variable = false;
    if !index.by_num.is_empty() {
        matched_variable |= check_single_number(doc, index, n, "fontSize", "Font size", out);
        matched_variable |= check_single_number(doc, index, n, "lineHeight", "Line height", out);
    }
    if matched_variable || has_bound_text_number(n) {
        return;
    }
    let style =
        doc.styles.iter().find(|(_, s)| matches!(s, Style::Text(ts) if same_text_style(&t.style, ts) && t.runs.iter().all(|r| same_text_style(&r.style, ts))));
    if let Some((key, _)) = style {
        let name = doc.style_name(key);
        out.push(finding(n, format!("Text matches style \"{name}\""), Fix::ApplyStyle { style: key.clone() }, format!("Apply style {name}")));
    }
}

fn effects_rule(doc: &Document, n: &Node, out: &mut Vec<Finding>) {
    if n.effects.is_empty() || n.style_refs.contains_key("effect") {
        return;
    }
    let style = doc.styles.iter().find(|(_, s)| matches!(s, Style::Effect(fx) if *fx == n.effects));
    if let Some((key, _)) = style {
        let name = doc.style_name(key);
        out.push(finding(n, format!("Effects match style \"{name}\""), Fix::ApplyStyle { style: key.clone() }, format!("Apply style {name}")));
    }
}

/// Concrete `NodeVariables::bindings` keys to write when applying a [`Fix::BindVariable`], or an
/// empty list when the finding is no longer current.
pub(super) fn keys_to_bind<'a>(doc: &Document, id: NodeId, key: &'a str, variable: &str) -> Vec<&'a str> {
    match key {
        "padding" => {
            if PADDING_KEYS.iter().all(|k| bind_is_current(doc, id, k, variable)) {
                PADDING_KEYS.to_vec()
            } else {
                Vec::new()
            }
        }
        "cornerRadius" => {
            if RADIUS_KEYS.iter().all(|k| bind_is_current(doc, id, k, variable)) {
                RADIUS_KEYS.to_vec()
            } else {
                Vec::new()
            }
        }
        other => {
            if bind_is_current(doc, id, other, variable) {
                vec![other]
            } else {
                Vec::new()
            }
        }
    }
}

/// Whether binding `variable` to `key` on `id` is still a pure re-link: the property is unbound and
/// its raw value already equals what the variable resolves to here.
pub(super) fn bind_is_current(doc: &Document, id: NodeId, key: &str, variable: &str) -> bool {
    if key == "padding" {
        return PADDING_KEYS.iter().all(|k| bind_is_current(doc, id, k, variable));
    }
    if key == "cornerRadius" {
        return RADIUS_KEYS.iter().all(|k| bind_is_current(doc, id, k, variable));
    }
    let (Some(n), Some(v)) = (doc.get(id), doc.variables.get(variable)) else { return false };
    if let Some((slot_name, rest)) = key.split_once('/') {
        let slot = match slot_name {
            "fills" => Slot::Fill,
            "strokes" => Slot::Stroke,
            _ => return false,
        };
        let Some(p) = rest.parse::<usize>().ok().and_then(|i| slot.paints(n).get(i)) else { return false };
        let PaintKind::Solid(c) = &p.kind else { return false };
        return p.visible && !n.variables.bindings.contains_key(key) && !n.style_refs.contains_key(slot.ref_key()) && variable_holds(doc, n, key, v, c);
    }
    let Some(val) = number_value_at(n, key) else { return false };
    !n.variables.bindings.contains_key(key) && number_variable_holds(doc, n, key, v, val)
}

/// Whether applying `style` to `id` is still a pure re-link: the layer is not linked yet and its
/// own values already equal the style's.
pub(super) fn style_is_current(doc: &Document, id: NodeId, style: &str) -> bool {
    let (Some(n), Some(s)) = (doc.get(id), doc.styles.get(style)) else { return false };
    match s {
        Style::Fill(ps) => {
            let slot = if style_property(doc, style) == StyleProperty::Stroke { Slot::Stroke } else { Slot::Fill };
            let any_bound = n.variables.bindings.keys().any(|k| k.starts_with(slot.prefix()));
            !n.style_refs.contains_key(slot.ref_key()) && !any_bound && slot.paints(n) == ps.as_slice()
        }
        Style::Text(ts) => match &n.kind {
            NodeKind::Text(t) => {
                !n.style_refs.contains_key("text")
                    && !has_bound_text_number(n)
                    && same_text_style(&t.style, ts)
                    && t.runs.iter().all(|r| same_text_style(&r.style, ts))
            }
            _ => false,
        },
        Style::Effect(fx) => !n.style_refs.contains_key("effect") && !n.effects.is_empty() && n.effects == *fx,
        Style::Grid(_) => false,
    }
}
