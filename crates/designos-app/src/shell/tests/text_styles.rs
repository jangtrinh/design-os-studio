//! Text styles in the inspector: create one from a text layer through the style button of the
//! Typography section, apply, rename and detach it, and keep it through a save and a `.fig` file.

use super::effect_grid_styles::pick;
use super::*;
use crate::panels::style_picker;
use designos_doc::{LineHeight, Style, StyleProperty, TextAlignH, TextAlignV, TextCase, TextDecoration, TextRun, TextStyle};

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

/// A text style with a value in every field, so none can pass a round trip by being a default.
fn every_field(line_height: LineHeight, fills: Vec<Paint>) -> TextStyle {
    TextStyle {
        font_family: "Inter".into(),
        font_weight: 600,
        italic: true,
        font_size: 18.0,
        line_height,
        letter_spacing: 1.5,
        paragraph_spacing: 8.0,
        align_h: TextAlignH::Center,
        align_v: TextAlignV::Bottom,
        case: TextCase::Upper,
        decoration: TextDecoration::Underline,
        fills,
        features: vec![("tnum".into(), 1)],
        variation_axes: vec![("wdth".into(), 85.0)],
    }
}

#[test]
fn a_created_text_style_survives_a_fig_export_and_import_in_every_field() {
    // Each kind of line height (the font's own, a percentage, pixels) comes back as it went.
    for line_height in [LineHeight::Auto, LineHeight::Percent(150.0), LineHeight::Pixels(26.0)] {
        let (mut s, id) = bare_sim("Continue");
        let expected = every_field(line_height, text_of(&s, id).style.fills);
        s.shell.app.edit_nodes(&[id], |n| {
            if let NodeKind::Text(t) = &mut n.kind {
                t.style = expected.clone();
            }
        });
        redraw(&mut s);
        pick(&mut s, BUTTON, 1, 1);
        assert_eq!(style_of(&s, "Text style 1"), &expected, "{line_height:?}: the style holds all of the layer's values");
        let bytes = designos_fig::export_fig(&s.app().doc).expect("export .fig");
        let back = designos_fig::import_fig(&bytes).expect("import .fig");
        let key = find_style(&back, "Text style 1");
        assert_eq!(back.style_meta[&key].property, StyleProperty::Text);
        let Style::Text(ts) = &back.styles[&key] else { panic!("{:?} is not a text style", back.styles[&key]) };
        // Colour is not part of a text style, and a `.fig` text style carries none.
        assert_eq!(&TextStyle { fills: expected.fills.clone(), ..ts.clone() }, &expected, "{line_height:?}: every field of the style comes back");
        let name = s.app().doc.get(id).unwrap().name.clone();
        let layer = back.nodes.values().find(|n| n.name == name).expect("the layer is in the file");
        assert_eq!(layer.style_refs.get("text"), Some(&key), "the layer is still linked");
    }
}

fn type_into(s: &mut Sim, r: Rect, text: &str) {
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text(text);
    s.key(Key::Enter);
}

fn redo(s: &mut Sim) {
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
}

/// What the layer shows and what its style says: the text, the layer's size, the style's size.
fn shown(s: &Sim, id: NodeId, key: &str) -> (String, f32, f32) {
    let t = text_of(s, id);
    (t.content, t.style.font_size, style_of(s, key).font_size)
}

/// Types "!" into the linked layer "Continue", changes the size of its style to 30 through the
/// editor that `open_editor` opens, ends the text editing and walks undo and redo.
fn editing_the_style_while_typing(open_editor: fn(&mut Sim)) {
    let (mut s, id) = bare_sim("Continue");
    pick(&mut s, BUTTON, 1, 1);
    s.frame(vec![]);
    let key = "Text style 1";
    let (start, typed, edited) = (("Continue".to_string(), 18.0, 18.0), ("Continue!".to_string(), 18.0, 18.0), ("Continue!".to_string(), 30.0, 30.0));
    assert_eq!(shown(&s, id, key), start);
    let depth = s.app().hist.undo_depth();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    let end = text_of(&s, id).content.len();
    {
        let sess = s.shell.app.canvas.text.as_mut().unwrap();
        sess.caret = end;
        sess.anchor = end;
    }
    crate::canvas::textsession::insert_text(&mut s.shell.app, "!", crate::textedit::EditKind::Typing);
    s.frame(vec![]);
    s.frame(vec![]);
    open_editor(&mut s);
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some(key), "the style editor is open");
    let field = s.field_rect("se-size");
    type_into(&mut s, field, "30");
    // The click on the empty canvas that ends the text editing, if the edit left it open.
    let empty = s.app().canvas_rect.max - Vec2::new(60.0, 60.0);
    s.click(empty);
    assert!(s.app().canvas.text.is_none());
    assert_eq!(shown(&s, id, key), edited);
    assert_eq!(link_of(&s, id).as_deref(), Some(key));
    assert_eq!(s.app().hist.undo_depth(), depth + 2, "the typing is one step and the style edit another");
    s.cmd_key(Key::Z);
    assert_eq!(shown(&s, id, key), typed, "the first undo takes back the style edit, and the layer matches its style");
    s.cmd_key(Key::Z);
    assert_eq!(shown(&s, id, key), start, "the second undo takes back the typing");
    redo(&mut s);
    assert_eq!(shown(&s, id, key), typed, "redo returns the typing first");
    redo(&mut s);
    assert_eq!(shown(&s, id, key), edited, "and then the style edit");
    assert_eq!(link_of(&s, id).as_deref(), Some(key), "the layer stays linked through every step");
}

#[test]
fn editing_the_style_from_the_linked_row_while_typing_keeps_undo_exact() {
    editing_the_style_while_typing(|s| {
        let edit = s.rect_of("text-style-edit");
        s.click(edit.center());
    });
}

#[test]
fn editing_the_style_from_the_menu_while_typing_keeps_undo_exact() {
    // Rows: the style, Create (off while linked), Rename, Detach, Edit.
    editing_the_style_while_typing(|s| {
        open_menu(s);
        click_row(s, 4);
    });
}

/// A new text box as the Text tool makes it: empty, its session open, the Typography section shown.
fn new_text_box(s: &mut Sim) -> NodeId {
    s.key(Key::T);
    let p = s.screen(300.0, 820.0);
    s.click(p);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().canvas.text.as_ref().is_some_and(|t| t.created), "the Text tool opened a session on a new box");
    s.app().selection[0]
}

const TYPE_FIRST: &str = "Type some text first";

#[test]
fn picking_a_style_on_a_new_empty_text_box_keeps_the_box_and_the_typing_uses_the_style() {
    let mut s = Sim::new();
    let depth = s.app().hist.undo_depth();
    let id = new_text_box(&mut s);
    // The first row is the demo file's Body/Regular: 16 px, 24 px line.
    pick(&mut s, BUTTON, 0, 0);
    assert!(s.app().canvas.text.is_some(), "the session stays open");
    assert!(s.app().doc.get(id).is_some(), "the box is still there");
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), None, "nothing is announced");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    let ts = text_of(&s, id).style;
    assert_eq!((ts.font_size, ts.line_height), (16.0, LineHeight::Pixels(24.0)));
    s.type_text("a");
    let t = text_of(&s, id);
    assert_eq!(t.content, "a");
    assert_eq!((t.style.font_size, t.style.line_height), (16.0, LineHeight::Pixels(24.0)), "the typed text takes the style");
    assert!(t.runs.iter().all(|r| r.style.font_size == 16.0));
    s.key(Key::Escape);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "the box keeps its style when editing ends");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one undo step makes the box");
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(id).is_none(), "and one undo removes it, as it always did");
}

#[test]
fn creating_a_style_on_a_new_empty_text_box_is_off_with_a_reason_and_deletes_nothing() {
    let mut s = Sim::new();
    let id = new_text_box(&mut s);
    let row = style_picker::create_style_row(s.app(), &[id], StyleProperty::Text);
    assert!(row.disabled);
    assert_eq!(row.tooltip, TYPE_FIRST);
    let styles = s.app().doc.styles.len();
    // Rows: Body/Regular, Heading/Large, then Create.
    pick(&mut s, BUTTON, 2, 1);
    assert!(s.app().doc.get(id).is_some(), "the box is still there");
    assert!(s.app().canvas.text.is_some(), "and still being edited");
    assert_eq!(s.app().doc.styles.len(), styles);
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), None);
    // Asked for in code, it says why and touches nothing, instead of reporting a layer id.
    assert_eq!(s.shell.app.create_style_from(id, StyleProperty::Text), None);
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some(TYPE_FIRST));
    assert!(s.app().doc.get(id).is_some() && s.app().canvas.text.is_some());
}

#[test]
fn detaching_and_editing_the_style_of_a_new_empty_text_box_never_delete_it() {
    let mut s = Sim::new();
    let id = new_text_box(&mut s);
    pick(&mut s, BUTTON, 0, 0);
    s.frame(vec![]);
    // Editing the style would need the box to be committed first, which would drop it.
    assert!(!s.shell.app.update_style_values("Body/Regular", Style::Text(TextStyle { font_size: 30.0, ..Default::default() })));
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some(TYPE_FIRST));
    assert!(s.app().doc.get(id).is_some() && s.app().canvas.text.is_some());
    assert!(matches!(&s.app().doc.styles["Body/Regular"], Style::Text(ts) if ts.font_size == 16.0), "the style is unchanged");
    // Detaching works inside the session.
    let detach = s.rect_of("text-style-detach");
    s.click(detach.center());
    assert_eq!(link_of(&s, id), None);
    assert!(s.app().doc.get(id).is_some() && s.app().canvas.text.is_some());
    assert_eq!(text_of(&s, id).style.font_size, 16.0, "the box keeps the values");
}

/// The link and the size of a layer: what a style pick sets.
fn picked(s: &Sim, id: NodeId) -> (Option<String>, f32) {
    (link_of(s, id), text_of(s, id).style.font_size)
}

#[test]
fn cmd_z_in_a_new_box_takes_back_the_picked_style_and_its_link_together() {
    let mut s = Sim::new();
    let id = new_text_box(&mut s);
    let start = text_of(&s, id).style;
    // The second row is the demo file's Heading/Large: 32 px.
    pick(&mut s, BUTTON, 1, 0);
    assert_eq!(picked(&s, id), (Some("Heading/Large".into()), 32.0));
    s.cmd_key(Key::Z);
    assert!(s.app().canvas.text.is_some(), "the undo stays inside the session");
    assert_eq!(link_of(&s, id), None, "the link goes with the values");
    assert_eq!(text_of(&s, id).style, start, "and the values are the ones the box had");
    redo(&mut s);
    assert_eq!(picked(&s, id), (Some("Heading/Large".into()), 32.0), "redo brings both back");
}

#[test]
fn two_picks_undo_one_at_a_time_and_the_committed_box_matches_its_link() {
    let mut s = Sim::new();
    let id = new_text_box(&mut s);
    pick(&mut s, BUTTON, 0, 0);
    pick(&mut s, BUTTON, 1, 0);
    assert_eq!(picked(&s, id), (Some("Heading/Large".into()), 32.0));
    s.cmd_key(Key::Z);
    assert_eq!(picked(&s, id), (Some("Body/Regular".into()), 16.0), "one undo gives the first style with its link");
    s.type_text("a");
    s.key(Key::Escape);
    let t = text_of(&s, id);
    assert_eq!(t.content, "a");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    assert_eq!(t.style, TextStyle { fills: t.style.fills.clone(), ..style_of(&s, "Body/Regular").clone() }, "the box is what its style says");
}

fn preedit(s: &mut Sim, text: &str) {
    s.frame(vec![Event::Ime(egui::ImeEvent::Preedit { text: text.into(), active_range_chars: None })]);
}

#[test]
fn a_composition_in_a_new_box_is_not_text_so_applying_a_style_keeps_the_box() {
    let mut s = Sim::new();
    let id = new_text_box(&mut s);
    preedit(&mut s, "vie");
    assert_eq!(text_of(&s, id).content, "vie");
    assert!(s.app().canvas.text.as_ref().is_some_and(|t| t.preedit.is_some()), "the composition is under way");
    pick(&mut s, BUTTON, 0, 0);
    assert!(s.app().doc.get(id).is_some() && s.app().canvas.text.is_some(), "the box and its session stay");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    // The composition goes on, and what it ends in takes the style.
    s.frame(vec![Event::Ime(egui::ImeEvent::Commit("việt".into()))]);
    let t = text_of(&s, id);
    assert_eq!((t.content.as_str(), t.style.font_size), ("việt", 16.0));
}

#[test]
fn a_composition_in_a_new_box_leaves_create_off_with_its_reason() {
    let mut s = Sim::new();
    let id = new_text_box(&mut s);
    preedit(&mut s, "vie");
    let row = style_picker::create_style_row(s.app(), &[id], StyleProperty::Text);
    assert!(row.disabled);
    assert_eq!(row.tooltip, TYPE_FIRST);
    assert_eq!(s.shell.app.create_style_from(id, StyleProperty::Text), None);
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some(TYPE_FIRST), "no layer id, and nothing deleted");
    assert!(s.app().doc.get(id).is_some() && s.app().canvas.text.is_some());
}

#[test]
fn no_style_toast_shows_a_layer_id() {
    let mut s = Sim::new();
    let hero = s.by_name("Hero");
    let toast = |s: &Sim| s.app().toast.as_ref().map(|t| t.text.clone()).unwrap_or_default();
    // A layer that is gone, one of the wrong kind, and one with nothing to turn into a style.
    s.shell.app.detach_style_of(&[NodeId(99_999)], StyleProperty::Text);
    assert_eq!(toast(&s), "Style not changed: the layer is gone");
    s.shell.app.create_style_from(hero, StyleProperty::Text);
    assert_eq!(toast(&s), "Style not changed: wrong layer type");
    s.shell.app.create_style_from(hero, StyleProperty::Effect);
    assert_eq!(toast(&s), "Style not changed: nothing to style");
}

const ONLY_NEW_BOX: &str = "Only the new text box was styled";

/// What a layer's type is and which style it is linked to: what a style pick would change.
fn look_of(s: &Sim, id: NodeId) -> (TextStyle, Option<String>) {
    (text_of(s, id).style, link_of(s, id))
}

/// A new text box made inside the auto-layout frame Card with the Text tool, selected together with
/// the card's own text "Quarterly report". Returns (box, sibling).
fn new_box_in_card_with_its_sibling(s: &mut Sim) -> (NodeId, NodeId) {
    let (card, sibling) = (s.by_name("Card"), s.by_name("Quarterly report"));
    // Empty space inside the card, right of the title.
    let at = s.app().doc.world_transform(card).apply(DV::new(250.0, 30.0));
    s.key(Key::T);
    let p = s.screen(at.x, at.y);
    s.click(p);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().canvas.text.as_ref().is_some_and(|t| t.created), "the Text tool opened a session on a new box");
    let id = s.app().selection[0];
    assert_eq!(s.app().doc.get(id).and_then(|n| n.parent), Some(card), "the box is inside the auto-layout frame");
    s.shell.app.selection = vec![id, sibling];
    redraw(s);
    (id, sibling)
}

fn toast_text(s: &Sim) -> Option<String> {
    s.app().toast.as_ref().map(|t| t.text.clone())
}

#[test]
fn a_style_picked_for_a_new_box_and_its_sibling_styles_the_box_alone_and_undo_stays_exact() {
    let mut s = Sim::new();
    let (id, sibling) = new_box_in_card_with_its_sibling(&mut s);
    let before = look_of(&s, sibling);
    pick(&mut s, BUTTON, 1, 0);
    assert_eq!(link_of(&s, id).as_deref(), Some("Heading/Large"));
    let toast = s.app().toast.as_ref().map(|t| (t.text.clone(), t.error)).expect("the pick says what was and was not styled");
    assert_eq!(toast, (ONLY_NEW_BOX.to_string(), false));
    assert_eq!(look_of(&s, sibling), before, "the sibling is untouched");
    s.shell.app.toast = None;
    s.type_text("q");
    s.key(Key::Escape);
    assert_eq!(text_of(&s, id).content, "q");
    assert_eq!(look_of(&s, sibling), before);
    // Undo until nothing is left: no step fails, none is announced, the sibling never changes.
    while s.app().hist.can_undo() {
        s.cmd_key(Key::Z);
        assert_eq!(toast_text(&s), None, "no toast at any undo step");
        assert_eq!(look_of(&s, sibling), before, "the sibling keeps its own style and link");
    }
    s.cmd_key(Key::Z);
    assert_eq!(toast_text(&s), None);
    assert!(s.app().doc.get(id).is_none(), "the box went with its one step");
}

#[test]
fn escape_after_a_pick_for_a_new_box_and_its_sibling_leaves_the_sibling_and_the_history_alone() {
    let mut s = Sim::new();
    let (id, sibling) = new_box_in_card_with_its_sibling(&mut s);
    let (before, depth) = (look_of(&s, sibling), s.app().hist.undo_depth());
    pick(&mut s, BUTTON, 1, 0);
    s.key(Key::Escape);
    assert!(s.app().doc.get(id).is_none(), "the box is empty, so it goes");
    assert_eq!(look_of(&s, sibling), before);
    assert_eq!(s.app().hist.undo_depth(), depth);
}

#[test]
fn cmd_z_after_a_pick_for_a_new_box_and_its_sibling_takes_back_the_box_style_only() {
    let mut s = Sim::new();
    let (id, sibling) = new_box_in_card_with_its_sibling(&mut s);
    let (before, start) = (look_of(&s, sibling), text_of(&s, id).style);
    pick(&mut s, BUTTON, 1, 0);
    s.cmd_key(Key::Z);
    assert!(s.app().canvas.text.is_some(), "the undo stays inside the session");
    assert_eq!(link_of(&s, id), None, "the box loses the link");
    assert_eq!(text_of(&s, id).style, start, "and the style");
    assert_eq!(look_of(&s, sibling), before, "the sibling never changed");
}

#[test]
fn a_new_box_selected_with_another_layer_is_styled_alone_but_detached_together() {
    let mut s = Sim::new();
    let other = s.by_name("Continue");
    s.shell.app.apply_style(&[other], "Body/Regular");
    let id = new_text_box(&mut s);
    s.shell.app.selection = vec![id, other];
    redraw(&mut s);
    let (before, depth) = (look_of(&s, other), s.app().hist.undo_depth());
    pick(&mut s, BUTTON, 1, 0);
    assert_eq!(link_of(&s, id).as_deref(), Some("Heading/Large"));
    assert_eq!(look_of(&s, other), before, "the other layer keeps Body/Regular");
    assert_eq!(s.app().hist.undo_depth(), depth, "nothing is recorded for it");
    assert_eq!(toast_text(&s).as_deref(), Some(ONLY_NEW_BOX));
    assert!(s.app().canvas.text.is_some());
    s.frame(vec![]);
    let detach = s.rect_of("text-style-detach");
    s.click(detach.center());
    assert_eq!((link_of(&s, id), link_of(&s, other)), (None, None), "detaching reaches both");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "the other layer's detach is a step of its own");
    assert!(s.app().canvas.text.is_some() && s.app().doc.get(id).is_some());
}

#[test]
fn the_styles_list_styles_a_new_box_inside_its_session() {
    let mut s = Sim::new();
    let id = new_text_box(&mut s);
    let depth = s.app().hist.undo_depth();
    s.scroll_inspector_down();
    let row = s.rect_of("style:Body/Regular");
    s.click(row.center());
    assert!(s.app().canvas.text.is_some() && s.app().doc.get(id).is_some(), "the box and its session stay");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    assert_eq!(s.app().hist.undo_depth(), depth, "no document step is recorded for a box that is not committed");
    s.key(Key::Escape);
    assert!(s.app().doc.get(id).is_none(), "an empty new box goes when editing ends");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), None, "there is nothing to undo, and nothing is said about a layer");
}

#[test]
fn the_styles_list_ends_a_text_session_before_it_applies_a_style() {
    let mut s = Sim::with(Options { select: Some("Continue".into()), ..Default::default() });
    let id = s.by_name("Continue");
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    let end = text_of(&s, id).content.len();
    {
        let sess = s.shell.app.canvas.text.as_mut().unwrap();
        sess.caret = end;
        sess.anchor = end;
    }
    crate::canvas::textsession::insert_text(&mut s.shell.app, "!", crate::textedit::EditKind::Typing);
    let depth = s.app().hist.undo_depth();
    s.scroll_inspector_down();
    let row = s.rect_of("style:Body/Regular");
    s.click(row.center());
    assert!(s.app().canvas.text.is_none(), "the session ends first");
    assert_eq!(text_of(&s, id).content, "Continue!");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    assert_eq!(s.app().hist.undo_depth(), depth + 2, "the typing is one step and the style another");
}

/// "Continue" and "Welcome back" selected with the same type.
fn two_alike_layers() -> (Sim, NodeId, NodeId) {
    let (mut s, a) = bare_sim("Continue");
    let b = s.by_name("Welcome back");
    let style = text_of(&s, a).style;
    s.shell.app.edit_nodes(&[b], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style = TextStyle { fills: t.style.fills.clone(), ..style.clone() };
        }
    });
    s.shell.app.selection = vec![a, b];
    redraw(&mut s);
    (s, a, b)
}

#[test]
fn creating_from_several_text_layers_with_the_same_type_links_all_of_them_in_one_step() {
    let (mut s, a, b) = two_alike_layers();
    let depth = s.app().hist.undo_depth();
    pick(&mut s, BUTTON, 1, 1);
    assert_eq!(link_of(&s, a).as_deref(), Some("Text style 1"));
    assert_eq!(link_of(&s, b).as_deref(), Some("Text style 1"), "no selected layer is left out");
    assert_eq!(s.app().doc.styles.len(), 1);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one undo step");
    s.frame(vec![]);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.styles.is_empty());
    assert_eq!((link_of(&s, a), link_of(&s, b)), (None, None));
}

#[test]
fn several_text_layers_with_different_type_have_no_single_style_to_create() {
    let (mut s, a, b) = two_alike_layers();
    s.shell.app.edit_nodes(&[b], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.font_size = 40.0;
        }
    });
    redraw(&mut s);
    let row = style_picker::create_style_row(s.app(), &[a, b], StyleProperty::Text);
    assert!(row.disabled);
    assert_eq!(row.tooltip, "Selected text has mixed styles");
    let depth = s.app().hist.undo_depth();
    pick(&mut s, BUTTON, 1, 1);
    assert!(s.app().doc.styles.is_empty());
    assert_eq!((link_of(&s, a), link_of(&s, b)), (None, None), "nothing is left half linked");
    assert_eq!(s.app().hist.undo_depth(), depth);
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
