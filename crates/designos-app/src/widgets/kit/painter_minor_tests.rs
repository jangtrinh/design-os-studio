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

fn paint_disabled(draw: impl Fn(&mut egui::Ui)) -> (Vec<egui::epaint::ClippedShape>, f32) {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let out = ctx.run_ui(screen(), |ui| {
        ui.add_enabled_ui(false, |ui| draw(ui));
    });
    (out.shapes, ctx.global_style().visuals.disabled_alpha())
}

fn mesh_color(shapes: &[egui::epaint::ClippedShape]) -> Option<egui::Color32> {
    shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Mesh(mesh) => mesh.vertices.first().map(|v| v.color),
        _ => None,
    })
}

#[test]
fn labelled_painters_dim_once_inside_a_disabled_ui() {
    let rect = Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(160.0, theme::TARGET_MIN));
    let track = Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(theme::CONTROL_SM, theme::TARGET_MIN - theme::SPACE_FACT));
    let once = |color: egui::Color32, alpha: f32| color.gamma_multiply(alpha);
    let twice = |color: egui::Color32, alpha: f32| half_color(color).gamma_multiply(alpha);
    let mut bad = Vec::new();

    let (shapes, alpha) = paint_disabled(|ui| {
        button(ui, rect, "go", "Go", Hierarchy::Primary, Size::Sm, false);
    });
    let fill = shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.rect == rect && shape.fill.a() > 0 => Some(shape.fill),
        _ => None,
    });
    if fill != Some(once(theme::BG_BRAND, alpha)) {
        bad.push(format!("button {fill:?}"));
    }

    let (shapes, alpha) = paint_disabled(|ui| {
        checkbox(ui, rect, "box", true, false);
    });
    let mark = mesh_color(&shapes);
    if mark != Some(once(theme::TEXT_ON_BRAND, alpha)) {
        bad.push(format!("checkbox {mark:?}"));
    }

    let (shapes, alpha) = paint_disabled(|ui| {
        toggle(ui, track, "switch", true, false);
    });
    let knob = circles_of(&shapes);
    if knob != Some(once(theme::ICON_WHITE, alpha)) {
        bad.push(format!("toggle {knob:?}"));
    }

    let (shapes, alpha) = paint_disabled(|ui| {
        radio_card(ui, rect, "mode", "Ask", false, false);
    });
    let stroke = shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.rect == rect && shape.stroke.width > 0.0 => Some(shape.stroke.color),
        _ => None,
    });
    if stroke != Some(once(theme::BORDER, alpha)) {
        bad.push(format!("radio {stroke:?}"));
    }

    let (shapes, alpha) = paint_disabled(|ui| {
        list_row(ui, rect, "page", "Page", true, false, Surface::Primary);
    });
    let row = shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.rect == rect && shape.fill.a() > 0 => Some(shape.fill),
        _ => None,
    });
    if row != Some(once(theme::BG_SELECTED, alpha)) {
        bad.push(format!("row {row:?}"));
    }

    assert_ne!(once(theme::BG_BRAND, alpha), twice(theme::BG_BRAND, alpha));
    assert!(bad.is_empty(), "{bad:?}");
}

fn circles_of(shapes: &[egui::epaint::ClippedShape]) -> Option<egui::Color32> {
    shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Circle(shape) => Some(shape.fill),
        _ => None,
    })
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
fn font_text_keeps_its_step_note() {
    let src = include_str!("../../theme.rs");
    let note = src.find("Inter at `size` in one of the three UI weights").expect("step note");
    let allow = src.find("staged: first used by the button labels").expect("allow");
    let font = src.find("pub fn font_text(").expect("font_text");
    let family = src.find("fn weight_family(").expect("weight family");
    assert!(note < font && allow < font && font < family, "the step note sits on font_text");
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
