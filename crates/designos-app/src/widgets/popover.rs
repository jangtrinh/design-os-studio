//! Inspector popovers (effect settings, stroke settings): a 13px-radius elevation-400 panel
//! with a title row and close button, placed left of the inspector next to the row that
//! opened it, like the colour picker.

use super::*;

pub const HEADER_H: f32 = 40.0;
pub const ROW_H: f32 = 32.0;
pub const LABEL_X: f32 = 16.0;
pub const CONTROL_X: f32 = 112.0;
pub const CONTROL_W: f32 = 112.0;

/// Draws the popover; `body` paints the content into the rect below the header.
/// Returns true when it should close: on the X, Escape, or a press outside the panel that is not on
/// `anchor` and not on another foreground popup (dropdown menus opened from the body).
#[allow(clippy::too_many_arguments)]
pub fn show(ctx: &egui::Context, id: Id, anchor: Rect, region: Rect, w: f32, h: f32, title: &str, body: impl FnOnce(&mut Ui, Rect)) -> bool {
    let screen = ctx.content_rect();
    let x = (region.left() - w).max(screen.left() + 8.0);
    let y = (anchor.top() - 8.0).min(screen.bottom() - h - 16.0).max(screen.top() + 8.0);
    let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    let mut close = false;
    let area = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        ui.allocate_rect(rect, Sense::hover());
        {
            let p = ui.painter();
            elevation_400(p, rect, theme::RADIUS_FRAME);
            p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), theme::BG);
            p.rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::ELEVATION_400_RING), StrokeKind::Inside);
            let font = theme::font_body_strong();
            let tr = Rect::from_min_size(rect.min, Vec2::new(w, HEADER_H));
            text_on_baseline(p, rect.left() + LABEL_X, centered_baseline(tr, &font), title, font, theme::TEXT);
            hline(p, rect.left(), rect.right(), rect.top() + HEADER_H - 1.0, theme::BORDER);
        }
        let close_r = Rect::from_min_size(Pos2::new(rect.right() - 8.0 - 24.0, rect.top() + 8.0), Vec2::splat(24.0));
        if icon_button(ui, close_r, "pop-close", Icon::Close, false, "").clicked() {
            close = true;
        }
        let content = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + HEADER_H), rect.right_bottom());
        body(ui, content);
    });
    let layer = area.response.layer_id;
    let (pressed, ptr, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
    let other_popup = |p: Pos2| ctx.layer_id_at(p).is_some_and(|l| l.order == egui::Order::Foreground && l != layer);
    let outside = pressed && ptr.is_some_and(|p| !rect.contains(p) && !anchor.contains(p) && !other_popup(p));
    close || outside || esc
}

/// A labelled row of the popover body: label at x+16, then `control(ui, rect)` in the 112px column.
pub fn row(ui: &mut Ui, body: Rect, index: usize, label: &str, control: impl FnOnce(&mut Ui, Rect)) {
    let top = body.top() + 8.0 + index as f32 * ROW_H;
    let line = Rect::from_min_size(Pos2::new(body.left(), top), Vec2::new(body.width(), 24.0));
    let font = theme::font_body();
    text_on_baseline(ui.painter(), body.left() + LABEL_X, centered_baseline(line, &font), label, font, theme::TEXT_SECONDARY);
    let r = Rect::from_min_size(Pos2::new(body.left() + CONTROL_X, top), Vec2::new(CONTROL_W, 24.0));
    control(ui, r);
}

/// Height of a popover with `rows` rows.
pub fn height(rows: usize) -> f32 {
    HEADER_H + 8.0 + rows as f32 * ROW_H + 8.0
}

/// Check box with a label (14px box, brand fill when on). Returns the new value on click.
pub fn check(ui: &mut Ui, r: Rect, key: impl Key, value: bool) -> Option<bool> {
    let resp = ui.interact(r, ui.id().with(("pop-check", key)), Sense::click());
    let b = Rect::from_min_size(Pos2::new(r.left(), r.center().y - 7.0), Vec2::splat(14.0));
    let p = ui.painter();
    if value {
        p.rect_filled(b, theme::cr(theme::RADIUS_MARK), theme::BG_BRAND);
        paint_icon_in(p, b, Icon::Check, 12.0, theme::TEXT);
    } else {
        p.rect_stroke(b, theme::cr(theme::RADIUS_MARK), Stroke::new(1.0, theme::TEXT_TERTIARY), StrokeKind::Inside);
    }
    resp.clicked().then_some(!value)
}

/// Round dial for an angle in degrees (0 = right, clockwise). Returns the new angle while dragging.
pub fn dial(ui: &mut Ui, r: Rect, key: impl Key, angle: f32) -> Option<f32> {
    let resp = ui.interact(r, ui.id().with(("pop-dial", key)), Sense::click_and_drag());
    let c = r.center();
    let rad = r.width().min(r.height()) / 2.0 - 0.5;
    let p = ui.painter();
    p.circle_filled(c, rad, theme::BG_SECONDARY);
    let hover = resp.hovered() || resp.dragged();
    p.circle_stroke(c, rad - 0.5, Stroke::new(1.0, if hover { theme::BORDER_STRONG.gamma_multiply(0.3) } else { theme::BORDER }));
    let a = angle.to_radians();
    let tip = c + Vec2::new(a.cos(), a.sin()) * (rad - 4.0);
    p.line_segment([c, tip], Stroke::new(1.5, theme::TEXT));
    p.circle_filled(tip, 2.5, theme::TEXT);
    let mut out = None;
    if resp.dragged() || resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(Id::new(field::SCRUB_FLAG), true));
        if let Some(pt) = resp.interact_pointer_pos() {
            let d = pt - c;
            if d.length() > 1.0 {
                let mut deg = d.y.atan2(d.x).to_degrees();
                if deg < 0.0 {
                    deg += 360.0;
                }
                out = Some(deg.round());
            }
        }
    }
    out
}
