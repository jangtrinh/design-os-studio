//! Clip, dimming and the font family cache.
use super::*;
use crate::theme;
use egui::{Pos2, RawInput, Rect, Vec2};

fn screen() -> RawInput {
    RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() }
}

#[test]
fn focus_ring_widens_the_parent_clip() {
    let ctx = egui::Context::default();
    let clip = Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(100.0, 80.0));
    let rect = Rect::from_min_size(egui::pos2(52.0, 52.0), egui::vec2(40.0, theme::TARGET_MIN));
    let out = ctx.run_ui(screen(), |ui| {
        let p = ui.painter().with_clip_rect(clip);
        focus::focus_ring(&p, rect, theme::RADIUS_CONTROL as f32);
    });
    let widened = out.shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.stroke.color == theme::FOCUS_RING => Some(clipped.clip_rect),
        _ => None,
    });
    assert_eq!(widened, Some(clip.expand(theme::SPACE_FACT)));
}

#[test]
fn disabled_field_is_dimmed_once() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(160.0, theme::CONTROL_SM));
    let out = ctx.run_ui(screen(), |ui| {
        ui.add_enabled_ui(false, |ui| {
            paint_field(ui, rect, egui::Id::new("dim-field"), Size::Sm, false);
        });
    });
    let alpha = ctx.global_style().visuals.disabled_alpha();
    let once = theme::BORDER_STRONG.gamma_multiply(alpha);
    let twice = half_color(theme::BORDER_STRONG).gamma_multiply(alpha);
    let stroke = out.shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.stroke.width > 0.0 => Some(shape.stroke.color),
        _ => None,
    });
    assert_eq!(stroke, Some(once));
    assert_ne!(once, twice);
}

#[test]
fn font_text_reuses_the_weight_family() {
    let small = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
    let large = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Medium);
    let arc = |id: &egui::FontId| match &id.family {
        egui::FontFamily::Name(name) => name.clone(),
        other => panic!("expected a named family, got {other:?}"),
    };
    assert!(std::sync::Arc::ptr_eq(&arc(&small), &arc(&large)));
    let strong = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let strong_again = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Semibold);
    assert!(std::sync::Arc::ptr_eq(&arc(&strong), &arc(&strong_again)));
}

#[test]
fn swatch_draws_the_focus_ring_only_when_its_control_has_focus() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(14.0, 14.0));
    let ring = |focused: bool| {
        let out = ctx.run_ui(screen(), |ui| crate::widgets::swatch_focused(ui.painter(), rect, theme::BG_BRAND, focused));
        out.shapes.iter().any(|clipped| matches!(&clipped.shape, egui::epaint::Shape::Rect(shape) if shape.stroke.color == theme::FOCUS_RING))
    };
    assert!(ring(true));
    assert!(!ring(false));
}
