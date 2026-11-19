//! Text inside instances made with Figma's scale tool (`uniformScaleFactor`). Figma keeps such a text linked to
//! its text style and draws the style's values at the instance's scale: a 14 px / 20 px style draws at
//! 11.2 px / 16 px in a 0.8 instance, line heights in pixels scale, percent and automatic ones follow the size.
//! Hand-written kiwi-shaped fixtures; nothing here comes from a real file.

mod scaled_text_fixtures;

use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes, import_rest_json};
use scaled_text_fixtures::*;
use serde_json::{json, Value};

#[test]
fn bound_style_draws_at_the_instance_scale_and_the_link_stays() {
    let doc = import_fig_node_changes(&badge_doc(vec![text((5, 2), (5, 1), "Rating", (8, 2), json!({}))], 0.8), "t").unwrap();
    let rating = scaled_rating(&doc);
    let (size, lh) = drawn(&doc, rating);
    assert!(close(size, 11.2) && close(lh, 16.0), "Figma draws 14 / 20 at 0.8 as 11.2 / 16; drawn {size} / {lh}");
    assert!(rating.style_refs.contains_key("text"), "Figma keeps the text style link in a scaled instance");
    let main = child(&doc, find(&doc, "Badge"), "Rating");
    assert_eq!(drawn(&doc, main), (14.0, 20.0), "the main component is not scaled");
}

#[test]
fn editing_a_static_style_reaches_scaled_instance_text_at_its_scale() {
    let mut doc = import_fig_node_changes(&badge_doc(vec![text((5, 2), (5, 1), "Rating", (8, 3), json!({}))], 0.8), "t").unwrap();
    let key = scaled_rating(&doc).style_refs.get("text").cloned().expect("the scaled text keeps its static style link");
    let Some(Style::Text(old)) = doc.styles.get(&key).cloned() else { panic!("text style") };
    let edited = TextStyle { font_size: 20.0, line_height: LineHeight::Pixels(28.0), ..old };
    update_style(&doc, &key, Style::Text(edited)).unwrap().apply(&mut doc).unwrap();
    let (size, lh) = drawn(&doc, scaled_rating(&doc));
    assert!(close(size, 16.0) && close(lh, 22.4), "an edit to 20 / 28 draws at 16 / 22.4 in the 0.8 instance; drawn {size} / {lh}");
    assert_eq!(drawn(&doc, child(&doc, find(&doc, "Badge"), "Rating")), (20.0, 28.0));
}

#[test]
fn bound_fill_and_bound_size_follow_the_mode_in_a_scaled_instance() {
    let mut fill = solid(1.0, 0.0, 0.0);
    fill["colorVar"] = color_alias("VAR_INK");
    let rating = text((5, 2), (5, 1), "Rating", (8, 2), json!({"fillPaints": [fill]}));
    let mut doc = import_fig_node_changes(&badge_doc(vec![rating], 0.8), "t").unwrap();
    let ink = |doc: &Document| {
        let r = resolve_node(doc, scaled_rating(doc));
        match text_of(&r).style.fills[0].kind {
            PaintKind::Solid(c) => c,
            ref k => panic!("{k:?}"),
        }
    };
    set_page_mode(&mut doc, false);
    assert_eq!(ink(&doc), Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
    let (size, lh) = drawn(&doc, scaled_rating(&doc));
    assert!(close(size, 11.2) && close(lh, 16.0), "light: drawn {size} / {lh}");
    set_page_mode(&mut doc, true);
    assert_eq!(ink(&doc), Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }, "the fill follows its variable");
    let (size, lh) = drawn(&doc, scaled_rating(&doc));
    assert!(close(size, 16.0) && close(lh, 19.2), "dark is 20 / 24, drawn at 0.8 as 16 / 19.2; drawn {size} / {lh}");
}

#[test]
fn relative_line_heights_are_not_scaled_twice_and_every_run_scales() {
    let runs = json!({
        "lineHeight": {"units": "PERCENT", "value": 150.0},
        "letterSpacing": {"units": "PIXELS", "value": 2.0},
        "textData": {"characters": "AB", "characterStyleIDs": [0, 1],
            "styleOverrideTable": [{"styleID": 1, "fontSize": 20.0, "letterSpacing": {"units": "PIXELS", "value": 4.0}}]}
    });
    let mut percent = text((5, 2), (5, 1), "Percent", (8, 3), runs);
    percent.as_object_mut().unwrap().remove("styleIdForText");
    let mut auto = text((5, 3), (5, 1), "Auto", (8, 3), json!({"lineHeight": {"units": "RAW", "value": 1.0}}));
    auto.as_object_mut().unwrap().remove("styleIdForText");
    let doc = import_fig_node_changes(&badge_doc(vec![percent, auto], 0.8), "t").unwrap();
    let inst = find(&doc, "Scaled badge");
    let main = find(&doc, "Badge");
    let (p, mp) = (text_of(child(&doc, inst, "Percent")), text_of(child(&doc, main, "Percent")));
    assert_eq!(p.style.line_height, mp.style.line_height, "a percent line height follows the scaled size by itself");
    assert!(matches!(p.style.line_height, LineHeight::Percent(v) if close(v, 150.0)));
    assert!(close(p.style.font_size, 11.2) && close(p.style.letter_spacing, 1.6), "base: {} / {}", p.style.font_size, p.style.letter_spacing);
    assert_eq!(p.runs.len(), mp.runs.len());
    for (r, m) in p.runs.iter().zip(&mp.runs) {
        assert!(close(r.style.font_size, m.style.font_size * 0.8), "run size {} vs main {}", r.style.font_size, m.style.font_size);
        assert!(close(r.style.letter_spacing, m.style.letter_spacing * 0.8), "run tracking {} vs main {}", r.style.letter_spacing, m.style.letter_spacing);
        assert_eq!(r.style.line_height, m.style.line_height);
    }
    let a = text_of(child(&doc, inst, "Auto"));
    assert_eq!(a.style.line_height, LineHeight::Auto, "an automatic line height stays automatic");
    assert!(close(a.style.font_size, 11.2));
}

#[test]
fn a_main_text_edit_synced_to_a_scaled_instance_keeps_the_scaled_size() {
    let mut doc = import_fig_node_changes(&badge_doc(vec![text((5, 2), (5, 1), "Rating", (8, 3), json!({}))], 0.8), "t").unwrap();
    let badge = find(&doc, "Badge").id;
    let main_rating = child(&doc, find(&doc, "Badge"), "Rating").id;
    if let NodeKind::Text(t) = &mut doc.get_mut(main_rating).unwrap().kind {
        t.content = "Best design tool".into();
    }
    sync_instances(&mut doc, badge).apply(&mut doc).unwrap();
    let rating = scaled_rating(&doc);
    assert_eq!(text_of(rating).content, "Best design tool", "the edit reached the instance");
    let (size, lh) = drawn(&doc, rating);
    assert!(close(size, 11.2) && close(lh, 16.0), "after the sync: drawn {size} / {lh}");
}

#[test]
fn nested_scales_multiply_and_a_scale_above_one_enlarges() {
    let mut all = base();
    all.push(node((5, 1), (0, 2), "c", "SYMBOL", "Badge", json!({"size": {"x": 100.0, "y": 28.0}})));
    all.push(text((5, 2), (5, 1), "Rating", (8, 2), json!({})));
    all.push(node((5, 10), (0, 2), "d", "SYMBOL", "Card", json!({"size": {"x": 200.0, "y": 60.0}})));
    all.push(node(
        (5, 11),
        (5, 10),
        "a",
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
    let doc = import_fig_node_changes(&Value::Array(all), "t").unwrap();
    let in_card = child(&doc, child(&doc, find(&doc, "Card"), "Half badge"), "Rating");
    assert_eq!(drawn(&doc, in_card), (7.0, 10.0), "0.5 of 14 / 20");
    let in_big = child(&doc, child(&doc, find(&doc, "Big card"), "Half badge"), "Rating");
    let (size, lh) = drawn(&doc, in_big);
    assert!(close(size, 10.5) && close(lh, 15.0), "1.5 x 0.5 of 14 / 20 is 10.5 / 15; drawn {size} / {lh}");
}

#[test]
fn a_nested_instance_swapped_inside_a_scaled_instance_draws_at_its_scale() {
    let mut all = base();
    all.push(node((5, 1), (0, 2), "c", "SYMBOL", "Icon", json!({"size": {"x": 20.0, "y": 20.0}})));
    all.push(node((5, 5), (0, 2), "d", "SYMBOL", "Icon with label", json!({"size": {"x": 100.0, "y": 28.0}})));
    all.push(text((5, 6), (5, 5), "Label", (8, 3), json!({})));
    all.push(node((5, 10), (0, 2), "e", "SYMBOL", "Card", json!({"size": {"x": 200.0, "y": 60.0}})));
    all.push(node(
        (5, 11),
        (5, 10),
        "a",
        "INSTANCE",
        "Slot",
        json!({"overrideKey": g(9, 11), "size": {"x": 20.0, "y": 20.0}, "symbolData": {"symbolID": g(5, 1)}}),
    ));
    all.push(node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "Scaled card",
        json!({"size": {"x": 160.0, "y": 48.0}, "symbolData": {"symbolID": g(5, 10), "uniformScaleFactor": 0.8, "symbolOverrides": [
            {"guidPath": {"guids": [g(9, 11)]}, "overriddenSymbolID": g(5, 5)}
        ]}}),
    ));
    let doc = import_fig_node_changes(&Value::Array(all), "t").unwrap();
    let slot = child(&doc, find(&doc, "Scaled card"), "Slot");
    assert!(matches!(slot.kind, NodeKind::Instance { component: Some(c), .. } if c == find(&doc, "Icon with label").id), "the swap took place");
    let label = child(&doc, slot, "Label");
    let (size, lh) = drawn(&doc, label);
    assert!(close(size, 11.2) && close(lh, 16.0), "the swapped label draws at 0.8; drawn {size} / {lh}");
    assert!(close(label.transform.tx, 4.8) && close(label.transform.ty, 3.2), "its position scales too: {:?}", label.transform.translation());
}

#[test]
fn a_swap_inside_a_scaled_nested_instance_takes_both_scales() {
    let mut all = base();
    all.push(node((5, 1), (0, 2), "c", "SYMBOL", "Icon", json!({"size": {"x": 20.0, "y": 20.0}})));
    all.push(node((5, 5), (0, 2), "d", "SYMBOL", "Icon with label", json!({"size": {"x": 100.0, "y": 28.0}})));
    all.push(text((5, 6), (5, 5), "Label", (8, 3), json!({})));
    all.push(node((5, 20), (0, 2), "e", "SYMBOL", "Holder", json!({"size": {"x": 100.0, "y": 28.0}})));
    all.push(node(
        (5, 21),
        (5, 20),
        "a",
        "INSTANCE",
        "Slot",
        json!({"overrideKey": g(9, 21), "size": {"x": 20.0, "y": 20.0}, "symbolData": {"symbolID": g(5, 1)}}),
    ));
    all.push(node((5, 10), (0, 2), "f", "SYMBOL", "Card", json!({"size": {"x": 200.0, "y": 60.0}})));
    all.push(node(
        (5, 11),
        (5, 10),
        "a",
        "INSTANCE",
        "Half holder",
        json!({"overrideKey": g(9, 11), "size": {"x": 50.0, "y": 14.0}, "symbolData": {"symbolID": g(5, 20), "uniformScaleFactor": 0.5}}),
    ));
    all.push(node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "Scaled card",
        json!({"size": {"x": 160.0, "y": 48.0}, "symbolData": {"symbolID": g(5, 10), "uniformScaleFactor": 0.8, "symbolOverrides": [
            {"guidPath": {"guids": [g(9, 11), g(9, 21)]}, "overriddenSymbolID": g(5, 5)}
        ]}}),
    ));
    let doc = import_fig_node_changes(&Value::Array(all), "t").unwrap();
    let slot = child(&doc, child(&doc, find(&doc, "Scaled card"), "Half holder"), "Slot");
    assert!(matches!(slot.kind, NodeKind::Instance { component: Some(c), .. } if c == find(&doc, "Icon with label").id), "the swap took place");
    let label = child(&doc, slot, "Label");
    let (size, lh) = drawn(&doc, label);
    assert!(close(size, 5.6) && close(lh, 8.0), "0.8 x 0.5 of 14 / 20 is 5.6 / 8; drawn {size} / {lh}");
    assert!(close(label.transform.tx, 2.4) && close(label.transform.ty, 1.6), "its position takes both scales: {:?}", label.transform.translation());
}

/// The symbol overrides of the record named `name` in a decoded file.
fn saved_symbol_overrides(bytes: &[u8], name: &str) -> Vec<Value> {
    let v = decode_fig_to_json(bytes).unwrap();
    let rec = v["message"]["nodeChanges"].as_array().unwrap().iter().find(|r| r["name"] == name).unwrap_or_else(|| panic!("no record {name}")).clone();
    rec["symbolData"]["symbolOverrides"].as_array().cloned().unwrap_or_default()
}

#[test]
fn saving_without_edits_writes_no_text_override_the_file_did_not_have() {
    let doc = import_fig_node_changes(&badge_doc(vec![text((5, 2), (5, 1), "Rating", (8, 2), json!({}))], 0.8), "t").unwrap();
    let bytes = export_fig(&doc).unwrap();
    let saved = saved_symbol_overrides(&bytes, "Scaled badge");
    assert!(saved.is_empty(), "the source instance had no overrides; the save wrote {saved:?}");
}

#[test]
fn a_reopened_save_keeps_the_scaled_size() {
    let doc = import_fig_node_changes(&badge_doc(vec![text((5, 2), (5, 1), "Rating", (8, 2), json!({}))], 0.8), "t").unwrap();
    let back = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let rating = scaled_rating(&back);
    let (size, lh) = drawn(&back, rating);
    assert!(close(size, 11.2) && close(lh, 16.0), "after a save and reopen: drawn {size} / {lh}");
    assert!(rating.style_refs.contains_key("text"));
    assert!(rating.plugin_data.is_empty(), "the factor comes back as the text's own, not as plugin data: {:?}", rating.plugin_data);
    let again = import_fig(&export_fig(&back).unwrap()).unwrap();
    let (size, lh) = drawn(&again, scaled_rating(&again));
    assert!(close(size, 11.2) && close(lh, 16.0), "a second save and reopen does not scale again: drawn {size} / {lh}");
}

#[test]
fn a_native_save_keeps_the_factor() {
    let doc = import_fig_node_changes(&badge_doc(vec![text((5, 2), (5, 1), "Rating", (8, 2), json!({}))], 0.8), "t").unwrap();
    let back = Document::from_json(&doc.to_json().unwrap()).unwrap();
    let (size, lh) = drawn(&back, scaled_rating(&back));
    assert!(close(size, 11.2) && close(lh, 16.0), "after a native save and reopen: drawn {size} / {lh}");
    let plain = Document::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(text_of(child(&plain, find(&plain, "Badge"), "Rating")).uniform_scale, 1.0, "a main text has no factor");
}

/// A REST file with a component and an instance of it carrying `_uniformScale = scale`.
fn rest_with_scale(scale: Value) -> String {
    json!({
        "name": "scale",
        "document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "name": "P", "children": [
            {"id": "1:1", "name": "Badge", "type": "COMPONENT", "size": {"x": 100, "y": 28},
             "children": [
                {"id": "1:2", "name": "Rating", "type": "TEXT", "characters": "4.9",
                 "relativeTransform": [[1, 0, 6], [0, 1, 4]], "size": {"x": 88, "y": 20},
                 "style": {"fontFamily": "Inter", "fontSize": 14, "lineHeightPx": 20, "lineHeightUnit": "PIXELS", "letterSpacing": 0}}
             ]},
            {"id": "2:1", "name": "Scaled badge", "type": "INSTANCE", "componentId": "1:1", "size": {"x": 100, "y": 28},
             "_uniformScale": scale}
        ]}]}
    })
    .to_string()
}

#[test]
fn an_instance_scale_that_is_not_finite_and_positive_is_ignored() {
    for scale in [json!(1e40), json!(0.0), json!(-1.0)] {
        let doc = import_rest_json(&rest_with_scale(scale.clone())).unwrap();
        for n in doc.nodes.values() {
            let t = n.transform;
            let finite = [t.a, t.b, t.c, t.d, t.tx, t.ty, n.size.x, n.size.y].iter().all(|v| v.is_finite());
            assert!(finite, "scale {scale}: {} has {:?} / {:?}", n.name, n.transform, n.size);
            if let NodeKind::Text(tp) = &n.kind {
                let s = &tp.style;
                assert!([s.font_size, s.letter_spacing, s.paragraph_spacing, tp.paragraph.indent].iter().all(|v| v.is_finite()), "scale {scale}: {}", n.name);
                assert!(close(s.font_size, 14.0), "scale {scale}: an ignored factor leaves the text at 14, got {}", s.font_size);
            }
        }
    }
}
