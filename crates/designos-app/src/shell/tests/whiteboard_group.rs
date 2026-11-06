//! The Whiteboard group of the Design toolbar through the whole window: the `W` key, the board toolbar.

use super::*;

fn reduced(s: &mut Sim) {
    s.shell.app.wb_group.reduced_motion_override = Some(true);
}

#[test]
fn w_toggles_the_group_in_a_design_file() {
    let mut s = Sim::new();
    reduced(&mut s);
    assert_eq!(s.app().doc.kind, designos_doc::DocumentKind::Design);
    assert!(!s.app().wb_group.open, "collapsed by default");
    s.key(Key::W);
    assert!(s.app().wb_group.open);
    s.frame(vec![]);
    s.rect_of("toolbar-whiteboard-tool-0");
    s.key(Key::W);
    assert!(!s.app().wb_group.open);
}

#[test]
fn the_group_button_and_w_agree() {
    let mut s = Sim::new();
    reduced(&mut s);
    s.frame(vec![]);
    let btn = s.rect_of("toolbar-whiteboard-group");
    s.click(btn.center());
    assert!(s.app().wb_group.open);
    s.key(Key::W);
    assert!(!s.app().wb_group.open);
}

#[test]
fn w_does_not_toggle_while_a_modifier_is_held() {
    let mut s = Sim::new();
    reduced(&mut s);
    s.mods = Modifiers::SHIFT;
    s.key(Key::W);
    s.mods = Modifiers::NONE;
    assert!(!s.app().wb_group.open);
}

#[test]
fn a_board_file_keeps_its_toolbar_and_ignores_w() {
    let mut s = Sim::with(Options { fixture: Some("board-empty".into()), ..Default::default() });
    assert!(s.app().is_board());
    s.frame(vec![]);
    let width = crate::board::toolbar::toolbar_width();
    s.key(Key::W);
    s.frame(vec![]);
    assert!(!s.app().wb_group.open, "W does nothing on a board");
    let has = |s: &Sim, n: &str| s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", n.to_string())))).is_some();
    assert!(!has(&s, "toolbar-whiteboard-group"), "no Whiteboard group on the board toolbar");
    assert_eq!(crate::board::toolbar::toolbar_width(), width);
    assert_eq!(crate::panels::toolbar::bar_extra_settled(s.app(), s.ctx.content_rect()), 0.0);
}
