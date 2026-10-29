//! Custom-painted widgets that replace stock egui looks.

/// Anything usable as an egui id salt.
pub trait Key: std::hash::Hash + std::fmt::Debug {}
impl<T: std::hash::Hash + std::fmt::Debug> Key for T {}

pub mod field;
pub mod field_tab;
pub mod kit;
pub mod menu;
pub mod picker;
pub mod popover;
pub mod popup_open;

use crate::icons::{self, Icon};
use crate::theme;
use egui::{Align2, Color32, CornerRadius, FontId, Id, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

/// True while the pointer is over a popup or menu (anything drawn as a foreground area).
pub fn pointer_over_popup(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.pointer.latest_pos()).is_some_and(|p| ctx.layer_id_at(p).is_some_and(|l| l.order == egui::Order::Foreground))
}

/// `one` when `n == 1`, `many` otherwise, so a count never reads `1 layers`.
pub fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// Width of a string in the given font.
pub fn text_width(p: &Painter, s: &str, font: &FontId) -> f32 {
    p.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x
}

pub(crate) fn elide_with(mut width_of: impl FnMut(&str) -> f32, s: &str, max_w: f32) -> (String, f32) {
    let full_w = width_of(s);
    if full_w <= max_w {
        return (s.to_string(), full_w);
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cand: String = chars[..mid].iter().collect::<String>() + "…";
        if width_of(&cand) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let out = chars[..lo].iter().collect::<String>() + "…";
    let w = width_of(&out);
    (out, w)
}

/// Shortens `s` with an ellipsis so that it fits `max_w`.
pub fn elide(p: &Painter, s: &str, font: &FontId, max_w: f32) -> String {
    elide_with(|cand| text_width(p, cand, font), s, max_w).0
}

pub fn hline(p: &Painter, x0: f32, x1: f32, y: f32, color: Color32) {
    p.rect_filled(Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + theme::BORDER_WIDTH)), 0.0, color);
}

pub fn vline(p: &Painter, x: f32, y0: f32, y1: f32, color: Color32) {
    p.rect_filled(Rect::from_min_max(Pos2::new(x, y0), Pos2::new(x + theme::BORDER_WIDTH, y1)), 0.0, color);
}

/// Floating chrome draws no shadow. The arguments stay so existing callers compile.
pub fn shadow(_p: &Painter, _rect: Rect, _radius: u8, _offset: Vec2, _blur: f32, _color: Color32) {}

/// Floating chrome draws no shadow.
pub fn elevation_shadow(_p: &Painter, _rect: Rect, _radius: u8) {}

/// Floating chrome draws no shadow.
pub fn elevation_400(_p: &Painter, _rect: Rect, _radius: u8) {}

/// Menu chrome. The caller's radius is ignored; the surface uses the frame radius.
pub fn menu_panel(p: &Painter, rect: Rect, _radius: u8) {
    kit::menu_surface(p, rect);
}

/// Icon-button fill and keyboard ring. The caller owns the corner radius and the glyph.
pub fn button_bg(p: &Painter, rect: Rect, r: &Response, selected: bool, radius: CornerRadius) {
    kit::paint_icon_bg(p, rect, r, selected, radius);
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
    let r = kit::button_icon(ui, rect, id, icon, kit::Hierarchy::Icon, kit::Size::Xs, ui.is_enabled(), selected);
    if !tip.is_empty() {
        return r.on_hover_text(tip);
    }
    r
}

/// Checkerboard-backed color swatch.
pub fn swatch(p: &Painter, rect: Rect, color: Color32) {
    swatch_focused(p, rect, color, false);
}

/// A swatch inside a control that holds keyboard focus: the control passes its own focus, so the swatch never takes a click.
#[cfg_attr(not(test), allow(dead_code))]
pub fn swatch_focused(p: &Painter, rect: Rect, color: Color32, focused: bool) {
    let r = theme::cr(theme::SWATCH_RADIUS);
    p.rect_filled(rect, r, theme::CHECKER_A);
    if color.a() < 255 {
        let h = rect.height() / 2.0;
        p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(0.0, 0.0), Vec2::new(h, h)), 0.0, theme::CHECKER_B);
        p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(h, h), Vec2::new(h, h)), 0.0, theme::CHECKER_B);
    }
    p.rect_filled(rect, r, color);
    kit::swatch_ring(p, rect, false, focused);
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
    let edge = kit::swatch_look(kit::State::Default);
    p.circle_stroke(c, r - 0.5, Stroke::new(edge.border_w, edge.border));
}

/// Sets the AccessKit role and accessible name on `id` when AccessKit is active.
///
/// Returns immediately without locking `Context` when AccessKit is disabled.
#[inline]
pub fn labelled(ui: &Ui, id: Id, role: egui::accesskit::Role, name: &str) {
    if !crate::agent_a11y::is_accesskit_active() {
        return;
    }
    ui.ctx().accesskit_node_builder(id, |node| {
        node.set_role(role);
        node.set_label(name);
    });
}

/// Relaxed flag turning on `record` pass listing in release builds (`--layout-json`).
pub static RECORDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    static CURRENT_PASS_NAMES: std::cell::RefCell<Vec<(String, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
    static LAST_PASS_NAMES: std::cell::RefCell<Vec<(String, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Moves the current pass's recorded `(name, rect)` pairs into `last_pass_names`.
pub fn finish_pass_records() {
    CURRENT_PASS_NAMES.with(|cur| {
        LAST_PASS_NAMES.with(|last| {
            *last.borrow_mut() = std::mem::take(&mut *cur.borrow_mut());
        });
    });
}

/// Returns the `(name, rect)` pairs recorded during the last completed pass.
#[allow(dead_code)]
pub fn last_pass_names() -> Vec<(String, Rect)> {
    LAST_PASS_NAMES.with(|last| last.borrow().clone())
}

/// Clears any in-progress pass records and `last_pass_names` on the current thread.
#[allow(dead_code)]
pub fn clear_pass_records() {
    CURRENT_PASS_NAMES.with(|cur| cur.borrow_mut().clear());
    LAST_PASS_NAMES.with(|last| last.borrow_mut().clear());
}

/// Test hook: remembers where a named widget was drawn so UI tests and layout snapshots can find it.
#[cfg(test)]
pub fn record(ui: &Ui, name: &str, rect: Rect) {
    if RECORD_DISABLED.with(std::cell::Cell::get) {
        return;
    }
    let _ = RECORDING.load(std::sync::atomic::Ordering::Relaxed);
    let global_rect = ui.ctx().layer_transform_to_global(ui.layer_id()).map_or(rect, |t| t * rect);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(("test-rect", name.to_string())), rect));
    CURRENT_PASS_NAMES.with(|cur| cur.borrow_mut().push((name.to_string(), global_rect)));
}

/// True when [`record`] will store the widget's name and rect on this pass.
#[cfg(test)]
#[inline]
pub fn recording() -> bool {
    !RECORD_DISABLED.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    static RECORD_DISABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Test support: makes [`record`] store nothing on this thread while the guard lives, as it does in a release
/// build. A test that reads a widget's rect first, then drops the stored rects and holds this guard, runs the
/// widget code the way a release build runs it: any production code that still reads a recorded rect finds none.
/// Per thread, so parallel tests keep their own `record` calls; the setting is restored when the guard drops,
/// also on a panic.
#[cfg(test)]
pub struct RecordDisabled(());

#[cfg(test)]
impl RecordDisabled {
    pub fn begin() -> Self {
        RECORD_DISABLED.with(|d| d.set(true));
        Self(())
    }
}

#[cfg(test)]
impl Drop for RecordDisabled {
    fn drop(&mut self) {
        RECORD_DISABLED.with(|d| d.set(false));
    }
}
#[cfg(not(test))]
#[inline]
pub fn record(ui: &Ui, name: &str, rect: Rect) {
    if RECORDING.load(std::sync::atomic::Ordering::Relaxed) {
        let global_rect = ui.ctx().layer_transform_to_global(ui.layer_id()).map_or(rect, |t| t * rect);
        CURRENT_PASS_NAMES.with(|cur| cur.borrow_mut().push((name.to_string(), global_rect)));
    }
}

/// True when [`record`] will store the widget's name and rect on this pass.
#[cfg(not(test))]
#[inline]
pub fn recording() -> bool {
    RECORDING.load(std::sync::atomic::Ordering::Relaxed)
}

/// Like [`record`], formatting `name` lazily only when [`recording`] is active so release frames
/// with `RECORDING = false` do not allocate.
#[inline]
pub fn record_with<S: AsRef<str>>(ui: &Ui, name: impl FnOnce() -> S, rect: Rect) {
    if recording() {
        let s = name();
        record(ui, s.as_ref(), rect);
    }
}

pub use designos_export::util::fmt_num;

#[cfg(test)]
mod tests {
    use super::*;

    /// Value text in a field sits on a baseline 0.3638 em below the field's centre line
    /// (Inter's centre-to-baseline distance), at any pixel ratio.
    #[test]
    fn field_text_baseline_is_centred_in_the_field() {
        for ppp in [1.0f32, 2.0] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            crate::theme::install_fonts(&ctx);
            let mut baseline = None;
            let field = Rect::from_min_size(Pos2::new(40.0, 100.0), Vec2::new(88.0, theme::FIELD_HEIGHT));
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
            let expected = field.center().y + 0.3638 * theme::FONT_SIZE_BODY;
            assert!((b - expected).abs() <= 0.5 / ppp + 1e-3, "baseline {b} at ppp {ppp}, expected {expected}");
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

    /// 1b: two calls are listed; none under `RecordDisabled`; the next pass starts empty.
    #[test]
    fn record_hook_lists_every_name_of_the_pass() {
        clear_pass_records();
        let ctx = egui::Context::default();
        let r1 = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(30.0, 40.0));
        let r2 = Rect::from_min_size(Pos2::new(50.0, 60.0), Vec2::new(70.0, 80.0));

        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))), ..Default::default() };
        let _ = ctx.run_ui(raw.clone(), |ui| {
            record(ui, "alpha", r1);
            record(ui, "beta", r2);
            let _disabled = RecordDisabled::begin();
            record(ui, "ignored", r1);
        });
        finish_pass_records();
        assert_eq!(last_pass_names(), vec![("alpha".to_string(), r1), ("beta".to_string(), r2)], "two calls must be listed and RecordDisabled call ignored");

        let _ = ctx.run_ui(raw, |_ui| {});
        finish_pass_records();
        assert!(last_pass_names().is_empty(), "the next pass with no record calls must start empty");
    }
}
