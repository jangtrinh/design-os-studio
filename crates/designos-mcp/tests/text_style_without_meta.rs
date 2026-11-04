//! A loaded document whose text style has no `style_meta` entry still treats that style as a text
//! style: applying it through `set_style` without a `property` links the text slot, takes the
//! style's values, and leaves the layer's colour binding alone.
#![allow(clippy::result_large_err)]

mod support;

use designos_doc::{Document, NodeKind, Style, TextStyle};
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
