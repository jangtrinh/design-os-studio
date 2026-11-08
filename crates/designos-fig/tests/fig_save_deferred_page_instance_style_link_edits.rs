//! Linking an unopened instance to another style, or detaching it from its style, is an edit even when the paints stay
//! as they were: a save follows the link the instance has now, not the link the file recorded.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

fn style_node(id: (u64, u64), name: &str, key: &str, style_type: &str, paint_key: &str, paint: Value) -> Value {
    fig_node(id, (0, 1), "RECTANGLE", name, json!({"styleType": style_type, "key": key, paint_key: [paint]}))
}

fn style_ref(key: &str) -> Value {
    json!({"assetRef": {"key": key, "version": "1:1"}})
}

fn shadow() -> Value {
    json!({"type": "DROP_SHADOW", "color": rgba(0.0, 0.0, 0.0), "offset": {"x": 0.0, "y": 4.0}, "radius": 8.0, "visible": true})
}

/// Two styles of the kind the link key names whose paints (or effects) are identical, each used by a layer on the
/// cover so that the importer creates both. The pending instance's own entry links the first.
fn fixture_linked_to(link_key: &str, style_type: &str, paint_key: &str) -> Vec<u8> {
    let paint = if paint_key == "effects" { shadow() } else { solid(&rgba(0.0, 1.0, 0.0), None) };
    let extra = vec![
        style_node((1, 60), "Style One", "STYLE_ONE", style_type, paint_key, paint.clone()),
        style_node((1, 61), "Style Two", "STYLE_TWO", style_type, paint_key, paint),
        fig_node((1, 63), (0, 1), "RECTANGLE", "user of two", json!({link_key: style_ref("STYLE_TWO")})),
    ];
    let paints_key = match link_key {
        "styleIdForFill" => "fillPaints",
        "styleIdForStrokeFill" => "strokePaints",
        _ => "effects",
    };
    let own_paint = if paints_key == "effects" { shadow() } else { solid(&rgba(0.0, 1.0, 0.0), None) };
    let own = json!({"guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]}, paints_key: [own_paint], link_key: style_ref("STYLE_ONE")});
    let fields = json!({"size": {"x": 130.0, "y": 48.0}, link_key: style_ref("STYLE_ONE")});
    let mut overrides = text_and_stroke_overrides();
    overrides.as_array_mut().expect("entries").push(own);
    fixture_with(fields, overrides, extra, Vec::new())
}

fn reopened(saved: &[u8]) -> Document {
    let mut doc = import_fig(saved).expect("reopen");
    doc.hydrate_all_pages();
    doc
}

fn style_id_named(doc: &Document, name: &str) -> String {
    doc.style_meta.iter().find(|(_, m)| m.name == name).map(|(k, _)| k.clone()).unwrap_or_else(|| panic!("no style {name}"))
}

/// The style the instance is linked to for `slot`, by the style's name.
fn linked_style(doc: &Document, slot: &str) -> Option<String> {
    let id = doc.get(variant_button(doc))?.style_refs.get(slot)?.clone();
    Some(doc.style_meta.get(&id)?.name.clone())
}

fn save_after(fig: &[u8], edit: impl FnOnce(&mut Document, NodeId)) -> Document {
    let mut live = import_fig(fig).expect("import");
    let button = variant_button(&live);
    edit(&mut live, button);
    reopened(&export_fig(&live).expect("export"))
}

fn assert_links_to(doc: &Document, slot: &str, name: &str) {
    let style = linked_style(doc, slot).unwrap_or_else(|| panic!("the {slot} link is gone"));
    assert!(style.contains(name), "linked to {style}, expected {name}");
}

#[test]
fn a_fill_style_link_nobody_touched_is_kept() {
    let fig = fixture_linked_to("styleIdForFill", "FILL", "fillPaints");
    let back = save_after(&fig, |_, _| {});
    assert_links_to(&back, "fill", "Style One");
}

#[test]
fn linking_to_another_fill_style_with_the_same_paints_survives_a_save() {
    let fig = fixture_linked_to("styleIdForFill", "FILL", "fillPaints");
    let back = save_after(&fig, |doc, button| {
        let two = style_id_named(doc, "Style Two");
        doc.get_mut(button).expect("instance").style_refs.insert("fill".into(), two);
    });
    assert_links_to(&back, "fill", "Style Two");
}

#[test]
fn detaching_a_fill_style_and_keeping_the_colour_survives_a_save() {
    let fig = fixture_linked_to("styleIdForFill", "FILL", "fillPaints");
    let back = save_after(&fig, |doc, button| {
        doc.get_mut(button).expect("instance").style_refs.remove("fill");
    });
    assert_eq!(linked_style(&back, "fill"), None);
    assert!(!back.get(variant_button(&back)).expect("instance").fills.is_empty(), "the colour stays");
}

#[test]
fn linking_to_another_stroke_style_with_the_same_paints_survives_a_save() {
    let fig = fixture_linked_to("styleIdForStrokeFill", "FILL", "fillPaints");
    let back = save_after(&fig, |doc, button| {
        let two = style_id_named(doc, "Style Two");
        doc.get_mut(button).expect("instance").style_refs.insert("stroke".into(), two);
    });
    assert_links_to(&back, "stroke", "Style Two");
}

#[test]
fn detaching_a_stroke_style_and_keeping_the_colour_survives_a_save() {
    let fig = fixture_linked_to("styleIdForStrokeFill", "FILL", "fillPaints");
    let back = save_after(&fig, |doc, button| {
        doc.get_mut(button).expect("instance").style_refs.remove("stroke");
    });
    assert_eq!(linked_style(&back, "stroke"), None);
}

/// The own entry of the saved file, as the file holds it: the entry itself tells a kept link from a restated one.
fn saved_own_entry(fig: &[u8], edit: impl FnOnce(&mut Document, NodeId)) -> Value {
    let mut live = import_fig(fig).expect("import");
    let button = variant_button(&live);
    edit(&mut live, button);
    let me = guid_text(&guid(BUTTON.0, BUTTON.1));
    saved_entries(&export_fig(&live).expect("export"), INSTANCE)
        .into_iter()
        .find(|e| e["guidPath"]["guids"].as_array().is_some_and(|g| g.len() == 1 && guid_text(&g[0]) == me))
        .expect("the own entry")
}

#[test]
fn an_effect_style_link_nobody_touched_is_kept() {
    let fig = fixture_linked_to("styleIdForEffect", "EFFECT", "effects");
    let own = saved_own_entry(&fig, |_, _| {});
    assert_eq!(own["styleIdForEffect"]["assetRef"]["key"], "STYLE_ONE");
}

#[test]
fn linking_to_another_effect_style_with_the_same_effects_names_the_new_style_in_the_entry() {
    let fig = fixture_linked_to("styleIdForEffect", "EFFECT", "effects");
    let own = saved_own_entry(&fig, |doc, button| {
        let two = style_id_named(doc, "Style Two");
        doc.get_mut(button).expect("instance").style_refs.insert("effect".into(), two);
    });
    let link = &own["styleIdForEffect"];
    assert!(link.get("assetRef").is_none_or(|r| r["key"] != "STYLE_ONE"), "the recorded link is still in the entry: {own}");
    assert!(link.get("guid").is_some_and(|g| g["sessionID"] != u32::MAX), "the entry names the new style, not the detach marker: {own}");
    assert!(own.get("effects").is_some(), "the effects are restated from the node: {own}");
}

#[test]
fn detaching_an_effect_style_and_keeping_the_effects_is_written_back() {
    let fig = fixture_linked_to("styleIdForEffect", "EFFECT", "effects");
    let own = saved_own_entry(&fig, |doc, button| {
        doc.get_mut(button).expect("instance").style_refs.remove("effect");
    });
    assert!(own.get("styleIdForEffect").is_none(), "the recorded link is still in the entry: {own}");
}
