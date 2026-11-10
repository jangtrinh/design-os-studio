//! The Color profile control in the Design panel when nothing is selected.

use super::*;
use designos_doc::ColorProfile;

fn page_swatch_has(s: &Sim, expected: [u8; 3]) -> bool {
    let swatch = s.rect_of("page-swatch");
    s.shapes.iter().any(|cs| match &cs.shape {
        egui::Shape::Rect(r) if r.rect == swatch => {
            (r.fill.r() as i16 - expected[0] as i16).abs() <= 1
                && (r.fill.g() as i16 - expected[1] as i16).abs() <= 1
                && (r.fill.b() as i16 - expected[2] as i16).abs() <= 1
        }
        _ => false,
    })
}

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

#[test]
fn undo_and_redo_draw_the_restored_profile_swatch_in_the_same_frame() {
    let mut s = page_panel_sim();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: designos_doc::Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 } };
    s.frame(vec![]);
    s.shell.app.set_color_profile(ColorProfile::DisplayP3);
    s.frame(vec![]);

    s.mods = Modifiers::COMMAND;
    s.one_frame().key(Key::Z).send();
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb);
    assert!(page_swatch_has(&s, [128, 102, 204]), "undo frame swatch uses sRGB");

    s.mods = Modifiers { shift: true, ..Modifiers::COMMAND };
    s.one_frame().key(Key::Z).send();
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().doc.color_profile, ColorProfile::DisplayP3);
    assert!(page_swatch_has(&s, [132, 101, 211]), "redo frame swatch uses Display P3");
}

#[test]
fn dropdown_profile_change_draws_the_new_profile_swatch_in_the_same_frame() {
    let mut s = page_panel_sim();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: designos_doc::Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 } };
    s.frame(vec![]);
    let ctl = s.rect_of("color-profile");
    s.click(ctl.center());
    let menu = s.rect_of("menu-level-0");
    let y = menu.top() + theme::MENU_PADDING + theme::MENU_ITEM_HEIGHT + theme::MENU_ITEM_HEIGHT / 2.0;
    s.one_frame().move_to(Pos2::new(ctl.left() + 20.0, y)).press().release().send();
    assert_eq!(s.app().doc.color_profile, ColorProfile::DisplayP3);
    assert!(page_swatch_has(&s, [132, 101, 211]), "dropdown frame swatch uses Display P3");
}
