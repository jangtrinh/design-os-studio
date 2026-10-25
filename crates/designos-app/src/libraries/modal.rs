//! The Libraries modal: publish this file, and enable or disable the libraries
//! other files published. Also the read-only preview of a library component
//! whose file is not on this device.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::widgets::{self, field};
use designos_doc::*;
use egui::{Align2, Color32, Context, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};

const HEADER_H: f32 = 48.0;
const PAD: f32 = 20.0;
const ROW_H: f32 = 60.0;
const CHANGE_ROW_H: f32 = 20.0;
const MAX_CHANGES_SHOWN: usize = 4;

pub(super) fn kind_label(kind: LibraryAssetKind) -> &'static str {
    match kind {
        LibraryAssetKind::Component => "Component",
        LibraryAssetKind::ComponentSet => "Component set",
        LibraryAssetKind::Style => "Style",
        LibraryAssetKind::Variable => "Variable",
        LibraryAssetKind::VariableCollection => "Collection",
    }
}

pub(super) fn kind_icon(kind: LibraryAssetKind) -> Icon {
    match kind {
        LibraryAssetKind::Component | LibraryAssetKind::ComponentSet => Icon::Component,
        LibraryAssetKind::Style => Icon::Design,
        LibraryAssetKind::Variable | LibraryAssetKind::VariableCollection => Icon::Variables,
    }
}

/// Modal rectangle of `w` x `h` (clamped to the window) centred on `screen`.
pub(super) fn modal_rect(screen: Rect, w: f32, h: f32) -> Rect {
    Rect::from_center_size(screen.center(), Vec2::new(w.min(screen.width() - 32.0), h.min(screen.height() - 64.0).max(240.0)))
}

/// Draws the scrim and a modal panel; returns true when the scrim was clicked.
pub(super) fn frame(ctx: &Context, id: &'static str, m: Rect, body: impl FnOnce(&mut Ui) -> bool) -> bool {
    let screen = ctx.content_rect();
    egui::Area::new(Id::new(id))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .constrain(false)
        .show(ctx, |ui| {
            ui.allocate_rect(screen, Sense::hover());
            let scrim = ui.interact(screen, Id::new((id, "scrim")), Sense::click_and_drag());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(120));
            let p = ui.painter().clone();
            widgets::elevation_400(&p, m, theme::RADIUS_FRAME);
            p.rect_filled(m, theme::cr(theme::RADIUS_FRAME), theme::BG);
            p.rect_stroke(m, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::ELEVATION_400_RING), StrokeKind::Inside);
            ui.interact(m, Id::new((id, "body")), Sense::click_and_drag());
            let closed = body(ui);
            closed || scrim.clicked() && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| !m.contains(p))
        })
        .inner
}

/// Title row with a close button; returns true when closed.
pub(super) fn header(ui: &mut Ui, m: Rect, title: &str, close_key: &'static str) -> bool {
    let p = ui.painter().clone();
    p.text(Pos2::new(m.left() + PAD, m.top() + HEADER_H / 2.0), Align2::LEFT_CENTER, title, theme::font_large_strong(), theme::TEXT);
    widgets::hline(&p, m.left(), m.right(), m.top() + HEADER_H - 1.0, theme::BORDER);
    let cb = Rect::from_center_size(Pos2::new(m.right() - 28.0, m.top() + HEADER_H / 2.0), Vec2::splat(28.0));
    let r = ui.interact(cb, Id::new(close_key), Sense::click());
    widgets::button_bg(&p, cb, &r, false, theme::cr(theme::RADIUS_CONTROL));
    widgets::record(ui, close_key, cb);
    let c = cb.center();
    let st = Stroke::new(1.3, theme::ICON);
    p.line_segment([c + Vec2::new(-4.5, -4.5), c + Vec2::new(4.5, 4.5)], st);
    p.line_segment([c + Vec2::new(-4.5, 4.5), c + Vec2::new(4.5, -4.5)], st);
    r.clicked()
}

/// Text button; `primary` fills it with the brand colour.
pub(super) fn button(ui: &mut Ui, rect: Rect, key: &str, label: &str, primary: bool, enabled: bool) -> bool {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let r = ui.interact(rect, Id::new(("lib-btn", key)), sense);
    widgets::record(ui, key, rect);
    let p = ui.painter();
    let radius = theme::cr_pill(rect.height());
    let (fill, text) = match (primary, enabled) {
        (true, true) => (
            if r.is_pointer_button_down_on() {
                theme::BG_BRAND_PRESSED
            } else if r.hovered() {
                theme::BG_BRAND_HOVER
            } else {
                theme::BG_BRAND
            },
            Color32::WHITE,
        ),
        (true, false) => (theme::BG_TERTIARY, theme::TEXT_DISABLED),
        (false, true) => (if r.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY }, theme::TEXT),
        (false, false) => (theme::BG_SECONDARY, theme::TEXT_DISABLED),
    };
    p.rect_filled(rect, radius, fill);
    p.text(rect.center(), Align2::CENTER_CENTER, label, theme::font_body_strong(), text);
    enabled && r.clicked()
}

/// 28x16 switch; returns the new state when clicked.
pub(super) fn toggle(ui: &mut Ui, rect: Rect, key: &str, on: bool) -> Option<bool> {
    let r = ui.interact(rect, Id::new(("lib-toggle", key)), Sense::click());
    widgets::record(ui, &format!("lib-toggle:{key}"), rect);
    let sw = Rect::from_center_size(rect.center(), Vec2::new(28.0, 16.0));
    let p = ui.painter();
    p.rect_filled(
        sw,
        theme::cr(8),
        if on {
            theme::BG_BRAND
        } else if r.hovered() {
            theme::BG_TERTIARY
        } else {
            theme::BG_SECONDARY
        },
    );
    let kx = if on { sw.right() - 8.0 } else { sw.left() + 8.0 };
    p.circle_filled(Pos2::new(kx, sw.center().y), 6.0, Color32::WHITE);
    r.clicked().then_some(!on)
}

fn search_box(ui: &mut Ui, rect: Rect, id: &'static str, text: &mut String, hint: &str) {
    let eid = Id::new(id);
    field::field_bg(ui, rect, eid);
    icons::paint(ui.painter(), Icon::Search, Pos2::new(rect.left() + 14.0, rect.center().y), 14.0, theme::ICON_SECONDARY);
    let te = TextEdit::singleline(text)
        .id(eid)
        .font(theme::font_body())
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .hint_text(hint)
        .margin(egui::Margin::symmetric(0, 0))
        .vertical_align(egui::Align::Center)
        .desired_width(rect.width() - 36.0);
    ui.put(Rect::from_min_max(Pos2::new(rect.left() + 28.0, rect.top()), rect.max), te);
    widgets::record(ui, id, rect);
}

fn change_tag(c: UpdateKind) -> (&'static str, Color32) {
    match c {
        UpdateKind::Added => ("New", theme::TEXT_BRAND),
        UpdateKind::Changed => ("Changed", theme::TEXT_SECONDARY),
        UpdateKind::Removed => ("Removed", theme::DANGER),
    }
}

/// Section label inside the modal.
fn label(p: &egui::Painter, x: f32, y: f32, s: &str) {
    p.text(Pos2::new(x, y), Align2::LEFT_CENTER, s, theme::font_body_strong(), theme::TEXT);
}

pub fn show(app: &mut App, ctx: &Context) {
    if !app.libs.open {
        return;
    }
    let screen = ctx.content_rect();
    let m = modal_rect(screen, 560.0, 640.0);
    let esc = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Escape));
    let mut close = frame(ctx, "libraries-modal", m, |ui| {
        let closed = header(ui, m, "Libraries", "libraries-close");
        let mut y = m.top() + HEADER_H + 16.0;
        y = this_file(app, ui, m, y);
        let p = ui.painter().clone();
        widgets::hline(&p, m.left(), m.right(), y, theme::BORDER);
        available(app, ui, m, y + 1.0);
        closed
    });
    close |= esc && !app.libs.confirm_unpublish;
    if esc && app.libs.confirm_unpublish {
        app.libs.confirm_unpublish = false;
    }
    if close {
        app.close_libraries();
    }
}

/// The "This file" section; returns the y below it.
fn this_file(app: &mut App, ui: &mut Ui, m: Rect, mut y: f32) -> f32 {
    let p = ui.painter().clone();
    let (x0, x1) = (m.left() + PAD, m.right() - PAD);
    label(&p, x0, y + 10.0, "This file");
    let published = app.doc.library.published.clone();
    let status = match &published {
        Some(info) => format!("Published \u{b7} v{}", info.version),
        None => "Not published".to_string(),
    };
    p.text(Pos2::new(x1, y + 10.0), Align2::RIGHT_CENTER, &status, theme::font_body(), theme::TEXT_TERTIARY);
    widgets::record(ui, "lib-status", Rect::from_min_max(Pos2::new(x1 - 160.0, y), Pos2::new(x1, y + 20.0)));
    y += 28.0;
    let name = if app.doc.name.is_empty() { "Untitled" } else { app.doc.name.as_str() };
    p.text(Pos2::new(x0, y + 8.0), Align2::LEFT_CENTER, name, theme::font_body(), theme::TEXT_SECONDARY);
    y += 24.0;

    let publishable = publishable_assets(&app.doc).len();
    let changes = app.libs.changes.clone();
    let summary = if publishable == 0 {
        "Create components, styles or variables to publish this file as a library.".to_string()
    } else if changes.is_empty() {
        "No changes to publish".to_string()
    } else if published.is_none() {
        format!("{} to publish", plural(changes.len(), "asset"))
    } else {
        format!("{} since the last publish", plural(changes.len(), "change"))
    };
    p.text(Pos2::new(x0, y + 8.0), Align2::LEFT_CENTER, &summary, theme::font_body(), theme::TEXT_SECONDARY);
    widgets::record(ui, "lib-summary", Rect::from_min_size(Pos2::new(x0, y), Vec2::new(x1 - x0, 16.0)));
    y += 22.0;
    for (i, c) in changes.iter().enumerate() {
        if i == MAX_CHANGES_SHOWN {
            p.text(
                Pos2::new(x0 + 8.0, y + CHANGE_ROW_H / 2.0),
                Align2::LEFT_CENTER,
                format!("and {} more", changes.len() - MAX_CHANGES_SHOWN),
                theme::font_caption(),
                theme::TEXT_TERTIARY,
            );
            y += CHANGE_ROW_H;
            break;
        }
        icons::paint(&p, kind_icon(c.kind), Pos2::new(x0 + 16.0, y + CHANGE_ROW_H / 2.0), 14.0, theme::ICON_SECONDARY);
        let s = widgets::elide(&p, &c.name, &theme::font_body(), x1 - x0 - 190.0);
        p.text(Pos2::new(x0 + 30.0, y + CHANGE_ROW_H / 2.0), Align2::LEFT_CENTER, s, theme::font_body(), theme::TEXT);
        let (tag, col) = change_tag(c.change);
        p.text(Pos2::new(x1 - 8.0, y + CHANGE_ROW_H / 2.0), Align2::RIGHT_CENTER, tag, theme::font_caption(), col);
        p.text(Pos2::new(x1 - 70.0, y + CHANGE_ROW_H / 2.0), Align2::RIGHT_CENTER, kind_label(c.kind), theme::font_caption(), theme::TEXT_TERTIARY);
        widgets::record(ui, &format!("lib-change:{}", c.name), Rect::from_min_size(Pos2::new(x0, y), Vec2::new(x1 - x0, CHANGE_ROW_H)));
        y += CHANGE_ROW_H;
    }
    y += 8.0;

    // description
    let dr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(x1 - x0, 28.0));
    let did = Id::new("lib-desc");
    field::field_bg(ui, dr, did);
    let mut desc = std::mem::take(&mut app.libs.description);
    let te = TextEdit::singleline(&mut desc)
        .id(did)
        .font(theme::font_body())
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .hint_text("Describe what changed (optional)")
        .margin(egui::Margin::symmetric(8, 0))
        .vertical_align(egui::Align::Center)
        .desired_width(dr.width());
    ui.put(dr, te);
    app.libs.description = desc;
    widgets::record(ui, "lib-desc", dr);
    y += 36.0;

    // actions
    let can_publish = publishable > 0 && (published.is_none() || !changes.is_empty());
    let label_pub = if published.is_some() { "Publish changes" } else { "Publish" };
    let bw = if published.is_some() { 120.0 } else { 80.0 };
    let pb = Rect::from_min_size(Pos2::new(x1 - bw, y), Vec2::new(bw, 28.0));
    if button(ui, pb, "lib-publish", label_pub, true, can_publish) {
        app.publish_file();
    }
    if published.is_some() {
        if app.libs.confirm_unpublish {
            let cb = Rect::from_min_size(Pos2::new(pb.left() - 8.0 - 64.0, y), Vec2::new(64.0, 28.0));
            let ub = Rect::from_min_size(Pos2::new(cb.left() - 8.0 - 100.0, y), Vec2::new(100.0, 28.0));
            p.text(Pos2::new(x0, y + 14.0), Align2::LEFT_CENTER, "Unpublish this library?", theme::font_body(), theme::TEXT_SECONDARY);
            if button(ui, ub, "lib-unpublish-confirm", "Unpublish", false, true) {
                app.libs.confirm_unpublish = false;
                app.unpublish_file();
            }
            if button(ui, cb, "lib-unpublish-cancel", "Cancel", false, true) {
                app.libs.confirm_unpublish = false;
            }
        } else {
            let ub = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(88.0, 28.0));
            if button(ui, ub, "lib-unpublish", "Unpublish", false, true) {
                app.libs.confirm_unpublish = true;
            }
        }
    }
    y + 28.0 + 16.0
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

fn counts(i: &designos_library::LibraryIndex) -> String {
    let mut parts = vec![format!("v{}", i.latest_version())];
    for (n, w) in [(i.components, "component"), (i.styles, "style"), (i.variables, "variable")] {
        if n > 0 {
            parts.push(plural(n, w));
        }
    }
    parts.join(" \u{b7} ")
}

/// The list of libraries available to enable.
fn available(app: &mut App, ui: &mut Ui, m: Rect, top: f32) {
    let p = ui.painter().clone();
    let (x0, x1) = (m.left() + PAD, m.right() - PAD);
    let y = top + 14.0;
    label(&p, x0, y + 12.0, "Libraries");
    let sr = Rect::from_min_size(Pos2::new(x1 - 220.0, y), Vec2::new(220.0, 24.0));
    let mut q = std::mem::take(&mut app.libs.search);
    search_box(ui, sr, "lib-search", &mut q, "Search libraries");
    app.libs.search = q;

    let own = app.doc.library.published.as_ref().map(|p| p.library_id.clone());
    let query = app.libs.search.trim().to_lowercase();
    let mut rows: Vec<(String, String, String, bool)> = app
        .libs
        .available
        .iter()
        .filter(|i| Some(&i.id) != own.as_ref())
        .filter(|i| query.is_empty() || i.name.to_lowercase().contains(&query))
        .map(|i| (i.id.clone(), i.name.clone(), counts(i), true))
        .collect();
    // Enabled libraries that are no longer published.
    for r in &app.doc.library.enabled {
        if !rows.iter().any(|x| x.0 == r.id)
            && !r.id.starts_with(REMOTE_LIBRARY_PREFIX)
            && (query.is_empty() || r.name.to_lowercase().contains(&query))
            && Some(&r.id) != own.as_ref()
        {
            rows.push((r.id.clone(), r.name.clone(), "Not available".into(), false));
        }
    }
    let list = Rect::from_min_max(Pos2::new(m.left(), y + 36.0), m.max);
    let content_h = rows.len() as f32 * ROW_H + 8.0;
    let mut scroll = app.libs.scroll;
    wheel_scroll(ui, list, &mut scroll, content_h);
    app.libs.scroll = scroll;
    if rows.is_empty() {
        let (a, b) = if app.libs.available.is_empty() {
            ("No libraries yet", "Publish a file to share its components, styles and variables.")
        } else {
            ("No results", "Try a different search.")
        };
        p.text(Pos2::new(list.center().x, list.top() + 40.0), Align2::CENTER_CENTER, a, theme::font_body(), theme::TEXT_TERTIARY);
        p.text(Pos2::new(list.center().x, list.top() + 60.0), Align2::CENTER_CENTER, b, theme::font_caption(), theme::TEXT_TERTIARY);
    }
    let pending = app.pending_libraries();
    let mut toggled: Option<(String, bool)> = None;
    let mut review = false;
    clipped(ui, list, |ui| {
        let p = ui.painter().clone();
        for (i, (id, name, sub, ok)) in rows.iter().enumerate() {
            let row = Rect::from_min_size(Pos2::new(list.left(), list.top() - scroll + i as f32 * ROW_H), Vec2::new(list.width(), ROW_H));
            if row.bottom() < list.top() || row.top() > list.bottom() {
                continue;
            }
            let enabled = app.doc.library.enabled.iter().any(|l| &l.id == id);
            let rr = ui.interact(row, Id::new(("lib-row", id)), Sense::hover());
            widgets::record(ui, &format!("lib-row:{name}"), row);
            if rr.hovered() {
                p.rect_filled(row.shrink2(Vec2::new(8.0, 2.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
            }
            let icon_box = Rect::from_center_size(Pos2::new(x0 + 20.0, row.center().y), Vec2::splat(36.0));
            p.rect_filled(icon_box, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
            icons::paint(&p, Icon::Book, icon_box.center(), 18.0, if enabled { theme::ICON_SELECTED } else { theme::ICON_SECONDARY });
            let tx = icon_box.right() + 12.0;
            let s = widgets::elide(&p, name, &theme::font_body_strong(), x1 - tx - 150.0);
            p.text(Pos2::new(tx, row.center().y - 9.0), Align2::LEFT_CENTER, s, theme::font_body_strong(), theme::TEXT);
            p.text(Pos2::new(tx, row.center().y + 9.0), Align2::LEFT_CENTER, sub, theme::font_caption(), theme::TEXT_TERTIARY);
            let mut right = x1;
            if *ok || enabled {
                let tr = Rect::from_center_size(Pos2::new(right - 14.0, row.center().y), Vec2::new(32.0, 24.0));
                if let Some(on) = toggle(ui, tr, name, enabled) {
                    toggled = Some((id.clone(), on));
                }
                right = tr.left() - 12.0;
            }
            if enabled && pending.iter().any(|pl| &pl.library_id == id) {
                let w = 116.0;
                let ur = Rect::from_center_size(Pos2::new(right - w / 2.0, row.center().y), Vec2::new(w, 22.0));
                if button(ui, ur, &format!("lib-update:{name}"), "Update available", false, true) {
                    review = true;
                }
            }
        }
    });
    scrollbar(ui, list, scroll, content_h);
    if let Some((id, on)) = toggled {
        app.set_library_enabled(&id, on);
    }
    if review {
        app.close_libraries();
        app.open_updates_review();
    }
}

/// Read-only preview of a library component copy (when its file cannot be opened).
pub(super) fn show_preview(app: &mut App, ctx: &Context) {
    let Some(comp) = app.libs.preview else { return };
    let Some(node) = app.doc.get(comp).cloned() else {
        app.libs.preview = None;
        return;
    };
    let info = library_asset_of_component(&app.doc, comp).map(|(_, e)| app.library_display_name(&e.library_id));
    let screen = ctx.content_rect();
    let m = modal_rect(screen, 420.0, 380.0);
    let esc = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Escape));
    if app.libs.preview_tex.as_ref().is_none_or(|(id, _)| *id != comp) {
        let scale = (240.0 / node.size.x.max(1.0)).min(160.0 / node.size.y.max(1.0)).clamp(0.05, 2.0);
        app.libs.preview_tex = designos_render::render_node(&app.doc, comp, scale).map(|pm| {
            let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
            (comp, ctx.load_texture("lib-preview", img, egui::TextureOptions::LINEAR))
        });
    }
    let tex = app.libs.preview_tex.as_ref().map(|(_, t)| t.clone());
    let mut close = frame(ctx, "library-preview", m, |ui| {
        let mut closed = header(ui, m, &node.name, "library-preview-close");
        let p = ui.painter().clone();
        let area = Rect::from_min_size(Pos2::new(m.left() + PAD, m.top() + HEADER_H + 16.0), Vec2::new(m.width() - 2.0 * PAD, 190.0));
        p.rect_filled(area, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        if let Some(t) = &tex {
            let sz = t.size_vec2() / ui.ctx().pixels_per_point();
            let sz = sz * (area.width() / sz.x).min(area.height() / sz.y).min(1.0);
            p.image(t.id(), Rect::from_center_size(area.center(), sz), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        let y = area.bottom() + 20.0;
        let from = info.clone().unwrap_or_else(|| "a library".into());
        p.text(Pos2::new(area.left(), y), Align2::LEFT_CENTER, format!("Read-only copy from {from}"), theme::font_body_strong(), theme::TEXT);
        p.text(
            Pos2::new(area.left(), y + 20.0),
            Align2::LEFT_CENTER,
            "The file that defines this component is not available on this device.",
            theme::font_caption(),
            theme::TEXT_SECONDARY,
        );
        let bw = 80.0;
        let b = Rect::from_min_size(Pos2::new(m.right() - PAD - bw, m.bottom() - 28.0 - 16.0), Vec2::new(bw, 28.0));
        closed |= button(ui, b, "library-preview-done", "Close", false, true);
        closed
    });
    close |= esc;
    if close {
        app.libs.preview = None;
    }
    if app.libs.preview.is_none() {
        app.libs.preview_tex = None;
    }
}
