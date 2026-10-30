use crate::theme;

#[test]
fn theme_apply_sets_stock_visuals_from_the_contract() {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let visuals = ctx.global_style().visuals.clone();
    assert_eq!(visuals.window_fill, theme::BG_PRIMARY_SOLID);
    assert_eq!(visuals.window_corner_radius, theme::cr(theme::RADIUS_CONTROL));
    assert_eq!(visuals.window_stroke, egui::Stroke::NONE);
    let pad = egui::Margin::symmetric(theme::INSET_BUTTON as i8, theme::INSET_FRAME as i8);
    let spacing = ctx.global_style().spacing.clone();
    assert_eq!(spacing.window_margin, pad);
    assert_eq!(spacing.menu_margin, pad);
    assert_eq!(visuals.menu_corner_radius, theme::cr(theme::RADIUS_CONTROL));
    assert_eq!(visuals.selection.bg_fill, theme::BG_TEXT_SELECTION);
    assert_eq!(visuals.widgets.active.bg_fill, theme::BG_PRESSED);
    assert_eq!(visuals.widgets.inactive.bg_fill, egui::Color32::TRANSPARENT);
    assert_eq!(visuals.widgets.hovered.bg_fill, theme::BG_HOVER);
}

#[test]
fn scroll_handle_opacity_lands_near_the_resting_grey() {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let scroll = ctx.global_style().spacing.scroll;
    assert!(scroll.foreground_color);
    assert!(scroll.floating);
    assert_eq!(scroll.bar_width, 6.0);
    assert_eq!(scroll.dormant_handle_opacity, 0.0);
    assert_eq!(scroll.active_handle_opacity, 0.225);
    assert_eq!(scroll.interact_handle_opacity, 0.225);
}
