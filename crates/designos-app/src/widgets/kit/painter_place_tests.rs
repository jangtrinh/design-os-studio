//! Column placement and the look a held press keeps.
use super::*;
use crate::theme;
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

fn input(at: Pos2, press: Option<bool>) -> RawInput {
    let mut raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() };
    raw.events.push(Event::PointerMoved(at));
    if let Some(pressed) = press {
        raw.events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::default() });
    }
    raw
}

fn brand_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Rect(shape) if shape.fill == theme::BG_BRAND => Some(shape.rect),
            _ => None,
        })
        .collect()
}

fn icon_boxes(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::RectShape> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Rect(shape) if (shape.rect.width() - theme::LAYER_ICON_SIZE).abs() < 0.5 => Some(shape.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn checkbox_box_sits_on_the_column_edge() {
    for width in [112.0_f32, 144.0] {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(egui::pos2(100.0, 80.0), Vec2::new(width, theme::TARGET_MIN));
        let out = ctx.run_ui(input(rect.center(), None), |ui| {
            checkbox(ui, rect, "column", false, true);
        });
        let boxes = icon_boxes(&out.shapes);
        assert_eq!(boxes.len(), 1, "width {width}");
        assert_eq!(boxes[0].rect.left(), rect.left(), "width {width}");
        assert!((boxes[0].rect.center().y - rect.center().y).abs() < 0.5, "width {width}");
        assert_eq!(boxes[0].rect.height(), theme::LAYER_ICON_SIZE, "width {width}");
        assert_eq!(boxes[0].corner_radius, theme::cr(theme::SPACE_FACT as u8), "width {width}");
    }
}

#[test]
fn checked_box_stays_filled_while_the_button_is_held() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(100.0, 80.0), Vec2::new(112.0, theme::TARGET_MIN));
    let mut held = Vec::new();
    for press in [None, Some(true), None, Some(false)] {
        let out = ctx.run_ui(input(rect.center(), press), |ui| {
            checkbox(ui, rect, "held", true, true);
        });
        held.push(!brand_rects(&out.shapes).is_empty());
    }
    assert_eq!(held, vec![true, true, true, true], "a checked box stays filled before, during and after the press");
}

#[test]
fn toggle_stays_on_while_the_button_is_held() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(theme::CONTROL_SM, theme::TARGET_MIN - theme::SPACE_FACT));
    let mut held = Vec::new();
    for press in [None, Some(true), None, Some(false)] {
        let out = ctx.run_ui(input(rect.center(), press), |ui| {
            toggle(ui, rect, "sw", true, true);
        });
        held.push(brand_rects(&out.shapes).iter().any(|r| (r.width() - rect.width()).abs() < 0.5));
    }
    assert_eq!(held, vec![true, true, true, true], "an on switch keeps its fill while held");
}

#[test]
fn a_short_switch_still_takes_a_click_inside_the_target_floor() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(theme::CONTROL_SM, theme::TARGET_MIN - theme::SPACE_FACT));
    let at = egui::pos2(rect.center().x, rect.top() - 1.5);
    let mut clicked = false;
    let mut hit = Rect::NOTHING;
    let mut track = Vec::new();
    for press in [None, Some(true), Some(false)] {
        let out = ctx.run_ui(input(at, press), |ui| {
            let response = toggle(ui, rect, "sw", true, true);
            clicked |= response.clicked();
            hit = response.rect;
        });
        track = brand_rects(&out.shapes);
    }
    assert!(hit.height() >= theme::TARGET_MIN, "the switch hits on {} px", hit.height());
    assert!(clicked, "a click 1.5 px above the track lands inside the target floor");
    assert!(track.iter().any(|r| (r.height() - rect.height()).abs() < 0.5), "the painted track keeps its own height: {track:?}");
}
