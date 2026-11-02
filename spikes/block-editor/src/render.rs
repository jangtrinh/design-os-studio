//! THROWAWAY block painting: the LayoutJob that carries bold runs (shared by both backends) and the
//! row chrome (hover handle, list marker, callout surface).

use crate::app::App;
use crate::model::{Block, Kind};
use crate::theme;
use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, UiBuilder};
use std::ops::Range;

pub const COLUMN_W: f32 = 690.0;
pub const GUTTER_W: f32 = 72.0;

pub fn font_size(kind: Kind) -> f32 {
    match kind {
        Kind::H1 => 28.0,
        Kind::H2 => 20.0,
        Kind::H3 => 16.0,
        Kind::Code => 13.0,
        _ => 14.0,
    }
}

pub fn line_height(kind: Kind) -> f32 {
    match kind {
        Kind::H1 => 36.0,
        Kind::H2 => 28.0,
        Kind::H3 => 24.0,
        _ => 24.0,
    }
}

pub fn indent(kind: Kind) -> f32 {
    match kind {
        Kind::Bullet | Kind::Numbered | Kind::Check => 24.0,
        Kind::Callout => 40.0,
        Kind::Code => 16.0,
        _ => 0.0,
    }
}

/// Builds the galley job. `bold` and `underline` are char ranges; both are clamped to `text`.
pub fn job(kind: Kind, text: &str, bold: &[Range<usize>], underline: Option<Range<usize>>, wrap: f32) -> LayoutJob {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut cuts = vec![0, n];
    for r in bold.iter().chain(underline.iter()) {
        cuts.push(r.start.min(n));
        cuts.push(r.end.min(n));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let heading = matches!(kind, Kind::H1 | Kind::H2 | Kind::H3);
    let color = if heading { theme::TEXT } else { theme::TEXT_SECONDARY };
    let size = font_size(kind);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    for w in cuts.windows(2) {
        let seg: String = chars[w[0]..w[1]].iter().collect();
        let is_bold = heading || bold.iter().any(|r| r.start <= w[0] && w[1] <= r.end);
        let under = underline.as_ref().is_some_and(|r| r.start <= w[0] && w[1] <= r.end);
        let font_id = match (kind, is_bold) {
            (Kind::Code, _) => theme::mono(size),
            (_, true) => theme::strong(size),
            _ => theme::regular(size),
        };
        let fmt = TextFormat {
            font_id,
            color: if is_bold && !heading { theme::TEXT } else { color },
            underline: if under { Stroke::new(1.0, theme::TEXT) } else { Stroke::NONE },
            line_height: Some(line_height(kind)),
            ..Default::default()
        };
        job.append(&seg, 0.0, fmt);
    }
    if n == 0 {
        let fmt = TextFormat { font_id: theme::regular(size), color, line_height: Some(line_height(kind)), ..Default::default() };
        job.append("", 0.0, fmt);
    }
    job
}

/// Lays one block out inside a centred 690 px column and calls `content(ui, app, width)` for the
/// editable text. Paints the marker, callout surface and the hover "+" / handle.
pub fn row(ui: &mut Ui, app: &mut App, idx: usize, content: impl FnOnce(&mut Ui, &mut App, f32)) {
    let kind = app.doc.blocks[idx].kind;
    let left = ui.max_rect().min.x + ((ui.available_width() - COLUMN_W) / 2.0).max(GUTTER_W);
    let top = ui.cursor().min.y;
    let inset = indent(kind);
    let pad_y = if matches!(kind, Kind::Callout | Kind::Code) { 12.0 } else { 2.0 };
    let width = COLUMN_W - inset - if kind == Kind::Callout || kind == Kind::Code { 16.0 } else { 0.0 };
    let child = Rect::from_min_size(pos2(left + inset, top + pad_y), vec2(width, 4000.0));
    let used = ui.scope_builder(UiBuilder::new().max_rect(child), |ui| content(ui, app, width)).response.rect;
    let bottom = used.max.y + pad_y;
    let full = Rect::from_min_max(pos2(left, top), pos2(left + COLUMN_W, bottom));
    paint_chrome(ui, app, idx, full);
    ui.allocate_rect(Rect::from_min_max(pos2(left, top), pos2(left + COLUMN_W, bottom + 2.0)), Sense::hover());
}

fn paint_chrome(ui: &mut Ui, app: &mut App, idx: usize, full: Rect) {
    let b: &Block = &app.doc.blocks[idx];
    let p = ui.painter();
    let first_line_y = full.min.y + if matches!(b.kind, Kind::Callout | Kind::Code) { 12.0 } else { 2.0 } + line_height(b.kind) / 2.0;
    match b.kind {
        Kind::Bullet => {
            p.circle_filled(pos2(full.min.x + 8.0, first_line_y), 2.5, theme::TEXT_SECONDARY);
        }
        Kind::Numbered => {
            let n = app.doc.blocks[..=idx].iter().rev().take_while(|x| x.kind == Kind::Numbered).count();
            p.text(pos2(full.min.x + 14.0, first_line_y), Align2::RIGHT_CENTER, format!("{n}."), theme::regular(14.0), theme::TEXT_SECONDARY);
        }
        Kind::Check => {
            let r = Rect::from_center_size(pos2(full.min.x + 8.0, first_line_y), vec2(16.0, 16.0));
            let hit = ui.interact(r, egui::Id::new(("check", b.id)), Sense::click());
            let (fill, stroke) = if b.checked { (theme::BG_BRAND, Stroke::NONE) } else { (Color32::TRANSPARENT, Stroke::new(1.0, theme::BORDER_STRONG)) };
            ui.painter().rect(r, CornerRadius::same(4), fill, stroke, egui::StrokeKind::Inside);
            if b.checked {
                let c = r.center();
                ui.painter().line_segment([c + vec2(-4.0, 0.0), c + vec2(-1.0, 3.0)], Stroke::new(1.8, theme::TEXT));
                ui.painter().line_segment([c + vec2(-1.0, 3.0), c + vec2(4.5, -3.5)], Stroke::new(1.8, theme::TEXT));
            }
            if hit.clicked() {
                app.op_toggle_check(idx);
            }
        }
        Kind::Callout | Kind::Code => {}
        _ => {}
    }
    // Hover handle: "+" and the drag handle (painted only; drag and the block menu are not in the spike).
    let hover = ui.rect_contains_pointer(Rect::from_min_max(pos2(full.min.x - GUTTER_W, full.min.y), pos2(full.max.x, full.max.y)));
    if hover {
        let plus = Rect::from_center_size(pos2(full.min.x - 50.0, first_line_y), vec2(24.0, 24.0));
        let grip = Rect::from_center_size(pos2(full.min.x - 24.0, first_line_y), vec2(24.0, 24.0));
        let resp = ui.interact(plus, egui::Id::new(("plus", app.doc.blocks[idx].id)), Sense::click());
        let col = if resp.hovered() { theme::TEXT_TERTIARY } else { theme::ICON_QUATERNARY };
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(plus, CornerRadius::same(8), theme::BG_SECONDARY);
        }
        p.line_segment([plus.center() + vec2(-5.0, 0.0), plus.center() + vec2(5.0, 0.0)], Stroke::new(1.4, col));
        p.line_segment([plus.center() + vec2(0.0, -5.0), plus.center() + vec2(0.0, 5.0)], Stroke::new(1.4, col));
        for (dx, dy) in [(-2.0, -4.0), (2.0, -4.0), (-2.0, 0.0), (2.0, 0.0), (-2.0, 4.0), (2.0, 4.0)] {
            p.circle_filled(grip.center() + vec2(dx, dy), 1.0, theme::ICON_QUATERNARY);
        }
        if resp.clicked() {
            app.op_insert_block_below(idx);
            app.slash_open_at_caret();
        }
    }
}

/// Reserves the paint slot behind callout / code text; fill it with `fill_surface` once the text rect is known.
pub fn reserve_surface(ui: &Ui) -> egui::layers::ShapeIdx {
    ui.painter().add(egui::Shape::Noop)
}

pub fn fill_surface(ui: &Ui, slot: egui::layers::ShapeIdx, kind: Kind, text_rect: Rect) {
    let pad = match kind {
        Kind::Callout => vec2(40.0, 12.0),
        Kind::Code => vec2(16.0, 12.0),
        _ => return,
    };
    let r = Rect::from_min_max(text_rect.min - pad, pos2(text_rect.min.x - pad.x + COLUMN_W, text_rect.max.y + pad.y));
    ui.painter().set(slot, egui::Shape::rect_filled(r, CornerRadius::same(12), theme::BG_SECONDARY));
    if kind == Kind::Callout {
        ui.painter().circle_stroke(pos2(r.min.x + 20.0, text_rect.min.y + 12.0), 7.0, Stroke::new(1.2, Color32::from_rgb(0x38, 0x9B, 0xF5)));
    }
}
