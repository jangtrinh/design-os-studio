//! The Color profile control in the Design panel when nothing is selected.

use super::*;
use designos_doc::ColorProfile;

/// Clicks item `i` of the dropdown menu that opened under `name`.
fn pick(s: &mut Sim, name: &str, i: usize) {
    let ctl = s.rect_of(name);
    s.click(ctl.center());
    let menu = s.rect_of("menu-level-0");
    let y = menu.top() + theme::MENU_PADDING + i as f32 * theme::MENU_ITEM_HEIGHT + theme::MENU_ITEM_HEIGHT / 2.0;
    s.click(Pos2::new(ctl.left() + 20.0, y));
}

fn page_panel_sim() -> Sim {
    let mut s = Sim::new();
    s.shell.app.selection.clear();
    s.frame(vec![]);
    s
}

#[test]
fn the_panel_shows_the_document_profile_and_changing_it_is_one_undo_step() {
    let mut s = page_panel_sim();
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb);
    pick(&mut s, "color-profile", 1);
    assert_eq!(s.app().doc.color_profile, ColorProfile::DisplayP3);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb);
}

#[test]
fn picking_the_current_profile_changes_nothing() {
    let mut s = page_panel_sim();
    let before = s.app().hist.undo_depth();
    pick(&mut s, "color-profile", 0);
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb);
    assert_eq!(s.app().hist.undo_depth(), before, "no undo step for an unchanged choice");
}
