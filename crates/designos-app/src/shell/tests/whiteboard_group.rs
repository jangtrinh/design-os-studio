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

/// One frame in a window `width` points wide, like `Sim::frame`.
fn frame_at(s: &mut Sim, width: f32, events: Vec<Event>) {
    s.t += 0.05;
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 1088.0));
    let mut raw = egui::RawInput { screen_rect: Some(screen), time: Some(s.t), modifiers: s.mods, events, ..Default::default() };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    eframe::App::raw_input_hook(shell, &ctx, &mut raw);
    let _ = ctx.run_ui(raw, |ui| shell.draw(ui));
}

fn click_at(s: &mut Sim, width: f32, p: Pos2) {
    let press = |pressed| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    frame_at(s, width, vec![Event::PointerMoved(p)]);
    frame_at(s, width, vec![press(true)]);
    frame_at(s, width, vec![press(false)]);
}

#[test]
fn a_tool_picked_from_the_more_menu_becomes_the_active_tool() {
    const WIDTH: f32 = 640.0;
    let mut s = Sim::new();
    reduced(&mut s);
    for _ in 0..3 {
        frame_at(&mut s, WIDTH, vec![]);
    }
    s.shell.app.wb_group.toggle(None).unwrap();
    for _ in 0..3 {
        frame_at(&mut s, WIDTH, vec![]);
    }
    let overflow = s.app().wb_group.overflow();
    assert!(!overflow.is_empty(), "a {WIDTH} px window folds the row into the ... menu");
    let more = s.rect_of("toolbar-whiteboard-more");
    click_at(&mut s, WIDTH, more.center());
    assert_eq!(s.app().ui.menu, Some(MenuKind::WhiteboardMore));
    let h = theme::MENU_PADDING * 2.0 + overflow.len() as f32 * theme::MENU_ITEM_HEIGHT;
    let pos = crate::panels::toolbar::tool_menu_pos(more, h);
    // the second row of the menu
    let want = overflow[1];
    click_at(&mut s, WIDTH, Pos2::new(pos.x + 100.0, pos.y + theme::MENU_PADDING + theme::MENU_ITEM_HEIGHT * 1.5));
    assert_eq!(s.app().tool, want, "the picked row starts its tool");
    assert_eq!(s.app().ui.menu, None, "the menu closes after the pick");
}
