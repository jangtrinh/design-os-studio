//! Paints the editing chrome of a selected table: the block outline, the grip and "+" under the
//! pointer, the line being resized, and where a dragged row or column would land.

use super::grips::{self, Hover, TableDrag, GRIP_AREA};
use super::TableGeo;
use crate::app::App;
use crate::theme;
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

const GRIP_LONG: f32 = 20.0;
const GRIP_SHORT: f32 = 12.0;
const PLUS_SIZE: f32 = 16.0;
/// Corner radius of a grip, of the "+" and of their lit hover boxes.
const GRIP_RADIUS: u8 = 4;

fn dots(p: &Painter, center: Pos2, vertical: bool) {
    let (cols, rows) = if vertical { (2, 3) } else { (3, 2) };
    for i in 0..cols {
        for j in 0..rows {
            let dx = (i as f32 - (cols as f32 - 1.0) / 2.0) * 4.0;
            let dy = (j as f32 - (rows as f32 - 1.0) / 2.0) * 4.0;
            p.circle_filled(center + Vec2::new(dx, dy), 1.0, theme::CANVAS_HANDLE_FILL);
        }
    }
}

fn paint_grip(p: &Painter, area: Rect, rows: bool, lit: bool) {
    let area = Rect::from_center_size(area.center(), Vec2::splat(GRIP_AREA));
    p.rect_filled(area, theme::cr(GRIP_RADIUS), if lit { theme::BG_SELECTED_AREA } else { Color32::TRANSPARENT });
    let size = if rows { Vec2::new(GRIP_SHORT, GRIP_LONG) } else { Vec2::new(GRIP_LONG, GRIP_SHORT) };
    let shape = Rect::from_center_size(area.center(), size);
    p.rect_filled(shape, theme::cr(GRIP_RADIUS), theme::CANVAS_SELECTION);
    dots(p, shape.center(), rows);
}

fn paint_plus(p: &Painter, area: Rect, lit: bool) {
    p.rect_filled(area, theme::cr(GRIP_RADIUS), if lit { theme::BG_SELECTED_AREA } else { Color32::TRANSPARENT });
    p.circle_filled(area.center(), PLUS_SIZE / 2.0, theme::CANVAS_SELECTION);
    let s = Stroke::new(1.5, theme::CANVAS_HANDLE_FILL);
    let c = area.center();
    p.line_segment([c - Vec2::new(4.0, 0.0), c + Vec2::new(4.0, 0.0)], s);
    p.line_segment([c - Vec2::new(0.0, 4.0), c + Vec2::new(0.0, 4.0)], s);
}

/// One track line of the table in screen space.
fn line(app: &App, geo: &TableGeo, rows: bool, at: f32) -> [Pos2; 2] {
    if rows {
        [grips::screen(app, geo, 0.0, at), grips::screen(app, geo, *geo.col_x.last().unwrap_or(&0.0), at)]
    } else {
        [grips::screen(app, geo, at, 0.0), grips::screen(app, geo, at, *geo.row_y.last().unwrap_or(&0.0))]
    }
}

pub fn draw(app: &App, p: &Painter) {
    let Some(table) = grips::active_table(app) else { return };
    let Some(geo) = TableGeo::of(&app.doc, table) else { return };
    let accent = Stroke::new(2.0, theme::CANVAS_SELECTION);
    if let super::TableDragView::Drag(d) = super::drag_view(app) {
        match d {
            TableDrag::Resize { rows, index, size, start_size, .. } => {
                let edges = if *rows { &geo.row_y } else { &geo.col_x };
                p.line_segment(line(app, &geo, *rows, edges[*index] + *size), accent);
                let _ = start_size;
            }
            TableDrag::Reorder { rows, to, from, refusal, .. } => {
                let edges = if *rows { &geo.row_y } else { &geo.col_x };
                let at = if to > from { edges[*to + 1] } else { edges[*to] };
                if refusal.is_none() {
                    p.line_segment(line(app, &geo, *rows, at), accent);
                }
            }
            TableDrag::Block { .. } => {}
        }
        return;
    }
    let Some(pos) = app.canvas.pointer_page.map(|pg| app.view.to_screen(app.canvas_rect.min, pg)) else { return };
    if app.canvas.text.is_some() {
        return;
    }
    let all = grips::areas(app, &geo, pos);
    let over = grips::hover_at(app, pos).map(|h| h.1);
    for (h, area) in all {
        match h {
            Hover::Grip { rows, .. } => paint_grip(p, area, rows, over == Some(h)),
            Hover::Plus { .. } => paint_plus(p, area, over == Some(h)),
            Hover::Line { rows, index } if over == Some(h) => {
                let edges = if rows { &geo.row_y } else { &geo.col_x };
                p.line_segment(line(app, &geo, rows, edges[index + 1]), accent);
            }
            Hover::Line { .. } => {}
        }
    }
}
