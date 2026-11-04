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
