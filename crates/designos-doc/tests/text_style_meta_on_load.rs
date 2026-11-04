//! A text style stored without `style_meta` keeps its own name and its text property when the
//! document is loaded or the style is edited, and a style that is only an orphan reference never
//! becomes a table style by being read.

use designos_doc::*;

const ORPHAN_KEYS: [&str; 3] = ["S:abc", "12:34", "key:xyz"];

fn body() -> Style {
    Style::Text(TextStyle { font_size: 14.0, ..Default::default() })
}

/// A saved document whose `styles` map holds text styles and no `style_meta` at all.
fn saved_without_style_meta() -> String {
    let mut d = Document::new("t");
    d.styles.insert("Body".into(), body());
    for key in ORPHAN_KEYS {
        d.styles.insert(key.into(), body());
    }
    assert!(d.style_meta.is_empty(), "the fixture must carry no style metadata");
    d.to_json().unwrap()
}

#[test]
fn a_loaded_text_style_without_metadata_is_named_by_its_key_and_is_a_text_style() {
    let loaded = Document::from_json(&saved_without_style_meta()).unwrap();

    assert_eq!(loaded.style_name("Body"), "Body");
    assert_eq!(style_property(&loaded, "Body"), StyleProperty::Text);
    assert_eq!(loaded.style_meta["Body"].property, StyleProperty::Text);
    assert!(!loaded.is_orphan_style("Body"));
    assert!(loaded.is_table_style("Body"));
    assert!(!loaded.is_unnamed_style("Body"));
}

#[test]
fn an_orphan_text_style_stays_an_orphan_after_load() {
    let loaded = Document::from_json(&saved_without_style_meta()).unwrap();

    for key in ORPHAN_KEYS {
        assert!(loaded.is_orphan_style(key), "{key} must still be an orphan");
        assert!(!loaded.style_meta.contains_key(key), "{key} must not gain metadata from being read");
        assert_eq!(loaded.style_name(key), key);
    }
}

/// A page with one text layer linked to `key`, carrying paragraph values of its own.
fn doc_with_consumer(key: &str, style: Style) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("Page");
    let id = d.alloc_id();
    let mut text = Node::text(id, Rect::new(0.0, 0.0, 100.0, 40.0), "hello", TextStyle::default());
    text.style_refs.insert("text".into(), key.into());
    {
        let t = text.kind.any_text_mut().unwrap();
        t.paragraph.indent = 12.0;
        t.paragraph.list_spacing = 7.0;
        t.paragraph.leading_trim = LeadingTrim::CapHeight;
    }
    d.insert(page, text).unwrap();
    d.styles.insert(key.into(), style);
    assert!(d.style_meta.is_empty());
    (d, id)
}

fn paragraph_of(d: &Document, id: NodeId) -> (f32, f32, LeadingTrim) {
    let t = d.get(id).unwrap().kind.any_text().unwrap();
    (t.paragraph.indent, t.paragraph.list_spacing, t.paragraph.leading_trim)
}

#[test]
fn editing_a_text_style_without_metadata_keeps_consumer_paragraph_values_and_the_style_identity() {
    let (mut d, id) = doc_with_consumer("Body", body());

    let bigger = Style::Text(TextStyle { font_size: 18.0, ..Default::default() });
    update_style(&d, "Body", bigger).unwrap().apply(&mut d).unwrap();

    let t = d.get(id).unwrap().kind.any_text().unwrap();
    assert_eq!(t.style.font_size, 18.0, "the edit reaches the linked layer");
    assert_eq!(paragraph_of(&d, id), (12.0, 7.0, LeadingTrim::CapHeight), "paragraph values the style never stored stay with the layer");
    assert_eq!(d.style_name("Body"), "Body");
    assert_eq!(style_property(&d, "Body"), StyleProperty::Text);
}

#[test]
fn changing_one_paragraph_field_of_a_style_without_metadata_promotes_only_that_field() {
    let (mut d, id) = doc_with_consumer("Body", body());

    let indented = Style::Text(TextStyle { font_size: 14.0, paragraph_indent: 3.0, ..Default::default() });
    update_style(&d, "Body", indented).unwrap().apply(&mut d).unwrap();

    assert_eq!(paragraph_of(&d, id), (3.0, 7.0, LeadingTrim::CapHeight));
    assert_eq!(d.style_meta["Body"].text_paragraph_fields, Some(0b001));
}

#[test]
fn editing_an_orphan_text_style_gives_it_no_metadata() {
    let (mut d, id) = doc_with_consumer("S:abc", body());

    let bigger = Style::Text(TextStyle { font_size: 18.0, ..Default::default() });
    update_style(&d, "S:abc", bigger).unwrap().apply(&mut d).unwrap();

    assert_eq!(paragraph_of(&d, id), (12.0, 7.0, LeadingTrim::CapHeight));
    assert!(d.is_orphan_style("S:abc"));
    assert!(!d.style_meta.contains_key("S:abc"));
}

#[test]
fn editing_a_fill_style_without_metadata_gives_it_no_metadata() {
    let mut d = Document::new("t");
    d.styles.insert("Brand".into(), Style::Fill(vec![Paint::solid(Color::rgb(255, 0, 0))]));

    let blue = Style::Fill(vec![Paint::solid(Color::rgb(0, 0, 255))]);
    update_style(&d, "Brand", blue).unwrap().apply(&mut d).unwrap();

    assert!(!d.style_meta.contains_key("Brand"));
    assert_eq!(style_property(&d, "Brand"), StyleProperty::Fill);
}

/// A saved document with text style `Body`, whose JSON `Text` object is edited by `edit`.
fn saved_body_with(edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>)) -> String {
    let mut d = Document::new("t");
    d.styles.insert("Body".into(), body());
    let mut json: serde_json::Value = serde_json::from_str(&d.to_json().unwrap()).unwrap();
    edit(json["styles"]["Body"]["Text"].as_object_mut().unwrap());
    json.to_string()
}

fn paragraph_bits_after_load(json: &str) -> Option<u8> {
    Document::from_json(json).unwrap().style_meta["Body"].text_paragraph_fields
}

#[test]
fn each_paragraph_field_the_text_style_json_holds_is_recorded_on_load() {
    assert_eq!(paragraph_bits_after_load(&saved_body_with(|_| {})), Some(0));
    for (key, value, bits) in [
        ("paragraph_indent", serde_json::json!(0.0), 0b001),
        ("list_spacing", serde_json::json!(0.0), 0b010),
        ("leading_trim", serde_json::json!("None"), 0b100),
    ] {
        let json = saved_body_with(|text| {
            text.insert(key.into(), value);
        });
        assert_eq!(paragraph_bits_after_load(&json), Some(bits), "{key} held as an explicit default");
    }
    let all = saved_body_with(|text| {
        text.insert("paragraph_indent".into(), serde_json::json!(4.0));
        text.insert("list_spacing".into(), serde_json::json!(2.0));
        text.insert("leading_trim".into(), serde_json::json!("CapHeight"));
    });
    assert_eq!(paragraph_bits_after_load(&all), Some(0b111));
}

#[test]
fn a_later_load_never_inherits_the_paragraph_fields_of_an_earlier_one() {
    let with_indent = saved_body_with(|text| {
        text.insert("paragraph_indent".into(), serde_json::json!(4.0));
    });
    assert_eq!(paragraph_bits_after_load(&with_indent), Some(0b001));

    assert_eq!(paragraph_bits_after_load(&saved_body_with(|_| {})), Some(0), "the next document has its own fields");
}

#[test]
fn a_null_number_in_a_text_style_is_repaired_and_the_paragraph_fields_are_still_recorded() {
    let json = saved_body_with(|text| {
        text.insert("font_size".into(), serde_json::Value::Null);
        text.insert("list_spacing".into(), serde_json::json!(2.0));
    });

    let (doc, report) = Document::from_json_with_report(&json).unwrap();

    assert_eq!(report.non_finite_numbers, 1);
    assert_eq!(doc.style_meta["Body"].text_paragraph_fields, Some(0b010));
    assert!(matches!(&doc.styles["Body"], Style::Text(t) if t.list_spacing == 2.0));
}

#[test]
fn styles_of_every_kind_load_and_an_unknown_kind_is_refused_by_name() {
    let mut d = Document::new("t");
    d.styles.insert("Fill".into(), Style::Fill(vec![Paint::solid(Color::rgb(1, 2, 3))]));
    d.styles.insert("Text".into(), body());
    d.styles.insert("Effect".into(), Style::Effect(vec![]));
    d.styles.insert("Grid".into(), Style::Grid(vec![]));
    let saved = d.to_json().unwrap();
    let loaded = Document::from_json(&saved).unwrap();
    assert_eq!(loaded.styles, d.styles);

    let unknown = saved.replace("\"Effect\":[]", "\"Bogus\":[]");
    let err = Document::from_json(&unknown).unwrap_err().to_string();
    assert!(err.contains("unknown variant `Bogus`"), "{err}");
}
