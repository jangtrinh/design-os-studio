//! Regression tests for typography bindings interacting with text styles.

use super::super::app::App;
use crate::canvas::textsession;
use crate::textedit::EditKind;
use designos_doc::*;

fn number_variable_doc(raw_font_size: f32, resolved_font_size: f32) -> (App, NodeId, String) {
    let mut d = Document::new("text-style-bindings");
    let page = d.add_page("Page");
    let id = d.alloc_id();
    let mut text = Node::text(id, Rect::new(0.0, 0.0, 120.0, 30.0), "Hello", TextStyle { font_size: raw_font_size, ..Default::default() });
    text.variables.bindings.insert("fontSize".into(), "size".into());
    let id = d.insert(page, text).unwrap();

    d.variable_collections.insert(
        "typography".into(),
        VariableCollection {
            id: "typography".into(),
            name: "Typography".into(),
            modes: vec![VariableMode { id: "default".into(), name: "Default".into() }],
            default_mode: "default".into(),
            variables: vec!["size".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert(
        "size".into(),
        Variable {
            id: "size".into(),
            name: "Size".into(),
            collection: "typography".into(),
            kind: VariableKind::Number,
            values: [("default".into(), VariableValue::Number(resolved_font_size))].into_iter().collect(),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    (App::new(d), id, "size".into())
}

fn add_body_style(app: &mut App, key: &str, font_size: f32) {
    app.doc.styles.insert(key.into(), Style::Text(TextStyle { font_size, ..Default::default() }));
    app.doc.style_meta.insert(key.into(), StyleMeta { name: key.into(), property: StyleProperty::Text, ..Default::default() });
}

fn link_text_style(app: &mut App, id: NodeId, key: &str) {
    app.doc.get_mut(id).unwrap().style_refs.insert("text".into(), key.into());
}

#[test]
fn applying_a_text_style_removes_owned_typography_bindings_in_the_same_undo_step() {
    let (mut app, id, _) = number_variable_doc(18.0, 32.0);
    add_body_style(&mut app, "Body", 60.0);
    app.doc.get_mut(id).unwrap().variables.bindings.insert("fills/0".into(), "unrelated-color".into());
    let before = app.doc.get(id).unwrap().clone();

    app.apply_style(&[id], "Body");

    let node = app.doc.get(id).unwrap();
    assert!(!node.variables.bindings.contains_key("fontSize"));
    assert_eq!(node.variables.bindings.get("fills/0"), Some(&"unrelated-color".to_string()));
    assert_eq!(node.kind.any_text().unwrap().style.font_size, 60.0);
    assert_eq!(resolve_node(&app.doc, node).kind.any_text().unwrap().style.font_size, 60.0);

    app.undo();
    assert_eq!(app.doc.get(id).unwrap(), &before, "undo restores the style and the removed binding");
}

#[test]
fn applying_a_text_style_does_not_touch_non_text_bindings_in_a_mixed_selection() {
    let (mut app, text_id, _) = number_variable_doc(18.0, 32.0);
    add_body_style(&mut app, "Body", 60.0);
    let page = app.doc.get(text_id).and_then(|n| n.parent).expect("text page");
    let rect_id = app.doc.alloc_id();
    let mut rect = Node::rectangle(rect_id, Rect::new(0.0, 40.0, 120.0, 30.0));
    rect.variables.bindings.insert("fontSize".into(), "size".into());
    app.doc.insert(page, rect).expect("rectangle inserted");

    app.apply_style(&[text_id, rect_id], "Body");

    assert_eq!(app.doc.get(rect_id).unwrap().variables.bindings.get("fontSize"), Some(&"size".to_string()));
}

#[test]
fn creating_a_text_style_uses_resolved_values_and_carries_shared_bindings() {
    let (mut app, id, variable) = number_variable_doc(18.0, 32.0);

    let key = app.create_style_from(id, StyleProperty::Text).expect("text style created");
    let Style::Text(style) = &app.doc.styles[&key] else { panic!("expected text style") };
    assert_eq!(style.font_size, 32.0, "the style must represent the value drawn in the active mode");
    assert_eq!(app.doc.style_meta[&key].bindings.get("fontSize"), Some(&variable));
    assert_eq!(app.doc.get(id).unwrap().style_refs.get("text"), Some(&key));
}

#[test]
fn creating_a_text_style_carries_bindings_inherited_from_the_linked_style() {
    let (mut app, id, variable) = number_variable_doc(18.0, 32.0);
    add_body_style(&mut app, "Body", 18.0);
    link_text_style(&mut app, id, "Body");
    app.doc.style_meta.get_mut("Body").unwrap().bindings.insert("fontSize".into(), variable.clone());

    let key = app.create_style_from(id, StyleProperty::Text).expect("text style created");

    assert_eq!(app.doc.style_meta[&key].bindings.get("fontSize"), Some(&variable));
    assert_eq!(resolve_node(&app.doc, app.doc.get(id).unwrap()).kind.any_text().unwrap().style.font_size, 32.0);
}

#[test]
fn creating_from_mixed_typography_bindings_is_refused() {
    let (mut app, id, _) = number_variable_doc(18.0, 32.0);
    let mut alternate = app.doc.variables["size"].clone();
    alternate.id = "alternate-size".into();
    alternate.name = "Alternate size".into();
    alternate.key = None;
    app.doc.variables.insert(alternate.id.clone(), alternate);

    let page = app.doc.get(id).and_then(|n| n.parent).expect("text page");
    let other_id = app.doc.alloc_id();
    let mut other = Node::text(other_id, Rect::new(0.0, 40.0, 120.0, 30.0), "World", TextStyle { font_size: 18.0, ..Default::default() });
    other.variables.bindings.insert("fontSize".into(), "alternate-size".into());
    app.doc.insert(page, other).expect("second text layer");

    assert!(app.create_text_style_from(&[id, other_id]).is_none(), "different maps cannot be represented by one style");
    for (node_id, expected) in [(id, "size"), (other_id, "alternate-size")] {
        let node = app.doc.get(node_id).unwrap();
        assert_eq!(node.variables.bindings.get("fontSize").map(String::as_str), Some(expected));
    }
}

#[test]
fn binding_a_text_style_ends_typing_before_the_binding_undo_step() {
    let (mut app, id, variable) = number_variable_doc(18.0, 32.0);
    add_body_style(&mut app, "Body", 60.0);
    link_text_style(&mut app, id, "Body");
    let before_typing = app.doc.get(id).unwrap().clone();

    textsession::begin_text_edit(&mut app, id);
    let end = app.doc.get(id).unwrap().kind.any_text().unwrap().content.len();
    {
        let session = app.canvas.text.as_mut().unwrap();
        session.caret = end;
        session.anchor = end;
    }
    textsession::insert_text(&mut app, "!", EditKind::Typing);
    assert!(app.canvas.text.is_some(), "the text edit remains open until the style binding starts");
    assert_eq!(app.doc.get(id).unwrap().kind.any_text().unwrap().content, "Hello!");

    assert!(app.bind_style_variable("Body", "fontSize", &variable));
    assert!(app.canvas.text.is_none(), "binding commits the active text edit first");
    assert_eq!(app.doc.style_meta["Body"].bindings.get("fontSize"), Some(&variable));

    app.undo();
    assert!(app.doc.style_meta["Body"].bindings.is_empty(), "first undo removes only the style binding");
    assert_eq!(app.doc.get(id).unwrap().kind.any_text().unwrap().content, "Hello!");
    app.undo();
    assert_eq!(app.doc.get(id).unwrap(), &before_typing, "second undo removes the committed text edit");
}

#[test]
fn updating_a_text_style_preserves_direct_typography_overrides() {
    let (mut app, id, _) = number_variable_doc(18.0, 32.0);
    add_body_style(&mut app, "Body", 60.0);
    link_text_style(&mut app, id, "Body");
    assert!(app.update_style_values("Body", Style::Text(TextStyle { font_size: 60.0, ..Default::default() })));
    let node = app.doc.get(id).unwrap();
    assert_eq!(node.variables.bindings.get("fontSize"), Some(&"size".to_string()));
    assert_eq!(node.kind.any_text().unwrap().style.font_size, 60.0);
    assert_eq!(resolve_node(&app.doc, node).kind.any_text().unwrap().style.font_size, 32.0);
}
