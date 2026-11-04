//! A loaded document whose text style has no `style_meta` entry still treats that style as a text
//! style: applying it through `set_style` without a `property` links the text slot, takes the
//! style's values, and leaves the layer's colour binding alone.
#![allow(clippy::result_large_err)]

mod support;

use designos_doc::{style_property, Document, LeadingTrim, NodeId, NodeKind, Style, StyleProperty, TextStyle};
use designos_mcp::engine::Engine;
use serde_json::json;
use support::*;

/// An engine whose document went through a save and a load with a `Body` text style that has no metadata.
fn engine_with_unmetered_text_style() -> Engine {
    let mut e = palette();
    e.doc.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 21.0, ..Default::default() }));
    assert!(e.doc.style_meta.is_empty(), "the fixture must carry no style metadata");
    let saved = e.doc.to_json().unwrap();
    let (loaded, report) = Document::from_json_with_report(&saved).unwrap();
    assert_eq!(report, Default::default(), "loading must not count as a repair, or the server would refuse the file");
    e.doc = loaded;
    e
}

#[test]
fn set_style_apply_without_property_on_a_loaded_text_style_keeps_the_binding_and_applies_the_text_style() {
    let mut e = engine_with_unmetered_text_style();
    let id = bound_text(&mut e);
    assert_bound_to(&e, id, "color/primary", PRIMARY);

    e.call("designos_apply_batch", json!({"ops": [{"op": "set_style", "apply": [{"node": id.0, "style": "Body"}]}]})).unwrap();

    let n = node(&e, id);
    assert_eq!(n.style_refs.get("text").map(String::as_str), Some("Body"), "the text slot is linked: {:?}", n.style_refs);
    assert!(!n.style_refs.contains_key("fill"), "the style is not linked as a fill: {:?}", n.style_refs);
    let NodeKind::Text(t) = &n.kind else { panic!("not text") };
    assert_eq!(t.style.font_size, 21.0, "the style's values reach the layer");
    assert_bound_to(&e, id, "color/primary", PRIMARY);
}

/// A text layer linked to `key` that carries paragraph values of its own.
fn consumer_of(e: &mut Engine, key: &str) -> NodeId {
    let id = bound_text(e);
    let n = e.doc.get_mut(id).unwrap();
    n.style_refs.insert("text".into(), key.into());
    let t = n.kind.any_text_mut().unwrap();
    t.paragraph.indent = 12.0;
    t.paragraph.list_spacing = 7.0;
    t.paragraph.leading_trim = LeadingTrim::CapHeight;
    id
}

fn paragraph_of(e: &Engine, id: NodeId) -> (f32, f32, LeadingTrim) {
    let t = node(e, id).kind.any_text().unwrap();
    (t.paragraph.indent, t.paragraph.list_spacing, t.paragraph.leading_trim)
}

fn upsert(e: &mut Engine, key: &str, style: TextStyle) {
    let style = serde_json::to_value(style).unwrap();
    e.call("designos_apply_batch", json!({"ops": [{"op": "set_style", "upsert": [{"key": key, "text_style": style}]}]})).unwrap();
}

#[test]
fn set_style_upsert_on_a_text_style_without_metadata_keeps_consumer_paragraph_values() {
    let mut e = palette();
    e.doc.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 21.0, ..Default::default() }));
    let id = consumer_of(&mut e, "Body");

    upsert(&mut e, "Body", TextStyle { font_size: 30.0, ..Default::default() });

    let NodeKind::Text(t) = &node(&e, id).kind else { panic!("not text") };
    assert_eq!(t.style.font_size, 30.0, "the new values reach the linked layer");
    assert_eq!(paragraph_of(&e, id), (12.0, 7.0, LeadingTrim::CapHeight), "paragraph values the style never held stay with the layer");
    assert_eq!(e.doc.style_name("Body"), "Body");
    assert_eq!(style_property(&e.doc, "Body"), StyleProperty::Text);
}

#[test]
fn set_style_upsert_on_a_text_style_without_metadata_promotes_only_the_changed_paragraph_field() {
    let mut e = palette();
    e.doc.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 21.0, ..Default::default() }));
    let id = consumer_of(&mut e, "Body");

    upsert(&mut e, "Body", TextStyle { font_size: 21.0, paragraph_indent: 3.0, ..Default::default() });

    assert_eq!(paragraph_of(&e, id), (3.0, 7.0, LeadingTrim::CapHeight));
    assert_eq!(e.doc.style_meta["Body"].text_paragraph_fields, Some(0b001));
}

#[test]
fn set_style_upsert_on_a_loaded_orphan_text_style_keeps_consumer_paragraph_values() {
    let mut e = palette();
    e.doc.styles.insert("S:abc".into(), Style::Text(TextStyle { font_size: 21.0, ..Default::default() }));
    let (loaded, _) = Document::from_json_with_report(&e.doc.to_json().unwrap()).unwrap();
    e.doc = loaded;
    assert!(e.doc.is_orphan_style("S:abc"));
    let id = consumer_of(&mut e, "S:abc");

    upsert(&mut e, "S:abc", TextStyle { font_size: 30.0, ..Default::default() });

    assert_eq!(paragraph_of(&e, id), (12.0, 7.0, LeadingTrim::CapHeight));
}

#[test]
fn a_text_style_created_by_the_upsert_owns_all_paragraph_fields() {
    let mut e = palette();
    let id = consumer_of(&mut e, "Fresh");
    let style = serde_json::to_value(TextStyle { font_size: 30.0, paragraph_indent: 5.0, ..Default::default() }).unwrap();

    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_style", "upsert": [{"key": "Fresh", "text_style": style}], "apply": [{"node": id.0, "style": "Fresh"}]}]}),
    )
    .unwrap();

    assert_eq!(paragraph_of(&e, id), (5.0, 0.0, LeadingTrim::None), "a new style supplies every paragraph value");
    assert_eq!(e.doc.style_meta["Fresh"].text_paragraph_fields, Some(0b111));
}
