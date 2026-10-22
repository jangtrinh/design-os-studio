//! Pure data behind the Inspect panel and MCP design context: box model, variables used,
//! component properties and the values pinned by annotations.

use crate::util::fmt_num;
use designos_doc::*;

// ------------------------------------------------------------------ box model

/// Padding, size and the space to neighbouring siblings ("margin") of a layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxModel {
    pub size: (f32, f32),
    /// top, right, bottom, left (own auto layout padding)
    pub padding: [f32; 4],
    /// top, right, bottom, left: space to the nearest sibling in that direction
    pub margin: [f32; 4],
}

pub fn box_model(doc: &Document, id: NodeId) -> Option<BoxModel> {
    let n = doc.get(id)?;
    let padding = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|a| a.padding).unwrap_or([0.0; 4]);
    let mut margin = [0.0f32; 4];
    if let Some(parent) = n.parent.and_then(|p| doc.get(p)).filter(|p| !matches!(p.kind, NodeKind::Page { .. })) {
        let rect = |m: &Node| (m.transform.tx, m.transform.ty, m.transform.tx + m.size.x, m.transform.ty + m.size.y);
        let (x0, y0, x1, y1) = rect(n);
        let overlap = |a0: f32, a1: f32, b0: f32, b1: f32| a1.min(b1) - a0.max(b0) > 0.0;
        let mut best = [f32::MAX; 4];
        for s in parent.children.iter().filter(|c| **c != id).filter_map(|c| doc.get(*c)).filter(|c| c.visible) {
            let (sx0, sy0, sx1, sy1) = rect(s);
            if overlap(x0, x1, sx0, sx1) {
                if sy1 <= y0 + 0.001 {
                    best[0] = best[0].min(y0 - sy1);
                }
                if sy0 >= y1 - 0.001 {
                    best[2] = best[2].min(sy0 - y1);
                }
            }
            if overlap(y0, y1, sy0, sy1) {
                if sx0 >= x1 - 0.001 {
                    best[1] = best[1].min(sx0 - x1);
                }
                if sx1 <= x0 + 0.001 {
                    best[3] = best[3].min(x0 - sx1);
                }
            }
        }
        for i in 0..4 {
            if best[i] < f32::MAX {
                margin[i] = best[i].max(0.0);
            }
        }
    }
    Some(BoxModel { size: (n.size.x, n.size.y), padding, margin })
}

// ------------------------------------------------------------------ variables

/// One variable used by a layer or its descendants, with its value in every mode of its collection.
#[derive(Clone, Debug, PartialEq)]
pub struct VarUse {
    pub id: String,
    pub name: String,
    /// Missing variable definitions have no known type.
    pub kind: Option<VariableKind>,
    pub collection: String,
    /// `(mode name, value)` in the order of the collection's modes.
    pub modes: Vec<(String, Option<VariableValue>)>,
    /// Properties bound on the inspected layer tree, e.g. "fills/0".
    pub props: Vec<String>,
    /// Each binding, in document order then property order, resolved at its own layer.
    pub effective: Vec<EffectiveVarUse>,
}

/// A token's actual value at one binding, including inherited collection modes.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveVarUse {
    pub node: NodeId,
    pub node_name: String,
    pub property: String,
    /// Active mode name; absent when the variable, collection or mode is unknown.
    pub mode: Option<String>,
    /// Absent for missing definitions, broken aliases or alias cycles.
    pub value: Option<VariableValue>,
}

/// Resolves `variable` in a fixed mode, following aliases (an alias into another collection
/// uses that collection's default mode).
pub fn resolve_in_mode(doc: &Document, variable: &str, mode: &str) -> Option<VariableValue> {
    let mut id = variable.to_string();
    let mut mode = mode.to_string();
    for _ in 0..16 {
        let v = doc.variables.get(&id)?;
        let coll = doc.variable_collections.get(&v.collection)?;
        let value = v.values.get(&mode).or_else(|| v.values.get(&coll.default_mode))?;
        match value {
            VariableValue::Alias(next) => {
                let target = doc.variables.get(next)?;
                if target.collection != v.collection {
                    mode = doc.variable_collections.get(&target.collection)?.default_mode.clone();
                }
                id = next.clone();
            }
            other => return Some(other.clone()),
        }
    }
    None
}

pub fn variables_used(doc: &Document, root: NodeId) -> Vec<VarUse> {
    let mut out: Vec<VarUse> = Vec::new();
    doc.walk(root, |n, _| {
        for (prop, var) in &n.variables.bindings {
            let definition = doc.variables.get(var);
            let collection = definition.and_then(|v| doc.variable_collections.get(&v.collection));
            let mode = collection.and_then(|c| {
                let active = doc.variable_mode(n.id, &c.id)?;
                c.modes.iter().find(|m| m.id == active).map(|m| m.name.clone())
            });
            let effective = EffectiveVarUse { node: n.id, node_name: n.name.clone(), property: prop.clone(), mode, value: doc.resolve_variable(n.id, var) };
            if let Some(u) = out.iter_mut().find(|u| u.id == *var) {
                if !u.props.contains(prop) {
                    u.props.push(prop.clone());
                }
                u.effective.push(effective);
                continue;
            }
            let modes = collection.map(|c| c.modes.iter().map(|m| (m.name.clone(), resolve_in_mode(doc, var, &m.id))).collect()).unwrap_or_default();
            out.push(VarUse {
                id: var.clone(),
                name: definition.map(|v| v.name.clone()).unwrap_or_else(|| var.clone()),
                kind: definition.map(|v| v.kind),
                collection: collection.map(|c| c.name.clone()).unwrap_or_default(),
                modes,
                props: vec![prop.clone()],
                effective: vec![effective],
            });
        }
    });
    out
}

pub fn format_value(v: &VariableValue) -> String {
    match v {
        VariableValue::Color(c) => {
            if c.a < 0.999 {
                format!("#{} {}%", c.to_hex(), fmt_num((c.a * 100.0).round()))
            } else {
                format!("#{}", c.to_hex())
            }
        }
        VariableValue::Number(n) => fmt_num(*n),
        VariableValue::String(s) => format!("\"{s}\""),
        VariableValue::Boolean(b) => b.to_string(),
        VariableValue::Alias(a) => a.clone(),
    }
}

// ------------------------------------------------------------------ component properties

fn clean_prop_label(raw: &str) -> String {
    raw.split('#').next().unwrap_or(raw).trim().to_string()
}

fn slot_label_for_node(doc: &Document, n: &Node) -> String {
    if let Some(b) = n.component.bindings.get(&OverrideField::SlotContent) {
        return clean_prop_label(b);
    }
    if let Some(src_id) = n.component.source {
        if let Some(src) = doc.get(src_id) {
            if let Some(b) = src.component.bindings.get(&OverrideField::SlotContent) {
                return clean_prop_label(b);
            }
        }
    }
    n.name.clone()
}

fn format_slot_items(doc: &Document, ids: &[NodeId]) -> String {
    let names: Vec<String> = ids.iter().filter_map(|cid| doc.get(*cid)).filter(|c| c.visible).map(|c| c.name.clone()).collect();
    if names.is_empty() {
        format!("Slot · {} item(s)", ids.len())
    } else {
        format!("Slot · {} item(s) ({})", names.len(), names.join(", "))
    }
}

/// `(property, value)` rows for an instance, component or slot frame; empty for other layers.
pub fn component_rows(doc: &Document, id: NodeId) -> Vec<(String, String)> {
    let Some(n) = doc.get(id) else { return Vec::new() };
    if let Some(comp) = instance_component(n) {
        let mut rows = vec![("Component".to_string(), doc.get(comp).map(|c| c.name.clone()).unwrap_or_default())];
        let mut seen_slots = Vec::new();
        for def in prop_defs(doc, comp) {
            let label = clean_prop_label(&def.name);
            let value = match prop_value(doc, id, &def) {
                PropValue::Bool(b) => b.to_string(),
                PropValue::Text(t) => t,
                PropValue::Variant(v) => v,
                PropValue::Instance(c) => doc.get(c).map(|c| c.name.clone()).unwrap_or_default(),
                PropValue::Slot(mut ids) => {
                    for &cid in &n.children {
                        if let Some(cn) = doc.get(cid) {
                            if (cn.component.is_slot || is_slot_container(doc, cid)) && slot_label_for_node(doc, cn).eq_ignore_ascii_case(&label) {
                                for &gc in &cn.children {
                                    if !ids.contains(&gc) {
                                        ids.push(gc);
                                    }
                                }
                            }
                        }
                    }
                    seen_slots.push(label.to_lowercase());
                    format_slot_items(doc, &ids)
                }
            };
            rows.push((label, value));
        }
        for &cid in &n.children {
            if let Some(cn) = doc.get(cid) {
                if cn.component.is_slot || is_slot_container(doc, cid) {
                    let label = slot_label_for_node(doc, cn);
                    if !seen_slots.contains(&label.to_lowercase()) {
                        seen_slots.push(label.to_lowercase());
                        rows.push((label, format_slot_items(doc, &cn.children)));
                    }
                }
            }
        }
        return rows;
    }
    if matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
        let mut slot_rows = Vec::new();
        let mut seen = Vec::new();
        for def in prop_defs(doc, id) {
            if def.kind == PropKind::Slot {
                let label = clean_prop_label(&def.name);
                seen.push(label.to_lowercase());
                slot_rows.push((label, "Slot".to_string()));
            }
        }
        for &cid in &n.children {
            if let Some(cn) = doc.get(cid) {
                if cn.component.is_slot || is_slot_container(doc, cid) {
                    let label = slot_label_for_node(doc, cn);
                    if !seen.contains(&label.to_lowercase()) {
                        seen.push(label.to_lowercase());
                        slot_rows.push((label, "Slot".to_string()));
                    }
                }
            }
        }
        if !slot_rows.is_empty() {
            let mut rows = vec![("Component".to_string(), n.name.clone())];
            rows.extend(slot_rows);
            return rows;
        }
        return Vec::new();
    }
    if n.component.is_slot || is_slot_container(doc, id) {
        return vec![("Slot".to_string(), slot_label_for_node(doc, n)), ("Items".to_string(), format!("{} item(s)", n.children.len()))];
    }
    Vec::new()
}

// ------------------------------------------------------------------ annotations

fn paints_hex(p: &[Paint]) -> String {
    let v: Vec<String> = p
        .iter()
        .filter(|p| p.visible)
        .map(|p| match &p.kind {
            PaintKind::Solid(c) => format!("#{}", c.to_hex()),
            PaintKind::Image { .. } => "Image".into(),
            _ => "Gradient".into(),
        })
        .collect();
    if v.is_empty() {
        "None".into()
    } else {
        v.join(", ")
    }
}

/// Human-readable font weight name matching the Dev Mode Inspect panel.
pub fn weight_name(w: u16) -> &'static str {
    match w {
        0..=149 => "Thin",
        150..=249 => "ExtraLight",
        250..=349 => "Light",
        350..=449 => "Regular",
        450..=549 => "Medium",
        550..=649 => "SemiBold",
        650..=749 => "Bold",
        750..=849 => "ExtraBold",
        _ => "Black",
    }
}

/// Labels from [`ANNOTATION_PROPERTIES`] that make sense for `n`.
pub fn applicable_properties(n: &Node) -> Vec<&'static str> {
    let text = matches!(n.kind, NodeKind::Text(_));
    let al = n.kind.frame_props().is_some_and(|f| f.auto_layout.is_some());
    ANNOTATION_PROPERTIES
        .iter()
        .map(|(l, _)| *l)
        .filter(|l| match *l {
            "Font" | "Font style" | "Font size" | "Font weight" | "Line height" | "Letter spacing" | "Text align" | "Text style" => text,
            "Gap" | "Padding" => al,
            "Corner radius" => !text,
            _ => true,
        })
        .collect()
}

/// Current value of a pinned property, as printed in the callout.
pub fn property_value(doc: &Document, n: &Node, label: &str) -> Option<String> {
    let px = |v: f32| format!("{}px", fmt_num(v));
    let lc = &n.layout_child;
    let al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
    let ts = match &n.kind {
        NodeKind::Text(t) => Some(&t.style),
        _ => None,
    };
    Some(match label {
        "Width" => px(n.size.x),
        "Height" => px(n.size.y),
        "Min width" => px(lc.min_w?),
        "Max width" => px(lc.max_w?),
        "Min height" => px(lc.min_h?),
        "Max height" => px(lc.max_h?),
        "Fill" => match &n.kind {
            NodeKind::Text(t) => paints_hex(&t.style.fills),
            _ => paints_hex(&n.fills),
        },
        "Stroke" => paints_hex(&n.stroke.paints),
        "Stroke weight" => px(n.stroke.weight),
        "Effects" => format!("{} effect{}", n.effects.len(), if n.effects.len() == 1 { "" } else { "s" }),
        "Corner radius" => {
            let r = n.corner_radii;
            if r.iter().all(|v| *v == r[0]) {
                px(r[0])
            } else {
                r.iter().map(|v| fmt_num(*v)).collect::<Vec<_>>().join(" ")
            }
        }
        "Text style" => doc.style_name(n.style_refs.get("text")?).to_string(),
        "Text align" => format!("{:?}", ts?.align_h),
        "Font" => ts?.font_family.clone(),
        "Font style" => format!("{}{}", weight_name(ts?.font_weight), if ts?.italic { " Italic" } else { "" }),
        "Font size" => px(ts?.font_size),
        "Font weight" => ts?.font_weight.to_string(),
        "Line height" => match ts?.line_height {
            LineHeight::Auto => "Auto".into(),
            LineHeight::Pixels(p) => px(p),
            LineHeight::Percent(p) => format!("{}%", fmt_num(p)),
        },
        "Letter spacing" => px(ts?.letter_spacing),
        "Gap" => px(al?.item_spacing),
        "Padding" => al?.padding.iter().map(|v| fmt_num(*v)).collect::<Vec<_>>().join(" "),
        "Opacity" => format!("{}%", fmt_num((n.opacity * 100.0).round())),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with_siblings() -> (Document, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let f = d.alloc_id();
        let mut fr = Node::frame(f, Rect::new(0.0, 0.0, 300.0, 300.0));
        if let NodeKind::Frame(fp) = &mut fr.kind {
            fp.auto_layout = Some(AutoLayout { padding: [4.0, 8.0, 4.0, 8.0], ..Default::default() });
        }
        let f = d.insert(page, fr).unwrap();
        let mk = |d: &mut Document, r: Rect| {
            let i = d.alloc_id();
            d.insert(f, Node::rectangle(i, r)).unwrap()
        };
        let a = mk(&mut d, Rect::new(0.0, 0.0, 50.0, 40.0));
        let b = mk(&mut d, Rect::new(70.0, 0.0, 50.0, 40.0));
        let c = mk(&mut d, Rect::new(0.0, 60.0, 50.0, 40.0));
        (d, a, b, c)
    }

    #[test]
    fn margins_are_the_gaps_to_neighbouring_siblings() {
        let (d, a, b, c) = doc_with_siblings();
        let ma = box_model(&d, a).unwrap();
        assert_eq!(ma.margin, [0.0, 20.0, 20.0, 0.0], "right neighbour b at 20, below neighbour c at 20");
        assert_eq!(box_model(&d, b).unwrap().margin, [0.0, 0.0, 0.0, 20.0]);
        assert_eq!(box_model(&d, c).unwrap().margin, [20.0, 0.0, 0.0, 0.0]);
        assert!(ma.padding.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn padding_comes_from_the_layers_own_auto_layout() {
        let (d, a, _, _) = doc_with_siblings();
        let parent = d.get(a).unwrap().parent.unwrap();
        let m = box_model(&d, parent).unwrap();
        assert_eq!(m.padding, [4.0, 8.0, 4.0, 8.0]);
        assert_eq!(m.margin, [0.0; 4], "top-level frames have no siblings");
        assert_eq!(m.size, (300.0, 300.0));
    }

    fn var_doc() -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let f = d.alloc_id();
        let f = d.insert(page, Node::frame(f, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        d.variable_collections.insert(
            "c".into(),
            VariableCollection {
                id: "c".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "l".into(), name: "Light".into() }, VariableMode { id: "d".into(), name: "Dark".into() }],
                default_mode: "l".into(),
                variables: vec!["base".into(), "bg".into()],
                key: None,
                source_library_key: None,
            },
        );
        let var = |id: &str, vals: Vec<(&str, VariableValue)>| Variable {
            id: id.into(),
            name: format!("Color/{id}"),
            collection: "c".into(),
            kind: VariableKind::Color,
            values: vals.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
        };
        d.variables.insert("base".into(), var("base", vec![("l", VariableValue::Color(Color::WHITE)), ("d", VariableValue::Color(Color::BLACK))]));
        d.variables.insert("bg".into(), var("bg", vec![("l", VariableValue::Alias("base".into())), ("d", VariableValue::Alias("base".into()))]));
        d.get_mut(f).unwrap().variables.bindings.insert("fills/0".into(), "bg".into());
        d.get_mut(f).unwrap().variables.bindings.insert("strokes/0".into(), "bg".into());
        (d, f)
    }

    #[test]
    fn variables_used_lists_every_mode_through_aliases() {
        let (d, f) = var_doc();
        let u = variables_used(&d, f);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].name, "Color/bg");
        assert_eq!(u[0].collection, "Theme");
        assert_eq!(u[0].props, vec!["fills/0".to_string(), "strokes/0".to_string()]);
        assert_eq!(
            u[0].modes,
            vec![("Light".to_string(), Some(VariableValue::Color(Color::WHITE))), ("Dark".to_string(), Some(VariableValue::Color(Color::BLACK)))]
        );
        assert_eq!(format_value(&VariableValue::Color(Color::WHITE)), "#FFFFFF");
        assert_eq!(format_value(&VariableValue::Number(8.0)), "8");
    }

    #[test]
    fn effective_variables_resolve_each_binding_at_its_own_layer() {
        let (mut d, f) = var_doc();
        d.get_mut(f).unwrap().variables.modes.insert("c".into(), "d".into());
        let child = d.alloc_id();
        let mut n = Node::rectangle(child, Rect::new(0.0, 0.0, 5.0, 5.0));
        n.name = "Light child".into();
        n.variables.modes.insert("c".into(), "l".into());
        n.variables.bindings.insert("fills/0".into(), "bg".into());
        d.insert(f, n).unwrap();

        let used = variables_used(&d, f);
        assert_eq!(used.len(), 1);
        assert_eq!(used[0].props, vec!["fills/0", "strokes/0"]);
        let effective = &used[0].effective;
        assert_eq!(effective.len(), 3);
        assert_eq!(effective[0].node, f);
        assert_eq!(effective[0].property, "fills/0");
        assert_eq!(effective[1].property, "strokes/0");
        for value in &effective[..2] {
            assert_eq!(value.mode.as_deref(), Some("Dark"));
            assert_eq!(value.value, Some(VariableValue::Color(Color::BLACK)));
        }
        assert_eq!(effective[2].node, child);
        assert_eq!(effective[2].node_name, "Light child");
        assert_eq!(effective[2].mode.as_deref(), Some("Light"));
        assert_eq!(effective[2].value, Some(VariableValue::Color(Color::WHITE)));
        assert_eq!(used, variables_used(&d, f), "ordering must be stable");
    }

    #[test]
    fn effective_cross_collection_alias_uses_the_inherited_target_mode() {
        let (mut d, f) = var_doc();
        let mut target = d.variable_collections["c"].clone();
        target.id = "target".into();
        target.name = "Palette".into();
        target.variables = vec!["base".into()];
        d.variable_collections.insert("target".into(), target);
        d.variables.get_mut("base").unwrap().collection = "target".into();
        let page = d.get(f).unwrap().parent.unwrap();
        d.get_mut(page).unwrap().variables.modes.insert("target".into(), "d".into());

        let used = variables_used(&d, f);
        assert_eq!(used[0].effective[0].mode.as_deref(), Some("Light"));
        assert_eq!(used[0].effective[0].value, Some(VariableValue::Color(Color::BLACK)));
        // The all-modes overview remains independent of the current layer context.
        assert_eq!(used[0].modes[0].1, Some(VariableValue::Color(Color::WHITE)));
    }

    #[test]
    fn missing_variable_definitions_remain_visible_and_unresolved() {
        let (mut d, f) = var_doc();
        d.get_mut(f).unwrap().variables.bindings.insert("width".into(), "missing".into());
        let used = variables_used(&d, f);
        let missing = used.iter().find(|v| v.id == "missing").unwrap();
        assert_eq!(missing.name, "missing");
        assert_eq!(missing.kind, None);
        assert!(missing.collection.is_empty());
        assert!(missing.modes.is_empty());
        assert_eq!(missing.effective[0].property, "width");
        assert_eq!(missing.effective[0].mode, None);
        assert_eq!(missing.effective[0].value, None);
    }

    #[test]
    fn missing_collections_broken_aliases_and_cycles_are_unresolved() {
        let (mut d, f) = var_doc();
        d.variables.get_mut("base").unwrap().values.insert("l".into(), VariableValue::Alias("bg".into()));
        let used = variables_used(&d, f);
        assert!(used[0].effective.iter().all(|u| u.value.is_none()));
        assert_eq!(used[0].modes[0].1, None);

        d.variables.remove("base");
        let used = variables_used(&d, f);
        assert!(used[0].effective.iter().all(|u| u.value.is_none()));

        d.variable_collections.remove("c");
        let used = variables_used(&d, f);
        assert_eq!(used[0].kind, Some(VariableKind::Color));
        assert!(used[0].modes.is_empty());
        assert!(used[0].effective.iter().all(|u| u.mode.is_none() && u.value.is_none()));
    }

    #[test]
    fn pinned_property_values() {
        let (d, a, _, _) = doc_with_siblings();
        let n = d.get(a).unwrap();
        assert_eq!(property_value(&d, n, "Width").as_deref(), Some("50px"));
        assert_eq!(property_value(&d, n, "Fill").as_deref(), Some("#D9D9D9"));
        assert_eq!(property_value(&d, n, "Font"), None);
        assert!(applicable_properties(n).contains(&"Width"));
        assert!(!applicable_properties(n).contains(&"Font"));
        let parent = d.get(d.get(a).unwrap().parent.unwrap()).unwrap();
        assert!(applicable_properties(parent).contains(&"Padding"));
        assert_eq!(property_value(&d, parent, "Padding").as_deref(), Some("4 8 4 8"));
    }

    #[test]
    fn inspect_output_unchanged() {
        let (doc, card_id) = crate::codegen::fixture::card();
        let bm = box_model(&doc, card_id).unwrap();
        assert_eq!(bm.size, (312.0, 200.0));
        assert_eq!(bm.padding, [16.0; 4]);
        assert_eq!(bm.margin, [0.0; 4]);

        let vars = variables_used(&doc, card_id);
        type VarSummary = (String, String, Vec<String>, Vec<(String, Option<String>)>);
        let var_summary: Vec<VarSummary> = vars
            .iter()
            .map(|u| (u.name.clone(), u.collection.clone(), u.props.clone(), u.modes.iter().map(|(m, v)| (m.clone(), v.as_ref().map(format_value))).collect()))
            .collect();
        assert_eq!(
            var_summary,
            vec![
                (
                    "Color/Text/Muted".to_string(),
                    "Tokens".to_string(),
                    vec!["fills/0".to_string()],
                    vec![("Light".to_string(), Some("#666666".to_string())), ("Dark".to_string(), Some("#AAAAAA".to_string()))],
                ),
                (
                    "Color/Primary".to_string(),
                    "Tokens".to_string(),
                    vec!["fills/0".to_string()],
                    vec![("Light".to_string(), Some("#0D99FF".to_string())), ("Dark".to_string(), Some("#4DB8FF".to_string()))],
                ),
                (
                    "Spacing/Md".to_string(),
                    "Tokens".to_string(),
                    vec!["itemSpacing".to_string()],
                    vec![("Light".to_string(), Some("12".to_string())), ("Dark".to_string(), Some("12".to_string()))],
                ),
            ]
        );

        let mut inst_id = None;
        doc.walk(card_id, |n, _| {
            if matches!(n.kind, NodeKind::Instance { .. }) {
                inst_id = Some(n.id);
            }
        });
        let rows = component_rows(&doc, inst_id.unwrap());
        assert_eq!(
            rows,
            vec![("Component".to_string(), "Button".to_string()), ("Label".to_string(), "Save".to_string()), ("Disabled".to_string(), "true".to_string()),]
        );
    }
}
