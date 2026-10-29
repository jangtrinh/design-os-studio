//! The Libraries modal: publish this file, and enable or disable the libraries
//! other files published. Also the read-only preview of a library component
//! whose file is not on this device.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::variables::widgets::{SWITCH_H, SWITCH_W};
use crate::widgets;
use designos_doc::*;
use egui::{Align2, Color32, Context, Id, Key, Order, Pos2, Rect, Sense, TextEdit, Ui, Vec2};

const HEADER_H: f32 = 48.0;
const PAD: f32 = theme::INSET_MODAL;
const ROW_H: f32 = 60.0;
const CHANGE_PITCH: f32 = theme::BADGE_MD;
const MAX_CHANGES_SHOWN: usize = 4;
/// The "and N more" caption row is text, not a chip, so it keeps the old row height.
const MORE_ROW_H: f32 = 20.0;

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
            crate::widgets::kit::modal_surface(ui.painter(), screen, m);
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
    let side = theme::CONTROL_SM;
    let cb = Rect::from_center_size(Pos2::new(m.right() - PAD - side / 2.0, m.top() + HEADER_H / 2.0), Vec2::splat(side));
    let r = widgets::icon_button(ui, cb, close_key, Icon::Close, false, "");
    widgets::record(ui, close_key, cb);
    r.clicked()
}

/// Text button; `primary` fills it with the brand colour.
pub(super) fn button(ui: &mut Ui, rect: Rect, key: &str, label: &str, primary: bool, enabled: bool) -> bool {
    let size = if rect.height() + 0.5 >= theme::CONTROL_MD {
        crate::widgets::kit::Size::Md
    } else if rect.height() + 0.5 >= theme::CONTROL_SM {
        crate::widgets::kit::Size::Sm
    } else {
        crate::widgets::kit::Size::Xs
    };
    let kind = if primary { crate::widgets::kit::Hierarchy::Primary } else { crate::widgets::kit::Hierarchy::Secondary };
    let response = crate::widgets::kit::button(ui, rect, ("lib-btn", key), label, kind, size, enabled);
    widgets::record(ui, key, rect);
    enabled && response.clicked()
}

/// Kit switch on `track` (36x20); returns the new state when clicked.
pub(super) fn toggle(ui: &mut Ui, track: Rect, key: &str, on: bool) -> Option<bool> {
    let response = crate::widgets::kit::toggle(ui, track, ("lib-toggle", key), on, true);
    // Record the hit band the kit senses, not the painted track, so the layout snapshot names it.
    widgets::record(ui, &format!("lib-toggle:{key}"), response.rect);
    response.clicked().then_some(!on)
}

fn search_box(ui: &mut Ui, rect: Rect, id: &'static str, text: &mut String, hint: &str) {
    let eid = Id::new(id);
    crate::variables::widgets::field_chrome(ui, rect, eid, crate::widgets::kit::Size::Sm);
    let icon = crate::widgets::kit::field_look(crate::widgets::kit::State::Default, false).icon;
    icons::paint(ui.painter(), Icon::Search, Pos2::new(rect.left() + 14.0, rect.center().y), 14.0, icon);
    let te = TextEdit::singleline(text)
        .id(eid)
        .font(theme::font_body())
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .hint_text(egui::RichText::new(hint).color(theme::TEXT_PLACEHOLDER))
        .margin(egui::Margin::symmetric(0, 0))
        .vertical_align(egui::Align::Center)
        .desired_width(rect.width() - 36.0);
    ui.put(Rect::from_min_max(Pos2::new(rect.left() + 28.0, rect.top()), rect.max), te);
    widgets::record(ui, id, rect);
}

/// Status tone of a library change; the Libraries dialog and the update review show the same chips.
pub(super) fn change_tone(c: UpdateKind) -> crate::widgets::kit::Tone {
    match c {
        UpdateKind::Added => crate::widgets::kit::Tone::Success,
        UpdateKind::Changed => crate::widgets::kit::Tone::Warning,
        UpdateKind::Removed => crate::widgets::kit::Tone::Error,
    }
}

fn change_tag(c: UpdateKind) -> &'static str {
    match c {
        UpdateKind::Added => "New",
        UpdateKind::Changed => "Changed",
        UpdateKind::Removed => "Removed",
    }
}

/// Paints `label` as a status chip whose right edge is `right`, centred on `cy`. Returns the chip's left edge.
pub(super) fn paint_change_chip(p: &egui::Painter, right: f32, cy: f32, label: &str, tone: crate::widgets::kit::Tone) -> f32 {
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
    let width = widgets::text_width(p, label, &font) + theme::SPACE_INLINE * 2.0;
    let chip = Rect::from_center_size(Pos2::new(right - width / 2.0, cy), Vec2::new(width, theme::BADGE_MD));
    crate::widgets::kit::status_tag(p, chip, label, tone);
    chip.left()
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
    y += theme::SPACE_BLOCK;
    for (i, c) in changes.iter().enumerate() {
        if i == MAX_CHANGES_SHOWN {
            p.text(
                Pos2::new(x0 + 8.0, y + MORE_ROW_H / 2.0),
                Align2::LEFT_CENTER,
                format!("and {} more", changes.len() - MAX_CHANGES_SHOWN),
                theme::font_caption(),
                theme::TEXT_TERTIARY,
            );
            y += MORE_ROW_H;
            break;
        }
        let cy = y + CHANGE_PITCH / 2.0;
        let row = Rect::from_center_size(Pos2::new((x0 + x1) / 2.0, cy), Vec2::new(x1 - x0, theme::BADGE_MD));
        icons::paint(&p, kind_icon(c.kind), Pos2::new(x0 + 16.0, cy), 14.0, theme::ICON_SECONDARY);
        let s = widgets::elide(&p, &c.name, &theme::font_body(), x1 - x0 - 190.0);
        p.text(Pos2::new(x0 + 30.0, cy), Align2::LEFT_CENTER, s, theme::font_body(), theme::TEXT);
        let chip_left = paint_change_chip(&p, x1 - 8.0, cy, change_tag(c.change), change_tone(c.change));
        p.text(Pos2::new(chip_left - 8.0, cy), Align2::RIGHT_CENTER, kind_label(c.kind), theme::font_caption(), theme::TEXT_TERTIARY);
        widgets::record(ui, &format!("lib-change:{}", c.name), row);
        y += CHANGE_PITCH;
    }
    y += theme::SPACE_INLINE;

    // description
    let dr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(x1 - x0, theme::CONTROL_SM));
    let did = Id::new("lib-desc");
    crate::variables::widgets::field_chrome(ui, dr, did, crate::widgets::kit::Size::Sm);
    let mut desc = std::mem::take(&mut app.libs.description);
    let te = TextEdit::singleline(&mut desc)
        .id(did)
        .font(theme::font_body())
        .text_color(theme::TEXT)
        // egui uses `margin` only when it builds the frame itself, so the inset lives on the given frame.
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(theme::INSET_CONTROL as i8, 0)))
        .hint_text(egui::RichText::new("Describe what changed (optional)").color(theme::TEXT_PLACEHOLDER))
        .vertical_align(egui::Align::Center)
        .desired_width(dr.width());
    ui.put(dr, te);
    app.libs.description = desc;
    widgets::record(ui, "lib-desc", dr);
    y += theme::CONTROL_SM + theme::SPACE_INLINE;

    // actions
    let can_publish = publishable > 0 && (published.is_none() || !changes.is_empty());
    let label_pub = if published.is_some() { "Publish changes" } else { "Publish" };
    let bh = theme::CONTROL_SM;
    let action_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let bw = (widgets::text_width(&p, label_pub, &action_font) + theme::INSET_BUTTON * 2.0).max(if published.is_some() { 120.0 } else { 80.0 });
    let pb = Rect::from_min_size(Pos2::new(x1 - bw, y), Vec2::new(bw, bh));
    if button(ui, pb, "lib-publish", label_pub, true, can_publish) {
        app.publish_file();
    }
    if published.is_some() {
        if app.libs.confirm_unpublish {
            let cancel_w = widgets::text_width(&p, "Cancel", &action_font) + theme::INSET_BUTTON * 2.0;
            let unpublish_w = widgets::text_width(&p, "Unpublish", &action_font) + theme::INSET_BUTTON * 2.0;
            let cb = Rect::from_min_size(Pos2::new(pb.left() - 8.0 - cancel_w, y), Vec2::new(cancel_w, bh));
            let ub = Rect::from_min_size(Pos2::new(cb.left() - 8.0 - unpublish_w, y), Vec2::new(unpublish_w, bh));
            p.text(Pos2::new(x0, y + bh / 2.0), Align2::LEFT_CENTER, "Unpublish this library?", theme::font_body(), theme::TEXT_SECONDARY);
            if button(ui, ub, "lib-unpublish-confirm", "Unpublish", false, true) {
                app.libs.confirm_unpublish = false;
                app.unpublish_file();
            }
            if button(ui, cb, "lib-unpublish-cancel", "Cancel", false, true) {
                app.libs.confirm_unpublish = false;
            }
        } else {
            let unpublish_w = (widgets::text_width(&p, "Unpublish", &action_font) + theme::INSET_BUTTON * 2.0).max(88.0);
            let ub = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(unpublish_w, bh));
            if button(ui, ub, "lib-unpublish", "Unpublish", false, true) {
                app.libs.confirm_unpublish = true;
            }
        }
    }
    y + bh + theme::SPACE_BLOCK
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
    let sr = Rect::from_min_size(Pos2::new(x1 - 220.0, y), Vec2::new(220.0, theme::CONTROL_SM));
    label(&p, x0, sr.center().y, "Libraries");
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
    let list = Rect::from_min_max(Pos2::new(m.left(), sr.bottom() + theme::SPACE_INLINE), m.max);
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
            // The controls on the right are laid out first, so the name and the counts stop short of them.
            let mut right = x1;
            let mut controls_left = x1;
            if *ok || enabled {
                let tr = Rect::from_min_size(Pos2::new(right - SWITCH_W, row.center().y - SWITCH_H / 2.0), Vec2::new(SWITCH_W, SWITCH_H));
                if let Some(on) = toggle(ui, tr, name, enabled) {
                    toggled = Some((id.clone(), on));
                }
                right = tr.left() - 12.0;
                controls_left = tr.left();
            }
            if enabled && pending.iter().any(|pl| &pl.library_id == id) {
                let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
                let w = widgets::text_width(&p, "Update available", &font) + theme::INSET_BUTTON * 2.0;
                let ur = Rect::from_center_size(Pos2::new(right - w / 2.0, row.center().y), Vec2::new(w, theme::CONTROL_SM));
                controls_left = ur.left();
                if button(ui, ur, &format!("lib-update:{name}"), "Update available", false, true) {
                    review = true;
                }
            }
            let text_w = (controls_left - theme::SPACE_INLINE - tx).max(theme::SPACE_INLINE);
            let s = widgets::elide(&p, name, &theme::font_body_strong(), text_w);
            p.text(Pos2::new(tx, row.center().y - 9.0), Align2::LEFT_CENTER, s, theme::font_body_strong(), theme::TEXT);
            let sub = widgets::elide(&p, sub, &theme::font_caption(), text_w);
            p.text(Pos2::new(tx, row.center().y + 9.0), Align2::LEFT_CENTER, sub, theme::font_caption(), theme::TEXT_TERTIARY);
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
        let close_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
        let bw = (widgets::text_width(&p, "Close", &close_font) + theme::INSET_BUTTON * 2.0).max(80.0);
        let bh = theme::CONTROL_MD;
        let b = Rect::from_min_size(Pos2::new(m.right() - PAD - bw, m.bottom() - bh - 16.0), Vec2::new(bw, bh));
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
