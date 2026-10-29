use crate::theme;

#[test]
fn theme_apply_sets_stock_visuals_from_the_contract() {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let visuals = ctx.global_style().visuals.clone();
    assert_eq!(visuals.window_fill, theme::BG);
    assert_eq!(visuals.menu_corner_radius, theme::cr(theme::RADIUS_FRAME));
    assert_eq!(visuals.selection.bg_fill, theme::BG_TEXT_SELECTION);
    assert_eq!(visuals.widgets.active.bg_fill, theme::BG_PRESSED);
    assert_eq!(visuals.widgets.inactive.bg_fill, egui::Color32::TRANSPARENT);
    assert_eq!(visuals.widgets.hovered.bg_fill, theme::BG_HOVER);
}
