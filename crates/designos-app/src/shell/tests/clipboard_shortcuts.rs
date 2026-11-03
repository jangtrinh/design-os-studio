//! Clipboard command events as delivered by egui-winit.
//!
//! `Event::Copy` and `Event::Paste` carry no modifiers. The letter's release does, and it can share a frame with the
//! command when the UI thread was busy across the tap, so the frame's own modifiers (the state after every change in
//! it) may already be empty. A Shift or Alt let go before the letter shows only in its own key release: winit's macOS
//! backend queues that key event before the modifier change, so it is still stamped with the modifier while the
//! letter's release is not.

use super::*;

fn command_alt() -> Modifiers {
    Modifiers { alt: true, ..Modifiers::COMMAND }
}

fn command_shift() -> Modifiers {
    Modifiers { shift: true, ..Modifiers::COMMAND }
}

fn released(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers }
}

fn pressed(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }
}

/// A copied style on `Button` (opacity 0.31) with a different layer selected; returns that layer.
fn select_a_layer_with_a_style_copied(s: &mut Sim) -> NodeId {
    let source = s.by_name("Button");
    s.shell.app.selection = vec![source];
    s.shell.app.duplicate_selection();
    let target = s.app().selection[0];
    s.shell.app.edit_nodes(&[source], |n| n.opacity = 0.31);
    s.shell.app.edit_nodes(&[target], |n| n.opacity = 0.82);
    s.shell.app.selection = vec![source];
    s.shell.app.copy_properties();
    s.shell.app.selection = vec![target];
    target
}

/// The in-app copy of `Dashboard` with `Welcome` selected; returns `Dashboard`, `Welcome` and the children `Welcome` has.
fn select_a_layer_with_layers_copied(s: &mut Sim) -> (NodeId, NodeId, usize) {
    let source = s.by_name("Dashboard");
    let target = s.by_name("Welcome");
    s.shell.app.selection = vec![source];
    s.shell.app.copy_selection().expect("copy source layers");
    s.shell.app.selection = vec![target];
    let before = s.app().doc.get(target).unwrap().children.len();
    (source, target, before)
}

fn assert_pasted_over(s: &Sim, source: NodeId, target: NodeId, before: usize) {
    assert_eq!(s.app().doc.get(target).unwrap().children.len(), before + 1, "the in-app copy goes over the selection");
    assert_eq!(s.app().selection.len(), 1);
    let pasted = s.app().doc.get(s.app().selection[0]).unwrap();
    assert_eq!(pasted.parent, Some(target));
    assert_eq!(pasted.name, s.app().doc.get(source).unwrap().name, "the in-app layer, not system text, was pasted");
}

#[test]
fn copy_event_with_alt_routes_to_copy_properties() {
    let mut s = Sim::new();
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = command_alt();
    s.frame(vec![Event::Copy]);
    s.mods = Modifiers::NONE;

    assert!(s.app().copied_style.is_some(), "Cmd+Alt+C must copy properties");
    assert!(s.app().clip.is_none(), "Cmd+Alt+C must not copy layers");
}

#[test]
fn paste_event_with_alt_routes_to_paste_properties() {
    let mut s = Sim::new();
    let target = select_a_layer_with_a_style_copied(&mut s);

    s.mods = command_alt();
    s.frame(vec![Event::Paste("clipboard text".into())]);
    s.mods = Modifiers::NONE;

    assert!((s.app().doc.get(target).unwrap().opacity - 0.31).abs() < 1e-6, "Cmd+Alt+V must paste properties");
}

#[test]
fn paste_event_with_shift_routes_to_paste_over_selection() {
    let mut s = Sim::new();
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    s.mods = command_shift();
    s.frame(vec![Event::Paste("clipboard text".into())]);
    s.mods = Modifiers::NONE;

    assert_pasted_over(&s, source, target, before);
}

#[test]
fn copy_properties_chord_whose_modifiers_are_up_by_the_end_of_the_frame_still_copies_properties() {
    let mut s = Sim::new();
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Copy, released(Key::C, command_alt())]);

    assert!(s.app().copied_style.is_some(), "Cmd+Alt+C must copy properties");
    assert!(s.app().clip.is_none(), "Cmd+Alt+C must not copy layers");
}

#[test]
fn paste_properties_chord_whose_modifiers_are_up_by_the_end_of_the_frame_still_pastes_properties() {
    let mut s = Sim::new();
    let target = select_a_layer_with_a_style_copied(&mut s);

    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Paste("clipboard text".into()), released(Key::V, command_alt())]);

    assert!((s.app().doc.get(target).unwrap().opacity - 0.31).abs() < 1e-6, "Cmd+Alt+V must paste properties");
}

#[test]
fn paste_over_chord_whose_modifiers_are_up_by_the_end_of_the_frame_still_pastes_over_the_selection() {
    let mut s = Sim::new();
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Paste("clipboard text".into()), released(Key::V, command_shift())]);

    assert_pasted_over(&s, source, target, before);
}

#[test]
fn a_release_that_came_before_the_copy_belongs_to_an_earlier_tap() {
    let mut s = Sim::new();
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = Modifiers::COMMAND;
    s.frame(vec![released(Key::C, command_alt()), Event::Copy]);

    assert!(s.app().copied_style.is_none(), "the plain copy after an earlier Cmd+Alt+C tap is not a properties copy");
    assert!(s.app().clip.is_some(), "the plain copy copies the layers");
}

#[test]
fn a_shift_let_go_before_v_still_pastes_over_the_selection_and_not_the_clipboard_text() {
    let mut s = Sim::new();
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Paste("clipboard text".into()), released(Key::ShiftLeft, command_shift()), released(Key::V, Modifiers::NONE)]);

    assert_pasted_over(&s, source, target, before);
    assert!(s.app().doc.nodes.values().all(|n| n.name != "clipboard text"), "the clipboard text was not inserted");
}

#[test]
fn an_alt_let_go_before_c_still_copies_properties() {
    let mut s = Sim::new();
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Copy, released(Key::AltLeft, command_alt()), released(Key::C, Modifiers::NONE)]);

    assert!(s.app().copied_style.is_some(), "Cmd+Alt+C must copy properties");
    assert!(s.app().clip.is_none(), "Cmd+Alt+C must not copy layers");
}

#[test]
fn an_alt_that_goes_down_after_the_letter_went_up_is_not_part_of_the_copy() {
    let mut s = Sim::new();
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Copy, released(Key::C, Modifiers::COMMAND), pressed(Key::AltLeft, Modifiers::COMMAND)]);

    assert!(s.app().copied_style.is_none(), "an Alt tap that follows a plain copy is not a properties copy");
    assert!(s.app().clip.is_some(), "the plain copy copies the layers");
}

#[test]
fn an_alt_tapped_while_c_is_still_down_is_not_part_of_the_copy() {
    let mut s = Sim::new();
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Copy, pressed(Key::AltLeft, Modifiers::COMMAND), released(Key::AltLeft, command_alt()), released(Key::C, Modifiers::COMMAND)]);

    assert!(s.app().copied_style.is_none(), "an Alt that began after the copy is not a properties copy");
    assert!(s.app().clip.is_some(), "the plain copy copies the layers");
}

#[test]
fn a_copy_whose_letter_is_still_down_takes_the_modifiers_of_the_frame() {
    let mut s = Sim::new();
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = command_alt();
    s.frame(vec![Event::Copy, Event::PointerMoved(Pos2::new(700.0, 500.0))]);
    s.mods = Modifiers::NONE;

    assert!(s.app().copied_style.is_some(), "the letter is still down, so the frame is the chord");
    assert!(s.app().clip.is_none());
}
