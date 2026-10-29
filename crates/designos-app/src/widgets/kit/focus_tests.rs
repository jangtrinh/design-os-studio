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
    assert_eq!(swatch_ring_color(true), theme::FOCUS_RING);
    assert_eq!(swatch_ring_color(false), Color32::TRANSPARENT);
}

fn ring_strokes(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::RectShape> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Rect(shape) if shape.stroke.color == theme::FOCUS_RING && shape.stroke.width == 2.0 => Some(shape.clone()),
            _ => None,
        })
        .collect()
}

fn snap_px(value: f32, pixels_per_point: f32) -> f32 {
    (value * pixels_per_point).round() / pixels_per_point
}

#[test]
fn focus_ring_is_two_px_outside_with_two_px_gap() {
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(10.5, 20.25), egui::vec2(40.5, 32.0));
    let pixels_per_point = 2.0;
    let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0))), ..Default::default() };
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(pixels_per_point);
    let out = ctx.run_ui(input, |ui| {
        focus::focus_ring(ui.painter(), rect, 8.0);
    });
    let rings = ring_strokes(&out.shapes);
    assert_eq!(rings.len(), 1, "one 2 px focus band");
    let ring = &rings[0];
    let outer = rect.expand(4.0);
    let snapped = egui::Rect::from_min_max(
        egui::pos2(snap_px(outer.min.x, pixels_per_point), snap_px(outer.min.y, pixels_per_point)),
        egui::pos2(snap_px(outer.max.x, pixels_per_point), snap_px(outer.max.y, pixels_per_point)),
    );
    assert_eq!(ring.rect, snapped);
    assert_eq!(ring.corner_radius, egui::CornerRadius::same(12));
    assert_eq!(ring.stroke_kind, egui::StrokeKind::Inside);
    assert_eq!(ring.stroke.color, theme::FOCUS_RING);
    assert!(ring.rect.min.x <= rect.min.x - 4.0 + 0.5);
    assert!(rect.min.x - ring.rect.min.x >= 3.5);
}

#[test]
fn ring_draws_on_a_focused_button() {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(80.0, 32.0));
    let mut saw = false;
    for _ in 0..3 {
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let response = button::button(ui, rect, "send", "Send", Hierarchy::Primary, Size::Xs, true);
            response.request_focus();
            assert!(response.has_focus() || saw);
            assert!(focus::keyboard_focus(ui, response.id));
        });
        saw = !ring_strokes(&out.shapes).is_empty();
    }
    assert!(saw, "a focused button paints the focus band");
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

fn focused_shapes(draw: impl Fn(&mut egui::Ui)) -> Vec<egui::epaint::ClippedShape> {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let mut shapes = Vec::new();
    for _ in 0..3 {
        shapes = ctx.run_ui(egui::RawInput::default(), |ui| draw(ui)).shapes;
    }
    shapes
}

#[test]
fn list_row_focus_ring_uses_the_row_radius() {
    let rect = egui::Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(200.0, theme::TARGET_MIN));
    let shapes = focused_shapes(|ui| {
        list_row(ui, rect, "row", "Page", false, true, Surface::Primary).request_focus();
    });
    let want = (theme::RADIUS_CONTROL as f32 + theme::SPACE_FACT).round() as u8;
    let rings = ring_strokes(&shapes);
    assert_eq!(rings.len(), 1, "the row paints one focus band");
    assert_eq!(rings[0].corner_radius, egui::CornerRadius::same(want));
}

#[test]
fn toggle_focus_ring_uses_the_pill_radius() {
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(theme::CONTROL_SM, theme::TARGET_MIN - theme::SPACE_FACT));
    let shapes = focused_shapes(|ui| {
        toggle(ui, rect, "switch", true, true).request_focus();
    });
    let want = (theme::radius_full(rect.height()).round() + theme::SPACE_FACT).round() as u8;
    let rings = ring_strokes(&shapes);
    assert_eq!(rings.len(), 1, "the toggle paints one focus band");
    assert_eq!(rings[0].corner_radius, egui::CornerRadius::same(want));
}
