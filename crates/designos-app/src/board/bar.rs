//! The floating contextual toolbar of a board: it replaces the right panel and hovers above the
//! selection with what applies to it, colour swatches, shape, line style, end caps, text size and
//! alignment. Every change is one undoable edit (or part of the text session being typed in).

use super::toolbar::swatch;
use super::{color_of, rgb_of, BarMenu};
use crate::app::App;
use crate::canvas::Interaction;
use crate::icons::{self, Icon};
use crate::layout;
use crate::theme;
use crate::util::*;
use crate::widgets;
use designos_doc::*;
use egui::{Align2, Color32, Pos2, Rect as ERect, Sense, Stroke, StrokeKind, Ui, Vec2 as EV};

/// Text sizes the +/- buttons step through.
pub const SIZES: [f32; 9] = [12.0, 14.0, 16.0, 20.0, 24.0, 32.0, 40.0, 48.0, 64.0];

/// What the bar offers for the current selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caps {
    pub fill: bool,
    pub ink: bool,
    pub shape: bool,
    pub line: bool,
    pub text: bool,
    /// Every selected node is a sticky note (the author line toggle).
    pub author: bool,
}

/// Which controls apply to the nodes `ids`.
pub fn caps_of(doc: &Document, ids: &[NodeId]) -> Caps {
    let mut c = Caps::default();
    let kinds: Vec<&NodeKind> = ids.iter().filter_map(|i| doc.get(*i)).map(|n| &n.kind).collect();
    if kinds.is_empty() {
        return c;
    }
    for k in &kinds {
        match k {
            NodeKind::Sticky(_) | NodeKind::ShapeWithText(_) | NodeKind::Section => c.fill = true,
            NodeKind::Connector(_) | NodeKind::Vector { .. } | NodeKind::Text(_) => c.ink = true,
            _ => {}
        }
        if k.any_text().is_some() && !matches!(k, NodeKind::Connector(_)) {
            c.text = true;
        }
    }
    c.shape = kinds.iter().all(|k| matches!(k, NodeKind::ShapeWithText(_)));
    c.line = kinds.iter().all(|k| matches!(k, NodeKind::Connector(_)));
    c.author = kinds.iter().all(|k| matches!(k, NodeKind::Sticky(_)));
    // a mixed selection keeps to the controls every node has
    if c.fill && c.ink {
        c.ink = false;
    }
    c
}

/// Sets the colour `rgb` on `n` the way its kind takes one: sticky colour, shape / section fill,
/// stroke of connectors and marker strokes, text colour of text layers.
pub fn set_color(n: &mut Node, rgb: u32) {
    let c = color_of(rgb);
    match &mut n.kind {
        NodeKind::Sticky(s) => s.color = c,
        NodeKind::ShapeWithText(_) | NodeKind::Section => n.fills = vec![Paint::solid(c)],
        NodeKind::Connector(_) => n.stroke.paints = vec![Paint::solid(c)],
        NodeKind::Vector { .. } => {
            let a = n.stroke.paints.first().map(paint_alpha).unwrap_or(1.0);
            n.stroke.paints = vec![Paint::solid(c.with_alpha(a))];
        }
        NodeKind::Text(t) => t.style.fills = vec![Paint::solid(c)],
        _ => {}
    }
}

fn paint_alpha(p: &Paint) -> f32 {
    match &p.kind {
        PaintKind::Solid(c) => c.a,
        _ => 1.0,
    }
}

/// The colour `n` currently shows in the bar's swatches.
pub fn current_color(n: &Node) -> Option<u32> {
    match &n.kind {
        NodeKind::Sticky(s) => Some(rgb_of(s.color)),
        NodeKind::ShapeWithText(_) | NodeKind::Section => n.fills.first().and_then(super::create::solid_rgb),
        NodeKind::Connector(_) | NodeKind::Vector { .. } => n.stroke.paints.first().and_then(super::create::solid_rgb),
        NodeKind::Text(t) => t.style.fills.first().and_then(super::create::solid_rgb),
        _ => None,
    }
}

/// Swatch palette for the selection.
fn palette(caps: Caps) -> Vec<u32> {
    if caps.ink {
        theme::BOARD_INK.to_vec()
    } else {
        let mut v = theme::BOARD_PALETTE.to_vec();
        if caps.shape {
            v.push(0xFFFFFF);
        }
        v
    }
}

/// Next size above or below `cur` in [`SIZES`].
pub fn step_size(cur: f32, up: bool) -> f32 {
    if up {
        SIZES.iter().copied().find(|s| *s > cur + 0.01).unwrap_or(SIZES[SIZES.len() - 1])
    } else {
        SIZES.iter().rev().copied().find(|s| *s < cur - 0.01).unwrap_or(SIZES[0])
    }
}

/// Applies `f` to the nodes as one undo step, or as part of the text session when one of them is
/// being typed into.
pub fn edit(app: &mut App, ids: &[NodeId], mut f: impl FnMut(&mut Node)) {
    if let (Some(sess), [only]) = (app.canvas.text.as_mut(), ids) {
        if sess.id == *only {
            let App { doc, canvas, .. } = app;
            if let Some(sess) = canvas.text.as_mut() {
                if let Some(n) = sess.tx.ed.node(doc, *only) {
                    f(n);
                }
                layout::relayout_up(doc, &mut sess.tx.ed, *only);
            }
            app.changed();
            return;
        }
    }
    app.edit_nodes(ids, f);
}

fn set_text_size(app: &mut App, ids: &[NodeId], size: f32) {
    edit(app, ids, |n| {
        let is_text = matches!(n.kind, NodeKind::Text(_));
        if let Some(t) = n.kind.any_text_mut() {
            t.style.font_size = size;
            for r in &mut t.runs {
                r.style.font_size = size;
            }
            if is_text {
                n.size = layout::fit_text(t, n.size);
            }
        }
    });
}

fn set_align(app: &mut App, ids: &[NodeId], a: TextAlignH) {
    edit(app, ids, |n| {
        if let Some(t) = n.kind.any_text_mut() {
            t.style.align_h = a;
        }
    });
}

/// Draws one end cap sample into `r`: a short line with the cap at its right end (`flip` mirrors it
/// for the start of a connector).
fn cap_glyph(p: &egui::Painter, r: ERect, cap: ConnectorCap, flip: bool, col: Color32) {
    let (x0, x1) = (r.left() + 5.0, r.right() - 5.0);
    let y = r.center().y;
    let (a, b) = if flip { (x1, x0) } else { (x0, x1) };
    let dir = if flip { -1.0 } else { 1.0 };
    let line_end = match cap {
        ConnectorCap::Triangle | ConnectorCap::Diamond => b - dir * 5.0,
        ConnectorCap::Circle => b - dir * 3.0,
        _ => b,
    };
    p.line_segment([Pos2::new(a, y), Pos2::new(line_end, y)], Stroke::new(1.5, col));
    let tip = Pos2::new(b, y);
    let pt = |along: f32, across: f32| Pos2::new(tip.x - dir * along, tip.y + across);
    match cap {
        ConnectorCap::None => {}
        ConnectorCap::Arrow => {
            p.line_segment([pt(4.5, -3.5), tip], Stroke::new(1.5, col));
            p.line_segment([pt(4.5, 3.5), tip], Stroke::new(1.5, col));
        }
        ConnectorCap::Triangle => {
            p.add(egui::Shape::convex_polygon(vec![tip, pt(6.0, -3.5), pt(6.0, 3.5)], col, Stroke::NONE));
        }
        ConnectorCap::Diamond => {
            p.add(egui::Shape::convex_polygon(vec![tip, pt(3.5, -3.0), pt(7.0, 0.0), pt(3.5, 3.0)], col, Stroke::NONE));
        }
        ConnectorCap::Circle => {
            p.circle_filled(pt(3.0, 0.0), 3.2, col);
        }
    }
}

/// The bar's rectangle for a selection occupying `sel` (screen), `width` wide: above it, or below
/// when there is no room, kept inside the canvas.
pub fn place(canvas: ERect, sel: ERect, width: f32) -> ERect {
    let h = theme::BOARD_BAR_H;
    let x = (sel.center().x - width / 2.0).clamp(canvas.left() + 8.0, (canvas.right() - width - 8.0).max(canvas.left() + 8.0));
    let above = sel.top() - theme::BOARD_BAR_GAP - h;
    let y = if above >= canvas.top() + 8.0 { above } else { (sel.bottom() + theme::BOARD_BAR_GAP).min(canvas.bottom() - h - 72.0) };
    ERect::from_min_size(Pos2::new(x, y), EV::new(width, h))
}

/// Paints the bar for the current selection and applies clicks.
pub fn show(app: &mut App, ui: &mut Ui, canvas: ERect) {
    if !app.is_board() || app.selection.is_empty() || app.canvas.vedit.is_some() {
        app.board.bar_menu = None;
        return;
    }
    // while dragging, drawing or resizing the bar stays out of the way
    if !matches!(app.canvas.inter, Interaction::None) || app.canvas.pen.is_some() || app.history.viewing.is_some() {
        return;
    }
    let ids = app.top_selection();
    let caps = caps_of(&app.doc, &ids);
    if !(caps.fill || caps.ink || caps.shape || caps.line || caps.text || caps.author) {
        return;
    }
    let Some(sel) = app.selection_bounds() else { return };
    let (a, b) = (app.view.to_screen(canvas.min, v2(sel.x, sel.y)), app.view.to_screen(canvas.min, v2(sel.x + sel.w, sel.y + sel.h)));
    let sel_screen = ERect::from_two_pos(a, b);
    if !sel_screen.intersects(canvas) {
        return;
    }

    let colors = if caps.fill || caps.ink { palette(caps) } else { vec![] };
    let first = app.doc.get(ids[0]).cloned();
    let cur_color = first.as_ref().and_then(current_color);
    let cur_text = first.as_ref().and_then(|n| n.kind.any_text().cloned());
    let (cur_line, cur_caps) = match first.as_ref().map(|n| &n.kind) {
        Some(NodeKind::Connector(c)) => (Some(c.line), Some((c.start_cap, c.end_cap))),
        _ => (None, None),
    };
    let cur_shape = match first.as_ref().map(|n| &n.kind) {
        Some(NodeKind::ShapeWithText(s)) => Some(s.shape),
        _ => None,
    };

    // ---- layout: items left to right, widths first so the bar can be centred
    const SW: f32 = 24.0;
    const BTN: f32 = theme::BOARD_BAR_BTN;
    let sep = theme::BOARD_BAR_SEP;
    let mut width = theme::BOARD_BAR_PAD * 2.0;
    let mut groups = 0;
    let add = |w: f32, width: &mut f32, groups: &mut i32| {
        if *groups > 0 {
            *width += sep;
        }
        *groups += 1;
        *width += w;
    };
    if !colors.is_empty() {
        add(colors.len() as f32 * SW, &mut width, &mut groups);
    }
    if caps.shape {
        add(BTN + 12.0, &mut width, &mut groups);
    }
    if caps.line {
        add(3.0 * BTN, &mut width, &mut groups);
        add(2.0 * BTN, &mut width, &mut groups);
    }
    if caps.text {
        add(2.0 * BTN + 30.0, &mut width, &mut groups);
        add(3.0 * BTN, &mut width, &mut groups);
    }
    if caps.author {
        add(BTN, &mut width, &mut groups);
    }
    let bar = place(canvas, sel_screen, width);

    let p = ui.painter().clone();
    widgets::shadow(&p, bar, theme::RADIUS_FRAME, EV::new(0.0, 3.0), 8.0, theme::SHADOW_COLOR);
    widgets::shadow(&p, bar, theme::RADIUS_FRAME, EV::new(0.0, 1.0), 3.0, theme::SHADOW_COLOR_STRONG);
    p.rect_filled(bar, theme::cr(theme::RADIUS_FRAME), theme::BG_TOOLBAR);
    p.rect_stroke(bar.shrink(0.5), theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::INNER_HIGHLIGHT), StrokeKind::Inside);
    ui.interact(bar, ui.id().with("board-bar-block"), Sense::click_and_drag());
    widgets::record(ui, "board-bar", bar);

    let cy = bar.center().y;
    let mut x = bar.left() + theme::BOARD_BAR_PAD;
    let mut first_group = true;
    let mut divider = |x: &mut f32, p: &egui::Painter| {
        if !first_group {
            widgets::vline(p, *x + sep / 2.0, bar.top() + 8.0, bar.bottom() - 8.0, theme::BORDER);
            *x += sep;
        }
        first_group = false;
    };

    // swatches
    if !colors.is_empty() {
        divider(&mut x, &p);
        for (i, c) in colors.iter().enumerate() {
            let r = ERect::from_min_size(Pos2::new(x, cy - SW / 2.0), EV::splat(SW));
            let resp = ui.interact(r, ui.id().with(("board-swatch", i)), Sense::click());
            swatch(&p, r.center(), *c, cur_color == Some(*c), resp.hovered());
            widgets::record(ui, &format!("board-swatch-{i}"), r);
            if resp.clicked() {
                let c = *c;
                if caps.fill && !caps.shape {
                    // the next sticky note starts out in the colour just picked
                    app.board.color = color_of(c);
                }
                if caps.ink {
                    app.board.ink = color_of(c);
                }
                edit(app, &ids, |n| set_color(n, c));
            }
            x += SW;
        }
    }

    // shape picker
    if caps.shape {
        divider(&mut x, &p);
        let r = ERect::from_min_size(Pos2::new(x, cy - BTN / 2.0), EV::new(BTN + 12.0, BTN));
        let open = matches!(app.board.bar_menu, Some((BarMenu::Shape, _)));
        let resp = ui.interact(r, ui.id().with("board-shape-btn"), Sense::click());
        widgets::button_bg(&p, r, &resp, open, theme::cr(theme::RADIUS_CONTROL));
        if let Some(t) = cur_shape.and_then(|k| app.board.previews.shape(ui.ctx(), k, color_of(0xE6E6E6), 16.0, 12.0)) {
            p.image(
                t,
                ERect::from_center_size(r.center() - EV::new(5.0, 0.0), EV::new(16.0, 12.0)),
                ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        icons::paint(&p, Icon::ChevronDown, Pos2::new(r.right() - 8.0, cy), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
        widgets::record(ui, "board-shape-btn", r);
        if resp.on_hover_text("Shape").clicked() {
            app.board.bar_menu = if open { None } else { Some((BarMenu::Shape, r)) };
        }
        x += r.width();
    }

    // connector line style and end caps
    if caps.line {
        divider(&mut x, &p);
        for (line, icon, tip) in [
            (ConnectorLine::Elbowed, Icon::Connector, "Elbowed"),
            (ConnectorLine::Straight, Icon::Line, "Straight"),
            (ConnectorLine::Curved, Icon::Vector, "Curved"),
        ] {
            let r = ERect::from_min_size(Pos2::new(x, cy - BTN / 2.0), EV::splat(BTN));
            let resp = ui.interact(r, ui.id().with(("board-line", tip)), Sense::click());
            let sel = cur_line == Some(line);
            widgets::button_bg(&p, r, &resp, sel, theme::cr(theme::RADIUS_CONTROL));
            widgets::paint_icon_in(&p, r, icon, 16.0, if sel { theme::ICON_SELECTED } else { theme::ICON });
            widgets::record(ui, &format!("board-line-{line:?}"), r);
            if resp.on_hover_text(tip).clicked() {
                edit(app, &ids, |n| {
                    if let NodeKind::Connector(c) = &mut n.kind {
                        c.line = line;
                    }
                });
            }
            x += BTN;
        }
        divider(&mut x, &p);
        for (menu, flip, tip) in [(BarMenu::StartCap, true, "Start cap"), (BarMenu::EndCap, false, "End cap")] {
            let r = ERect::from_min_size(Pos2::new(x, cy - BTN / 2.0), EV::splat(BTN));
            let resp = ui.interact(r, ui.id().with(("board-cap-btn", tip)), Sense::click());
            let open = app.board.bar_menu.map(|m| m.0) == Some(menu);
            widgets::button_bg(&p, r, &resp, open, theme::cr(theme::RADIUS_CONTROL));
            let cap = cur_caps.map(|c| if flip { c.0 } else { c.1 }).unwrap_or_default();
            cap_glyph(&p, r.shrink(2.0), cap, flip, theme::ICON);
            widgets::record(ui, &format!("board-cap-{}", if flip { "start" } else { "end" }), r);
            if resp.on_hover_text(tip).clicked() {
                app.board.bar_menu = if open { None } else { Some((menu, r)) };
            }
            x += BTN;
        }
    }

    // text size and alignment
    if caps.text {
        divider(&mut x, &p);
        let size = cur_text.as_ref().map(|t| t.style.font_size).unwrap_or(16.0);
        let minus = ERect::from_min_size(Pos2::new(x, cy - BTN / 2.0), EV::splat(BTN));
        let plus = ERect::from_min_size(Pos2::new(x + BTN + 30.0, cy - BTN / 2.0), EV::splat(BTN));
        for (r, icon, up, key) in [(minus, Icon::Minus, false, "board-size-minus"), (plus, Icon::Plus, true, "board-size-plus")] {
            let resp = ui.interact(r, ui.id().with(key), Sense::click());
            widgets::button_bg(&p, r, &resp, false, theme::cr(theme::RADIUS_CONTROL));
            widgets::paint_icon_in(&p, r, icon, 14.0, theme::ICON);
            widgets::record(ui, key, r);
            if resp.on_hover_text(if up { "Larger text" } else { "Smaller text" }).clicked() {
                set_text_size(app, &ids, step_size(size, up));
            }
        }
        p.text(Pos2::new(x + BTN + 15.0, cy), Align2::CENTER_CENTER, format!("{}", size.round() as i64), theme::font_body_strong(), theme::TEXT);
        x += 2.0 * BTN + 30.0;
        divider(&mut x, &p);
        let cur = cur_text.as_ref().map(|t| t.style.align_h).unwrap_or_default();
        for (a, icon, key) in [
            (TextAlignH::Left, Icon::TextLeft, "board-align-left"),
            (TextAlignH::Center, Icon::TextCenter, "board-align-center"),
            (TextAlignH::Right, Icon::TextRight, "board-align-right"),
        ] {
            let r = ERect::from_min_size(Pos2::new(x, cy - BTN / 2.0), EV::splat(BTN));
            let resp = ui.interact(r, ui.id().with(key), Sense::click());
            let sel = cur == a;
            widgets::button_bg(&p, r, &resp, sel, theme::cr(theme::RADIUS_CONTROL));
            widgets::paint_icon_in(&p, r, icon, 16.0, if sel { theme::ICON_SELECTED } else { theme::ICON });
            widgets::record(ui, key, r);
            if resp.clicked() {
                set_align(app, &ids, a);
            }
            x += BTN;
        }
    }

    // author line of sticky notes
    if caps.author {
        divider(&mut x, &p);
        let shown = matches!(first.as_ref().map(|n| &n.kind), Some(NodeKind::Sticky(s)) if s.show_author);
        let r = ERect::from_min_size(Pos2::new(x, cy - BTN / 2.0), EV::splat(BTN));
        let resp = ui.interact(r, ui.id().with("board-author"), Sense::click());
        widgets::button_bg(&p, r, &resp, shown, theme::cr(theme::RADIUS_CONTROL));
        widgets::paint_icon_in(&p, r, if shown { Icon::Eye } else { Icon::EyeOff }, 16.0, if shown { theme::ICON_SELECTED } else { theme::ICON });
        widgets::record(ui, "board-author", r);
        if resp.on_hover_text("Show author").clicked() {
            let name = super::create::author_name();
            edit(app, &ids, |n| {
                if let NodeKind::Sticky(s) = &mut n.kind {
                    s.show_author = !shown;
                    if s.author.is_none() {
                        s.author = Some(name.clone());
                    }
                }
            });
        }
    }

    popup(app, ui, &ids);
}

/// The shape grid or cap list hanging off the bar.
fn popup(app: &mut App, ui: &mut Ui, ids: &[NodeId]) {
    let Some((menu, anchor)) = app.board.bar_menu else { return };
    let ctx = ui.ctx().clone();
    let pad = 8.0;
    let (size, rows_cols) = match menu {
        BarMenu::Shape => (EV::new(5.0 * 40.0 + pad * 2.0, 2.0 * 40.0 + pad * 2.0), (2, 5)),
        BarMenu::StartCap | BarMenu::EndCap => (EV::new(5.0 * 36.0 + pad * 2.0, 36.0 + pad * 2.0), (1, 5)),
    };
    let screen = ctx.content_rect();
    let above = anchor.top() - 8.0 - size.y;
    let y = if above >= screen.top() + 8.0 { above } else { anchor.bottom() + 8.0 };
    let rect = ERect::from_min_size(Pos2::new((anchor.center().x - size.x / 2.0).clamp(screen.left() + 8.0, screen.right() - size.x - 8.0), y), size);
    let mut picked: Option<Pick> = None;
    let area = egui::Area::new(egui::Id::new("board-bar-menu")).order(egui::Order::Foreground).fixed_pos(rect.min).show(&ctx, |ui| {
        ui.allocate_rect(rect, Sense::hover());
        let p = ui.painter().clone();
        widgets::menu_panel(&p, rect, theme::RADIUS_FRAME);
        let inner = rect.shrink(pad);
        match menu {
            BarMenu::Shape => {
                let cur = ids.first().and_then(|i| app.doc.get(*i)).and_then(|n| if let NodeKind::ShapeWithText(s) = &n.kind { Some(s.shape) } else { None });
                for (i, k) in ShapeKind::ALL.iter().enumerate() {
                    let cell = ERect::from_min_size(inner.min + EV::new((i % rows_cols.1) as f32 * 40.0, (i / rows_cols.1) as f32 * 40.0), EV::splat(40.0));
                    let resp = ui.interact(cell, ui.id().with(("board-bar-shape", i)), Sense::click());
                    let r = cell.shrink(2.0);
                    if cur == Some(*k) {
                        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
                    } else if resp.hovered() {
                        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                    }
                    if let Some(t) = app.board.previews.shape(ui.ctx(), *k, color_of(0xE6E6E6), 24.0, 18.0) {
                        p.image(
                            t,
                            ERect::from_center_size(cell.center(), EV::new(24.0, 18.0)),
                            ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    widgets::record(ui, &format!("board-bar-shape-{k:?}"), cell);
                    if resp.on_hover_text(k.label()).clicked() {
                        picked = Some(Pick::Shape(*k));
                    }
                }
            }
            BarMenu::StartCap | BarMenu::EndCap => {
                let start = menu == BarMenu::StartCap;
                let cur = ids.first().and_then(|i| app.doc.get(*i)).and_then(|n| {
                    if let NodeKind::Connector(c) = &n.kind {
                        Some(if start { c.start_cap } else { c.end_cap })
                    } else {
                        None
                    }
                });
                for (i, cap) in ConnectorCap::ALL.iter().enumerate() {
                    let cell = ERect::from_min_size(inner.min + EV::new(i as f32 * 36.0, 0.0), EV::splat(36.0));
                    let resp = ui.interact(cell, ui.id().with(("board-bar-cap", i)), Sense::click());
                    let r = cell.shrink(2.0);
                    if cur == Some(*cap) {
                        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
                    } else if resp.hovered() {
                        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                    }
                    cap_glyph(&p, r, *cap, start, theme::ICON);
                    widgets::record(ui, &format!("board-bar-cap-{cap:?}"), cell);
                    if resp.clicked() {
                        picked = Some(Pick::Cap(start, *cap));
                    }
                }
            }
        }
    });
    let layer = area.response.layer_id;
    let (pressed, ptr, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
    let other_popup = |p: Pos2| ctx.layer_id_at(p).is_some_and(|l| l.order == egui::Order::Foreground && l != layer);
    let outside = pressed && ptr.is_some_and(|p| !rect.contains(p) && !anchor.contains(p) && !other_popup(p));
    match picked {
        Some(Pick::Shape(k)) => {
            edit(app, ids, |n| {
                if let NodeKind::ShapeWithText(s) = &mut n.kind {
                    s.shape = k;
                }
            });
            app.board.shape = k;
            app.board.bar_menu = None;
        }
        Some(Pick::Cap(start, cap)) => {
            edit(app, ids, |n| {
                if let NodeKind::Connector(c) = &mut n.kind {
                    if start {
                        c.start_cap = cap;
                    } else {
                        c.end_cap = cap;
                    }
                }
            });
            app.board.bar_menu = None;
        }
        None => {
            if outside || esc {
                app.board.bar_menu = None;
            }
        }
    }
}

enum Pick {
    Shape(ShapeKind),
    Cap(bool, ConnectorCap),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with(kinds: Vec<Node>) -> (Document, Vec<NodeId>) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut ids = Vec::new();
        for mut n in kinds {
            n.id = d.alloc_id();
            ids.push(d.insert(page, n).unwrap());
        }
        (d, ids)
    }

    #[test]
    fn controls_follow_the_selected_kinds() {
        let (d, ids) = doc_with(vec![
            Node::sticky(NodeId(0), Rect::new(0.0, 0.0, 10.0, 10.0), ""),
            Node::shape_with_text(NodeId(0), Rect::new(0.0, 0.0, 10.0, 10.0), ShapeKind::Square, ""),
            Node::connector(NodeId(0), ConnectorProps::default()),
        ]);
        let sticky = caps_of(&d, &ids[..1]);
        assert_eq!(sticky, Caps { fill: true, ink: false, shape: false, line: false, text: true, author: true });
        let shape = caps_of(&d, &ids[1..2]);
        assert!(shape.fill && shape.shape && shape.text && !shape.line);
        let conn = caps_of(&d, &ids[2..]);
        assert_eq!(conn, Caps { fill: false, ink: true, shape: false, line: true, text: false, author: false });
        let mixed = caps_of(&d, &ids[..2]);
        assert!(mixed.fill && !mixed.shape && mixed.text);
    }

    #[test]
    fn colours_go_where_each_kind_keeps_them() {
        let mut s = Node::sticky(NodeId(1), Rect::new(0.0, 0.0, 10.0, 10.0), "");
        set_color(&mut s, 0xB6D4FF);
        assert_eq!(current_color(&s), Some(0xB6D4FF));
        let mut sh = Node::shape_with_text(NodeId(2), Rect::new(0.0, 0.0, 10.0, 10.0), ShapeKind::Square, "");
        set_color(&mut sh, 0xFFB3B3);
        assert_eq!(current_color(&sh), Some(0xFFB3B3));
        assert_eq!(sh.fills.len(), 1);
        let mut c = Node::connector(NodeId(3), ConnectorProps::default());
        set_color(&mut c, 0xF24822);
        assert_eq!(current_color(&c), Some(0xF24822));
    }

    #[test]
    fn size_steps_walk_the_scale() {
        assert_eq!(step_size(24.0, true), 32.0);
        assert_eq!(step_size(24.0, false), 20.0);
        assert_eq!(step_size(28.0, true), 32.0);
        assert_eq!(step_size(28.0, false), 24.0);
        assert_eq!(step_size(64.0, true), 64.0);
        assert_eq!(step_size(12.0, false), 12.0);
    }

    #[test]
    fn bar_sits_above_the_selection_or_below_when_cramped() {
        let canvas = ERect::from_min_size(Pos2::new(0.0, 0.0), EV::new(1000.0, 800.0));
        let sel = ERect::from_min_size(Pos2::new(400.0, 400.0), EV::new(200.0, 100.0));
        let b = place(canvas, sel, 300.0);
        assert!(b.bottom() < sel.top());
        assert!((b.center().x - sel.center().x).abs() < 1.0);
        let top = ERect::from_min_size(Pos2::new(400.0, 10.0), EV::new(200.0, 100.0));
        let b = place(canvas, top, 300.0);
        assert!(b.top() > top.bottom());
        let edge = ERect::from_min_size(Pos2::new(0.0, 400.0), EV::new(20.0, 20.0));
        assert!(place(canvas, edge, 300.0).left() >= 8.0);
    }
}
