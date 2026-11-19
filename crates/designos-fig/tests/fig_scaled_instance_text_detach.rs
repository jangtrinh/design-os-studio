//! Detaching an instance made with Figma's scale tool. A text in such an instance draws its style and typography
//! variables at the instance's scale and reports no style link and no variable binding; once the instance is
//! detached the text holds the values it drew as its own, and no longer follows the main.
//! Hand-written kiwi-shaped fixtures; nothing here comes from a real file.

mod scaled_text_fixtures;

use designos_doc::*;
use designos_fig::import_fig_node_changes;
use scaled_text_fixtures::*;
use serde_json::{json, Value};

/// A text bound straight to the size and line height variables, with no text style.
fn bound_directly(id: (u64, u64), name: &str) -> Value {
    let mut v = text(
        id,
        (5, 1),
        name,
        (8, 3),
        json!({"variableConsumptionMap": {"entries": [
            {"variableField": "FONT_SIZE", "variableData": alias("VAR_SIZE")},
            {"variableField": "LINE_HEIGHT", "variableData": alias("VAR_LH")}
        ]}}),
    );
    v.as_object_mut().unwrap().remove("styleIdForText");
    v
}

/// Texts of every kind a scaled instance can hold: linked to a static style, linked to a style bound to variables,
/// bound to variables on its own, and plain.
fn badge_texts() -> Vec<Value> {
    let mut plain = text((5, 5), (5, 1), "Plain", (8, 3), json!({}));
    plain.as_object_mut().unwrap().remove("styleIdForText");
    vec![
        text((5, 2), (5, 1), "Linked", (8, 3), json!({})),
        text((5, 3), (5, 1), "Styled bound", (8, 2), json!({})),
        bound_directly((5, 4), "Bound directly"),
        plain,
    ]
}

fn pixels(doc: &Document, id: NodeId) -> Vec<u8> {
    let pm = designos_render::render_node(doc, id, 1.0).expect("the layer renders");
    let data = pm.data().to_vec();
    assert!(data.chunks(4).any(|p| p[3] != 0), "the render is not blank");
    data
}

fn has_typography_binding(doc: &Document, n: &Node) -> bool {
    TEXT_STYLE_BINDING_KEYS.iter().any(|k| doc.effective_binding(n.id, k).is_some())
}

#[test]
fn detaching_a_scaled_instance_keeps_every_text_drawing_the_same_at_its_own_values() {
    for dark in [false, true] {
        let mut doc = import_fig_node_changes(&badge_doc(badge_texts(), 0.8), "t").unwrap();
        set_page_mode(&mut doc, dark);
        let inst = find(&doc, "Scaled badge").id;
        let names = ["Linked", "Styled bound", "Bound directly", "Plain"];
        let ids: Vec<NodeId> = names.iter().map(|n| child(&doc, find(&doc, "Scaled badge"), n).id).collect();
        let before: Vec<(f32, f32)> = ids.iter().map(|id| drawn(&doc, doc.get(*id).unwrap())).collect();
        let before_px = pixels(&doc, inst);
        // preconditions: the fixture really is a scaled text with a link and with bindings
        assert!(text_of(doc.get(ids[0]).unwrap()).uniform_scale < 1.0);
        assert!(doc.get(ids[0]).unwrap().style_refs.contains_key("text"));
        assert!(has_typography_binding(&doc, doc.get(ids[1]).unwrap()), "a style bound to variables counts as a binding");
        assert!(doc.get(ids[2]).unwrap().variables.bindings.contains_key("fontSize"));

        detach_instance(&mut doc, inst).apply(&mut doc).unwrap();
        assert!(!matches!(doc.get(inst).unwrap().kind, NodeKind::Instance { .. }), "the instance is detached");
        for ((id, name), drew) in ids.iter().zip(names).zip(before) {
            let n = doc.get(*id).unwrap();
            let t = text_of(n);
            assert_eq!(t.uniform_scale, 1.0, "{name} (dark {dark}): the factor is gone");
            assert!(!n.style_refs.contains_key("text"), "{name}: the style link is gone");
            assert!(!has_typography_binding(&doc, n), "{name}: no typography variable is bound");
            assert!(n.text_style_overrides.is_empty(), "{name}: no style override is left");
            let (size, lh) = (t.style.font_size, px(t.style.line_height));
            assert!(close(size, drew.0) && close(lh, drew.1), "{name} (dark {dark}): holds {size} / {lh}, drew {drew:?}");
            assert_eq!(drawn(&doc, n), drew, "{name}: draws what it drew");
        }
        let (size, lh) = (text_of(doc.get(ids[0]).unwrap()).style.font_size, px(text_of(doc.get(ids[0]).unwrap()).style.line_height));
        assert!(close(size, 11.2) && close(lh, 16.0), "a static 14 / 20 style at 0.8 is held as 11.2 / 16, got {size} / {lh}");
        assert!(pixels(&doc, inst) == before_px, "dark {dark}: the frame renders the same before and after the detach");
    }
}

#[test]
fn a_detached_text_keeps_the_scaled_letter_spacing_paragraph_values_and_every_run() {
    let rich = json!({
        "letterSpacing": {"units": "PIXELS", "value": 2.0},
        "paragraphSpacing": 10.0, "paragraphIndent": 5.0, "listSpacing": 6.0,
        "textData": {"characters": "AB", "characterStyleIDs": [0, 1],
            "styleOverrideTable": [{"styleID": 1, "fontSize": 20.0, "letterSpacing": {"units": "PIXELS", "value": 4.0}}]}
    });
    let mut t = text((5, 2), (5, 1), "Rich", (8, 3), rich);
    t.as_object_mut().unwrap().remove("styleIdForText");
    let mut doc = import_fig_node_changes(&badge_doc(vec![t], 0.8), "t").unwrap();
    let inst = find(&doc, "Scaled badge").id;
    let id = child(&doc, find(&doc, "Scaled badge"), "Rich").id;
    let before = text_of(&resolve_node(&doc, doc.get(id).unwrap())).clone();
    assert!(!before.runs.is_empty() && before.uniform_scale < 1.0, "{} runs, factor {}", before.runs.len(), before.uniform_scale);
    assert!(
        close(before.paragraph.indent, 4.0) && close(before.paragraph.list_spacing, 4.8),
        "indent {} / list {}",
        before.paragraph.indent,
        before.paragraph.list_spacing
    );
    let before_px = pixels(&doc, inst);
    detach_instance(&mut doc, inst).apply(&mut doc).unwrap();
    let after = text_of(doc.get(id).unwrap());
    let mut expect = before.clone();
    expect.uniform_scale = 1.0;
    assert_eq!(*after, expect, "every length, run and paragraph value is held as drawn");
    assert!(pixels(&doc, inst) == before_px);
}

/// A card holding a plain text and a nested badge instance, scaled by 1.5; the badge inside is scaled by 0.5.
fn card_doc() -> Value {
    let mut all = base();
    all.push(node((5, 1), (0, 2), "c", "SYMBOL", "Badge", json!({"size": {"x": 100.0, "y": 28.0}})));
    all.push(text((5, 2), (5, 1), "Rating", (8, 2), json!({})));
    all.push(node((5, 10), (0, 2), "d", "SYMBOL", "Card", json!({"size": {"x": 200.0, "y": 60.0}})));
    all.push(text((5, 12), (5, 10), "Caption", (8, 3), json!({})));
    all.push(node(
        (5, 11),
        (5, 10),
        "b",
        "INSTANCE",
        "Half badge",
        json!({"overrideKey": g(9, 11), "size": {"x": 50.0, "y": 14.0}, "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 0.5}}),
    ));
    all.push(node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "Big card",
        json!({"size": {"x": 300.0, "y": 90.0}, "symbolData": {"symbolID": g(5, 10), "uniformScaleFactor": 1.5}}),
    ));
    Value::Array(all)
}

#[test]
fn a_nested_instance_keeps_its_texts_scale_and_link_while_the_outer_text_is_baked() {
    let mut doc = import_fig_node_changes(&card_doc(), "t").unwrap();
    let card = find(&doc, "Big card").id;
    let nested = child(&doc, find(&doc, "Big card"), "Half badge").id;
    let inner = child(&doc, doc.get(nested).unwrap(), "Rating").id;
    let caption = child(&doc, find(&doc, "Big card"), "Caption").id;
    let inner_before = doc.get(inner).unwrap().clone();
    let (caption_size, caption_lh) = drawn(&doc, doc.get(caption).unwrap());
    assert!(close(caption_size, 21.0) && close(caption_lh, 30.0), "1.5 of 14 / 20, got {caption_size} / {caption_lh}");
    let before_px = pixels(&doc, card);

    detach_instance(&mut doc, card).apply(&mut doc).unwrap();

    assert!(matches!(doc.get(nested).unwrap().kind, NodeKind::Instance { .. }), "the nested instance stays an instance");
    let inner_after = doc.get(inner).unwrap();
    assert!(close(text_of(inner_after).uniform_scale, 0.75), "the nested text keeps its factor, got {}", text_of(inner_after).uniform_scale);
    assert_eq!(inner_after.style_refs, inner_before.style_refs, "the nested text keeps its style link");
    assert_eq!(inner_after.variables.bindings, inner_before.variables.bindings, "and its variable bindings");
    assert_eq!(text_of(inner_after).style, text_of(&inner_before).style);
    let c = doc.get(caption).unwrap();
    assert_eq!(text_of(c).uniform_scale, 1.0, "the caption outside the nested instance is baked");
    assert!(!c.style_refs.contains_key("text"));
    assert_eq!(drawn(&doc, c), (caption_size, caption_lh));
    assert!(pixels(&doc, card) == before_px, "the frame renders the same before and after the detach");
}

#[test]
fn detaching_a_nested_scaled_instance_inside_an_instance_changes_no_text() {
    let mut doc = import_fig_node_changes(&card_doc(), "t").unwrap();
    let nested = child(&doc, find(&doc, "Big card"), "Half badge").id;
    let inner = child(&doc, doc.get(nested).unwrap(), "Rating").id;
    let before = doc.get(inner).unwrap().clone();
    detach_instance(&mut doc, nested).apply(&mut doc).unwrap();
    let after = doc.get(inner).unwrap();
    assert_eq!(text_of(after).uniform_scale, text_of(&before).uniform_scale, "the text still sits in the outer instance");
    assert_eq!(after.style_refs, before.style_refs);
}

#[test]
fn turning_a_scaled_instance_into_a_component_bakes_its_texts_too() {
    let mut doc = import_fig_node_changes(&badge_doc(badge_texts(), 0.8), "t").unwrap();
    let inst = find(&doc, "Scaled badge").id;
    let id = child(&doc, find(&doc, "Scaled badge"), "Styled bound").id;
    let drew = drawn(&doc, doc.get(id).unwrap());
    create_component(&mut doc, inst).apply(&mut doc).unwrap();
    let n = doc.get(id).unwrap();
    assert_eq!(text_of(n).uniform_scale, 1.0);
    assert!(!n.style_refs.contains_key("text") && !has_typography_binding(&doc, n));
    assert_eq!(drawn(&doc, n), drew);
}

/// The badge document with four more two-mode number variables: letter spacing, paragraph spacing, indent and list
/// spacing (light, then dark).
fn badge_doc_with_spacing_variables(children: Vec<Value>) -> Value {
    let mut all = badge_doc(children, 0.8);
    all.as_array_mut().unwrap().extend([
        float_var((6, 6), "Spacing/letter", "VAR_LS", 1.0, 3.0),
        float_var((6, 7), "Spacing/paragraph", "VAR_PS", 5.0, 12.0),
        float_var((6, 8), "Spacing/indent", "VAR_PI", 4.0, 9.0),
        float_var((6, 9), "Spacing/list", "VAR_LIST", 2.0, 7.0),
    ]);
    all
}

fn variable_id(doc: &Document, name: &str) -> String {
    doc.variables.values().find(|v| v.name == name).unwrap_or_else(|| panic!("no variable {name}")).id.clone()
}

#[test]
fn a_detached_text_bound_to_variables_holds_every_value_it_drew_in_dark_mode_runs_included() {
    let rich = json!({
        "letterSpacing": {"units": "PIXELS", "value": 2.0},
        "lineHeight": {"units": "PERCENT", "value": 150.0},
        "paragraphSpacing": 10.0, "paragraphIndent": 5.0, "listSpacing": 6.0,
        "textData": {"characters": "AB", "characterStyleIDs": [0, 1],
            "styleOverrideTable": [{"styleID": 1, "fontSize": 20.0, "letterSpacing": {"units": "PIXELS", "value": 4.0}}]}
    });
    let mut t = text((5, 2), (5, 1), "Rich", (8, 3), rich);
    t.as_object_mut().unwrap().remove("styleIdForText");
    let mut doc = import_fig_node_changes(&badge_doc_with_spacing_variables(vec![t]), "t").unwrap();
    let inst = find(&doc, "Scaled badge").id;
    let id = child(&doc, find(&doc, "Scaled badge"), "Rich").id;
    for (key, name) in [
        ("fontSize", "Font size/text-sm"),
        ("letterSpacing", "Spacing/letter"),
        ("paragraphSpacing", "Spacing/paragraph"),
        ("paragraphIndent", "Spacing/indent"),
        ("listSpacing", "Spacing/list"),
    ] {
        let var = variable_id(&doc, name);
        doc.get_mut(id).unwrap().variables.bindings.insert(key.into(), var);
    }
    set_page_mode(&mut doc, true);
    let stored = text_of(doc.get(id).unwrap()).clone();
    let before = text_of(&resolve_node(&doc, doc.get(id).unwrap())).clone();
    assert!(!before.runs.is_empty() && before.uniform_scale < 1.0);
    // dark values at 0.8: size 20 -> 16, letter 3 -> 2.4, paragraph 12 -> 9.6, indent 9 -> 7.2, list 7 -> 5.6
    assert!(
        close(before.style.font_size, 16.0) && close(before.style.letter_spacing, 2.4),
        "drawn {} / {}",
        before.style.font_size,
        before.style.letter_spacing
    );
    assert!(close(before.style.paragraph_spacing, 9.6) && close(before.paragraph.indent, 7.2) && close(before.paragraph.list_spacing, 5.6));
    assert!(!close(stored.style.letter_spacing, 2.4) && !close(stored.paragraph.indent, 7.2), "the stored values are not the drawn ones");
    let before_px = pixels(&doc, inst);

    detach_instance(&mut doc, inst).apply(&mut doc).unwrap();

    let n = doc.get(id).unwrap();
    let after = text_of(n);
    assert_eq!(after.uniform_scale, 1.0);
    for key in ["fontSize", "letterSpacing", "paragraphSpacing", "paragraphIndent", "listSpacing"] {
        assert!(!n.variables.bindings.contains_key(key), "{key} is no longer bound");
    }
    assert!(close(after.style.font_size, 16.0) && close(after.style.letter_spacing, 2.4) && close(after.style.paragraph_spacing, 9.6), "base style");
    assert!(close(after.paragraph.indent, 7.2) && close(after.paragraph.list_spacing, 5.6), "paragraph values");
    for (run, drawn_run) in after.runs.iter().zip(&before.runs) {
        assert!(close(run.style.font_size, 16.0) && close(run.style.letter_spacing, 2.4), "run holds {} / {}", run.style.font_size, run.style.letter_spacing);
        assert_eq!(run.style.line_height, drawn_run.style.line_height);
    }
    assert!(matches!(after.style.line_height, LineHeight::Percent(v) if close(v, 150.0)), "a percent line height stays a percent");
    let mut expect = before.clone();
    expect.uniform_scale = 1.0;
    assert_eq!(*after, expect, "the text holds exactly what it drew");
    assert!(pixels(&doc, inst) == before_px, "the frame renders the same before and after the detach");
}

#[test]
fn undoing_a_detach_brings_back_the_factor_the_link_and_the_bindings() {
    let mut doc = import_fig_node_changes(&badge_doc(badge_texts(), 0.8), "t").unwrap();
    let inst = find(&doc, "Scaled badge").id;
    let before: Vec<Node> = ["Linked", "Styled bound", "Bound directly", "Plain"].iter().map(|n| child(&doc, find(&doc, "Scaled badge"), n).clone()).collect();
    let inverse = detach_instance(&mut doc, inst).apply(&mut doc).unwrap();
    inverse.apply(&mut doc).unwrap();
    assert!(matches!(doc.get(inst).unwrap().kind, NodeKind::Instance { .. }), "an instance again");
    for n in before {
        assert_eq!(*doc.get(n.id).unwrap(), n, "{} is as it was", n.name);
    }
}

#[test]
fn an_instance_placed_inside_another_instance_keeps_its_scaled_texts_when_detached() {
    let mut doc = import_fig_node_changes(&card_doc(), "t").unwrap();
    let card = find(&doc, "Big card").id;
    let badge_main = find(&doc, "Badge").id;
    designos_doc::create_instance(&mut doc, badge_main, card, Vec2::ZERO).apply(&mut doc).unwrap();
    let placed = *doc.get(card).unwrap().children.last().unwrap();
    assert!(matches!(doc.get(placed).unwrap().kind, NodeKind::Instance { .. }));
    let inner = doc.get(placed).unwrap().children[0];
    if let NodeKind::Text(t) = &mut doc.get_mut(inner).unwrap().kind {
        t.apply_uniform_scale(0.8);
    }
    let before = doc.get(inner).unwrap().clone();
    detach_instance(&mut doc, placed).apply(&mut doc).unwrap();
    assert!(!matches!(doc.get(placed).unwrap().kind, NodeKind::Instance { .. }), "detached");
    let after = doc.get(inner).unwrap();
    assert_eq!(text_of(after).uniform_scale, text_of(&before).uniform_scale, "its text still sits inside the outer instance");
    assert_eq!(after.style_refs, before.style_refs);
    assert_eq!(after.variables.bindings, before.variables.bindings);
}

#[test]
fn turning_a_nested_instance_into_a_component_changes_no_text() {
    let mut doc = import_fig_node_changes(&card_doc(), "t").unwrap();
    let nested = child(&doc, find(&doc, "Big card"), "Half badge").id;
    let inner = child(&doc, doc.get(nested).unwrap(), "Rating").id;
    let before = doc.get(inner).unwrap().clone();
    create_component(&mut doc, nested).apply(&mut doc).unwrap();
    let after = doc.get(inner).unwrap();
    assert!(close(text_of(after).uniform_scale, 0.75), "the text still sits inside the outer instance, factor {}", text_of(after).uniform_scale);
    assert_eq!(after.style_refs, before.style_refs);
    assert_eq!(after.variables.bindings, before.variables.bindings);
}
