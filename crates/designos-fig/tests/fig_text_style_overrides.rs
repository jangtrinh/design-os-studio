//! Figma lets a text layer keep its text style while it overrides only the weight, the italic font style,
//! the text decoration or a hyperlink. In a `.fig` those overrides are style override table entries with
//! `isOverrideOverTextStyle`. Everything else a linked layer carries (its own size, line height, ...) is
//! stale: Figma renders the style, so the import takes the style and finds no override there.
use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn body() -> TextStyle {
    TextStyle { font_size: 16.0, font_weight: 400, ..Default::default() }
}

/// A page with the text style "Body" (16 px, 400) and three layers linked to it: "Plain" follows it,
/// "Whole" is bold and underlined as a layer, "Range" has a bold range over its first two characters.
fn linked_doc() -> Document {
    let mut doc = Document::new("TextStyleOverrides");
    let page = doc.add_page("Page 1");
    doc.styles.insert("Body".into(), Style::Text(body()));
    doc.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), property: StyleProperty::Text, ..Default::default() });
    for (name, y) in [("Plain", 0.0), ("Whole", 40.0), ("Range", 80.0)] {
        let id = doc.alloc_id();
        let mut n = Node::text(id, Rect::new(0.0, y, 200.0, 30.0), "Hello", body());
        n.name = name.into();
        n.style_refs.insert("text".into(), "Body".into());
        if let NodeKind::Text(t) = &mut n.kind {
            match name {
                "Whole" => {
                    t.style.set_font_weight(700);
                    t.style.decoration = TextDecoration::Underline;
                    n.text_style_overrides.insert(OwnedField::Weight);
                    n.text_style_overrides.insert(OwnedField::Decoration);
                }
                "Range" => t.runs.push(TextRun { start: 0, end: 2, style: TextStyle { font_weight: 700, ..body() } }),
                _ => {}
            }
        }
        doc.insert(page, n).unwrap();
    }
    doc
}

fn by_name<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Text(_))).unwrap_or_else(|| panic!("text layer {name}"))
}

/// The weight and decoration each character is drawn with.
fn drawn(n: &Node) -> Vec<(u16, TextDecoration)> {
    let t = n.kind.any_text().unwrap();
    (0..t.content.len())
        .map(|i| {
            let s = t.runs.iter().find(|r| r.start <= i && i < r.end).map(|r| &r.style).unwrap_or(&t.style);
            (s.font_weight, s.decoration)
        })
        .collect()
}

fn effective(doc: &Document, name: &str) -> Vec<&'static str> {
    text_style_overrides(doc, by_name(doc, name)).labels()
}

#[test]
fn a_layer_wide_and_a_range_override_survive_export_and_reimport_with_the_link() {
    let original = linked_doc();
    let imported = import_fig(&export_fig(&original).expect("export_fig")).expect("import_fig");

    for name in ["Plain", "Whole", "Range"] {
        assert!(by_name(&imported, name).style_refs.contains_key("text"), "{name} keeps its text style link");
    }
    assert!(effective(&imported, "Plain").is_empty(), "a layer that follows its style has no overrides");
    assert_eq!(effective(&imported, "Whole"), vec!["Weight", "Decoration"]);
    assert_eq!(effective(&imported, "Range"), vec!["Weight"]);
    for name in ["Whole", "Range"] {
        assert_eq!(drawn(by_name(&imported, name)), drawn(by_name(&original, name)), "{name}: every character is drawn as before");
    }

    // A second round changes nothing.
    let again = import_fig(&export_fig(&imported).expect("second export")).expect("second import");
    for name in ["Plain", "Whole", "Range"] {
        assert_eq!(effective(&again, name), effective(&imported, name), "{name} is stable");
        assert_eq!(drawn(by_name(&again, name)), drawn(by_name(&imported, name)));
    }
}

#[test]
fn the_file_carries_the_overrides_as_flagged_table_entries_and_nothing_studio_only() {
    let bytes = export_fig(&linked_doc()).expect("export_fig");
    let v = decode_fig_to_json(&bytes).expect("decode_fig_to_json");
    let changes = v["message"]["nodeChanges"].as_array().expect("nodeChanges");
    let named = |name: &str| changes.iter().find(|n| n["name"] == name && n["type"] == "TEXT").unwrap_or_else(|| panic!("no text change {name}"));

    let table = |name: &str| named(name)["textData"]["styleOverrideTable"].as_array().cloned().unwrap_or_default();
    assert!(table("Plain").is_empty(), "no override, no entry");
    let whole = table("Whole");
    assert_eq!(whole.len(), 1);
    assert_eq!(whole[0]["isOverrideOverTextStyle"], true);
    assert_eq!(whole[0]["semanticWeight"], "BOLD");
    assert_eq!(whole[0]["textDecoration"], "UNDERLINE");
    assert!(whole[0].get("fontSize").is_none(), "an override entry holds the overridden properties only");
    let range = table("Range");
    assert_eq!((range.len(), &range[0]["isOverrideOverTextStyle"], &range[0]["semanticWeight"]), (1, &json!(true), &json!("BOLD")));
    assert_eq!(named("Whole")["textData"]["characterStyleIDs"], json!([1, 1, 1, 1, 1]));
    assert_eq!(named("Range")["textData"]["characterStyleIDs"], json!([1, 1, 0, 0, 0]));

    let studio_only = changes
        .iter()
        .any(|n| n["pluginData"].as_array().is_some_and(|p| p.iter().any(|e| e["key"].as_str().is_some_and(|k| k.to_lowercase().contains("overrid")))));
    assert!(!studio_only, "no Studio-only plugin data for the overrides");
}

/// One linked layer "Mixed": the layer-wide override `layer` (applied to the base and stored) plus one
/// range over the first two characters that changes `word` on top of it.
fn mixed_doc(layer: fn(&mut TextStyle), stored: &[OwnedField], word: fn(&mut TextStyle)) -> Document {
    let mut doc = linked_doc();
    let id = by_name(&doc, "Plain").id;
    let n = doc.get_mut(id).unwrap();
    n.name = "Mixed".into();
    let t = n.kind.any_text_mut().unwrap();
    layer(&mut t.style);
    let mut run = t.style.clone();
    word(&mut run);
    t.runs.push(TextRun { start: 0, end: 2, style: run });
    for f in stored {
        n.text_style_overrides.insert(*f);
    }
    doc
}

fn drawn_full(n: &Node) -> Vec<(u16, bool, TextDecoration)> {
    let t = n.kind.any_text().unwrap();
    (0..t.content.len())
        .map(|i| {
            let s = t.runs.iter().find(|r| r.start <= i && i < r.end).map(|r| &r.style).unwrap_or(&t.style);
            (s.font_weight, s.italic, s.decoration)
        })
        .collect()
}

fn assert_mixed_round_trip(doc: Document, labels: Vec<&'static str>) {
    let want = drawn_full(by_name(&doc, "Mixed"));
    let back = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    let n = by_name(&back, "Mixed");
    assert!(n.style_refs.contains_key("text"), "the link stays");
    assert_eq!(effective(&back, "Mixed"), labels, "same overrides, same count");
    assert_eq!(drawn_full(n), want, "every character keeps its weight, italic and decoration");
}

#[test]
fn a_bold_layer_with_an_underlined_word_keeps_both_through_a_round_trip() {
    assert_mixed_round_trip(
        mixed_doc(|s| s.set_font_weight(700), &[OwnedField::Weight], |s| s.decoration = TextDecoration::Underline),
        vec!["Weight", "Decoration"],
    );
}

#[test]
fn an_underlined_layer_with_a_bold_word_keeps_both_through_a_round_trip() {
    assert_mixed_round_trip(
        mixed_doc(|s| s.decoration = TextDecoration::Underline, &[OwnedField::Decoration], |s| s.set_font_weight(700)),
        vec!["Weight", "Decoration"],
    );
}

#[test]
fn an_unbolded_word_inside_a_bold_layer_stays_regular_through_a_round_trip() {
    assert_mixed_round_trip(mixed_doc(|s| s.set_font_weight(700), &[OwnedField::Weight], |s| s.set_font_weight(400)), vec!["Weight"]);
}

#[test]
fn an_italic_layer_with_an_underlined_word_keeps_both_through_a_round_trip() {
    assert_mixed_round_trip(mixed_doc(|s| s.italic = true, &[OwnedField::Italic], |s| s.decoration = TextDecoration::Underline), vec!["Italic", "Decoration"]);
}

#[test]
fn a_base_value_that_differs_from_the_linked_style_is_stale_not_an_override() {
    let mut doc = linked_doc();
    let id = by_name(&doc, "Plain").id;
    doc.get_mut(id).unwrap().kind.any_text_mut().unwrap().style.font_size = 24.0;
    let imported = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    let plain = by_name(&imported, "Plain");
    assert!(effective(&imported, "Plain").is_empty(), "the style is authoritative: no override is invented");
    assert_eq!(plain.kind.any_text().unwrap().style.font_size, 16.0);
}

fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": g(id.0, id.1), "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
        "size": {"x": 10.0, "y": 10.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

#[test]
fn a_figma_file_with_an_override_over_a_text_style_shows_the_override_and_no_false_one() {
    let style = json!({"guid": g(7, 1)});
    let input = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        node(
            (7, 1),
            (0, 2),
            "a",
            "TEXT",
            "Body",
            json!({"styleType": "TEXT", "fontSize": 16.0, "fontName": {"family": "Inter", "style": "Regular"}, "lineHeight": {"units": "PIXELS", "value": 24.0}})
        ),
        // A linked text with a stale node size (48) and a bold-underline override over the whole text.
        node(
            (1, 1),
            (0, 1),
            "a",
            "TEXT",
            "Overridden",
            json!({
                "fontSize": 48.0, "fontName": {"family": "Inter", "style": "Regular"}, "styleIdForText": style,
                "textData": {"characters": "Hello", "characterStyleIDs": [1, 1, 1, 1, 1],
                    "styleOverrideTable": [{"styleID": 1, "isOverrideOverTextStyle": true, "semanticWeight": "BOLD",
                        "fontName": {"family": "Inter", "style": "Bold"}, "textDecoration": "UNDERLINE", "hyperlink": {"url": "https://example.com"}}]}
            })
        ),
        // A linked text with only stale node values.
        node(
            (1, 2),
            (0, 1),
            "b",
            "TEXT",
            "Stale",
            json!({"fontSize": 48.0, "fontName": {"family": "Inter", "style": "Bold"}, "styleIdForText": style, "textData": {"characters": "Hello"}})
        )
    ]);
    let doc = import_fig_node_changes(&input, "t").unwrap();
    assert_eq!(effective(&doc, "Overridden"), vec!["Weight", "Decoration", "Hyperlink"]);
    assert_eq!(by_name(&doc, "Overridden").hyperlink.as_deref(), Some("https://example.com"), "the link in the flagged entry is kept");
    assert!(by_name(&doc, "Overridden").style_refs.contains_key("text"));
    assert_eq!(drawn(by_name(&doc, "Overridden"))[0], (700, TextDecoration::Underline));
    assert_eq!(by_name(&doc, "Overridden").kind.any_text().unwrap().style.font_size, 16.0, "the style supplies the size");
    assert!(effective(&doc, "Stale").is_empty());
    assert_eq!(by_name(&doc, "Stale").kind.any_text().unwrap().style.font_weight, 400);
}
