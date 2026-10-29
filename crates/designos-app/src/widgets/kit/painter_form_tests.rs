//! Tabs, rows and the remaining surfaces.
use super::*;
use crate::theme;
use egui::{Pos2, RawInput, Rect, Vec2};

fn paint(draw: impl FnMut(&mut egui::Ui)) -> Vec<egui::epaint::ClippedShape> {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), ..Default::default() };
    ctx.run_ui(raw, draw).shapes
}

fn font_of(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::FontId> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) => text.galley.job.sections.first().map(|section| section.format.font_id.clone()),
            _ => None,
        })
        .collect()
}

fn strong_xs(id: &egui::FontId) -> bool {
    id.size == theme::FONT_SIZE_TEXT_XS && id.family == egui::FontFamily::Name(theme::FAMILY_STRONG.into())
}

fn strong_sm(id: &egui::FontId) -> bool {
    id.size == theme::FONT_SIZE_TEXT_SM && id.family == egui::FontFamily::Name(theme::FAMILY_STRONG.into())
}

fn radius_of(shapes: &[egui::epaint::ClippedShape], rect: Rect) -> Option<egui::CornerRadius> {
    shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.rect == rect => Some(shape.corner_radius),
        _ => None,
    })
}

#[test]
fn tab_and_segmented_labels_match_their_rows() {
    let tab_rect = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(72.0, theme::CONTROL_DENSE));
    let strip = Rect::from_min_size(egui::pos2(10.0, 50.0), egui::vec2(160.0, theme::CONTROL_DENSE));
    let shapes = paint(|ui| {
        tab(ui, tab_rect, "tab", "Layers", true);
        segmented(ui, strip, "seg", &["A", "B"], 0, Size::Sm);
    });
    let fonts = font_of(&shapes);
    assert!(fonts.iter().any(strong_xs), "labels are text-xs semibold, got {fonts:?}");
    assert_eq!(radius_of(&shapes, strip), Some(theme::cr(theme::RADIUS_CONTROL)), "the segmented strip has a container");
    assert!(shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) => shape.corner_radius == theme::cr(theme::RADIUS_SEGMENT) && shape.fill == theme::BG_SELECTED,
        _ => false,
    }));
}

/// The strips painted by `segmented` and the rect each option's hit cell covers, read back from egui.
fn segmented_strips(short: Rect, tall: Rect) -> (Vec<egui::epaint::ClippedShape>, Vec<Rect>) {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), ..Default::default() };
    let mut ids = Vec::new();
    let shapes = ctx
        .run_ui(raw, |ui| {
            for (key, rect) in [("short", short), ("tall", tall)] {
                segmented(ui, rect, key, &["A", "B"], 0, Size::Xs);
                for i in 0..2 {
                    ids.push(ui.id().with(key).with(i));
                }
            }
        })
        .shapes;
    let cells = ids.iter().map(|id| ctx.read_response(*id).expect("every option is interactive").rect).collect();
    (shapes, cells)
}

fn tile_fills(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Rect, egui::CornerRadius)> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Rect(shape) if shape.fill == theme::BG_SELECTED => Some((shape.rect, shape.corner_radius)),
            _ => None,
        })
        .collect()
}

#[test]
fn segmented_short_strip_keeps_the_inset_tile_and_a_full_height_hit() {
    let short = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(160.0, theme::TARGET_MIN));
    let tall = Rect::from_min_size(egui::pos2(10.0, 50.0), egui::vec2(160.0, theme::CONTROL_MD));
    let (shapes, cells) = segmented_strips(short, tall);
    let pad = theme::INSET_SEGMENT;
    let tiles = tile_fills(&shapes);
    let short_tile =
        Rect::from_min_size(egui::pos2(short.left() + pad, short.top() + pad), egui::vec2((short.width() - pad * 2.0) / 2.0, short.height() - pad * 2.0));
    let tall_tile =
        Rect::from_min_size(egui::pos2(tall.left() + pad, tall.top() + pad), egui::vec2((tall.width() - pad * 2.0) / 2.0, tall.height() - pad * 2.0));
    assert!(tiles.contains(&(short_tile, theme::cr(theme::RADIUS_SEGMENT))), "a short strip still paints the inset tile, got {tiles:?}");
    assert!(tiles.contains(&(tall_tile, theme::cr(theme::RADIUS_SEGMENT))), "a tall strip paints the inset tile, got {tiles:?}");
    assert_eq!(theme::RADIUS_SEGMENT as f32, theme::RADIUS_CONTROL as f32 - pad, "the tile radius stays concentric with the strip");
    let half = short.width() / 2.0;
    assert_eq!(cells[0], Rect::from_min_size(short.min, egui::vec2(half, short.height())), "a short strip hits on the full height column");
    assert_eq!(cells[1], Rect::from_min_size(egui::pos2(short.left() + half, short.top()), egui::vec2(half, short.height())));
    assert!(cells[0].height() >= theme::TARGET_MIN);
    assert_eq!(cells[2], tall_tile, "a tall strip hits on the tile it paints");
}

#[test]
fn tree_and_list_rows_use_the_control_radius() {
    let tree = Rect::from_min_size(egui::pos2(8.0, 8.0), egui::vec2(200.0, theme::CONTROL_XS));
    let row = Rect::from_min_size(egui::pos2(8.0, 48.0), egui::vec2(200.0, theme::CONTROL_SM));
    let shapes = paint(|ui| {
        tree_row_bg(ui.painter(), tree, State::Hover, Surface::Primary);
        list_row(ui, row, "page", "Page", true, true, Surface::Primary);
    });
    assert_eq!(radius_of(&shapes, tree), Some(theme::cr(theme::RADIUS_CONTROL)));
    assert_eq!(radius_of(&shapes, row), Some(theme::cr(theme::RADIUS_CONTROL)));
    assert!(font_of(&shapes).iter().any(strong_sm), "a list row label is text-sm semibold");
}

#[test]
fn empty_state_and_headers_match_their_rows() {
    let bounds = Rect::from_min_size(egui::pos2(12.0, 12.0), egui::vec2(280.0, 160.0));
    let header = Rect::from_min_size(egui::pos2(12.0, 190.0), egui::vec2(240.0, theme::CONTROL_MD));
    let section = Rect::from_min_size(egui::pos2(12.0, 240.0), egui::vec2(240.0, theme::CONTROL_MD));
    let shapes = paint(|ui| {
        empty_state(ui.painter(), bounds, "Nothing yet");
        panel_header(ui.painter(), header, "Layers");
        section_header(ui.painter(), section, "Layout", true);
    });
    let tile = Vec2::splat(theme::CONTROL_XS);
    assert!(shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) => shape.rect.size() == tile && shape.corner_radius == theme::cr(theme::RADIUS_CONTROL),
        _ => false,
    }));
    let fonts = font_of(&shapes);
    let title_font = shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Text(text) if text.galley.job.text == "Nothing yet" => {
            text.galley.job.sections.first().map(|section| section.format.font_id.clone())
        }
        _ => None,
    });
    assert!(title_font.as_ref().is_some_and(strong_sm), "empty-state title is text-sm semibold, got {title_font:?}");
    assert!(fonts.iter().any(strong_sm));
    let title = shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Text(text) if text.galley.text() == "Layers" => Some(text.pos.x),
        _ => None,
    });
    assert_eq!(title, Some(header.left() + theme::INSET_PANEL));
}

#[test]
fn remaining_painters_match_their_rows() {
    let dot = egui::pos2(20.0, 20.0);
    let tile = Rect::from_min_size(egui::pos2(40.0, 8.0), Vec2::splat(theme::CONTROL_XS));
    let modal = Rect::from_min_size(egui::pos2(80.0, 80.0), egui::vec2(200.0, 120.0));
    let command = Rect::from_min_size(egui::pos2(80.0, 220.0), egui::vec2(220.0, theme::CONTROL_MD));
    let toast = Rect::from_min_size(egui::pos2(80.0, 280.0), egui::vec2(240.0, theme::CONTROL_MD));
    let shapes = paint(|ui| {
        let p = ui.painter();
        status_dot(p, dot, DotTone::Info);
        tile_icon(p, tile, TileKind::Design);
        modal_surface(p, Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0)), modal);
        command_bar_surface(p, command);
        toast_surface(p, toast);
    });
    assert!(shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Circle(shape) => (shape.radius - theme::SPACE_FACT).abs() < 0.5 && shape.fill == theme::ICON_BRAND_PRIMARY,
        _ => false,
    }));
    assert_eq!(radius_of(&shapes, tile), Some(theme::cr(theme::RADIUS_CONTROL)));
    assert_eq!(radius_of(&shapes, modal), Some(theme::cr(MENU_RADIUS)));
    assert_eq!(radius_of(&shapes, command), Some(theme::cr(theme::RADIUS_XL)));
    assert_eq!(radius_of(&shapes, toast), Some(theme::cr(theme::RADIUS_XL)));
    let frame = tooltip_frame();
    assert_eq!(frame.fill, theme::BG_PRIMARY_SOLID);
    assert_eq!(frame.corner_radius, theme::cr(theme::RADIUS_CONTROL));
    assert_eq!(frame.stroke, egui::Stroke::NONE);
    assert_eq!(frame.inner_margin, egui::Margin::symmetric(theme::INSET_BUTTON as i8, theme::INSET_FRAME as i8));
}
