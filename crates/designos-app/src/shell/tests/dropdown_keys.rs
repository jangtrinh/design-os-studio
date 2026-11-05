//! Tab walks the real `field_tab` registry onto panel dropdowns, and Escape on an open dropdown
//! keeps the layer (and the focus ring) instead of clearing the selection.

use super::*;
use crate::theme;
use egui::epaint::ClippedShape;
use egui::StrokeKind;

fn click_field(s: &mut Sim, key: &str) {
    let r = s.field_rect(key);
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
}

fn tab(s: &mut Sim, shift: bool) {
    s.mods = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
    s.key(Key::Tab);
    s.mods = Modifiers::NONE;
}

fn test_dropdowns(s: &Sim) -> Vec<(egui::Id, Rect)> {
    s.ctx.data(|d| d.get_temp::<Vec<(egui::Id, Rect)>>(egui::Id::new("test-dd-list")).unwrap_or_default())
}

fn dropdown_is_open(s: &Sim, id: egui::Id) -> bool {
    s.ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false)
}

fn focus_ring_on(shapes: &[ClippedShape], target: Rect) -> bool {
    fn walk(shape: &egui::epaint::Shape, target: Rect) -> bool {
        match shape {
            egui::epaint::Shape::Rect(r) => {
                let same = (r.rect.min.x - target.min.x).abs() <= 0.5
                    && (r.rect.min.y - target.min.y).abs() <= 0.5
                    && (r.rect.max.x - target.max.x).abs() <= 0.5
                    && (r.rect.max.y - target.max.y).abs() <= 0.5;
                same && r.stroke.color == theme::BORDER_SELECTED && (r.stroke.width - theme::BORDER_WIDTH).abs() <= 0.01 && r.stroke_kind == StrokeKind::Inside
            }
            egui::epaint::Shape::Vec(children) => children.iter().any(|c| walk(c, target)),
            _ => false,
        }
    }
    shapes.iter().any(|cs| walk(&cs.shape, target))
}

fn welcome_sim() -> (Sim, NodeId) {
    let s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let id = s.by_name("Welcome");
    (s, id)
}

fn enable_auto_layout(s: &mut Sim) {
    let r = s.rect_of("title-action:Auto layout");
    s.click(r.center());
    s.frame(vec![]);
}

fn tab_until_dropdown(s: &mut Sim) -> (egui::Id, Rect) {
    click_field(s, "x");
    for _ in 0..40 {
        tab(s, false);
        let focused = s.ctx.memory(|m| m.focused());
        if let Some(id) = focused {
            if let Some((_, rect)) = test_dropdowns(s).into_iter().find(|(did, _)| *did == id) {
                return (id, rect);
            }
        }
    }
    panic!("Tab never reached a dropdown; focused={:?} dropdowns={:?}", s.ctx.memory(|m| m.focused()), test_dropdowns(s));
}

fn focus_a_dropdown(s: &mut Sim) -> (egui::Id, Rect) {
    enable_auto_layout(s);
    tab_until_dropdown(s)
}

#[test]
fn tab_walk_reaches_a_dropdown_and_paints_the_ring() {
    let (mut s, _) = welcome_sim();
    let (id, rect) = focus_a_dropdown(&mut s);
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(id));
    assert!(focus_ring_on(&s.shapes, rect), "the focused dropdown must paint the 1px BORDER_SELECTED inside stroke");

    tab(&mut s, false);
    assert_ne!(s.ctx.memory(|m| m.focused()), Some(id), "Tab must leave the dropdown");
    assert!(!focus_ring_on(&s.shapes, rect), "Tabbing off the dropdown must remove its ring");

    tab(&mut s, true);
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(id), "Shift+Tab must return to the dropdown");
    assert!(focus_ring_on(&s.shapes, rect), "Shift+Tab back onto the dropdown must restore the ring");
}

#[test]
fn escape_on_an_open_dropdown_keeps_the_layer_and_the_ring() {
    let (mut s, welcome) = welcome_sim();
    let (id, rect) = focus_a_dropdown(&mut s);
    s.key(Key::Enter);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(dropdown_is_open(&s, id), "Enter on a focused dropdown must open the menu");

    s.key(Key::Escape);
    assert_eq!(s.app().selection, vec![welcome], "Escape on an open dropdown must not clear the selection");
    assert!(!dropdown_is_open(&s, id), "Escape must close the menu");
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(id), "Escape must leave keyboard focus on the dropdown");

    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().selection, vec![welcome], "the Design panel must still show the Welcome layer");
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(id));
    assert!(focus_ring_on(&s.shapes, rect), "after Escape the focused-closed ring must still be painted on the dropdown");
}

#[test]
fn escape_with_no_dropdown_open_still_clears_the_selection() {
    let (mut s, welcome) = welcome_sim();
    assert_eq!(s.app().selection, vec![welcome]);
    assert!(test_dropdowns(&s).iter().all(|(id, _)| !dropdown_is_open(&s, *id)), "no dropdown menu is open");
    s.key(Key::Escape);
    assert!(s.app().selection.is_empty(), "Escape with no dropdown open must still clear the selection");
}

#[test]
fn an_open_dropdown_that_stopped_drawing_does_not_swallow_escape() {
    let (mut s, _) = welcome_sim();
    let (id, _) = focus_a_dropdown(&mut s);
    s.key(Key::Enter);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(dropdown_is_open(&s, id));

    s.shell.app.clear_selection();
    if let Some(fid) = s.ctx.memory(|m| m.focused()) {
        s.ctx.memory_mut(|m| m.surrender_focus(fid));
    }
    s.frame(vec![]);
    s.frame(vec![]);
    s.frame(vec![]);

    s.key(Key::F);
    assert_eq!(s.app().tool, Tool::Frame, "after the Design panel is gone, F must still pick the Frame tool");
    s.key(Key::Escape);
    assert_eq!(s.app().tool, Tool::Move, "a dropdown that stopped drawing must not swallow a later Escape");
}
