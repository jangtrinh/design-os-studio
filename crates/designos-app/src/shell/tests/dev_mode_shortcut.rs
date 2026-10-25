//! Shift+D switches Dev Mode on and off, as the toolbar's `Exit Dev Mode   Shift+D` tooltip says. It is a canvas
//! shortcut: a text field, a text session or the agent's terminal that has the keyboard gets the D, Alt+D still
//! aligns right, and a board, which has no Dev Mode, ignores it.

use super::*;
use crate::app::RightTab;
use crate::canvas::tools::Tool;

/// A capital `D` as a keyboard makes it: egui delivers the key with Shift held and its text in the same frame.
pub(super) fn type_capital_d(s: &mut Sim) {
    s.mods = Modifiers::SHIFT;
    s.one_frame().key(Key::D).text("D").send();
    s.mods = Modifiers::NONE;
    s.frame(vec![]);
}

/// The state both Dev Mode buttons leave behind: the mode and the tab the right panel shows.
fn dev_state(s: &Sim) -> (bool, Option<RightTab>) {
    (s.app().ui.dev_mode, s.app().ui.right_tab)
}

/// A window whose right panel shows the Prototype tab, so a switch that forgets to bring back the Design tab shows.
fn sim_on_prototype_tab() -> Sim {
    let mut s = Sim::new();
    s.shell.app.ui.right_tab = Some(RightTab::Prototype);
    s.frame(vec![]);
    s
}

#[test]
fn shift_d_enters_dev_mode_like_the_toolbar_button() {
    let mut by_button = sim_on_prototype_tab();
    assert!(!by_button.app().ui.dev_mode, "precondition: the window starts in Design mode");
    let segment = by_button.rect_of("toolbar-mode-dev");
    by_button.click(segment.center());
    assert!(by_button.app().ui.dev_mode, "precondition: the </> segment enters Dev Mode");

    let mut by_key = sim_on_prototype_tab();
    type_capital_d(&mut by_key);
    assert!(by_key.app().ui.dev_mode, "Shift+D enters Dev Mode");
    assert_eq!(dev_state(&by_key), dev_state(&by_button), "the same mode and the same right tab as the </> button");
    assert_eq!(by_key.app().ui.right_tab, Some(RightTab::Design), "Dev Mode opens on its Inspect tab");
    by_key.rect_of("dev-mode-toggle");
}

#[test]
fn shift_d_leaves_dev_mode_like_the_exit_button() {
    let in_dev_mode_on_plugins_tab = || {
        let mut s = sim_on_prototype_tab();
        let segment = s.rect_of("toolbar-mode-dev");
        s.click(segment.center());
        assert!(s.app().ui.dev_mode, "precondition: Dev Mode is on");
        s.shell.app.ui.right_tab = Some(RightTab::Prototype);
        s.frame(vec![]);
        s
    };

    let mut by_button = in_dev_mode_on_plugins_tab();
    let exit = by_button.rect_of("dev-mode-toggle");
    by_button.click(exit.center());
    assert!(!by_button.app().ui.dev_mode, "precondition: the Exit button leaves Dev Mode");

    let mut by_key = in_dev_mode_on_plugins_tab();
    type_capital_d(&mut by_key);
    assert!(!by_key.app().ui.dev_mode, "Shift+D again leaves Dev Mode");
    assert_eq!(dev_state(&by_key), dev_state(&by_button), "the same mode and the same right tab as the Exit button");
    assert_eq!(by_key.app().ui.right_tab, Some(RightTab::Design));
    by_key.rect_of("toolbar-mode-dev");
}

#[test]
fn a_capital_d_typed_into_a_layer_rename_does_not_toggle_dev_mode() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    s.shell.app.rename_selection();
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().layers.renaming, Some(hero), "precondition: the layer name is being edited");
    type_capital_d(&mut s);
    assert!(!s.app().ui.dev_mode, "the D belongs to the name field");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(hero).unwrap().name, "D", "the capital D was typed into the name");
    assert!(!s.app().ui.dev_mode);
}

#[test]
fn a_capital_d_typed_into_text_on_the_canvas_does_not_toggle_dev_mode() {
    let mut s = Sim::new();
    s.key(Key::T);
    let p = s.screen(300.0, 820.0);
    s.click(p);
    assert!(s.app().canvas.text.is_some(), "precondition: a text session is open");
    type_capital_d(&mut s);
    assert!(!s.app().ui.dev_mode, "the D belongs to the text");
    s.key(Key::Escape);
    let id = s.app().selection[0];
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Text(t) => assert_eq!(t.content, "D"),
        k => panic!("unexpected {k:?}"),
    }
}

#[test]
fn alt_d_still_aligns_right_and_plain_d_does_nothing() {
    let mut s = Sim::new();
    let (welcome, dashboard) = (s.by_name("Welcome"), s.by_name("Dashboard"));
    let right_edge = |s: &Sim, id| {
        let r = crate::util::world_aabb(&s.app().doc, id);
        r.x + r.w
    };
    s.shell.app.selection = vec![welcome, dashboard];
    s.frame(vec![]);
    assert_ne!(right_edge(&s, welcome), right_edge(&s, dashboard), "precondition: the two frames end at different x");

    s.key(Key::D);
    assert!(!s.app().ui.dev_mode, "a plain D does not switch Dev Mode");
    assert_eq!(s.app().tool, Tool::Move, "a plain D picks no tool");
    assert!(!s.app().hist.can_undo(), "a plain D edits nothing");

    s.mods = Modifiers::ALT;
    s.key(Key::D);
    s.mods = Modifiers::NONE;
    assert_eq!(right_edge(&s, welcome), right_edge(&s, dashboard), "Alt+D aligns the selection to the right");
    assert!(!s.app().ui.dev_mode, "Alt+D is not Shift+D");
}

#[test]
fn shift_d_does_nothing_on_a_board() {
    let mut s = Sim::with(Options { fixture: Some("board-empty".into()), ..Default::default() });
    assert!(s.app().is_board(), "precondition: a board has no </> button");
    type_capital_d(&mut s);
    assert!(!s.app().ui.dev_mode, "a board has no Dev Mode to enter");
}

#[test]
fn the_shortcuts_dialog_lists_shift_d() {
    let mut s = Sim::new();
    s.shell.app.show_shortcuts();
    let rows = &s.app().dialog.as_ref().expect("the shortcuts dialog is open").lines;
    assert!(rows.iter().any(|(keys, action)| keys == "Shift+D" && action == "Toggle dev mode"), "{rows:?}");
}
