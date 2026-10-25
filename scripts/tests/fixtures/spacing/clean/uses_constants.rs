// Every dimension comes from a theme constant: nothing for the ratchet to count.
fn paint(p: &Painter, rect: Rect) {
    p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), BG);
    let font = FontId::new(theme::FONT_SIZE_BODY, FontFamily::Proportional);
    let inner = rect.shrink(theme::INSET_FRAME);
    p.rect_filled(inner, theme::cr(theme::RADIUS_CONTROL), BG);
}
