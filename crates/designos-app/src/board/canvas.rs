//! Canvas painting of board mode: the dotted background and the magnet dots.

use super::connect;
use crate::app::App;
use crate::theme;
use crate::util::*;
use designos_doc::NodeKind;
use egui::{Color32, Mesh, Painter, Pos2, Rect as ERect, Stroke, Ui, Vec2 as EV};

/// Page colour of the board (its alpha is zero in the file, so the dots below show through the tiles).
fn page_color(app: &App) -> Color32 {
    match app.doc.get(app.page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => super::color32(*background),
        _ => theme::rgb(theme::BOARD_BG),
    }
}

/// Spacing of the dot grid in page units at the current zoom: doubles while the dots would be
/// closer than [`theme::BOARD_DOT_MIN_PX`] on screen.
pub fn dot_step(zoom: f32) -> f32 {
    let mut step = theme::BOARD_DOT_SPACING;
    while step * zoom < theme::BOARD_DOT_MIN_PX && step < 1.0e7 {
        step *= 2.0;
    }
    step
}

/// Paints the board's canvas colour and a grid of dots under the page tiles.
pub fn background(app: &App, ui: &Ui, rect: ERect) {
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 0.0, page_color(app));
    let z = app.view.zoom;
    let step = dot_step(z);
    let (ox, oy) = (app.view.offset.x, app.view.offset.y);
    let (w, h) = (rect.width() / z, rect.height() / z);
    let (i0, i1) = ((ox / step).floor() as i64, ((ox + w) / step).ceil() as i64);
    let (j0, j1) = ((oy / step).floor() as i64, ((oy + h) / step).ceil() as i64);
    if (i1 - i0).saturating_mul(j1 - j0) > 60_000 {
        return;
    }
    // dots keep a constant on-screen size, a little bigger when zoomed way in
    let d = (1.4 + z.min(4.0) * 0.15).clamp(1.5, 2.2);
    let mut mesh = Mesh::default();
    for j in j0..=j1 {
        for i in i0..=i1 {
            let c = Pos2::new(rect.left() + (i as f32 * step - ox) * z, rect.top() + (j as f32 * step - oy) * z);
            mesh.add_colored_rect(ERect::from_center_size(c, EV::splat(d)), theme::BOARD_DOT);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

/// Magnet dots of the hovered node and the drop target of a connector being dragged.
pub fn overlay(app: &App, p: &Painter, rect: ERect) {
    if !app.is_board() {
        return;
    }
    let pointer = app.canvas.pointer_page;
    let mut nodes: Vec<designos_doc::NodeId> = Vec::new();
    nodes.extend(app.board.dots);
    nodes.extend(app.board.target.filter(|t| Some(*t) != app.board.dots));
    for id in nodes {
        if app.doc.get(id).is_none() {
            continue;
        }
        if app.board.target == Some(id) {
            let corners = world_corners(&app.doc, id).map(|c| app.view.to_screen(rect.min, c));
            p.add(egui::Shape::closed_line(corners.to_vec(), Stroke::new(2.0, theme::BG_BRAND)));
        }
        for (_, at) in connect::dot_positions(app, id) {
            let c = app.view.to_screen(rect.min, at);
            let hot = pointer.is_some_and(|q| (app.view.to_screen(rect.min, q) - c).length() <= theme::BOARD_MAGNET_HIT);
            let r = theme::BOARD_MAGNET_RADIUS * if hot { 1.3 } else { 1.0 };
            p.circle_filled(c, r, Color32::WHITE);
            p.circle_stroke(c, r, Stroke::new(1.5, theme::BG_BRAND));
            if hot {
                p.circle_filled(c, r * 0.45, theme::BG_BRAND);
            }
        }
    }
}

/// Zoom percentage in the top right corner (a board has no right panel to hold it); a click
/// opens the usual zoom menu.
pub fn zoom_pill(app: &mut App, ui: &mut Ui, canvas: ERect) {
    use crate::app::MenuKind;
    use crate::icons::{self, Icon};
    let r = ERect::from_min_size(Pos2::new(canvas.right() - 8.0 - 68.0, canvas.top() + 8.0), EV::new(68.0, 28.0));
    let resp = ui.interact(r, ui.id().with("board-zoom"), egui::Sense::click());
    let open = app.ui.menu == Some(MenuKind::Zoom);
    let p = ui.painter().clone();
    p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG);
    crate::widgets::button_bg(&p, r, &resp, open, theme::cr(theme::RADIUS_CONTROL));
    p.rect_stroke(r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::INNER_HIGHLIGHT), egui::StrokeKind::Inside);
    p.text(
        Pos2::new(r.left() + 10.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{}%", (app.view.zoom * 100.0).round()),
        theme::font_body(),
        theme::TEXT,
    );
    icons::paint(&p, Icon::ChevronDown, Pos2::new(r.right() - 12.0, r.center().y), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
    crate::widgets::record(ui, "board-zoom", r);
    if resp.clicked() {
        app.ui.menu = if open { None } else { Some(MenuKind::Zoom) };
        app.ui.menu_anchor = Some(r);
    }
}
