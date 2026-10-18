//! Canvas guides drawn over the page: the pixel grid (from 800% zoom) and the
//! layout grids (`FrameProps::grids`) of frames.

use super::*;
use crate::theme;
use crate::util::*;
use designos_doc::layout_solver::{place_children, ChildIn};
use designos_doc::{AutoLayout, Color, Document, GridTrack, LayoutDirection, LayoutGrid, NodeKind, Sizing, MAX_GRID_TRACKS};
use egui::{Color32, Painter, Pos2, Shape, Stroke};

/// One resolved `(row, col)` cell in a `LayoutDirection::Grid` container (frame-local coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CssGridCell {
    pub row: usize,
    pub col: usize,
    pub rect: Rect,
}

/// Complete on-canvas overlay geometry for a 2D CSS Grid container (`LayoutDirection::Grid`).
#[derive(Clone, Debug, PartialEq)]
pub struct CssGridOverlay {
    pub cols: usize,
    pub rows: usize,
    pub col_x: Vec<f32>,
    pub col_widths: Vec<f32>,
    pub row_y: Vec<f32>,
    pub row_heights: Vec<f32>,
    pub col_gap: f32,
    pub row_gap: f32,
    pub padding: [f32; 4],
    pub cells: Vec<CssGridCell>,
    pub col_tracks: Vec<GridTrack>,
    pub row_tracks: Vec<GridTrack>,
}

/// Formats a [`GridTrack`] for an on-canvas column/row track header pill (`1fr`, `120px`, `Hug`).
pub fn format_grid_track(track: GridTrack) -> String {
    match track {
        GridTrack::Flex(fr) => {
            let f = fr.max(0.1);
            if (f - f.round()).abs() < 0.05 {
                format!("{}fr", f.round() as i32)
            } else {
                format!("{:.1}fr", f)
            }
        }
        GridTrack::Fixed(px) => format!("{}px", px.round() as i32),
        GridTrack::Hug => "Hug".to_string(),
    }
}

/// How many tracks the canvas shows on one axis of `grid_id`; 0 when it is not a grid. The children are placed exactly
/// as the layout solver places them (explicit rows and columns included), so the rows are the ones it lays out
/// (`grid_rows`, or more when children fill more rows) and a pill, a `+` button, an inspector row and a track edit
/// all name the same index.
pub fn grid_track_count(doc: &Document, grid_id: NodeId, is_col: bool) -> usize {
    node_css_grid_overlay(doc, grid_id).map_or(0, |ov| if is_col { ov.cols } else { ov.rows })
}

/// One edit of the tracks on one axis of a grid.
#[derive(Clone, Copy, Debug)]
enum TrackEdit {
    Insert { at: usize, track: GridTrack },
    Delete { at: usize },
    Move { from: usize, to: usize },
    Resize { at: usize, track: GridTrack },
}

/// Track slots of one axis. A list is never shorter than `MAX_GRID_TRACKS`, so every laid-out track has a slot.
fn slots_of(al: &mut AutoLayout, is_col: bool) -> &mut Vec<GridTrack> {
    let tracks = if is_col { &mut al.grid_col_tracks } else { &mut al.grid_row_tracks };
    tracks.resize_with_default(MAX_GRID_TRACKS);
    &mut tracks.0
}

/// Drops the default slots an insert pushed past `MAX_GRID_TRACKS` and pads a delete back up to it.
fn trim_slots(slots: &mut Vec<GridTrack>) {
    let used = slots.iter().rposition(|&t| t != GridTrack::default()).map_or(0, |i| i + 1);
    slots.truncate(used.max(MAX_GRID_TRACKS));
    if slots.len() < MAX_GRID_TRACKS {
        slots.resize(MAX_GRID_TRACKS, GridTrack::default());
    }
}

fn remap_reordered_track(idx: usize, from: usize, to: usize) -> usize {
    if idx == from {
        to
    } else if from < to && (from + 1..=to).contains(&idx) {
        idx - 1
    } else if to < from && (to..from).contains(&idx) {
        idx + 1
    } else {
        idx
    }
}

impl TrackEdit {
    /// Whether the edit fits a `count`-track axis; `is_col` axes keep at least one track.
    fn fits(self, count: usize, is_col: bool) -> bool {
        match self {
            TrackEdit::Insert { .. } => count < MAX_GRID_TRACKS,
            TrackEdit::Delete { at } => at < count && !(is_col && count <= 1),
            TrackEdit::Move { from, to } => from < count && to < count && from != to,
            TrackEdit::Resize { at, .. } => at < count,
        }
    }

    /// Writes this edit into the layout. Only the slots it changes are touched: a resize sets one slot, a
    /// move turns the slots between its ends, an insert or delete shifts the slots after it. The track
    /// counts change only when a track is added or removed.
    fn write(self, al: &mut AutoLayout, is_col: bool, count: usize) {
        let slots = slots_of(al, is_col);
        match self {
            TrackEdit::Insert { at, track } => {
                slots.insert(at, track);
                trim_slots(slots);
            }
            TrackEdit::Delete { at } => {
                slots.remove(at);
                trim_slots(slots);
            }
            TrackEdit::Move { from, to } if from < to => slots[from..=to].rotate_left(1),
            TrackEdit::Move { from, to } => slots[to..=from].rotate_right(1),
            TrackEdit::Resize { at, track } => slots[at] = track,
        }
        match (self, is_col) {
            (TrackEdit::Insert { .. }, true) => al.grid_columns = count as u32 + 1,
            (TrackEdit::Insert { .. }, false) => al.grid_rows = count as u32 + 1,
            (TrackEdit::Delete { .. }, true) => al.grid_columns = count as u32 - 1,
            // rows past `grid_rows` come from children, so removing one does not shrink the explicit count
            (TrackEdit::Delete { at }, false) if (at as u32) < al.grid_rows => al.grid_rows -= 1,
            _ => {}
        }
    }

    /// Where a child pinned to track `pin` goes (`None`: the pin is dropped). Pins are not limited to the
    /// tracks that exist, so every pin at or after the edit moves, whatever its value.
    fn remap_pin(self, pin: u16) -> Option<u16> {
        match self {
            TrackEdit::Insert { at, .. } if pin as usize >= at => Some(pin.saturating_add(1)),
            TrackEdit::Delete { at } => match (pin as usize).cmp(&at) {
                std::cmp::Ordering::Less => Some(pin),
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Greater => Some(pin - 1),
            },
            TrackEdit::Move { from, to } => Some(remap_reordered_track(pin as usize, from, to) as u16),
            _ => Some(pin),
        }
    }
}

/// Applies `edit` to one axis of the grid frame `grid_id` and re-flows its children in one undo step.
fn edit_grid_tracks(app: &mut App, grid_id: NodeId, is_col: bool, edit: TrackEdit) -> bool {
    let Some(ov) = node_css_grid_overlay(&app.doc, grid_id) else { return false };
    let count = if is_col { ov.cols } else { ov.rows };
    let edit = match edit {
        TrackEdit::Insert { at, track } => TrackEdit::Insert { at: at.min(count), track },
        other => other,
    };
    if !edit.fits(count, is_col) {
        return false;
    }
    if let TrackEdit::Delete { at } = edit {
        if !grid_track_delete_changes_document(&app.doc, grid_id, is_col, at, count) {
            return false;
        }
    }
    let Some(children) = app.doc.get(grid_id).map(|n| n.children.clone()) else { return false };

    let new_count = match edit {
        TrackEdit::Insert { .. } => count + 1,
        TrackEdit::Delete { .. } => count - 1,
        TrackEdit::Move { .. } | TrackEdit::Resize { .. } => count,
    };
    let mut tx = crate::edit::Tx::new();
    if let Some(gn) = tx.ed.node(&mut app.doc, grid_id) {
        if let Some(gal) = gn.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
            edit.write(gal, is_col, count);
        }
    }
    for cid in children {
        let Some(c) = app.doc.get(cid) else { continue };
        let Some(pin) = (if is_col { c.layout_child.grid_column } else { c.layout_child.grid_row }) else { continue };
        let next = edit.remap_pin(pin);
        let old_span = c.layout_child.col_span;
        let mut next_span = old_span;
        if is_col && old_span > 1 {
            let mut span = old_span as usize;
            if let TrackEdit::Delete { at } = edit {
                let start = pin as usize;
                if (start..start.saturating_add(span)).contains(&at) {
                    span = span.saturating_sub(1).max(1);
                }
            }
            if let Some(p) = next {
                let p = p as usize;
                if p < new_count && p.saturating_add(span) > new_count {
                    span = (new_count - p).max(1);
                }
            }
            next_span = span as u16;
        }
        if next != Some(pin) || next_span != old_span {
            if let Some(cn) = tx.ed.node(&mut app.doc, cid) {
                *(if is_col { &mut cn.layout_child.grid_column } else { &mut cn.layout_child.grid_row }) = next;
                if is_col {
                    cn.layout_child.col_span = next_span;
                }
            }
        }
    }
    crate::layout::relayout_around(&mut app.doc, &mut tx.ed, grid_id);
    app.run_tx(tx)
}

/// Whether deleting track `at` of one axis of `grid_id` (which has `count` tracks there) would remove a track or
/// remap a pinned child. A row only the children make (past `grid_rows`, with no child pinned at or after it) is
/// rebuilt by the same children even when it has a custom size, so deleting it would only reset its size to `1fr`
/// instead of removing the row and the inspector must not offer a delete button for it.
pub fn grid_track_delete_changes_document(doc: &Document, grid_id: NodeId, is_col: bool, at: usize, count: usize) -> bool {
    let edit = TrackEdit::Delete { at };
    if !edit.fits(count, is_col) {
        return false;
    }
    let Some(node) = doc.get(grid_id) else { return false };
    let Some(al) = node.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) else { return false };
    if is_col || (at as u32) < al.grid_rows {
        return true;
    }
    node.children.iter().filter_map(|&cid| doc.get(cid)).any(|c| c.layout_child.grid_row.is_some_and(|p| edit.remap_pin(p) != Some(p)))
}

/// Inserts a column (`is_col = true`) or row (`is_col = false`) track at `at` in one undo step, moving
/// the children pinned at or after it. `at` counts tracks as [`grid_track_count`] does.
pub fn insert_grid_track(app: &mut App, grid_id: NodeId, is_col: bool, at: usize, track: GridTrack) -> bool {
    edit_grid_tracks(app, grid_id, is_col, TrackEdit::Insert { at, track })
}

/// Deletes column (`is_col = true`) or row (`is_col = false`) track `at` and re-flows children in one undo step.
pub fn delete_grid_track(app: &mut App, grid_id: NodeId, is_col: bool, at: usize) -> bool {
    edit_grid_tracks(app, grid_id, is_col, TrackEdit::Delete { at })
}

/// Resizes/updates column (`is_col = true`) or row (`is_col = false`) track `idx` in one undo step.
pub fn resize_grid_track(app: &mut App, grid_id: NodeId, is_col: bool, idx: usize, track: GridTrack) -> bool {
    edit_grid_tracks(app, grid_id, is_col, TrackEdit::Resize { at: idx, track })
}

/// Moves column (`is_col = true`) or row (`is_col = false`) track `from` to `to`, carrying anchored items in one undo step.
pub fn move_grid_track(app: &mut App, grid_id: NodeId, is_col: bool, from: usize, to: usize) -> bool {
    edit_grid_tracks(app, grid_id, is_col, TrackEdit::Move { from, to })
}

/// Computes the 2D CSS Grid cell rects, column/row tracks, padding, and gaps for `LayoutDirection::Grid`.
/// `kids` are the flow children as the solver sees them.
pub fn compute_css_grid_overlay(al: &AutoLayout, frame_size: DV, kids: &[ChildIn]) -> CssGridOverlay {
    let [pt, pr, pb, pl] = al.padding;
    let cols = (if al.grid_columns > 0 { al.grid_columns as usize } else { 3 }).clamp(1, designos_doc::MAX_GRID_TRACKS);
    let col_gap = al.item_spacing.max(0.0);
    let row_gap = if al.counter_spacing > 0.0 { al.counter_spacing } else { col_gap };

    // Place the children exactly as the solver does (explicit rows and columns, spans, then auto flow), so the
    // rows and cells drawn here are the ones the layout lays out.
    let child_sizes: Vec<DV> = kids.iter().map(|k| k.size).collect();
    let placements: Vec<(usize, usize, usize, usize)> = place_children(kids, cols).iter().map(|p| (p.row, p.col, p.row_span, p.col_span)).collect();
    let max_row = placements.iter().map(|&(r, _, rs, _)| r + rs).max().unwrap_or(0);

    let default_rows = if child_sizes.is_empty() { 2 } else { 1 };
    let rows = max_row.max(al.grid_rows as usize).max(default_rows).clamp(1, designos_doc::MAX_GRID_TRACKS);

    // Resolve column widths from `grid_col_tracks` + `frame_size.x`.
    let inner_w = (frame_size.x - pl - pr - col_gap * (cols.saturating_sub(1) as f32)).max(0.0);
    let mut col_widths = vec![0.0f32; cols];
    let mut flex_total = 0.0f32;
    let mut fixed_used = 0.0f32;

    for (c, col_w) in col_widths.iter_mut().enumerate() {
        let track = al.grid_col_tracks.get(c).copied().unwrap_or(GridTrack::Flex(1.0));
        match track {
            GridTrack::Fixed(w) => {
                *col_w = w.max(1.0);
                fixed_used += *col_w;
            }
            GridTrack::Hug => {
                let mut max_w = 0.0f32;
                for (i, &(_, col, _, cs)) in placements.iter().enumerate() {
                    if col == c && cs == 1 {
                        max_w = max_w.max(child_sizes[i].x);
                    }
                }
                *col_w = max_w.max(1.0);
                fixed_used += *col_w;
            }
            GridTrack::Flex(fr) => {
                flex_total += fr.max(0.1);
            }
        }
    }
    let rem_w = (inner_w - fixed_used).max(0.0);
    for (c, col_w) in col_widths.iter_mut().enumerate() {
        if let GridTrack::Flex(fr) = al.grid_col_tracks.get(c).copied().unwrap_or(GridTrack::Flex(1.0)) {
            *col_w = if al.primary_sizing == Sizing::Hug && !child_sizes.is_empty() {
                let mut max_w = 0.0f32;
                for (i, &(_, col, _, cs)) in placements.iter().enumerate() {
                    if col == c && cs == 1 {
                        max_w = max_w.max(child_sizes[i].x);
                    }
                }
                max_w.max(40.0)
            } else if flex_total > 0.0 {
                (rem_w * (fr.max(0.1) / flex_total)).max(1.0)
            } else {
                (rem_w / cols as f32).max(1.0)
            };
        }
    }

    // Resolve row heights from child heights and `grid_row_tracks`.
    let inner_h = (frame_size.y - pt - pb - row_gap * (rows.saturating_sub(1) as f32)).max(0.0);
    let mut row_heights = vec![0.0f32; rows];
    for (i, &(r, _, rs, _)) in placements.iter().enumerate() {
        let per_row = (child_sizes[i].y - row_gap * (rs.saturating_sub(1) as f32)).max(0.0) / (rs as f32);
        for dr in 0..rs {
            if r + dr < rows {
                row_heights[r + dr] = row_heights[r + dr].max(per_row);
            }
        }
    }
    let mut row_flex_total = 0.0f32;
    let mut row_fixed_used = 0.0f32;
    for (r, row_h) in row_heights.iter_mut().enumerate() {
        let track = al.grid_row_tracks.get(r).copied().unwrap_or(GridTrack::Flex(1.0));
        match track {
            GridTrack::Fixed(h) => {
                *row_h = row_h.max(h.max(1.0));
                row_fixed_used += *row_h;
            }
            GridTrack::Hug => {
                *row_h = row_h.max(1.0);
                row_fixed_used += *row_h;
            }
            GridTrack::Flex(fr) => {
                if *row_h > 0.0 && al.counter_sizing == Sizing::Hug {
                    row_fixed_used += *row_h;
                } else {
                    row_flex_total += fr.max(0.1);
                }
            }
        }
    }
    let rem_h = (inner_h - row_fixed_used).max(0.0);
    for (r, row_h) in row_heights.iter_mut().enumerate() {
        if let GridTrack::Flex(fr) = al.grid_row_tracks.get(r).copied().unwrap_or(GridTrack::Flex(1.0)) {
            if al.counter_sizing != Sizing::Hug || *row_h <= 0.0 {
                let share = if row_flex_total > 0.0 { rem_h * (fr.max(0.1) / row_flex_total) } else { rem_h / rows as f32 };
                *row_h = row_h.max(share).max(1.0);
            }
        }
    }

    let mut col_x = vec![0.0f32; cols];
    let mut cx = pl;
    for (x, &w) in col_x.iter_mut().zip(&col_widths) {
        *x = cx;
        cx += w + col_gap;
    }

    let mut row_y = vec![0.0f32; rows];
    let mut cy = pt;
    for (y, &h) in row_y.iter_mut().zip(&row_heights) {
        *y = cy;
        cy += h + row_gap;
    }

    let mut cells = Vec::with_capacity(rows * cols);
    for (r, (&y, &h)) in row_y.iter().zip(&row_heights).enumerate() {
        for (c, (&x, &w)) in col_x.iter().zip(&col_widths).enumerate() {
            cells.push(CssGridCell { row: r, col: c, rect: Rect::new(x, y, w, h) });
        }
    }

    let col_tracks = (0..cols).map(|c| al.grid_col_tracks.get(c).copied().unwrap_or_default()).collect();
    let row_tracks = (0..rows).map(|r| al.grid_row_tracks.get(r).copied().unwrap_or_default()).collect();

    CssGridOverlay { cols, rows, col_x, col_widths, row_y, row_heights, col_gap, row_gap, padding: al.padding, cells, col_tracks, row_tracks }
}

/// Returns the 2D CSS Grid overlay geometry if `id` is a container with `LayoutDirection::Grid`.
pub fn node_css_grid_overlay(doc: &Document, id: NodeId) -> Option<CssGridOverlay> {
    let n = doc.get(id)?;
    let al = n.kind.frame_props()?.auto_layout.as_ref()?;
    if al.direction != LayoutDirection::Grid {
        return None;
    }
    let kids: Vec<ChildIn> = n.children.iter().filter_map(|&cid| doc.flow_child_input(cid, true)).collect();
    Some(compute_css_grid_overlay(al, n.size, &kids))
}

/// Start and length of each stripe of a columns / rows grid along an extent.
/// `count` stripes separated by `gutter`, inset by `offset` on both ends. A negative `gutter`
/// overlaps neighbouring stripes and a negative `offset` pushes them past the extent: imported
/// files carry both, so only non-finite numbers and a non-positive stripe width draw nothing.
pub fn stripes(count: u32, gutter: f32, offset: f32, extent: f32) -> Vec<(f32, f32)> {
    if count == 0 || !extent.is_finite() || extent <= 0.0 || !gutter.is_finite() || !offset.is_finite() {
        return vec![];
    }
    let avail = extent - 2.0 * offset;
    let w = (avail - gutter * (count - 1) as f32) / count as f32;
    if w.is_nan() || w <= 0.0 || !w.is_finite() {
        return vec![];
    }
    (0..count).map(|i| (offset + i as f32 * (w + gutter), w)).collect()
}

use designos_doc::LayoutGridAlign;

/// Most stripes an auto-count guide builds; beyond this a stripe is thinner than a pixel at any
/// useful zoom and the overlay is rebuilt every frame.
const MAX_AUTO_STRIPES: u32 = 4096;

/// Start and length of each stripe of a columns / rows guide with `alignment`, fixed `section_size`,
/// and `count == 0` meaning auto-count (`(avail + gutter) / (section_size + gutter)`). Stripes are
/// clipped to `0..extent`: a guide never paints outside its frame.
pub fn stripes_aligned(count: u32, gutter: f32, offset: f32, section_size: f32, alignment: LayoutGridAlign, extent: f32) -> Vec<(f32, f32)> {
    if !extent.is_finite() || extent <= 0.0 || !gutter.is_finite() || !offset.is_finite() {
        return vec![];
    }
    let n = if count > 0 {
        count
    } else {
        if !section_size.is_finite() || section_size <= 0.0 {
            return vec![];
        }
        let avail = (extent - 2.0 * offset).max(0.0);
        let step = section_size + gutter;
        if step <= 0.0 {
            return vec![];
        }
        // capped as a float: a tiny width would saturate the cast to u32::MAX
        ((avail + gutter) / step).floor().clamp(0.0, MAX_AUTO_STRIPES as f32) as u32
    };
    if n == 0 {
        return vec![];
    }
    let all = match alignment {
        LayoutGridAlign::Stretch => stripes(n, gutter, offset, extent),
        LayoutGridAlign::Min | LayoutGridAlign::Center | LayoutGridAlign::Max => {
            let w = if section_size.is_finite() && section_size > 0.0 {
                section_size
            } else {
                // equal widths that fill the extent; gutters or margins past it leave no width
                let avail = extent - 2.0 * offset;
                (avail - gutter * (n - 1) as f32) / n as f32
            };
            if w.is_nan() || w <= 0.0 || !w.is_finite() {
                return vec![];
            }
            let total = n as f32 * w + (n - 1) as f32 * gutter;
            let start0 = match alignment {
                LayoutGridAlign::Min => offset,
                LayoutGridAlign::Center => (extent - total) * 0.5,
                LayoutGridAlign::Max => extent - offset - total,
                LayoutGridAlign::Stretch => offset,
            };
            (0..n).map(|i| (start0 + i as f32 * (w + gutter), w)).collect()
        }
    };
    clip_to_extent(all, extent)
}

/// The part of each `(start, length)` stripe inside `0..extent`; stripes outside it are dropped.
fn clip_to_extent(stripes: Vec<(f32, f32)>, extent: f32) -> Vec<(f32, f32)> {
    stripes
        .into_iter()
        .filter_map(|(start, len)| {
            if start >= 0.0 && start + len <= extent {
                return Some((start, len));
            }
            let (s, e) = (start.max(0.0), (start + len).min(extent));
            (e > s).then_some((s, e - s))
        })
        .collect()
}

/// Positions of the lines of a square grid with cells of `size` inside `extent`.
pub fn grid_lines(size: f32, extent: f32) -> Vec<f32> {
    if size.is_nan() || size < 1.0 || !extent.is_finite() {
        return vec![];
    }
    let n = (extent / size).floor() as usize;
    (1..=n).map(|i| i as f32 * size).filter(|v| *v <= extent).collect()
}

fn c32(c: &Color) -> Color32 {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(f(c.r), f(c.g), f(c.b), f(c.a))
}

pub fn paint(app: &App, ui: &Ui, rect: ERect) {
    let p = ui.painter().with_clip_rect(rect);
    if app.ui.pixel_grid && app.view.zoom >= theme::PIXEL_GRID_MIN_ZOOM {
        pixel_grid(app, &p, rect);
    }
    if !app.ui.hide_layout_grids {
        layout_grids(app, &p, rect);
    }
}

fn page_is_light(app: &App) -> bool {
    match app.doc.get(app.page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => {
            let f = |v: f32| (v * 255.0) as u8;
            theme::is_light(Color32::from_rgb(f(background.r), f(background.g), f(background.b)))
        }
        _ => false,
    }
}

fn pixel_grid(app: &App, p: &Painter, rect: ERect) {
    let col = if page_is_light(app) { theme::PIXEL_GRID_DARK_LINES } else { theme::PIXEL_GRID_LIGHT_LINES };
    let stroke = Stroke::new(1.0, col);
    let tl = app.view.to_page(rect.min, rect.min);
    let br = app.view.to_page(rect.min, rect.max);
    let (x0, x1) = (tl.x.floor() as i64, br.x.ceil() as i64);
    let (y0, y1) = (tl.y.floor() as i64, br.y.ceil() as i64);
    for x in x0..=x1 {
        let sx = app.view.to_screen(rect.min, v2(x as f32, 0.0)).x;
        p.line_segment([Pos2::new(sx, rect.top()), Pos2::new(sx, rect.bottom())], stroke);
    }
    for y in y0..=y1 {
        let sy = app.view.to_screen(rect.min, v2(0.0, y as f32)).y;
        p.line_segment([Pos2::new(rect.left(), sy), Pos2::new(rect.right(), sy)], stroke);
    }
}

fn layout_grids(app: &App, p: &Painter, rect: ERect) {
    layout_grids_in(app, p, rect, super::interact::active_index(app));
}

/// Paints the layout grids of the visible frames, taking them from `idx` when there is one.
fn layout_grids_in(app: &App, p: &Painter, rect: ERect, idx: Option<&designos_render::SceneIndex>) {
    let z = app.view.zoom.max(1e-3);
    let view = Rect::new(app.view.offset.x, app.view.offset.y, rect.width() / z, rect.height() / z);
    let win = (view.x, view.y, view.x + view.w, view.y + view.h);
    let overlaps_win = |(x0, y0, x1, y1): (f32, f32, f32, f32)| x0 <= win.2 && win.0 <= x1 && y0 <= win.3 && win.1 <= y1;
    let Some(page) = app.doc.get(app.page) else { return };

    // Painted back to front: the stack pops from its end, so everything is pushed in reverse.
    let mut stack: Vec<(NodeId, Affine)> = Vec::new();
    if let Some(index) = idx {
        for &id in index.grid_nodes().iter().rev() {
            if index.bounds_of(id).is_none_or(overlaps_win) {
                stack.push((id, super::fast_world_transform(&app.doc, id)));
            }
        }
    } else {
        for &cid in page.children.iter().rev() {
            stack.push((cid, Affine::IDENTITY));
        }
    }

    while let Some((id, parent_world)) = stack.pop() {
        let Some(n) = app.doc.get(id) else { continue };
        if !n.visible {
            continue;
        }
        if let Some(b) = idx.and_then(|i| i.bounds_of(id)) {
            if !overlaps_win(b) {
                continue;
            }
        }
        let w = if idx.is_some() { parent_world } else { parent_world * n.transform };
        let own_bb = super::affine_rect_aabb(&w, n.size);
        if idx.is_none() && !n.children.is_empty() {
            let descend = if n.kind.frame_props().is_some_and(|f| f.clips_content) { own_bb.intersects(&view) } else { true };
            if descend {
                for &cid in n.children_front_to_back() {
                    stack.push((cid, w));
                }
            }
        }
        let Some(fp) = n.kind.frame_props() else { continue };
        if fp.grids.is_empty() || !own_bb.intersects(&view) {
            continue;
        }
        let to_s = |x: f32, y: f32| app.view.to_screen(rect.min, w.apply(v2(x, y)));
        let quad = |x0: f32, y0: f32, x1: f32, y1: f32, col: Color32| {
            p.add(Shape::convex_polygon(vec![to_s(x0, y0), to_s(x1, y0), to_s(x1, y1), to_s(x0, y1)], col, Stroke::NONE));
        };
        for g in &fp.grids {
            if !g.is_visible() {
                continue;
            }
            match g {
                LayoutGrid::Grid { size, color, .. } => {
                    // too dense to read: skip below 4 screen px per cell
                    if *size * z < 4.0 {
                        continue;
                    }
                    let stroke = Stroke::new(1.0, c32(color));
                    for x in grid_lines(*size, n.size.x) {
                        p.line_segment([to_s(x, 0.0), to_s(x, n.size.y)], stroke);
                    }
                    for y in grid_lines(*size, n.size.y) {
                        p.line_segment([to_s(0.0, y), to_s(n.size.x, y)], stroke);
                    }
                }
                LayoutGrid::Columns { count, gutter, offset, section_size, alignment, color, .. } => {
                    for (x, cw) in stripes_aligned(*count, *gutter, *offset, *section_size, *alignment, n.size.x) {
                        quad(x, 0.0, x + cw, n.size.y, c32(color));
                    }
                }
                LayoutGrid::Rows { count, gutter, offset, section_size, alignment, color, .. } => {
                    for (y, rh) in stripes_aligned(*count, *gutter, *offset, *section_size, *alignment, n.size.y) {
                        quad(0.0, y, n.size.x, y + rh, c32(color));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_stripes_fill_the_extent() {
        // 4 columns, 20px gutters, 10px margins in 400px: (380 - 60) / 4 = 80 each
        let s = stripes(4, 20.0, 10.0, 400.0);
        assert_eq!(s, vec![(10.0, 80.0), (110.0, 80.0), (210.0, 80.0), (310.0, 80.0)]);
        let last = s.last().unwrap();
        assert_eq!(last.0 + last.1, 390.0);
    }

    #[test]
    fn degenerate_stripes_are_empty() {
        assert!(stripes(0, 10.0, 0.0, 100.0).is_empty());
        assert!(stripes(10, 50.0, 0.0, 100.0).is_empty(), "gutters wider than the extent");
        assert!(stripes(2, 0.0, 60.0, 100.0).is_empty(), "margins eat everything");
    }

    #[test]
    fn negative_gutter_overlaps_columns_and_negative_margin_extends_them() {
        use designos_doc::LayoutGridAlign::Stretch;
        // imported files keep the raw numbers: a negative gutter overlaps neighbouring columns
        assert_eq!(stripes(2, -10.0, 0.0, 100.0), vec![(0.0, 55.0), (45.0, 55.0)]);
        assert_eq!(stripes_aligned(2, -10.0, 0.0, 0.0, Stretch, 100.0), vec![(0.0, 55.0), (45.0, 55.0)]);
        // a negative margin grows the columns past the frame edges; the part outside the frame is clipped
        assert_eq!(stripes(2, 0.0, -10.0, 100.0), vec![(-10.0, 60.0), (50.0, 60.0)]);
        assert_eq!(stripes_aligned(2, 0.0, -10.0, 0.0, Stretch, 100.0), vec![(0.0, 50.0), (50.0, 50.0)]);
        // a stretch guide whose computed width is not positive still draws nothing
        assert!(stripes_aligned(3, 100.0, 0.0, 0.0, Stretch, 100.0).is_empty());
        // non-finite numbers never draw
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(stripes(2, bad, 0.0, 100.0).is_empty(), "gutter {bad}");
            assert!(stripes(2, 0.0, bad, 100.0).is_empty(), "offset {bad}");
            assert!(stripes_aligned(2, bad, 0.0, 0.0, Stretch, 100.0).is_empty(), "aligned gutter {bad}");
            assert!(stripes_aligned(2, 0.0, bad, 0.0, Stretch, 100.0).is_empty(), "aligned offset {bad}");
        }
    }

    #[test]
    fn aligned_stripes_are_clipped_to_the_frame() {
        use designos_doc::LayoutGridAlign::{Center, Max, Min};
        // a 300px column on a 100px frame shows the 100px that overlap it, whichever side it is aligned to
        for a in [Min, Center, Max] {
            assert_eq!(stripes_aligned(1, 0.0, 0.0, 300.0, a, 100.0), vec![(0.0, 100.0)], "{a:?}");
        }
        // the second column is cut by the frame edge, the third lies outside and is dropped
        assert_eq!(stripes_aligned(3, 20.0, 0.0, 60.0, Min, 100.0), vec![(0.0, 60.0), (80.0, 20.0)]);
        assert_eq!(stripes_aligned(3, 20.0, 0.0, 60.0, Max, 100.0), vec![(0.0, 20.0), (40.0, 60.0)]);
        // inside the frame nothing changes
        assert_eq!(stripes_aligned(2, 10.0, 5.0, 20.0, Min, 100.0), vec![(5.0, 20.0), (35.0, 20.0)]);
    }

    #[test]
    fn aligned_stripes_without_a_fixed_width_need_room_for_the_gutters() {
        use designos_doc::LayoutGridAlign::{Center, Max, Min};
        for a in [Min, Center, Max] {
            // gutters wider than the frame leave no width, so there is nothing to draw (not 1px slivers)
            assert!(stripes_aligned(3, 200.0, 0.0, 0.0, a, 100.0).is_empty(), "{a:?}");
            assert!(stripes_aligned(2, 0.0, 60.0, 0.0, a, 100.0).is_empty(), "margins eat everything {a:?}");
        }
        // equal widths when there is room: (400 - 80) / 5
        assert_eq!(stripes_aligned(5, 20.0, 0.0, 0.0, Min, 400.0)[0], (0.0, 64.0));
    }

    #[test]
    fn auto_count_is_capped() {
        // a tiny width would otherwise build ten million stripes on every frame
        let s = stripes_aligned(0, 0.0, 0.0, 0.001, designos_doc::LayoutGridAlign::Min, 10000.0);
        assert!(!s.is_empty() && s.len() <= MAX_AUTO_STRIPES as usize, "{} stripes", s.len());
        // a width so small that the count overflows u32 is capped too, and stretches to the full extent
        let s = stripes_aligned(0, 0.0, 0.0, 1e-30, designos_doc::LayoutGridAlign::Min, 10000.0);
        assert!(s.len() <= MAX_AUTO_STRIPES as usize, "{} stripes", s.len());
        let s = stripes_aligned(0, 0.0, 0.0, 1e-30, designos_doc::LayoutGridAlign::Stretch, 10000.0);
        assert_eq!(s.len(), MAX_AUTO_STRIPES as usize);
        assert!((s[0].1 * MAX_AUTO_STRIPES as f32 - 10000.0).abs() < 1.0);
        // ordinary auto counts are untouched
        assert_eq!(stripes_aligned(0, 20.0, 20.0, 60.0, designos_doc::LayoutGridAlign::Center, 400.0).len(), 4);
    }

    #[test]
    fn square_grid_lines() {
        assert_eq!(grid_lines(10.0, 35.0), vec![10.0, 20.0, 30.0]);
        assert!(grid_lines(0.0, 100.0).is_empty());
        assert!(grid_lines(10.0, 5.0).is_empty());
    }

    #[test]
    fn css_grid_overlay_calculates_2d_cells_and_tracks() {
        let mut tracks = [GridTrack::Flex(1.0); 8];
        tracks[0] = GridTrack::Fixed(120.0);
        tracks[1] = GridTrack::Flex(1.0);
        tracks[2] = GridTrack::Flex(2.0);
        let al = AutoLayout {
            direction: LayoutDirection::Grid,
            grid_columns: 3,
            grid_rows: 2,
            item_spacing: 16.0,
            counter_spacing: 12.0,
            padding: [20.0, 24.0, 20.0, 24.0],
            grid_col_tracks: tracks.into(),
            ..AutoLayout::default()
        };
        // Frame width = 24 + 120 + 16 + 100 + 16 + 200 + 24 = 500
        let child_sizes = [v2(120.0, 80.0), v2(100.0, 80.0), v2(200.0, 80.0), v2(200.0, 100.0)];
        let spans = [(1, 1), (1, 1), (1, 1), (2, 1)];
        let kids: Vec<ChildIn> = child_sizes
            .iter()
            .zip(spans)
            .map(|(&size, (col_span, row_span))| ChildIn {
                size,
                cell: designos_doc::layout_solver::GridCell { col_span, row_span, ..Default::default() },
                ..Default::default()
            })
            .collect();
        let ov = compute_css_grid_overlay(&al, v2(500.0, 232.0), &kids);
        assert_eq!(ov.cols, 3);
        assert_eq!(ov.rows, 2);
        assert_eq!(ov.cells.len(), 6);
        assert!((ov.col_widths[0] - 120.0).abs() < 1e-3);
        assert!((ov.col_widths[1] - 100.0).abs() < 1e-3);
        assert!((ov.col_widths[2] - 200.0).abs() < 1e-3);
        assert!((ov.col_x[0] - 24.0).abs() < 1e-3);
        assert!((ov.col_x[1] - 160.0).abs() < 1e-3);
        assert!((ov.col_x[2] - 276.0).abs() < 1e-3);
        assert_eq!(format_grid_track(ov.col_tracks[0]), "120px");
        assert_eq!(format_grid_track(ov.col_tracks[1]), "1fr");
        assert_eq!(format_grid_track(ov.col_tracks[2]), "2fr");
        assert_eq!(format_grid_track(GridTrack::Hug), "Hug");
    }

    fn gridded_frame(app: &mut App, parent: NodeId, rect: Rect, color: Color) -> NodeId {
        let id = app.doc.alloc_id();
        let mut frame = designos_doc::Node::frame(id, rect);
        if let NodeKind::Frame(fp) = &mut frame.kind {
            fp.grids.push(LayoutGrid::columns(1, 0.0, 0.0, color));
        }
        app.doc.insert(parent, frame).unwrap()
    }

    /// Fill colours of the stripes painted for the layout grids, in paint order.
    fn painted_stripe_fills(app: &App, idx: Option<&designos_render::SceneIndex>) -> Vec<Color32> {
        let ctx = egui::Context::default();
        let rect = ERect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 600.0));
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter().clone();
            layout_grids_in(app, &painter, rect, idx);
        });
        out.shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Path(path) => Some(path.fill),
                _ => None,
            })
            .collect()
    }

    /// Screen-space bounds of every stripe painted for the layout grids, in paint order.
    fn painted_stripe_bounds(app: &App) -> Vec<(Color32, ERect)> {
        let ctx = egui::Context::default();
        let rect = ERect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 600.0));
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter().clone();
            layout_grids_in(app, &painter, rect, None);
        });
        out.shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Path(path) => Some((path.fill, ERect::from_points(&path.points))),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_column_wider_than_its_frame_does_not_paint_over_the_next_frame() {
        use designos_doc::LayoutGridAlign;
        let mut doc = Document::new("grid-clip");
        doc.add_page("page-1");
        let mut app = App::new(doc);
        let page = app.page;
        let wide = Color::rgba(255, 0, 0, 40);
        let id = app.doc.alloc_id();
        let mut frame = designos_doc::Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        if let NodeKind::Frame(fp) = &mut frame.kind {
            fp.grids.push(LayoutGrid::Columns {
                count: 1,
                gutter: 0.0,
                offset: 0.0,
                section_size: 300.0,
                alignment: LayoutGridAlign::Min,
                color: wide,
                visible: true,
            });
        }
        app.doc.insert(page, frame).unwrap();
        // a sibling frame the 300px column would reach
        gridded_frame(&mut app, page, Rect::new(150.0, 0.0, 100.0, 100.0), Color::rgba(0, 0, 255, 40));

        let right_edge = app.view.to_screen(Pos2::ZERO, v2(100.0, 0.0)).x;
        let boxes = painted_stripe_bounds(&app);
        assert_eq!(boxes.len(), 2);
        let (fill, bounds) = boxes[0];
        assert_eq!(fill, c32(&wide));
        assert!(bounds.max.x <= right_edge + 0.01, "the wide column paints to x = {} but its frame ends at {}", bounds.max.x, right_edge);
    }

    #[test]
    fn layout_grids_paint_parents_beneath_children_and_lower_siblings_beneath_upper_ones() {
        let mut doc = Document::new("grid-paint-order");
        doc.add_page("page-1");
        let mut app = App::new(doc);
        let page = app.page;
        let parent_color = Color::rgba(255, 0, 0, 40);
        let child_color = Color::rgba(0, 0, 255, 40);
        let lower_color = Color::rgba(0, 255, 0, 40);
        let upper_color = Color::rgba(255, 255, 0, 40);
        let parent = gridded_frame(&mut app, page, Rect::new(0.0, 0.0, 400.0, 300.0), parent_color);
        gridded_frame(&mut app, parent, Rect::new(20.0, 20.0, 200.0, 200.0), child_color);
        gridded_frame(&mut app, page, Rect::new(600.0, 0.0, 200.0, 200.0), lower_color);
        gridded_frame(&mut app, page, Rect::new(600.0, 0.0, 200.0, 200.0), upper_color);

        let expected = [parent_color, child_color, lower_color, upper_color].map(|c| c32(&c)).to_vec();

        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&app.doc, app.page, 1);
        assert_eq!(painted_stripe_fills(&app, Some(&idx)), expected, "with a scene index");
        assert_eq!(painted_stripe_fills(&app, None), expected, "without a scene index");
    }

    #[test]
    fn centred_12x60_columns_draw_at_literal_x_and_hidden_guides_do_not_draw() {
        use designos_doc::LayoutGridAlign;
        let cols = stripes_aligned(12, 20.0, 0.0, 60.0, LayoutGridAlign::Center, 1440.0);
        let expected_cols: Vec<(f32, f32)> = (0..12).map(|i| (250.0 + i as f32 * 80.0, 60.0)).collect();
        assert_eq!(cols, expected_cols);

        let auto_cols = stripes_aligned(0, 20.0, 20.0, 60.0, LayoutGridAlign::Center, 400.0);
        assert_eq!(auto_cols, vec![(50.0, 60.0), (130.0, 60.0), (210.0, 60.0), (290.0, 60.0)]);

        let mut doc = Document::new("grid-parity");
        doc.add_page("page-1");
        let mut app = App::new(doc);
        let page = app.page;
        let id = app.doc.alloc_id();
        let mut frame = designos_doc::Node::frame(id, Rect::new(0.0, 0.0, 1440.0, 400.0));
        let vis_col = Color::rgba(255, 0, 0, 40);
        let hid_col = Color::rgba(0, 255, 0, 40);
        if let NodeKind::Frame(fp) = &mut frame.kind {
            fp.grids.push(LayoutGrid::Columns {
                count: 12,
                gutter: 20.0,
                offset: 0.0,
                section_size: 60.0,
                alignment: LayoutGridAlign::Center,
                color: vis_col,
                visible: true,
            });
            fp.grids.push(LayoutGrid::Rows {
                count: 4,
                gutter: 16.0,
                offset: 0.0,
                section_size: 40.0,
                alignment: LayoutGridAlign::Min,
                color: hid_col,
                visible: false,
            });
        }
        app.doc.insert(page, frame).unwrap();
        let fills = painted_stripe_fills(&app, None);
        assert_eq!(fills.len(), 12, "only the 12 visible column stripes should paint");
        assert!(fills.iter().all(|c| *c == c32(&vis_col)));
    }
}
