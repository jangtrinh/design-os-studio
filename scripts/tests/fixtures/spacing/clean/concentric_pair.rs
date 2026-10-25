// Outer radius 14, inset 8, inner radius 6: concentric, so not counted as a pair.
fn paint(p: &Painter, rect: Rect) {
    p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), BG);
    let inner = rect.shrink(8.0);
    p.rect_filled(inner, theme::cr(theme::RADIUS_CONTROL), BG);
}
