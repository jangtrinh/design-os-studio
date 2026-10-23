//! Text styles in the inspector: create one from a text layer through the style button of the
//! Typography section, apply, rename and detach it, and keep it through a save and a `.fig` file.

use super::effect_grid_styles::pick;
use super::*;
use designos_doc::{LineHeight, Style, StyleProperty, TextRun, TextStyle};

const BUTTON: &str = "text-style-button";

/// `name` selected in a file that has no styles at all (the demo file ships some), with a text style
/// no default would produce, so a copied value is told from a default one.
fn bare_sim(name: &str) -> (Sim, NodeId) {
    let mut s = Sim::with(Options { select: Some(name.into()), ..Default::default() });
    let id = s.by_name(name);
    s.shell.app.doc.styles.clear();
    s.shell.app.doc.style_meta.clear();
    for n in s.shell.app.doc.nodes.values_mut() {
        n.style_refs.clear();
    }
    s.shell.app.edit_nodes(&[id], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.font_weight = 600;
            t.style.font_size = 18.0;
            t.style.line_height = LineHeight::Pixels(26.0);
            t.style.letter_spacing = 0.5;
        }
    });
    redraw(&mut s);
    (s, id)
}

/// Draws the inspector again, from its top, after the test changed the document directly.
fn redraw(s: &mut Sim) {
    s.shell.app.props.scroll = 0.0;
    s.frame(vec![]);
    s.frame(vec![]);
}

fn link_of(s: &Sim, id: NodeId) -> Option<String> {
    s.app().doc.get(id).unwrap().style_refs.get("text").cloned()
}

fn style_of<'a>(s: &'a Sim, key: &str) -> &'a TextStyle {
    match &s.app().doc.styles[key] {
        Style::Text(ts) => ts,
        other => panic!("{key} is not a text style: {other:?}"),
    }
}

/// Opens the menu behind the style button.
fn open_menu(s: &mut Sim) {
    let r = s.rect_of(BUTTON);
    s.click(r.center());
}

/// Clicks the `row`-th row of the open style menu.
fn click_row(s: &mut Sim, row: usize) {
    let r = s.rect_of(&format!("menu-row-0-{row}"));
    s.click(r.center());
}

#[test]
fn creating_a_text_style_in_a_file_without_styles_links_the_layer_in_one_undo_step() {
    let (mut s, id) = bare_sim("Continue");
    assert!(s.app().doc.styles.is_empty(), "the file starts without a style");
    let layer = text_of(&s, id);
    let depth = s.app().hist.undo_depth();
    // Rows: "No text styles" (disabled), then Create.
    pick(&mut s, BUTTON, 1, 1);
    let key = link_of(&s, id).expect("linked to the new style");
    assert_eq!(key, "Text style 1");
    assert_eq!(s.app().doc.style_name(&key), "Text style 1");
    assert_eq!(s.app().doc.style_meta[&key].property, StyleProperty::Text);
    let ts = style_of(&s, &key);
    assert_eq!(ts, &layer.style, "the style carries every value of the layer");
    assert_eq!((ts.font_family.as_str(), ts.font_weight, ts.font_size), ("Inter", 600, 18.0));
    assert_eq!((ts.line_height, ts.letter_spacing), (LineHeight::Pixels(26.0), 0.5));
    assert_eq!(text_of(&s, id), layer, "creating a style leaves the layer as it is");
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some("Created \"Text style 1\""));
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one undo step");
    s.frame(vec![]);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.styles.is_empty(), "undo takes the style away");
    assert_eq!(link_of(&s, id), None, "and the link with it");
    assert_eq!(text_of(&s, id), layer);
}

#[test]
fn a_second_text_style_is_text_style_2_with_the_values_of_its_own_layer() {
    let (mut s, first) = bare_sim("Continue");
    pick(&mut s, BUTTON, 1, 1);
    let second = s.by_name("Welcome back");
    s.shell.app.selection = vec![second];
    s.shell.app.edit_nodes(&[second], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.font_size = 40.0;
        }
    });
    redraw(&mut s);
    // Rows: Text style 1, then Create.
    pick(&mut s, BUTTON, 1, 1);
    assert_eq!(link_of(&s, second).as_deref(), Some("Text style 2"));
    assert_eq!(style_of(&s, "Text style 2").font_size, 40.0);
    assert_eq!(style_of(&s, "Text style 1").font_size, 18.0, "the first style is untouched");
    assert_eq!(link_of(&s, first).as_deref(), Some("Text style 1"));
}

#[test]
fn a_style_renamed_to_a_free_looking_name_is_not_named_again() {
    let (mut s, id) = bare_sim("Continue");
    pick(&mut s, BUTTON, 1, 1);
    s.shell.app.detach_style_of(&[id], StyleProperty::Text);
    // The key stays "Text style 1" but the style now answers to "Text style 2".
    s.shell.app.rename_style_to("Text style 1", "Text style 2");
    redraw(&mut s);
    pick(&mut s, BUTTON, 1, 1);
    let key = link_of(&s, id).expect("linked");
    assert_eq!(s.app().doc.style_name(&key), "Text style 3", "no two styles share a name");
}

#[test]
fn the_menu_lists_the_text_styles_checks_the_linked_one_and_applies_one() {
    let mut s = Sim::with(Options { select: Some("Continue".into()), ..Default::default() });
    let id = s.by_name("Continue");
    // The demo file has the text styles Body/Regular and Heading/Large (and colour styles, not listed here).
    open_menu(&mut s);
    s.rect_of("menu-row-0-0");
    s.rect_of("menu-row-0-1");
    assert_eq!(s.rect_of("menu-check-0-0"), Rect::NOTHING, "nothing is linked yet");
    s.click(s.rect_of("menu-row-0-0").center());
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    let ts = text_of(&s, id).style;
    assert_eq!((ts.font_size, ts.line_height), (16.0, LineHeight::Pixels(24.0)), "the layer takes the style's values");
    open_menu(&mut s);
    assert!(s.rect_of("menu-check-0-0").is_positive(), "the linked style is checked");
    assert_eq!(s.rect_of("menu-check-0-1"), Rect::NOTHING);
}

#[test]
fn create_is_off_while_the_layer_is_linked() {
    let (mut s, id) = bare_sim("Continue");
    pick(&mut s, BUTTON, 1, 1);
    s.frame(vec![]);
    let depth = s.app().hist.undo_depth();
    // Rows: Text style 1, then Create (disabled while linked).
    pick(&mut s, BUTTON, 1, 1);
    assert_eq!(s.app().doc.styles.len(), 1);
    assert_eq!(link_of(&s, id).as_deref(), Some("Text style 1"));
    assert_eq!(s.app().hist.undo_depth(), depth, "a disabled row does nothing");
}

#[test]
fn a_layer_with_mixed_styles_has_no_single_style_to_create() {
    let (mut s, id) = bare_sim("Continue");
    s.shell.app.edit_nodes(&[id], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.runs.push(TextRun { start: 0, end: 3, style: TextStyle { font_weight: 700, ..t.style.clone() } });
        }
    });
    redraw(&mut s);
    let depth = s.app().hist.undo_depth();
    pick(&mut s, BUTTON, 1, 1);
    assert!(s.app().doc.styles.is_empty(), "no style is made from a layer whose runs differ");
    assert_eq!(link_of(&s, id), None);
    assert_eq!(s.app().hist.undo_depth(), depth);
}

#[test]
fn runs_that_differ_only_in_colour_do_not_stop_creation() {
    let (mut s, id) = bare_sim("Continue");
    s.shell.app.edit_nodes(&[id], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            let red = TextStyle { fills: vec![Paint::solid(Color::rgb(255, 0, 0))], ..t.style.clone() };
            t.runs.push(TextRun { start: 0, end: 3, style: red });
        }
    });
    redraw(&mut s);
    pick(&mut s, BUTTON, 1, 1);
    assert_eq!(link_of(&s, id).as_deref(), Some("Text style 1"), "colour is not part of a text style");
}

#[test]
fn renaming_and_detaching_from_the_linked_row() {
    let (mut s, id) = bare_sim("Continue");
    pick(&mut s, BUTTON, 1, 1);
    s.frame(vec![]);
    s.rect_of("text-style-row");
    // Rows: the style, Create (disabled while linked), Rename.
    pick(&mut s, BUTTON, 2, 1);
    assert_eq!(s.app().props.style_rename, Some(StyleProperty::Text));
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().props.style_rename, Some(StyleProperty::Text), "the name field keeps the focus");
    s.type_text("Heading/H1");
    s.key(Key::Enter);
    s.frame(vec![]);
    assert_eq!(s.app().doc.style_name("Text style 1"), "Heading/H1");
    assert_eq!(s.app().props.style_rename, None);
    assert_eq!(link_of(&s, id).as_deref(), Some("Text style 1"), "the link is kept");
    let layer = text_of(&s, id);
    let detach = s.rect_of("text-style-detach");
    s.click(detach.center());
    assert_eq!(link_of(&s, id), None);
    assert!(s.app().doc.styles.contains_key("Text style 1"), "the style stays in the file");
    assert_eq!(text_of(&s, id), layer, "detaching keeps the values");
}

#[test]
fn the_linked_row_opens_the_style_editor() {
    let (mut s, _id) = bare_sim("Continue");
    pick(&mut s, BUTTON, 1, 1);
    s.frame(vec![]);
    let edit = s.rect_of("text-style-edit");
    s.click(edit.center());
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some("Text style 1"));
}

#[test]
fn creating_from_text_being_edited_ends_the_session_and_styles_the_whole_layer() {
    let (mut s, id) = bare_sim("Continue");
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    let end = text_of(&s, id).content.len();
    {
        let sess = s.shell.app.canvas.text.as_mut().unwrap();
        sess.caret = end;
        sess.anchor = end;
    }
    crate::canvas::textsession::insert_text(&mut s.shell.app, "!", crate::textedit::EditKind::Typing);
    // A range inside the text is selected when the button is used.
    {
        let sess = s.shell.app.canvas.text.as_mut().unwrap();
        sess.caret = 3;
        sess.anchor = 0;
    }
    s.frame(vec![]);
    s.frame(vec![]);
    let depth = s.app().hist.undo_depth();
    open_menu(&mut s);
    assert!(s.app().canvas.text.is_some(), "opening the menu leaves the session alone");
    click_row(&mut s, 1);
    assert!(s.app().canvas.text.is_none(), "the session ends before the style is made");
    assert_eq!(text_of(&s, id).content, "Continue!", "what was typed is kept");
    assert_eq!(link_of(&s, id).as_deref(), Some("Text style 1"), "the whole layer is linked");
    assert_eq!(s.app().hist.undo_depth(), depth + 2, "the typing is one step, the style another");
    s.frame(vec![]);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.styles.is_empty(), "one undo removes only the style and the link");
    assert_eq!(link_of(&s, id), None);
    assert_eq!(text_of(&s, id).content, "Continue!");
}

#[test]
fn applying_and_detaching_end_a_text_session_first_too() {
    let mut s = Sim::with(Options { select: Some("Continue".into()), ..Default::default() });
    let id = s.by_name("Continue");
    let content = text_of(&s, id).content;
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    // The first row of the menu is the demo file's Body/Regular.
    open_menu(&mut s);
    assert!(s.app().canvas.text.is_some(), "opening the menu leaves the session alone");
    click_row(&mut s, 0);
    assert!(s.app().canvas.text.is_none(), "applying a style ends the session first");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    assert_eq!(text_of(&s, id).content, content);
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    let detach = s.rect_of("text-style-detach");
    s.click(detach.center());
    assert!(s.app().canvas.text.is_none(), "detaching a style ends the session first");
    assert_eq!(link_of(&s, id), None);
}

/// A directory of its own under the system temp dir, removed when dropped.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("designos_text_style_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn find_style(doc: &designos_doc::Document, name: &str) -> String {
    doc.styles.keys().find(|k| doc.style_name(k) == name).unwrap_or_else(|| panic!("no style named {name}")).clone()
}

#[test]
fn a_created_text_style_survives_a_designos_file() {
    let (mut s, id) = bare_sim("Continue");
    pick(&mut s, BUTTON, 1, 1);
    let dir = Scratch::new();
    let path = dir.0.join("styled.designos");
    crate::io::save_path(&s.app().doc, &path).expect("save");
    let back = crate::io::load_path(&path).expect("reopen");
    let key = find_style(&back, "Text style 1");
    assert_eq!(back.styles[&key], s.app().doc.styles["Text style 1"], "the values are exactly the ones created");
    assert_eq!(back.style_meta[&key].property, StyleProperty::Text);
    let layer = back.get(id).expect("the layer is still there");
    assert_eq!(layer.style_refs.get("text"), Some(&key), "the layer is still linked");
}

#[test]
fn a_created_text_style_survives_a_fig_export_and_import() {
    let (mut s, id) = bare_sim("Continue");
    pick(&mut s, BUTTON, 1, 1);
    let bytes = designos_fig::export_fig(&s.app().doc).expect("export .fig");
    let back = designos_fig::import_fig(&bytes).expect("import .fig");
    let key = find_style(&back, "Text style 1");
    assert_eq!(back.style_meta[&key].property, StyleProperty::Text);
    let Style::Text(ts) = &back.styles[&key] else { panic!("{:?} is not a text style", back.styles[&key]) };
    assert_eq!((ts.font_family.as_str(), ts.font_weight), ("Inter", 600));
    assert!((ts.font_size - 18.0).abs() < 1e-3, "size {}", ts.font_size);
    assert!((ts.letter_spacing - 0.5).abs() < 1e-3, "letter spacing {}", ts.letter_spacing);
    match ts.line_height {
        LineHeight::Pixels(px) => assert!((px - 26.0).abs() < 1e-3, "line height {px}"),
        other => panic!("line height came back as {other:?}"),
    }
    let name = s.app().doc.get(id).unwrap().name.clone();
    let layer = back.nodes.values().find(|n| n.name == name).expect("the layer is in the file");
    assert_eq!(layer.style_refs.get("text"), Some(&key), "the layer is still linked");
}

#[test]
fn the_page_panel_has_no_styles_plus_that_does_nothing() {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    // A header action that works, so a missing record below means the control is gone, not that nothing records.
    s.rect_of("title-action:Layout guide");
    s.shell.app.selection.clear();
    s.shell.app.props.scroll = 1.0e6;
    s.frame(vec![]);
    s.frame(vec![]);
    let recorded = |name: &str| s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))));
    assert!(recorded("title-action:Export").is_some(), "the page panel was drawn and its header actions are recorded");
    assert!(recorded("title-action:Styles").is_none(), "creation lives in each section's style button");
}
