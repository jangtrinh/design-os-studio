//! Canvas overlays: frame labels, hover/selection outlines, handles, size
//! badge, guides, marquee, pen preview and the text caret.

use super::handles::Handle;
use super::interact::selection_box;
use super::snap::GuideAxis;
use super::*;
use crate::theme;
use crate::util::*;
use designos_doc::{LayoutDirection, Node, NodeKind};
use egui::{Align2, Color32, Painter, Pos2, Stroke, StrokeKind, Vec2 as EV};
use std::time::Duration;

/// Component / variant / instance / slot cyan accent (`#06B6D4`, unified with `theme::CANVAS_COMPONENT`).
pub const FIGMA_COMPONENT_PURPLE: Color32 = theme::CANVAS_COMPONENT;
/// Figma UI3 primary selection & handle stroke blue (`#0D99FF`).
pub const FIGMA_BLUE: Color32 = Color32::from_rgb(0x0D, 0x99, 0xFF);
/// Figma UI3 auto-layout padding & gap handle pink (`#FF388C`).
pub const FIGMA_PINK: Color32 = Color32::from_rgb(0xFF, 0x38, 0x8C);
/// Figma UI3 export slice accent green (`#14AE5C`).
pub const FIGMA_SLICE_GREEN: Color32 = Color32::from_rgb(0x14, 0xAE, 0x5C);

/// Whether `n` is a Slice node (either imported `SLICE` or drawn with `Tool::Slice`).
pub fn is_slice_node(n: &Node) -> bool {
    matches!(&n.kind, NodeKind::Unknown { type_name } if type_name.eq_ignore_ascii_case("SLICE"))
        || (matches!(n.kind, NodeKind::Rectangle) && n.fills.is_empty() && n.stroke.paints.is_empty() && n.name.starts_with("Slice"))
}

#[inline]
fn fast_is_slot_container(doc: &designos_doc::Document, n: &Node, id: NodeId) -> bool {
    let has_own = n.component.is_slot || n.component.overrides.contains(&designos_doc::OverrideField::SlotContent) || !n.component.bindings.is_empty();
    let has_src = n.component.source.and_then(|s| doc.get(s)).is_some_and(|sn| {
        sn.component.is_slot || sn.component.overrides.contains(&designos_doc::OverrideField::SlotContent) || !sn.component.bindings.is_empty()
    });
    (has_own || has_src) && designos_doc::is_slot_container(doc, id)
}

pub fn sel_color(app: &App, id: NodeId) -> Color32 {
    let Some(n) = app.doc.get(id) else {
        return theme::CANVAS_SELECTION;
    };
    if is_slice_node(n) {
        return FIGMA_SLICE_GREEN;
    }
    if fast_is_slot_container(&app.doc, n, id) {
        return FIGMA_COMPONENT_PURPLE;
    }
    match &n.kind {
        NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } => FIGMA_COMPONENT_PURPLE,
        // Layers inside an instance share the instance's purple.
        _ if n.component.source.is_some() => FIGMA_COMPONENT_PURPLE,
        _ => theme::CANVAS_SELECTION,
    }
}

fn page_is_light(app: &App) -> bool {
    match app.doc.get(app.page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => {
            let f = |v: f32| (v * 255.0) as u8;
            theme::is_light(Color32::from_rgb(f(background.r), f(background.g), f(background.b)))
        }
        _ => true,
    }
}

/// Draws a dashed line segment between `a` and `b` in screen coordinates.
fn dashed_segment(p: &Painter, a: Pos2, b: Pos2, dash: f32, gap: f32, stroke: Stroke) {
    let d = b - a;
    let len = d.length();
    if len < 0.5 {
        return;
    }
    let dir = d / len;
    let step = (dash + gap).max(2.0);
    let mut t = 0.0f32;
    while t < len {
        let t1 = (t + dash).min(len);
        p.line_segment([a + dir * t, a + dir * t1], stroke);
        t += step;
    }
}

/// Draws a closed dashed quad given its 4 screen corners.
fn dashed_quad(p: &Painter, corners: &[Pos2; 4], dash: f32, gap: f32, stroke: Stroke) {
    for i in 0..4 {
        dashed_segment(p, corners[i], corners[(i + 1) % 4], dash, gap, stroke);
    }
}

/// Draws a hollow vector diamond glyph (`◇`) for an Instance label.
fn paint_instance_diamond(p: &Painter, center: Pos2, radius: f32, col: Color32) {
    let pts = vec![
        Pos2::new(center.x, center.y - radius),
        Pos2::new(center.x + radius, center.y),
        Pos2::new(center.x, center.y + radius),
        Pos2::new(center.x - radius, center.y),
    ];
    p.add(egui::Shape::closed_line(pts, Stroke::new(1.3, col)));
}

type LabelRectsKey = (u64, NodeId, u64, u32, u32, u32, u32, u32, u32, u32);
type LabelRectsCacheEntry = (LabelRectsKey, Vec<(NodeId, ERect)>);
type ChromeNodesKey = (u64, NodeId, u64);
type ChromeNodesCacheEntry = (ChromeNodesKey, Vec<(NodeId, bool, bool, bool)>);
thread_local! {
    static LABEL_RECTS_CACHE: std::cell::RefCell<Option<LabelRectsCacheEntry>> = const { std::cell::RefCell::new(None) };
    static CHROME_NODES_CACHE: std::cell::RefCell<Option<ChromeNodesCacheEntry>> = const { std::cell::RefCell::new(None) };
}

/// Screen rects of the name labels above top-level frames, components, instances, and sections.
pub fn label_rects(app: &App, rect: ERect) -> Vec<(NodeId, ERect)> {
    let mut out = Vec::new();
    if app.view.zoom < theme::CANVAS_LABEL_MAX_ZOOM_HIDE {
        return out;
    }
    let key: LabelRectsKey = (
        app.doc.runtime_id(),
        app.page,
        app.doc.mutation_epoch(),
        app.view.zoom.to_bits(),
        app.view.offset.x.to_bits(),
        app.view.offset.y.to_bits(),
        rect.min.x.to_bits(),
        rect.min.y.to_bits(),
        rect.max.x.to_bits(),
        rect.max.y.to_bits(),
    );
    if let Some(cached) = LABEL_RECTS_CACHE.with(|c| {
        let borrow = c.borrow();
        borrow.as_ref().filter(|(k, _)| *k == key).map(|(_, v)| v.clone())
    }) {
        return cached;
    }
    let Some(page) = app.doc.get(app.page) else { return out };
    let h = theme::FONT_SIZE_LABEL_CANVAS + 3.0;
    for id in &page.children {
        let Some(n) = app.doc.get(*id) else { continue };
        if !n.visible
            || !matches!(n.kind, NodeKind::Frame(_) | NodeKind::Section | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. })
        {
            continue;
        }
        // Top-level child of the page: its own frame rectangle in world space is directly `n.transform` applied to `n.size`
        // (with SceneIndex fallback if size is zero).
        let bb = if n.size.x > 0.0 || n.size.y > 0.0 {
            super::affine_rect_aabb(&n.transform, n.size)
        } else if let Some((x0, y0, x1, y1)) = app.canvas.gpu.scene_index().bounds_of(*id) {
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        } else {
            super::affine_rect_aabb(&n.transform, n.size)
        };
        let tl = app.view.to_screen(rect.min, v2(bb.x, bb.y));
        let label_y = tl.y - theme::CANVAS_LABEL_GAP - h;
        if label_y > rect.max.y || label_y + h < rect.min.y || tl.x > rect.max.x {
            continue;
        }
        let w_px = bb.w * app.view.zoom;
        let is_comp_like = matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. });
        let icon_w = if is_comp_like { 18.0 } else { 0.0 };
        let text_w = n.name.chars().count() as f32 * theme::FONT_SIZE_LABEL_CANVAS * 0.66 + icon_w + 16.0;
        let w = text_w.min(w_px.max(24.0));
        let r = ERect::from_min_size(Pos2::new(tl.x, label_y), EV::new(w, h));
        if r.intersects(rect) {
            out.push((*id, r));
        }
    }
    LABEL_RECTS_CACHE.with(|c| {
        *c.borrow_mut() = Some((key, out.clone()));
    });
    out
}

fn outline(app: &App, p: &Painter, rect: ERect, id: NodeId, col: Color32, width: f32) {
    // a connector is outlined along its line, not around its box
    if let Some(route) = designos_doc::connector_route(&app.doc, id) {
        let pw = parent_world(&app.doc, id);
        let pts: Vec<Pos2> = route.polyline().into_iter().map(|q| app.view.to_screen(rect.min, pw.apply(q))).collect();
        p.add(egui::Shape::line(pts, Stroke::new(width + 2.0, col.gamma_multiply(0.45))));
        return;
    }
    let corners = super::fast_world_corners(&app.doc, id).map(|c| app.view.to_screen(rect.min, c));
    let kind = app.doc.get(id).map(|n| &n.kind);
    let is_line = matches!(kind, Some(NodeKind::Line));
    if is_line {
        p.line_segment([corners[0], corners[1]], Stroke::new(width.max(1.0), col));
    } else if matches!(kind, Some(NodeKind::ComponentSet(_))) {
        dashed_quad(p, &corners, 6.0, 4.0, Stroke::new(width.max(1.25), FIGMA_COMPONENT_PURPLE));
    } else if app.doc.get(id).is_some_and(is_slice_node) {
        dashed_quad(p, &corners, 5.0, 3.0, Stroke::new(width.max(1.25), col));
    } else {
        p.add(egui::Shape::closed_line(corners.to_vec(), Stroke::new(width, col)));
    }
}

fn badge(p: &Painter, center_top: Pos2, text: &str, col: Color32) {
    let font = theme::font_canvas_strong();
    let w = crate::widgets::text_width(p, text, &font) + theme::CANVAS_BADGE_PAD_X * 2.0;
    let r = ERect::from_min_size(Pos2::new(center_top.x - w / 2.0, center_top.y), EV::new(w, theme::CANVAS_BADGE_HEIGHT));
    p.rect_filled(r, theme::cr(theme::CANVAS_BADGE_RADIUS), col);
    p.text(r.center(), Align2::CENTER_CENTER, text, font, theme::CANVAS_SIZE_BADGE_TEXT);
}

fn collect_page_chrome_nodes(doc: &designos_doc::Document, page_id: NodeId) -> Vec<(NodeId, bool, bool, bool)> {
    let mut out = Vec::new();
    let Some(page) = doc.get(page_id) else { return out };
    let huge = doc.nodes.len() > 10_000;
    let max_depth: u8 = if huge { 3 } else { u8::MAX };
    let mut stack: Vec<(NodeId, u8)> = page.children.iter().rev().map(|&cid| (cid, 1)).collect();
    while let Some((id, depth)) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        if !n.visible {
            continue;
        }
        let is_comp_set = matches!(n.kind, NodeKind::ComponentSet(_));
        let is_slot = fast_is_slot_container(doc, n, id);
        let is_slice = is_slice_node(n);
        if is_comp_set || is_slot || is_slice {
            out.push((id, is_comp_set, is_slot, is_slice));
        }
        if !n.children.is_empty() && depth < max_depth {
            for &cid in n.children.iter().rev() {
                stack.push((cid, depth.saturating_add(1)));
            }
        }
    }
    out
}

/// Page-space window (x0, y0, x1, y1) the canvas shows, with a 64 unit margin.
fn chrome_window(app: &App, rect: ERect) -> (f32, f32, f32, f32) {
    let z = app.view.zoom.max(1e-3);
    (app.view.offset.x - 64.0, app.view.offset.y - 64.0, app.view.offset.x + rect.width() / z + 64.0, app.view.offset.y + rect.height() / z + 64.0)
}

fn chrome_flags(doc: &designos_doc::Document, id: NodeId) -> Option<(NodeId, bool, bool, bool)> {
    let n = doc.get(id)?;
    let is_comp_set = matches!(n.kind, NodeKind::ComponentSet(_));
    let is_slot = fast_is_slot_container(doc, n, id);
    let is_slice = is_slice_node(n);
    (n.visible && (is_comp_set || is_slot || is_slice)).then_some((id, is_comp_set, is_slot, is_slice))
}

/// True when a node above `id` is hidden, so nothing of `id` is drawn.
fn under_hidden_ancestor(doc: &designos_doc::Document, id: NodeId) -> bool {
    let mut cur = doc.get(id).and_then(|n| n.parent);
    while let Some(c) = cur {
        let Some(n) = doc.get(c) else { return false };
        if !n.visible {
            return true;
        }
        cur = n.parent;
    }
    false
}

/// ComponentSet / slot / slice nodes to draw chrome for and to hit-test (`+` buttons), shared by the
/// painter and the overlay so a painted button is always a clickable one.
///
/// With a scene index the list is the index's own incrementally kept chrome list: no tree walk and no
/// depth limit, filtered to the window by its stored bounds. Without one (the index is rebuilding) it
/// falls back to the cached tree walk, which stops at depth 3 on documents above 10,000 nodes; the
/// selection and hover are added to that so they keep their chrome.
fn page_chrome_nodes(app: &App, win: (f32, f32, f32, f32)) -> Vec<(NodeId, bool, bool, bool)> {
    let overlaps = |(x0, y0, x1, y1): (f32, f32, f32, f32)| x0 <= win.2 && win.0 <= x1 && y0 <= win.3 && win.1 <= y1;
    if let Some(idx) = super::interact::active_index(app) {
        return idx
            .chrome_nodes()
            .iter()
            .filter(|&&id| idx.bounds_of(id).is_some_and(overlaps))
            .filter_map(|&id| chrome_flags(&app.doc, id))
            .filter(|&(id, ..)| !under_hidden_ancestor(&app.doc, id))
            .collect();
    }
    let key = (app.doc.runtime_id(), app.page, app.doc.mutation_epoch());
    let in_transform_drag = matches!(
        app.canvas.inter,
        Interaction::Move(_)
            | Interaction::Resize(_)
            | Interaction::Rotate(_)
            | Interaction::CornerRadius(_)
            | Interaction::AutoLayoutDrag(_)
            | Interaction::SmartSelectionDrag(_)
    );
    let mut chrome_nodes = CHROME_NODES_CACHE.with(|c| {
        let mut borrow = c.borrow_mut();
        if let Some((cached_key, nodes)) = borrow.as_mut() {
            if *cached_key == key || (in_transform_drag && cached_key.0 == key.0 && cached_key.1 == key.1) {
                *cached_key = key;
                return nodes.clone();
            }
        }
        let built = collect_page_chrome_nodes(&app.doc, app.page);
        *borrow = Some((key, built.clone()));
        built
    });
    if app.doc.nodes.len() > 10_000 {
        for &extra in app.selection.iter().chain(app.hover.iter()).chain(app.hover_layer.iter()) {
            if let Some(entry) = chrome_flags(&app.doc, extra) {
                if !chrome_nodes.iter().any(|(cid, ..)| *cid == extra) {
                    chrome_nodes.push(entry);
                }
            }
        }
    }
    chrome_nodes
}

/// Paints persistent dashed borders for `ComponentSet`, component slots, and `Slice` nodes on the active page.
fn draw_component_slot_and_slice_chrome(app: &App, p: &Painter, rect: ERect) {
    if app.view.zoom < 0.15 {
        return;
    }
    let win = chrome_window(app, rect);
    let overlaps_win = |(x0, y0, x1, y1): (f32, f32, f32, f32)| x0 <= win.2 && win.0 <= x1 && y0 <= win.3 && win.1 <= y1;
    let idx = super::interact::active_index(app);
    let chrome_nodes = page_chrome_nodes(app, win);

    for (id, is_comp_set, is_slot, is_slice) in chrome_nodes {
        let Some(n) = app.doc.get(id) else { continue };
        if !n.visible {
            continue;
        }
        if let Some(b) = idx.and_then(|i| i.bounds_of(id)) {
            if !overlaps_win(b) {
                continue;
            }
        }
        let world = super::fast_world_transform(&app.doc, id);
        if idx.is_none() {
            let bb = super::affine_rect_aabb(&world, n.size);
            if !overlaps_win((bb.x, bb.y, bb.x + bb.w, bb.y + bb.h)) {
                continue;
            }
        }
        let corners =
            [world.apply(DV::ZERO), world.apply(DV::new(n.size.x, 0.0)), world.apply(DV::new(n.size.x, n.size.y)), world.apply(DV::new(0.0, n.size.y))]
                .map(|c| app.view.to_screen(rect.min, c));
        if is_comp_set && !app.is_selected(id) {
            dashed_quad(p, &corners, 6.0, 4.0, Stroke::new(1.25, FIGMA_COMPONENT_PURPLE.gamma_multiply(0.85)));
        }
        if is_slot {
            let show_placeholder = designos_doc::should_show_empty_slot_placeholder(&app.doc, id);
            if show_placeholder {
                p.add(egui::Shape::convex_polygon(corners.to_vec(), FIGMA_COMPONENT_PURPLE.gamma_multiply(0.08), Stroke::NONE));
                dashed_quad(p, &corners, 4.0, 3.0, Stroke::new(1.25, FIGMA_COMPONENT_PURPLE.gamma_multiply(0.85)));
                if let Some(plus_r) = empty_slot_plus_button_rect(app, rect, id) {
                    p.rect_filled(plus_r, theme::cr_pill(plus_r.height()), FIGMA_COMPONENT_PURPLE);
                    crate::icons::paint(p, crate::icons::Icon::Plus, plus_r.center(), 12.0, Color32::WHITE);
                    let s_rect = ERect::from_two_pos(corners[0], corners[2]);
                    if s_rect.width() >= 64.0 && s_rect.height() >= 48.0 {
                        let font = theme::font_canvas_strong();
                        p.text(
                            Pos2::new(plus_r.center().x, plus_r.bottom() + theme::SPACE_INLINE),
                            Align2::CENTER_CENTER,
                            &n.name,
                            font,
                            FIGMA_COMPONENT_PURPLE,
                        );
                    }
                }
            } else if !n.children.is_empty() || app.is_selected(id) || app.hover == Some(id) || app.hover_layer == Some(id) {
                dashed_quad(p, &corners, 4.0, 3.0, Stroke::new(1.0, FIGMA_COMPONENT_PURPLE.gamma_multiply(0.75)));
            }
            if app.is_selected(id) || app.hover == Some(id) || app.hover_layer == Some(id) {
                let font = theme::font_canvas_strong();
                let w = crate::widgets::text_width(p, "Slot", &font) + 8.0;
                let pill = ERect::from_min_size(Pos2::new(corners[0].x, corners[0].y - 17.0), EV::new(w, 15.0));
                p.rect_filled(pill, theme::cr(3), FIGMA_COMPONENT_PURPLE);
                p.text(pill.center(), Align2::CENTER_CENTER, "Slot", font, Color32::WHITE);
            }
        }
        if is_slice {
            let col = if app.is_selected(id) || app.hover == Some(id) { Color32::from_rgb(0xF5, 0x9E, 0x0B) } else { FIGMA_SLICE_GREEN };
            dashed_quad(p, &corners, 5.0, 3.0, Stroke::new(1.0, col));
            let font = theme::font_canvas_strong();
            let w = crate::widgets::text_width(p, "Slice", &font) + 8.0;
            let pill = ERect::from_min_size(Pos2::new(corners[0].x, corners[0].y - 16.0), EV::new(w, 14.0));
            p.rect_filled(pill, theme::cr(2), col);
            p.text(pill.center(), Align2::CENTER_CENTER, "Slice", font, Color32::WHITE);
        }
    }
}

/// Screen rect of the centered `+` button inside an empty slot placeholder,
/// or `None` when `id` should not display an empty-slot placeholder.
pub fn empty_slot_plus_button_rect(app: &App, rect: ERect, id: NodeId) -> Option<ERect> {
    if !designos_doc::should_show_empty_slot_placeholder(&app.doc, id) {
        return None;
    }
    let n = app.doc.get(id)?;
    let world = super::fast_world_transform(&app.doc, id);
    let c0 = app.view.to_screen(rect.min, world.apply(DV::ZERO));
    let c2 = app.view.to_screen(rect.min, world.apply(DV::new(n.size.x, n.size.y)));
    let s_rect = ERect::from_two_pos(c0, c2);
    if s_rect.width() < 16.0 || s_rect.height() < 16.0 {
        return None;
    }
    Some(ERect::from_center_size(s_rect.center(), EV::splat(22.0)))
}

/// Records and handles clicks on the on-canvas `+` buttons of empty slot placeholders,
/// and renders the active preferred-instance picker popup.
pub fn empty_slot_picker_overlay(app: &mut App, ui: &mut Ui, rect: ERect) {
    if app.view.zoom >= 0.15 && app.history.viewing.is_none() {
        let chrome_nodes = page_chrome_nodes(app, chrome_window(app, rect));
        let blocked = crate::widgets::pointer_over_popup(ui.ctx());
        let mut clicked_slot: Option<(NodeId, ERect)> = None;
        for (id, _, is_slot, _) in chrome_nodes {
            if !is_slot {
                continue;
            }
            let Some(plus_r) = empty_slot_plus_button_rect(app, rect, id) else { continue };
            if !rect.intersects(plus_r) {
                continue;
            }
            crate::widgets::record(ui, &format!("canvas-slot-plus:{}", id.0), plus_r);
            let resp = ui.interact(plus_r, ui.id().with(("canvas-slot-plus", id)), if blocked { Sense::hover() } else { Sense::click() });
            if resp.hovered() && !blocked {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let ptr_clicked = !blocked
                && ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary) && i.pointer.interact_pos().is_some_and(|p| plus_r.contains(p)));
            if resp.clicked() || ptr_clicked {
                clicked_slot = Some((id, plus_r));
            }
        }
        if let Some((slot_id, plus_r)) = clicked_slot {
            app.selection = vec![slot_id];
            app.ui.slot_picker = if app.ui.slot_picker.map(|(s, _)| s) == Some(slot_id) { None } else { Some((slot_id, plus_r)) };
        }
    }
    crate::panels::components::show_slot_picker_popup(app, ui.ctx());
}

/// Screen rect and status of the on-canvas Dev Status pill badge right after a top-level frame label (`S3-12`).
pub fn dev_status_badge_rect(id: NodeId, n: &Node, label_rect: ERect) -> Option<(ERect, designos_doc::DevStatus)> {
    let status = n.dev.status?;
    let _ = id;
    let text_w = status.label().chars().count() as f32 * theme::FONT_SIZE_CANVAS * 0.62 + 12.0;
    let h = 16.0;
    let r = ERect::from_min_size(Pos2::new(label_rect.right() + 6.0, label_rect.center().y - h * 0.5), EV::new(text_w, h));
    Some((r, status))
}

/// The grid frames whose header pills and `+` buttons a click can reach: the selected ones and the hovered one.
/// What is painted and what is hit-tested both come from this list.
fn grid_track_candidates(app: &App) -> Vec<NodeId> {
    let mut candidates = app.selection.clone();
    if let Some(h) = app.hover {
        if !candidates.contains(&h) {
            candidates.push(h);
        }
    }
    candidates
}

/// Paints the Figma UI3 2D CSS Grid Overlay (`LayoutDirection::Grid`) for selected/hovered grid containers.
fn draw_css_grid_overlays(app: &App, p: &Painter, rect: ERect) {
    let mut grid_ids: Vec<NodeId> = Vec::new();
    let mut push_unique = |id: NodeId| {
        if !grid_ids.contains(&id) && super::grids::node_css_grid_overlay(&app.doc, id).is_some() {
            grid_ids.push(id);
        }
    };
    for &id in &app.selection {
        push_unique(id);
        if let Some(parent) = app.doc.get(id).and_then(|n| n.parent) {
            push_unique(parent);
        }
    }
    for h in [app.hover, app.hover_layer].into_iter().flatten() {
        push_unique(h);
    }

    let reachable = grid_track_candidates(app);
    let cell_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x0D, 0x99, 0xFF, 140));
    let gap_fill = Color32::from_rgba_unmultiplied(0xFF, 0x38, 0x8C, 36);
    let pill_bg = Color32::from_rgba_unmultiplied(0x1E, 0x1E, 0x1E, 230);
    let pill_border = Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x0D, 0x99, 0xFF, 180));

    for grid_id in grid_ids {
        let Some(n) = app.doc.get(grid_id) else { continue };
        let Some(ov) = super::grids::node_css_grid_overlay(&app.doc, grid_id) else { continue };
        let w_xform = super::fast_world_transform(&app.doc, grid_id);
        let to_s = |x: f32, y: f32| app.view.to_screen(rect.min, w_xform.apply(v2(x, y)));
        let [pt, pr, pb, pl] = ov.padding;
        let inner_x0 = pl;
        let inner_x1 = (n.size.x - pr).max(pl);
        let inner_y0 = pt;
        let inner_y1 = (n.size.y - pb).max(pt);

        // 1. Subtle tinted gap strips between columns and rows.
        if ov.col_gap > 0.0 && ov.cols > 1 {
            for c in 0..(ov.cols - 1) {
                let gx0 = ov.col_x[c] + ov.col_widths[c];
                let gx1 = gx0 + ov.col_gap;
                let quad = vec![to_s(gx0, inner_y0), to_s(gx1, inner_y0), to_s(gx1, inner_y1), to_s(gx0, inner_y1)];
                p.add(egui::Shape::convex_polygon(quad, gap_fill, Stroke::NONE));
            }
        }
        if ov.row_gap > 0.0 && ov.rows > 1 {
            for r in 0..(ov.rows - 1) {
                let gy0 = ov.row_y[r] + ov.row_heights[r];
                let gy1 = gy0 + ov.row_gap;
                let quad = vec![to_s(inner_x0, gy0), to_s(inner_x1, gy0), to_s(inner_x1, gy1), to_s(inner_x0, gy1)];
                p.add(egui::Shape::convex_polygon(quad, gap_fill, Stroke::NONE));
            }
        }

        // 2. Dotted/dashed cell boundary lines for every (row, col) cell rect.
        for cell in &ov.cells {
            let r = &cell.rect;
            let c_pts = [to_s(r.x, r.y), to_s(r.x + r.w, r.y), to_s(r.x + r.w, r.y + r.h), to_s(r.x, r.y + r.h)];
            dashed_quad(p, &c_pts, 3.0, 3.0, cell_stroke);
        }

        // 3. Compact Column & Row Track Header Pills outside top/left edges when zoom >= 0.4.
        // Only the grids a click can reach get pills and `+` buttons, so a drawn control always works.
        if app.view.zoom >= 0.4 && reachable.contains(&grid_id) {
            let font = theme::font_canvas_strong();
            for (is_col, idx, _hit, pill) in grid_pill_rects_of(&ov, to_s) {
                let label = super::grids::format_grid_track(if is_col { ov.col_tracks[idx] } else { ov.row_tracks[idx] });
                p.rect_filled(pill, theme::cr(3), pill_bg);
                p.rect_stroke(pill, theme::cr(3), pill_border, StrokeKind::Inside);
                // a pill squeezed by its neighbours clips its label instead of spilling over them
                p.with_clip_rect(pill).text(pill.center(), Align2::CENTER_CENTER, label, font.clone(), FIGMA_BLUE);
            }
            for (_, add) in grid_add_buttons(&ov, n.size, to_s) {
                p.rect_filled(add, theme::cr(3), pill_bg);
                p.rect_stroke(add, theme::cr(3), pill_border, StrokeKind::Inside);
                p.text(add.center(), Align2::CENTER_CENTER, "+", font.clone(), FIGMA_BLUE);
            }
        }
    }

    // 4. Child span badge (`2×1`) when a selected child inside a Grid frame has col_span > 1 or row_span > 1.
    for &id in &app.selection {
        let Some(n) = app.doc.get(id) else { continue };
        let Some(pid) = n.parent else { continue };
        let is_grid_parent = app
            .doc
            .get(pid)
            .and_then(|pn| pn.kind.frame_props())
            .and_then(|fp| fp.auto_layout.as_ref())
            .is_some_and(|al| al.direction == LayoutDirection::Grid);
        if !is_grid_parent {
            continue;
        }
        let cs = n.layout_child.col_span.max(1);
        let rs = n.layout_child.row_span.max(1);
        if cs > 1 || rs > 1 {
            let corners = super::fast_world_corners(&app.doc, id).map(|c| app.view.to_screen(rect.min, c));
            let text = format!("{cs}×{rs}");
            let font = theme::font_canvas_strong();
            let tw = crate::widgets::text_width(p, &text, &font) + 8.0;
            let pill = ERect::from_min_size(Pos2::new(corners[1].x - tw - 4.0, corners[1].y + 4.0), EV::new(tw, 15.0));
            p.rect_filled(pill, theme::cr(3), FIGMA_COMPONENT_PURPLE);
            p.text(pill.center(), Align2::CENTER_CENTER, text, font, Color32::WHITE);
        }
    }
}

/// Painted height (screen px) of a track header pill.
const GRID_PILL_HEIGHT: f32 = 15.0;

/// Minimum hit-target dimension (screen px) for interactive track header pills (`P3` 24x24 floor).
const GRID_PILL_HIT_FLOOR: f32 = 24.0;

/// Screen rects `(is_col, track, hit_rect, paint_rect)` of the header pills of one grid: column pills sit above
/// the frame, row pills left of it, each centred on its track. Every `hit_rect` stays at least 24x24 px (`nearest_pill`
/// picks the nearest centre when narrow tracks overlap), while `paint_rect` clips the painted pill and label to the
/// track pitch so painted pills on the same axis never spill over each other.
fn grid_pill_rects_of(ov: &super::grids::CssGridOverlay, to_s: impl Fn(f32, f32) -> Pos2) -> Vec<(bool, usize, ERect, ERect)> {
    let label_w = |t: designos_doc::GridTrack| (super::grids::format_grid_track(t).chars().count() as f32 * 6.0 + 10.0).max(GRID_PILL_HIT_FLOOR);
    // room a pill has along its axis: the distance to the nearest neighbour centre
    let room = |centres: &[Pos2], i: usize| {
        let before = i.checked_sub(1).map_or(f32::INFINITY, |j| centres[i].distance(centres[j]));
        let after = centres.get(i + 1).map_or(f32::INFINITY, |next| centres[i].distance(*next));
        before.min(after)
    };
    let col_centres: Vec<Pos2> = (0..ov.cols).map(|c| to_s(ov.col_x[c] + ov.col_widths[c] * 0.5, 0.0)).collect();
    let row_centres: Vec<Pos2> = (0..ov.rows).map(|r| to_s(0.0, ov.row_y[r] + ov.row_heights[r] * 0.5)).collect();
    let mut out = Vec::with_capacity(ov.cols + ov.rows);
    for (c, &centre) in col_centres.iter().enumerate() {
        let paint_w = label_w(ov.col_tracks[c]).min(room(&col_centres, c));
        let hit_w = paint_w.max(GRID_PILL_HIT_FLOOR);
        let center = Pos2::new(centre.x, centre.y - 12.0);
        let hit = ERect::from_center_size(center, EV::new(hit_w, GRID_PILL_HIT_FLOOR));
        let paint = ERect::from_center_size(center, EV::new(paint_w, GRID_PILL_HEIGHT));
        out.push((true, c, hit, paint));
    }
    for (r, &centre) in row_centres.iter().enumerate() {
        let w = label_w(ov.row_tracks[r]);
        let paint_h = GRID_PILL_HEIGHT.min(room(&row_centres, r));
        let hit_h = paint_h.max(GRID_PILL_HIT_FLOOR);
        let center = Pos2::new(centre.x - w * 0.5 - 5.0, centre.y);
        let hit = ERect::from_center_size(center, EV::new(w.max(GRID_PILL_HIT_FLOOR), hit_h));
        let paint = ERect::from_center_size(center, EV::new(w, paint_h));
        out.push((false, r, hit, paint));
    }
    out
}

/// Returns `(grid_frame_id, is_col, track_index, screen_rect)` for every visible 2D CSS Grid
/// column/row track header pill on selected/hovered grid frames (`#18`).
pub fn grid_track_pill_rects(app: &App, rect: ERect) -> Vec<(NodeId, bool, usize, ERect)> {
    let mut out = Vec::new();
    if app.view.zoom < 0.4 {
        return out;
    }
    for id in grid_track_candidates(app) {
        let Some(ov) = super::grids::node_css_grid_overlay(&app.doc, id) else { continue };
        let w_xform = super::fast_world_transform(&app.doc, id);
        let to_s = |x: f32, y: f32| app.view.to_screen(rect.min, w_xform.apply(v2(x, y)));
        out.extend(grid_pill_rects_of(&ov, to_s).into_iter().map(|(is_col, idx, hit, _)| (id, is_col, idx, hit)));
    }
    out
}

/// The header pill under `pos`. Of the pills that contain it, the one whose centre is nearest wins, so the answer
/// never depends on list order.
pub fn grid_track_pill_at(app: &App, rect: ERect, pos: Pos2) -> Option<(NodeId, bool, usize, ERect)> {
    nearest_pill(grid_track_pill_rects(app, rect), pos)
}

/// Of `pills`, the one that contains `pos` and has its centre nearest to it.
pub fn nearest_pill(pills: impl IntoIterator<Item = (NodeId, bool, usize, ERect)>, pos: Pos2) -> Option<(NodeId, bool, usize, ERect)> {
    pills.into_iter().filter(|(_, _, _, r)| r.contains(pos)).min_by(|a, b| a.3.center().distance_sq(pos).total_cmp(&b.3.center().distance_sq(pos)))
}

/// Side (screen px) of the `+` add-track buttons: the pointer-target floor of the design system.
const GRID_ADD_BUTTON_SIZE: f32 = 24.0;

/// The `+` add-column / add-row buttons of one grid frame as `(is_col, screen_rect)`. An axis that already
/// has the most tracks the solver lays out has no button.
fn grid_add_buttons(ov: &super::grids::CssGridOverlay, frame: designos_doc::Vec2, to_s: impl Fn(f32, f32) -> Pos2) -> Vec<(bool, ERect)> {
    let [pt, pr, pb, pl] = ov.padding;
    let size = EV::splat(GRID_ADD_BUTTON_SIZE);
    let mut out = Vec::with_capacity(2);
    if ov.cols < designos_doc::MAX_GRID_TRACKS {
        // right of the column pills, level with them
        let top_right = to_s((frame.x - pr).max(pl), 0.0);
        out.push((true, ERect::from_min_size(Pos2::new(top_right.x + 4.0, top_right.y - 12.0 - GRID_ADD_BUTTON_SIZE * 0.5), size)));
    }
    if ov.rows < designos_doc::MAX_GRID_TRACKS {
        // under the row pills, ending where they end
        let bot_left = to_s(0.0, (frame.y - pb).max(pt));
        out.push((false, ERect::from_min_size(Pos2::new(bot_left.x - 5.0 - GRID_ADD_BUTTON_SIZE, bot_left.y + 4.0), size)));
    }
    out
}

/// Returns `(grid_frame_id, is_col, screen_rect)` for the on-canvas `+` add-column / add-row buttons (`#144`)
/// of the selected and hovered grid frames.
pub fn grid_track_add_button_rects(app: &App, rect: ERect) -> Vec<(NodeId, bool, ERect)> {
    let mut out = Vec::new();
    if app.view.zoom < 0.4 {
        return out;
    }
    for id in grid_track_candidates(app) {
        let Some(n) = app.doc.get(id) else { continue };
        let Some(ov) = super::grids::node_css_grid_overlay(&app.doc, id) else { continue };
        let w_xform = super::fast_world_transform(&app.doc, id);
        let to_s = |x: f32, y: f32| app.view.to_screen(rect.min, w_xform.apply(v2(x, y)));
        out.extend(grid_add_buttons(&ov, n.size, to_s).into_iter().map(|(is_col, r)| (id, is_col, r)));
    }
    out
}

/// Returns the screen rect for the on-canvas `"Tidy up"` action pill when 3+ nodes are selected (`#16`).
pub fn tidy_up_pill_rect(app: &App, rect: ERect) -> Option<ERect> {
    if app.selection.len() < 3 || app.canvas.text.is_some() || app.canvas.vedit.is_some() {
        return None;
    }
    let (corners, _) = selection_box(app)?;
    let s = corners.map(|c| app.view.to_screen(rect.min, c));
    let right = s.iter().map(|c| c.x).fold(f32::MIN, f32::max);
    let bottom = s.iter().map(|c| c.y).fold(f32::MIN, f32::max);
    Some(ERect::from_min_size(Pos2::new(right - 52.0, bottom + 6.0), EV::new(52.0, 18.0)))
}

/// Paints Corner Radius Handles, Pink Auto-Layout Padding/Gap Handles, and Shape Control Handles for a single selected node.
fn draw_single_node_handles(app: &App, p: &Painter, rect: ERect, id: NodeId, dvs: &[DV; 4]) {
    let Some(n) = app.doc.get(id) else { return };
    let z = app.view.zoom;

    // 1. On-Canvas Corner Radius Handles (4px white circle with #0D99FF 1.5px stroke).
    if handles::supports_corner_radius_handles(&n.kind) {
        if let Some(cr_pts) = handles::corner_radius_handle_positions(dvs, n.corner_radii, z) {
            for pt in cr_pts {
                let c = Pos2::new(pt.x, pt.y);
                p.circle_filled(c, 4.0, Color32::WHITE);
                p.circle_stroke(c, 4.0, Stroke::new(1.5, FIGMA_BLUE));
            }
        }
    }

    // 2. On-Canvas Auto-Layout Padding & Gap Handles (`Horizontal` / `Vertical`).
    if let Some(al) = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) {
        if matches!(al.direction, LayoutDirection::Horizontal | LayoutDirection::Vertical) {
            let mut child_rects = Vec::new();
            for &cid in &n.children {
                if let Some(c) = app.doc.get(cid) {
                    if c.visible && !c.layout_child.absolute {
                        child_rects.push(Rect::new(c.transform.tx, c.transform.ty, c.size.x, c.size.y));
                    }
                }
            }
            let w_xform = super::fast_world_transform(&app.doc, id);
            for hnd in handles::auto_layout_handles(al, n.size, &child_rects, z) {
                let sc = app.view.to_screen(rect.min, w_xform.apply(hnd.local_center));
                let pill = ERect::from_center_size(sc, EV::new(hnd.pill_size.x, hnd.pill_size.y));
                p.rect_filled(pill.expand(0.75), theme::cr(2), Color32::WHITE);
                p.rect_filled(pill, theme::cr(2), FIGMA_PINK);
            }
        }
    }

    // 3. On-Canvas Arc / Star / Polygon Control Handles.
    for pt in handles::shape_control_handles(&n.kind, dvs, z) {
        let c = Pos2::new(pt.x, pt.y);
        p.circle_filled(c, 4.0, Color32::WHITE);
        p.circle_stroke(c, 4.0, Stroke::new(1.5, FIGMA_BLUE));
    }

    // 4. Floating live parameter badge during CornerRadius, AutoLayoutDrag, or ShapeControl interaction.
    match &app.canvas.inter {
        Interaction::CornerRadius(cr) if cr.id == id => {
            let r_val = n.corner_radii[cr.corner].round() as i64;
            let c = Pos2::new(dvs[cr.corner].x, dvs[cr.corner].y - 22.0);
            badge(p, c, &format!("R {r_val}"), FIGMA_BLUE);
        }
        Interaction::AutoLayoutDrag(ad) if ad.id == id => {
            if let Some(al) = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) {
                let text = match ad.kind {
                    handles::AutoLayoutHandleKind::Padding(side) => format!("Pad {}", al.padding[side.min(3)].round() as i64),
                    handles::AutoLayoutHandleKind::Gap(_) => format!("Gap {}", al.item_spacing.round() as i64),
                };
                let top_cx = (dvs[0].x + dvs[1].x) * 0.5;
                let top_cy = dvs[0].y.min(dvs[1].y) - 22.0;
                badge(p, Pos2::new(top_cx, top_cy), &text, FIGMA_PINK);
            }
        }
        Interaction::ShapeControl(sc) if sc.id == id => {
            let text = match (&n.kind, sc.kind) {
                (NodeKind::Ellipse(arc), handles::ShapeControlKind::EllipseSweep) => {
                    let deg = (arc.end - arc.start).to_degrees().round() as i64;
                    format!("Sweep {deg}°")
                }
                (NodeKind::Ellipse(arc), handles::ShapeControlKind::EllipseInnerRadius) => {
                    format!("Ratio {}%", (arc.inner_radius * 100.0).round() as i64)
                }
                (NodeKind::Ellipse(arc), handles::ShapeControlKind::EllipseStartAngle) => {
                    format!("Start {}°", arc.start.to_degrees().round() as i64)
                }
                (NodeKind::Polygon { count }, _) | (NodeKind::Star { count, .. }, handles::ShapeControlKind::StarCount) => {
                    format!("{count} pts")
                }
                (NodeKind::Star { inner_ratio, .. }, handles::ShapeControlKind::StarInnerRatio) => {
                    format!("{}%", (inner_ratio * 100.0).round() as i64)
                }
                _ => String::new(),
            };
            if !text.is_empty() {
                let top_cx = (dvs[0].x + dvs[1].x) * 0.5;
                let top_cy = dvs[0].y.min(dvs[1].y) - 22.0;
                badge(p, Pos2::new(top_cx, top_cy), &text, FIGMA_BLUE);
            }
        }
        _ => {}
    }
}

pub fn overlays(app: &App, ui: &Ui, rect: ERect) {
    let p = ui.painter().with_clip_rect(rect);
    let to_s = |v: DV| app.view.to_screen(rect.min, v);
    let light = page_is_light(app);
    let sel_set: Option<std::collections::HashSet<NodeId>> = (app.selection.len() > 16).then(|| app.selection.iter().copied().collect());
    let is_sel = |id: NodeId| sel_set.as_ref().map_or_else(|| app.is_selected(id), |s| s.contains(&id));

    // Persistent ComponentSet dashed border, Slot indicator, and Slice export border.
    draw_component_slot_and_slice_chrome(app, &p, rect);

    // Frame / Component / Instance / Section labels + Dev status pills (`S3-12`).
    let lab_col = if light { theme::CANVAS_LABEL } else { theme::CANVAS_LABEL_DARK };
    let label_clip = rect.expand(40.0);
    for (id, r) in label_rects(app, rect) {
        if !label_clip.intersects(r) {
            continue;
        }
        let Some(n) = app.doc.get(id) else { continue };
        let selected = is_sel(id);
        let hovered = app.hover == Some(id) || app.hover_layer == Some(id);
        let is_comp = matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_));
        let is_inst = matches!(n.kind, NodeKind::Instance { .. });
        let is_comp_like = is_comp || is_inst;
        // Main components, component sets, and instances carry #06B6D4 cyan label + diamond glyph.
        let col = if selected || hovered || is_comp_like { sel_color(app, id) } else { lab_col };
        let dx = if is_comp_like { 18.0 } else { 0.0 };
        if is_comp {
            crate::icons::paint(&p, crate::icons::Icon::Component, Pos2::new(r.left() + 7.0, r.center().y), 14.0, col);
        } else if is_inst {
            paint_instance_diamond(&p, Pos2::new(r.left() + 7.0, r.center().y), 5.0, col);
        }
        let clean_name = n.name.trim_start_matches("❖ ").trim_start_matches("◇ ");
        let name = crate::widgets::elide(&p, clean_name, &theme::font_body(), (r.width() - dx + 8.0).max(8.0));
        p.text(Pos2::new(r.left() + dx, r.center().y), Align2::LEFT_CENTER, name, theme::font_body(), col);
        if let Some((badge_r, status)) = dev_status_badge_rect(id, n, r) {
            let badge_col = match status {
                designos_doc::DevStatus::ReadyForDev => theme::DEV_READY,
                designos_doc::DevStatus::Completed => theme::DEV_COMPLETED,
                designos_doc::DevStatus::Changed => theme::DEV_CHANGED,
            };
            p.rect_filled(badge_r, theme::cr(3), badge_col);
            p.text(badge_r.center(), Align2::CENTER_CENTER, status.label(), theme::font_canvas_strong(), Color32::WHITE);
        }
    }

    // 2D CSS Grid Overlay (`LayoutDirection::Grid`).
    draw_css_grid_overlays(app, &p, rect);

    // Dev Mode Box-Model Padding Tint (`#87`): semi-transparent emerald padding bands on hovered/selected Auto-Layout frame.
    if app.ui.dev_mode {
        let mut dev_frames = app.selection.clone();
        if let Some(h) = app.hover {
            if !dev_frames.contains(&h) {
                dev_frames.push(h);
            }
        }
        let pad_tint = Color32::from_rgba_unmultiplied(0x14, 0xAE, 0x5C, 38);
        for fid in dev_frames {
            let Some(n) = app.doc.get(fid) else { continue };
            let Some(al) = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()) else { continue };
            let [pt, pr, pb, pl] = al.padding;
            let (w, h) = (n.size.x, n.size.y);
            let w_xform = super::fast_world_transform(&app.doc, fid);
            let quad = |x0: f32, y0: f32, x1: f32, y1: f32| {
                vec![to_s(w_xform.apply(v2(x0, y0))), to_s(w_xform.apply(v2(x1, y0))), to_s(w_xform.apply(v2(x1, y1))), to_s(w_xform.apply(v2(x0, y1)))]
            };
            if pt > 0.0 {
                p.add(egui::Shape::convex_polygon(quad(0.0, 0.0, w, pt.min(h)), pad_tint, Stroke::NONE));
            }
            if pb > 0.0 {
                p.add(egui::Shape::convex_polygon(quad(0.0, (h - pb).max(0.0), w, h), pad_tint, Stroke::NONE));
            }
            if pl > 0.0 {
                p.add(egui::Shape::convex_polygon(quad(0.0, pt.min(h), pl.min(w), (h - pb).max(pt)), pad_tint, Stroke::NONE));
            }
            if pr > 0.0 {
                p.add(egui::Shape::convex_polygon(quad((w - pr).max(0.0), pt.min(h), w, (h - pb).max(pt)), pad_tint, Stroke::NONE));
            }
        }
    }

    // Hover outline.
    for h in [app.hover, app.hover_layer].into_iter().flatten() {
        if !is_sel(h) && app.doc.get(h).is_some() {
            outline(app, &p, rect, h, sel_color(app, h), theme::CANVAS_HOVER_WIDTH);
        }
    }

    // Selection (the shape builder paints its own regions instead).
    if !app.selection.is_empty() && app.canvas.builder.is_none() {
        let multi = app.selection.len() > 1;
        for id in app.selection.iter().take(256) {
            if app.doc.get(*id).is_some() {
                outline(app, &p, rect, *id, sel_color(app, *id), theme::CANVAS_OUTLINE_WIDTH);
            }
        }
        if let Some((corners, is_line)) = selection_box(app) {
            let col = sel_color(app, app.selection[0]);
            let s = corners.map(&to_s);
            if multi {
                p.add(egui::Shape::closed_line(s.to_vec(), Stroke::new(theme::CANVAS_OUTLINE_WIDTH, col)));
            }
            let w = (s[1] - s[0]).length();
            let h = (s[3] - s[0]).length();
            let small = w.min(h) <= theme::CANVAS_HANDLE_SIZE * 3.0;
            let only_corners = small && !is_line;
            let dvs = corners.map(|c| dv(to_s(c)));
            let only_connectors = app.selection.iter().take(64).all(|id| matches!(app.doc.get(*id).map(|n| &n.kind), Some(NodeKind::Connector(_))));
            if app.canvas.text.is_none() && app.canvas.vedit.is_none() && matches!(app.tool, tools::Tool::Move | tools::Tool::Scale) && !only_connectors {
                for hnd in Handle::ALL {
                    if is_line && !matches!(hnd, Handle::L | Handle::R) {
                        continue;
                    }
                    if only_corners && !hnd.is_corner() {
                        continue;
                    }
                    let c = handles::handle_pos(&dvs, hnd);
                    let r = ERect::from_center_size(Pos2::new(c.x, c.y), EV::splat(theme::CANVAS_HANDLE_SIZE));
                    p.rect_filled(r, 0.0, theme::CANVAS_HANDLE_FILL);
                    p.rect_stroke(r, 0.0, Stroke::new(theme::CANVAS_HANDLE_BORDER, col), StrokeKind::Inside);
                }
                if let [one] = app.selection.as_slice() {
                    draw_single_node_handles(app, &p, rect, *one, &dvs);
                } else if multi && app.view.zoom >= 0.4 {
                    if let Some((_, items)) = super::interact::smart_selection_items(app) {
                        for (kind, page_center, size) in handles::smart_selection_handles(&items) {
                            let sc = to_s(page_center);
                            match kind {
                                handles::SmartSelectionHandleKind::Swap(_) => {
                                    p.circle_filled(sc, 4.5, Color32::WHITE);
                                    p.circle_stroke(sc, 4.5, Stroke::new(1.5, FIGMA_PINK));
                                }
                                handles::SmartSelectionHandleKind::Gap { .. } => {
                                    let pill = ERect::from_center_size(sc, EV::new(size.x, size.y));
                                    p.rect_filled(pill.expand(0.75), theme::cr(2), Color32::WHITE);
                                    p.rect_filled(pill, theme::cr(2), FIGMA_PINK);
                                }
                            }
                        }
                    }
                }
            }
            // Aspect ratio magnetic snap diagonal dashed line (`#94`).
            if let Interaction::Resize(res) = &app.canvas.inter {
                let ratio_locked = match &res.kind {
                    super::ResizeKind::Single { id, size0, .. } => app.doc.get(*id).is_some_and(|n| {
                        size0.x > 1.0 && size0.y > 1.0 && n.size.x > 1.0 && n.size.y > 1.0 && ((n.size.x / n.size.y) - (size0.x / size0.y)).abs() <= 0.015
                    }),
                    super::ResizeKind::Multi { bbox0, .. } => {
                        let bw = (corners[1].x - corners[0].x).abs();
                        let bh = (corners[3].y - corners[0].y).abs();
                        bbox0.w > 1.0 && bbox0.h > 1.0 && bw > 1.0 && bh > 1.0 && ((bw / bh) - (bbox0.w / bbox0.h)).abs() <= 0.015
                    }
                };
                if ratio_locked {
                    let diag = [s[0], s[2], s[0], s[2]];
                    dashed_quad(&p, &diag, 4.0, 4.0, Stroke::new(1.0, FIGMA_BLUE));
                }
            }
            // Size / Rotation / Scale / Min-Max Clamp badge under the lowest corner.
            let label = match &app.canvas.inter {
                Interaction::Rotate(rot) => {
                    let deg = app.doc.get(rot.id).map(|n| n.transform.rotation_rad().to_degrees()).unwrap_or(0.0);
                    Some(format!("∠ {:.1}°", deg).replace(".0°", "°"))
                }
                Interaction::Resize(res) if app.tool == tools::Tool::Scale => {
                    let (factor, w_px, h_px) = match &res.kind {
                        super::ResizeKind::Single { id, size0, .. } => {
                            let sz = app.doc.get(*id).map(|n| n.size).unwrap_or(*size0);
                            (sz.x / size0.x.max(1.0), sz.x.round() as i64, sz.y.round() as i64)
                        }
                        super::ResizeKind::Multi { bbox0, .. } => {
                            let bw = (corners[1].x - corners[0].x).abs();
                            let bh = (corners[3].y - corners[0].y).abs();
                            (bw / bbox0.w.max(1.0), bw.round() as i64, bh.round() as i64)
                        }
                    };
                    Some(format!("{factor:.2}× · {w_px} × {h_px}"))
                }
                _ => {
                    if let [one] = app.selection.as_slice() {
                        app.doc.get(*one).map(|n| {
                            let mut base = format!("{} × {}", n.size.x.round() as i64, n.size.y.round() as i64);
                            if matches!(app.canvas.inter, Interaction::Resize(_)) {
                                if n.layout_child.min_w.is_some_and(|m| (n.size.x - m).abs() < 0.5) {
                                    base.push_str(" · Min W");
                                } else if n.layout_child.max_w.is_some_and(|m| (n.size.x - m).abs() < 0.5) {
                                    base.push_str(" · Max W");
                                }
                                if n.layout_child.min_h.is_some_and(|m| (n.size.y - m).abs() < 0.5) {
                                    base.push_str(" · Min H");
                                } else if n.layout_child.max_h.is_some_and(|m| (n.size.y - m).abs() < 0.5) {
                                    base.push_str(" · Max H");
                                }
                            }
                            base
                        })
                    } else {
                        let bw = (corners[1].x - corners[0].x).abs().round() as i64;
                        let bh = (corners[3].y - corners[0].y).abs().round() as i64;
                        Some(format!("{bw} × {bh}"))
                    }
                }
            };
            if let Some(label) = label.filter(|_| !only_connectors) {
                let bottom = s.iter().map(|c| c.y).fold(f32::MIN, f32::max);
                let cx = s.iter().map(|c| c.x).sum::<f32>() / 4.0;
                badge(&p, Pos2::new(cx, bottom + theme::CANVAS_BADGE_OFFSET + 3.0), &label, col);
            }
            if let Some(tidy_r) = tidy_up_pill_rect(app, rect) {
                p.rect_filled(tidy_r, theme::cr(3), FIGMA_PINK);
                p.text(tidy_r.center(), Align2::CENTER_CENTER, "Tidy up", theme::font_canvas_strong(), Color32::WHITE);
            }
        }
    }

    vector::overlay(app, &p, rect);
    super::builder::overlay(app, &p, rect);

    // Snap guides & measurements.
    let gcol = theme::GUIDE_COLOR;
    for g in &app.canvas.guides {
        let (a, b) = match g.axis {
            GuideAxis::X => (to_s(v2(g.pos, g.from)), to_s(v2(g.pos, g.to))),
            GuideAxis::Y => (to_s(v2(g.from, g.pos)), to_s(v2(g.to, g.pos))),
        };
        p.line_segment([a, b], Stroke::new(theme::GUIDE_WIDTH, gcol));
        for e in [a, b] {
            let k = theme::GUIDE_X_SIZE;
            p.line_segment([e + EV::new(-k, -k), e + EV::new(k, k)], Stroke::new(theme::GUIDE_WIDTH, gcol));
            p.line_segment([e + EV::new(-k, k), e + EV::new(k, -k)], Stroke::new(theme::GUIDE_WIDTH, gcol));
        }
    }
    for m in &app.canvas.measures {
        let a = to_s(v2(m.a.0, m.a.1));
        let b = to_s(v2(m.b.0, m.b.1));
        p.line_segment([a, b], Stroke::new(theme::GUIDE_WIDTH, gcol));
        let horiz = (a.y - b.y).abs() < 0.5;
        let tick = if horiz { EV::new(0.0, 4.0) } else { EV::new(4.0, 0.0) };
        for e in [a, b] {
            p.line_segment([e - tick, e + tick], Stroke::new(theme::GUIDE_WIDTH, gcol));
        }
        let mid = Pos2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let text = format!("{}", m.value.round() as i64);
        let font = theme::font_canvas_strong();
        let w = crate::widgets::text_width(&p, &text, &font) + theme::GUIDE_TEXT_PAD * 2.0;
        let r = ERect::from_center_size(mid + EV::new(0.0, if horiz { -10.0 } else { 0.0 }), EV::new(w, theme::GUIDE_TEXT_HEIGHT));
        p.rect_filled(r, theme::cr(theme::CANVAS_BADGE_RADIUS), gcol);
        p.text(r.center(), Align2::CENTER_CENTER, text, font, theme::GUIDE_TEXT);
    }

    // Auto-layout insertion indicator.
    if let Interaction::Move(m) = &app.canvas.inter {
        if let Some(r) = &m.reorder {
            p.line_segment([to_s(r.line[0]), to_s(r.line[1])], Stroke::new(theme::AL_INDICATOR_WIDTH, theme::DROP_INDICATOR));
        }
    }

    // Marquee.
    if let Interaction::Marquee { start, cur, .. } = &app.canvas.inter {
        let (a, b) = (to_s(*start), to_s(*cur));
        let r = ERect::from_two_pos(a, b);
        p.rect_filled(r, 0.0, theme::CANVAS_MARQUEE_FILL);
        p.rect_stroke(r, 0.0, Stroke::new(1.0, theme::CANVAS_SELECTION), StrokeKind::Inside);
    }
    if let Interaction::ZoomMarquee { start, cur, alt } = &app.canvas.inter {
        let (a, b) = (to_s(*start), to_s(*cur));
        let r = ERect::from_two_pos(a, b);
        p.rect_filled(r, 0.0, theme::CANVAS_MARQUEE_FILL);
        let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
        dashed_quad(&p, &corners, 5.0, 3.0, Stroke::new(1.25, FIGMA_BLUE));
        let label = if *alt { "🔍 −" } else { "🔍 +" };
        badge(&p, Pos2::new(r.center().x, r.bottom() + 6.0), label, FIGMA_BLUE);
    }

    // Pen / pencil previews.
    if let Some(pen) = &app.canvas.pen {
        draw_pen(app, &p, rect, &pen.pts, ui.input(|i| i.pointer.latest_pos()));
    }
    if let Interaction::Pencil(pts) = &app.canvas.inter {
        let line: Vec<Pos2> = pts.iter().map(|q| to_s(*q)).collect();
        if app.tool.is_brush() {
            // a marker stroke is previewed in its own ink and thickness
            let (w, a) = if app.tool == tools::Tool::Highlighter {
                (theme::BOARD_HIGHLIGHTER_WIDTH, theme::BOARD_HIGHLIGHTER_ALPHA)
            } else {
                (theme::BOARD_MARKER_WIDTH, 1.0)
            };
            let ink = crate::board::color32(app.board.ink).gamma_multiply(a);
            p.add(egui::Shape::line(line, Stroke::new(w * app.view.zoom, ink)));
        } else {
            p.add(egui::Shape::line(line, Stroke::new(theme::PEN_LINE_WIDTH, theme::CANVAS_SELECTION)));
        }
    }
    crate::board::canvas::overlay(app, &p, rect);

    draw_find_matches(app, &p, rect);

    // Text caret.
    if let Some(sess) = &app.canvas.text {
        if let Some(n) = app.doc.get(sess.id) {
            // a text layer, or the text area of a sticky, shape or connector label
            if let Some(f) = crate::board::create::text_frame(&app.doc, n) {
                let (t, o) = (&f.props, f.origin);
                let w = super::fast_world_transform(&app.doc, sess.id);
                for (x, y, rw, rh) in crate::textmeasure::selection_rects(t, f.size, sess.range()) {
                    let (x, y) = (x + o.x, y + o.y);
                    let quad = [v2(x, y), v2(x + rw, y), v2(x + rw, y + rh), v2(x, y + rh)].map(|c| to_s(w.apply(c)));
                    p.add(egui::Shape::convex_polygon(quad.to_vec(), theme::TEXT_SELECTION, Stroke::NONE));
                }
                let (x, y, lh) = crate::textmeasure::caret_rect_hint(t, f.size, sess.caret, sess.hint_x);
                let (x, y) = (x + o.x, y + o.y);
                let a = to_s(w.apply(v2(x, y)));
                let b = to_s(w.apply(v2(x, y + lh)));
                // Tell the platform where the caret is so IME candidate windows follow it.
                if let Some(cr) = super::textsession::caret_screen_rect(app, rect.min) {
                    ui.ctx().output_mut(|o| {
                        o.ime = Some(egui::output::IMEOutput {
                            rect: ERect::from_min_max(to_s(w.apply(v2(0.0, 0.0))), to_s(w.apply(v2(n.size.x, n.size.y)))),
                            cursor_rect: cr,
                            should_interrupt_composition: false,
                        })
                    });
                }
                let blink = sess.range().is_empty() && (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0;
                if blink {
                    p.line_segment([a, b], Stroke::new(theme::TEXT_CARET_WIDTH, if light { Color32::BLACK } else { Color32::WHITE }));
                }
                ui.ctx().request_repaint_after(Duration::from_millis(250));
            }
        }
    }

    // On-canvas Eyedropper (`I`) floating magnifier loupe & #RRGGBB badge (MI-19).
    if app.props.eyedropper.is_some() || app.props.picker.as_ref().is_some_and(|pk| pk.eyedrop) {
        let fallback_pos = app
            .hover
            .or_else(|| app.selection.first().copied())
            .map(|id| {
                let bb = super::fast_world_aabb(&app.doc, super::interact::active_index(app), id);
                app.view.to_screen(rect.min, v2(bb.x + bb.w * 0.5, bb.y + bb.h * 0.5))
            })
            .filter(|p| rect.contains(*p))
            .unwrap_or_else(|| rect.center());
        if let Some(pos) = ui.ctx().input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p)).or(Some(fallback_pos)) {
            if let Some([r, g, b]) = crate::panels::properties::sample_canvas(app, pos) {
                let sampled = Color32::from_rgb(r, g, b);
                let loupe_c = pos + EV::new(28.0, -28.0);
                let radius = 22.0;
                p.circle_filled(loupe_c, radius + 2.5, Color32::from_black_alpha(0x66));
                p.circle_filled(loupe_c, radius, sampled);
                p.circle_stroke(loupe_c, radius, Stroke::new(2.0, Color32::WHITE));
                let px_box = ERect::from_center_size(loupe_c, EV::splat(6.0));
                p.rect_stroke(px_box, 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Outside);
                p.rect_stroke(px_box, 0.0, Stroke::new(1.0, Color32::from_black_alpha(0x88)), StrokeKind::Inside);
                let color_label = app.props.eyedropper_format.format_rgb(r, g, b);
                badge(&p, Pos2::new(loupe_c.x, loupe_c.y + radius + 12.0), &color_label, theme::BG_SECONDARY);
            }
        }
    }
}

/// Highlights the Find results inside text layers (the current one outlined), using the same
/// glyph geometry as the editor so the boxes sit exactly on the drawn characters.
fn draw_find_matches(app: &App, p: &Painter, rect: ERect) {
    if !app.find.open || app.find.results.is_empty() {
        return;
    }
    let to_s = |v: DV| app.view.to_screen(rect.min, v);
    let current = app.find.current;
    let idx = super::interact::active_index(app);
    let mut drawn = 0;
    for (i, m) in app.find.results.iter().enumerate() {
        if m.field != designos_doc::find::MatchField::Text || drawn >= theme::FIND_CANVAS_MATCH_MAX {
            continue;
        }
        let Some(n) = app.doc.get(m.node) else { continue };
        let Some(f) = crate::board::create::text_frame(&app.doc, n) else { continue };
        let (t, o) = (&f.props, f.origin);
        if !n.visible || app.doc.page_of(m.node) != Some(app.page) {
            continue;
        }
        // skip layers outside the view
        let bb = super::fast_world_aabb(&app.doc, idx, m.node);
        let (a, b) = (to_s(v2(bb.x, bb.y)), to_s(v2(bb.x + bb.w, bb.y + bb.h)));
        if !ERect::from_two_pos(a, b).expand(4.0).intersects(rect) {
            continue;
        }
        let w = super::fast_world_transform(&app.doc, m.node);
        drawn += 1;
        for (x, y, rw, rh) in crate::textmeasure::selection_rects(t, f.size, m.range.clone()) {
            let (x, y) = (x + o.x, y + o.y);
            let quad = [v2(x, y), v2(x + rw, y), v2(x + rw, y + rh), v2(x, y + rh)].map(|c| to_s(w.apply(c)));
            let is_current = current == Some(i);
            p.add(egui::Shape::convex_polygon(
                quad.to_vec(),
                theme::FIND_CANVAS_MATCH,
                if is_current { Stroke::new(1.0, theme::FIND_CANVAS_MATCH_CURRENT) } else { Stroke::NONE },
            ));
        }
    }
}

fn dv(p: Pos2) -> DV {
    DV::new(p.x, p.y)
}

fn draw_pen(app: &App, p: &Painter, rect: ERect, pts: &[create::PenPoint], pointer: Option<Pos2>) {
    let to_s = |v: DV| app.view.to_screen(rect.min, v);
    let col = theme::CANVAS_SELECTION;
    let stroke = Stroke::new(theme::PEN_LINE_WIDTH, col);
    for w in pts.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if a.hout.is_some() || b.hin.is_some() {
            let shape = egui::epaint::CubicBezierShape::from_points_stroke(
                [to_s(a.pos), to_s(a.hout.unwrap_or(a.pos)), to_s(b.hin.unwrap_or(b.pos)), to_s(b.pos)],
                false,
                Color32::TRANSPARENT,
                stroke,
            );
            p.add(shape);
        } else {
            p.line_segment([to_s(a.pos), to_s(b.pos)], stroke);
        }
    }
    if let (Some(last), Some(ptr)) = (pts.last(), pointer) {
        if rect.contains(ptr) {
            p.line_segment([to_s(last.pos), ptr], Stroke::new(1.0, col.gamma_multiply(0.6)));
        }
    }
    for (i, pt) in pts.iter().enumerate() {
        let c = to_s(pt.pos);
        for h in [pt.hin, pt.hout].into_iter().flatten() {
            p.line_segment([c, to_s(h)], Stroke::new(1.0, col));
            p.circle_filled(to_s(h), 2.5, col);
        }
        let near_first = i == 0 && pts.len() >= 3 && pointer.map(|q| (q - c).length() < 8.0).unwrap_or(false);
        let r = ERect::from_center_size(c, EV::splat(theme::PEN_POINT_SIZE * 2.0 * if near_first { 1.5 } else { 1.0 }));
        p.rect_filled(r, 0.0, theme::PEN_POINT);
        p.rect_stroke(r, 0.0, Stroke::new(1.0, col), StrokeKind::Inside);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::Document;

    #[test]
    fn large_page_label_rects_hit_marquee_and_snap_under_5ms() {
        let mut doc = Document::new("large-bench");
        let page = doc.add_page("Page 1");
        let mut first_leaf = None;
        for f_idx in 0..50 {
            let fx = (f_idx % 10) as f32 * 600.0;
            let fy = (f_idx / 10) as f32 * 500.0;
            let fid = doc.alloc_id();
            let frame = doc.insert(page, Node::frame(fid, Rect::new(fx, fy, 480.0, 400.0))).unwrap();
            for c_idx in 0..49 {
                let cx = (c_idx % 7) as f32 * 60.0 + 10.0;
                let cy = (c_idx / 7) as f32 * 50.0 + 10.0;
                let cid = doc.alloc_id();
                let child = doc.insert(frame, Node::rectangle(cid, Rect::new(cx, cy, 48.0, 36.0))).unwrap();
                if first_leaf.is_none() {
                    first_leaf = Some(child);
                }
            }
        }
        assert!(doc.nodes.len() >= 2500);

        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&doc, page, 1);

        let mut app = App::new(doc);
        app.canvas_rect = ERect::from_min_size(Pos2::ZERO, EV::new(1440.0, 900.0));
        app.view.offset = DV::ZERO;
        app.view.zoom = 1.0;
        app.canvas.dirty = false;

        let screen_rect = app.canvas_rect;
        let leaf = first_leaf.unwrap();
        let targets = super::super::interact::snap_targets(&app, &[leaf]);
        assert!(targets.len() <= 128);

        let budget = std::time::Duration::from_millis(5);
        let (elapsed, (labels, hit, marquee, snapped)) = crate::test_timing::fastest_within(budget, std::time::Duration::from_secs(2), || {
            // Every sample times the cache-miss path, as the single sample did before.
            LABEL_RECTS_CACHE.with(|c| *c.borrow_mut() = None);
            let labels = label_rects(&app, screen_rect);
            let hit = super::super::hit::hit_test_with_index(&app.doc, app.page, v2(25.0, 25.0), 2.0, false, &[], Some(&idx));
            let marquee = super::super::hit::marquee_select_with_index(&app.doc, app.page, &Rect::new(0.0, 0.0, 120.0, 100.0), Some(&idx));
            let snapped = super::super::snap::snap_rect(&Rect::new(71.0, 10.0, 48.0, 36.0), &targets, 4.0);
            (labels, hit, marquee, snapped)
        });

        assert!(!labels.is_empty());
        assert_eq!(hit, Some(leaf));
        assert!(marquee.contains(&leaf));
        assert_eq!(snapped.dx, -1.0);
        assert!(
            elapsed < budget,
            "expected < 5ms for label_rects + hit_test_with_index + marquee_select_with_index + snap_rect on 2,500 nodes, fastest run took {:.3}ms",
            elapsed.as_secs_f64() * 1000.0
        );
    }

    /// Steady-state `sync_live_file_context` on the synthetic doc may take at most this multiple of the
    /// reference workload (serialize the doc once, write and rename 7 files), timed on the same runner.
    /// Fastest of 40 each, measured 2026-10-17: sync/reference was 1.06 on macos-14 CI, 1.33 on Linux,
    /// 1.15 on Windows and 1.05-1.10 on an Apple-silicon laptop; three syncs measured 3.17-3.87. So a
    /// 3x slowdown fails on every OS while a healthy sync keeps at least 1.6x headroom.
    const TERM_SYNC_MAX_REFERENCE_RATIO: f64 = 2.25;

    #[test]
    fn vsf_pcp_single_container_instance_and_multi_selection_under_2ms() {
        // The real "VSF - PCP.fig" bench is opt-in through DESIGNOS_BENCH_FIG. A hard-coded home path
        // ties the test to one machine, and probing ~/Downloads blocks on the macOS privacy prompt.
        let fig_path = std::env::var_os("DESIGNOS_BENCH_FIG").map(std::path::PathBuf::from);
        let real_fixture = fig_path.is_some();
        let doc = if let Some(fig_path) = fig_path {
            let bytes = std::fs::read(&fig_path).unwrap_or_else(|e| panic!("read DESIGNOS_BENCH_FIG {}: {e}", fig_path.display()));
            designos_fig::import_fig(&bytes).expect("import DESIGNOS_BENCH_FIG")
        } else {
            let mut d = Document::new("synth-multi");
            let p = d.add_page("V-Builder");
            let sec_id = d.alloc_id();
            let mut sec = Node::new(sec_id, NodeKind::Section);
            sec.name = "Section".into();
            sec.size = DV::new(4000.0, 3000.0);
            let sec = d.insert(p, sec).unwrap();
            let comp_id = d.alloc_id();
            let comp = d.insert(p, Node::frame(comp_id, Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
            for i in 0..300 {
                let id = d.alloc_id();
                let x = (i % 20) as f32 * 120.0;
                let y = (i / 20) as f32 * 90.0;
                let mut n = Node::rectangle(id, Rect::new(x, y, 100.0, 60.0));
                if i % 3 == 0 {
                    n.kind = NodeKind::Instance { frame: Default::default(), component: Some(comp), component_key: None };
                }
                d.insert(sec, n).unwrap();
            }
            d
        };

        // Pick the largest page (e.g., "V-Builder" with 159,644 nodes in VSF - PCP.fig).
        let largest_page = doc.pages.iter().copied().max_by_key(|&pid| doc.get(pid).map_or(0, |p| p.children.len())).unwrap_or(doc.pages[0]);

        let mut app = App::new(doc);
        app.page = largest_page;
        app.canvas_rect = ERect::from_min_size(Pos2::ZERO, EV::new(1440.0, 900.0));
        app.view.offset = DV::ZERO;
        app.view.zoom = 0.5;
        app.canvas.dirty = false;

        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&app.doc, app.page, app.canvas.tiles.rev());

        let page_children = app.doc.get(app.page).unwrap().children.clone();
        let big_container = page_children[0];
        let mut sample_instance = big_container;
        let mut sample_leaf = big_container;
        let mut multi_nodes = Vec::new();

        // Collect a sample Instance, a leaf, and 200 nodes on this page for multi-selection testing.
        let mut stack = page_children.clone();
        while let Some(nid) = stack.pop() {
            if let Some(n) = app.doc.get(nid) {
                if matches!(n.kind, NodeKind::Instance { .. }) {
                    sample_instance = nid;
                }
                if n.children.is_empty() {
                    sample_leaf = nid;
                }
                if multi_nodes.len() < 200 && nid != big_container {
                    multi_nodes.push(nid);
                }
                if multi_nodes.len() < 200 {
                    for &c in n.children.iter().take(16) {
                        stack.push(c);
                    }
                }
            }
        }

        // 1. Single Leaf Selection
        let t0 = std::time::Instant::now();
        app.select_only(sample_leaf);
        let _top1 = app.top_selection();
        let _box1 = super::super::interact::selection_box(&app);
        let _cols1 = crate::styles::selection_colors_with_counts(&app.doc, &app.selection, 512);
        let _rows1 = crate::panels::layers::visible_rows(&app);
        let single_leaf_ms = t0.elapsed().as_secs_f64() * 1000.0;

        // 2. Massive Container / Section Selection (30,000+ descendants in VSF - PCP.fig)
        let t1 = std::time::Instant::now();
        app.select_only(big_container);
        let _top2 = app.top_selection();
        let _box2 = super::super::interact::selection_box(&app);
        let _aabb2 = super::super::fast_world_aabb(&app.doc, Some(&idx), big_container);
        let _cols2 = crate::styles::selection_colors_with_counts(&app.doc, &app.selection, 512);
        let _rows2 = crate::panels::layers::visible_rows(&app);
        let container_sel_ms = t1.elapsed().as_secs_f64() * 1000.0;

        // 3. Component Instance Selection
        let t2 = std::time::Instant::now();
        app.select_only(sample_instance);
        let _top3 = app.top_selection();
        let _box3 = super::super::interact::selection_box(&app);
        let _cols3 = crate::styles::selection_colors_with_counts(&app.doc, &app.selection, 512);
        let _rows3 = crate::panels::layers::visible_rows(&app);
        let instance_sel_ms = t2.elapsed().as_secs_f64() * 1000.0;

        // 4. Multi-Selection (200 layers + Marquee Select across 4000x3000 world rect)
        let t3 = std::time::Instant::now();
        let marquee_hits = super::super::hit::marquee_select_with_index(&app.doc, app.page, &Rect::new(-2000.0, -2000.0, 4000.0, 3000.0), Some(&idx));
        app.selection = multi_nodes;
        let _top4 = app.top_selection();
        let _box4 = super::super::interact::selection_box(&app);
        let _cols4 = crate::styles::selection_colors_with_counts(&app.doc, &app.selection, 512);
        let _rows4 = crate::panels::layers::visible_rows(&app);
        let multi_sel_ms = t3.elapsed().as_secs_f64() * 1000.0;

        // 5. Post-Load UI-Thread Passes (fonts.load_for + sync_live_file_context)
        let t_fonts = std::time::Instant::now();
        app.fonts.load_for(&app.doc);
        let fonts_ms = t_fonts.elapsed().as_secs_f64() * 1000.0;

        // Every App::new binds the shared `tab-1` session dir, so tests running in parallel write and
        // rename the same files. Give this one a private dir so it only times its own I/O.
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        app.terminal.bind_session(&format!("draw-bench-{}-{nonce}", std::process::id()));
        let session_dir = app.terminal.session_dir.clone();

        // The first sync of a tab creates the session directories and files.
        let t_sync_cold = std::time::Instant::now();
        app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);
        let term_sync_cold_ms = t_sync_cold.elapsed().as_secs_f64() * 1000.0;

        // Steady state: the sync that runs on every page or selection change. Hosted runners differ in
        // disk and CPU speed from run to run, so a fixed millisecond budget either flakes or misses a 3x
        // slowdown. The synthetic doc is instead compared with a reference workload timed on the same
        // runner: the work a sync cannot avoid (serialize the doc, write and rename 7 files). Both run
        // interleaved and are compared by their fastest call, which file-I/O stalls can only raise:
        // 10 pairs at least, then more until the sync is within budget or 5 s are up. A healthy sync
        // passes after 10 pairs; a regressed one never gets under, however long it is sampled.
        // DESIGNOS_BENCH_FIG docs keep a fixed 60 ms budget: they take the scoped >25k-node branch,
        // which serializes far less than a full to_json.
        let ref_dir = std::env::temp_dir().join(format!("draw-bench-ref-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&ref_dir).expect("create reference dir");
        let filler = vec![b'x'; 1024];
        let sample_until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut term_sync_ms = f64::INFINITY;
        let mut reference_ms = f64::INFINITY;
        let mut term_sync_calls = 0;
        let term_sync_budget_ms = loop {
            if !real_fixture {
                let t = std::time::Instant::now();
                let json = app.doc.to_json().expect("serialize reference doc");
                for i in 0..7 {
                    let tmp = ref_dir.join(format!("out-{i}.tmp"));
                    std::fs::write(&tmp, if i == 0 { json.as_bytes() } else { &filler[..] }).expect("write reference file");
                    std::fs::rename(&tmp, ref_dir.join(format!("out-{i}"))).expect("rename reference file");
                }
                reference_ms = reference_ms.min(t.elapsed().as_secs_f64() * 1000.0);
            }

            app.terminal.last_synced_key = None;
            let t = std::time::Instant::now();
            app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);
            term_sync_ms = term_sync_ms.min(t.elapsed().as_secs_f64() * 1000.0);
            term_sync_calls += 1;

            let budget_ms = if real_fixture { 60.0 } else { TERM_SYNC_MAX_REFERENCE_RATIO * reference_ms };
            if (term_sync_calls >= 10 && term_sync_ms < budget_ms) || std::time::Instant::now() >= sample_until {
                break budget_ms;
            }
        };
        let _ = std::fs::remove_dir_all(&ref_dir);
        let _ = std::fs::remove_dir_all(&session_dir);

        // 6. Canvas Overlay Chrome, Label Rects & Hover Hit-Test at zoom=1.0
        app.view.zoom = 1.0;
        let t_labels = std::time::Instant::now();
        let _labels = super::label_rects(&app, app.canvas_rect);
        let labels_ms = t_labels.elapsed().as_secs_f64() * 1000.0;

        let t_chrome = std::time::Instant::now();
        let chrome_count = idx.chrome_nodes().len();
        let chrome_ms = t_chrome.elapsed().as_secs_f64() * 1000.0;

        let t_hit = std::time::Instant::now();
        let _hit = super::super::hit::hit_test_with_index(&app.doc, app.page, DV::new(400.0, 300.0), 8.0, false, &[], Some(&idx));
        let hit_ms = t_hit.elapsed().as_secs_f64() * 1000.0;

        // 7. Live Edit / Drag Commit (comp_ops::with_sync + layout_node)
        let t_ws = std::time::Instant::now();
        let mut moved = app.doc.get(sample_leaf).unwrap().clone();
        moved.transform.tx += 1.0;
        let _cmd = crate::comp_ops::with_sync(&mut app.doc, designos_doc::Command::Update { node: Box::new(moved) });
        let with_sync_ms = t_ws.elapsed().as_secs_f64() * 1000.0;

        let t_layout = std::time::Instant::now();
        let mut ed_layout = crate::edit::Edit::new();
        crate::layout::layout_node(&mut app.doc, &mut ed_layout, big_container);
        let layout_ms = t_layout.elapsed().as_secs_f64() * 1000.0;

        eprintln!(
            "[SELECTION BENCH] doc_nodes={} | single_leaf={:.3}ms | big_container={:.3}ms | instance={:.3}ms | multi_200_and_marquee(hits={})={:.3}ms",
            app.doc.nodes.len(),
            single_leaf_ms,
            container_sel_ms,
            instance_sel_ms,
            marquee_hits.len(),
            multi_sel_ms
        );
        eprintln!(
            "[INTERACTION & POST-LOAD BENCH] fonts_load={:.3}ms | term_sync={:.3}ms (fastest of {} calls, reference {:.3}ms, cold {:.3}ms) | label_rects={:.3}ms | chrome_nodes({})={:.4}ms | hit_test={:.4}ms | with_sync={:.4}ms | layout_node={:.3}ms",
            fonts_ms,
            term_sync_ms,
            term_sync_calls,
            reference_ms,
            term_sync_cold_ms,
            labels_ms,
            chrome_count,
            chrome_ms,
            hit_ms,
            with_sync_ms,
            layout_ms
        );

        assert!(single_leaf_ms < 25.0, "single leaf selection took {:.3}ms", single_leaf_ms);
        assert!(container_sel_ms < 25.0, "big container selection took {:.3}ms", container_sel_ms);
        assert!(instance_sel_ms < 25.0, "instance selection took {:.3}ms", instance_sel_ms);
        assert!(multi_sel_ms < 25.0, "200-node multi-selection + marquee took {:.3}ms", multi_sel_ms);
        assert!(fonts_ms < 60.0, "fonts.load_for took {:.3}ms", fonts_ms);
        assert!(
            term_sync_ms < term_sync_budget_ms,
            "steady-state sync_live_file_context took {term_sync_ms:.3}ms at best over {term_sync_calls} calls; budget {term_sync_budget_ms:.3}ms (reference workload {reference_ms:.3}ms)"
        );
        assert!(hit_ms < 10.0, "hit_test_with_index took {:.4}ms", hit_ms);
        assert!(with_sync_ms < 10.0, "comp_ops::with_sync took {:.4}ms", with_sync_ms);
        assert!(layout_ms < 30.0, "layout_node took {:.3}ms", layout_ms);
    }

    /// The overlay of an empty grid with the given tracks (columns/rows as set on `al`).
    fn bare_overlay(al: &designos_doc::AutoLayout) -> super::super::grids::CssGridOverlay {
        super::super::grids::compute_css_grid_overlay(al, DV::new(2000.0, 2000.0), &[])
    }

    fn disjoint_per_axis(pills: &[(bool, usize, ERect, ERect)]) -> bool {
        pills.iter().enumerate().all(|(i, (a_col, _, _, a))| {
            pills[i + 1..].iter().all(|(b_col, _, _, b)| a_col != b_col || !(a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y))
        })
    }

    #[test]
    fn header_pills_keep_a_24x24_hit_floor_while_painted_pills_shrink_to_the_track_pitch() {
        let mut al = designos_doc::AutoLayout { grid_columns: 8, grid_rows: 8, ..Default::default() };
        for i in 0..8 {
            al.grid_col_tracks[i] = designos_doc::GridTrack::Fixed(24.0);
            al.grid_row_tracks[i] = designos_doc::GridTrack::Fixed(10.0);
        }
        let ov = bare_overlay(&al);
        let zoomed_out = |x: f32, y: f32| Pos2::new(x * 0.4, y * 0.4);
        let pills = grid_pill_rects_of(&ov, zoomed_out);
        assert_eq!(pills.len(), 16);
        assert!(disjoint_per_axis(&pills), "painted pills overlap: {pills:?}");
        for (is_col, i, hit, paint) in &pills {
            assert!(
                hit.width() >= GRID_PILL_HIT_FLOOR && hit.height() >= GRID_PILL_HIT_FLOOR,
                "pill {i} (is_col={is_col}) hit rect is {}x{}, below 24x24 floor",
                hit.width(),
                hit.height()
            );
            assert!((hit.center() - paint.center()).length() < 1e-4, "hit and paint rects share the same centre");
            if *is_col {
                assert!(paint.width() <= 9.6 + 1e-3, "column pill {i} paint is {} px wide at a 9.6 px pitch", paint.width());
            } else {
                assert!(paint.height() <= 4.0 + 1e-3, "row pill {i} paint is {} px tall at a 4 px pitch", paint.height());
            }
        }
    }

    #[test]
    fn header_pills_keep_their_label_size_when_the_tracks_have_room() {
        let mut al = designos_doc::AutoLayout { grid_columns: 3, grid_rows: 2, ..Default::default() };
        al.grid_col_tracks[0] = designos_doc::GridTrack::Fixed(100.0);
        al.grid_col_tracks[1] = designos_doc::GridTrack::Hug;
        al.grid_row_tracks[0] = designos_doc::GridTrack::Fixed(60.0);
        let ov = bare_overlay(&al);
        let pills = grid_pill_rects_of(&ov, Pos2::new);
        let (_, _, col0_hit, col0_paint) = *pills.iter().find(|(c, i, _, _)| *c && *i == 0).unwrap();
        assert_eq!((col0_hit.width(), col0_hit.height()), (40.0, GRID_PILL_HIT_FLOOR));
        assert_eq!((col0_paint.width(), col0_paint.height()), (40.0, GRID_PILL_HEIGHT), "`100px` is 5 characters: 5 * 6 + 10");
        let (_, _, col1_hit, col1_paint) = *pills.iter().find(|(c, i, _, _)| *c && *i == 1).unwrap();
        assert_eq!((col1_hit.width(), col1_hit.height()), (28.0, GRID_PILL_HIT_FLOOR));
        assert_eq!(col1_paint.width(), 28.0, "`Hug` is 3 characters: 3 * 6 + 10");
        let (_, _, row0_hit, row0_paint) = *pills.iter().find(|(c, i, _, _)| !*c && *i == 0).unwrap();
        assert_eq!(row0_hit.height(), GRID_PILL_HIT_FLOOR);
        assert_eq!(row0_paint.height(), GRID_PILL_HEIGHT);
    }

    #[test]
    fn the_nearest_centre_wins_when_pills_overlap() {
        let near = (NodeId(1), true, 1, ERect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(30.0, 15.0)));
        let far = (NodeId(1), true, 0, ERect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(20.0, 15.0)));
        let at = Pos2::new(18.0, 7.0);
        assert_eq!(nearest_pill([far, near], at).map(|p| p.2), Some(1));
        assert_eq!(nearest_pill([near, far], at).map(|p| p.2), Some(1), "list order does not matter");
        assert!(nearest_pill([far, near], Pos2::new(50.0, 7.0)).is_none(), "outside every pill is nothing");
    }
}
