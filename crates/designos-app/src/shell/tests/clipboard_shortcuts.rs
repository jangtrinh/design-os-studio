//! Clipboard command events as delivered by egui-winit.

use super::*;

fn command_alt() -> Modifiers {
    Modifiers { alt: true, ..Modifiers::COMMAND }
}

fn command_shift() -> Modifiers {
    Modifiers { shift: true, ..Modifiers::COMMAND }
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
    let source = s.by_name("Button");
    s.shell.app.selection = vec![source];
    s.shell.app.duplicate_selection();
    let target = s.app().selection[0];
    s.shell.app.edit_nodes(&[source], |n| n.opacity = 0.31);
    s.shell.app.edit_nodes(&[target], |n| n.opacity = 0.82);
    s.shell.app.selection = vec![source];
    s.shell.app.copy_properties();
    s.shell.app.selection = vec![target];

    s.mods = command_alt();
    s.frame(vec![Event::Paste("clipboard text".into())]);
    s.mods = Modifiers::NONE;

    assert!((s.app().doc.get(target).unwrap().opacity - 0.31).abs() < 1e-6, "Cmd+Alt+V must paste properties");
}

#[test]
fn paste_event_with_shift_routes_to_paste_over_selection() {
    let mut s = Sim::new();
    let source = s.by_name("Dashboard");
    let target = s.by_name("Welcome");
    s.shell.app.selection = vec![source];
    s.shell.app.copy_selection().expect("copy source layers");
    s.shell.app.selection = vec![target];
    let before = s.app().doc.get(target).unwrap().children.len();

    s.mods = command_shift();
    s.frame(vec![Event::Paste("clipboard text".into())]);
    s.mods = Modifiers::NONE;

    assert_eq!(s.app().doc.get(target).unwrap().children.len(), before + 1, "Cmd+Shift+V must paste the in-app copy over the selection");
    assert_eq!(s.app().selection.len(), 1);
    let pasted = s.app().doc.get(s.app().selection[0]).unwrap();
    assert_eq!(pasted.parent, Some(target));
    assert_eq!(pasted.name, s.app().doc.get(source).unwrap().name, "the in-app layer, not system text, was pasted");
}
