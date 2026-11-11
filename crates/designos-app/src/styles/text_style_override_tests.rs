//! A text layer keeps its text style link while it overrides the weight, italic or decoration of the
//! style, as in Figma. The layer records them, a style edit leaves them alone, "Reapply style" clears
//! them and an edit of any other style-owned property detaches the style.

use crate::app::App;
use designos_doc::*;

fn number_variable(id: &str, default: f32, dark: f32) -> Variable {
    Variable {
        id: id.into(),
        name: id.into(),
        collection: "typo".into(),
        kind: VariableKind::Number,
        values: [("default".to_string(), VariableValue::Number(default)), ("dark".to_string(), VariableValue::Number(dark))].into_iter().collect(),
        description: String::new(),
        scopes: Vec::new(),
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

/// Two text layers of 16 px, weight 400, linked to the style "Body" (the same type): `a` (returned first)
/// and `b`, the second inside a frame set to the Dark mode of the collection "typo" that holds the numbers
/// `size`, `lh` and `weight`.
fn linked_layers() -> (App, NodeId, NodeId) {
    let mut d = Document::new("overrides");
    let page = d.add_page("Page");
    let mut ids = Vec::new();
    for dark in [false, true] {
        let fid = d.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, if dark { 100.0 } else { 0.0 }, 300.0, 90.0));
        if dark {
            frame.variables.modes.insert("typo".into(), "dark".into());
        }
        let fid = d.insert(page, frame).unwrap();
        let id = d.alloc_id();
        let mut text = Node::text(id, Rect::new(0.0, 0.0, 120.0, 30.0), "Hello", TextStyle { font_size: 16.0, ..Default::default() });
        text.style_refs.insert("text".into(), "Body".into());
        ids.push(d.insert(fid, text).unwrap());
    }
    d.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 16.0, ..Default::default() }));
    d.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), property: StyleProperty::Text, ..Default::default() });
    d.variable_collections.insert(
        "typo".into(),
        VariableCollection {
            id: "typo".into(),
            name: "Typo".into(),
            modes: vec![VariableMode { id: "default".into(), name: "Default".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "default".into(),
            variables: vec!["size".into(), "lh".into(), "weight".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert("size".into(), number_variable("size", 16.0, 32.0));
    d.variables.insert("lh".into(), number_variable("lh", 20.0, 40.0));
    d.variables.insert("weight".into(), number_variable("weight", 700.0, 800.0));
    (App::new(d), ids[0], ids[1])
}

fn overrides(app: &App, id: NodeId) -> Vec<&'static str> {
    text_style_overrides(&app.doc, app.doc.get(id).unwrap()).labels()
}

fn style_of(app: &App, id: NodeId) -> TextStyle {
    app.doc.get(id).unwrap().kind.any_text().unwrap().style.clone()
}

fn linked(app: &App, id: NodeId) -> bool {
    app.doc.get(id).unwrap().style_refs.get("text").map(String::as_str) == Some("Body")
}

fn edit(app: &mut App, id: NodeId, f: impl Fn(&mut TextStyle)) {
    crate::panels::typography::set_style(app, &[id], f);
}

#[test]
fn a_layer_that_follows_its_style_has_no_overrides() {
    let (app, a, _) = linked_layers();
    assert!(overrides(&app, a).is_empty());
}

#[test]
fn weight_italic_and_decoration_keep_the_link_record_overrides_and_are_one_undo_step() {
    for (what, f) in [
        ("Weight", (|s: &mut TextStyle| s.set_font_weight(700)) as fn(&mut TextStyle)),
        ("Italic", |s| s.italic = true),
        ("Decoration", |s| s.decoration = TextDecoration::Underline),
    ] {
        let (mut app, a, _) = linked_layers();
        let before = app.doc.get(a).unwrap().clone();
        edit(&mut app, a, f);
        assert!(linked(&app, a), "{what} keeps the link");
        assert_eq!(overrides(&app, a), vec![what]);
        app.undo();
        assert_eq!(app.doc.get(a).unwrap(), &before, "{what}: one undo restores the value and the empty set");
        app.redo();
        assert_eq!(overrides(&app, a), vec![what]);
    }
}

#[test]
fn every_other_style_owned_edit_detaches_the_style_and_keeps_the_values() {
    for (what, f) in [
        ("size", (|s: &mut TextStyle| s.font_size = 24.0) as fn(&mut TextStyle)),
        ("line height", |s| s.line_height = LineHeight::Pixels(30.0)),
        ("letter spacing", |s| s.letter_spacing = 2.0),
        ("paragraph spacing", |s| s.paragraph_spacing = 9.0),
        ("family", |s| s.font_family = "Serif".into()),
        ("case", |s| s.case = TextCase::Upper),
    ] {
        let (mut app, a, _) = linked_layers();
        let mut want = style_of(&app, a);
        f(&mut want);
        edit(&mut app, a, f);
        assert!(!linked(&app, a), "{what} detaches the style");
        assert_eq!(style_of(&app, a), want, "{what}: the value stays");
        assert!(app.doc.get(a).unwrap().text_style_overrides.is_empty());
    }
}

#[test]
fn a_weight_and_a_size_in_one_edit_detach() {
    let (mut app, a, _) = linked_layers();
    edit(&mut app, a, |s| {
        s.set_font_weight(700);
        s.font_size = 20.0;
    });
    assert!(!linked(&app, a));
}

#[test]
fn a_fill_only_or_alignment_only_edit_changes_nothing_about_the_link() {
    let (mut app, a, _) = linked_layers();
    edit(&mut app, a, |s| {
        s.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        s.align_h = TextAlignH::Center;
    });
    assert!(linked(&app, a) && overrides(&app, a).is_empty());
}

#[test]
fn a_weight_variable_keeps_the_link_and_records_the_override() {
    let (mut app, a, _) = linked_layers();
    app.bind_variable(&[a], &["fontWeight"], "weight");
    assert!(linked(&app, a));
    assert_eq!(overrides(&app, a), vec!["Weight"]);
}

#[test]
fn editing_the_style_keeps_the_weight_a_layer_overrides_and_updates_its_size() {
    let (mut app, a, b) = linked_layers();
    edit(&mut app, a, |s| s.set_font_weight(700));
    assert!(app.update_style_values("Body", Style::Text(TextStyle { font_size: 18.0, font_weight: 500, ..Default::default() })));
    assert_eq!((style_of(&app, a).font_size, style_of(&app, a).font_weight), (18.0, 700), "weight stays, size follows");
    assert_eq!((style_of(&app, b).font_size, style_of(&app, b).font_weight), (18.0, 500), "the plain layer follows the style");
    assert_eq!(overrides(&app, a), vec!["Weight"]);
    app.undo();
    assert_eq!((style_of(&app, a).font_size, style_of(&app, b).font_size), (16.0, 16.0), "undo restores both exactly");
}

#[test]
fn a_bold_toggled_on_and_off_again_follows_the_next_style_edit() {
    let (mut app, a, _) = linked_layers();
    edit(&mut app, a, |s| s.set_font_weight(700));
    edit(&mut app, a, |s| s.set_font_weight(400));
    assert!(overrides(&app, a).is_empty(), "no pill: the weight equals the style's");
    assert!(app.update_style_values("Body", Style::Text(TextStyle { font_size: 16.0, font_weight: 500, ..Default::default() })));
    assert_eq!(style_of(&app, a).font_weight, 500, "the style edit reaches the layer");
}

#[test]
fn reapplying_clears_values_bindings_and_the_set_keeps_the_link_and_is_one_undo_step() {
    let (mut app, a, _) = linked_layers();
    edit(&mut app, a, |s| {
        s.set_font_weight(700);
        s.decoration = TextDecoration::Underline;
    });
    app.bind_variable(&[a], &["fontWeight"], "weight");
    app.doc.get_mut(a).unwrap().variables.bindings.insert("fills/0".into(), "colour".into());
    let before = app.doc.get(a).unwrap().clone();

    assert!(app.reapply_text_style(&[a]));

    let n = app.doc.get(a).unwrap();
    assert!(n.text_style_overrides.is_empty() && overrides(&app, a).is_empty());
    assert!(linked(&app, a));
    assert!(!n.variables.bindings.contains_key("fontWeight"));
    assert_eq!(n.variables.bindings.get("fills/0").map(String::as_str), Some("colour"));
    assert_eq!((style_of(&app, a).font_weight, style_of(&app, a).decoration), (400, TextDecoration::None));
    let after = n.clone();

    app.undo();
    assert_eq!(app.doc.get(a).unwrap(), &before, "one undo restores the overrides");
    app.redo();
    assert_eq!(app.doc.get(a).unwrap(), &after, "redo reapplies");
}

#[test]
fn reapplying_with_nothing_overridden_changes_nothing() {
    let (mut app, a, _) = linked_layers();
    assert!(!app.reapply_text_style(&[a]));
}

#[test]
fn detaching_drops_the_link_and_the_set_and_keeps_the_values() {
    let (mut app, a, _) = linked_layers();
    edit(&mut app, a, |s| s.set_font_weight(700));
    app.detach_style_of(&[a], StyleProperty::Text);
    let n = app.doc.get(a).unwrap();
    assert!(!n.style_refs.contains_key("text") && n.text_style_overrides.is_empty());
    assert_eq!(style_of(&app, a).font_weight, 700);
}

#[test]
fn applying_a_style_clears_the_overrides_it_replaces() {
    let (mut app, a, _) = linked_layers();
    edit(&mut app, a, |s| s.set_font_weight(700));
    app.apply_style(&[a], "Body");
    assert!(overrides(&app, a).is_empty());
    assert_eq!(style_of(&app, a).font_weight, 400);
}

#[test]
fn binding_a_style_variable_makes_no_override_on_a_layer_in_another_mode() {
    let (mut app, _, b) = linked_layers();
    assert!(app.bind_style_variable("Body", "fontSize", "size"));
    assert!(overrides(&app, b).is_empty(), "the Dark layer resolves 32 against a default of 16 and still has no override");
    assert_eq!(style_of(&app, b).font_size, 32.0);
}

#[test]
fn pasting_properties_keeps_the_link_for_a_weight_and_detaches_for_a_size() {
    let (mut app, a, b) = linked_layers();
    edit(&mut app, b, |s| s.set_font_weight(700));
    app.selection = vec![b];
    app.copy_properties();
    app.selection = vec![a];
    app.paste_properties();
    assert!(linked(&app, a));
    assert_eq!(overrides(&app, a), vec!["Weight"]);

    let (mut app, a, b) = linked_layers();
    edit(&mut app, b, |s| s.font_size = 40.0);
    app.selection = vec![b];
    app.copy_properties();
    app.selection = vec![a];
    app.paste_properties();
    assert!(!linked(&app, a), "a pasted size detaches");
    assert_eq!(style_of(&app, a).font_size, 40.0);
}

#[test]
fn a_hyperlink_on_a_linked_text_is_an_override_and_reapply_removes_it() {
    let (mut app, a, _) = linked_layers();
    app.doc.get_mut(a).unwrap().hyperlink = Some("https://example.com".into());
    assert_eq!(overrides(&app, a), vec!["Hyperlink"]);
    assert!(app.reapply_text_style(&[a]));
    assert!(app.doc.get(a).unwrap().hyperlink.is_none() && overrides(&app, a).is_empty());
}

#[test]
fn applying_the_linked_style_clears_hyperlink_range_and_binding_overrides() {
    let (mut app, a, _) = linked_layers();
    edit(&mut app, a, |s| s.set_font_weight(700));
    app.bind_variable(&[a], &["fontWeight"], "weight");
    if let NodeKind::Text(tp) = &mut app.doc.get_mut(a).unwrap().kind {
        tp.runs.push(TextRun { start: 0, end: 2, style: TextStyle { italic: true, ..tp.style.clone() } });
    }
    app.doc.get_mut(a).unwrap().hyperlink = Some("https://example.com".into());
    assert_eq!(overrides(&app, a), vec!["Weight", "Italic", "Hyperlink"]);

    app.apply_style(&[a], "Body");
    let n = app.doc.get(a).unwrap();
    assert!(overrides(&app, a).is_empty(), "re-applying the linked style clears all overrides");
    assert!(n.hyperlink.is_none(), "hyperlink override is cleared");
    assert!(!n.variables.bindings.contains_key("fontWeight"), "owned typography binding is cleared");
    assert_eq!(style_of(&app, a).font_weight, 400);
    assert!(!n.kind.any_text().unwrap().runs[0].style.italic, "range override is cleared");
}

#[test]
fn undoing_a_style_edit_restores_every_linked_layer_and_its_overrides_as_it_was() {
    let (mut app, a, b) = linked_layers();
    edit(&mut app, a, |s| {
        s.set_font_weight(700);
        s.italic = true;
    });
    if let NodeKind::Text(tp) = &mut app.doc.get_mut(a).unwrap().kind {
        tp.runs.push(TextRun { start: 0, end: 3, style: TextStyle { decoration: TextDecoration::Underline, ..tp.style.clone() } });
    }
    app.doc.get_mut(a).unwrap().hyperlink = Some("https://example.com".into());

    // `b` had bold toggled on and off again.
    edit(&mut app, b, |s| s.set_font_weight(700));
    edit(&mut app, b, |s| s.set_font_weight(400));

    let before_a = app.doc.get(a).unwrap().clone();
    let before_b = app.doc.get(b).unwrap().clone();

    assert!(app.update_style_values("Body", Style::Text(TextStyle { font_size: 22.0, font_weight: 500, ..Default::default() })));
    assert_eq!((style_of(&app, a).font_size, style_of(&app, a).font_weight, style_of(&app, a).italic), (22.0, 700, true));
    assert_eq!(app.doc.get(a).unwrap().kind.any_text().unwrap().runs[0].style.decoration, TextDecoration::Underline);
    assert_eq!((style_of(&app, b).font_size, style_of(&app, b).font_weight), (22.0, 500));

    app.undo();
    assert_eq!(app.doc.get(a).unwrap(), &before_a);
    assert_eq!(app.doc.get(b).unwrap(), &before_b);

    assert!(app.bind_style_variable("Body", "fontWeight", "weight"));
    assert_eq!(style_of(&app, a).font_weight, 700, "a keeps its overridden weight");
    assert_eq!(style_of(&app, b).font_weight, 800, "b (in Dark mode, with turned-off bold) takes the bound weight");

    app.undo();
    assert_eq!(app.doc.get(a).unwrap(), &before_a);
    assert_eq!(app.doc.get(b).unwrap(), &before_b);
}
