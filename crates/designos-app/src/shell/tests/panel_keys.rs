//! Keys that arrive in the same frame as a click on a panel field belong to that field, and Tab walks
//! the panel fields in the order they are drawn.

use super::*;
use designos_doc::PaintKind;

/// The key events a keyboard makes for `text`: press, text and release for every character, with
/// Shift held for an uppercase letter.
fn keyboard_events(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    for ch in text.chars() {
        let key = Key::from_name(&ch.to_string()).unwrap_or_else(|| panic!("no key for {ch:?}"));
        let modifiers = if ch.is_ascii_uppercase() { Modifiers::SHIFT } else { Modifiers::NONE };
        events.push(Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers });
        events.push(Event::Text(ch.to_string()));
        events.push(Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers });
    }
    events
}

/// One frame holding a primary click at `p` and `text` typed right after it: what a slow frame
/// delivers when the user clicks a field and starts typing at once.
fn click_and_type_in_one_frame(s: &mut Sim, p: Pos2, text: &str) {
    let mut frame = s.one_frame().move_to(p).press().release();
    frame.events.extend(keyboard_events(text));
    frame.send();
    s.frame(vec![]);
}

fn click_field(s: &mut Sim, key: &str) {
    let r = s.field_rect(key);
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
}

fn tab(s: &mut Sim, shift: bool) {
    s.mods = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
    s.key(Key::Tab);
    s.mods = Modifiers::NONE;
}

fn first_fill_hex(s: &Sim, id: NodeId) -> String {
    match &s.app().doc.get(id).unwrap().fills[0].kind {
        PaintKind::Solid(c) => c.to_hex().trim_start_matches('#').to_uppercase(),
        other => panic!("not a solid fill: {other:?}"),
    }
}

/// A frame with an unbound solid fill and a Layout section. The Card of the sample file has its
/// fill bound to a variable, which shows a pill instead of a hex field, and auto layout.
fn frame_sim() -> (Sim, NodeId) {
    let s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let id = s.by_name("Welcome");
    (s, id)
}

fn hex_field_point(s: &Sim) -> Pos2 {
    let sw = s.rect_of("sw:false:0");
    Pos2::new(sw.right() + 20.0, sw.center().y)
}

#[test]
fn a_hex_typed_right_after_the_click_reaches_the_field_not_the_shortcuts() {
    let (mut s, id) = frame_sim();
    let opacity = s.app().doc.get(id).unwrap().opacity;
    let tool = s.app().tool;
    let p = hex_field_point(&s);
    click_and_type_in_one_frame(&mut s, p, "34C759");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().opacity, opacity, "the digits must not set the opacity");
    assert_eq!(s.app().tool, tool, "the letters must not pick a tool");
    assert_eq!(first_fill_hex(&s, id), "34C759");
}

#[test]
fn lowercase_letters_typed_right_after_the_click_do_not_pick_tools() {
    let (mut s, id) = frame_sim();
    let tool = s.app().tool;
    let p = hex_field_point(&s);
    click_and_type_in_one_frame(&mut s, p, "ff0c9a");
    s.key(Key::Enter);
    assert_eq!(s.app().tool, tool, "f and c are the Frame and Comment shortcuts");
    assert_eq!(first_fill_hex(&s, id), "FF0C9A");
}

#[test]
fn digits_typed_right_after_a_click_on_a_number_field_reach_the_field() {
    let (mut s, id) = frame_sim();
    let opacity = s.app().doc.get(id).unwrap().opacity;
    let r = s.field_rect("x");
    click_and_type_in_one_frame(&mut s, Pos2::new(r.left() + 40.0, r.center().y), "70");
    s.type_text("0");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().transform.tx, 700.0, "the caret is after the keys of the click frame, so the next key extends them");
    assert_eq!(s.app().doc.get(id).unwrap().opacity, opacity);
}

#[test]
fn digits_do_not_run_as_shortcuts_while_a_field_has_focus() {
    let (mut s, id) = frame_sim();
    let opacity = s.app().doc.get(id).unwrap().opacity;
    click_field(&mut s, "x");
    s.frame(keyboard_events("5"));
    assert_eq!(s.app().doc.get(id).unwrap().opacity, opacity);
}

#[test]
fn shortcuts_still_run_when_no_field_has_focus() {
    let (mut s, id) = frame_sim();
    s.key(Key::Num3);
    assert!((s.app().doc.get(id).unwrap().opacity - 0.3).abs() < 1e-4, "3 sets the opacity to 30%");
    s.key(Key::F);
    assert_eq!(s.app().tool, Tool::Frame, "F picks the Frame tool");
}

#[test]
fn a_click_on_the_canvas_and_a_shortcut_in_the_same_frame_still_run_the_shortcut() {
    let (mut s, id) = frame_sim();
    let c = aabb_center(&s, id);
    let p = s.screen(c.x, c.y);
    s.mv(p);
    s.frame(vec![]);
    click_and_type_in_one_frame(&mut s, p, "3");
    // the shortcuts run before the canvas takes the click, so they act on the layer that was selected
    assert!((s.app().doc.get(id).unwrap().opacity - 0.3).abs() < 1e-4, "3 sets the opacity to 30%: only a click on a field takes the keys");
}

#[test]
fn tab_walks_the_position_and_size_fields_in_order_and_shift_tab_walks_back() {
    let (mut s, id) = frame_sim();
    click_field(&mut s, "x");
    s.type_text("101");
    tab(&mut s, false);
    s.type_text("102");
    tab(&mut s, false);
    s.type_text("0");
    tab(&mut s, false);
    s.type_text("301");
    tab(&mut s, false);
    s.type_text("202");
    s.key(Key::Enter);
    let n = s.app().doc.get(id).unwrap();
    assert_eq!((n.transform.tx, n.transform.ty), (101.0, 102.0), "X then Y");
    assert_eq!((n.size.x, n.size.y), (301.0, 202.0), "rotation sits between Y and W, then W then H");
    // back from H: W takes the next value
    click_field(&mut s, "h");
    tab(&mut s, true);
    s.type_text("303");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().size.x, 303.0, "Shift+Tab from H lands on W");
    assert_eq!(s.app().doc.get(id).unwrap().size.y, 202.0);
}

#[test]
fn tab_goes_on_from_the_size_fields_into_the_appearance_section_and_back() {
    let (mut s, id) = frame_sim();
    click_field(&mut s, "h");
    tab(&mut s, false);
    s.type_text("50");
    s.key(Key::Enter);
    assert!((s.app().doc.get(id).unwrap().opacity - 0.5).abs() < 1e-4, "Tab from H lands on the opacity field of the next section");
    click_field(&mut s, "opacity");
    tab(&mut s, true);
    s.type_text("250");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().size.y, 250.0, "Shift+Tab from opacity lands on H");
}

#[test]
fn a_held_tab_moves_one_field_per_press() {
    let (mut s, id) = frame_sim();
    click_field(&mut s, "x");
    // the keyboard repeats Tab every other frame while it is held: egui must not walk its own order in between
    for repeat in [false, true, true] {
        s.frame(vec![Event::Key { key: Key::Tab, physical_key: Some(Key::Tab), pressed: true, repeat, modifiers: Modifiers::NONE }]);
        s.frame(vec![]);
    }
    s.type_text("333");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().size.x, 333.0, "three presses from X land on W: Y, rotation, W");
}

#[test]
fn tabbing_into_a_number_field_selects_its_whole_value() {
    let (mut s, id) = frame_sim();
    let y0 = s.app().doc.get(id).unwrap().transform.ty;
    click_field(&mut s, "x");
    tab(&mut s, false);
    s.type_text("9");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().transform.ty, 9.0, "typing replaced {y0} instead of extending it");
}

#[test]
fn clicking_a_number_field_selects_its_whole_value() {
    let (mut s, id) = frame_sim();
    click_field(&mut s, "y");
    s.type_text("9");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().transform.ty, 9.0);
}

#[test]
fn the_scrub_prefix_is_not_a_tab_stop_and_still_scrubs() {
    let (mut s, id) = frame_sim();
    click_field(&mut s, "x");
    tab(&mut s, false);
    // the stop after X is Y's value, not Y's scrub label: the label cannot take text
    s.type_text("8");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().transform.ty, 8.0);
    let x0 = s.app().doc.get(id).unwrap().transform.tx;
    let a = prefix_point(&s, "x");
    s.drag(a, a + Vec2::new(20.0, 0.0));
    let x1 = s.app().doc.get(id).unwrap().transform.tx;
    assert!((x1 - x0 - 20.0).abs() <= 1.0, "dragging the prefix still scrubs: {}", x1 - x0);
}

#[test]
fn tab_with_no_field_focused_selects_the_next_layer() {
    let (mut s, id) = frame_sim();
    tab(&mut s, false);
    assert_eq!(s.app().selection.len(), 1);
    assert_ne!(s.app().selection[0], id, "Tab moves on to the next sibling");
    tab(&mut s, true);
    assert_eq!(s.app().selection, vec![id], "Shift+Tab comes back");
}

#[test]
fn tab_in_a_layer_rename_commits_it_and_renames_the_next_row() {
    let (mut s, id) = frame_sim();
    s.shell.app.layers.renaming = Some(id);
    s.frame(vec![]);
    s.frame(vec![]);
    s.type_text("Intro");
    tab(&mut s, false);
    assert_eq!(s.app().doc.get(id).unwrap().name, "Intro", "Tab commits the name that was typed");
    let next = s.app().layers.renaming;
    assert!(next.is_some_and(|n| n != id), "the rename moves on to the next layer row, not to a panel field: {next:?}");
}

fn tx_of(s: &Sim, id: NodeId) -> f32 {
    s.app().doc.get(id).unwrap().transform.tx
}

fn ty_of(s: &Sim, id: NodeId) -> f32 {
    s.app().doc.get(id).unwrap().transform.ty
}

#[test]
fn shift_tab_from_a_typed_rotation_does_not_write_the_old_position_back() {
    let (mut s, id) = frame_sim();
    let ty0 = ty_of(&s, id);
    click_field(&mut s, "rot");
    s.type_text("45");
    tab(&mut s, true);
    let moved = ty_of(&s, id);
    assert!((moved - ty0).abs() > 0.5, "the rotation committed on Shift+Tab and moved the layer: {ty0} -> {moved}");
    s.key(Key::Enter);
    assert_eq!(ty_of(&s, id), moved, "Enter on the untouched Y must not write the position from before the rotation back");
}

#[test]
fn clicking_a_field_above_a_typed_rotation_does_not_write_the_old_position_back() {
    let (mut s, id) = frame_sim();
    let ty0 = ty_of(&s, id);
    click_field(&mut s, "rot");
    s.type_text("45");
    click_field(&mut s, "y");
    let moved = ty_of(&s, id);
    assert!((moved - ty0).abs() > 0.5, "the rotation committed when Y was clicked: {ty0} -> {moved}");
    s.key(Key::Enter);
    assert_eq!(ty_of(&s, id), moved, "Enter on the untouched Y must not write a stale value back");
}

#[test]
fn a_field_that_was_edited_keeps_what_was_typed_when_the_focus_moves_on() {
    let (mut s, id) = frame_sim();
    click_field(&mut s, "rot");
    s.type_text("45");
    tab(&mut s, true);
    s.type_text("77");
    s.key(Key::Enter);
    assert_eq!(ty_of(&s, id), 77.0, "Y was typed into after the rotation moved it, so Enter writes what was typed");
}

/// Types into X, presses Escape (egui drops the focus, and the shortcut handler clears the selection), and
/// selects the layer again, so that X draws again with whatever the first focus left behind.
fn type_into_x_then_escape_and_reselect(s: &mut Sim, id: NodeId) {
    click_field(s, "x");
    s.type_text("12345");
    s.key(Key::Escape);
    s.shell.app.select_only(id);
    s.frame(vec![]);
    s.frame(vec![]);
}

#[test]
fn text_typed_before_escape_does_not_come_back_when_the_field_is_clicked() {
    let (mut s, id) = frame_sim();
    let x0 = tx_of(&s, id);
    type_into_x_then_escape_and_reselect(&mut s, id);
    assert_eq!(tx_of(&s, id), x0, "Escape drops what was typed");
    click_field(&mut s, "x");
    s.key(Key::Enter);
    assert_eq!(tx_of(&s, id), x0, "clicking X and pressing Enter without typing must not write the text typed before Escape");
}

#[test]
fn text_typed_before_escape_does_not_come_back_when_tab_moves_into_the_field() {
    let (mut s, id) = frame_sim();
    let x0 = tx_of(&s, id);
    type_into_x_then_escape_and_reselect(&mut s, id);
    click_field(&mut s, "y");
    tab(&mut s, true);
    s.key(Key::Enter);
    assert_eq!(tx_of(&s, id), x0, "Shift+Tab into X and Enter without typing must not write the text typed before Escape");
}

#[test]
fn tab_in_a_page_rename_commits_it_and_does_not_jump_into_the_design_panel() {
    let (mut s, id) = frame_sim();
    let page = s.app().page;
    let (x0, y0, name0) = (tx_of(&s, id), ty_of(&s, id), s.app().doc.get(id).unwrap().name.clone());
    s.shell.app.layers.renaming_page = Some(page);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(crate::widgets::field_tab::a_field_has_focus(&s.ctx), "the page rename has the focus");
    s.type_text("Cover");
    tab(&mut s, false);
    assert_eq!(s.app().doc.get(page).unwrap().name, "Cover", "Tab commits the page name, as Enter does");
    assert!(!crate::widgets::field_tab::a_field_has_focus(&s.ctx), "the focus did not land in a field of the Design panel");
    s.type_text("9");
    s.frame(vec![]);
    assert_eq!((tx_of(&s, id), ty_of(&s, id), s.app().doc.get(id).unwrap().name.clone()), (x0, y0, name0), "the layer was not edited");
}
