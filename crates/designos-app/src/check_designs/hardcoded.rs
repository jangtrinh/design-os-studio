//! Hard-coded values rule: a raw fill, stroke, number property, text style or effect list that
//! equals an existing variable or shared style (in this file or an enabled library) but is not
//! linked to it.
//!
//! A variable wins over a style for paints and text (one finding per property). Styles are matched
//! on the whole list: a layer whose paints, text properties or effects equal a style's values
//! exactly.
//!
//! The token and style [`Index`] lives in [`designos_doc::lint::token_match`] and is shared with
//! `designos_lint` (`hardcoded-value`).

use super::{Finding, Fix, Rule};
pub(super) use designos_doc::lint::token_match::{
    bind_is_current, fmt_num, fmt_style_apply_label, fmt_style_quoted, fmt_var_bind_label, fmt_var_quoted, keys_to_bind, library_bind_is_current,
    library_style_is_current, name_list, number_inherited, number_value_at, same_number, style_is_current, style_name_list, text_style_inherited,
    variable_library_id, Index, Match, Slot,
};
use designos_doc::{Document, Node, NodeKind, PaintKind, Style};

fn finding(n: &Node, rule: Rule, property: impl Into<String>, message: String, fix: Fix, fix_label: String) -> Finding {
    Finding { node: n.id, node_name: n.name.clone(), rule, property: property.into(), message, fix: Some(fix), fix_label }
}

/// A finding that is reported but cannot be fixed without a choice only the user can make.
fn report_only(n: &Node, rule: Rule, property: impl Into<String>, message: String) -> Finding {
    Finding { node: n.id, node_name: n.name.clone(), rule, property: property.into(), message, fix: None, fix_label: String::new() }
}

fn rule_on(doc: &Document, rule: Rule) -> bool {
    !doc.check_designs.disabled_rules.contains(rule.figma_id())
}

pub(super) fn check(doc: &Document, index: &Index, n: &Node, out: &mut Vec<Finding>) {
    if matches!(n.kind, NodeKind::Unknown { .. }) {
        return;
    }
    let resolved = designos_doc::resolved_node(doc, n.id);
    let rn = resolved.as_deref().unwrap_or(n);
    let src_resolved = n.component.source.and_then(|s| designos_doc::resolved_node(doc, s));
    if n.component.source.is_some() && src_resolved.is_none() {
        return;
    }
    let src = src_resolved.as_deref();
    if rule_on(doc, Rule::HardCodedValue) {
        paints_rule(doc, index, rn, src, Slot::Fill, out);
        if rn.stroke.weight > 0.0 {
            paints_rule(doc, index, rn, src, Slot::Stroke, out);
        }
    }
    numbers_rule(doc, index, rn, src, out);
    if rule_on(doc, Rule::TypographyAndStroke) {
        check_typography_numbers(doc, index, rn, src, out);
    }
    if rule_on(doc, Rule::TextStyles) {
        super::text_styles::check(index, rn, src, out);
    }
    if rule_on(doc, Rule::HardCodedValue) {
        effects_rule(index, rn, src, out);
    }
}

#[allow(clippy::too_many_arguments)]
fn check_number_prop(
    doc: &Document,
    index: &Index,
    n: &Node,
    rule: Rule,
    prop_key: &str,
    keys: &[&str],
    label: &str,
    val: f32,
    emit: bool,
    out: &mut Vec<Finding>,
) -> bool {
    let Some(&first_key) = keys.first() else { return false };
    match index.find_number(doc, n, first_key, val) {
        Match::None => false,
        Match::One(e) => {
            if emit {
                let message = format!("{label} {} matches variable {}", fmt_num(val), fmt_var_quoted(e));
                let fix = if keys.len() == 1 {
                    Fix::BindVariable { key: first_key.into(), variable: e.fix_id.clone() }
                } else {
                    Fix::BindVariables { keys: keys.iter().map(|k| (*k).to_string()).collect(), variable: e.fix_id.clone() }
                };
                out.push(finding(n, rule, prop_key, message, fix, fmt_var_bind_label(e)));
            }
            true
        }
        Match::Ambiguous(vars) => {
            if emit {
                let message = format!("{label} {} matches {} variables: {}", fmt_num(val), vars.len(), name_list(&vars));
                out.push(report_only(n, rule, prop_key, message));
            }
            true
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_single_number(
    doc: &Document,
    index: &Index,
    n: &Node,
    src: Option<&Node>,
    rule: Rule,
    key: &str,
    label: &str,
    emit: bool,
    out: &mut Vec<Finding>,
) -> bool {
    if n.variables.bindings.contains_key(key) || doc.effective_binding(n.id, key).is_some() || number_inherited(n, src, key) {
        return false;
    }
    let Some(val) = number_value_at(n, key) else { return false };
    check_number_prop(doc, index, n, rule, key, &[key], label, val, emit, out)
}

#[allow(clippy::too_many_arguments)]
fn check_quad(
    doc: &Document,
    index: &Index,
    n: &Node,
    src: Option<&Node>,
    rule: Rule,
    uniform_key: &str,
    uniform_label: &str,
    sides: [(&str, &str); 4],
    out: &mut Vec<Finding>,
) {
    if !rule_on(doc, rule) || sides.iter().all(|(k, _)| number_inherited(n, src, k)) {
        return;
    }
    let all_unbound = sides.iter().all(|(k, _)| !n.variables.bindings.contains_key(*k));
    if all_unbound {
        if let Some(v0) = number_value_at(n, sides[0].0) {
            let all_equal = sides[1..].iter().all(|(k, _)| number_value_at(n, k).is_some_and(|v| same_number(v, v0)));
            if all_equal {
                let keys = [sides[0].0, sides[1].0, sides[2].0, sides[3].0];
                check_number_prop(doc, index, n, rule, uniform_key, &keys, uniform_label, v0, true, out);
                return;
            }
        }
    }
    for (k, label) in sides {
        check_single_number(doc, index, n, src, rule, k, label, true, out);
    }
}

/// Spec 158 section 3.1: the gap of a horizontal stack is a horizontal spacing field, the gap of a
/// vertical stack and every `counterAxisSpacing` (the row gap) are vertical ones.
fn spacing_rule_for(n: &Node, key: &str) -> Rule {
    let dir = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).map(|al| al.direction).unwrap_or(designos_doc::LayoutDirection::Horizontal);
    match (dir, key) {
        (_, "counterAxisSpacing") | (designos_doc::LayoutDirection::Vertical, "itemSpacing") => Rule::VerticalSpacing,
        _ => Rule::HorizontalSpacing,
    }
}

fn numbers_rule(doc: &Document, index: &Index, n: &Node, src: Option<&Node>, out: &mut Vec<Finding>) {
    if index.by_num.is_empty() {
        return;
    }
    let gap_rule = spacing_rule_for(n, "itemSpacing");
    if rule_on(doc, gap_rule) {
        check_single_number(doc, index, n, src, gap_rule, "itemSpacing", "Gap", true, out);
    }
    let rgap_rule = spacing_rule_for(n, "counterAxisSpacing");
    if rule_on(doc, rgap_rule) {
        check_single_number(doc, index, n, src, rgap_rule, "counterAxisSpacing", "Row gap", true, out);
    }
    check_quad(
        doc,
        index,
        n,
        src,
        Rule::Padding,
        "padding",
        "Padding",
        [("paddingTop", "Top padding"), ("paddingRight", "Right padding"), ("paddingBottom", "Bottom padding"), ("paddingLeft", "Left padding")],
        out,
    );
    check_quad(
        doc,
        index,
        n,
        src,
        Rule::CornerRadius,
        "cornerRadius",
        "Corner radius",
        [
            ("topLeftRadius", "Top-left radius"),
            ("topRightRadius", "Top-right radius"),
            ("bottomRightRadius", "Bottom-right radius"),
            ("bottomLeftRadius", "Bottom-left radius"),
        ],
        out,
    );
    if rule_on(doc, Rule::TypographyAndStroke) {
        check_single_number(doc, index, n, src, Rule::TypographyAndStroke, "strokeWeight", "Stroke weight", true, out);
        for (k, label) in [
            ("strokeTopWeight", "Stroke top weight"),
            ("strokeRightWeight", "Stroke right weight"),
            ("strokeBottomWeight", "Stroke bottom weight"),
            ("strokeLeftWeight", "Stroke left weight"),
        ] {
            check_single_number(doc, index, n, src, Rule::TypographyAndStroke, k, label, true, out);
        }
    }
}

fn paints_rule(doc: &Document, index: &Index, n: &Node, src: Option<&Node>, slot: Slot, out: &mut Vec<Finding>) {
    let paints = slot.paints(n);
    if paints.is_empty() || n.style_refs.contains_key(slot.ref_key()) {
        return;
    }
    if let Some(s) = src {
        let same_paints = slot.paints(s) == paints && s.style_refs.get(slot.ref_key()) == n.style_refs.get(slot.ref_key());
        let same_bindings = (0..paints.len()).all(|i| {
            let key = format!("{}/{i}", slot.prefix());
            s.variables.bindings.get(&key) == n.variables.bindings.get(&key)
        });
        if same_paints && same_bindings {
            return;
        }
    }
    let mut matched_variable = false;
    for (i, p) in paints.iter().enumerate() {
        let PaintKind::Solid(c) = &p.kind else { continue };
        let key = format!("{}/{i}", slot.prefix());
        if !p.visible || n.variables.bindings.contains_key(&key) {
            continue;
        }
        if src.is_some_and(|s| slot.paints(s).get(i) == Some(p) && s.variables.bindings.get(&key) == n.variables.bindings.get(&key)) {
            continue;
        }
        match index.find(doc, n, &key, c) {
            Match::None => {}
            Match::One(e) => {
                matched_variable = true;
                let message = format!("{} #{} matches variable {}", slot.label(), c.to_hex(), fmt_var_quoted(e));
                out.push(finding(n, Rule::HardCodedValue, key.clone(), message, Fix::BindVariable { key, variable: e.fix_id.clone() }, fmt_var_bind_label(e)));
            }
            Match::Ambiguous(vars) => {
                matched_variable = true;
                let message = format!("{} #{} matches {} variables: {}", slot.label(), c.to_hex(), vars.len(), name_list(&vars));
                out.push(report_only(n, Rule::HardCodedValue, key, message));
            }
        }
    }
    // A style replaces the whole list, so it is only suggested when no paint of the layer is bound.
    let any_bound = n.variables.bindings.keys().any(|k| k.starts_with(slot.prefix()));
    if matched_variable || any_bound {
        return;
    }
    let style = index.styles.iter().find(|s| matches!(&s.style, Style::Fill(ps) if ps == paints) && s.property == slot.property());
    if let Some(s) = style {
        out.push(finding(
            n,
            Rule::HardCodedValue,
            slot.ref_key(),
            format!("{} matches style {}", slot.label(), fmt_style_quoted(s)),
            Fix::ApplyStyle { style: s.fix_key.clone() },
            fmt_style_apply_label(s),
        ));
    }
}

fn check_typography_numbers(doc: &Document, index: &Index, n: &Node, src: Option<&Node>, out: &mut Vec<Finding>) {
    let NodeKind::Text(_) = &n.kind else { return };
    if n.style_refs.contains_key("text") || index.by_num.is_empty() || text_style_inherited(n, src) {
        return;
    }
    check_single_number(doc, index, n, src, Rule::TypographyAndStroke, "fontSize", "Font size", true, out);
    check_single_number(doc, index, n, src, Rule::TypographyAndStroke, "lineHeight", "Line height", true, out);
}

fn effects_rule(index: &Index, n: &Node, src: Option<&Node>, out: &mut Vec<Finding>) {
    if n.effects.is_empty() || n.style_refs.contains_key("effect") {
        return;
    }
    if src.is_some_and(|s| s.effects == n.effects) {
        return;
    }
    let style = index.styles.iter().find(|s| matches!(&s.style, Style::Effect(fx) if *fx == n.effects));
    if let Some(s) = style {
        out.push(finding(
            n,
            Rule::HardCodedValue,
            "effect",
            format!("Effects match style {}", fmt_style_quoted(s)),
            Fix::ApplyStyle { style: s.fix_key.clone() },
            fmt_style_apply_label(s),
        ));
    }
}
