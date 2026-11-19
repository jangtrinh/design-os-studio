//! "Create text style" from a text in an instance made with Figma's scale tool. The text draws its style at the
//! instance's scale, so the new style holds the values at the main's scale and the text keeps drawing what it drew.
//! Hand-written kiwi-shaped fixtures; nothing here comes from a real file.

mod scaled_text_fixtures;

use designos_doc::*;
use designos_fig::import_fig_node_changes;
use scaled_text_fixtures::*;
use serde_json::json;

/// Creates a text style from `id`, applies it, and returns the new style.
fn create_style(doc: &mut Document, id: NodeId) -> TextStyle {
    let (key, cmd) = create_text_style_from_nodes(doc, &[id]).expect("one text style describes the text");
    cmd.apply(doc).unwrap();
    assert_eq!(doc.get(id).unwrap().style_refs.get("text"), Some(&key), "the text links to the new style");
    match doc.styles.get(&key) {
        Some(Style::Text(ts)) => ts.clone(),
        other => panic!("{other:?}"),
    }
}

fn assert_kept_and_style_unscaled(doc: &mut Document, id: NodeId) {
    let (size, lh) = drawn(doc, doc.get(id).unwrap());
    assert!(close(size, 11.2) && close(lh, 16.0), "before: drawn {size} / {lh}");
    let style = create_style(doc, id);
    assert!(close(style.font_size, 14.0) && close(px(style.line_height), 20.0), "the style holds 14 / 20, got {} / {:?}", style.font_size, style.line_height);
    let (size, lh) = drawn(doc, doc.get(id).unwrap());
    assert!(close(size, 11.2) && close(lh, 16.0), "creating a style must not change what is drawn: drawn {size} / {lh}");
}

#[test]
fn a_style_created_from_a_linked_scaled_text_holds_the_main_scale_values() {
    let mut doc = import_fig_node_changes(&badge_doc(vec![text((5, 2), (5, 1), "Rating", (8, 3), json!({}))], 0.8), "t").unwrap();
    let id = scaled_rating(&doc).id;
    assert_kept_and_style_unscaled(&mut doc, id);
}

#[test]
fn a_style_created_from_an_unlinked_scaled_text_holds_the_main_scale_values() {
    let mut rating = text((5, 2), (5, 1), "Rating", (8, 3), json!({}));
    rating.as_object_mut().unwrap().remove("styleIdForText");
    let mut doc = import_fig_node_changes(&badge_doc(vec![rating], 0.8), "t").unwrap();
    let id = scaled_rating(&doc).id;
    assert!(!doc.get(id).unwrap().style_refs.contains_key("text"));
    assert_kept_and_style_unscaled(&mut doc, id);
}

#[test]
fn a_style_created_after_detaching_the_scaled_instance_holds_the_drawn_values() {
    let mut doc = import_fig_node_changes(&badge_doc(vec![text((5, 2), (5, 1), "Rating", (8, 3), json!({}))], 0.8), "t").unwrap();
    let inst = find(&doc, "Scaled badge").id;
    detach_instance(&mut doc, inst).apply(&mut doc).unwrap();
    assert!(!matches!(doc.get(inst).unwrap().kind, NodeKind::Instance { .. }), "the instance is detached");
    let id = scaled_rating(&doc).id;
    assert_eq!(text_of(doc.get(id).unwrap()).uniform_scale, 1.0, "the detached text has no factor");
    let (size, lh) = drawn(&doc, doc.get(id).unwrap());
    assert!(close(size, 11.2) && close(lh, 16.0), "before: drawn {size} / {lh}");
    let style = create_style(&mut doc, id);
    assert!(
        close(style.font_size, 11.2) && close(px(style.line_height), 16.0),
        "the style holds the drawn 11.2 / 16, got {} / {:?}",
        style.font_size,
        style.line_height
    );
    let (size, lh) = drawn(&doc, doc.get(id).unwrap());
    assert!(close(size, 11.2) && close(lh, 16.0), "creating a style must not change what is drawn: drawn {size} / {lh}");
}

#[test]
fn indent_and_list_spacing_are_not_scaled_twice_by_a_created_style() {
    let rating = text((5, 2), (5, 1), "Rating", (8, 3), json!({"paragraphIndent": 10.0, "listSpacing": 6.0}));
    let mut doc = import_fig_node_changes(&badge_doc(vec![rating], 0.8), "t").unwrap();
    let id = scaled_rating(&doc).id;
    let before = text_of(doc.get(id).unwrap()).paragraph.clone();
    assert!(close(before.indent, 8.0) && close(before.list_spacing, 4.8), "imported at 0.8: {} / {}", before.indent, before.list_spacing);
    let style = create_style(&mut doc, id);
    assert!(
        close(style.paragraph_indent, 10.0) && close(style.list_spacing, 6.0),
        "the style holds 10 / 6, got {} / {}",
        style.paragraph_indent,
        style.list_spacing
    );
    let after = &text_of(doc.get(id).unwrap()).paragraph;
    assert!(close(after.indent, 8.0) && close(after.list_spacing, 4.8), "the text keeps 8 / 4.8, got {} / {}", after.indent, after.list_spacing);
}
