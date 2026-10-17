//! Editing a text style in a file with component instances: which instance text is re-fitted.

use crate::app::App;
use crate::layout;
use designos_doc::*;

const OWN_TEXT: &str = "Submit the whole order now please";

/// How the instance label comes to differ from the main's characters.
#[derive(Clone, Copy)]
enum OwnCharacters {
    /// The user typed into the instance.
    Typed,
    /// The main binds its characters to a text property the instance sets.
    Property,
}

/// A main label on the `Body` text style (12) with one instance whose label has its own characters,
/// fitted to its content. Returns the app, the main label and the instance label.
fn instance_with_own_label(mode: TextAutoResize, chars: OwnCharacters) -> (App, NodeId, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 200.0))).unwrap();
    let tid = d.alloc_id();
    let mut t = Node::text(tid, Rect::new(10.0, 10.0, 100.0, 20.0), "Label", TextStyle::default());
    if let NodeKind::Text(tp) = &mut t.kind {
        tp.auto_resize = mode;
    }
    let t = d.insert(f, t).unwrap();
    d.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 12.0, ..Default::default() }));
    let mut app = App::new(d);
    app.apply_style(&[t], "Body");
    app.selection = vec![f];
    app.create_component();
    let comp = app.doc.get(t).and_then(|n| n.parent).expect("component root");
    let inst = app.insert_instance(comp, Vec2::new(500.0, 100.0)).expect("instance");
    let label = app.doc.get(inst).unwrap().children[0];
    assert_eq!(app.doc.get(label).unwrap().component.source, Some(t), "the instance label mirrors the main label");
    match chars {
        OwnCharacters::Typed => app.edit_nodes(&[label], |n| {
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.content = OWN_TEXT.into();
                n.size = layout::fit_text(tp, n.size);
            }
        }),
        OwnCharacters::Property => {
            let def = ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Label".into()) };
            let add = add_prop(&mut app.doc, comp, def);
            add.apply(&mut app.doc).expect("add property");
            let bind = bind_prop(&mut app.doc, t, OverrideField::Characters, Some("Label"));
            bind.apply(&mut app.doc).expect("bind property");
            let set = set_instance_prop(&mut app.doc, inst, "Label", Some(PropValue::Text(OWN_TEXT.into())));
            set.apply(&mut app.doc).expect("instance property");
            app.edit_nodes(&[label], |n| {
                if let NodeKind::Text(tp) = &n.kind {
                    n.size = layout::fit_text(tp, n.size);
                }
            });
        }
    }
    let own = app.doc.get(label).unwrap();
    assert!(matches!(&own.kind, NodeKind::Text(tp) if tp.content == OWN_TEXT), "the instance label has its own characters");
    (app, t, label)
}

fn body_at(size: f32) -> Style {
    Style::Text(TextStyle { font_size: size, ..Default::default() })
}

/// The label's size must be what its own text needs at its current type, in its own mode.
fn assert_fits_its_own_text(app: &App, label: NodeId) {
    let n = app.doc.get(label).unwrap();
    let NodeKind::Text(tp) = &n.kind else { panic!("not text") };
    assert_eq!(n.size, layout::fit_text(tp, n.size), "{:?}: the label is fitted to its own characters", tp.auto_resize);
}

#[test]
fn instance_text_with_its_own_characters_refits_when_the_style_grows_in_width_and_height_mode() {
    let (mut app, main, label) = instance_with_own_label(TextAutoResize::WidthAndHeight, OwnCharacters::Typed);
    let before = app.doc.get(label).unwrap().size;
    assert!(app.update_style_values("Body", body_at(40.0)));
    let after = app.doc.get(label).unwrap().size;
    assert!(after.x > before.x * 2.0 && after.y > before.y * 2.0, "the label grows with its type: {before:?} -> {after:?}");
    assert_fits_its_own_text(&app, label);
    assert_ne!(after, app.doc.get(main).unwrap().size, "the size is the label's own, not the main's");
    assert!(matches!(&app.doc.get(label).unwrap().kind, NodeKind::Text(tp) if tp.content == OWN_TEXT), "the characters stay the instance's");
}

#[test]
fn instance_text_with_its_own_characters_refits_when_the_style_grows_in_height_mode() {
    let (mut app, _main, label) = instance_with_own_label(TextAutoResize::Height, OwnCharacters::Typed);
    assert!(
        !app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Size),
        "typing in Height mode refits height without recording a Size override"
    );
    let before = app.doc.get(label).unwrap().size;
    assert!(app.update_style_values("Body", body_at(40.0)));
    let after = app.doc.get(label).unwrap().size;
    assert_eq!(after.x, before.x, "height mode keeps the width");
    assert!(after.y > before.y * 2.0, "the label gets taller with its type: {before:?} -> {after:?}");
    assert!(
        !app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Size),
        "a style edit refit in Height mode does not record a Size override"
    );
    assert_fits_its_own_text(&app, label);
}

#[test]
fn instance_text_whose_characters_come_from_a_property_refits_when_the_style_changes() {
    for mode in [TextAutoResize::WidthAndHeight, TextAutoResize::Height] {
        let (mut app, _main, label) = instance_with_own_label(mode, OwnCharacters::Property);
        assert!(!app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Characters), "the characters are bound, not overridden");
        assert!(!app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Size), "{mode:?}: property refit does not record a Size override");
        let before = app.doc.get(label).unwrap().size;
        assert!(app.update_style_values("Body", body_at(40.0)));
        let after = app.doc.get(label).unwrap().size;
        assert!(after.y > before.y * 2.0, "{mode:?}: the label gets taller: {before:?} -> {after:?}");
        assert!(!app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Size), "{mode:?}: style edit refit does not record a Size override");
        assert_fits_its_own_text(&app, label);
    }
}

#[test]
fn a_style_edit_on_an_instance_label_is_one_undo_step() {
    let (mut app, _main, label) = instance_with_own_label(TextAutoResize::WidthAndHeight, OwnCharacters::Typed);
    let before = app.doc.get(label).unwrap().size;
    assert!(app.update_style_values("Body", body_at(40.0)));
    let size_after_edit = app.doc.get(label).unwrap().size;
    assert_ne!(size_after_edit, before, "the edit re-fitted the label, so undo has something to restore");
    app.undo();
    assert_eq!(app.doc.get(label).unwrap().size, before);
    assert!(matches!(&app.doc.styles["Body"], Style::Text(ts) if ts.font_size == 12.0));
}

/// A main label on `Heading` (24) with one instance whose label has `Body` (12) applied on top,
/// so its text style is the instance's own. Characters are the main's. Returns app, main label, instance label.
fn instance_with_own_text_style(mode: TextAutoResize) -> (App, NodeId, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 200.0))).unwrap();
    let tid = d.alloc_id();
    let mut t = Node::text(tid, Rect::new(10.0, 10.0, 100.0, 40.0), "Label", TextStyle::default());
    if let NodeKind::Text(tp) = &mut t.kind {
        tp.auto_resize = mode;
    }
    let t = d.insert(f, t).unwrap();
    d.styles.insert("Heading".into(), Style::Text(TextStyle { font_size: 24.0, ..Default::default() }));
    d.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 12.0, ..Default::default() }));
    let mut app = App::new(d);
    app.apply_style(&[t], "Heading");
    app.selection = vec![f];
    app.create_component();
    let comp = app.doc.get(t).and_then(|n| n.parent).expect("component root");
    let inst = app.insert_instance(comp, Vec2::new(500.0, 100.0)).expect("instance");
    let label = app.doc.get(inst).unwrap().children[0];
    assert_eq!(app.doc.get(label).unwrap().component.source, Some(t), "the instance label mirrors the main label");
    app.apply_style(&[label], "Body");
    let own = app.doc.get(label).unwrap();
    assert!(own.component.overrides.contains(&OverrideField::TextStyle), "the label's text style is its own");
    assert!(!own.component.overrides.contains(&OverrideField::Characters), "the characters are the main's");
    assert!(!own.component.overrides.contains(&OverrideField::Size), "{mode:?}: applying a text style refits height without recording a Size override");
    assert_eq!(own.style_refs.get("text").map(String::as_str), Some("Body"));
    (app, t, label)
}

#[test]
fn instance_text_with_its_own_text_style_refits_when_that_style_grows() {
    for mode in [TextAutoResize::WidthAndHeight, TextAutoResize::Height] {
        let (mut app, main, label) = instance_with_own_text_style(mode);
        let before = app.doc.get(label).unwrap().size;
        assert!(app.update_style_values("Body", body_at(40.0)));
        let after = app.doc.get(label).unwrap().size;
        assert!(after.y > before.y * 2.0, "{mode:?}: the label gets taller with its own type: {before:?} -> {after:?}");
        assert!(!app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Size), "{mode:?}: style edit refit does not record a Size override");
        assert_fits_its_own_text(&app, label);
        assert_ne!(after, app.doc.get(main).unwrap().size, "{mode:?}: the size is the label's own, not the main's");
    }
}

#[test]
fn editing_the_mains_style_does_not_copy_the_mains_size_onto_text_with_its_own_text_style() {
    for mode in [TextAutoResize::WidthAndHeight, TextAutoResize::Height] {
        let (mut app, main, label) = instance_with_own_text_style(mode);
        let before = app.doc.get(label).unwrap().size;
        assert!(app.update_style_values("Heading", Style::Text(TextStyle { font_size: 60.0, ..Default::default() })));
        assert!(app.doc.get(main).unwrap().size.y > before.y * 2.0, "{mode:?}: the main grew with Heading");
        let own = app.doc.get(label).unwrap();
        assert_eq!(own.size, before, "{mode:?}: the label keeps its own size");
        assert!(matches!(&own.kind, NodeKind::Text(tp) if tp.style.font_size == 12.0), "{mode:?}: the label keeps its own type");
        assert_fits_its_own_text(&app, label);
    }
}

#[test]
fn height_mode_refit_does_not_record_size_override_unless_width_changes() {
    let (mut app, main, label) = instance_with_own_label(TextAutoResize::Height, OwnCharacters::Typed);
    assert!(!app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Size));

    // Resetting the Characters override lets the label take the main's size again because Size was not overridden.
    app.reset_node_overrides(label, &[OverrideField::Characters]);
    assert_eq!(app.doc.get(label).unwrap().size, app.doc.get(main).unwrap().size);

    // Explicitly widening the Height-mode text box IS a user size override.
    app.edit_nodes(&[label], |n| {
        if let NodeKind::Text(tp) = &n.kind {
            n.size.x = 180.0;
            n.size = layout::fit_text(tp, n.size);
        }
    });
    assert!(app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Size));
}

#[test]
fn explicit_height_edit_on_height_mode_instance_label_survives_main_sync() {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 200.0))).unwrap();
    let tid = d.alloc_id();
    let mut t = Node::text(tid, Rect::new(10.0, 10.0, 100.0, 20.0), "Label", TextStyle::default());
    if let NodeKind::Text(tp) = &mut t.kind {
        tp.auto_resize = TextAutoResize::Height;
    }
    let t = d.insert(f, t).unwrap();
    let mut app = App::new(d);
    app.selection = vec![f];
    app.create_component();
    let comp = app.doc.get(t).and_then(|n| n.parent).expect("component root");
    let inst = app.insert_instance(comp, Vec2::new(500.0, 100.0)).expect("instance");
    let label = app.doc.get(inst).unwrap().children[0];

    // Explicitly set H to 90 on the Height-mode instance label (no auto layout, no characters override).
    app.edit_nodes(&[label], |n| n.size.y = 90.0);
    assert!(
        app.doc.get(label).unwrap().component.overrides.contains(&OverrideField::Size),
        "explicit height edit on Height-mode label must record a Size override"
    );

    // Make an edit on the main component so the instance syncs.
    app.edit_nodes(&[t], |n| n.opacity = 0.8);
    assert_eq!(app.doc.get(label).unwrap().size.y, 90.0, "explicit height edit on Height-mode instance label must survive main sync");
}
