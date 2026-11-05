use crate::theme;
use egui::{Color32, Painter, Rect, Stroke, StrokeKind};

/// Focus band colour. The 2 px gap and the outer radius are drawn with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingTone {
    Brand,
    Error,
}

pub const fn ring_color(tone: RingTone) -> Color32 {
    match tone {
        RingTone::Brand => theme::FOCUS_RING,
        RingTone::Error => theme::FOCUS_RING_ERROR,
    }
}

fn snap(p: &Painter, value: f32) -> f32 {
    let pixels = p.ctx().pixels_per_point();
    (value * pixels).round() / pixels
}

/// 2 px band from 2 px to 4 px outside `rect`. The outer corner radius is `radius + 4`.
pub fn focus_ring(p: &Painter, rect: Rect, radius: f32) {
    paint_ring(p, rect, radius, ring_color(RingTone::Brand));
}

pub(crate) fn paint_ring(p: &Painter, rect: Rect, radius: f32, color: Color32) {
    if color == Color32::TRANSPARENT {
        return;
    }
    let widened = p.clip_rect().expand(theme::SPACE_FACT);
    // `with_clip_rect` intersects, so it cannot grow. Set the widened rect after that call.
    let mut p = p.with_clip_rect(widened);
    p.set_clip_rect(widened);
    let outer = rect.expand(theme::SPACE_FACT);
    let snapped = Rect::from_min_max(egui::pos2(snap(&p, outer.min.x), snap(&p, outer.min.y)), egui::pos2(snap(&p, outer.max.x), snap(&p, outer.max.y)));
    let curve = (radius + theme::SPACE_FACT).round().clamp(0.0, 255.0) as u8;
    p.rect_stroke(snapped, egui::CornerRadius::same(curve), Stroke::new(2.0, color), StrokeKind::Inside);
}

/// Keyboard focus, separate from the pointer state a look encodes.
pub fn keyboard_focus(ui: &egui::Ui, id: egui::Id) -> bool {
    ui.memory(|mem| mem.has_focus(id))
}
