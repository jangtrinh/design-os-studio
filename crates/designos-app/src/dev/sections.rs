//! Sections of the Inspect panel added for Dev mode: status, component properties, annotations,
//! links, box model, code, variables used and assets.

use super::data::{self, BoxModel};
use super::highlight::{self, Span};
use super::{AnnotEdit, Language, LinkEdit};
use crate::app::App;
use crate::panels::inspect::{copy, download, section_title, small_button, Asset, Cur, PAD, ROW_H};
use crate::theme;
use crate::widgets::field;
use crate::widgets::menu::dropdown;
use crate::widgets::{self, elide, fmt_num, text_width};
use designos_doc::*;
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

pub fn status_color(s: DevStatus) -> Color32 {
    match s {
        DevStatus::ReadyForDev => theme::DEV_READY,
        DevStatus::Completed => theme::DEV_COMPLETED,
        DevStatus::Changed => theme::DEV_CHANGED,
    }
}

/// A rounded status pill with a dot; returns its width.
pub fn status_pill(p: &egui::Painter, at: Pos2, s: DevStatus) -> f32 {
    let font = theme::font_caption_medium();
    let label = s.label();
    let w = text_width(p, label, &font) + 26.0;
    let r = Rect::from_min_size(at, Vec2::new(w, 20.0));
    p.rect_filled(r, theme::cr_pill(r.height()), status_color(s).gamma_multiply(0.22));
    p.circle_filled(Pos2::new(r.left() + 10.0, r.center().y), 3.5, status_color(s));
    p.text(Pos2::new(r.left() + 19.0, r.center().y), Align2::LEFT_CENTER, label, font, theme::TEXT);
    w
}

fn primary_button(ui: &mut Ui, rect: Rect, key: &str, label: &str) -> bool {
    widgets::record(ui, &format!("dev-{key}"), rect);
    let r = ui.interact(rect, ui.id().with(("dev-primary", key)), Sense::click());
    ui.painter().rect_filled(rect, theme::cr_pill(rect.height()), if r.hovered() { theme::BG_BRAND_HOVER } else { theme::BG_BRAND });
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, theme::font_body_strong(), theme::TEXT);
    r.clicked()
}

/// Status pill and the Ready for dev actions of the selected layer.
pub fn status_header(app: &mut App, ui: &mut Ui, c: &mut Cur, id: NodeId) {
    let Some(n) = app.doc.get(id) else { return };
    if !super::markable(app, id) && n.dev.status.is_none() {
        return;
    }
    let status = n.dev.status;
    let updated = n.dev.status_updated.clone();
    let w = c.w - 2.0 * PAD;
    if let Some(s) = status {
        let r = c.rect(PAD, 0.0, w, 22.0);
        let pw = status_pill(ui.painter(), r.min, s);
        if let Some(u) = updated {
            let date = u.split('T').next().unwrap_or(&u).to_string();
            ui.painter().text(Pos2::new(r.left() + pw + 8.0, r.center().y), Align2::LEFT_CENTER, date, theme::font_caption(), theme::TEXT_TERTIARY);
        }
        c.y += 28.0;
    }
    let row = c.rect(PAD, 0.0, w, 28.0);
    let half = Rect::from_min_size(row.min, Vec2::new((w - 8.0) / 2.0, 28.0));
    let other = half.translate(Vec2::new(half.width() + 8.0, 0.0));
    match status {
        None => {
            if primary_button(ui, row, "ready", "Mark as ready for dev") {
                app.dev_mark_ready(&[id]);
            }
        }
        Some(DevStatus::ReadyForDev) => {
            if primary_button(ui, half, "complete", "Mark completed") {
                app.dev_mark_completed(&[id]);
            }
            if small_button(ui, other, ("dev-clear", 0), "Remove status") {
                app.dev_clear_status(&[id]);
            }
        }
        Some(DevStatus::Changed) => {
            if primary_button(ui, half, "compare", "Compare changes") {
                super::compare::open(app, id);
            }
            if small_button(ui, other, ("dev-remark", 0), "Mark ready") {
                app.dev_mark_ready(&[id]);
            }
        }
        Some(DevStatus::Completed) => {
            if primary_button(ui, half, "reopen", "Back to ready") {
                app.dev_mark_ready(&[id]);
            }
            if small_button(ui, other, ("dev-clear", 1), "Remove status") {
                app.dev_clear_status(&[id]);
            }
        }
    }
    c.y += 36.0;
}

/// Component properties of an instance, followed by the read-only Component Playground.
pub fn component_section(app: &mut App, ui: &mut Ui, c: &mut Cur, id: NodeId) {
    let rows = data::component_rows(&app.doc, id);
    if !rows.is_empty() {
        section_title(ui, c, "Component properties");
        for (label, value) in rows {
            crate::panels::inspect::kv(ui, app, c, &label, &value);
        }
        c.y += 8.0;
    }
    super::playground::playground_section(app, ui, c, id);
}

/// Wrapped chips toggling the pinned properties.
fn chips(ui: &mut Ui, c: &mut Cur, labels: &[&str], on: &mut Vec<String>) {
    let font = theme::font_caption();
    let max_x = c.w - 2.0 * PAD;
    let (mut x, mut y) = (0.0f32, 0.0f32);
    for (i, l) in labels.iter().enumerate() {
        let w = text_width(ui.painter(), l, &font) + 16.0;
        if x + w > max_x {
            x = 0.0;
            y += 26.0;
        }
        let r = c.rect(PAD + x, y, w, 22.0);
        widgets::record(ui, &format!("annot-chip-{l}"), r);
        let active = on.iter().any(|o| o == l);
        let resp = ui.interact(r, ui.id().with(("annot-chip", i)), Sense::click());
        let bg = if active {
            theme::BG_SELECTED
        } else if resp.hovered() {
            theme::BG_TERTIARY
        } else {
            theme::BG_SECONDARY
        };
        ui.painter().rect_filled(r, theme::cr_pill(r.height()), bg);
        if active {
            ui.painter().rect_stroke(r, theme::cr_pill(r.height()), Stroke::new(1.0, theme::BORDER_SELECTED), egui::StrokeKind::Inside);
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, l, font.clone(), theme::TEXT);
        if resp.clicked() {
            if active {
                on.retain(|o| o != l);
            } else {
                on.push(l.to_string());
            }
        }
        x += w + 4.0;
    }
    c.y += y + 28.0;
}

fn annotation_editor(app: &mut App, ui: &mut Ui, c: &mut Cur, mut edit: AnnotEdit) {
    let w = c.w - 2.0 * PAD;
    let labels: Vec<&'static str> = app.doc.get(edit.node).map(data::applicable_properties).unwrap_or_default();
    let r = c.rect(PAD, 0.0, w, 28.0);
    widgets::record(ui, "annot-text", r);
    if let Some(t) = field::text_input_ph(ui, r, "annot-text", &edit.text, "Add a note") {
        edit.text = t;
    }
    c.y += 36.0;
    ui.painter().text(Pos2::new(c.x0 + PAD, c.oy + c.y + 8.0), Align2::LEFT_CENTER, "Pin properties", theme::font_caption(), theme::TEXT_SECONDARY);
    c.y += 20.0;
    chips(ui, c, &labels, &mut edit.props);
    let row = c.rect(PAD, 0.0, w, 28.0);
    let third = (w - 16.0) / 3.0;
    let b = |i: f32| Rect::from_min_size(Pos2::new(row.left() + i * (third + 8.0), row.top()), Vec2::new(third, 28.0));
    let mut done = false;
    let mut keep = true;
    if primary_button(ui, b(0.0), "annot-save", "Save") {
        done = true;
    }
    if small_button(ui, b(1.0), "annot-cancel", "Cancel") {
        keep = false;
    }
    if let Some(i) = edit.index {
        if small_button(ui, b(2.0), "annot-delete", "Delete") {
            let node = edit.node;
            app.dev_edit(node, |d| {
                if i < d.annotations.len() {
                    d.annotations.remove(i);
                }
            });
            keep = false;
            done = false;
        }
    }
    c.y += 40.0;
    if done {
        let (node, text, props, index) = (edit.node, edit.text.trim().to_string(), edit.props.clone(), edit.index);
        if !text.is_empty() || !props.is_empty() {
            app.dev_edit(node, |d| match index {
                Some(i) if i < d.annotations.len() => {
                    d.annotations[i].text = text.clone();
                    d.annotations[i].properties = props.clone();
                }
                _ => d.annotations.push(Annotation { text: text.clone(), properties: props.clone(), category: None }),
            });
        }
        keep = false;
    }
    app.dev.annot = keep.then_some(edit);
}

/// Annotations of the layer with the add / edit flow.
pub fn annotations_section(app: &mut App, ui: &mut Ui, c: &mut Cur, id: NodeId) {
    section_title(ui, c, "Annotations");
    let head = c.rect(c.w - PAD - 56.0, -24.0, 56.0, 24.0);
    widgets::record(ui, "annot-add", head);
    if small_button(ui, head, "annot-add", "Add") {
        app.dev.annot = Some(AnnotEdit { node: id, index: None, text: String::new(), props: Vec::new() });
    }
    let annots = app.doc.get(id).map(|n| n.dev.annotations.clone()).unwrap_or_default();
    let editing = app.dev.annot.clone().filter(|e| e.node == id);
    if annots.is_empty() && editing.is_none() {
        let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, ROW_H);
        ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, "No annotations", theme::font_body(), theme::TEXT_TERTIARY);
        c.y += ROW_H;
    }
    for (i, a) in annots.iter().enumerate() {
        if let Some(e) = editing.as_ref().filter(|e| e.index == Some(i)) {
            annotation_editor(app, ui, c, e.clone());
            continue;
        }
        let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, ROW_H);
        let resp = ui.interact(r, ui.id().with(("annot-row", i)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r.expand2(Vec2::new(8.0, 0.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        }
        ui.painter().circle_filled(Pos2::new(r.left() + 4.0, r.center().y), 3.5, theme::ANNOTATION_LINE);
        let text = elide(ui.painter(), if a.text.is_empty() { "(no text)" } else { &a.text }, &theme::font_body(), r.width() - 20.0);
        ui.painter().text(Pos2::new(r.left() + 14.0, r.center().y), Align2::LEFT_CENTER, text, theme::font_body(), theme::TEXT);
        if resp.clicked() {
            app.dev.annot = Some(AnnotEdit { node: id, index: Some(i), text: a.text.clone(), props: a.properties.clone() });
        }
        c.y += ROW_H;
        for label in &a.properties {
            let v = app.doc.get(id).and_then(|n| data::property_value(&app.doc, n, label)).unwrap_or_else(|| "-".into());
            let r = c.rect(PAD + 14.0, 0.0, c.w - 2.0 * PAD - 14.0, 20.0);
            ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, label, theme::font_caption(), theme::TEXT_SECONDARY);
            let v = elide(ui.painter(), &v, &theme::font_caption(), r.width() - 100.0);
            ui.painter().text(Pos2::new(r.left() + 100.0, r.center().y), Align2::LEFT_CENTER, v, theme::font_caption(), theme::TEXT);
            c.y += 20.0;
        }
    }
    if let Some(e) = editing.filter(|e| e.index.is_none()) {
        annotation_editor(app, ui, c, e);
    }
    c.y += 8.0;
}

fn link_editor(app: &mut App, ui: &mut Ui, c: &mut Cur, mut edit: LinkEdit) {
    let w = c.w - 2.0 * PAD;
    let r = c.rect(PAD, 0.0, w, 28.0);
    widgets::record(ui, "link-label", r);
    if let Some(t) = field::text_input_ph(ui, r, "link-label", &edit.label, "Label (Storybook, Jira ...)") {
        edit.label = t;
    }
    c.y += 34.0;
    let r = c.rect(PAD, 0.0, w, 28.0);
    widgets::record(ui, "link-url", r);
    if let Some(t) = field::text_input_ph(ui, r, "link-url", &edit.url, "https://") {
        edit.url = t;
    }
    c.y += 36.0;
    let row = c.rect(PAD, 0.0, w, 28.0);
    let half = (w - 8.0) / 2.0;
    let add = Rect::from_min_size(row.min, Vec2::new(half, 28.0));
    let cancel = add.translate(Vec2::new(half + 8.0, 0.0));
    let mut keep = true;
    let valid = crate::present::url_allowed(&edit.url);
    if primary_button(ui, add, "link-add", "Add link") {
        if valid {
            let (node, label, url) = (edit.node, edit.label.trim().to_string(), edit.url.trim().to_string());
            app.dev_edit(node, |d| d.links.push(DevLink { label: if label.is_empty() { url.clone() } else { label }, url }));
            keep = false;
        } else {
            app.show_toast("Enter an http(s) or mailto link", true);
        }
    }
    if small_button(ui, cancel, "link-cancel", "Cancel") {
        keep = false;
    }
    c.y += 40.0;
    app.dev.link = keep.then_some(edit);
}

/// Links attached to the layer; clicking one asks before opening it.
pub fn links_section(app: &mut App, ui: &mut Ui, c: &mut Cur, id: NodeId) {
    section_title(ui, c, "Dev links");
    let head = c.rect(c.w - PAD - 56.0, -24.0, 56.0, 24.0);
    widgets::record(ui, "link-new", head);
    if small_button(ui, head, "link-new", "Add") {
        app.dev.link = Some(LinkEdit { node: id, label: String::new(), url: String::new() });
    }
    let links = app.doc.get(id).map(|n| n.dev.links.clone()).unwrap_or_default();
    let editing = app.dev.link.clone().filter(|e| e.node == id);
    if links.is_empty() && editing.is_none() {
        let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, ROW_H);
        ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, "No links", theme::font_body(), theme::TEXT_TERTIARY);
        c.y += ROW_H;
    }
    for (i, l) in links.iter().enumerate() {
        let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, 36.0);
        let open = Rect::from_min_max(r.min, Pos2::new(r.right() - 28.0, r.max.y));
        widgets::record(ui, &format!("dev-link-{i}"), open);
        let resp = ui.interact(open, ui.id().with(("dev-link", i)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r.expand2(Vec2::new(8.0, 0.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        }
        let name = elide(ui.painter(), if l.label.is_empty() { &l.url } else { &l.label }, &theme::font_body(), open.width());
        ui.painter().text(Pos2::new(r.left(), r.top() + 11.0), Align2::LEFT_CENTER, name, theme::font_body(), theme::TEXT_BRAND);
        let url = elide(ui.painter(), &l.url, &theme::font_caption(), open.width());
        ui.painter().text(Pos2::new(r.left(), r.top() + 26.0), Align2::LEFT_CENTER, url, theme::font_caption(), theme::TEXT_TERTIARY);
        let x = Rect::from_min_size(Pos2::new(r.right() - 24.0, r.center().y - 12.0), Vec2::splat(24.0));
        if widgets::icon_button(ui, x, ("link-del", i), crate::icons::Icon::Close, false, "Remove link").clicked() {
            app.dev_edit(id, |d| {
                if i < d.links.len() {
                    d.links.remove(i);
                }
            });
        }
        if resp.clicked() {
            app.dev_open_link(&l.label, &l.url);
        }
        c.y += 36.0;
    }
    if let Some(e) = editing {
        link_editor(app, ui, c, e);
    }
    c.y += 8.0;
}

fn band_label(p: &egui::Painter, at: Pos2, v: f32) {
    if v > 0.0 {
        p.text(at, Align2::CENTER_CENTER, fmt_num((v * 10.0).round() / 10.0), theme::font_caption_medium(), theme::BOX_TEXT);
    } else {
        p.text(at, Align2::CENTER_CENTER, "-", theme::font_caption(), theme::BOX_TEXT.gamma_multiply(0.5));
    }
}

/// Paints the box model diagram (margin -> padding -> content) into `r`.
pub fn paint_box_model(p: &egui::Painter, r: Rect, m: &BoxModel) {
    let band = 26.0;
    let rr = theme::cr(theme::RADIUS_CONTROL);
    p.rect_filled(r, rr, theme::BOX_MARGIN);
    let pad = r.shrink(band);
    p.rect_filled(pad, theme::cr(3), theme::BOX_PADDING);
    let content = pad.shrink(band);
    p.rect_filled(content, theme::cr(2), theme::BOX_CONTENT);
    let cap = theme::font_caption();
    p.text(Pos2::new(r.left() + 6.0, r.top() + 2.0), Align2::LEFT_TOP, "margin", cap.clone(), theme::BOX_TEXT.gamma_multiply(0.7));
    p.text(Pos2::new(pad.left() + 6.0, pad.top() + 2.0), Align2::LEFT_TOP, "padding", cap, theme::BOX_TEXT.gamma_multiply(0.7));
    // top, right, bottom, left
    let side = |outer: Rect, i: usize| match i {
        0 => Pos2::new(outer.center().x, outer.top() + band / 2.0 + 4.0),
        1 => Pos2::new(outer.right() - band / 2.0, outer.center().y),
        2 => Pos2::new(outer.center().x, outer.bottom() - band / 2.0),
        _ => Pos2::new(outer.left() + band / 2.0, outer.center().y),
    };
    for i in 0..4 {
        band_label(p, side(r, i), m.margin[i]);
        band_label(p, side(pad, i), m.padding[i]);
    }
    p.text(
        content.center(),
        Align2::CENTER_CENTER,
        format!("{} \u{00D7} {}", fmt_num(m.size.0), fmt_num(m.size.1)),
        theme::font_caption_medium(),
        theme::BOX_TEXT,
    );
}

/// Layout rows plus the box model diagram.
pub fn box_model_section(ui: &mut Ui, c: &mut Cur, doc: &Document, id: NodeId) {
    let Some(m) = data::box_model(doc, id) else { return };
    let w = c.w - 2.0 * PAD;
    let r = c.rect(PAD, 0.0, w, 150.0);
    paint_box_model(ui.painter(), r, &m);
    c.y += 158.0;
}

fn mono_char_w(p: &egui::Painter) -> f32 {
    text_width(p, "MMMMMMMMMM", &theme::font_mono()) / 10.0
}

/// Syntax-highlighted code, wrapped to the panel width.
pub fn code_block(ui: &mut Ui, c: &mut Cur, lang: Language, code: &str) {
    let font = theme::font_mono();
    let line_h = theme::CODE_LINE_HEIGHT;
    let inner_w = c.w - 2.0 * PAD - 16.0;
    let cw = mono_char_w(ui.painter());
    let cols = (inner_w / cw).floor() as usize;
    let mut rows: Vec<Vec<Span>> = Vec::new();
    for l in highlight::highlight(lang, code) {
        rows.extend(highlight::wrap(&l, cols));
    }
    let h = rows.len() as f32 * line_h + 16.0;
    let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, h);
    ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    let clip = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    let mut y = r.top() + 8.0;
    for row in rows {
        let mut x = r.left() + 8.0;
        for s in row {
            clip.text(Pos2::new(x, y + line_h / 2.0), Align2::LEFT_CENTER, &s.text, font.clone(), highlight::color(s.tok));
            x += cw * s.text.chars().count() as f32;
        }
        y += line_h;
    }
    c.y += h + 8.0;
}

type DevCodeCacheKey = (u64, NodeId, u64, Language, bool, u64);
type DevAssetsCacheKey = (u64, NodeId, u64, usize);
type DevAssetsCacheEntry = (DevAssetsCacheKey, Vec<(String, NodeId, Asset)>);
thread_local! {
    static DEV_CODE_CACHE: std::cell::RefCell<Option<(DevCodeCacheKey, String)>> = const { std::cell::RefCell::new(None) };
    static DEV_ASSETS_CACHE: std::cell::RefCell<Option<DevAssetsCacheEntry>> = const { std::cell::RefCell::new(None) };
}

/// Code section: language dropdown, copy button and the generated code.
pub fn code_section(app: &mut App, ui: &mut Ui, c: &mut Cur, id: NodeId) {
    let lang = app.dev.lang;
    section_title(ui, c, "Code");
    let copy_r = c.rect(c.w - PAD - 56.0, -24.0, 56.0, 24.0);
    let dd = c.rect(c.w - PAD - 56.0 - 6.0 - 108.0, -24.0, 108.0, 24.0);
    widgets::record(ui, "dev-lang", dd);
    widgets::record(ui, "dev-copy-code", copy_r);
    let options: Vec<String> = Language::ALL.iter().map(|l| l.label().to_string()).collect();
    if let Some(i) = dropdown(ui, dd, "dev-lang", lang.label(), &options) {
        app.dev.lang = Language::ALL[i];
    }
    let lang = app.dev.lang;
    let pg_rev = if app.dev.playground.has_overrides_for(id) { app.dev.playground.rev } else { 0 };
    let key: DevCodeCacheKey = (app.doc.runtime_id(), id, app.doc.mutation_epoch(), lang, app.dev.react_tailwind, pg_rev);
    let code = DEV_CODE_CACHE.with(|cache| {
        if let Some((k, v)) = cache.borrow().as_ref() {
            if *k == key {
                return v.clone();
            }
        }
        let generated = app.dev_code_for(id, lang, app.dev.react_tailwind);
        *cache.borrow_mut() = Some((key, generated.clone()));
        generated
    });
    if small_button(ui, copy_r, "copy-code", "Copy") {
        copy(app, ui, code.clone(), lang.label());
    }
    if lang == Language::React {
        let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, 24.0);
        let resp = ui.interact(r, ui.id().with("react-tailwind"), Sense::click());
        ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, "Style with Tailwind classes", theme::font_body(), theme::TEXT_SECONDARY);
        let on = app.dev.react_tailwind;
        let tr = Rect::from_center_size(Pos2::new(r.right() - 14.0, r.center().y), Vec2::new(28.0, 16.0));
        ui.painter().rect_filled(tr, theme::cr(8), if on { theme::BG_BRAND } else { theme::BG_TERTIARY });
        ui.painter().circle_filled(Pos2::new(if on { tr.right() - 8.0 } else { tr.left() + 8.0 }, tr.center().y), 6.0, theme::TEXT);
        if resp.clicked() {
            app.dev.react_tailwind = !on;
        }
        c.y += 28.0;
    }
    if code.is_empty() {
        let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, ROW_H);
        ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, "No code", theme::font_body(), theme::TEXT_TERTIARY);
        c.y += ROW_H;
    } else {
        code_block(ui, c, lang, &code);
    }
}

fn kind_name(k: VariableKind) -> &'static str {
    match k {
        VariableKind::Color => "Color",
        VariableKind::Number => "Number",
        VariableKind::String => "String",
        VariableKind::Boolean => "Boolean",
    }
}

fn c32(c: &Color) -> Color32 {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(f(c.r), f(c.g), f(c.b), f(c.a))
}

/// Variables bound anywhere in the layer tree, with the value of every mode.
pub fn variables_section(app: &mut App, ui: &mut Ui, c: &mut Cur, id: NodeId) {
    let used = data::variables_used(&app.doc, id);
    if used.is_empty() {
        return;
    }
    section_title(ui, c, "Variables used");
    for (i, u) in used.iter().enumerate() {
        let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, 24.0);
        let resp = ui.interact(r, ui.id().with(("var-use", i)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r.expand2(Vec2::new(8.0, 0.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        }
        let name = elide(ui.painter(), &u.name, &theme::font_body_strong(), r.width() - 60.0);
        ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, name, theme::font_body_strong(), theme::TEXT);
        ui.painter().text(
            Pos2::new(r.right(), r.center().y),
            Align2::RIGHT_CENTER,
            u.kind.map(kind_name).unwrap_or("Missing"),
            theme::font_caption(),
            theme::TEXT_TERTIARY,
        );
        if resp.clicked() {
            copy(app, ui, u.name.clone(), "variable name");
        }
        c.y += 24.0;
        token_note(ui, c, &format!("Collection: {}", if u.collection.is_empty() { "Unavailable" } else { &u.collection }));
        token_note(ui, c, &format!("Bindings: {}", u.props.join(", ")));
        if let Some(v) = app.doc.variables.get(&u.id) {
            let reference = designos_export::css_variable_reference(v);
            let label = if v.code_syntax_for(CodeSyntaxPlatform::Web).is_some() { "Web code syntax" } else { "CSS reference" };
            // Android and iOS code syntax, as written on the variable
            let other_syntax: Vec<(&str, &str, String)> =
                [(CodeSyntaxPlatform::Android, "android", "Android code syntax"), (CodeSyntaxPlatform::Ios, "ios", "iOS code syntax")]
                    .into_iter()
                    .filter_map(|(platform, key, label)| Some((key, label, v.code_syntax_for(platform)?.to_string())))
                    .collect();
            token_copy_row(app, ui, c, &format!("dev-token-ref-{i}"), label, &reference, None);
            for (key, label, code) in other_syntax {
                token_copy_row(app, ui, c, &format!("dev-token-syntax-{i}-{key}"), label, &code, None);
            }
        }
        token_note(ui, c, "Effective values · click to copy");
        for (j, binding) in u.effective.iter().enumerate() {
            // A token often drives all four corners. Keep distinct layer/mode values,
            // but show identical values on one layer only once.
            let same_value = |other: &&data::EffectiveVarUse| other.node == binding.node && other.mode == binding.mode && other.value == binding.value;
            if u.effective[..j].iter().any(|other| same_value(&other)) {
                continue;
            }
            let properties = u.effective.iter().filter(same_value).map(|other| other.property.as_str()).collect::<Vec<_>>().join(", ");
            token_note(ui, c, &format!("{} · {}", binding.node_name, properties));
            let mode = binding.mode.as_deref().unwrap_or("Unknown mode");
            match &binding.value {
                Some(value) => {
                    let text = match value {
                        VariableValue::Color(col) => super::color::ColorFormat::Rgb.value(*col, col.a),
                        VariableValue::String(s) => serde_json::to_string(s).unwrap_or_default(),
                        _ => data::format_value(value),
                    };
                    let swatch = match value {
                        VariableValue::Color(col) => Some(*col),
                        _ => None,
                    };
                    token_copy_row(app, ui, c, &format!("dev-token-value-{i}-{j}"), mode, &text, swatch);
                }
                None => token_note(ui, c, &format!("{mode} · Unresolved")),
            }
        }
        let toggle = c.rect(PAD, 0.0, c.w - 2.0 * PAD, 24.0);
        let open_id = ui.id().with(("token-mode-previews", &u.id));
        let mut open = ui.ctx().data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
        if small_button(ui, toggle, ("token-mode-previews", i), if open { "Hide mode previews" } else { "Show mode previews" }) {
            open = !open;
            ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
        }
        c.y += 28.0;
        if open {
            token_note(ui, c, "Cross-collection aliases use defaults here");
            for (mode, value) in &u.modes {
                let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, 22.0);
                ui.painter().text(
                    Pos2::new(r.left() + 8.0, r.center().y),
                    Align2::LEFT_CENTER,
                    elide(ui.painter(), mode, &theme::font_caption(), 72.0),
                    theme::font_caption(),
                    theme::TEXT_SECONDARY,
                );
                let mut x = r.left() + 88.0;
                if let Some(VariableValue::Color(col)) = value {
                    widgets::swatch(ui.painter(), Rect::from_min_size(Pos2::new(x, r.center().y - 7.0), Vec2::splat(14.0)), c32(col));
                    x += 20.0;
                }
                let text = value.as_ref().map(data::format_value).unwrap_or_else(|| "-".into());
                ui.painter().text(
                    Pos2::new(x, r.center().y),
                    Align2::LEFT_CENTER,
                    elide(ui.painter(), &text, &theme::font_caption(), r.right() - x),
                    theme::font_caption(),
                    theme::TEXT,
                );
                c.y += 22.0;
            }
        }
        c.y += 4.0;
    }
    c.y += 4.0;
}

fn token_note(ui: &mut Ui, c: &mut Cur, text: &str) {
    let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, 20.0);
    ui.painter().text(
        r.left_center(),
        Align2::LEFT_CENTER,
        elide(ui.painter(), text, &theme::font_caption(), r.width()),
        theme::font_caption(),
        theme::TEXT_SECONDARY,
    );
    ui.interact(r, ui.id().with(("token-note", c.y.to_bits(), text)), Sense::hover()).on_hover_text(text);
    c.y += 20.0;
}

/// A two-line copy target keeps full token references usable in a narrow inspector.
fn token_copy_row(app: &mut App, ui: &mut Ui, c: &mut Cur, key: &str, label: &str, value: &str, color: Option<Color>) {
    let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, 42.0);
    widgets::record(ui, key, r);
    let resp = ui.interact(r, ui.id().with(key), Sense::click()).on_hover_text(format!("Copy {label}: {value}"));
    ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
    ui.painter().text(
        Pos2::new(r.left() + 8.0, r.top() + 10.0),
        Align2::LEFT_CENTER,
        elide(ui.painter(), label, &theme::font_caption(), r.width() - 16.0),
        theme::font_caption(),
        theme::TEXT_SECONDARY,
    );
    let mut x = r.left() + 8.0;
    if let Some(col) = color {
        widgets::swatch(ui.painter(), Rect::from_min_size(Pos2::new(x, r.top() + 22.0), Vec2::splat(12.0)), c32(&col));
        x += 18.0;
    }
    ui.painter().text(
        Pos2::new(x, r.top() + 29.0),
        Align2::LEFT_CENTER,
        elide(ui.painter(), value, &theme::font_body(), r.right() - x - 8.0),
        theme::font_body(),
        theme::TEXT,
    );
    if resp.clicked() {
        copy(app, ui, value.to_string(), label);
    }
    c.y += 48.0;
}

/// Icon and vector subtrees (plus embedded images) with an SVG export for each.
pub fn assets_section(app: &mut App, ui: &mut Ui, c: &mut Cur, id: NodeId) {
    section_title(ui, c, "Assets");
    let key = (app.doc.runtime_id(), id, app.doc.mutation_epoch(), app.doc.images.len());
    let rows = DEV_ASSETS_CACHE.with(|cache| {
        if let Some((k, v)) = cache.borrow().as_ref() {
            if *k == key {
                return v.clone();
            }
        }
        let base = app.doc.get(id).map(|n| designos_export::sanitize_file_name(&n.name)).unwrap_or_default();
        let icons = if app.doc.nodes.len() > 10_000 && app.doc.get(id).is_some_and(|n| n.children.len() > 64 || matches!(n.kind, NodeKind::Section)) {
            Vec::new()
        } else {
            designos_export::vector_assets(&app.doc, id)
        };
        let mut rows: Vec<(String, NodeId, Asset)> = Vec::new();
        for i in icons {
            let name = app.doc.get(i).map(|n| designos_export::sanitize_file_name(&n.name)).unwrap_or_default();
            rows.push((format!("{name}.svg"), i, Asset::Svg));
        }
        for a in crate::panels::inspect::collect_assets(&app.doc, id) {
            let label = match &a {
                Asset::Image(r) => format!("Image {}", r.chars().take(6).collect::<String>()),
                Asset::Svg => format!("{base}.svg"),
                Asset::Png2x => format!("{base}@2x.png"),
            };
            // the layer's own SVG is listed once even when it is an icon itself
            if matches!(a, Asset::Svg) && rows.iter().any(|(_, n, _)| *n == id) {
                continue;
            }
            rows.push((label, id, a));
        }
        *cache.borrow_mut() = Some((key, rows.clone()));
        rows
    });
    for (i, (label, node, a)) in rows.iter().enumerate() {
        let r = c.rect(PAD, 0.0, c.w - 2.0 * PAD, 28.0);
        let label = elide(ui.painter(), label, &theme::font_body(), r.width() - 84.0);
        ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT);
        let b = Rect::from_min_size(Pos2::new(r.right() - 76.0, r.top() + 2.0), Vec2::new(76.0, 24.0));
        let text = if matches!(a, Asset::Svg) { "Export SVG" } else { "Download" };
        if small_button(ui, b, ("asset", i), text) {
            download(app, *node, a);
        }
        c.y += 28.0;
    }
}
