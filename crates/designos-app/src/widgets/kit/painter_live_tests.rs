//! Live painters: a shape change here has to fail a test.
use super::*;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::menu::{dropdown, popup_menu, MenuItem};
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Sense, Vec2};

fn screen() -> RawInput {
    RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), ..Default::default() }
}

fn strokes(shapes: &[egui::epaint::ClippedShape], color: egui::Color32) -> Vec<egui::Stroke> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Rect(shape) if shape.stroke.color == color && shape.stroke.width > 0.0 => Some(shape.stroke),
            _ => None,
        })
        .collect()
}

#[test]
fn button_frame_uses_the_control_radius() {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let rect = Rect::from_min_size(egui::pos2(12.0, 12.0), egui::vec2(96.0, theme::CONTROL_XS));
    let out = ctx.run_ui(screen(), |ui| {
        button(ui, rect, "send", "Send", Hierarchy::Primary, Size::Xs, true);
    });
    let frame = out.shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.fill == theme::BG_BRAND && shape.rect == rect => Some(shape.corner_radius),
        _ => None,
    });
    assert_eq!(frame, Some(theme::cr(theme::RADIUS_CONTROL)));
}

#[test]
fn icon_button_glyph_uses_the_icon_colour() {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let rect = Rect::from_min_size(egui::pos2(12.0, 12.0), Vec2::splat(theme::CONTROL_XS));
    let out = ctx.run_ui(screen(), |ui| {
        button_icon(ui, rect, "ico", Icon::Check, Hierarchy::Icon, Size::Xs, true, false);
    });
    let tinted = out.shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Mesh(mesh) => mesh.vertices.iter().any(|v| v.color == theme::ICON_QUATERNARY),
        _ => false,
    });
    assert!(tinted, "the glyph is tinted with the icon colour");
}

#[test]
fn icon_button_background_draws_the_focus_ring() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(16.0, 16.0), Vec2::splat(theme::CONTROL_XS));
    let mut saw = false;
    for _ in 0..3 {
        let out = ctx.run_ui(screen(), |ui| {
            let response = ui.interact(rect, ui.id().with("icon-bg"), Sense::click());
            response.request_focus();
            paint_icon_bg(ui.painter(), rect, &response, false, theme::cr(theme::RADIUS_CONTROL));
        });
        saw |= strokes(&out.shapes, theme::FOCUS_RING).iter().any(|stroke| stroke.width == 2.0);
    }
    assert!(saw, "keyboard focus paints the 2 px ring");
}

#[test]
fn checked_box_is_filled_at_rest() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(24.0, 24.0), egui::vec2(112.0, theme::TARGET_MIN));
    let out = ctx.run_ui(screen(), |ui| {
        checkbox(ui, rect, "on", true, true);
    });
    let filled = out.shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) => shape.fill == theme::BG_BRAND,
        _ => false,
    });
    assert!(filled, "a checked box is filled");
}

#[test]
fn field_focus_border_is_two_px() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(180.0, theme::CONTROL_SM));
    let id = egui::Id::new("field-focus");
    let out = ctx.run_ui(screen(), |ui| {
        ui.memory_mut(|mem| mem.request_focus(id));
        paint_field(ui, rect, id, Size::Sm, false);
    });
    let stroke = strokes(&out.shapes, theme::BORDER_SELECTED);
    assert!(!stroke.is_empty(), "focus uses the selected border");
    assert!(stroke.iter().any(|s| s.width == 2.0), "the focused border is 2 px, got {stroke:?}");
}

#[test]
fn menu_surface_uses_the_frame_radius() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::pos2(8.0, 8.0), egui::vec2(200.0, 120.0));
    let out = ctx.run_ui(screen(), |ui| menu_surface(ui.painter(), rect));
    let radius = out.shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.rect == rect => Some(shape.corner_radius),
        _ => None,
    });
    assert_eq!(radius, Some(theme::cr(MENU_RADIUS)));
}

#[test]
fn menu_row_hover_paints_the_hover_fill() {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let pos = egui::pos2(24.0, 24.0);
    let min_w = 180.0;
    let items = [MenuItem::new("Alpha")];
    let menu_id = egui::Id::new("row-hover");
    let _ = ctx.run_ui(screen(), |ui| {
        let _ = popup_menu(ui.ctx(), menu_id, pos, min_w, &items, None);
    });
    let row = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "menu-row-0-0".to_string())))).expect("menu row");
    let at = row.center();
    // The menu area fades in. A frame past that fade paints the row fill opaque.
    let frames = [(0.05, vec![Event::PointerMoved(at + egui::vec2(-12.0, 0.0))]), (0.10, vec![Event::PointerMoved(at)]), (0.35, vec![])];
    let mut shapes = Vec::new();
    for (time, events) in frames {
        let mut raw = screen();
        raw.time = Some(time);
        raw.events.extend(events);
        let out = ctx.run_ui(raw, |ui| {
            let _ = popup_menu(ui.ctx(), menu_id, pos, min_w, &items, None);
        });
        shapes = out.shapes;
    }
    let hover = shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) => shape.fill == theme::BG_HOVER && shape.corner_radius == theme::cr(theme::RADIUS_CONTROL),
        _ => false,
    });
    assert!(hover, "the row under the pointer paints the hover fill");
}

#[test]
fn open_dropdown_uses_the_focused_border() {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let rect = Rect::from_min_size(egui::pos2(30.0, 30.0), egui::vec2(160.0, theme::CONTROL_SM));
    let at = rect.center();
    let mut shapes = Vec::new();
    let frames = [
        vec![Event::PointerMoved(at)],
        vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::default() }],
        vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::default() }],
        vec![Event::PointerMoved(at)],
    ];
    for events in frames {
        let mut raw = screen();
        raw.events.extend(events);
        let out = ctx.run_ui(raw, |ui| {
            let _ = dropdown(ui, rect, "pick", "One", &["One".to_string(), "Two".to_string()]);
        });
        shapes = out.shapes;
    }
    let open = shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) => shape.rect == rect && shape.stroke.color == theme::BORDER_SELECTED && shape.stroke.width == 2.0,
        _ => false,
    });
    assert!(open, "an open dropdown paints the 2 px focused border");
}
