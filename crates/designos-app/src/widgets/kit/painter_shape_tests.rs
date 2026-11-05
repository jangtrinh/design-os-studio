//! Choice, badge and feedback geometry.
use super::*;
use crate::theme;
use egui::{Pos2, RawInput, Rect, Vec2};

fn paint(draw: impl FnMut(&mut egui::Ui)) -> Vec<egui::epaint::ClippedShape> {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), ..Default::default() };
    ctx.run_ui(raw, draw).shapes
}

fn rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<&egui::epaint::RectShape> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Rect(shape) => Some(shape),
            _ => None,
        })
        .collect()
}

fn circles(shapes: &[egui::epaint::ClippedShape]) -> Vec<&egui::epaint::CircleShape> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Circle(shape) => Some(shape),
            _ => None,
        })
        .collect()
}

#[test]
fn radio_card_matches_its_row() {
    let rect = Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(240.0, 72.0));
    let shapes = paint(|ui| {
        radio_card(ui, rect, "mode", "Ask", true, true);
    });
    let card = rects(&shapes).into_iter().find(|shape| shape.rect == rect);
    assert_eq!(card.map(|shape| shape.corner_radius), Some(theme::cr(theme::RADIUS_XL)));
    let radio_r = theme::LAYER_ICON_SIZE * 0.5;
    let center = egui::pos2(rect.left() + theme::INSET_PANEL + radio_r, rect.center().y);
    let ring = circles(&shapes).into_iter().find(|shape| (shape.radius - radio_r).abs() < 0.5);
    assert!(ring.is_some(), "radio diameter is 16");
    assert!((ring.unwrap().center.x - center.x).abs() < 0.5);
    let dot_r = theme::INSET_SEGMENT + theme::BORDER_WIDTH;
    assert!(circles(&shapes).iter().any(|shape| (shape.radius - dot_r).abs() < 0.5 && shape.fill == theme::TEXT_ON_BRAND));
}

#[test]
fn slider_handle_uses_the_brand_disc() {
    let rect = Rect::from_center_size(egui::pos2(80.0, 40.0), Vec2::splat(theme::LAYER_ICON_SIZE));
    let shapes = paint(|ui| {
        slider_handle(ui, rect, "handle", true);
    });
    assert!(circles(&shapes).iter().any(|shape| shape.fill == theme::ICON_BRAND_PRIMARY));
}

#[test]
fn loading_ring_is_an_arc() {
    let shapes = paint(|ui| loading_ring(ui.painter(), egui::pos2(48.0, 48.0), theme::LAYER_ICON_SIZE * 0.5));
    let brand = theme::ICON_BRAND_PRIMARY;
    let full = circles(&shapes).iter().any(|shape| shape.stroke.color == brand);
    let arc = shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::LineSegment { stroke, .. } => stroke.color == brand,
        _ => false,
    });
    let track = circles(&shapes).iter().any(|shape| shape.stroke.color == theme::BG_TERTIARY);
    assert!(!full, "the indicator is not a full circle");
    assert!(arc, "the indicator is an open arc");
    assert!(track, "the track is the tertiary ring");
}

fn ring_frames() -> (Vec<egui::epaint::ClippedShape>, Vec<egui::epaint::ClippedShape>, std::time::Duration) {
    let ctx = egui::Context::default();
    let mut frames = Vec::new();
    let mut delay = std::time::Duration::MAX;
    for time in [0.0, 0.4] {
        let mut raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), ..Default::default() };
        raw.time = Some(time);
        let out = ctx.run_ui(raw, |ui| loading_ring(ui.painter(), egui::pos2(48.0, 48.0), theme::LAYER_ICON_SIZE * 0.5));
        delay = out.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        frames.push(out.shapes);
    }
    (frames.remove(0), frames.remove(0), delay)
}

fn segments(shapes: &[egui::epaint::ClippedShape]) -> Vec<[egui::Pos2; 2]> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::LineSegment { points, .. } => Some(*points),
            _ => None,
        })
        .collect()
}

#[test]
fn loading_ring_turns_on_a_two_px_track() {
    let (early, later, delay) = ring_frames();
    let width = circles(&early).into_iter().find(|shape| shape.stroke.color == theme::BG_TERTIARY).map(|shape| shape.stroke.width);
    let path = early.iter().any(|clipped| matches!(&clipped.shape, egui::epaint::Shape::Path(_)));
    let moved = segments(&early) != segments(&later);
    let wide = width.is_some_and(|width| (width - theme::BORDER_WIDTH * 2.0).abs() < 0.01);
    assert!(wide && delay == std::time::Duration::ZERO && !path && moved, "track {width:?}, repaint {delay:?}, path {path}, moved {moved}");
}

#[test]
fn count_badge_tag_and_progress_use_their_radii() {
    let badge = Rect::from_min_size(egui::pos2(8.0, 8.0), egui::vec2(28.0, theme::BADGE_SM));
    let chip = Rect::from_min_size(egui::pos2(8.0, 48.0), egui::vec2(88.0, theme::BADGE_MD));
    let bar = Rect::from_min_size(egui::pos2(8.0, 90.0), egui::vec2(120.0, theme::SPACE_INLINE));
    let shapes = paint(|ui| {
        count_badge(ui.painter(), badge, "3", Tone::Neutral);
        tag(ui, chip, "chip", "Label");
        progress_bar(ui.painter(), bar, 0.4);
    });
    let found = rects(&shapes);
    assert!(found.iter().any(|shape| shape.rect == badge && shape.corner_radius == theme::cr_pill(badge.height())));
    assert!(found.iter().any(|shape| shape.rect == chip && shape.corner_radius == theme::cr(theme::RADIUS_CONTROL)));
    assert!(found.iter().any(|shape| shape.rect == bar && shape.corner_radius == theme::cr_pill(bar.height())));
    let close = shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Mesh(mesh) => mesh.vertices.iter().any(|v| v.color == theme::ICON_QUATERNARY),
        _ => false,
    });
    assert!(close, "a tag paints its close glyph");
}

#[test]
fn tag_label_sits_clear_of_the_close_glyph() {
    let glyph = theme::FONT_SIZE_TEXT_SM;
    let _ = crate::lint::take_icons();
    let shapes = paint(|ui| {
        let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
        let label_w = ui.painter().layout_no_wrap("Design".to_string(), font, theme::TEXT).rect.width();
        let width = theme::INSET_FRAME + label_w + theme::SPACE_FACT + glyph + theme::INSET_SEGMENT + theme::SPACE_FACT;
        let rect = Rect::from_min_size(egui::pos2(12.0, 12.0), egui::vec2(width, theme::BADGE_MD));
        tag(ui, rect, "tag", "Design");
    });
    let close = crate::lint::take_icons().into_iter().find(|icon| icon.name == "Close").expect("close glyph");
    assert!((close.glyph.width() - glyph).abs() < 0.51, "close glyph side {}", close.glyph.width());
    let text = shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Text(text) if text.galley.job.text.contains("Design") => Some(text.galley.rect.translate(text.pos.to_vec2())),
        _ => None,
    });
    let text = text.expect("label");
    assert!(text.right() <= close.glyph.left() + 0.5, "label ends at {} and the glyph starts at {}", text.right(), close.glyph.left());
}

#[test]
fn toggle_track_is_a_pill_with_a_sixteen_px_knob() {
    let rect = Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(theme::CONTROL_SM, theme::TARGET_MIN - theme::SPACE_FACT));
    let shapes = paint(|ui| {
        toggle(ui, rect, "track", true, true);
    });
    let radius = theme::radius_full(rect.height()).round() as u8;
    assert!(rects(&shapes).iter().any(|shape| shape.rect == rect && shape.corner_radius == theme::cr(radius)));
    let knob = theme::radius_full(theme::LAYER_ICON_SIZE);
    let cx = rect.right() - theme::INSET_SEGMENT - knob;
    assert!(circles(&shapes).iter().any(|shape| (shape.center.x - cx).abs() < 0.5 && (shape.radius - knob).abs() < 0.5));
}
