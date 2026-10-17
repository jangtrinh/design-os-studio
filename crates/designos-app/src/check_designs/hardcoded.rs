//! Hard-coded values rule: a raw fill, stroke, text style or effect list that equals an existing
//! variable or shared style but is not linked to it.
//!
//! A variable wins over a style for paints (one finding per paint). Styles are matched on the whole
//! list: a layer whose paints, text properties or effects equal a style's values exactly.

use super::{Finding, Fix, Rule};
use crate::styles::rgb_key;
use designos_doc::{
    scope_allows, style_property, Color, Document, Node, NodeId, NodeKind, Paint, PaintKind, Style, StyleProperty, TextStyle, Variable, VariableKind,
    VariableStore, VariableValue,
};
use std::collections::HashMap;

/// Alpha tolerance when comparing colours: half an 8-bit step.
const ALPHA_EPS: f32 = 0.002;

/// A colour variable and whether it resolves to one and the same colour in every mode.
struct Entry<'a> {
    var: &'a Variable,
    constant: bool,
}

/// What [`Index::find`] concluded for one paint.
pub(super) enum Match<'a> {
    None,
    /// Exactly one variable is the right one.
    One(&'a Variable),
    /// Several variables fit equally well, so none may be picked for the user.
    Ambiguous(Vec<&'a Variable>),
}

/// Colour variables by the 8-bit RGB value they resolve to in any mode of their collection.
pub(super) struct Index<'a> {
    by_rgb: HashMap<u32, Vec<Entry<'a>>>,
}

impl<'a> Index<'a> {
    pub(super) fn new(doc: &'a Document) -> Self {
        let store = VariableStore::from_doc(doc);
        let mut by_rgb: HashMap<u32, Vec<Entry>> = HashMap::new();
        for v in doc.variables.values().filter(|v| v.kind == VariableKind::Color) {
            let Some(coll) = doc.variable_collections.get(&v.collection) else { continue };
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
        for list in by_rgb.values_mut() {
            list.sort_by(|a, b| (&a.var.name, &a.var.id).cmp(&(&b.var.name, &b.var.id)));
        }
        Index { by_rgb }
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
}

/// Same colour to the 8-bit step, and the same alpha within [`ALPHA_EPS`].
fn same_colour(a: &Color, b: &Color) -> bool {
    rgb_key(a) == rgb_key(b) && (a.a - b.a).abs() <= ALPHA_EPS
}

fn variable_holds(doc: &Document, node: &Node, key: &str, v: &Variable, c: &Color) -> bool {
    scope_allows(&v.scopes, key, &node.kind) && matches!(doc.resolve_variable(node.id, &v.id), Some(VariableValue::Color(vc)) if same_colour(&vc, c))
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
    text_rule(doc, n, out);
    effects_rule(doc, n, out);
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

fn text_rule(doc: &Document, n: &Node, out: &mut Vec<Finding>) {
    let NodeKind::Text(t) = &n.kind else { return };
    if n.style_refs.contains_key("text") {
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

/// Whether binding `variable` to `key` on `id` is still a pure re-link: the paint is an unbound
/// visible solid whose colour already equals what the variable resolves to here.
pub(super) fn bind_is_current(doc: &Document, id: NodeId, key: &str, variable: &str) -> bool {
    let (Some(n), Some(v)) = (doc.get(id), doc.variables.get(variable)) else { return false };
    let (slot, rest) = match key.split_once('/') {
        Some(("fills", rest)) => (Slot::Fill, rest),
        Some(("strokes", rest)) => (Slot::Stroke, rest),
        _ => return false,
    };
    let Some(p) = rest.parse::<usize>().ok().and_then(|i| slot.paints(n).get(i)) else { return false };
    let PaintKind::Solid(c) = &p.kind else { return false };
    p.visible && !n.variables.bindings.contains_key(key) && !n.style_refs.contains_key(slot.ref_key()) && variable_holds(doc, n, key, v, c)
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
            NodeKind::Text(t) => !n.style_refs.contains_key("text") && same_text_style(&t.style, ts) && t.runs.iter().all(|r| same_text_style(&r.style, ts)),
            _ => false,
        },
        Style::Effect(fx) => !n.style_refs.contains_key("effect") && !n.effects.is_empty() && n.effects == *fx,
        Style::Grid(_) => false,
    }
}
