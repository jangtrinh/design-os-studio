//! Clipboard command events as delivered by egui-winit.
//!
//! `Event::Copy` and `Event::Paste` carry no modifiers. The letter's release does, and it can share a frame with the
//! command when the UI thread was busy across the tap, so the frame's own modifiers (the state after every change in
//! it) may already be empty. A modifier let go before the letter shows only in its own key release, and which key
//! releases are stamped with what depends on the backend: winit's macOS backend queues a modifier's key event before
//! the modifier change (its stamp still has the modifier), its Windows backend changes the modifiers first (it does
//! not). The command key is Super on macOS and Control elsewhere; the tests that depend on it set the platform the
//! context reports.

use super::*;
use egui::os::OperatingSystem;

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

/// The press the OS repeats while the key is held: the key was already down.
fn repeated(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: true, repeat: true, modifiers }
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
    s.ctx.set_os(OperatingSystem::Mac);
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    s.mods = Modifiers::NONE;
    s.frame(vec![
        Event::Paste("clipboard text".into()),
        released(Key::ShiftLeft, command_shift()),
        released(Key::SuperLeft, Modifiers::COMMAND),
        released(Key::V, Modifiers::NONE),
    ]);

    assert_pasted_over(&s, source, target, before);
    assert!(s.app().doc.nodes.values().all(|n| n.name != "clipboard text"), "the clipboard text was not inserted");
}

#[test]
fn an_alt_let_go_before_c_still_copies_properties() {
    let mut s = Sim::new();
    s.ctx.set_os(OperatingSystem::Mac);
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Copy, released(Key::AltLeft, command_alt()), released(Key::SuperLeft, Modifiers::COMMAND), released(Key::C, Modifiers::NONE)]);

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

/// The in-app layer was pasted over `target`, not the clipboard text.
fn assert_pasted_text_not_layers(s: &Sim, source: NodeId, target: NodeId) {
    assert!(s.app().doc.nodes.values().any(|n| n.name == "text"), "the clipboard text was pasted");
    let source_name = s.app().doc.get(source).unwrap().name.clone();
    let children = &s.app().doc.get(target).unwrap().children;
    assert!(children.iter().all(|c| s.app().doc.get(*c).unwrap().name != source_name), "the in-app layer was not pasted over the selection");
}

#[test]
fn a_shift_let_go_after_cmd_on_macos_still_pastes_over_the_selection_and_not_the_clipboard_text() {
    let mut s = Sim::new();
    s.ctx.set_os(OperatingSystem::Mac);
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    s.mods = Modifiers::NONE;
    s.frame(vec![
        Event::Paste("clipboard text".into()),
        released(Key::SuperLeft, command_shift()),
        released(Key::ShiftLeft, Modifiers::SHIFT),
        released(Key::V, Modifiers::NONE),
    ]);

    assert_pasted_over(&s, source, target, before);
    assert!(s.app().doc.nodes.values().all(|n| n.name != "clipboard text"), "the clipboard text was not inserted");
}

#[test]
fn an_alt_let_go_after_cmd_on_macos_still_copies_properties() {
    let mut s = Sim::new();
    s.ctx.set_os(OperatingSystem::Mac);
    let id = s.by_name("Button");
    s.shell.app.selection = vec![id];
    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Copy, released(Key::SuperLeft, command_alt()), released(Key::AltLeft, Modifiers::ALT), released(Key::C, Modifiers::NONE)]);

    assert!(s.app().copied_style.is_some(), "Cmd+Alt+C must copy properties");
    assert!(s.app().clip.is_none(), "Cmd+Alt+C must not copy layers");
}

#[test]
fn a_shift_let_go_after_ctrl_on_windows_still_pastes_over_the_selection_and_not_the_clipboard_text() {
    let mut s = Sim::new();
    // the context decides which key is the command key; egui-winit stamps by the platform it was built for
    s.ctx.set_os(OperatingSystem::Windows);
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    s.mods = Modifiers::NONE;
    s.frame(vec![
        Event::Paste("clipboard text".into()),
        released(Key::ControlLeft, Modifiers::SHIFT),
        released(Key::ShiftLeft, Modifiers::NONE),
        released(Key::V, Modifiers::NONE),
    ]);

    assert_pasted_over(&s, source, target, before);
    assert!(s.app().doc.nodes.values().all(|n| n.name != "clipboard text"), "the clipboard text was not inserted");
}

#[test]
fn a_shift_let_go_while_v_is_still_down_adds_to_the_frame_modifiers() {
    let mut s = Sim::new();
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    s.mods = Modifiers::COMMAND;
    s.frame(vec![Event::Paste("clipboard text".into()), released(Key::ShiftLeft, command_shift())]);
    s.mods = Modifiers::NONE;

    assert_pasted_over(&s, source, target, before);
}

#[test]
fn a_shift_released_with_the_shift_bit_already_clear_still_counts() {
    let mut s = Sim::new();
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    s.mods = Modifiers::NONE;
    s.frame(vec![Event::Paste("clipboard text".into()), released(Key::ShiftLeft, Modifiers::COMMAND), released(Key::V, Modifiers::COMMAND)]);

    assert_pasted_over(&s, source, target, before);
}

#[test]
fn a_paste_with_shift_and_no_command_stays_a_plain_system_paste() {
    // Shift+Insert on Windows and Linux: an `Event::Paste` with Shift held and no V
    let mut s = Sim::new();
    let (source, target, _) = select_a_layer_with_layers_copied(&mut s);
    s.mods = Modifiers::SHIFT;
    s.frame(vec![Event::Paste("text".into())]);
    s.mods = Modifiers::NONE;
    assert_pasted_text_not_layers(&s, source, target);

    let mut s = Sim::new();
    let (source, target, _) = select_a_layer_with_layers_copied(&mut s);
    s.mods = Modifiers::SHIFT;
    s.frame(vec![Event::Paste("text".into()), released(Key::ShiftLeft, Modifiers::SHIFT)]);
    s.mods = Modifiers::NONE;
    assert_pasted_text_not_layers(&s, source, target);
}

#[test]
fn a_command_key_pressed_after_the_paste_does_not_turn_a_text_paste_into_a_paste_over() {
    let mut s = Sim::new();
    s.ctx.set_os(OperatingSystem::Windows);
    let (source, target, _) = select_a_layer_with_layers_copied(&mut s);

    // Shift+Insert with Control tapped before Shift is let go: Shift was held at the paste, Control was not
    s.mods = Modifiers::NONE;
    s.frame(vec![
        Event::Paste("text".into()),
        pressed(Key::ControlLeft, command_shift()),
        released(Key::ShiftLeft, Modifiers::COMMAND),
        released(Key::ControlLeft, Modifiers::NONE),
    ]);

    assert_pasted_text_not_layers(&s, source, target);
}

#[test]
fn a_repeated_shift_press_in_the_batch_does_not_hide_the_shift_let_go() {
    let mut s = Sim::new();
    s.ctx.set_os(OperatingSystem::Windows);
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    // Shift went down in an earlier frame and was already down at the paste, so the OS repeating it is not the
    // start of the hold (egui marks a press as a repeat when the key is already down)
    s.mods = command_shift();
    s.frame(vec![pressed(Key::ShiftLeft, command_shift())]);
    s.mods = Modifiers::NONE;
    s.frame(vec![
        Event::Paste("clipboard text".into()),
        repeated(Key::ShiftLeft, command_shift()),
        released(Key::ShiftLeft, Modifiers::COMMAND),
        released(Key::V, Modifiers::COMMAND),
    ]);

    assert_pasted_over(&s, source, target, before);
    assert!(s.app().doc.nodes.values().all(|n| n.name != "clipboard text"), "the clipboard text was not inserted");
}

#[test]
fn a_repeated_control_press_in_the_batch_does_not_hide_the_command_let_go() {
    let mut s = Sim::new();
    s.ctx.set_os(OperatingSystem::Windows);
    let (source, target, before) = select_a_layer_with_layers_copied(&mut s);

    // Control went down in an earlier frame, so the repeat is not the start of its hold
    s.mods = command_shift();
    s.frame(vec![pressed(Key::ControlLeft, command_shift())]);
    s.mods = Modifiers::NONE;
    s.frame(vec![
        Event::Paste("clipboard text".into()),
        repeated(Key::ControlLeft, command_shift()),
        released(Key::ControlLeft, Modifiers::SHIFT),
        released(Key::ShiftLeft, Modifiers::NONE),
        released(Key::V, Modifiers::NONE),
    ]);

    assert_pasted_over(&s, source, target, before);
    assert!(s.app().doc.nodes.values().all(|n| n.name != "clipboard text"), "the clipboard text was not inserted");
}

#[test]
fn a_repeated_paste_after_shift_was_let_go_pastes_what_is_still_held() {
    let mut s = Sim::new();
    s.ctx.set_os(OperatingSystem::Mac);
    let (source, target, _) = select_a_layer_with_layers_copied(&mut s);

    // Cmd+Shift+V held, Shift let go, V repeats: the first paste had Shift, the second has Cmd alone
    s.mods = Modifiers::NONE;
    s.frame(vec![
        Event::Paste("clipboard text".into()),
        released(Key::ShiftLeft, command_shift()),
        Event::Paste("clipboard text".into()),
        released(Key::V, Modifiers::COMMAND),
    ]);

    let source_name = s.app().doc.get(source).unwrap().name.clone();
    let pasted_over = s.app().doc.get(target).unwrap().children.iter().filter(|c| s.app().doc.get(**c).unwrap().name == source_name).count();
    assert_eq!(pasted_over, 1, "the in-app layer went over the selection once, by the first paste");
    assert!(s.app().doc.nodes.values().any(|n| n.name == "clipboard text"), "the second paste is a plain paste of the clipboard text");
}
