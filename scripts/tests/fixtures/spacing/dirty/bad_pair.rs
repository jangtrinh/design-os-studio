// Outer radius 14, inset 8 would need inner 6; the inner uses 3.
fn paint(p: &Painter, rect: Rect) {
    p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), BG);
    let inner = rect.shrink(8.0);
    p.rect_filled(inner, CornerRadius::same(3), BG);
}
