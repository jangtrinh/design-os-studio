//! Menu row icons at rest.
use crate::theme;
use crate::widgets::menu::{popup_menu, MenuItem};
use egui::{Pos2, RawInput, Rect, Vec2};

fn screen(time: f64) -> RawInput {
    let mut raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), ..Default::default() };
    raw.time = Some(time);
    raw
}

#[test]
fn submenu_chevron_uses_the_item_icon_at_rest() {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let mut parent = MenuItem::new("Parent");
    parent.children.push(MenuItem::new("Child"));
    let items = [parent];
    let mut shapes = Vec::new();
    for time in [0.0, 0.5] {
        let out = ctx.run_ui(screen(time), |ui| {
            let _ = popup_menu(ui.ctx(), egui::Id::new("chevron-rest"), egui::pos2(24.0, 24.0), 180.0, &items, None);
        });
        shapes = out.shapes;
    }
    let colors: Vec<_> = shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Mesh(mesh) => mesh.vertices.first().map(|vertex| vertex.color),
            _ => None,
        })
        .collect();
    assert!(colors.contains(&theme::ICON_QUATERNARY), "chevron {colors:?}");
    assert!(!colors.contains(&theme::TEXT_QUATERNARY), "chevron {colors:?}");
}
