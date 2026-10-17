//! Canvas guides drawn over the page: the pixel grid (from 800% zoom) and the
//! layout grids (`FrameProps::grids`) of frames.

use super::*;
use crate::theme;
use crate::util::*;
use designos_doc::{AutoLayout, Color, Document, GridTrack, LayoutDirection, LayoutGrid, NodeKind, Sizing};
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

/// Computes the 2D CSS Grid cell rects, column/row tracks, padding, and gaps for `LayoutDirection::Grid`.
pub fn compute_css_grid_overlay(al: &AutoLayout, frame_size: DV, child_sizes: &[DV], spans: &[(u16, u16)]) -> CssGridOverlay {
    let [pt, pr, pb, pl] = al.padding;
    let cols = (if al.grid_columns > 0 { al.grid_columns as usize } else { 3 }).clamp(1, 16);
    let col_gap = al.item_spacing.max(0.0);
    let row_gap = if al.counter_spacing > 0.0 { al.counter_spacing } else { col_gap };

    // Place children into a 2D occupancy grid respecting col_span and row_span.
    let mut occupied: Vec<Vec<bool>> = Vec::new();
    let mut placements: Vec<(usize, usize, usize, usize)> = Vec::with_capacity(child_sizes.len());
    let mut max_row = 0usize;

    for idx in 0..child_sizes.len() {
        let (cs, rs) = spans.get(idx).copied().unwrap_or((1, 1));
        let c_span = (cs.max(1) as usize).min(cols);
        let r_span = (rs.max(1) as usize).min(16);
        let mut r = 0usize;
        let placed = loop {
            while occupied.len() < r + r_span {
                occupied.push(vec![false; cols]);
            }
            let mut found_c = None;
            for c in 0..=(cols - c_span) {
                let mut fits = true;
                for dr in 0..r_span {
                    for dc in 0..c_span {
                        if occupied[r + dr][c + dc] {
                            fits = false;
                            break;
                        }
                    }
                    if !fits {
                        break;
                    }
                }
                if fits {
                    found_c = Some(c);
                    break;
                }
            }
            if let Some(c) = found_c {
                for dr in 0..r_span {
                    for dc in 0..c_span {
                        occupied[r + dr][c + dc] = true;
                    }
                }
                break (r, c, r_span, c_span);
            }
            r += 1;
        };
        max_row = max_row.max(placed.0 + placed.2);
        placements.push(placed);
    }

    let default_rows = if child_sizes.is_empty() { 2 } else { 1 };
    let rows = max_row.max(al.grid_rows as usize).max(default_rows).clamp(1, 16);

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
    let mut child_sizes = Vec::new();
    let mut spans = Vec::new();
    for &cid in &n.children {
        let Some(c) = doc.get(cid) else { continue };
        if !c.visible || c.layout_child.absolute {
            continue;
        }
        child_sizes.push(c.size);
        spans.push((c.layout_child.col_span.max(1), c.layout_child.row_span.max(1)));
    }
    Some(compute_css_grid_overlay(al, n.size, &child_sizes, &spans))
}

/// Start and length of each stripe of a columns / rows grid along an extent.
/// `count` stripes separated by `gutter`, inset by `offset` on both ends.
pub fn stripes(count: u32, gutter: f32, offset: f32, extent: f32) -> Vec<(f32, f32)> {
    if count == 0 {
        return vec![];
    }
    let avail = extent - 2.0 * offset;
    let w = (avail - gutter * (count - 1) as f32) / count as f32;
    if w.is_nan() || w <= 0.0 || !w.is_finite() {
        return vec![];
    }
    (0..count).map(|i| (offset + i as f32 * (w + gutter), w)).collect()
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
                for &cid in n.children.iter().rev() {
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
            match g {
                LayoutGrid::Grid { size, color } => {
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
                LayoutGrid::Columns { count, gutter, offset, color } => {
                    for (x, cw) in stripes(*count, *gutter, *offset, n.size.x) {
                        quad(x, 0.0, x + cw, n.size.y, c32(color));
                    }
                }
                LayoutGrid::Rows { count, gutter, offset, color } => {
                    for (y, rh) in stripes(*count, *gutter, *offset, n.size.y) {
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
            grid_col_tracks: tracks,
            ..AutoLayout::default()
        };
        // Frame width = 24 + 120 + 16 + 100 + 16 + 200 + 24 = 500
        let child_sizes = [v2(120.0, 80.0), v2(100.0, 80.0), v2(200.0, 80.0), v2(200.0, 100.0)];
        let spans = [(1, 1), (1, 1), (1, 1), (2, 1)];
        let ov = compute_css_grid_overlay(&al, v2(500.0, 232.0), &child_sizes, &spans);
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
            fp.grids.push(LayoutGrid::Columns { count: 1, gutter: 0.0, offset: 0.0, color });
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
}
