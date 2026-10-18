//! Small painted pieces shared by the Variables modal and the inspector:
//! variable glyphs, the "bound variable" pill and the searchable picker popup.

use crate::theme;
use crate::widgets;
use designos_doc::{Color, Document, Variable, VariableKind, VariableValue};
use egui::{Align2, Color32, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

pub fn to_c32(c: Color) -> Color32 {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(f(c.r), f(c.g), f(c.b), f(c.a))
}

pub fn from_c32(c: Color32) -> Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color::rgba(r, g, b, a)
}

/// Hexagon outline, the glyph of a bound variable.
pub fn hexagon(p: &egui::Painter, c: Pos2, r: f32, col: Color32, filled: bool) {
    let pts: Vec<Pos2> = (0..6)
        .map(|i| {
            let a = std::f32::consts::FRAC_PI_3 * i as f32 - std::f32::consts::FRAC_PI_2;
            c + Vec2::new(a.cos(), a.sin()) * r
        })
        .collect();
    if filled {
        p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    } else {
        p.add(egui::Shape::closed_line(pts, Stroke::new(1.2, col)));
    }
}

/// Glyph for a variable type: palette, "#", "Aa" or a toggle (16px box centred on `c`).
pub fn kind_glyph(p: &egui::Painter, c: Pos2, kind: VariableKind, col: Color32) {
    match kind {
        VariableKind::Color => {
            let st = Stroke::new(1.2, col);
            p.circle_stroke(c, 6.2, st);
            for (dx, dy) in [(-2.4, -2.2), (0.6, -3.0), (2.8, -0.6), (-3.2, 1.0)] {
                p.circle_filled(c + Vec2::new(dx, dy), 0.9, col);
            }
            p.circle_filled(c + Vec2::new(2.2, 3.2), 1.9, theme::BG);
            p.circle_stroke(c + Vec2::new(2.2, 3.2), 1.9, st);
        }
        VariableKind::Number => {
            p.text(c, Align2::CENTER_CENTER, "#", theme::font_large(), col);
        }
        VariableKind::String => {
            p.text(c, Align2::CENTER_CENTER, "Aa", theme::font_body_strong(), col);
        }
        VariableKind::Boolean => {
            let r = Rect::from_center_size(c, Vec2::new(13.0, 8.0));
            p.rect_stroke(r, theme::cr(255), Stroke::new(1.2, col), StrokeKind::Inside);
            p.circle_filled(Pos2::new(r.right() - 4.0, c.y), 2.0, col);
        }
    }
}

/// Two slider tracks with knobs (collection settings).
pub fn sliders_glyph(p: &egui::Painter, c: Pos2, col: Color32) {
    let st = Stroke::new(1.2, col);
    for (dx, ky) in [(-3.5, 2.0), (3.5, -2.0)] {
        let x = c.x + dx;
        p.line_segment([Pos2::new(x, c.y - 6.0), Pos2::new(x, c.y + 6.0)], st);
        p.circle_filled(Pos2::new(x, c.y + ky), 2.3, theme::BG);
        p.circle_stroke(Pos2::new(x, c.y + ky), 2.3, st);
    }
}

/// Circle with three shrinking lines (filter).
pub fn filter_glyph(p: &egui::Painter, c: Pos2, col: Color32) {
    let st = Stroke::new(1.1, col);
    p.circle_stroke(c, 7.2, st);
    for (dy, hw) in [(-2.4, 3.6), (0.0, 2.4), (2.4, 1.0)] {
        p.line_segment([Pos2::new(c.x - hw, c.y + dy), Pos2::new(c.x + hw, c.y + dy)], st);
    }
}

/// Diagonal arrow pair: pointing inward (collapse) or outward (expand).
pub fn arrows_glyph(p: &egui::Painter, c: Pos2, col: Color32, inward: bool) {
    let st = Stroke::new(1.2, col);
    let sign = if inward { 1.0 } else { -1.0 };
    for s in [-1.0f32, 1.0] {
        // the arrow sits in the top-right (s = 1) / bottom-left (s = -1) corner
        let dir = Vec2::new(s, -s);
        let (tip, tail) = if inward { (1.5, 5.5) } else { (5.5, 1.5) };
        let t = c + dir * tip;
        p.line_segment([c + dir * tail, t], st);
        p.line_segment([t, t + Vec2::new(sign * s * 3.5, 0.0)], st);
        p.line_segment([t, t + Vec2::new(0.0, -sign * s * 3.5)], st);
    }
}

pub fn cross_glyph(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    let st = Stroke::new(1.3, col);
    p.line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], st);
    p.line_segment([c + Vec2::new(-r, r), c + Vec2::new(r, -r)], st);
}

pub fn kind_name(kind: VariableKind) -> &'static str {
    match kind {
        VariableKind::Color => "Color",
        VariableKind::Number => "Number",
        VariableKind::String => "String",
        VariableKind::Boolean => "Boolean",
    }
}

pub struct PillResponse {
    pub body: egui::Response,
    pub detach: bool,
}

/// A bound-variable chip: `bg_secondary` rounded box, hexagon icon, optional
/// colour swatch and the variable name. Shows a detach "x" on hover.
pub fn pill(ui: &mut Ui, rect: Rect, id: impl crate::widgets::Key, name: &str, swatch: Option<Color32>, detachable: bool) -> PillResponse {
    let id = ui.id().with(("pill", id));
    let body = ui.interact(rect, id, Sense::click());
    let hovered = body.hovered();
    let p = ui.painter();
    let fill = if hovered { theme::BG_TERTIARY } else { theme::BG_SECONDARY };
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), fill);
    let mut x = rect.left() + 12.0;
    hexagon(p, Pos2::new(x, rect.center().y), 5.0, theme::ICON_SECONDARY, false);
    x += 12.0;
    if let Some(c) = swatch {
        let sw = Rect::from_center_size(Pos2::new(x + 6.0, rect.center().y), Vec2::splat(12.0));
        widgets::swatch(p, sw, c);
        x += 18.0;
    }
    let right_pad = if detachable { 26.0 } else { 8.0 };
    let name = widgets::elide(p, name, &theme::font_body(), (rect.right() - right_pad - x).max(8.0));
    p.text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, name, theme::font_body(), theme::TEXT);
    let mut detach = false;
    if detachable && (hovered || ui.rect_contains_pointer(rect)) {
        let b = Rect::from_center_size(Pos2::new(rect.right() - 13.0, rect.center().y), Vec2::splat(20.0));
        let r = ui.interact(b, id.with("x"), Sense::click());
        let p = ui.painter();
        if r.hovered() {
            p.rect_filled(b, theme::cr(theme::RADIUS_CONTROL), theme::BG_PRESSED);
        }
        let c = b.center();
        let st = Stroke::new(1.2, theme::ICON);
        p.line_segment([c + Vec2::new(-3.0, -3.0), c + Vec2::new(3.0, 3.0)], st);
        p.line_segment([c + Vec2::new(-3.0, 3.0), c + Vec2::new(3.0, -3.0)], st);
        detach = r.clicked();
    }
    PillResponse { body, detach }
}

/// One selectable variable in a picker.
#[derive(Clone, Debug)]
pub struct PickItem {
    pub id: String,
    pub group: String,
    pub name: String,
    pub kind: VariableKind,
    pub preview: Option<Color32>,
}

/// Candidate list entry for `v`, with its colour resolved in the default mode.
pub fn pick_item(doc: &Document, v: &Variable) -> PickItem {
    let coll = doc.variable_collections.get(&v.collection);
    let mode = coll.map(|c| c.default_mode.clone()).unwrap_or_default();
    let preview = match v.kind {
        VariableKind::Color => {
            let store = designos_doc::VariableStore::from_doc(doc);
            match store.resolve(&v.id, &mode) {
                Some(VariableValue::Color(c)) => Some(to_c32(c)),
                _ => None,
            }
        }
        _ => None,
    };
    PickItem { id: v.id.clone(), group: coll.map(|c| c.name.clone()).unwrap_or_default(), name: v.name.clone(), kind: v.kind, preview }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickResult {
    Open,
    Picked(String),
    Detach,
    Closed,
}

const PICK_W: f32 = 248.0;
const PICK_MAX_H: f32 = 320.0;
const PICK_ROW: f32 = 24.0;
const PICK_GROUP: f32 = 26.0;
const PICK_HEAD: f32 = 40.0;

/// Searchable variable picker floating at `pos`. `query` is the live search
/// text; `current` is highlighted; `detach` adds a "Detach variable" row.
#[allow(clippy::too_many_arguments)]
pub fn picker(
    ctx: &egui::Context,
    id: Id,
    pos: Pos2,
    anchor: Option<Rect>,
    title: &str,
    query: &mut String,
    items: &[PickItem],
    current: Option<&str>,
    detach: bool,
) -> PickResult {
    let q = query.to_lowercase();
    let shown: Vec<&PickItem> = items.iter().filter(|i| q.is_empty() || i.name.to_lowercase().contains(&q) || i.group.to_lowercase().contains(&q)).collect();
    // rows: (group header | item)
    enum Row<'a> {
        Group(&'a str),
        Item(&'a PickItem),
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut last: Option<&str> = None;
    let mut groups: Vec<&str> = Vec::new();
    for i in &shown {
        if !groups.contains(&i.group.as_str()) {
            groups.push(i.group.as_str());
        }
    }
    for g in &groups {
        for i in shown.iter().filter(|i| i.group == *g) {
            if last != Some(*g) {
                rows.push(Row::Group(g));
                last = Some(*g);
            }
            rows.push(Row::Item(i));
        }
    }
    let content_h: f32 =
        rows.iter().map(|r| if matches!(r, Row::Group(_)) { PICK_GROUP } else { PICK_ROW }).sum::<f32>() + if detach { PICK_ROW + 9.0 } else { 0.0 };
    let empty_h = if rows.is_empty() { 40.0 } else { 0.0 };
    let body_h = (content_h + empty_h + 8.0).min(PICK_MAX_H);
    let size = Vec2::new(PICK_W, PICK_HEAD + body_h);
    let screen = ctx.content_rect();
    let mut tl = pos;
    tl.x = tl.x.min(screen.right() - size.x - 8.0).max(screen.left() + 8.0);
    tl.y = tl.y.min(screen.bottom() - size.y - 8.0).max(screen.top() + 8.0);
    let rect = Rect::from_min_size(tl, size);
    let scroll_id = id.with("scroll");
    let mut scroll: f32 = ctx.data(|d| d.get_temp(scroll_id)).unwrap_or(0.0);
    let mut result = PickResult::Open;
    let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));

    egui::Area::new(id).order(Order::Foreground).fixed_pos(tl).constrain(false).show(ctx, |ui| {
        let (full, _) = ui.allocate_exact_size(size, Sense::hover());
        let p = ui.painter().clone();
        widgets::menu_panel(&p, full, theme::RADIUS_FRAME);
        let _ = title;
        let sr = Rect::from_min_size(Pos2::new(full.left() + 8.0, full.top() + 8.0), Vec2::new(full.width() - 16.0, 24.0));
        p.rect_filled(sr, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        crate::icons::paint(&p, crate::icons::Icon::Search, Pos2::new(sr.left() + 14.0, sr.center().y), 12.0, theme::ICON_TERTIARY);
        let sr = Rect::from_min_max(sr.min + Vec2::new(22.0, 0.0), sr.max);
        let te = egui::TextEdit::singleline(query)
            .id(id.with("q"))
            .hint_text("Search")
            .font(theme::font_body())
            .text_color(theme::TEXT)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::symmetric(4, 0))
            .vertical_align(egui::Align::Center)
            .desired_width(sr.width());
        let out = ui.put(sr, te);
        if !ctx.data(|d| d.get_temp::<bool>(id.with("focused")).unwrap_or(false)) {
            out.request_focus();
            ctx.data_mut(|d| d.insert_temp(id.with("focused"), true));
        }
        widgets::hline(&p, full.left(), full.right(), full.top() + PICK_HEAD - 1.0, theme::BORDER_MENU);
        // list
        let list = Rect::from_min_max(Pos2::new(full.left(), full.top() + PICK_HEAD), full.max - Vec2::new(0.0, 4.0));
        let max_scroll = (content_h + empty_h - list.height() + 8.0).max(0.0);
        if ui.rect_contains_pointer(list) {
            scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        scroll = scroll.clamp(0.0, max_scroll);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(list));
            let p = ui.painter().clone();
            let mut y = list.top() + 4.0 - scroll;
            if detach {
                let r = Rect::from_min_size(Pos2::new(list.left() + 8.0, y), Vec2::new(list.width() - 16.0, PICK_ROW));
                let resp = ui.interact(r, id.with("detach"), Sense::click());
                if resp.hovered() {
                    p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_MENU_SELECTED);
                }
                p.text(Pos2::new(r.left() + 8.0, r.center().y), Align2::LEFT_CENTER, "Detach variable", theme::font_menu(), theme::TEXT);
                if resp.clicked() {
                    result = PickResult::Detach;
                }
                y += PICK_ROW;
                widgets::hline(&p, list.left() + 8.0, list.right() - 8.0, y + 4.0, theme::BORDER_MENU);
                y += 9.0;
            }
            if rows.is_empty() {
                p.text(Pos2::new(list.center().x, y + 16.0), Align2::CENTER_CENTER, "No matching variables", theme::font_body(), theme::TEXT_TERTIARY);
            }
            for row in &rows {
                match row {
                    Row::Group(g) => {
                        p.text(
                            Pos2::new(list.left() + 14.0, y + PICK_GROUP / 2.0 + 2.0),
                            Align2::LEFT_CENTER,
                            g,
                            theme::font_caption_strong(),
                            theme::TEXT_TERTIARY,
                        );
                        y += PICK_GROUP;
                    }
                    Row::Item(it) => {
                        let r = Rect::from_min_size(Pos2::new(list.left() + 8.0, y), Vec2::new(list.width() - 16.0, PICK_ROW));
                        y += PICK_ROW;
                        if r.bottom() < list.top() || r.top() > list.bottom() {
                            continue;
                        }
                        widgets::record(ui, &format!("pick:{}", it.name), r);
                        let resp = ui.interact(r, id.with(("item", &it.id)), Sense::click());
                        let cur = current == Some(it.id.as_str());
                        if resp.hovered() {
                            p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_MENU_SELECTED);
                        } else if cur {
                            p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
                        }
                        let c = Pos2::new(r.left() + 16.0, r.center().y);
                        match it.preview {
                            Some(col) => widgets::swatch_circle(&p, c, 6.0, &[col]),
                            None => kind_glyph(&p, c, it.kind, theme::ICON_SECONDARY),
                        }
                        let nm = widgets::elide(&p, &it.name, &theme::font_menu(), r.width() - 40.0);
                        p.text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, nm, theme::font_menu(), theme::TEXT);
                        if resp.clicked() {
                            result = PickResult::Picked(it.id.clone());
                        }
                    }
                }
            }
        });
    });
    ctx.data_mut(|d| d.insert_temp(scroll_id, scroll));

    if result == PickResult::Open {
        let (pressed, ptr) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
        let outside = pressed && ptr.is_some_and(|p| !rect.contains(p) && !anchor.is_some_and(|a| a.contains(p)));
        if esc || outside {
            result = PickResult::Closed;
        }
    }
    if result != PickResult::Open {
        ctx.data_mut(|d| {
            d.remove_temp::<bool>(id.with("focused"));
            d.remove_temp::<f32>(scroll_id);
        });
    }
    result
}
