//! Pointer/keyboard interaction on the canvas: tool state machine.

use super::builder;
use super::create::{self, PenPoint};
use super::handles::{self, Handle, HandleHit};
use super::hit;
use super::reorder;
use super::snap;
use super::textsession as ts;
pub use super::textsession::{begin_text_edit, finish_text};
use super::tools::{press_action, PressAction, PressTarget, Tool};
use super::vector;
use super::*;
use crate::edit::Tx;
use crate::layout;
use crate::ops::apply_resize;
use crate::theme;
use crate::util::*;
use designos_doc::{Command, Document, NodeKind};
use egui::{Modifiers, Pos2, Response};

pub fn dv(p: Pos2) -> DV {
    DV::new(p.x, p.y)
}

struct Inp {
    pos: Option<Pos2>,
    pressed: bool,
    down: bool,
    released: bool,
    mid_pressed: bool,
    mid_down: bool,
    mods: Modifiers,
    space: bool,
    z_down: bool,
}

fn gather(ui: &Ui, typing: bool) -> Inp {
    ui.input(|i| Inp {
        pos: i.pointer.latest_pos(),
        pressed: i.pointer.primary_pressed(),
        down: i.pointer.primary_down(),
        released: i.pointer.primary_released(),
        mid_pressed: i.pointer.button_pressed(egui::PointerButton::Middle),
        mid_down: i.pointer.middle_down(),
        mods: i.modifiers,
        space: !typing && i.key_down(egui::Key::Space),
        z_down: !typing && i.key_down(egui::Key::Z),
    })
}

/// Corners (page space) of the box the selection handles wrap, plus whether
/// it is a single straight line (only end handles).
pub fn selection_box(app: &App) -> Option<([DV; 4], bool)> {
    match app.selection.as_slice() {
        [] => None,
        [one] => {
            let n = app.doc.get(*one)?;
            Some((super::fast_world_corners(&app.doc, *one), matches!(n.kind, NodeKind::Line)))
        }
        many => {
            let idx = active_index(app);
            let bb = union_all(many.iter().take(512).map(|i| super::fast_world_aabb(&app.doc, idx, *i)))?;
            Some((rect_corners(&bb), false))
        }
    }
}

/// Containers that may be descended into on click (ancestors of the selection).
fn entered(app: &App) -> Vec<NodeId> {
    let mut v = Vec::new();
    for id in app.selection.iter().take(64) {
        let mut cur = app.doc.get(*id).and_then(|n| n.parent);
        while let Some(p) = cur {
            if !v.contains(&p) {
                v.push(p);
            }
            cur = app.doc.get(p).and_then(|n| n.parent);
        }
    }
    v
}

pub(crate) fn active_index(app: &App) -> Option<&designos_render::SceneIndex> {
    let gpu_idx = app.canvas.gpu.scene_index();
    let tile_idx = app.canvas.tiles.scene_index();
    let first_vis = app.doc.get(app.page).and_then(|p| p.children.iter().find(|&&cid| app.doc.get(cid).is_some_and(|n| n.visible)).copied());
    if app.canvas.gpu.active() && first_vis.is_none_or(|cid| gpu_idx.bounds_of(cid).is_some()) {
        return Some(gpu_idx);
    }
    if first_vis.is_some_and(|cid| gpu_idx.bounds_of(cid).is_some()) {
        return Some(gpu_idx);
    }
    if first_vis.is_some_and(|cid| tile_idx.bounds_of(cid).is_some()) {
        return Some(tile_idx);
    }
    if !app.canvas.dirty {
        Some(if app.canvas.gpu.active() { gpu_idx } else { tile_idx })
    } else {
        None
    }
}

pub fn hit_under(app: &App, page: DV, deep: bool) -> Option<NodeId> {
    let tol = 2.0 / app.view.zoom;
    hit::hit_test_with_index(&app.doc, app.page, page, tol, deep, &entered(app), active_index(app))
}

/// Rects of snap candidates (visible nodes near the viewport/moving selection except `exclude` subtrees, capped to 128).
pub fn snap_targets(app: &App, exclude: &[NodeId]) -> Vec<Rect> {
    const MAX_SNAP_CANDIDATES: usize = 128;
    const SNAP_RADIUS_PAGE: f32 = 2000.0;

    let z = app.view.zoom.max(1e-3);
    let cr = app.canvas_rect;
    let view = Rect::new(app.view.offset.x, app.view.offset.y, cr.width() / z, cr.height() / z);
    let view = Rect::new(view.x - view.w * 0.5, view.y - view.h * 0.5, view.w * 2.0, view.h * 2.0);
    let idx = active_index(app).or_else(|| {
        let g = app.canvas.gpu.scene_index();
        app.doc.get(app.page).and_then(|p| p.children.first()).and_then(|&cid| g.bounds_of(cid)).map(|_| g)
    });

    let moving_bb = if exclude.is_empty() { None } else { union_all(exclude.iter().map(|&id| super::fast_world_aabb(&app.doc, idx, id))) };
    let search_rect = moving_bb
        .map(|b| Rect::new(b.x - SNAP_RADIUS_PAGE, b.y - SNAP_RADIUS_PAGE, b.w + SNAP_RADIUS_PAGE * 2.0, b.h + SNAP_RADIUS_PAGE * 2.0))
        .unwrap_or(view);

    let mut out = Vec::with_capacity(MAX_SNAP_CANDIDATES + 8);
    let mut visited_parents = Vec::new();

    // 1. Direct siblings of the excluded/moving nodes are always highest-priority snap targets.
    for &ex in exclude {
        let Some(parent_id) = app.doc.get(ex).and_then(|n| n.parent) else { continue };
        if visited_parents.contains(&parent_id) {
            continue;
        }
        visited_parents.push(parent_id);
        let Some(pn) = app.doc.get(parent_id) else { continue };
        let parent_world = super::fast_world_transform(&app.doc, parent_id);
        for &sib in &pn.children {
            if exclude.contains(&sib) || out.len() >= MAX_SNAP_CANDIDATES * 2 {
                continue;
            }
            let Some(sn) = app.doc.get(sib) else { continue };
            if !sn.visible {
                continue;
            }
            let bb = super::affine_rect_aabb(&(parent_world * sn.transform), sn.size);
            if bb.intersects(&view) && bb.intersects(&search_rect) {
                out.push(bb);
            }
        }
    }

    // 2. Top-level and shallow visible containers overlapping the search window.
    if let Some(page) = app.doc.get(app.page) {
        let win = (
            view.x.max(search_rect.x),
            view.y.max(search_rect.y),
            (view.x + view.w).min(search_rect.x + search_rect.w),
            (view.y + view.h).min(search_rect.y + search_rect.h),
        );
        let mut top_ids: Vec<NodeId> = Vec::new();
        if let Some(index) = idx {
            if page.children.len() > 32 && win.0 <= win.2 && win.1 <= win.3 {
                index.query_top_level(win, &mut top_ids);
            } else {
                top_ids.extend_from_slice(&page.children);
            }
        } else {
            top_ids.extend_from_slice(&page.children);
        }

        for cid in top_ids {
            if exclude.contains(&cid) || out.len() >= MAX_SNAP_CANDIDATES * 2 {
                continue;
            }
            let Some(cn) = app.doc.get(cid) else { continue };
            if !cn.visible {
                continue;
            }
            let world = cn.transform;
            let bb = idx
                .and_then(|i| i.bounds_of(cid))
                .filter(|_| cn.children.is_empty() && cn.effects.is_empty() && cn.stroke.paints.is_empty())
                .map(|(x0, y0, x1, y1)| Rect::new(x0, y0, x1 - x0, y1 - y0))
                .unwrap_or_else(|| super::affine_rect_aabb(&world, cn.size));
            if !bb.intersects(&view) || !bb.intersects(&search_rect) {
                continue;
            }
            if !visited_parents.contains(&app.page) {
                out.push(bb);
            }
            if !visited_parents.contains(&cid) {
                for &gc in &cn.children {
                    if exclude.contains(&gc) || out.len() >= MAX_SNAP_CANDIDATES * 2 {
                        break;
                    }
                    let Some(gn) = app.doc.get(gc) else { continue };
                    if !gn.visible {
                        continue;
                    }
                    let gbb = super::affine_rect_aabb(&(world * gn.transform), gn.size);
                    if gbb.intersects(&view) && gbb.intersects(&search_rect) {
                        out.push(gbb);
                    }
                }
            }
        }
    }

    if out.len() > MAX_SNAP_CANDIDATES {
        let ref_c = moving_bb.as_ref().map(rect_center).unwrap_or_else(|| rect_center(&view));
        out.sort_by(|a, b| {
            let ca = rect_center(a);
            let cb = rect_center(b);
            let da = (ca.x - ref_c.x).abs() + (ca.y - ref_c.y).abs();
            let db = (cb.x - ref_c.x).abs() + (cb.y - ref_c.y).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(MAX_SNAP_CANDIDATES);
    }

    out.extend(rulers::guide_rects(app));
    out
}

/// Rounds `v` to a whole pixel unless snapping to the pixel grid is off.
fn px(app: &App, v: f32) -> f32 {
    if app.ui.no_pixel_snap {
        v
    } else {
        v.round()
    }
}

fn snapped_on(guides: &[snap::Guide], axis: snap::GuideAxis) -> bool {
    guides.iter().any(|g| g.axis == axis)
}

fn snap_point(p: DV, targets: &[Rect], zoom: f32) -> (DV, snap::SnapResult) {
    let thr = theme::SNAP_THRESHOLD_PX / zoom;
    let r = snap::snap_edges(&Rect::new(p.x, p.y, 0.0, 0.0), &[p.x], &[p.y], targets, thr);
    (v2(p.x + r.dx, p.y + r.dy), r)
}

// ------------------------------------------------------------------ main entry
pub fn process(app: &mut App, ui: &mut Ui, rect: ERect, resp: &Response) {
    let ctx = ui.ctx().clone();
    let agent_rail_open = !app.ui.left_collapsed && matches!(app.ui.rail, Some(crate::app::RailItem::Agents | crate::app::RailItem::Tools));
    let typing = (ctx.egui_wants_keyboard_input() || app.terminal.is_capturing_keyboard(agent_rail_open)) && app.canvas.text.is_none();
    let inp = gather(ui, typing);
    let hovered = resp.hovered();

    // Wheel pan / pinch & ctrl+wheel zoom.
    if hovered && (app.canvas.text.is_none() || matches!(app.canvas.inter, Interaction::None)) {
        let (scroll, zoom_delta, mods) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers));
        if (zoom_delta - 1.0).abs() > 1e-4 {
            if let Some(p) = inp.pos {
                let z = app.view.zoom * zoom_delta;
                app.view.zoom_at(rect.min, p, z);
            }
        } else if scroll != egui::Vec2::ZERO {
            let mut s = scroll;
            if mods.shift && s.x == 0.0 {
                s = egui::vec2(s.y, 0.0);
            }
            app.view.offset = v2(app.view.offset.x - s.x / app.view.zoom, app.view.offset.y - s.y / app.view.zoom);
        }
    }

    if app.canvas.text.is_some() {
        ts::text_keys(app, &ctx);
        // drag-select with the primary button held
        if app.canvas.text.as_ref().is_some_and(|t| t.dragging) {
            match inp.pos {
                Some(p) if inp.down => ts::drag_to(app, app.view.to_page(rect.min, p)),
                _ => {
                    if let Some(t) = app.canvas.text.as_mut() {
                        t.dragging = false;
                    }
                }
            }
        }
    }

    if let Some(v) = &app.canvas.vedit {
        if app.selection != [v.id] || !matches!(app.tool, Tool::Move) {
            vector::exit(app);
        } else {
            vector::reload(app);
        }
    }

    app.canvas.pointer_page = inp.pos.filter(|p| rect.contains(*p)).map(|p| app.view.to_page(rect.min, p));
    if app.canvas.vedit.is_some() {
        vector::hover(app, rect, inp.pos);
    }
    if app.tool == Tool::ShapeBuilder {
        builder::hover(app, app.canvas.pointer_page);
    }

    if inp.pressed {
        // a release that never reached us must not complete a later gesture
        ctx.data_mut(|d| d.remove_temp::<Pos2>(grid_pill_press_id()));
    }
    if (inp.pressed || inp.mid_pressed) && hovered {
        app.terminal.viewport_focused = false;
        if let Some(focused_id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(focused_id));
        }
        on_press(app, &ctx, rect, &inp);
    }
    if inp.released {
        finish_grid_track_pill(app, &ctx, rect, inp.pos);
    }
    if !matches!(app.canvas.inter, Interaction::None) {
        if inp.down || inp.mid_down {
            on_drag(app, ui, rect, &inp);
        }
        let ended = match app.canvas.inter {
            Interaction::Pan => !inp.down && !inp.mid_down,
            Interaction::PenDrag => !inp.down,
            _ => inp.released || !inp.down && !inp.pressed,
        };
        if ended {
            on_release(app, rect, &inp);
        }
    }
    if resp.secondary_clicked() && hovered {
        if let Some(pos) = inp.pos {
            if rulers::secondary_click(app, rect, pos) {
                app.show_toast("Cleared canvas guides", false);
            }
        }
    }
    if resp.triple_clicked() && hovered {
        if let (Some(id), Some(pos)) = (app.canvas.text.as_ref().map(|t| t.id), inp.pos) {
            let page = app.view.to_page(rect.min, pos);
            if ts::node_contains(app, id, page) {
                ts::select_paragraph(app, page);
            }
        }
    } else if resp.double_clicked() && hovered {
        on_double_click(app, rect, &inp);
    }
    update_hover_and_cursor(app, rect, &inp, hovered);
}

// ------------------------------------------------------------------ press
fn handle_hit(app: &App, rect: ERect, pos: Pos2) -> Option<HandleHit> {
    if !matches!(app.tool, Tool::Move | Tool::Scale) || app.canvas.vedit.is_some() {
        return None;
    }
    // a connector has no box to resize: it is moved by its ends
    if !app.selection.is_empty() && app.selection.iter().all(|id| matches!(app.doc.get(*id).map(|n| &n.kind), Some(NodeKind::Connector(_)))) {
        return None;
    }
    let (corners, is_line) = selection_box(app)?;
    let s = corners.map(|c| dv(app.view.to_screen(rect.min, c)));
    let w = dist(s[0], s[1]);
    let h = dist(s[0], s[3]);
    let allow_mid = w.min(h) > theme::CANVAS_HANDLE_SIZE * 3.0;
    let hit = handles::hit_handle(&s, dv(pos), theme::CANVAS_HANDLE_HIT, theme::CANVAS_ROTATE_ZONE, allow_mid || is_line)?;
    match hit {
        HandleHit::Resize(h) if is_line && !matches!(h, Handle::L | Handle::R) => None,
        HandleHit::Rotate(_) if is_line => None,
        other => Some(other),
    }
}

/// Checks if the click hits an on-canvas `Ready for dev` / `Completed` status pill badge (`S3-12`),
/// cycling `n.dev.status` (`ReadyForDev` -> `Completed` -> `ReadyForDev`) with a toast.
pub fn try_click_dev_status_badge(app: &mut App, rect: ERect, pos: Pos2) -> bool {
    for (id, label_r) in super::draw::label_rects(app, rect) {
        let Some(n) = app.doc.get(id) else { continue };
        let Some((badge_r, status)) = super::draw::dev_status_badge_rect(id, n, label_r) else { continue };
        if badge_r.contains(pos) {
            let next = match status {
                designos_doc::DevStatus::ReadyForDev | designos_doc::DevStatus::Changed => designos_doc::DevStatus::Completed,
                designos_doc::DevStatus::Completed => designos_doc::DevStatus::ReadyForDev,
            };
            let mut tx = Tx::new();
            if let Some(node) = tx.ed.node(&mut app.doc, id) {
                node.dev.status = Some(next);
            }
            if app.run_tx(tx) {
                app.show_toast(format!("Status: {}", next.label()), false);
            }
            return true;
        }
    }
    false
}

/// Checks if the click hits the on-canvas `"Tidy up"` pill (`#16`), a 2D CSS Grid track header pill (`#18`)
/// or a grid `+` add-track button (`#144`).
pub fn try_click_canvas_action_pill(app: &mut App, rect: ERect, pos: Pos2) -> bool {
    if let Some(tidy_r) = super::draw::tidy_up_pill_rect(app, rect) {
        if tidy_r.contains(pos) {
            app.tidy_up();
            return true;
        }
    }
    if try_cycle_grid_track_pill(app, rect, pos) {
        return true;
    }
    for (grid_id, is_col, add_r) in super::draw::grid_track_add_button_rects(app, rect) {
        if add_r.contains(pos) {
            let at = super::grids::grid_track_count(&app.doc, grid_id, is_col);
            if super::grids::insert_grid_track(app, grid_id, is_col, at, designos_doc::GridTrack::Flex(1.0)) {
                app.show_toast(if is_col { "Added column track" } else { "Added row track" }, false);
            }
            return true;
        }
    }
    false
}

/// Cycles the sizing (`1fr` -> `Npx` -> `Hug` -> `1fr`) of the grid track whose header pill is at `pos`, in one undo step.
pub fn try_cycle_grid_track_pill(app: &mut App, rect: ERect, pos: Pos2) -> bool {
    let Some((grid_id, is_col, idx, _)) = super::draw::grid_track_pill_at(app, rect, pos) else {
        return false;
    };
    let mut tx = Tx::new();
    let mut label = String::new();
    if let Some(ov) = super::grids::node_css_grid_overlay(&app.doc, grid_id) {
        if let Some(n) = tx.ed.node(&mut app.doc, grid_id) {
            if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                if is_col {
                    if let Some(tr) = al.grid_col_tracks.get_mut(idx) {
                        *tr = match *tr {
                            designos_doc::GridTrack::Flex(_) => {
                                designos_doc::GridTrack::Fixed(ov.col_widths.get(idx).copied().unwrap_or(120.0).round().max(24.0))
                            }
                            designos_doc::GridTrack::Fixed(_) => designos_doc::GridTrack::Hug,
                            designos_doc::GridTrack::Hug => designos_doc::GridTrack::Flex(1.0),
                        };
                        label = super::grids::format_grid_track(*tr);
                    }
                } else if let Some(tr) = al.grid_row_tracks.get_mut(idx) {
                    *tr = match *tr {
                        designos_doc::GridTrack::Flex(_) => designos_doc::GridTrack::Fixed(ov.row_heights.get(idx).copied().unwrap_or(80.0).round().max(24.0)),
                        designos_doc::GridTrack::Fixed(_) => designos_doc::GridTrack::Hug,
                        designos_doc::GridTrack::Hug => designos_doc::GridTrack::Flex(1.0),
                    };
                    label = super::grids::format_grid_track(*tr);
                }
            }
        }
        layout::relayout_around(&mut app.doc, &mut tx.ed, grid_id);
    }
    if app.run_tx(tx) && !label.is_empty() {
        app.show_toast(format!("Grid track → {label}"), false);
    }
    true
}

/// Pointer travel (screen px) past which a press on a grid track pill is a drag instead of a click.
const GRID_PILL_DRAG_SLOP: f32 = 4.0;

fn grid_pill_press_id() -> egui::Id {
    egui::Id::new("grid-track-pill-press")
}

/// Holds back a press on a grid track header pill until the button is released, so one gesture is one edit:
/// a drag onto another pill of the same grid and axis reorders the tracks, a click cycles the sizing.
fn arm_grid_track_pill(app: &App, ctx: &egui::Context, rect: ERect, pos: Pos2) -> bool {
    if super::draw::tidy_up_pill_rect(app, rect).is_some_and(|r| r.contains(pos)) {
        return false;
    }
    if super::draw::grid_track_pill_at(app, rect, pos).is_none() {
        return false;
    }
    ctx.data_mut(|d| d.insert_temp(grid_pill_press_id(), pos));
    true
}

/// Completes a press that [`arm_grid_track_pill`] held back. A release still on the source pill cycles its sizing;
/// a drag onto another pill of the same grid and axis reorders the tracks; a release off every pill changes nothing.
fn finish_grid_track_pill(app: &mut App, ctx: &egui::Context, rect: ERect, release: Option<Pos2>) {
    let Some(press) = ctx.data_mut(|d| d.remove_temp::<Pos2>(grid_pill_press_id())) else { return };
    let Some(release) = release else { return };
    let same_pill = match (super::draw::grid_track_pill_at(app, rect, press), super::draw::grid_track_pill_at(app, rect, release)) {
        (Some((g0, c0, i0, _)), Some((g1, c1, i1, _))) => g0 == g1 && c0 == c1 && i0 == i1,
        _ => false,
    };
    if (release - press).length() > GRID_PILL_DRAG_SLOP && !same_pill {
        try_drag_grid_track_pill(app, rect, press, release);
    } else {
        try_cycle_grid_track_pill(app, rect, press);
    }
}

/// Reorders a grid column/row track when dragging from one on-canvas track header pill to another (`#144`).
pub fn try_drag_grid_track_pill(app: &mut App, rect: ERect, from_pos: Pos2, to_pos: Pos2) -> bool {
    let Some((grid_id, is_col, from_idx, _)) = super::draw::grid_track_pill_at(app, rect, from_pos) else {
        return false;
    };
    let same_axis = super::draw::grid_track_pill_rects(app, rect).into_iter().filter(|&(gid, col, idx, _)| gid == grid_id && col == is_col && idx != from_idx);
    let Some((_, _, to_idx, _)) = super::draw::nearest_pill(same_axis, to_pos) else {
        return false;
    };
    super::grids::move_grid_track(app, grid_id, is_col, from_idx, to_idx)
}

/// Returns `(shared_parent_id, items_with_world_aabb)` when 2 or more selected nodes
/// share the same non-Auto-Layout parent (`S1-11`).
pub(crate) fn smart_selection_items(app: &App) -> Option<(NodeId, Vec<(NodeId, Rect)>)> {
    if app.selection.len() < 2 || app.selection.len() > 64 {
        return None;
    }
    let first = app.selection[0];
    let parent = app.doc.get(first)?.parent?;
    if app.doc.get(parent)?.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).is_some() {
        return None;
    }
    let idx = active_index(app);
    let mut items = Vec::with_capacity(app.selection.len());
    for &id in &app.selection {
        let n = app.doc.get(id)?;
        if n.parent != Some(parent) || parent_has_auto_layout(&app.doc, id) {
            return None;
        }
        items.push((id, super::fast_world_aabb(&app.doc, idx, id)));
    }
    Some((parent, items))
}

fn try_start_smart_selection_handle(app: &mut App, rect: ERect, pos: Pos2, page: DV) -> bool {
    let Some((_, items)) = smart_selection_items(app) else { return false };
    let hnds = handles::smart_selection_handles(&items);
    let Some(kind) = handles::hit_smart_selection_handle(&hnds, |c| dv(app.view.to_screen(rect.min, c)), dv(pos), 4.0) else {
        return false;
    };
    let mut min_cx = f32::MAX;
    let mut max_cx = f32::MIN;
    let mut min_cy = f32::MAX;
    let mut max_cy = f32::MIN;
    for (_, r) in &items {
        let cx = r.x + r.w * 0.5;
        let cy = r.y + r.h * 0.5;
        min_cx = min_cx.min(cx);
        max_cx = max_cx.max(cx);
        min_cy = min_cy.min(cy);
        max_cy = max_cy.max(cy);
    }
    let horiz = (max_cx - min_cx) >= (max_cy - min_cy);
    let mut sorted_items = items;
    if horiz {
        sorted_items.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then_with(|| a.1.y.total_cmp(&b.1.y)));
    } else {
        sorted_items.sort_by(|a, b| a.1.y.total_cmp(&b.1.y).then_with(|| a.1.x.total_cmp(&b.1.x)));
    }
    let start_gap = match kind {
        handles::SmartSelectionHandleKind::Gap { horiz: gh, index } => {
            let i = index.min(sorted_items.len().saturating_sub(2));
            let a = &sorted_items[i].1;
            let b = &sorted_items[i + 1].1;
            if gh {
                (b.x - (a.x + a.w)).max(0.0)
            } else {
                (b.y - (a.y + a.h)).max(0.0)
            }
        }
        handles::SmartSelectionHandleKind::Swap(_) => 0.0,
    };
    let sorted: Vec<(NodeId, Rect, f32, f32)> = sorted_items
        .into_iter()
        .filter_map(|(id, r)| {
            let n = app.doc.get(id)?;
            Some((id, r, n.transform.tx, n.transform.ty))
        })
        .collect();
    app.canvas.inter = Interaction::SmartSelectionDrag(SmartSelectionDragState { tx: Tx::new(), kind, start_page: page, sorted, start_gap });
    true
}

pub fn update_smart_selection_drag(app: &mut App, sd: &mut SmartSelectionDragState, cur: DV) {
    sd.tx.ed.revert(&mut app.doc);
    match sd.kind {
        handles::SmartSelectionHandleKind::Gap { horiz, .. } => {
            if sd.sorted.len() >= 2 {
                let delta = if horiz { cur.x - sd.start_page.x } else { cur.y - sd.start_page.y };
                let new_gap = (sd.start_gap + delta).max(0.0).round();
                let mut cursor = if horiz { sd.sorted[0].1.x + sd.sorted[0].1.w } else { sd.sorted[0].1.y + sd.sorted[0].1.h };
                for k in 1..sd.sorted.len() {
                    let (id, aabb0, tx0, ty0) = sd.sorted[k];
                    let target_pos = cursor + new_gap;
                    let world_delta = if horiz { v2(target_pos - aabb0.x, 0.0) } else { v2(0.0, target_pos - aabb0.y) };
                    let dp = delta_to_parent(&app.doc, id, world_delta);
                    if let Some(n) = sd.tx.ed.node(&mut app.doc, id) {
                        n.transform.tx = tx0 + dp.x;
                        n.transform.ty = ty0 + dp.y;
                    }
                    cursor = target_pos + if horiz { aabb0.w } else { aabb0.h };
                }
            }
        }
        handles::SmartSelectionHandleKind::Swap(dragged_id) => {
            if let Some(&(_, _, drag_tx0, drag_ty0)) = sd.sorted.iter().find(|&&(id, ..)| id == dragged_id) {
                if let Some(&(other_id, _, other_tx0, other_ty0)) =
                    sd.sorted.iter().find(|&&(id, ref r, _, _)| id != dragged_id && cur.x >= r.x && cur.x <= r.x + r.w && cur.y >= r.y && cur.y <= r.y + r.h)
                {
                    if let Some(n1) = sd.tx.ed.node(&mut app.doc, dragged_id) {
                        n1.transform.tx = other_tx0;
                        n1.transform.ty = other_ty0;
                    }
                    if let Some(n2) = sd.tx.ed.node(&mut app.doc, other_id) {
                        n2.transform.tx = drag_tx0;
                        n2.transform.ty = drag_ty0;
                    }
                }
            }
        }
    }
    app.changed();
}

fn on_press(app: &mut App, ctx: &egui::Context, rect: ERect, inp: &Inp) {
    let Some(pos) = inp.pos else { return };
    if app.props.eyedropper.is_some() || app.props.picker.as_ref().is_some_and(|pk| pk.eyedrop) {
        return;
    }
    app.canvas.al_inline = None;
    let page = app.view.to_page(rect.min, pos);
    if inp.pressed && inp.z_down && app.canvas.text.is_none() {
        app.canvas.inter = Interaction::ZoomMarquee { start: page, cur: page, alt: inp.mods.alt };
        return;
    }
    if inp.pressed && !inp.space && rulers::press_with_alt(app, rect, pos, inp.mods.alt) {
        return;
    }
    // Text editing: click inside the edited node moves the caret, elsewhere ends editing.
    if let Some(sess) = &app.canvas.text {
        let id = sess.id;
        if ts::node_contains(app, id, page) {
            ts::press_at(app, page, inp.mods.shift);
            return;
        }
        finish_text(app, true);
    }
    let mid = inp.mid_pressed && !inp.pressed;
    if !mid
        && !inp.space
        && (try_click_dev_status_badge(app, rect, pos) || arm_grid_track_pill(app, ctx, rect, pos) || try_click_canvas_action_pill(app, rect, pos))
    {
        return;
    }
    if app.canvas.vedit.is_some() && !mid && !inp.space && vector::press(app, rect, pos, inp.mods) {
        return;
    }
    // board: a magnet dot starts a connector
    if app.is_board() && !mid && !inp.space && matches!(app.tool, Tool::Move | Tool::Connector) {
        if let Some((node, magnet)) = crate::board::connect::dot_hit(app, rect, pos) {
            crate::board::connect::start(app, Some((node, magnet)), page);
            return;
        }
    }
    let deep = inp.mods.command;
    let over_handle = handle_hit(app, rect, pos);
    let over_corner = matches!(over_handle, Some(HandleHit::Resize(h)) if h.is_corner());
    if !over_corner && !mid && !inp.space && matches!(app.tool, Tool::Move) {
        if let [sel_id] = app.selection.as_slice() {
            let sel_id = *sel_id;
            if try_start_special_handle(app, rect, pos, page, sel_id) {
                return;
            }
        } else if app.selection.len() >= 2 && try_start_smart_selection_handle(app, rect, pos, page) {
            return;
        }
    }
    let target = if let Some(h) = over_handle {
        PressTarget::Handle(h)
    } else if let Some(id) = label_hit(app, rect, pos).or_else(|| hit_under(app, page, deep)) {
        if app.is_selected(id) {
            PressTarget::SelectedNode(id)
        } else {
            PressTarget::Node(id)
        }
    } else {
        PressTarget::Empty
    };
    let action = press_action(app.tool, target, inp.space, mid);
    match action {
        PressAction::Pan => app.canvas.inter = Interaction::Pan,
        PressAction::Resize(h) => start_resize(app, h, page),
        PressAction::Rotate(c) => start_rotate(app, c, page),
        PressAction::MoveNode { select_first } => {
            let id = match target {
                PressTarget::Node(i) | PressTarget::SelectedNode(i) => i,
                _ => return,
            };
            if select_first {
                if inp.mods.shift {
                    app.selection.push(id);
                } else {
                    app.select_only(id);
                }
            }
            app.canvas.inter =
                Interaction::Pending(Pending { node: id, start_screen: pos, start_page: page, shift: inp.mods.shift, was_selected: !select_first });
        }
        PressAction::Marquee => {
            let base = if inp.mods.shift { app.selection.clone() } else { vec![] };
            if !inp.mods.shift {
                app.clear_selection();
            }
            app.canvas.inter = Interaction::Marquee { start: page, cur: page, base };
        }
        PressAction::Draw(tool) => start_draw(app, tool, page),
        PressAction::Build => builder::press(app, page, inp.mods.alt),
        PressAction::PenPoint => pen_press(app, rect, pos, page, inp.mods),
        PressAction::Connect => {
            let from = hit_under(app, page, true)
                .filter(|id| app.doc.get(*id).is_some_and(|n| crate::board::connectable(&n.kind)))
                .map(|id| (id, designos_doc::Magnet::Auto));
            crate::board::connect::start(app, from, page);
        }
        PressAction::TextCreate => text_press(app, page),
        PressAction::Nothing => {
            if app.tool == Tool::Image {
                place_image(app, page);
            } else if app.tool == Tool::Comment {
                app.show_toast("Comments are not available in the offline editor", false);
            }
        }
    }
}

fn try_start_special_handle(app: &mut App, rect: ERect, pos: Pos2, page: DV, id: NodeId) -> bool {
    let Some(n) = app.doc.get(id) else { return false };
    let z = app.view.zoom;
    let corners = super::fast_world_corners(&app.doc, id).map(|c| dv(app.view.to_screen(rect.min, c)));
    if handles::supports_corner_radius_handles(&n.kind) {
        if let Some(corner) = handles::hit_corner_radius_handle(&corners, n.corner_radii, z, dv(pos), 4.5) {
            app.canvas.inter = Interaction::CornerRadius(CornerRadiusDragState { tx: Tx::new(), id, corner, start_radii: n.corner_radii });
            return true;
        }
    }
    if let Some(al) = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) {
        let mut child_rects = Vec::new();
        for &cid in &n.children {
            if let Some(c) = app.doc.get(cid) {
                if c.visible && !c.layout_child.absolute {
                    child_rects.push(Rect::new(c.transform.tx, c.transform.ty, c.size.x, c.size.y));
                }
            }
        }
        let hnds = handles::auto_layout_handles(al, n.size, &child_rects, z);
        let w_xform = super::fast_world_transform(&app.doc, id);
        if let Some(kind) = handles::hit_auto_layout_handle(&hnds, |lc| dv(app.view.to_screen(rect.min, w_xform.apply(lc))), dv(pos), 3.0) {
            let start_value = match kind {
                handles::AutoLayoutHandleKind::Padding(s) => al.padding[s.min(3)],
                handles::AutoLayoutHandleKind::Gap(_) => al.item_spacing,
            };
            app.canvas.inter = Interaction::AutoLayoutDrag(AutoLayoutDragState { tx: Tx::new(), id, kind, start_page: page, start_value, moved: false });
            return true;
        }
    }
    if let Some(kind) = handles::hit_shape_control_handle(&n.kind, &corners, z, dv(pos), 6.0) {
        let (start_count, start_ratio, start_end_angle) = match &n.kind {
            NodeKind::Ellipse(arc) => (0, arc.inner_radius, arc.end),
            NodeKind::Polygon { count } => (*count, 0.0, 0.0),
            NodeKind::Star { count, inner_ratio } => (*count, *inner_ratio, 0.0),
            _ => (0, 0.0, 0.0),
        };
        app.canvas.inter =
            Interaction::ShapeControl(ShapeControlDragState { tx: Tx::new(), id, kind, start_page: page, start_count, start_ratio, start_end_angle });
        return true;
    }
    false
}

fn special_handle_hovered(app: &App, rect: ERect, pos: Pos2) -> bool {
    if app.selection.len() >= 2 {
        if let Some((_, items)) = smart_selection_items(app) {
            let hnds = handles::smart_selection_handles(&items);
            if handles::hit_smart_selection_handle(&hnds, |c| dv(app.view.to_screen(rect.min, c)), dv(pos), 4.0).is_some() {
                return true;
            }
        }
        return false;
    }
    let [id] = app.selection.as_slice() else { return false };
    let id = *id;
    let Some(n) = app.doc.get(id) else { return false };
    let z = app.view.zoom;
    let corners = super::fast_world_corners(&app.doc, id).map(|c| dv(app.view.to_screen(rect.min, c)));
    if handles::supports_corner_radius_handles(&n.kind) && handles::hit_corner_radius_handle(&corners, n.corner_radii, z, dv(pos), 4.5).is_some() {
        return true;
    }
    if let Some(al) = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) {
        let mut child_rects = Vec::new();
        for &cid in &n.children {
            if let Some(c) = app.doc.get(cid) {
                if c.visible && !c.layout_child.absolute {
                    child_rects.push(Rect::new(c.transform.tx, c.transform.ty, c.size.x, c.size.y));
                }
            }
        }
        let hnds = handles::auto_layout_handles(al, n.size, &child_rects, z);
        let w_xform = super::fast_world_transform(&app.doc, id);
        if handles::hit_auto_layout_handle(&hnds, |lc| dv(app.view.to_screen(rect.min, w_xform.apply(lc))), dv(pos), 3.0).is_some() {
            return true;
        }
    }
    handles::hit_shape_control_handle(&n.kind, &corners, z, dv(pos), 6.0).is_some()
}

pub fn update_shape_control(app: &mut App, s: &mut ShapeControlDragState, cur: DV, shift: bool) {
    let _ = (s.start_ratio, s.start_end_angle);
    s.tx.ed.revert(&mut app.doc);
    let inv = app.doc.world_transform(s.id).invert().unwrap_or(Affine::IDENTITY);
    let local_p = inv.apply(cur);
    if let Some(n) = s.tx.ed.node(&mut app.doc, s.id) {
        let w = n.size.x.max(1.0);
        let h = n.size.y.max(1.0);
        let cx = w * 0.5;
        let cy = h * 0.5;
        let radius = (w.min(h) * 0.5).max(1.0);
        match (&mut n.kind, s.kind) {
            (NodeKind::Ellipse(arc), handles::ShapeControlKind::EllipseSweep) => {
                let mut ang = (local_p.y - cy).atan2(local_p.x - cx);
                if shift {
                    ang = handles::snap_angle(ang, 15.0);
                }
                if ang <= arc.start {
                    ang += std::f32::consts::TAU;
                }
                arc.end = ang.clamp(arc.start + 0.05, arc.start + std::f32::consts::TAU);
            }
            (NodeKind::Ellipse(arc), handles::ShapeControlKind::EllipseInnerRadius) => {
                let d = ((local_p.x - cx).hypot(local_p.y - cy) / radius).clamp(0.0, 0.95);
                arc.inner_radius = d;
            }
            (NodeKind::Ellipse(arc), handles::ShapeControlKind::EllipseStartAngle) => {
                let mut ang = (local_p.y - cy).atan2(local_p.x - cx);
                if shift {
                    ang = handles::snap_angle(ang, 15.0);
                }
                let sweep = (s.start_end_angle - arc.start).clamp(0.05, std::f32::consts::TAU);
                arc.start = ang;
                arc.end = ang + sweep;
            }
            (NodeKind::Polygon { count }, handles::ShapeControlKind::PolygonCount) | (NodeKind::Star { count, .. }, handles::ShapeControlKind::StarCount) => {
                let dy = cur.y - s.start_page.y;
                let delta = (dy / -8.0).round() as i32;
                *count = (s.start_count as i32 + delta).clamp(3, 60) as u32;
            }
            (NodeKind::Star { inner_ratio, .. }, handles::ShapeControlKind::StarInnerRatio) => {
                let d = ((local_p.x - cx).hypot(local_p.y - cy) / radius).clamp(0.05, 0.95);
                *inner_ratio = d;
            }
            _ => {}
        }
    }
    app.changed();
}

fn label_hit(app: &App, rect: ERect, pos: Pos2) -> Option<NodeId> {
    super::draw::label_rects(app, rect).into_iter().find(|(_, r)| r.contains(pos)).map(|(id, _)| id)
}

fn start_resize(app: &mut App, handle: Handle, page: DV) {
    let sel = app.top_selection();
    if sel.is_empty() {
        return;
    }
    let tx = Tx::new();
    let idx = active_index(app);
    let kind = if sel.len() > 1 {
        let nodes: Vec<_> =
            sel.iter().filter_map(|id| app.doc.get(*id).map(|n| (*id, super::fast_world_aabb(&app.doc, idx, *id), n.transform, n.size))).collect();
        let Some(bbox0) = union_all(nodes.iter().map(|n| n.1)) else { return };
        ResizeKind::Multi { bbox0, nodes }
    } else {
        let id = sel[0];
        let Some(n) = app.doc.get(id) else { return };
        ResizeKind::Single { id, world0: super::fast_world_transform(&app.doc, id), t0: n.transform, size0: n.size }
    };
    let targets = snap_targets(app, &sel);
    let _ = page;
    app.canvas.inter = Interaction::Resize(ResizeState { tx, handle, kind, targets });
}

fn start_rotate(app: &mut App, _corner: usize, page: DV) {
    let tops = app.top_selection();
    let idx = active_index(app);
    if tops.len() > 1 {
        let Some(bb) = union_all(tops.iter().map(|i| super::fast_world_aabb(&app.doc, idx, *i))) else { return };
        let center = rect_center(&bb);
        let group = tops.iter().map(|i| (*i, super::fast_world_transform(&app.doc, *i))).collect();
        app.canvas.inter = Interaction::Rotate(RotateState {
            tx: Tx::new(),
            id: tops[0],
            center,
            start_angle: handles::angle_of(center, page),
            t0: Affine::IDENTITY,
            size: DV::ZERO,
            rot0: 0.0,
            group,
        });
        return;
    }
    let Some(id) = app.selection.first().copied() else { return };
    let Some(n) = app.doc.get(id) else { return };
    let w = super::fast_world_transform(&app.doc, id);
    let center = w.apply(v2(n.size.x / 2.0, n.size.y / 2.0));
    app.canvas.inter = Interaction::Rotate(RotateState {
        tx: Tx::new(),
        id,
        center,
        start_angle: handles::angle_of(center, page),
        t0: n.transform,
        size: n.size,
        rot0: n.transform.rotation_rad(),
        group: Vec::new(),
    });
}

fn start_draw(app: &mut App, tool: Tool, page: DV) {
    app.clear_selection();
    let targets = snap_targets(app, &[]);
    let (start, sr) = snap_point(page, &targets, app.view.zoom);
    let start = v2(
        if snapped_on(&sr.guides, snap::GuideAxis::X) { start.x } else { px(app, start.x) },
        if snapped_on(&sr.guides, snap::GuideAxis::Y) { start.y } else { px(app, start.y) },
    );
    let parent = create::container_at_with_index(&app.doc, active_index(app), app.page, start, &[]);
    let node = create::new_shape(app, tool);
    let id = node.id;
    if !app.slot_insert_allowed(parent, &[&node]) {
        return;
    }
    let mut tx = Tx::new();
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    if tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![node] }).is_err() {
        return;
    }
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    app.selection = vec![id];
    app.canvas.inter = Interaction::Draw(DrawState { tx, tool, id, parent, start, last_page: start, targets, dragged: false });
    app.changed();
}

// ------------------------------------------------------------------ drag
fn on_drag(app: &mut App, ui: &Ui, rect: ERect, inp: &Inp) {
    let Some(pos) = inp.pos else { return };
    let cur = app.view.to_page(rect.min, pos);
    let zoom = app.view.zoom;
    let mut inter = std::mem::take(&mut app.canvas.inter);
    match &mut inter {
        Interaction::Pan => {
            let d = ui.input(|i| i.pointer.delta());
            app.view.offset = v2(app.view.offset.x - d.x / zoom, app.view.offset.y - d.y / zoom);
        }
        Interaction::ZoomMarquee { cur: c, .. } => {
            *c = cur;
        }
        Interaction::Pending(p) => {
            if (pos - p.start_screen).length() > theme::DRAG_THRESHOLD_PX {
                let (node, start, alt) = (p.node, p.start_page, inp.mods.alt);
                if let Some(mut m) = start_move(app, node, start, alt) {
                    update_move(app, &mut m, cur, inp);
                    inter = Interaction::Move(m);
                } else {
                    inter = Interaction::None;
                }
            }
        }
        Interaction::Move(m) => update_move(app, m, cur, inp),
        Interaction::Guide(g) => rulers::drag(app, g, rect, pos),
        Interaction::Connect(c) => crate::board::connect::drag(app, c, cur),
        Interaction::Marquee { start, cur: c, base } => {
            if inp.space {
                let delta = sub(cur, *c);
                *start = add(*start, delta);
            }
            *c = cur;
            let r = rect_from_points(*start, cur);
            let hits = hit::marquee_select_with_index(&app.doc, app.page, &r, active_index(app));
            if base.is_empty() {
                app.selection = hits;
            } else {
                let mut sel = base.clone();
                let mut seen: std::collections::HashSet<NodeId> = sel.iter().copied().collect();
                for id in hits {
                    if seen.insert(id) {
                        sel.push(id);
                    }
                }
                app.selection = sel;
            }
        }
        Interaction::Resize(r) => update_resize(app, r, cur, inp),
        Interaction::Rotate(r) => {
            r.tx.ed.revert(&mut app.doc);
            if r.group.is_empty() {
                let mut ang = r.rot0 + (handles::angle_of(r.center, cur) - r.start_angle);
                if inp.mods.shift {
                    ang = handles::snap_angle(ang, 15.0);
                }
                let t = handles::rotate_about_center(&r.t0, r.size, ang);
                if let Some(n) = r.tx.ed.node(&mut app.doc, r.id) {
                    n.transform = t;
                }
                layout::relayout_up(&mut app.doc, &mut r.tx.ed, r.id);
            } else {
                let mut delta = handles::angle_of(r.center, cur) - r.start_angle;
                if inp.mods.shift {
                    delta = handles::snap_angle(delta, 15.0);
                }
                crate::ops::rotate_nodes_about(&mut app.doc, &mut r.tx.ed, &r.group, r.center, delta);
            }
            app.changed();
        }
        Interaction::CornerRadius(cr) => {
            cr.tx.ed.revert(&mut app.doc);
            let inv = app.doc.world_transform(cr.id).invert().unwrap_or(Affine::IDENTITY);
            let ml = inv.apply(cur);
            if let Some(n) = cr.tx.ed.node(&mut app.doc, cr.id) {
                let dx = if matches!(cr.corner, 0 | 3) { ml.x } else { n.size.x - ml.x };
                let dy = if matches!(cr.corner, 0 | 1) { ml.y } else { n.size.y - ml.y };
                let max_r = (n.size.x.min(n.size.y) * 0.5).max(0.0);
                let new_r = dx.min(dy).clamp(0.0, max_r).round();
                if inp.mods.alt {
                    let mut r = cr.start_radii;
                    r[cr.corner] = new_r;
                    n.corner_radii = r;
                } else {
                    n.corner_radii = [new_r; 4];
                }
            }
            app.changed();
        }
        Interaction::AutoLayoutDrag(ad) => {
            if dist(ad.start_page, cur) * zoom > theme::DRAG_THRESHOLD_PX {
                ad.moved = true;
            }
            if !ad.moved {
                app.canvas.inter = inter;
                return;
            }
            ad.tx.ed.revert(&mut app.doc);
            let horiz = app
                .doc
                .get(ad.id)
                .and_then(|n| n.kind.frame_props())
                .and_then(|fp| fp.auto_layout.as_ref())
                .is_some_and(|al| al.direction == designos_doc::LayoutDirection::Horizontal);
            let delta = match ad.kind {
                handles::AutoLayoutHandleKind::Padding(0) => cur.y - ad.start_page.y,
                handles::AutoLayoutHandleKind::Padding(1) => ad.start_page.x - cur.x,
                handles::AutoLayoutHandleKind::Padding(2) => ad.start_page.y - cur.y,
                handles::AutoLayoutHandleKind::Padding(_) => cur.x - ad.start_page.x,
                handles::AutoLayoutHandleKind::Gap(_) => {
                    if horiz {
                        cur.x - ad.start_page.x
                    } else {
                        cur.y - ad.start_page.y
                    }
                }
            };
            let new_val = match ad.kind {
                handles::AutoLayoutHandleKind::Gap(_) => (ad.start_value + delta).clamp(-200.0, 2000.0).round(),
                handles::AutoLayoutHandleKind::Padding(_) => (ad.start_value + delta).max(0.0).round(),
            };
            if let Some(n) = ad.tx.ed.node(&mut app.doc, ad.id) {
                if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                    match ad.kind {
                        handles::AutoLayoutHandleKind::Padding(side) => {
                            if inp.mods.shift {
                                al.padding = [new_val; 4];
                            } else if inp.mods.alt {
                                if side % 2 == 0 {
                                    al.padding[0] = new_val;
                                    al.padding[2] = new_val;
                                } else {
                                    al.padding[1] = new_val;
                                    al.padding[3] = new_val;
                                }
                            } else {
                                al.padding[side.min(3)] = new_val;
                            }
                        }
                        handles::AutoLayoutHandleKind::Gap(_) => {
                            al.item_spacing = new_val;
                        }
                    }
                }
            }
            layout::relayout_around(&mut app.doc, &mut ad.tx.ed, ad.id);
            app.changed();
        }
        Interaction::SmartSelectionDrag(sd) => {
            update_smart_selection_drag(app, sd, cur);
        }
        Interaction::ShapeControl(sc) => {
            update_shape_control(app, sc, cur, inp.mods.shift);
        }
        Interaction::Draw(d) => {
            if inp.space {
                let delta = sub(cur, d.last_page);
                d.start = add(d.start, delta);
            }
            d.last_page = cur;
            let (mut c, snapped) = if inp.mods.command { (cur, snap::SnapResult::default()) } else { snap_point(cur, &d.targets, zoom) };
            if inp.mods.shift {
                c = cur;
            }
            if !inp.mods.command {
                if !snapped_on(&snapped.guides, snap::GuideAxis::X) {
                    c.x = px(app, c.x);
                }
                if !snapped_on(&snapped.guides, snap::GuideAxis::Y) {
                    c.y = px(app, c.y);
                }
            }
            app.canvas.guides = snapped.guides;
            app.canvas.measures.clear();
            if dist(d.start, c) * zoom > theme::DRAG_THRESHOLD_PX {
                d.dragged = true;
            }
            let g = create::draw_geom(d.tool, d.start, c, inp.mods.shift || crate::board::create::keeps_square(d.tool), inp.mods.alt);
            create::apply_geom(&mut app.doc, &mut d.tx, d.id, d.parent, &g);
            app.changed();
        }
        Interaction::Pencil(pts) => {
            if pts.last().map(|l| dist(*l, cur) * zoom > 1.5).unwrap_or(true) {
                pts.push(cur);
            }
        }
        Interaction::VectorDrag => vector::drag(app, cur, inp.mods),
        Interaction::Builder => builder::drag(app, cur),
        Interaction::PenDrag => {
            if let Some(pen) = &mut app.canvas.pen {
                if let Some(last) = pen.pts.last_mut() {
                    if dist(last.pos, cur) * zoom > 3.0 {
                        // Shift keeps the handle on a 45 degree angle
                        let cur = if inp.mods.shift { create::snap_to_45(last.pos, cur) } else { cur };
                        last.hout = Some(cur);
                        last.hin = Some(sub(scale(last.pos, 2.0), cur));
                    }
                }
            }
        }
        Interaction::None => {}
    }
    if matches!(app.canvas.inter, Interaction::None) {
        app.canvas.inter = inter;
    }
}

fn start_move(app: &mut App, clicked: NodeId, start: DV, alt: bool) -> Option<MoveState> {
    let mut ids = app.top_selection();
    if ids.is_empty() {
        ids.push(clicked);
    }
    let mut tx = Tx::new();
    let mut dup_blocked = None;
    if alt {
        let mut dup = Vec::new();
        // one clip for the whole selection, so connectors between duplicated nodes join the copies
        let clip = crate::clipboard::capture(&app.doc, &ids);
        let copies = clip.map(|c| crate::clipboard::instantiate(&mut app.doc, &c)).unwrap_or_default();
        // A copy lands next to its original, but the drop may carry it elsewhere: only a copy that
        // ends up in a slot it would overfill is refused, and that is decided at release.
        let mut blocked = Vec::new();
        for (id, _) in ids.iter().zip(&copies) {
            let Some(parent) = app.doc.get(*id).and_then(|n| n.parent) else { continue };
            if blocked.iter().any(|(slot, _)| *slot == parent) {
                continue;
            }
            let roots: Vec<&designos_doc::Node> =
                ids.iter().zip(&copies).filter(|(o, _)| app.doc.get(**o).and_then(|n| n.parent) == Some(parent)).filter_map(|(_, c)| c.first()).collect();
            if let Some(err) = crate::ops::slot_edit_error(&app.doc, &[], Some((parent, &roots))) {
                blocked.push((parent, err));
            }
        }
        dup_blocked = Some(blocked);
        for (id, nodes) in ids.iter().zip(copies) {
            let Some(parent) = app.doc.get(*id).and_then(|n| n.parent) else { continue };
            let index = app.doc.get(parent).and_then(|p| p.children.iter().position(|c| c == id)).map(|i| i + 1).unwrap_or(usize::MAX);
            let root = nodes.first().map(|n| n.id);
            if tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes }).is_ok() {
                dup.extend(root);
            }
        }
        if !dup.is_empty() {
            ids = dup;
            app.selection = ids.clone();
        }
    }
    let idx = active_index(app);
    let orig_bbox = union_all(ids.iter().map(|i| super::fast_world_aabb(&app.doc, idx, *i)))?;
    let targets = snap_targets(app, &ids);
    Some(MoveState { tx, ids, start, orig_bbox, targets, moved: false, reorder: None, dup_blocked })
}

fn parent_has_auto_layout(doc: &Document, id: NodeId) -> bool {
    let Some(n) = doc.get(id) else { return false };
    if n.layout_child.absolute {
        return false;
    }
    n.parent.and_then(|p| doc.get(p)).and_then(|p| p.kind.frame_props()).map(|f| f.auto_layout.is_some()).unwrap_or(false)
}

fn update_move(app: &mut App, m: &mut MoveState, cur: DV, inp: &Inp) {
    m.tx.ed.revert(&mut app.doc);
    let mut d = sub(cur, m.start);
    let mut lock_x = false;
    let mut lock_y = false;
    if inp.mods.shift {
        if d.x.abs() > d.y.abs() {
            d.y = 0.0;
            lock_y = true;
        } else {
            d.x = 0.0;
            lock_x = true;
        }
    }
    app.canvas.guides.clear();
    app.canvas.measures.clear();
    if !inp.mods.command {
        let moving = Rect::new(m.orig_bbox.x + d.x, m.orig_bbox.y + d.y, m.orig_bbox.w, m.orig_bbox.h);
        let s = snap::snap_rect(&moving, &m.targets, theme::SNAP_THRESHOLD_PX / app.view.zoom);
        if !lock_x {
            d.x += s.dx;
        }
        if !lock_y {
            d.y += s.dy;
        }
        // whole pixels where no layer / guide edge was snapped
        if !lock_x && !snapped_on(&s.guides, snap::GuideAxis::X) {
            d.x = px(app, m.orig_bbox.x + d.x) - m.orig_bbox.x;
        }
        if !lock_y && !snapped_on(&s.guides, snap::GuideAxis::Y) {
            d.y = px(app, m.orig_bbox.y + d.y) - m.orig_bbox.y;
        }
        app.canvas.guides = s.guides;
        app.canvas.measures = s.measures;
    }
    // Auto-layout children follow the pointer too (their slot is decided on release).
    let mut relayout_roots = Vec::new();
    let mut seen_parents = Vec::new();
    for id in &m.ids {
        let dp = delta_to_parent(&app.doc, *id, d);
        if let Some(n) = m.tx.ed.node(&mut app.doc, *id) {
            n.transform.tx += dp.x;
            n.transform.ty += dp.y;
        }
        if !parent_has_auto_layout(&app.doc, *id) {
            if let Some(p) = app.doc.get(*id).and_then(|n| n.parent) {
                if !seen_parents.contains(&p) {
                    seen_parents.push(p);
                    relayout_roots.push(*id);
                }
            }
        }
    }
    for id in relayout_roots {
        layout::relayout_up(&mut app.doc, &mut m.tx.ed, id);
    }
    m.reorder = match m.ids.as_slice() {
        [only] if parent_has_auto_layout(&app.doc, *only) => {
            let parent = app.doc.get(*only).and_then(|n| n.parent);
            parent.filter(|p| ts::node_contains(app, *p, cur)).and_then(|p| reorder::insertion_at(&app.doc, p, *only, cur))
        }
        _ => None,
    };
    m.moved = true;
    app.changed();
}

fn update_resize(app: &mut App, r: &mut ResizeState, cur: DV, inp: &Inp) {
    r.tx.ed.revert(&mut app.doc);
    let mut mouse = cur;
    app.canvas.guides.clear();
    app.canvas.measures.clear();
    let axis_aligned = match &r.kind {
        ResizeKind::Single { world0, .. } => world0.b.abs() < 1e-4 && world0.c.abs() < 1e-4,
        ResizeKind::Multi { .. } => true,
    };
    if axis_aligned && !inp.mods.command && !inp.mods.shift {
        let xe: Vec<f32> = if r.handle.affects_x() { vec![mouse.x] } else { vec![] };
        let ye: Vec<f32> = if r.handle.affects_y() { vec![mouse.y] } else { vec![] };
        let s = snap::snap_edges(&Rect::new(mouse.x, mouse.y, 0.0, 0.0), &xe, &ye, &r.targets, theme::SNAP_THRESHOLD_PX / app.view.zoom);
        mouse = v2(mouse.x + s.dx, mouse.y + s.dy);
        if r.handle.affects_x() && !snapped_on(&s.guides, snap::GuideAxis::X) {
            mouse.x = px(app, mouse.x);
        }
        if r.handle.affects_y() && !snapped_on(&s.guides, snap::GuideAxis::Y) {
            mouse.y = px(app, mouse.y);
        }
        app.canvas.guides = s.guides;
    }
    let is_scale_tool = app.tool == Tool::Scale;
    match &r.kind {
        ResizeKind::Single { id, world0, t0, size0 } => {
            let Some(inv) = world0.invert() else { return };
            let ml = inv.apply(mouse);
            let is_text = matches!(app.doc.get(*id).map(|n| &n.kind), Some(NodeKind::Text(_)));
            let is_line = matches!(app.doc.get(*id).map(|n| &n.kind), Some(NodeKind::Line));
            let lock_aspect = is_scale_tool || (inp.mods.shift && !is_line) || (is_text && r.handle.is_corner());
            let (mut ns, d) = handles::resize_local(*size0, r.handle, ml, lock_aspect, inp.mods.alt);
            if is_line {
                ns.y = 0.0;
            }
            let t = *t0 * Affine::translate(d.x, if is_line { 0.0 } else { d.y });
            // Cmd/Ctrl while resizing a container ignores child constraints & auto-layout (MI-11).
            if inp.mods.command && app.doc.get(*id).is_some_and(|n| n.kind.is_container()) {
                if let Some(n) = r.tx.ed.node(&mut app.doc, *id) {
                    n.size = ns;
                    n.transform = t;
                }
            } else {
                apply_resize(&mut app.doc, &mut r.tx.ed, *id, ns, t, true);
                if is_scale_tool {
                    let factor = (ns.x / size0.x.max(1.0)).clamp(0.05, 50.0);
                    scale_node_properties_proportionally(&mut app.doc, &mut r.tx.ed, *id, factor);
                }
            }
        }
        ResizeKind::Multi { bbox0, nodes } => {
            let ml = sub(mouse, v2(bbox0.x, bbox0.y));
            let (ns, d) = handles::resize_local(v2(bbox0.w, bbox0.h), r.handle, ml, inp.mods.shift || is_scale_tool, inp.mods.alt);
            let nb = Rect::new(bbox0.x + d.x, bbox0.y + d.y, ns.x, ns.y);
            let (sx, sy) = (ns.x / bbox0.w.max(1e-3), ns.y / bbox0.h.max(1e-3));
            let mut relayout_roots = Vec::new();
            let mut seen_parents = Vec::new();
            for (id, aabb0, t0, size0) in nodes {
                let (nx, ny) = (nb.x + (aabb0.x - bbox0.x) * sx, nb.y + (aabb0.y - bbox0.y) * sy);
                let pinv = parent_world(&app.doc, *id).invert().unwrap_or(Affine::IDENTITY);
                let rotated = t0.b.abs() > 1e-4 || t0.c.abs() > 1e-4;
                if rotated {
                    let c0 = rect_center(aabb0);
                    let c1 = v2(nb.x + (c0.x - bbox0.x) * sx, nb.y + (c0.y - bbox0.y) * sy);
                    let dp = apply_linear(&pinv, sub(c1, c0));
                    apply_resize(&mut app.doc, &mut r.tx.ed, *id, *size0, Affine { tx: t0.tx + dp.x, ty: t0.ty + dp.y, ..*t0 }, false);
                } else {
                    let o = pinv.apply(v2(nx, ny));
                    let t = Affine { tx: o.x, ty: o.y, ..*t0 };
                    apply_resize(&mut app.doc, &mut r.tx.ed, *id, v2((size0.x * sx).max(1.0), (size0.y * sy).max(1.0)), t, false);
                }
                if is_scale_tool {
                    scale_node_properties_proportionally(&mut app.doc, &mut r.tx.ed, *id, sx.clamp(0.05, 50.0));
                }
                if let Some(p) = app.doc.get(*id).and_then(|n| n.parent) {
                    if !seen_parents.contains(&p) {
                        seen_parents.push(p);
                        relayout_roots.push(*id);
                    }
                }
            }
            for id in relayout_roots {
                layout::relayout_up(&mut app.doc, &mut r.tx.ed, id);
            }
        }
    }
    app.changed();
}

/// Proportionally scales stroke weights, corner radii, text font size, and Auto-Layout padding/gap
/// for `Tool::Scale` (`K` `#32`).
fn scale_node_properties_proportionally(doc: &mut Document, ed: &mut crate::edit::Edit, id: NodeId, factor: f32) {
    let mut stack = vec![id];
    while let Some(cur) = stack.pop() {
        let children = doc.get(cur).map(|n| n.children.clone()).unwrap_or_default();
        if let Some(n) = ed.node(doc, cur) {
            for r in &mut n.corner_radii {
                *r = (*r * factor).max(0.0);
            }
            n.stroke.weight = (n.stroke.weight * factor).max(0.0);
            if let Some(tp) = n.kind.any_text_mut() {
                crate::richtext::apply_all(tp, |st| {
                    st.font_size = (st.font_size * factor).clamp(4.0, 512.0);
                });
            }
            if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                for p in &mut al.padding {
                    *p = (*p * factor).max(0.0).round();
                }
                al.item_spacing = (al.item_spacing * factor).round();
            }
        }
        stack.extend(children);
    }
}

// ------------------------------------------------------------------ release
fn on_release(app: &mut App, rect: ERect, inp: &Inp) {
    let cur = inp.pos.map(|p| app.view.to_page(rect.min, p)).unwrap_or(DV::ZERO);
    let inter = std::mem::take(&mut app.canvas.inter);
    app.canvas.guides.clear();
    app.canvas.measures.clear();
    match inter {
        Interaction::Pending(p) => {
            if p.was_selected {
                if p.shift {
                    app.selection.retain(|s| *s != p.node);
                } else {
                    app.select_only(p.node);
                }
            }
        }
        Interaction::Guide(g) => rulers::release(app, g),
        Interaction::Connect(c) => crate::board::connect::release(app, c),
        Interaction::Move(mut m) => {
            if release_move(app, &mut m, cur, inp.space) {
                app.run_tx(m.tx);
            } else {
                // A refused drop leaves the document as it was before the drag.
                m.tx.cancel(&mut app.doc);
            }
        }
        Interaction::Resize(r) => {
            app.run_tx(r.tx);
        }
        Interaction::Rotate(r) => {
            app.run_tx(r.tx);
        }
        Interaction::CornerRadius(c) => {
            app.run_tx(c.tx);
        }
        Interaction::AutoLayoutDrag(a) => {
            if a.moved {
                app.run_tx(a.tx);
            } else {
                a.tx.ed.revert(&mut app.doc);
                app.canvas.al_inline = Some(super::AutoLayoutInlineEdit {
                    id: a.id,
                    kind: a.kind,
                    anchor_page: a.start_page,
                    text: format!("{}", a.start_value.round() as i64),
                    request_focus: true,
                });
            }
        }
        Interaction::SmartSelectionDrag(s) => {
            app.run_tx(s.tx);
        }
        Interaction::ZoomMarquee { start, cur, alt } => {
            let s0 = app.view.to_screen(rect.min, start);
            let s1 = app.view.to_screen(rect.min, cur);
            if (s1 - s0).length() > 6.0 {
                let r = rect_from_points(start, cur);
                let vw = rect.width().max(1.0);
                let vh = rect.height().max(1.0);
                let new_zoom = (vw / r.w.max(1.0)).min(vh / r.h.max(1.0)).clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
                let cx = r.x + r.w * 0.5;
                let cy = r.y + r.h * 0.5;
                app.view.zoom = new_zoom;
                app.view.offset = v2(cx - (vw * 0.5) / new_zoom, cy - (vh * 0.5) / new_zoom);
            } else if let Some(pos) = inp.pos {
                let factor = if alt || inp.mods.alt { 0.5 } else { 2.0 };
                app.view.zoom_at(rect.min, pos, factor);
            }
            return;
        }
        Interaction::ShapeControl(s) => {
            app.run_tx(s.tx);
        }
        Interaction::Draw(mut d) => {
            if !d.dragged && (crate::board::create::types_into(d.tool) || d.tool == Tool::Stamp) {
                // a click places a board item at its default size, centred on the pointer
                let r = crate::board::create::click_rect(d.tool, d.start);
                create::apply_geom(&mut app.doc, &mut d.tx, d.id, d.parent, &create::DrawGeom { rect: r, line: None });
            } else if !d.dragged {
                let (w, h) = (theme::DEFAULT_SHAPE_SIZE, theme::DEFAULT_SHAPE_SIZE);
                let g = if matches!(d.tool, Tool::Line | Tool::Arrow) {
                    create::draw_geom(d.tool, d.start, add(d.start, v2(w, 0.0)), false, false)
                } else {
                    create::draw_geom(d.tool, d.start, add(d.start, v2(w, h)), false, false)
                };
                create::apply_geom(&mut app.doc, &mut d.tx, d.id, d.parent, &g);
            }
            let id = d.id;
            app.run_tx(d.tx);
            app.selection = vec![id];
            app.tool = if app.keep_tool { d.tool } else { Tool::Move };
            // a new sticky or shape takes text right away
            if crate::board::create::types_into(d.tool) {
                begin_text_edit(app, id);
            }
        }
        Interaction::Pencil(pts) => {
            // the stroke is smoothed into a few bezier anchors; ending on its start closes the shape
            let (dec, closed) = create::smooth_stroke(&pts, app.view.zoom);
            let brush = app.tool.is_brush().then_some(app.tool);
            let mut tx = Tx::new();
            if let Some(id) = create::commit_pen(app, &mut tx, &dec, closed && brush.is_none()) {
                if let Some(tool) = brush {
                    // marker and highlighter strokes stay open, thick and round
                    let ink = app.board.ink;
                    if let Some(n) = tx.ed.node(&mut app.doc, id) {
                        crate::board::create::brush_style(n, tool, ink);
                    }
                }
                app.run_tx(tx);
                if brush.is_none() {
                    app.selection = vec![id];
                    app.tool = Tool::Move;
                }
            }
        }
        Interaction::VectorDrag => vector::release(app),
        Interaction::Builder => builder::release(app),
        Interaction::PenDrag => {}
        Interaction::Pan | Interaction::Marquee { .. } | Interaction::None => return,
    }
    app.changed();
}

/// Renders and commits the inline Auto-Layout gap/padding popover when a pink bar is clicked (`S1-08`).
pub fn auto_layout_inline_overlay(app: &mut App, ui: &mut Ui, rect: ERect) {
    let Some(mut inline) = app.canvas.al_inline.take() else { return };
    if !app.selection.contains(&inline.id) || app.doc.get(inline.id).is_none() {
        return;
    }
    let screen_pt = app.view.to_screen(rect.min, inline.anchor_page);
    let box_rect = ERect::from_center_size(screen_pt, egui::vec2(56.0, 22.0));
    let mut commit = false;
    let mut cancel = false;

    egui::Area::new(egui::Id::new("al_inline_edit_area")).fixed_pos(box_rect.min).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
        egui::Frame::NONE
            .fill(theme::BG_SECONDARY)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(244, 63, 94)))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                let resp = ui.add_sized(
                    [48.0, 18.0],
                    egui::TextEdit::singleline(&mut inline.text).font(egui::FontId::proportional(11.0)).horizontal_align(egui::Align::Center),
                );
                if inline.request_focus {
                    resp.request_focus();
                    inline.request_focus = false;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    commit = true;
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel = true;
                } else if resp.lost_focus() {
                    commit = true;
                }
            });
    });

    if cancel {
        return;
    }
    if commit {
        commit_auto_layout_inline_value(app, inline.id, inline.kind, &inline.text);
        return;
    }
    app.canvas.al_inline = Some(inline);
}

pub fn commit_auto_layout_inline_value(app: &mut App, id: NodeId, kind: handles::AutoLayoutHandleKind, raw: &str) -> bool {
    let trimmed = raw.trim();
    let is_auto = trimmed.eq_ignore_ascii_case("auto");
    let parsed = crate::widgets::field::eval_number_expr(trimmed, None, false).or_else(|| trimmed.parse::<f32>().ok());
    if !is_auto && parsed.is_none() {
        return false;
    }
    let mut tx = Tx::new();
    let Some(n) = tx.ed.node(&mut app.doc, id) else { return false };
    let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) else {
        return false;
    };
    match kind {
        handles::AutoLayoutHandleKind::Gap(_) => {
            if is_auto {
                al.primary_align = designos_doc::LayoutAlign::SpaceBetween;
            } else if let Some(v) = parsed {
                if al.primary_align == designos_doc::LayoutAlign::SpaceBetween {
                    al.primary_align = designos_doc::LayoutAlign::Start;
                }
                al.item_spacing = v.max(0.0).round();
            }
        }
        handles::AutoLayoutHandleKind::Padding(side) => {
            if let Some(v) = parsed {
                al.padding[side.min(3)] = v.max(0.0).round();
            } else {
                return false;
            }
        }
    }
    layout::relayout_around(&mut app.doc, &mut tx.ed, id);
    app.run_tx(tx)
}

/// Settles a finished drag: reparents into the container under the pointer and records the slot
/// edits it made. Returns false when a slot limit refuses the drop (the caller undoes the drag).
fn release_move(app: &mut App, m: &mut MoveState, cur: DV, space: bool) -> bool {
    // Holding Space on release suppresses reparenting into a different container (MI-12).
    if m.moved && !m.ids.is_empty() && !space && !reparent_after_move(app, m, cur) {
        return false;
    }
    for id in m.ids.clone() {
        let Some(parent) = app.doc.get(id).and_then(|n| n.parent) else { continue };
        if let Some(blocked) = &m.dup_blocked {
            if let Some((_, err)) = blocked.iter().find(|(slot, _)| *slot == parent) {
                app.show_toast(err.to_string(), true);
                return false;
            }
            // A copy that stayed in its slot changed that slot's content.
            App::mark_slot_content_override(&mut app.doc, &mut m.tx.ed, parent);
        }
    }
    true
}

fn reparent_after_move(app: &mut App, m: &mut MoveState, cur: DV) -> bool {
    let target = create::container_at_with_index(&app.doc, active_index(app), app.page, cur, &m.ids);
    let copies = m.dup_blocked.is_some();
    let mut touched_parents = Vec::new();
    if let Some(r) = m.reorder.take() {
        // Dropped inside its auto-layout frame: move to the indicated slot.
        if let Some(&id) = m.ids.first() {
            if !app.slot_move_allowed(&[id], r.parent) {
                return false;
            }
            let old_parent = app.doc.get(id).and_then(|n| n.parent);
            let order_before = app.doc.get(r.parent).map(|p| p.children.clone());
            if m.tx.cmd(&mut app.doc, Command::Reparent { id, parent: r.parent, index: r.index }).is_ok() {
                if app.doc.get(r.parent).map(|p| &p.children) != order_before.as_ref() {
                    App::mark_slot_content_override(&mut app.doc, &mut m.tx.ed, r.parent);
                }
                if let Some(p) = old_parent.filter(|p| *p != r.parent && !copies) {
                    App::mark_slot_content_override(&mut app.doc, &mut m.tx.ed, p);
                }
                layout::relayout_around(&mut app.doc, &mut m.tx.ed, r.parent);
            }
        }
        return true;
    }
    // Layers that stay inside their container only reorder; leaving it takes them out of the flow.
    let moving: Vec<NodeId> =
        m.ids.iter().copied().filter(|id| app.doc.get(*id).is_some_and(|n| n.parent != Some(target)) && !app.doc.is_ancestor(*id, target)).collect();
    if !moving.is_empty() && !app.slot_move_allowed(&moving, target) {
        return false;
    }
    let mut moved = false;
    for id in moving {
        let old_parent = app.doc.get(id).and_then(|n| n.parent);
        let Some(inv) = app.doc.world_transform(target).invert() else { continue };
        let world = app.doc.world_transform(id);
        let index = app.doc.get(target).map(|p| p.children.len()).unwrap_or(0);
        if m.tx.cmd(&mut app.doc, Command::Reparent { id, parent: target, index }).is_ok() {
            if let Some(n) = m.tx.ed.node(&mut app.doc, id) {
                n.transform = inv * world;
            }
            moved = true;
            // The slot an Alt-copy was parked in never gained or lost anything.
            if !copies {
                touched_parents.extend(old_parent);
            }
        }
    }
    if moved {
        for p in touched_parents {
            if app.doc.get(p).is_some() {
                App::mark_slot_content_override(&mut app.doc, &mut m.tx.ed, p);
                layout::relayout_around(&mut app.doc, &mut m.tx.ed, p);
            }
        }
        App::mark_slot_content_override(&mut app.doc, &mut m.tx.ed, target);
        layout::relayout_around(&mut app.doc, &mut m.tx.ed, target);
    }
    true
}

/// 1:1 Figma `Enter` (select direct children) and `Shift+Enter` (select parent container)
/// hierarchy navigation when not editing text or in a modal.
pub fn navigate_hierarchy_enter(app: &mut App, shift: bool) -> bool {
    if app.canvas.text.is_some() || app.canvas.vedit.is_some() || app.canvas.pen.is_some() || app.cmdk.open || app.find.open || app.selection.is_empty() {
        return false;
    }
    let before = app.selection.clone();
    if shift {
        app.select_parent();
        return app.selection != before;
    }
    if let [one] = app.selection.as_slice() {
        let id = *one;
        if let Some(n) = app.doc.get(id) {
            if n.kind.is_container() {
                app.select_children();
                return app.selection != before;
            }
            if matches!(n.kind, NodeKind::Text(_) | NodeKind::Sticky(_) | NodeKind::ShapeWithText(_)) {
                begin_text_edit(app, id);
                return app.canvas.text.is_some();
            }
            if vector::is_vector(app, id) {
                return vector::enter(app, id);
            }
        }
    }
    app.select_children();
    app.selection != before
}

// ------------------------------------------------------------------ double click
fn on_double_click(app: &mut App, rect: ERect, inp: &Inp) {
    let Some(pos) = inp.pos else { return };
    let page = app.view.to_page(rect.min, pos);
    // Double-click on a ruler guide removes it (`S3-08`).
    if let Some((idx, _)) = rulers::hit_guide(app, rect, pos) {
        rulers::remove_guide(app, idx);
        return;
    }
    if app.tool == Tool::Pen {
        finish_pen(app);
        app.tool = Tool::Move;
        return;
    }
    if let Some(id) = app.canvas.text.as_ref().map(|t| t.id) {
        if ts::node_contains(app, id, page) {
            ts::select_word(app, page);
            return;
        }
    }
    if app.tool != Tool::Move {
        return;
    }
    // Double-click on a resize edge/corner handle (`S1-02`, `S1-03`, `S2-05`, `S2-06`):
    // - Text layers: right/bottom/corner handle -> Auto-Width / Auto-Height fit
    // - Auto-Layout frames & children: Fixed -> Hug -> Fill (or Alt+Double-Click -> Fill)
    if let Some(HandleHit::Resize(h)) = handle_hit(app, rect, pos) {
        if double_click_resize_edge(app, h, inp.mods.alt) {
            return;
        }
    }
    let tol = 2.0 / app.view.zoom;
    let deep = inp.mods.command || inp.mods.shift;
    let mut entered_drill = entered(app);
    entered_drill.extend(app.selection.iter().copied());
    let drilled = hit::hit_test_with_index(&app.doc, app.page, page, tol, deep, &entered_drill, active_index(app)).or_else(|| hit_under(app, page, true));
    if let Some(id) = drilled {
        let is_text = matches!(app.doc.get(id).map(|n| &n.kind), Some(NodeKind::Text(_)));
        let board_kind = app.doc.get(id).map(|n| &n.kind);
        if is_text {
            begin_text_edit(app, id);
        } else if matches!(board_kind, Some(NodeKind::Sticky(_) | NodeKind::ShapeWithText(_))) {
            // stickies and shapes take their text on double-click
            begin_text_edit(app, id);
        } else if matches!(board_kind, Some(NodeKind::Connector(_))) {
            crate::board::create::edit_label(app, id);
        } else if vector::is_vector(app, id) && (app.is_selected(id) || matches!(app.doc.get(id).map(|n| &n.kind), Some(NodeKind::Vector { .. }))) {
            // vectors open right away; basic shapes first get selected (stepping into their group)
            vector::enter(app, id);
        } else if app.is_selected(id) {
            // already the deepest hit: nothing further to enter
        } else {
            app.select_only(id);
        }
    }
}

/// Applies Figma-parity double-click resize handle behavior (`S1-02`, `S1-03`, `S2-05`, `S2-06`).
pub fn double_click_resize_edge(app: &mut App, h: Handle, alt: bool) -> bool {
    let [id] = app.selection.as_slice() else { return false };
    let id = *id;
    let Some(node) = app.doc.get(id) else { return false };

    // 1. Text node double-click resize (`S2-05`, `S2-06`)
    if matches!(node.kind, NodeKind::Text(_)) {
        let mut tx = Tx::new();
        let Some(n) = tx.ed.node(&mut app.doc, id) else { return false };
        let cur_size = n.size;
        let NodeKind::Text(t) = &mut n.kind else { return false };
        match h {
            Handle::R | Handle::L | Handle::TR | Handle::BR | Handle::TL | Handle::BL => {
                t.auto_resize = designos_doc::TextAutoResize::WidthAndHeight;
                let fitted = layout::fit_text(t, cur_size);
                n.size = fitted;
            }
            Handle::B | Handle::T => {
                t.auto_resize = designos_doc::TextAutoResize::Height;
                let fitted = layout::fit_text(t, cur_size);
                n.size.y = fitted.y;
            }
        }
        layout::relayout_around(&mut app.doc, &mut tx.ed, id);
        return app.run_tx(tx);
    }

    // 2. Auto-Layout frame or child of Auto-Layout parent (`S1-02`, `S1-03`)
    let has_own_al = node.kind.frame_props().is_some_and(|fp| fp.auto_layout.is_some());
    let in_al_parent = parent_has_auto_layout(&app.doc, id);
    if !has_own_al && !in_al_parent {
        return false;
    }

    let mut tx = Tx::new();
    let Some(n) = tx.ed.node(&mut app.doc, id) else { return false };
    let affects_x = h.affects_x();
    let affects_y = h.affects_y();

    let next_sizing = |cur: designos_doc::Sizing, can_hug: bool, can_fill: bool| -> designos_doc::Sizing {
        if alt && can_fill {
            return designos_doc::Sizing::Fill;
        }
        match cur {
            designos_doc::Sizing::Fixed => {
                if can_hug {
                    designos_doc::Sizing::Hug
                } else if can_fill {
                    designos_doc::Sizing::Fill
                } else {
                    designos_doc::Sizing::Fixed
                }
            }
            designos_doc::Sizing::Hug => {
                if can_fill {
                    designos_doc::Sizing::Fill
                } else {
                    designos_doc::Sizing::Fixed
                }
            }
            designos_doc::Sizing::Fill => {
                if can_hug {
                    designos_doc::Sizing::Hug
                } else {
                    designos_doc::Sizing::Fixed
                }
            }
        }
    };

    if affects_x {
        let cur = if has_own_al {
            let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
            if al.direction == designos_doc::LayoutDirection::Horizontal {
                if n.layout_child.horizontal == designos_doc::Sizing::Fill {
                    designos_doc::Sizing::Fill
                } else {
                    al.primary_sizing
                }
            } else if n.layout_child.horizontal == designos_doc::Sizing::Fill {
                designos_doc::Sizing::Fill
            } else {
                al.counter_sizing
            }
        } else {
            n.layout_child.horizontal
        };
        let next = next_sizing(cur, has_own_al, in_al_parent);
        n.layout_child.horizontal = next;
        if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
            let self_sizing = if next == designos_doc::Sizing::Fill && in_al_parent { designos_doc::Sizing::Fixed } else { next };
            if al.direction == designos_doc::LayoutDirection::Horizontal {
                al.primary_sizing = self_sizing;
            } else {
                al.counter_sizing = self_sizing;
            }
        }
    }
    if affects_y {
        let cur = if has_own_al {
            let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
            if al.direction == designos_doc::LayoutDirection::Vertical {
                if n.layout_child.vertical == designos_doc::Sizing::Fill {
                    designos_doc::Sizing::Fill
                } else {
                    al.primary_sizing
                }
            } else if n.layout_child.vertical == designos_doc::Sizing::Fill {
                designos_doc::Sizing::Fill
            } else {
                al.counter_sizing
            }
        } else {
            n.layout_child.vertical
        };
        let next = next_sizing(cur, has_own_al, in_al_parent);
        n.layout_child.vertical = next;
        if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
            let self_sizing = if next == designos_doc::Sizing::Fill && in_al_parent { designos_doc::Sizing::Fixed } else { next };
            if al.direction == designos_doc::LayoutDirection::Vertical {
                al.primary_sizing = self_sizing;
            } else {
                al.counter_sizing = self_sizing;
            }
        }
    }

    layout::relayout_around(&mut app.doc, &mut tx.ed, id);
    app.run_tx(tx)
}

// ------------------------------------------------------------------ pen
fn pen_press(app: &mut App, rect: ERect, pos: Pos2, page: DV, mods: Modifiers) {
    let zoom = app.view.zoom;
    if app.canvas.pen.is_none() && !app.tool.is_brush() {
        app.clear_selection();
        app.canvas.pen = Some(PenState { pts: vec![] });
    }
    let close = app
        .canvas
        .pen
        .as_ref()
        .and_then(|p| p.pts.first().filter(|_| p.pts.len() >= 3))
        .map(|f| {
            let s = app.view.to_screen(rect.min, f.pos);
            (s - pos).length() < 8.0
        })
        .unwrap_or(false);
    if app.tool == Tool::Pencil || app.tool.is_brush() {
        app.canvas.pen = None;
        app.canvas.inter = Interaction::Pencil(vec![page]);
        return;
    }
    if close {
        let pts = app.canvas.pen.take().map(|p| p.pts).unwrap_or_default();
        let mut tx = Tx::new();
        if let Some(id) = create::commit_pen(app, &mut tx, &pts, true) {
            app.run_tx(tx);
            app.selection = vec![id];
        }
        app.tool = Tool::Move;
        return;
    }
    let _ = zoom;
    if let Some(p) = &mut app.canvas.pen {
        // Shift constrains the new segment to 45 degree steps
        let at = match p.pts.last() {
            Some(last) if mods.shift => create::snap_to_45(last.pos, page),
            _ => page,
        };
        p.pts.push(PenPoint::new(at));
    }
    app.canvas.inter = Interaction::PenDrag;
}

pub fn finish_pen(app: &mut App) {
    let Some(pen) = app.canvas.pen.take() else { return };
    if matches!(app.canvas.inter, Interaction::PenDrag) {
        app.canvas.inter = Interaction::None;
    }
    if pen.pts.len() >= 2 {
        let mut tx = Tx::new();
        if let Some(id) = create::commit_pen(app, &mut tx, &pen.pts, false) {
            app.run_tx(tx);
            app.selection = vec![id];
        }
    }
}

// ------------------------------------------------------------------ images
/// Extensions offered by the image picker.
pub const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "svg"];

/// Validates image bytes (PNG, JPEG or WebP) and returns the bytes to embed
/// plus the pixel size. WebP is re-encoded as PNG so every renderer build can
/// decode it.
pub fn prepare_image(bytes: Vec<u8>) -> Result<(Vec<u8>, u32, u32), String> {
    let fmt = image::guess_format(&bytes).map_err(|_| "Unsupported image (use PNG, JPEG or WebP)".to_string())?;
    if !matches!(fmt, image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP) {
        return Err("Unsupported image (use PNG, JPEG or WebP)".into());
    }
    let img = image::load_from_memory_with_format(&bytes, fmt).map_err(|e| format!("Cannot decode image: {e}"))?;
    let (w, h) = (img.width(), img.height());
    if fmt == image::ImageFormat::WebP {
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| format!("Cannot convert image: {e}"))?;
        return Ok((out.into_inner(), w, h));
    }
    Ok((bytes, w, h))
}

fn place_image(app: &mut App, page: DV) {
    let exts: Vec<&str> = IMAGE_EXTENSIONS.iter().chain(crate::video::VIDEO_EXTENSIONS.iter()).copied().collect();
    let Some(path) = rfd::FileDialog::new().add_filter("Images and video", &exts).pick_file() else { return };
    let Ok(bytes) = std::fs::read(&path) else {
        app.show_toast("Cannot read file", true);
        return;
    };
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Image").to_string();
    if path.extension().and_then(|e| e.to_str()).is_some_and(crate::video::is_video_extension) {
        place_video_bytes(app, page, &name, bytes);
        return;
    }
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
        // Vector layers, centered on the click.
        let parent = create::container_at_with_index(&app.doc, active_index(app), app.page, page, &[]);
        match String::from_utf8(bytes) {
            Ok(text) => {
                if let Err(e) = crate::clipboard::place_svg(app, &text, &name, page, parent) {
                    app.show_toast(e, true);
                } else {
                    app.tool = Tool::Move;
                }
            }
            Err(_) => app.show_toast("The SVG is not valid UTF-8", true),
        }
        return;
    }
    place_image_bytes(app, page, &name, bytes);
}

/// Embeds a video file and inserts a rectangle showing its poster frame at `page`. The poster is
/// a normal image fill; a [`designos_doc::VideoFill`] on the layer's prototype settings makes it a
/// video in Present (see `crate::video`).
pub fn place_video_bytes(app: &mut App, page: DV, name: &str, bytes: Vec<u8>) -> Option<NodeId> {
    if designos_doc::VideoContainer::sniff(&bytes).is_none() {
        app.show_toast("Not a supported video file (MP4, MOV or WebM)", true);
        return None;
    }
    let Some((poster, iw, ih)) = crate::video::poster_png(&bytes) else {
        app.show_toast("Cannot make a poster for this video", true);
        return None;
    };
    let mut n = app.doc.images.len() + 1;
    while app.doc.images.contains_key(&format!("img-{n}")) || app.doc.images.contains_key(&format!("video-{n}")) {
        n += 1;
    }
    let (poster_key, video_key) = (format!("img-{n}"), format!("video-{n}"));
    let parent = create::container_at_with_index(&app.doc, active_index(app), app.page, page, &[]);
    let inv = app.doc.world_transform(parent).invert()?;
    let o = inv.apply(page);
    let id = app.doc.alloc_id();
    let (w, h) = (iw as f32, ih as f32);
    let scale = (480.0f32 / w.max(h)).min(1.0);
    let mut node = designos_doc::Node::rectangle(id, Rect::new(o.x, o.y, w * scale, h * scale));
    node.name = name.to_string();
    node.fills = vec![designos_doc::Paint {
        kind: designos_doc::PaintKind::Image { image_ref: poster_key.clone(), scale_mode: designos_doc::ImageScaleMode::Fill },
        opacity: 1.0,
        visible: true,
        blend: designos_doc::BlendMode::Normal,
    }];
    node.prototype.videos = vec![designos_doc::VideoFill { paint: 0, source: video_key.clone(), autoplay: true, looping: false, muted: true }];
    // Nothing is stored in the file for a drop that a slot refuses.
    if !app.slot_insert_allowed(parent, &[&node]) {
        return None;
    }
    app.doc.images.insert(poster_key, poster);
    app.doc.images.insert(video_key, bytes);
    let mut tx = Tx::new();
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![node] }).ok()?;
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    app.run_tx(tx);
    app.selection = vec![id];
    app.tool = Tool::Move;
    Some(id)
}

/// Embeds `bytes` and inserts a rectangle filled with the image at `page`.
pub fn place_image_bytes(app: &mut App, page: DV, name: &str, bytes: Vec<u8>) -> Option<NodeId> {
    let (bytes, iw, ih) = match prepare_image(bytes) {
        Ok(v) => v,
        Err(e) => {
            app.show_toast(e, true);
            return None;
        }
    };
    let key = format!("img-{}", app.doc.images.len() + 1);
    let parent = create::container_at_with_index(&app.doc, active_index(app), app.page, page, &[]);
    let inv = app.doc.world_transform(parent).invert()?;
    let o = inv.apply(page);
    let id = app.doc.alloc_id();
    let (w, h) = (iw as f32, ih as f32);
    let scale = (400.0f32 / w.max(h)).min(1.0);
    let mut n = designos_doc::Node::rectangle(id, Rect::new(o.x, o.y, w * scale, h * scale));
    n.name = name.to_string();
    n.fills = vec![designos_doc::Paint {
        kind: designos_doc::PaintKind::Image { image_ref: key.clone(), scale_mode: designos_doc::ImageScaleMode::Fill },
        opacity: 1.0,
        visible: true,
        blend: designos_doc::BlendMode::Normal,
    }];
    // Nothing is stored in the file for a drop that a slot refuses.
    if !app.slot_insert_allowed(parent, &[&n]) {
        return None;
    }
    app.doc.images.insert(key, bytes);
    let mut tx = Tx::new();
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![n] }).ok()?;
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    app.run_tx(tx);
    app.selection = vec![id];
    app.tool = Tool::Move;
    Some(id)
}

// ------------------------------------------------------------------ text
fn text_press(app: &mut App, page: DV) {
    if let Some(id) = hit_under(app, page, true) {
        if matches!(app.doc.get(id).map(|n| &n.kind), Some(NodeKind::Text(_))) {
            begin_text_edit(app, id);
            ts::press_at(app, page, false);
            return;
        }
    }
    let parent = create::container_at_with_index(&app.doc, active_index(app), app.page, page, &[]);
    let Some((node, _)) = create::new_text(app, parent, page) else { return };
    let id = node.id;
    if !app.slot_insert_allowed(parent, &[&node]) {
        return;
    }
    let mut tx = Tx::new();
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    if tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![node] }).is_err() {
        return;
    }
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    app.selection = vec![id];
    app.canvas.text = Some(TextSession::new(id, 0, 0, tx, true));
    app.changed();
}

// ------------------------------------------------------------------ hover / cursor
fn update_hover_and_cursor(app: &mut App, rect: ERect, inp: &Inp, hovered: bool) {
    let mut cursor = egui::CursorIcon::Default;
    let idle = matches!(app.canvas.inter, Interaction::None);
    app.canvas.cursor = None;

    if !idle {
        // Skip recomputing hover / hit_under while actively panning, dragging, resizing, or marquee-selecting.
        if app.is_board() && matches!(app.canvas.inter, Interaction::Connect(_)) {
            let board_page = (hovered && app.canvas.text.is_none()).then(|| inp.pos.map(|p| app.view.to_page(rect.min, p))).flatten();
            crate::board::connect::update_hover(app, board_page);
        } else {
            app.board.dots = None;
        }
        cursor = match &app.canvas.inter {
            Interaction::Pan => egui::CursorIcon::Grabbing,
            Interaction::ZoomMarquee { alt, .. } => {
                if *alt || inp.mods.alt {
                    egui::CursorIcon::ZoomOut
                } else {
                    egui::CursorIcon::ZoomIn
                }
            }
            Interaction::Resize(r) => r.handle.cursor(),
            Interaction::Rotate(_) => egui::CursorIcon::Alias,
            Interaction::CornerRadius(_) | Interaction::AutoLayoutDrag(_) | Interaction::SmartSelectionDrag(_) | Interaction::ShapeControl(_) => {
                egui::CursorIcon::PointingHand
            }
            Interaction::Draw(_) | Interaction::Pencil(_) | Interaction::PenDrag | Interaction::Connect(_) => egui::CursorIcon::Crosshair,
            Interaction::Guide(g) => {
                if g.axis == snap::GuideAxis::X {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::ResizeVertical
                }
            }
            _ => egui::CursorIcon::Default,
        };
        app.canvas.cursor = Some(cursor);
        app.hover = None;
        app.canvas.last_hover_query = None;
        return;
    }

    // board: magnet dots follow the pointer only on board documents
    let over_dot = if app.is_board() {
        let board_page = (hovered && app.canvas.text.is_none()).then(|| inp.pos.map(|p| app.view.to_page(rect.min, p))).flatten();
        crate::board::connect::update_hover(app, board_page);
        hovered && inp.pos.is_some_and(|p| crate::board::connect::dot_hit(app, rect, p).is_some())
    } else {
        false
    };

    if hovered && app.canvas.text.is_none() {
        let page = inp.pos.map(|p| app.view.to_page(rect.min, p));
        if inp.z_down {
            app.hover = None;
            app.canvas.last_hover_query = None;
            cursor = if inp.mods.alt { egui::CursorIcon::ZoomOut } else { egui::CursorIcon::ZoomIn };
            app.canvas.cursor = Some(cursor);
            return;
        }
        match app.tool {
            Tool::Move | Tool::Scale => {
                if inp.space {
                    app.hover = None;
                    app.canvas.last_hover_query = None;
                    cursor = egui::CursorIcon::Grab;
                } else if let Some(pos) = inp.pos {
                    match handle_hit(app, rect, pos) {
                        Some(HandleHit::Resize(h)) => {
                            app.hover = None;
                            app.canvas.last_hover_query = None;
                            cursor = h.cursor();
                        }
                        Some(HandleHit::Rotate(_)) => {
                            app.hover = None;
                            app.canvas.last_hover_query = None;
                            cursor = egui::CursorIcon::Alias;
                        }
                        None => {
                            if special_handle_hovered(app, rect, pos) {
                                cursor = egui::CursorIcon::PointingHand;
                            }
                            let query_key = (pos, app.canvas.tiles.rev(), inp.mods.command, app.view.zoom);
                            if app.canvas.dirty || app.canvas.last_hover_query != Some(query_key) {
                                app.hover = label_hit(app, rect, pos).or_else(|| page.and_then(|p| hit_under(app, p, inp.mods.command)));
                                app.canvas.last_hover_query = Some(query_key);
                            }
                        }
                    }
                } else {
                    app.hover = None;
                    app.canvas.last_hover_query = None;
                }
            }
            Tool::Hand => {
                app.hover = None;
                app.canvas.last_hover_query = None;
                cursor = egui::CursorIcon::Grab;
            }
            Tool::Text => {
                app.hover = None;
                app.canvas.last_hover_query = None;
                cursor = egui::CursorIcon::Text;
            }
            _ => {
                app.hover = None;
                app.canvas.last_hover_query = None;
                cursor = egui::CursorIcon::Crosshair;
            }
        }
        if let Some(c) = inp.pos.and_then(|p| rulers::cursor_at(app, rect, p)) {
            cursor = c;
            app.hover = None;
            app.canvas.last_hover_query = None;
        }
        if over_dot {
            cursor = egui::CursorIcon::Crosshair;
        }
        app.canvas.cursor = Some(cursor);
    } else if app.canvas.text.is_some() {
        app.canvas.cursor = Some(egui::CursorIcon::Text);
        app.canvas.last_hover_query = None;
    } else if !hovered {
        app.hover = None;
        app.canvas.last_hover_query = None;
    }

    let show_redlines = inp.mods.alt || app.ui.dev_mode || app.dev.show_redlines;
    if show_redlines && !app.selection.is_empty() && hovered && app.canvas.text.is_none() {
        if let Some(sel_rect) = app.selection_bounds() {
            let idx = active_index(app);
            let target_rect = app.hover.filter(|id| !app.selection.contains(id)).map(|id| super::fast_world_aabb(&app.doc, idx, id));
            let container_rect = app
                .selection
                .first()
                .and_then(|&id| app.doc.get(id)?.parent)
                .filter(|&pid| pid != app.page)
                .map(|pid| super::fast_world_aabb(&app.doc, idx, pid));
            app.canvas.measures = super::snap::compute_alt_measures(sel_rect, target_rect, container_rect);
        } else {
            app.canvas.measures.clear();
        }
    } else {
        app.canvas.measures.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_point_aligns_to_edges() {
        let t = [Rect::new(100.0, 100.0, 50.0, 50.0)];
        let (p, r) = snap_point(v2(102.0, 300.0), &t, 1.0);
        assert_eq!(p.x, 100.0);
        assert_eq!(p.y, 300.0);
        assert_eq!(r.guides.len(), 1);
    }

    fn app_with_two() -> (App, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a = d.alloc_id();
        let a = d.insert(page, designos_doc::Node::rectangle(a, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let b = d.alloc_id();
        let b = d.insert(page, designos_doc::Node::rectangle(b, Rect::new(300.0, 0.0, 100.0, 100.0))).unwrap();
        (App::new(d), a, b)
    }

    #[test]
    fn move_with_snap_and_commit() {
        let (mut app, a, b) = app_with_two();
        app.selection = vec![a];
        let mut m = start_move(&mut app, a, v2(50.0, 50.0), false).unwrap();
        let inp = Inp {
            pos: None,
            pressed: false,
            down: true,
            released: false,
            mid_pressed: false,
            mid_down: false,
            mods: Modifiers::NONE,
            space: false,
            z_down: false,
        };
        // move a so its right edge is 2 units from b's left edge -> snaps flush
        update_move(&mut app, &mut m, v2(50.0 + 198.0, 50.0), &inp);
        assert_eq!(app.doc.get(a).unwrap().transform.tx, 200.0);
        assert!(!app.canvas.guides.is_empty());
        assert!(app.run_tx(m.tx));
        assert_eq!(app.doc.get(a).unwrap().transform.tx, 200.0);
        app.undo();
        assert_eq!(app.doc.get(a).unwrap().transform.tx, 0.0);
        assert_eq!(app.doc.get(b).unwrap().transform.tx, 300.0);
    }

    #[test]
    fn shift_locks_axis_and_alt_duplicates() {
        let (mut app, a, _b) = app_with_two();
        app.selection = vec![a];
        let mut m = start_move(&mut app, a, v2(0.0, 0.0), true).unwrap();
        assert_ne!(m.ids[0], a, "alt-drag moves a duplicate");
        let inp = Inp {
            pos: None,
            pressed: false,
            down: true,
            released: false,
            mid_pressed: false,
            mid_down: false,
            mods: Modifiers { shift: true, command: true, ..Modifiers::NONE },
            space: false,
            z_down: false,
        };
        update_move(&mut app, &mut m, v2(80.0, 10.0), &inp);
        let dup = app.doc.get(m.ids[0]).unwrap();
        assert_eq!(dup.transform.tx, 80.0);
        assert_eq!(dup.transform.ty, 0.0);
        assert_eq!(app.doc.get(a).unwrap().transform.tx, 0.0);
        app.run_tx(m.tx);
        assert_eq!(app.doc.get(app.page).unwrap().children.len(), 3);
    }

    #[test]
    fn resize_single_rotated_keeps_anchor() {
        let (mut app, a, _b) = app_with_two();
        app.doc.get_mut(a).unwrap().transform = Affine::translate(100.0, 100.0) * Affine::rotate(std::f32::consts::FRAC_PI_2);
        let world0 = app.doc.world_transform(a);
        let anchor = world0.apply(v2(0.0, 0.0)); // TL stays when dragging BR
        let mut r = ResizeState {
            tx: Tx::new(),
            handle: Handle::BR,
            kind: ResizeKind::Single { id: a, world0, t0: app.doc.get(a).unwrap().transform, size0: v2(100.0, 100.0) },
            targets: vec![],
        };
        let inp = Inp {
            pos: None,
            pressed: false,
            down: true,
            released: false,
            mid_pressed: false,
            mid_down: false,
            mods: Modifiers { command: true, ..Modifiers::NONE },
            space: false,
            z_down: false,
        };
        // local (150, 120) maps to page via world0
        let target = world0.apply(v2(150.0, 120.0));
        update_resize(&mut app, &mut r, target, &inp);
        let n = app.doc.get(a).unwrap();
        assert!((n.size.x - 150.0).abs() < 1e-3 && (n.size.y - 120.0).abs() < 1e-3);
        let anchor_after = app.doc.world_transform(a).apply(v2(0.0, 0.0));
        assert!(dist(anchor, anchor_after) < 1e-3);
    }

    #[test]
    fn marquee_selects_enclosed() {
        let (mut app, a, b) = app_with_two();
        app.canvas.inter = Interaction::Marquee { start: v2(-10.0, -10.0), cur: v2(-10.0, -10.0), base: vec![] };
        let r = rect_from_points(v2(-10.0, -10.0), v2(150.0, 150.0));
        let ids = hit::marquee_select(&app.doc, app.page, &r);
        assert_eq!(ids, vec![a]);
        assert!(!ids.contains(&b));
    }

    fn encode(fmt: image::ImageFormat) -> Vec<u8> {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(8, 4, |x, y| image::Rgb([x as u8 * 30, y as u8 * 60, 90])));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, fmt).unwrap();
        out.into_inner()
    }

    #[test]
    fn prepare_image_accepts_png_jpeg_webp() {
        for fmt in [image::ImageFormat::Png, image::ImageFormat::Jpeg, image::ImageFormat::WebP] {
            let (bytes, w, h) = prepare_image(encode(fmt)).unwrap_or_else(|e| panic!("{fmt:?}: {e}"));
            assert_eq!((w, h), (8, 4), "{fmt:?}");
            if fmt == image::ImageFormat::WebP {
                assert_eq!(image::guess_format(&bytes).unwrap(), image::ImageFormat::Png, "webp is stored as png");
            }
        }
        assert!(prepare_image(b"not an image".to_vec()).is_err());
        assert!(prepare_image(b"GIF89a-not-supported".to_vec()).is_err());
    }

    #[test]
    fn place_jpeg_creates_image_fill_and_undoes() {
        let (mut app, _a, _b) = app_with_two();
        let n0 = app.doc.nodes.len();
        let id = place_image_bytes(&mut app, v2(500.0, 500.0), "photo", encode(image::ImageFormat::Jpeg)).unwrap();
        assert_eq!(app.doc.nodes.len(), n0 + 1);
        assert_eq!(app.doc.images.len(), 1);
        let n = app.doc.get(id).unwrap();
        assert_eq!(n.name, "photo");
        assert!(matches!(n.fills[0].kind, designos_doc::PaintKind::Image { .. }));
        assert!((n.size.x / n.size.y - 2.0).abs() < 1e-3);
        app.undo();
        assert_eq!(app.doc.nodes.len(), n0);
    }

    #[test]
    fn place_video_makes_a_poster_layer_with_a_video_sidecar() {
        let (mut app, _a, _b) = app_with_two();
        let jpeg = |c: u8| {
            let mut out = Vec::new();
            let img = image::RgbImage::from_pixel(32, 16, image::Rgb([c, 0, 0]));
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 95).encode_image(&img).unwrap();
            out
        };
        let mp4 = crate::video::build_mjpeg_mp4(&[jpeg(250), jpeg(10)], 32, 16, 10);
        let n0 = app.doc.nodes.len();
        let id = place_video_bytes(&mut app, v2(500.0, 500.0), "clip", mp4.clone()).unwrap();
        let n = app.doc.get(id).unwrap();
        assert_eq!(n.name, "clip");
        assert!((n.size.x / n.size.y - 2.0).abs() < 1e-3);
        // the paint is a plain image (the poster = first frame), the sidecar points at the video bytes
        let designos_doc::PaintKind::Image { image_ref, .. } = &n.fills[0].kind else { panic!() };
        let poster = image::load_from_memory(&app.doc.images[image_ref]).unwrap().to_rgba8();
        assert!(poster.get_pixel(4, 4)[0] > 200, "first frame is the poster");
        let v = &n.prototype.videos[0];
        assert_eq!((v.paint, v.autoplay, v.looping, v.muted), (0, true, false, true));
        assert_eq!(app.doc.images[&v.source], mp4);
        app.undo();
        assert_eq!(app.doc.nodes.len(), n0);
        // not a video: refused with a message, nothing inserted
        assert!(place_video_bytes(&mut app, v2(0.0, 0.0), "x", b"definitely not a video".to_vec()).is_none());
        assert_eq!(app.doc.nodes.len(), n0);
        // an unsupported codec still places, with the placeholder poster
        let mut h264 = mp4;
        let at = h264.windows(4).position(|w| w == b"jpeg").unwrap();
        h264[at..at + 4].copy_from_slice(b"avc1");
        let id = place_video_bytes(&mut app, v2(0.0, 0.0), "h264", h264).unwrap();
        assert_eq!(app.doc.get(id).unwrap().prototype.videos.len(), 1);
    }

    #[test]
    fn enter_selects_children_and_shift_enter_climbs_to_parent() {
        let (mut app, _, _) = app_with_two();
        let fid = app.doc.alloc_id();
        let frame = app.doc.insert(app.page, designos_doc::Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();
        let gid = app.doc.alloc_id();
        let mut grp = designos_doc::Node::new(gid, NodeKind::Group);
        grp.size = v2(200.0, 150.0);
        let grp = app.doc.insert(frame, grp).unwrap();
        let c1_id = app.doc.alloc_id();
        let c1 = app.doc.insert(grp, designos_doc::Node::rectangle(c1_id, Rect::new(10.0, 10.0, 60.0, 60.0))).unwrap();
        let c2_id = app.doc.alloc_id();
        let c2 = app.doc.insert(grp, designos_doc::Node::rectangle(c2_id, Rect::new(80.0, 10.0, 60.0, 60.0))).unwrap();

        // Start with Frame selected -> Enter selects direct child (Group)
        app.selection = vec![frame];
        assert!(navigate_hierarchy_enter(&mut app, false));
        assert_eq!(app.selection, vec![grp]);

        // Enter on Group selects all its direct children (c1, c2)
        assert!(navigate_hierarchy_enter(&mut app, false));
        assert_eq!(app.selection, vec![c1, c2]);

        // Shift+Enter climbs back up to Group
        assert!(navigate_hierarchy_enter(&mut app, true));
        assert_eq!(app.selection, vec![grp]);

        // Shift+Enter climbs back up to Frame
        assert!(navigate_hierarchy_enter(&mut app, true));
        assert_eq!(app.selection, vec![frame]);
    }

    #[test]
    fn double_click_drills_down_nested_containers_then_enters_vector_edit() {
        let (mut app, _, _) = app_with_two();
        app.canvas_rect = ERect::from_min_size(Pos2::ZERO, egui::Vec2::new(800.0, 600.0));
        app.view.zoom = 1.0;
        app.view.offset = DV::ZERO;

        let g1_id = app.doc.alloc_id();
        let mut g1 = designos_doc::Node::new(g1_id, NodeKind::Group);
        g1.size = v2(200.0, 200.0);
        let g1 = app.doc.insert(app.page, g1).unwrap();

        let g2_id = app.doc.alloc_id();
        let mut g2 = designos_doc::Node::new(g2_id, NodeKind::Group);
        g2.size = v2(200.0, 200.0);
        let g2 = app.doc.insert(g1, g2).unwrap();

        let r_id = app.doc.alloc_id();
        let r = app.doc.insert(g2, designos_doc::Node::rectangle(r_id, Rect::new(20.0, 20.0, 100.0, 100.0))).unwrap();

        let inp = Inp {
            pos: Some(Pos2::new(50.0, 50.0)),
            pressed: false,
            down: false,
            released: false,
            mid_pressed: false,
            mid_down: false,
            mods: Modifiers::NONE,
            space: false,
            z_down: false,
        };

        // Start with outer group g1 selected
        app.selection = vec![g1];
        let rect = app.canvas_rect;
        on_double_click(&mut app, rect, &inp);
        assert_eq!(app.selection, vec![g2], "first double-click drills down to direct child g2");

        on_double_click(&mut app, rect, &inp);
        assert_eq!(app.selection, vec![r], "second double-click drills down to leaf rectangle r");
        assert!(app.canvas.vedit.is_none());

        on_double_click(&mut app, rect, &inp);
        assert_eq!(app.canvas.vedit.as_ref().map(|v| v.id), Some(r), "third double-click enters vector edit mode on leaf");
    }

    #[test]
    fn shortcuts_sibling_nudge_resize_opacity_and_alt_measures() {
        let (mut app, a, b) = app_with_two();
        // 1. Tab / Shift+Tab sibling traversal
        app.selection = vec![b];
        app.select_sibling(true);
        assert_eq!(app.selection, vec![a]);
        app.select_sibling(true);
        assert_eq!(app.selection, vec![b]);
        app.select_sibling(false);
        assert_eq!(app.selection, vec![a]);

        // 2. Rapid 0-9 opacity buffer
        app.selection = vec![a];
        app.now = 1.0;
        app.apply_opacity_digit(5);
        assert!((app.doc.get(a).unwrap().opacity - 0.5).abs() < 1e-5);
        app.now = 2.0;
        app.apply_opacity_digit(4);
        app.now = 2.2;
        app.apply_opacity_digit(5);
        assert!((app.doc.get(a).unwrap().opacity - 0.45).abs() < 1e-5);
        app.now = 3.0;
        app.apply_opacity_digit(0);
        assert!((app.doc.get(a).unwrap().opacity - 1.0).abs() < 1e-5);
        app.now = 3.1;
        app.apply_opacity_digit(0);
        assert!((app.doc.get(a).unwrap().opacity - 0.0).abs() < 1e-5);

        // 3. Resize selection by ±1px / ±10px
        app.resize_selection_by(1.0, 10.0);
        assert_eq!(app.doc.get(a).unwrap().size, v2(101.0, 110.0));
        app.resize_selection_by(-1.0, -10.0);
        assert_eq!(app.doc.get(a).unwrap().size, v2(100.0, 100.0));

        // 4. Nudge free node vs Auto-Layout flow child reorder
        app.nudge(1.0, 10.0);
        assert_eq!(app.doc.get(a).unwrap().transform.tx, 1.0);
        assert_eq!(app.doc.get(a).unwrap().transform.ty, 10.0);

        app.selection = vec![a, b];
        app.toggle_auto_layout();
        let al_frame = app.selection[0];
        assert_eq!(app.doc.get(al_frame).unwrap().children, vec![a, b]);
        app.selection = vec![a];
        app.nudge(1.0, 0.0);
        assert_eq!(app.doc.get(al_frame).unwrap().children, vec![b, a]);
        app.nudge(-1.0, 0.0);
        assert_eq!(app.doc.get(al_frame).unwrap().children, vec![a, b]);

        // 5. Alt/Option measurement guides via compute_alt_measures & remove_auto_layout
        app.selection = vec![al_frame];
        app.remove_auto_layout();
        assert!(app.doc.get(al_frame).unwrap().kind.frame_props().unwrap().auto_layout.is_none());

        let m = super::snap::compute_alt_measures(Rect::new(0.0, 0.0, 100.0, 100.0), Some(Rect::new(150.0, 0.0, 100.0, 100.0)), None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].value, 50.0);
    }

    #[test]
    fn shape_control_handle_dragging_and_cmd_space_modifiers() {
        let (mut app, _, _) = app_with_two();
        let corners = [v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 100.0), v2(0.0, 100.0)];

        // 1. Hit-test EllipseSweep, PolygonCount, StarCount, StarInnerRatio
        let ell_id = app.doc.alloc_id();
        let ell = app.doc.insert(app.page, designos_doc::Node::ellipse(ell_id, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let ell_pts = handles::shape_control_handles(&app.doc.get(ell).unwrap().kind, &corners, 1.0);
        assert_eq!(
            handles::hit_shape_control_handle(&app.doc.get(ell).unwrap().kind, &corners, 1.0, ell_pts[0], 6.0),
            Some(handles::ShapeControlKind::EllipseSweep)
        );

        // Drag EllipseSweep to 90 degrees (bottom of ellipse: local (50, 100))
        let mut sc_ell = ShapeControlDragState {
            tx: Tx::new(),
            id: ell,
            kind: handles::ShapeControlKind::EllipseSweep,
            start_page: ell_pts[0],
            start_count: 0,
            start_ratio: 0.0,
            start_end_angle: std::f32::consts::TAU,
        };
        update_shape_control(&mut app, &mut sc_ell, v2(50.0, 100.0), true);
        app.run_tx(sc_ell.tx);
        let NodeKind::Ellipse(arc) = app.doc.get(ell).unwrap().kind else { panic!("expected ellipse") };
        assert!((arc.end - std::f32::consts::FRAC_PI_2).abs() < 1e-3);

        // Drag StarCount & StarInnerRatio
        let star_id = app.doc.alloc_id();
        let mut star_node = designos_doc::Node::new(star_id, NodeKind::Star { count: 5, inner_ratio: 0.382 });
        star_node.size = v2(100.0, 100.0);
        let star = app.doc.insert(app.page, star_node).unwrap();
        let star_pts = handles::shape_control_handles(&app.doc.get(star).unwrap().kind, &corners, 1.0);
        assert_eq!(
            handles::hit_shape_control_handle(&app.doc.get(star).unwrap().kind, &corners, 1.0, star_pts[0], 6.0),
            Some(handles::ShapeControlKind::StarCount)
        );
        assert_eq!(
            handles::hit_shape_control_handle(&app.doc.get(star).unwrap().kind, &corners, 1.0, star_pts[1], 6.0),
            Some(handles::ShapeControlKind::StarInnerRatio)
        );

        let mut sc_star_cnt = ShapeControlDragState {
            tx: Tx::new(),
            id: star,
            kind: handles::ShapeControlKind::StarCount,
            start_page: v2(50.0, 12.0),
            start_count: 5,
            start_ratio: 0.382,
            start_end_angle: 0.0,
        };
        update_shape_control(&mut app, &mut sc_star_cnt, v2(50.0, 12.0 - 24.0), false);
        app.run_tx(sc_star_cnt.tx);
        let NodeKind::Star { count, .. } = app.doc.get(star).unwrap().kind else { panic!("expected star") };
        assert_eq!(count, 8);

        let mut sc_star_ratio = ShapeControlDragState {
            tx: Tx::new(),
            id: star,
            kind: handles::ShapeControlKind::StarInnerRatio,
            start_page: star_pts[1],
            start_count: 8,
            start_ratio: 0.382,
            start_end_angle: 0.0,
        };
        update_shape_control(&mut app, &mut sc_star_ratio, v2(50.0 + 30.0, 50.0), false);
        app.run_tx(sc_star_ratio.tx);
        let NodeKind::Star { inner_ratio, .. } = app.doc.get(star).unwrap().kind else { panic!("expected star") };
        assert!((inner_ratio - 0.6).abs() < 1e-3);

        // 2. Cmd + Resize on a Frame ignores child constraints (MI-11)
        let fid = app.doc.alloc_id();
        let frame = app.doc.insert(app.page, designos_doc::Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        let cid = app.doc.alloc_id();
        let mut child = designos_doc::Node::rectangle(cid, Rect::new(20.0, 20.0, 100.0, 100.0));
        child.constraints.horizontal = designos_doc::Constraint::Scale;
        child.constraints.vertical = designos_doc::Constraint::Scale;
        let child = app.doc.insert(frame, child).unwrap();

        let mut r = ResizeState {
            tx: Tx::new(),
            handle: Handle::BR,
            kind: ResizeKind::Single { id: frame, world0: Affine::IDENTITY, t0: Affine::IDENTITY, size0: v2(200.0, 200.0) },
            targets: vec![],
        };
        let inp_cmd = Inp {
            pos: None,
            pressed: false,
            down: true,
            released: false,
            mid_pressed: false,
            mid_down: false,
            mods: Modifiers { command: true, ..Modifiers::NONE },
            space: false,
            z_down: false,
        };
        update_resize(&mut app, &mut r, v2(400.0, 400.0), &inp_cmd);
        app.run_tx(r.tx);
        assert_eq!(app.doc.get(frame).unwrap().size, v2(400.0, 400.0));
        assert_eq!(app.doc.get(child).unwrap().size, v2(100.0, 100.0), "Cmd+Resize on Frame must leave Scale-constrained child untouched");
    }
}
