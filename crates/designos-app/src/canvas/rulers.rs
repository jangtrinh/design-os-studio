//! Rulers (Shift+R) and user guides.
//!
//! Two 20px strips on the top and left of the canvas show tick labels in page
//! units, relative to the origin of the selected frame (or 0). The extent of the
//! selection is highlighted. Dragging from a ruler creates a guide, a red line
//! that other layers snap to; dragging it back onto its ruler deletes it.
//!
//! Guides live in [`designos_doc::Document::guides`] (saved with the file); every
//! create / move / delete is one `Command::SetGuides` undo step. A guide dropped
//! over a selected frame becomes a frame guide: its offset is frame-local, it is
//! drawn within the frame and moves with it.

use super::snap::{self, GuideAxis};
use super::*;
use crate::theme;
use crate::util::*;
use egui::{Align2, Color32, Painter, Pos2, Stroke, Vec2 as EV};

/// A stored guide resolved to page coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RulerGuide {
    pub axis: GuideAxis,
    /// Page-space position (frame origin + offset for frame guides).
    pub pos: f32,
    pub frame: Option<NodeId>,
}

fn to_doc_axis(a: GuideAxis) -> designos_doc::GuideAxis {
    match a {
        GuideAxis::X => designos_doc::GuideAxis::X,
        GuideAxis::Y => designos_doc::GuideAxis::Y,
    }
}

fn from_doc_axis(a: designos_doc::GuideAxis) -> GuideAxis {
    match a {
        designos_doc::GuideAxis::X => GuideAxis::X,
        designos_doc::GuideAxis::Y => GuideAxis::Y,
    }
}

/// World-space origin of a frame guide's frame.
fn frame_origin(doc: &designos_doc::Document, f: NodeId) -> Option<DV> {
    doc.get(f)?;
    Some(doc.world_transform(f).apply(DV::ZERO))
}

fn axis_of(o: DV, axis: GuideAxis) -> f32 {
    if axis == GuideAxis::X {
        o.x
    } else {
        o.y
    }
}

/// Resolves a stored guide to page space; `None` when its frame is gone.
pub fn resolve(doc: &designos_doc::Document, g: &designos_doc::Guide) -> Option<RulerGuide> {
    let axis = from_doc_axis(g.axis);
    let base = match g.frame {
        None => 0.0,
        Some(f) => axis_of(frame_origin(doc, f)?, axis),
    };
    Some(RulerGuide { axis, pos: base + g.offset, frame: g.frame })
}

/// Inverse of [`resolve`]: the stored guide for a page-space position.
pub fn store(doc: &designos_doc::Document, axis: GuideAxis, pos: f32, frame: Option<NodeId>) -> designos_doc::Guide {
    let base = frame.and_then(|f| frame_origin(doc, f)).map(|o| axis_of(o, axis)).unwrap_or(0.0);
    designos_doc::Guide { axis: to_doc_axis(axis), offset: pos - base, frame }
}

/// The selected frame under a page point (the smallest one), if any.
fn selected_frame_at(app: &App, p: DV) -> Option<NodeId> {
    let idx = super::interact::active_index(app);
    app.selection
        .iter()
        .copied()
        .filter(|id| app.doc.get(*id).is_some_and(|n| n.kind.frame_props().is_some()))
        .map(|id| (id, super::fast_world_aabb(&app.doc, idx, id)))
        .filter(|(_, bb)| p.x >= bb.x && p.x <= bb.x + bb.w && p.y >= bb.y && p.y <= bb.y + bb.h)
        .min_by(|a, b| (a.1.w * a.1.h).total_cmp(&(b.1.w * b.1.h)))
        .map(|(id, _)| id)
}

/// A guide being created or moved.
#[derive(Clone, Copy, Debug)]
pub struct GuideDrag {
    pub axis: GuideAxis,
    /// Index in the page's guides when an existing guide is dragged.
    pub index: Option<usize>,
    /// Page-space position.
    pub pos: f32,
    /// Frame the guide belongs to (new guides pick the selected frame under the pointer).
    pub frame: Option<NodeId>,
    /// Whether the pointer currently sits on the ruler it came from (releasing there deletes).
    pub on_ruler: bool,
}

/// The ruler strips: top (without the corner) and left.
pub fn strips(rect: ERect) -> (ERect, ERect) {
    let s = theme::RULER_SIZE;
    (
        ERect::from_min_max(Pos2::new(rect.left() + s, rect.top()), Pos2::new(rect.right(), rect.top() + s)),
        ERect::from_min_max(Pos2::new(rect.left(), rect.top() + s), Pos2::new(rect.left() + s, rect.bottom())),
    )
}

/// Page position that reads as zero on the rulers: the origin of the top-level
/// frame holding the selection (when it is a single frame), otherwise (0, 0).
pub fn origin(app: &App) -> DV {
    let mut frame = None;
    for id in app.selection.iter().take(64) {
        let mut cur = *id;
        let mut top = cur;
        while let Some(p) = app.doc.get(cur).and_then(|n| n.parent) {
            if p == app.page {
                break;
            }
            top = p;
            cur = p;
        }
        let top_level = app.doc.get(top).is_some_and(|n| n.parent == Some(app.page) && n.kind.frame_props().is_some());
        if !top_level {
            return DV::ZERO;
        }
        match frame {
            None => frame = Some(top),
            Some(f) if f == top => {}
            _ => return DV::ZERO,
        }
    }
    frame.map(|f| app.doc.world_transform(f).apply(DV::ZERO)).unwrap_or(DV::ZERO)
}

/// Major tick step in page units: 1, 2, 5, 10, 20, 50, ... such that ticks are
/// at least ~50 screen px apart.
pub fn tick_step(zoom: f32) -> f32 {
    let mut mag = 1.0f32;
    loop {
        for m in [1.0, 2.0, 5.0] {
            let s = mag * m;
            if s * zoom >= 50.0 {
                return s;
            }
        }
        mag *= 10.0;
        if mag > 1.0e9 {
            return mag;
        }
    }
}

/// Tick positions (value relative to the origin, level 0 = labelled major, 1 = half, 2 = minor)
/// covering `lo..=hi` (relative values).
pub fn ticks(lo: f32, hi: f32, zoom: f32) -> Vec<(f32, u8)> {
    let step = tick_step(zoom);
    let minor = if step * zoom / 10.0 >= 6.0 {
        step / 10.0
    } else if step * zoom / 5.0 >= 6.0 {
        step / 5.0
    } else {
        step
    };
    let n_lo = (lo / minor).floor() as i64 - 1;
    let n_hi = (hi / minor).ceil() as i64 + 1;
    let per = (step / minor).round() as i64;
    let mut out = Vec::new();
    for n in n_lo..=n_hi {
        let v = n as f32 * minor;
        let level = if n.rem_euclid(per.max(1)) == 0 {
            0
        } else if per % 2 == 0 && n.rem_euclid(per) == per / 2 {
            1
        } else {
            2
        };
        out.push((v, level));
    }
    out
}

fn fmt_value(v: f32) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

fn crisp(v: f32) -> f32 {
    v.round() + 0.5
}

fn rotated_text(p: &Painter, pos: Pos2, text: &str, color: Color32) {
    let galley = p.layout_no_wrap(text.to_string(), theme::font_ruler(), color);
    p.add(egui::epaint::TextShape::new(pos, galley, color).with_angle(-std::f32::consts::FRAC_PI_2));
}

/// The page's guides with their stored index (guides of missing frames are skipped).
fn guides_of(app: &App) -> Vec<(usize, RulerGuide)> {
    let Some(list) = app.doc.guides.get(&app.page) else { return vec![] };
    list.iter().enumerate().filter_map(|(i, g)| resolve(&app.doc, g).map(|r| (i, r))).collect()
}

/// Guides as snap targets (see [`snap::guide_rect`]).
pub fn guide_rects(app: &App) -> Vec<Rect> {
    if !app.ui.rulers {
        return vec![];
    }
    guides_of(app).iter().map(|(_, g)| snap::guide_rect(g.axis, g.pos)).collect()
}

fn page_pos(app: &App, rect: ERect, pos: Pos2, axis: GuideAxis) -> f32 {
    let p = app.view.to_page(rect.min, pos);
    let v = match axis {
        GuideAxis::X => p.x,
        GuideAxis::Y => p.y,
    };
    if app.ui.no_pixel_snap {
        v
    } else {
        v.round()
    }
}

/// Starts a guide drag when the press hits a ruler (new guide) or, with the move
/// tool, an existing guide, duplicating an existing guide when `alt` is held (`#71`).
pub fn press_with_alt(app: &mut App, rect: ERect, pos: Pos2, alt: bool) -> bool {
    if !app.ui.rulers || app.canvas.text.is_some() {
        return false;
    }
    let (top, left) = strips(rect);
    let new = if top.contains(pos) {
        Some(GuideAxis::Y)
    } else if left.contains(pos) {
        Some(GuideAxis::X)
    } else {
        None
    };
    if let Some(axis) = new {
        let v = page_pos(app, rect, pos, axis);
        app.canvas.inter = Interaction::Guide(GuideDrag { axis, index: None, pos: v, frame: None, on_ruler: true });
        return true;
    }
    if !matches!(app.tool, tools::Tool::Move | tools::Tool::Scale) {
        return false;
    }
    if let Some((i, g)) = hit_guide(app, rect, pos) {
        let index = if alt {
            let dup = store(&app.doc, g.axis, g.pos, g.frame);
            let list = app.doc.guides.entry(app.page).or_default();
            list.push(dup);
            Some(list.len() - 1)
        } else {
            Some(i)
        };
        app.canvas.inter = Interaction::Guide(GuideDrag { axis: g.axis, index, pos: g.pos, frame: g.frame, on_ruler: false });
        return true;
    }
    false
}

/// Clears all guides on the active page when right-clicking inside the top or left `20px` ruler bar (`#72`).
pub fn secondary_click(app: &mut App, rect: ERect, pos: Pos2) -> bool {
    if !app.ui.rulers || app.canvas.text.is_some() {
        return false;
    }
    let (top, left) = strips(rect);
    let corner = ERect::from_min_size(rect.min, EV::splat(theme::RULER_SIZE));
    if !top.contains(pos) && !left.contains(pos) && !corner.contains(pos) {
        return false;
    }
    let page = app.page;
    let has_guides = app.doc.guides.get(&page).is_some_and(|g| !g.is_empty());
    if has_guides {
        if let Err(e) = app.hist.exec(&mut app.doc, designos_doc::Command::SetGuides { page, guides: Vec::new() }) {
            app.show_toast(format!("Guide edit failed: {e}"), true);
            return true;
        }
        app.changed();
    }
    app.show_toast("Cleared all guides", false);
    true
}

/// The guide under a screen point (within 4px), if any.
pub fn hit_guide(app: &App, rect: ERect, pos: Pos2) -> Option<(usize, RulerGuide)> {
    if !app.ui.rulers {
        return None;
    }
    let (top, left) = strips(rect);
    if top.contains(pos) || left.contains(pos) {
        return None;
    }
    let pg = app.view.to_page(rect.min, pos);
    guides_of(app).into_iter().find_map(|(i, g)| {
        let s = match g.axis {
            GuideAxis::X => app.view.to_screen(rect.min, v2(g.pos, 0.0)).x - pos.x,
            GuideAxis::Y => app.view.to_screen(rect.min, v2(0.0, g.pos)).y - pos.y,
        };
        (s.abs() <= 4.0 && frame_covers(app, &g, pg)).then_some((i, g))
    })
}

/// Whether the page point lies along a frame guide's extent (always true for page guides).
fn frame_covers(app: &App, g: &RulerGuide, p: DV) -> bool {
    let Some(f) = g.frame else { return true };
    let bb = super::fast_world_aabb(&app.doc, super::interact::active_index(app), f);
    match g.axis {
        GuideAxis::X => p.y >= bb.y && p.y <= bb.y + bb.h,
        GuideAxis::Y => p.x >= bb.x && p.x <= bb.x + bb.w,
    }
}

pub fn drag(app: &App, g: &mut GuideDrag, rect: ERect, pos: Pos2) {
    g.pos = page_pos(app, rect, pos, g.axis);
    if g.index.is_none() {
        g.frame = selected_frame_at(app, app.view.to_page(rect.min, pos));
    }
    let (top, left) = strips(rect);
    // Back on a ruler (or off the canvas): releasing now removes / cancels the guide.
    let over = match g.axis {
        GuideAxis::Y => pos.y < top.bottom(),
        GuideAxis::X => pos.x < left.right(),
    };
    g.on_ruler = over || !rect.contains(pos);
}

pub fn release(app: &mut App, g: GuideDrag) {
    let page = app.page;
    let old = app.doc.guides.get(&page).cloned().unwrap_or_default();
    let mut list = old.clone();
    match (g.index, g.on_ruler) {
        (Some(i), true) => {
            if i < list.len() {
                list.remove(i);
            }
        }
        (Some(i), false) => {
            if i < list.len() {
                list[i] = store(&app.doc, g.axis, g.pos, list[i].frame);
            }
        }
        (None, false) => list.push(store(&app.doc, g.axis, g.pos, g.frame)),
        (None, true) => {}
    }
    if list == old {
        return;
    }
    if let Err(e) = app.hist.exec(&mut app.doc, designos_doc::Command::SetGuides { page, guides: list }) {
        app.show_toast(format!("Guide edit failed: {e}"), true);
    }
    app.changed();
}

/// Removes the guide at `index` on the active page (used on double-click `S3-08`).
pub fn remove_guide(app: &mut App, index: usize) -> bool {
    let page = app.page;
    let mut list = app.doc.guides.get(&page).cloned().unwrap_or_default();
    if index >= list.len() {
        return false;
    }
    list.remove(index);
    if let Err(e) = app.hist.exec(&mut app.doc, designos_doc::Command::SetGuides { page, guides: list }) {
        app.show_toast(format!("Guide edit failed: {e}"), true);
        return false;
    }
    app.changed();
    true
}

/// Draws the guides (under the strips) and the rulers.
pub fn overlay(app: &App, ui: &Ui, rect: ERect) {
    if !app.ui.rulers {
        return;
    }
    let p = ui.painter().with_clip_rect(rect);
    let (top, left) = strips(rect);
    let dragging = match &app.canvas.inter {
        Interaction::Guide(g) => Some(*g),
        _ => None,
    };
    let to_s = |v: DV| app.view.to_screen(rect.min, v);
    let idx = super::interact::active_index(app);
    // Page guides span the canvas; frame guides only the frame they belong to.
    let line = |axis: GuideAxis, pos: f32, frame: Option<NodeId>| {
        let ext = frame.map(|f| super::fast_world_aabb(&app.doc, idx, f));
        match axis {
            GuideAxis::X => {
                let x = crisp(to_s(v2(pos, 0.0)).x);
                let (y0, y1) = ext.map(|b| (to_s(v2(0.0, b.y)).y, to_s(v2(0.0, b.y + b.h)).y)).unwrap_or((top.bottom(), rect.bottom()));
                [Pos2::new(x, y0.max(top.bottom())), Pos2::new(x, y1.min(rect.bottom()))]
            }
            GuideAxis::Y => {
                let y = crisp(to_s(v2(0.0, pos)).y);
                let (x0, x1) = ext.map(|b| (to_s(v2(b.x, 0.0)).x, to_s(v2(b.x + b.w, 0.0)).x)).unwrap_or((left.right(), rect.right()));
                [Pos2::new(x0.max(left.right()), y), Pos2::new(x1.min(rect.right()), y)]
            }
        }
    };
    for (i, g) in guides_of(app) {
        if dragging.is_some_and(|d| d.index == Some(i)) {
            continue;
        }
        p.line_segment(line(g.axis, g.pos, g.frame), Stroke::new(1.0, theme::RULER_GUIDE));
    }
    if let Some(d) = dragging.filter(|d| !d.on_ruler) {
        p.line_segment(line(d.axis, d.pos, d.frame), Stroke::new(1.0, theme::RULER_GUIDE));
        // value badge next to the pointer
        if let Some(ptr) = ui.input(|i| i.pointer.latest_pos()) {
            let o = d.frame.and_then(|f| frame_origin(&app.doc, f)).unwrap_or_else(|| origin(app));
            let v = d.pos - axis_of(o, d.axis);
            let axis_lbl = match d.axis {
                GuideAxis::X => "X",
                GuideAxis::Y => "Y",
            };
            let text = format!("{axis_lbl}: {}", fmt_value(v));
            let font = theme::font_caption_strong();
            let w = crate::widgets::text_width(&p, &text, &font) + theme::CANVAS_BADGE_PAD_X * 2.0;
            let r = ERect::from_min_size(ptr + EV::new(12.0, 12.0), EV::new(w, theme::CANVAS_BADGE_HEIGHT));
            p.rect_filled(r, theme::cr(theme::CANVAS_BADGE_RADIUS), theme::RULER_GUIDE);
            p.text(r.center(), Align2::CENTER_CENTER, text, font, Color32::WHITE);
        }
    }

    // ---- strips
    let bg = theme::BG;
    p.rect_filled(top, 0.0, bg);
    p.rect_filled(left, 0.0, bg);
    p.rect_filled(ERect::from_min_size(rect.min, EV::splat(theme::RULER_SIZE)), 0.0, bg);
    p.line_segment([Pos2::new(rect.left(), top.bottom() - 0.5), Pos2::new(rect.right(), top.bottom() - 0.5)], Stroke::new(1.0, theme::BORDER));
    p.line_segment([Pos2::new(left.right() - 0.5, rect.top()), Pos2::new(left.right() - 0.5, rect.bottom())], Stroke::new(1.0, theme::BORDER));

    let o = origin(app);
    let z = app.view.zoom;
    let tick = Stroke::new(1.0, theme::RULER_TICK);
    let font = theme::font_ruler();
    // horizontal ruler
    {
        let lo = app.view.to_page(rect.min, Pos2::new(top.left(), 0.0)).x - o.x;
        let hi = app.view.to_page(rect.min, Pos2::new(top.right(), 0.0)).x - o.x;
        let tp = p.with_clip_rect(top);
        for (v, level) in ticks(lo, hi, z) {
            let x = crisp(to_s(v2(v + o.x, 0.0)).x);
            let len = [theme::RULER_SIZE - 6.0, 6.0, 3.0][level as usize];
            tp.line_segment([Pos2::new(x, top.bottom() - len), Pos2::new(x, top.bottom())], tick);
            if level == 0 {
                tp.text(Pos2::new(x + 3.0, top.top() + 2.0), Align2::LEFT_TOP, fmt_value(v), font.clone(), theme::RULER_TEXT);
            }
        }
    }
    // vertical ruler
    {
        let lo = app.view.to_page(rect.min, Pos2::new(0.0, left.top())).y - o.y;
        let hi = app.view.to_page(rect.min, Pos2::new(0.0, left.bottom())).y - o.y;
        let lp = p.with_clip_rect(left);
        for (v, level) in ticks(lo, hi, z) {
            let y = crisp(to_s(v2(0.0, v + o.y)).y);
            let len = [theme::RULER_SIZE - 6.0, 6.0, 3.0][level as usize];
            lp.line_segment([Pos2::new(left.right() - len, y), Pos2::new(left.right(), y)], tick);
            if level == 0 {
                rotated_text(&lp, Pos2::new(left.left() + 2.0, y - 3.0), &fmt_value(v), theme::RULER_TEXT);
            }
        }
    }

    // ---- selection extent
    let sel_box = union_all(app.selection.iter().take(256).map(|i| super::fast_world_aabb(&app.doc, idx, *i)));
    if let Some(bb) = sel_box {
        let (x0, x1) = (to_s(v2(bb.x, 0.0)).x, to_s(v2(bb.x + bb.w, 0.0)).x);
        let (y0, y1) = (to_s(v2(0.0, bb.y)).y, to_s(v2(0.0, bb.y + bb.h)).y);
        let tp = p.with_clip_rect(top);
        let band = ERect::from_min_max(Pos2::new(x0.max(top.left()), top.top()), Pos2::new(x1.max(top.left()), top.bottom()));
        tp.rect_filled(band, 0.0, theme::RULER_SELECTION_FILL);
        let lp = p.with_clip_rect(left);
        let band = ERect::from_min_max(Pos2::new(left.left(), y0.max(left.top())), Pos2::new(left.right(), y1.max(left.top())));
        lp.rect_filled(band, 0.0, theme::RULER_SELECTION_FILL);
        for (x, v) in [(x0, bb.x - o.x), (x1, bb.x + bb.w - o.x)] {
            let text = fmt_value(v);
            let w = crate::widgets::text_width(&p, &text, &font) + 4.0;
            let r = ERect::from_center_size(Pos2::new(x, top.center().y), EV::new(w, 12.0));
            tp.rect_filled(r, 2.0, bg);
            tp.text(r.center(), Align2::CENTER_CENTER, text, font.clone(), theme::CANVAS_SELECTION);
        }
        for (y, v) in [(y0, bb.y - o.y), (y1, bb.y + bb.h - o.y)] {
            let text = fmt_value(v);
            let w = crate::widgets::text_width(&p, &text, &font) + 4.0;
            let r = ERect::from_center_size(Pos2::new(left.center().x, y), EV::new(12.0, w));
            lp.rect_filled(r, 2.0, bg);
            let galley = lp.layout_no_wrap(text, font.clone(), theme::CANVAS_SELECTION);
            let (gw, gh) = (galley.size().x, galley.size().y);
            lp.add(
                egui::epaint::TextShape::new(Pos2::new(r.center().x - gh / 2.0, r.center().y + gw / 2.0), galley, theme::CANVAS_SELECTION)
                    .with_angle(-std::f32::consts::FRAC_PI_2),
            );
        }
    }
}

/// Cursor for the point under the pointer: resize arrows over rulers and guides.
pub fn cursor_at(app: &App, rect: ERect, pos: Pos2) -> Option<egui::CursorIcon> {
    if !app.ui.rulers || app.canvas.text.is_some() {
        return None;
    }
    let (top, left) = strips(rect);
    if top.contains(pos) {
        return Some(egui::CursorIcon::ResizeVertical);
    }
    if left.contains(pos) {
        return Some(egui::CursorIcon::ResizeHorizontal);
    }
    hit_guide(app, rect, pos).map(|(_, g)| if g.axis == GuideAxis::X { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_nice_and_wide_enough() {
        assert_eq!(tick_step(1.0), 50.0);
        assert_eq!(tick_step(0.1), 500.0);
        assert_eq!(tick_step(8.0), 10.0);
        assert_eq!(tick_step(100.0), 1.0);
        assert_eq!(tick_step(0.02), 5000.0);
        for z in [0.013f32, 0.3, 0.77, 1.0, 2.5, 13.0, 200.0] {
            let s = tick_step(z);
            assert!(s * z >= 50.0 && (s / 1.0).fract() == 0.0, "zoom {z} step {s}");
        }
    }

    #[test]
    fn ticks_cover_range_with_majors_on_steps() {
        let t = ticks(-30.0, 260.0, 2.0);
        let majors: Vec<f32> = t.iter().filter(|(_, l)| *l == 0).map(|(v, _)| *v).collect();
        assert!(majors.contains(&0.0) && majors.contains(&50.0) && majors.contains(&250.0));
        assert!(majors.iter().all(|v| (v / 50.0).fract() == 0.0));
        assert!(t.iter().any(|(_, l)| *l == 1));
        assert!(t.first().unwrap().0 <= -30.0 && t.last().unwrap().0 >= 260.0);
        // ticks are ordered and unique
        assert!(t.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn values_format_without_noise() {
        assert_eq!(fmt_value(100.0), "100");
        assert_eq!(fmt_value(-0.0), "0");
        assert_eq!(fmt_value(2.5), "2.5");
    }
}
