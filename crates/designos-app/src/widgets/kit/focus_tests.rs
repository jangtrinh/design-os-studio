use super::*;
use crate::theme;
use egui::Color32;

#[test]
fn focused_selected_segment_keeps_its_ring() {
    let selected = segmented_look(State::Selected);
    assert_ne!(selected.fill, Color32::TRANSPARENT);
    assert_eq!(segmented_ring(true), theme::FOCUS_RING);
    assert_eq!(segmented_ring(false), Color32::TRANSPARENT);
}

#[test]
fn focused_hovered_button_keeps_its_ring() {
    let hover = button_look(Hierarchy::Primary, State::Hover);
    assert_eq!(hover.fill, theme::BG_BRAND_HOVER);
    assert_eq!(button_ring(Hierarchy::Primary, true), theme::FOCUS_RING);
    let pressed = button_look(Hierarchy::Primary, State::Pressed);
    assert_eq!(pressed.fill, theme::BG_BRAND_PRESSED);
    assert_eq!(button_ring(Hierarchy::Primary, true), theme::FOCUS_RING);
    assert_eq!(button_ring(Hierarchy::Destructive, true), theme::FOCUS_RING_ERROR);
    assert_eq!(button_ring(Hierarchy::SecondaryDestructive, true), theme::FOCUS_RING_ERROR);
    assert_eq!(button_ring(Hierarchy::Primary, false), Color32::TRANSPARENT);
}

#[test]
fn selected_hover_keeps_the_fill_and_takes_the_hot_label() {
    let sel = segmented_look(State::Selected);
    let hot = segmented_look(State::Hover);
    let mixed = selected_hover(sel, hot);
    assert_eq!(mixed.fill, sel.fill);
    assert_eq!(mixed.border, sel.border);
    assert_eq!(mixed.text, hot.text);
    assert_eq!(mixed.icon, hot.icon);
}

#[test]
fn swatch_keyboard_ring_is_the_focus_ring() {
    assert_eq!(swatch_ring(true), theme::FOCUS_RING);
    assert_eq!(swatch_ring(false), Color32::TRANSPARENT);
}

#[test]
fn keyboard_focus_keeps_the_selected_state() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("kit-focus");
    let mut saw = (false, State::Default);
    for _ in 0..3 {
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(60.0, 24.0));
            let response = ui.interact(rect, id, egui::Sense::click());
            response.request_focus();
            saw = (response.has_focus(), state_of(&response, true, true));
        });
    }
    assert!(saw.0);
    assert_eq!(saw.1, State::Selected);
    assert_eq!(segmented_ring(saw.0), theme::FOCUS_RING);
}
