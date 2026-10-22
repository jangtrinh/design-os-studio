//! Custom-painted widgets that replace stock egui looks.

/// Anything usable as an egui id salt.
pub trait Key: std::hash::Hash + std::fmt::Debug {}
impl<T: std::hash::Hash + std::fmt::Debug> Key for T {}

pub mod field;
pub mod menu;
pub mod picker;
pub mod popover;

use crate::icons::{self, Icon};
use crate::theme;
use egui::{Align2, Color32, CornerRadius, FontId, Id, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

/// True while the pointer is over a popup or menu (anything drawn as a foreground area).
pub fn pointer_over_popup(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.pointer.latest_pos()).is_some_and(|p| ctx.layer_id_at(p).is_some_and(|l| l.order == egui::Order::Foreground))
}

/// Width of a string in the given font.
pub fn text_width(p: &Painter, s: &str, font: &FontId) -> f32 {
    p.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x
}

/// Shortens `s` with an ellipsis so that it fits `max_w`.
pub fn elide(p: &Painter, s: &str, font: &FontId, max_w: f32) -> String {
    if text_width(p, s, font) <= max_w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cand: String = chars[..mid].iter().collect::<String>() + "…";
        if text_width(p, &cand, font) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>() + "…"
}

pub fn hline(p: &Painter, x0: f32, x1: f32, y: f32, color: Color32) {
    p.rect_filled(Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + theme::BORDER_WIDTH)), 0.0, color);
}

pub fn vline(p: &Painter, x: f32, y0: f32, y1: f32, color: Color32) {
    p.rect_filled(Rect::from_min_max(Pos2::new(x, y0), Pos2::new(x + theme::BORDER_WIDTH, y1)), 0.0, color);
}

/// Layered soft shadow (approximates a CSS box-shadow with blur).
pub fn shadow(p: &Painter, rect: Rect, radius: u8, offset: Vec2, blur: f32, color: Color32) {
    let steps = (blur / 1.5).ceil().max(1.0) as i32;
    for i in (0..steps).rev() {
        let t = i as f32 / steps as f32;
        let grow = blur * t;
        let a = (color.a() as f32 * (1.0 - t) * (1.0 - t) / steps as f32 * 2.0).clamp(0.0, 255.0) as u8;
        let c = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a);
        p.rect_filled(rect.translate(offset).expand(grow), CornerRadius::same(radius.saturating_add(grow as u8)), c);
    }
}

/// Elevation 200 style shadow stack used by menus/popovers.
pub fn elevation_shadow(p: &Painter, rect: Rect, radius: u8) {
    shadow(p, rect, radius, Vec2::new(0.0, 3.0), 8.0, theme::SHADOW_COLOR);
    shadow(p, rect, radius, Vec2::new(0.0, 1.0), 3.0, theme::SHADOW_COLOR_STRONG);
}

/// Elevation 400 (menus): two drop shadows plus a half-pixel inner highlight ring
/// and a fainter top edge highlight, drawn over the panel fill by `menu_panel`.
pub fn elevation_400(p: &Painter, rect: Rect, radius: u8) {
    shadow(p, rect, radius, Vec2::new(0.0, 2.0), 5.0, theme::ELEVATION_400_SHADOW);
    shadow(p, rect, radius, Vec2::new(0.0, 10.0), 16.0, theme::ELEVATION_400_SHADOW);
}

/// Panel chrome of a menu: shadow, fill, 0.5px inset ring.
pub fn menu_panel(p: &Painter, rect: Rect, radius: u8) {
    elevation_400(p, rect, radius);
    p.rect_filled(rect, theme::cr(radius), theme::BG_MENU);
    p.rect_stroke(rect, theme::cr(radius), Stroke::new(1.0, theme::ELEVATION_400_RING), StrokeKind::Inside);
    p.line_segment(
        [Pos2::new(rect.left() + radius as f32, rect.top() + 0.5), Pos2::new(rect.right() - radius as f32, rect.top() + 0.5)],
        Stroke::new(1.0, theme::ELEVATION_400_TOP),
    );
}

/// Background for icon-like buttons: hover / pressed / selected states.
pub fn button_bg(p: &Painter, rect: Rect, r: &Response, selected: bool, radius: CornerRadius) {
    let fill = if selected {
        theme::BG_SELECTED
    } else if r.is_pointer_button_down_on() {
        theme::BG_PRESSED
    } else if r.hovered() {
        theme::BG_HOVER
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        p.rect_filled(rect, radius, fill);
    }
}

/// Paints `icon` centered in `boxr` (a button or field slot). Under test the box is
/// recorded so the layout lint can check the glyph really is centered in it.
pub fn paint_icon_in(p: &Painter, boxr: Rect, icon: Icon, size: f32, col: Color32) {
    #[cfg(test)]
    crate::lint::with_box(boxr, || icons::paint(p, icon, boxr.center(), size, col));
    #[cfg(not(test))]
    icons::paint(p, icon, boxr.center(), size, col);
}

// Inter at 11px sits with its baseline 12px below the top of a 16px line box; a 24px
// field centres that line box, so the baseline is 16px under the field's top edge.
// Egui centres the galley by font metrics, which can land half a pixel off, so
// text in fields is placed by baseline instead.

/// Baseline (distance from the galley top) of the first row of `galley`.
pub fn galley_baseline(g: &egui::Galley) -> f32 {
    g.rows.first().map(|r| r.pos.y + r.row.glyphs.first().map(|gl| gl.pos.y).unwrap_or(r.row.size.y * 0.8)).unwrap_or(0.0)
}

/// Pixel-snapped y for text whose baseline must land on `baseline`.
fn snap_y(p: &Painter, y: f32) -> f32 {
    let ppp = p.ctx().pixels_per_point();
    (y * ppp).round() / ppp
}

/// Paints single-line text starting at `x` with its baseline on `baseline`.
/// Returns the text's rect.
pub fn text_on_baseline(p: &Painter, x: f32, baseline: f32, s: &str, font: FontId, col: Color32) -> Rect {
    let g = p.layout_no_wrap(s.to_string(), font, col);
    let pos = Pos2::new(x, snap_y(p, baseline - galley_baseline(&g)));
    let rect = Rect::from_min_size(pos, g.size());
    p.galley(pos, g, col);
    rect
}

/// Inter's (ascent - descent) / 2 in em: the baseline sits this far below the centre of its line box.
const INTER_CENTER_TO_BASELINE: f32 = 0.3638;

/// Baseline of text of `font` size vertically centred in a field/row of `rect`
/// (11px body text in a 24px field: 16px under the top edge).
pub fn centered_baseline(rect: Rect, font: &FontId) -> f32 {
    rect.center().y + INTER_CENTER_TO_BASELINE * font.size
}

/// Text inside a field: left edge at `rect.left() + dx`, baseline per [`centered_baseline`],
/// elided with an ellipsis so it never passes `rect.right() - pad_r`.
pub fn field_text(p: &Painter, rect: Rect, dx: f32, pad_r: f32, s: &str, font: FontId, col: Color32) -> Rect {
    let shown = elide(p, s, &font, (rect.width() - dx - pad_r).max(0.0));
    let b = centered_baseline(rect, &font);
    text_on_baseline(p, rect.left() + dx, b, &shown, font, col)
}

/// Square icon button used throughout the panels (24px by default).
pub fn icon_button(ui: &mut Ui, rect: Rect, id: impl crate::widgets::Key, icon: Icon, selected: bool, tip: &str) -> Response {
    let r = ui.interact(rect, ui.id().with(id), Sense::click());
    let p = ui.painter();
    button_bg(p, rect, &r, selected, theme::cr(theme::RADIUS_CONTROL));
    let col = if selected {
        theme::ICON_SELECTED
    } else if r.hovered() {
        theme::ICON
    } else {
        theme::ICON_SECONDARY
    };
    paint_icon_in(p, rect, icon, 16.0, col);
    if !tip.is_empty() {
        return r.on_hover_text(tip);
    }
    r
}

/// Checkerboard-backed color swatch.
pub fn swatch(p: &Painter, rect: Rect, color: Color32) {
    let r = theme::cr(theme::SWATCH_RADIUS);
    p.rect_filled(rect, r, theme::CHECKER_A);
    if color.a() < 255 {
        let h = rect.height() / 2.0;
        p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(0.0, 0.0), Vec2::new(h, h)), 0.0, theme::CHECKER_B);
        p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(h, h), Vec2::new(h, h)), 0.0, theme::CHECKER_B);
    }
    p.rect_filled(rect, r, color);
    p.rect_stroke(rect, r, Stroke::new(1.0, theme::BORDER_TRANSLUCENT), StrokeKind::Inside);
}

/// Filled circle swatch with a thin translucent ring; several colours blend along the diagonal.
pub fn swatch_circle(p: &Painter, c: Pos2, r: f32, colors: &[Color32]) {
    match colors {
        [] => {}
        [one] => {
            p.circle_filled(c, r, *one);
        }
        many => {
            const N: usize = 40;
            let at = |t: f32| {
                let x = t.clamp(0.0, 1.0) * (many.len() - 1) as f32;
                let i = (x.floor() as usize).min(many.len() - 2);
                let f = x - i as f32;
                many[i].lerp_to_gamma(many[i + 1], f)
            };
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(c, at(0.5));
            for k in 0..=N {
                let a = k as f32 / N as f32 * std::f32::consts::TAU;
                let d = Vec2::new(a.cos(), a.sin());
                mesh.colored_vertex(c + d * r, at(0.5 + (d.x + d.y) * 0.35));
            }
            for k in 1..=N as u32 {
                mesh.add_triangle(0, k, k + 1);
            }
            p.add(egui::Shape::mesh(mesh));
        }
    }
    p.circle_stroke(c, r - 0.5, Stroke::new(1.0, theme::BORDER_TRANSLUCENT));
}

/// Test hook: remembers where a named widget was drawn so UI tests can find it.
#[cfg(test)]
pub fn record(ui: &Ui, name: &str, rect: Rect) {
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(("test-rect", name.to_string())), rect));
}
#[cfg(not(test))]
#[inline]
pub fn record(_ui: &Ui, _name: &str, _rect: Rect) {}

pub use designos_export::util::fmt_num;

#[cfg(test)]
mod tests {
    use super::*;

    /// Value text in a 24px field sits on a baseline 16px below the field's top edge
    /// (Figma: 11px Inter in a 16px line box centred in the field), at any pixel ratio.
    #[test]
    fn field_text_baseline_is_16px_below_the_top() {
        for ppp in [1.0f32, 2.0] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            crate::theme::install_fonts(&ctx);
            let mut baseline = None;
            let field = Rect::from_min_size(Pos2::new(40.0, 100.0), Vec2::new(88.0, 24.0));
            let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))), ..Default::default() };
            raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
            for _ in 0..2 {
                let out = ctx.run_ui(raw.clone(), |ui| {
                    field_text(ui.painter(), field, 8.0, 8.0, "1234", theme::font_body(), theme::TEXT);
                });
                for cs in &out.shapes {
                    if let egui::Shape::Text(t) = &cs.shape {
                        baseline = Some(t.pos.y + galley_baseline(&t.galley));
                    }
                }
            }
            let b = baseline.expect("text painted");
            assert!((b - (field.top() + 16.0)).abs() <= 0.5 / ppp + 1e-3, "baseline {b} at ppp {ppp}");
        }
    }

    #[test]
    fn number_formatting() {
        assert_eq!(fmt_num(10.0), "10");
        assert_eq!(fmt_num(10.5), "10.5");
        assert_eq!(fmt_num(-0.001), "0");
        assert_eq!(fmt_num(1.239), "1.24");
        assert_eq!(fmt_num(0.0), "0");
    }
}
