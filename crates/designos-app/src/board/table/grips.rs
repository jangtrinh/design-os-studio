//! Row and column grips, the "+" on a line, and the drags they start. Nothing is committed while a
//! drag runs: the overlay draws a ghost line, and the release runs one command (one undo step).

use super::{commit, table_of, TableGeo};
use crate::app::App;
use crate::theme;
use designos_doc::table_ops;
use designos_doc::{NodeId, Vec2};
use egui::{Pos2, Rect};

/// Visible grip: 12 x 20, "+" 16, each in a pointer area of at least 24 px (owner choice for row and column handles).
pub const GRIP_AREA: f32 = 24.0;
/// How far from the table edge the grips sit.
pub const GRIP_GAP: f32 = 16.0;
/// Pointer reach, in screen px, of a track line.
const LINE_REACH: f32 = 4.0;
/// How far, in screen px, the grip and "+" lanes reach out before the table's top-left corner.
const CHROME_REACH: f32 = 3.0 * GRIP_AREA;

/// What the pointer is over on a selected table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    /// Grip of row / column `i`.
    Grip { rows: bool, index: usize },
    /// "+" on the line before row / column `index` (`index == len` is the last line).
    Plus { rows: bool, index: usize },
    /// The line after row / column `index`, drag to resize.
    Line { rows: bool, index: usize },
}

/// An open drag on a table, state only; the document is untouched until the release.
#[derive(Clone, Debug)]
pub enum TableDrag {
    /// Dragging across cells to select a block, from `anchor`.
    Block {
        table: NodeId,
        anchor: NodeId,
    },
    Resize {
        table: NodeId,
        rows: bool,
        index: usize,
        start_size: f32,
        start_local: f32,
        size: f32,
        moved: bool,
    },
    Reorder {
        table: NodeId,
        rows: bool,
        from: usize,
        to: usize,
        refusal: Option<String>,
    },
}

/// Screen position of a table-local point.
pub fn screen(app: &App, geo: &TableGeo, x: f32, y: f32) -> Pos2 {
    let w = geo.world.apply(Vec2::new(x, y));
    app.view.to_screen(app.canvas_rect.min, w)
}

/// The table whose editing chrome shows: the selected table, or the table of the selected cells.
pub fn active_table(app: &App) -> Option<NodeId> {
    app.selection.first().and_then(|id| table_of(&app.doc, *id))
}

fn track_under(edges: &[f32], at: impl Fn(f32) -> f32, v: f32) -> usize {
    (0..edges.len() - 1).find(|i| at(edges[i + 1]) > v).unwrap_or(edges.len() - 2)
}

fn mid(edges: &[f32], i: usize) -> f32 {
    (edges[i] + edges[i + 1]) / 2.0
}

/// Where a control of the table chrome can be pressed: the canvas, less the ruler strips and their press band.
pub fn live_area(app: &App) -> Rect {
    let mut r = app.canvas_rect;
    if app.ui.rulers {
        r.min += egui::vec2(theme::DRAG_DIVIDER_HIT, theme::DRAG_DIVIDER_HIT);
    } else if !app.ui.left_hidden() {
        // without rulers the Layers panel's resize band reaches this far into the canvas
        r.min.x += theme::DRAG_DIVIDER_HIT * 0.5;
    }
    r
}

/// Pointer areas, in screen px, of everything a selected table offers around `pos`.
pub fn areas(app: &App, geo: &TableGeo, pos: Pos2) -> Vec<(Hover, Rect)> {
    // the chrome is laid out along the screen axes: a rotated table gets none (its menu still works)
    if geo.world.b.abs() > 1e-4 || geo.world.c.abs() > 1e-4 || geo.world.a < 0.0 || geo.world.d < 0.0 {
        return Vec::new();
    }
    let (rows, cols) = (geo.props.rows.len(), geo.props.cols.len());
    let tl = screen(app, geo, 0.0, 0.0);
    let br = screen(app, geo, *geo.col_x.last().unwrap_or(&0.0), *geo.row_y.last().unwrap_or(&0.0));
    let near_x = pos.x >= tl.x - CHROME_REACH && pos.x <= br.x + LINE_REACH;
    let near_y = pos.y >= tl.y - CHROME_REACH && pos.y <= br.y + LINE_REACH;
    if !(near_x && near_y) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let row = track_under(&geo.row_y, |y| screen(app, geo, 0.0, y).y, pos.y);
    let col = track_under(&geo.col_x, |x| screen(app, geo, x, 0.0).x, pos.x);
    let gx = tl.x - GRIP_GAP;
    let gy = tl.y - GRIP_GAP;
    let area = |c: Pos2| Rect::from_center_size(c, egui::vec2(GRIP_AREA, GRIP_AREA));
    // the grip is tested first: a 24 px box on screen, whatever the zoom, centred on the drawn grip
    let gy_mid = screen(app, geo, 0.0, mid(&geo.row_y, row)).y;
    out.push((Hover::Grip { rows: true, index: row }, area(Pos2::new(gx, gy_mid))));
    let gx_mid = screen(app, geo, mid(&geo.col_x, col), 0.0).x;
    out.push((Hover::Grip { rows: false, index: col }, area(Pos2::new(gx_mid, gy))));
    // the "+" gets what the grips leave: it sits on a line, the grip on a track
    let line_row =
        (0..=rows).min_by(|a, b| (screen(app, geo, 0.0, geo.row_y[*a]).y - pos.y).abs().total_cmp(&(screen(app, geo, 0.0, geo.row_y[*b]).y - pos.y).abs()));
    let line_col =
        (0..=cols).min_by(|a, b| (screen(app, geo, geo.col_x[*a], 0.0).x - pos.x).abs().total_cmp(&(screen(app, geo, geo.col_x[*b], 0.0).x - pos.x).abs()));
    // the "+" has a lane of its own, one grip further out
    if let Some(i) = line_row {
        out.push((Hover::Plus { rows: true, index: i }, area(Pos2::new(gx - GRIP_AREA, screen(app, geo, 0.0, geo.row_y[i]).y))));
    }
    if let Some(j) = line_col {
        out.push((Hover::Plus { rows: false, index: j }, area(Pos2::new(screen(app, geo, geo.col_x[j], 0.0).x, gy - GRIP_AREA))));
    }
    // drawn means live: a box that leaves the live area of the canvas (the ruler strips and their press band
    // included) is neither drawn nor pressable, so a grip or "+" is never a dead control
    let live = live_area(app);
    out.retain(|(_, r)| live.contains_rect(*r));
    // inner lines: drag to resize
    let inside = pos.x >= tl.x && pos.x <= br.x && pos.y >= tl.y && pos.y <= br.y;
    if inside {
        for i in 0..rows {
            let y = screen(app, geo, 0.0, geo.row_y[i + 1]).y;
            out.push((Hover::Line { rows: true, index: i }, Rect::from_min_max(Pos2::new(tl.x, y - LINE_REACH), Pos2::new(br.x, y + LINE_REACH))));
        }
        for j in 0..cols {
            let x = screen(app, geo, geo.col_x[j + 1], 0.0).x;
            out.push((Hover::Line { rows: false, index: j }, Rect::from_min_max(Pos2::new(x - LINE_REACH, tl.y), Pos2::new(x + LINE_REACH, br.y))));
        }
    }
    out
}

/// The first area under `pos` (the order of [`areas`] is the priority: lines last).
pub fn hover_at(app: &App, pos: Pos2) -> Option<(NodeId, Hover)> {
    // while a cell is edited the chrome is hidden, so it must not act either
    if app.canvas.text.is_some() {
        return None;
    }
    let table = active_table(app)?;
    let geo = TableGeo::of(&app.doc, table)?;
    let all = areas(app, &geo, pos);
    let line = all.iter().find(|(h, r)| matches!(h, Hover::Line { .. }) && r.contains(pos));
    let other = all.iter().find(|(h, r)| !matches!(h, Hover::Line { .. }) && r.contains(pos));
    other.or(line).map(|(h, _)| (table, *h))
}

fn local_axis(_app: &App, geo: &TableGeo, page: Vec2, rows: bool) -> f32 {
    let l = geo.world.invert().map_or(page, |inv| inv.apply(page));
    if rows {
        l.y
    } else {
        l.x
    }
}

/// Starts the drag or the click a press on `hover` means. Returns the drag, if any.
pub fn press(app: &mut App, table: NodeId, hover: Hover, page: Vec2) -> Option<TableDrag> {
    let geo = TableGeo::of(&app.doc, table)?;
    match hover {
        Hover::Plus { rows, index } => {
            let cmd = if rows { table_ops::insert_rows(&app.doc, table, index, 1) } else { table_ops::insert_cols(&app.doc, table, index, 1) };
            match cmd {
                Ok(cmd) => {
                    commit(app, table, cmd, if rows { "Insert row" } else { "Insert column" });
                }
                Err(e) => app.show_toast(e.to_string(), true),
            }
            None
        }
        Hover::Line { rows, index } => {
            let edges = if rows { &geo.row_y } else { &geo.col_x };
            let start_size = edges[index + 1] - edges[index];
            Some(TableDrag::Resize { table, rows, index, start_size, start_local: local_axis(app, &geo, page, rows), size: start_size, moved: false })
        }
        Hover::Grip { rows, index } => {
            // a grip click selects its row or column; a drag past it reorders
            select_track(app, &geo, rows, index);
            Some(TableDrag::Reorder { table, rows, from: index, to: index, refusal: None })
        }
    }
}

/// Selects every cell of one row or column.
pub fn select_track(app: &mut App, geo: &TableGeo, rows: bool, index: usize) {
    let r = if rows { (index, 0, index, geo.props.cols.len() - 1) } else { (0, index, geo.props.rows.len() - 1, index) };
    app.selection = geo.cells_in(r);
}

/// Moves an open drag to the pointer.
/// `reach` is egui's click distance in screen px: a line moves only once the pointer is past it.
pub fn drag(app: &App, d: &mut TableDrag, page: Vec2, reach: f32) {
    match d {
        TableDrag::Resize { table, rows, start_size, start_local, size, moved, .. } => {
            if let Some(geo) = TableGeo::of(&app.doc, *table) {
                let delta = local_axis(app, &geo, page, *rows) - *start_local;
                if !*moved && delta.abs() * app.view.zoom < reach {
                    return;
                }
                *moved = true;
                *size = (*start_size + delta).max(designos_doc::table::TABLE_MIN_TRACK);
            }
        }
        TableDrag::Reorder { table, rows, from, to, refusal } => {
            let Some(geo) = TableGeo::of(&app.doc, *table) else { return };
            let v = local_axis(app, &geo, page, *rows);
            let edges = if *rows { &geo.row_y } else { &geo.col_x };
            *to = (0..edges.len() - 1).find(|i| edges[i + 1] > v).unwrap_or(edges.len() - 2);
            let id = if *rows { geo.row_id(*from) } else { geo.col_id(*from) };
            let plan = if *rows { table_ops::move_row(&app.doc, *table, id, *to) } else { table_ops::move_col(&app.doc, *table, id, *to) };
            *refusal = match plan {
                Err(table_ops::TableOpError::SplitsMerge(m)) => Some(m),
                _ => None,
            };
        }
        TableDrag::Block { .. } => {}
    }
}

/// Ends the drag: one command, or nothing. A refused reorder says what to do first.
pub fn release(app: &mut App, d: TableDrag) {
    match d {
        TableDrag::Resize { table, rows, index, size, start_size, moved, .. } => {
            if !moved || (size - start_size).abs() < 0.5 {
                return;
            }
            let Some(geo) = TableGeo::of(&app.doc, table) else { return };
            let id = if rows { geo.row_id(index) } else { geo.col_id(index) };
            let cmd = if rows { table_ops::resize_row(&app.doc, table, id, size) } else { table_ops::resize_col(&app.doc, table, id, size) };
            match cmd {
                Ok(cmd) => {
                    commit(app, table, cmd, if rows { "Resize row" } else { "Resize column" });
                }
                Err(table_ops::TableOpError::NoChange(_)) => {}
                Err(e) => app.show_toast(e.to_string(), true),
            }
        }
        TableDrag::Reorder { table, rows, from, to, refusal } => {
            if from == to {
                return;
            }
            if let Some(msg) = refusal {
                app.show_toast(msg, true);
                return;
            }
            let Some(geo) = TableGeo::of(&app.doc, table) else { return };
            let id = if rows { geo.row_id(from) } else { geo.col_id(from) };
            let cmd = if rows { table_ops::move_row(&app.doc, table, id, to) } else { table_ops::move_col(&app.doc, table, id, to) };
            match cmd {
                Ok(cmd) => {
                    if commit(app, table, cmd, if rows { "Move row" } else { "Move column" }) {
                        if let Some(g) = TableGeo::of(&app.doc, table) {
                            select_track(app, &g, rows, to);
                        }
                    }
                }
                Err(table_ops::TableOpError::NoChange(_)) => {}
                Err(e) => app.show_toast(e.to_string(), true),
            }
        }
        TableDrag::Block { .. } => {}
    }
}
