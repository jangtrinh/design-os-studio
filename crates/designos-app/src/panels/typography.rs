//! Typography section of the inspector. Edits the selected character range
//! while a text session is open (splitting and merging runs), otherwise every
//! selected text layer. Properties that differ over the range show "Mixed".

use super::properties::{open_picker, row_end, PickTarget};
use super::properties::{segmented, Pane};
use super::var_bind;
use crate::app::App;
use crate::canvas::textsession as ts;
use crate::layout;
use crate::richtext::{self, ListKind, Summary};
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::dropdown;
use crate::widgets::{self};
use designos_doc::*;
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

pub(super) const WEIGHTS: [(&str, u16); 9] = [
    ("Thin", 100),
    ("Extra Light", 200),
    ("Light", 300),
    ("Regular", 400),
    ("Medium", 500),
    ("Semi Bold", 600),
    ("Bold", 700),
    ("Extra Bold", 800),
    ("Black", 900),
];

/// Combines two summaries: properties that differ become mixed.
pub fn combine(a: &Summary, b: &Summary) -> Summary {
    fn m<T: PartialEq + Clone>(x: &Option<T>, y: &Option<T>) -> Option<T> {
        if x == y {
            x.clone()
        } else {
            None
        }
    }
    Summary {
        family: m(&a.family, &b.family),
        weight: m(&a.weight, &b.weight),
        italic: m(&a.italic, &b.italic),
        size: m(&a.size, &b.size),
        line_height: m(&a.line_height, &b.line_height),
        letter_spacing: m(&a.letter_spacing, &b.letter_spacing),
        fills: m(&a.fills, &b.fills),
        decoration: m(&a.decoration, &b.decoration),
        case: m(&a.case, &b.case),
        features: m(&a.features, &b.features),
    }
}

/// Summary over whole text layers (not editing).
pub fn nodes_summary(app: &App, sel: &[NodeId]) -> Option<Summary> {
    sel.iter()
        .take(64)
        .filter_map(|id| match app.doc.get(*id).map(|n| &n.kind) {
            Some(NodeKind::Text(t)) => Some(richtext::summary(t, 0..t.content.len())),
            _ => None,
        })
        .reduce(|a, b| combine(&a, &b))
}

fn editing(app: &App, sel: &[NodeId]) -> bool {
    app.canvas.text.as_ref().is_some_and(|s| sel.contains(&s.id))
}

/// Applies `f` to the character range being edited, or to every selected text layer.
pub fn set_style(app: &mut App, sel: &[NodeId], f: impl Fn(&mut TextStyle)) {
    if editing(app, sel) {
        ts::style_selection_as(app, &f, crate::textedit::EditKind::Style);
        return;
    }
    app.edit_nodes(sel, |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            richtext::apply_all(t, &f);
            n.size = layout::fit_text(t, n.size);
        }
    });
}

/// Applies `f` to the base style only (paragraph level fields such as alignment).
fn set_base(app: &mut App, sel: &[NodeId], f: impl Fn(&mut TextStyle)) {
    if editing(app, sel) {
        ts::base_style(app, &f);
        return;
    }
    app.edit_nodes(sel, |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            f(&mut t.style);
            n.size = layout::fit_text(t, n.size);
        }
    });
}

/// Segmented control with short text labels instead of icons.
fn seg_text(ui: &mut Ui, pane: &Pane, x: f32, key: &str, labels: &[&str], selected: Option<usize>, w: f32) -> Option<usize> {
    let mut out = None;
    let n = labels.len();
    for (i, label) in labels.iter().enumerate() {
        let r = pane.rect(x + i as f32 * w, 0.0, w, theme::FIELD_HEIGHT);
        let radius = match (i == 0, i + 1 == n) {
            (true, true) => theme::cr(theme::RADIUS_CONTROL),
            (true, false) => theme::cr_each(theme::RADIUS_CONTROL, 0, 0, theme::RADIUS_CONTROL),
            (false, true) => theme::cr_each(0, theme::RADIUS_CONTROL, theme::RADIUS_CONTROL, 0),
            _ => theme::cr(0),
        };
        let resp = ui.interact(r, ui.id().with(("segt", key, i)), Sense::click());
        let sel = selected == Some(i);
        let bg = if sel {
            theme::BG_TERTIARY
        } else if resp.hovered() {
            theme::BG_TERTIARY.gamma_multiply(0.6)
        } else {
            theme::BG_SECONDARY
        };
        ui.painter().rect_filled(r, radius, bg);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *label, theme::font_body_strong(), if sel { theme::ICON } else { theme::ICON_SECONDARY });
        if i > 0 {
            widgets::vline(ui.painter(), r.left(), r.top(), r.bottom(), theme::BG);
        }
        if resp.clicked() {
            out = Some(i);
        }
    }
    out
}

pub(super) fn weight_name(w: u16) -> &'static str {
    WEIGHTS.iter().min_by_key(|x| (x.1 as i32 - w as i32).abs()).map(|x| x.0).unwrap_or("Regular")
}

fn solid_of(fills: &[Paint]) -> Option<(Color, f32)> {
    fills.iter().find(|p| p.visible).and_then(|p| match &p.kind {
        PaintKind::Solid(c) => Some((*c, p.opacity)),
        _ => None,
    })
}

pub(super) fn section(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], t: &TextProps) {
    let in_session = editing(app, sel);
    let sum = if in_session { ts::session_summary(app) } else { nodes_summary(app, sel) };
    let Some(sum) = sum else { return };
    let more_id = ui.id().with("typo-more");
    let more = ui.data(|d| d.get_temp::<bool>(more_id)).unwrap_or(false);

    // "Type settings" (Details popover) button, left of the "More" toggle in the title row.
    let tb = pane.rect(
        theme::SECTION_TRAILING_X - theme::PANEL_ACTION_BUTTON - 2.0,
        -theme::SECTION_TITLE_HEIGHT + 8.0,
        theme::PANEL_ACTION_BUTTON,
        theme::PANEL_ACTION_BUTTON,
    );
    let details_open = app.props.pop.as_ref().is_some_and(|p| p.kind == super::effects::PopKind::TextDetails);
    if widgets::icon_button(ui, tb, "type-details", crate::icons::Icon::StrokeSettings, details_open, "Type settings").clicked() {
        app.props.pop = (!details_open).then(|| super::effects::PopState { kind: super::effects::PopKind::TextDetails, anchor: tb, ids: sel.to_vec() });
    }

    // "More type settings" toggle sits in the title row (the title was drawn by the caller).
    let mb = pane.rect(theme::SECTION_TRAILING_X, -theme::SECTION_TITLE_HEIGHT + 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    let mr = ui.interact(mb, ui.id().with("typo-more-btn"), Sense::click());
    if mr.hovered() || more {
        ui.painter().rect_filled(mb, theme::cr(theme::RADIUS_CONTROL), if more { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
    }
    ui.painter().text(mb.center(), Align2::CENTER_CENTER, "\u{2026}", theme::font_body_strong(), if more { theme::ICON } else { theme::ICON_SECONDARY });
    mr.clone().on_hover_text("More type settings");
    if mr.clicked() {
        ui.data_mut(|d| d.insert_temp(more_id, !more));
    }

    // ---- family
    let fam_label = sum.family.clone().unwrap_or_else(|| "Mixed".to_string());
    if let Some(f) = family_picker(ui, pane.full(0.0), &fam_label) {
        set_style(app, sel, |s| s.font_family = f.clone());
        app.fonts.load_for(&app.doc);
    }
    row_end(pane);

    // ---- weight + size
    let names: Vec<String> = WEIGHTS.iter().map(|w| w.0.to_string()).collect();
    let cur = sum.weight.map(weight_name).unwrap_or("Mixed");
    if let Some(i) = dropdown(ui, pane.col(0, 0.0), "font-weight", cur, &names) {
        let w = WEIGHTS[i].1;
        set_style(app, sel, |s| s.font_weight = w);
        app.fonts.load_for(&app.doc);
    }
    let fs_r = pane.col(1, 0.0);
    if let Some(v) = var_bind::numeric(app, ui, fs_r, sel, &["fontSize"], |ui| field::num_field(ui, fs_r, "font-size", Prefix::Text("S"), sum.size, "")) {
        let v = v.max(1.0);
        set_style(app, sel, |s| s.font_size = v);
    }
    row_end(pane);

    // ---- line height + letter spacing
    let lh = sum.line_height.map(|l| richtext::display_line_height_px(l, sum.size.unwrap_or(t.style.font_size)));
    if let Some(v) = field::num_field(ui, pane.col(0, 0.0), "line-height", Prefix::Text("\u{2195}"), lh, "") {
        let v = v.max(1.0);
        set_style(app, sel, |s| s.line_height = LineHeight::Pixels(v));
    }
    if let Some(v) = field::num_field(ui, pane.col(1, 0.0), "letter-spacing", Prefix::Text("\u{2194}"), sum.letter_spacing, "") {
        set_style(app, sel, |s| s.letter_spacing = v);
    }
    row_end(pane);

    // ---- alignment (paragraph level: always the base style)
    let cur = match t.style.align_h {
        TextAlignH::Left | TextAlignH::Justified => 0,
        TextAlignH::Center => 1,
        TextAlignH::Right => 2,
    };
    if let Some(i) = segmented(
        ui,
        pane,
        theme::SECTION_CONTENT_X,
        "text-align",
        &[crate::icons::Icon::TextLeft, crate::icons::Icon::TextCenter, crate::icons::Icon::TextRight],
        Some(cur),
    ) {
        let a = [TextAlignH::Left, TextAlignH::Center, TextAlignH::Right][i];
        set_base(app, sel, |s| s.align_h = a);
    }
    // list buttons share the row
    // the buttons show the list kind of the edited paragraphs (or of the whole layer)
    let list_range = if in_session { app.canvas.text.as_ref().map(|s| s.range()) } else { Some(0..t.content.len()) };
    let list_now = list_range.and_then(|r| richtext::list_kind_in(t, r)).map(|k| if k == ListKind::Bullet { 0 } else { 1 });
    if let Some(i) = seg_text(ui, pane, theme::FIELD_COL_X[1], "list", &["\u{2022}", "1."], list_now, theme::SEGMENT_W) {
        ts::text_command(app, ts::TextCmd::List(if i == 0 { ListKind::Bullet } else { ListKind::Numbered }));
    }
    row_end(pane);

    if more {
        // ---- decoration + case
        let dec = sum.decoration.map(|d| match d {
            TextDecoration::None => 0,
            TextDecoration::Underline => 1,
            TextDecoration::Strikethrough => 2,
        });
        if let Some(i) = seg_text(ui, pane, theme::SECTION_CONTENT_X, "decoration", &["\u{2013}", "U", "S"], dec, theme::SEGMENT_W) {
            let d = [TextDecoration::None, TextDecoration::Underline, TextDecoration::Strikethrough][i];
            set_style(app, sel, |s| s.decoration = d);
        }
        let case = sum.case.and_then(|c| match c {
            TextCase::Original => Some(0),
            TextCase::Upper => Some(1),
            TextCase::Lower => Some(2),
            TextCase::Title => Some(3),
            TextCase::SmallCaps => None,
        });
        if let Some(i) = seg_text(ui, pane, theme::FIELD_COL_X[1], "case", &["\u{2013}", "AG", "ag", "Ag"], case, theme::SEGMENT_W) {
            let c = [TextCase::Original, TextCase::Upper, TextCase::Lower, TextCase::Title][i];
            set_style(app, sel, |s| s.case = c);
        }
        row_end(pane);
        let italic = sum.italic;
        if let Some(i) = seg_text(ui, pane, theme::SECTION_CONTENT_X, "italic", &["Normal", "Italic"], italic.map(|b| b as usize), 48.0) {
            set_style(app, sel, |s| s.italic = i == 1);
            app.fonts.load_for(&app.doc);
        }
        row_end(pane);
    }
}

/// The Fill section of text layers: the text colour (`TextStyle::fills`, per character range
/// while a text session has a selection) with the variable bind button.
pub(super) fn fill_section(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId]) {
    let sum = if editing(app, sel) { ts::session_summary(app) } else { nodes_summary(app, sel) };
    if let Some(sum) = sum {
        color_row(app, ui, pane, sel, &sum);
    }
}

fn color_row(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], sum: &Summary) {
    let sw = pane.rect(theme::SWATCH_ROW_X, 4.0, theme::SWATCH_SIZE, theme::SWATCH_SIZE);
    let hex_r = pane.rect(theme::SWATCH_ROW_X + theme::SWATCH_SIZE + 8.0, 0.0, 96.0, theme::FIELD_HEIGHT);
    let op_r = pane.rect(theme::SWATCH_ROW_X + theme::SWATCH_SIZE + 8.0 + 96.0 + 4.0, 0.0, theme::OPACITY_FIELD_W, theme::FIELD_HEIGHT);
    let solid = sum.fills.as_deref().and_then(solid_of);
    match (&sum.fills, solid) {
        (Some(_), Some((c, op))) => {
            let col = {
                let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                Color32::from_rgba_unmultiplied(f(c.r), f(c.g), f(c.b), f(c.a * op))
            };
            widgets::swatch(ui.painter(), sw, col);
            widgets::record(ui, "text-sw", sw);
            widgets::record(ui, "text-hex", hex_r);
            let swr = ui.interact(sw, ui.id().with("typo-sw"), Sense::click());
            // A variable binds the whole layer's colour (key `fills/0`, as the renderer resolves it).
            let field_r = Rect::from_min_max(hex_r.min, Pos2::new(op_r.left() - 4.0, hex_r.bottom()));
            let bound = var_bind::pill_field(app, ui, field_r, sel, &["fills/0"]);
            if swr.clicked() && !bound {
                open_picker(app, PickTarget::TextFill, col, sw, sel.to_vec());
            }
            if !bound {
                if let Some(s) = field::text_input(ui, hex_r, "text-hex", &c.to_hex()) {
                    if let Some(nc) = Color::from_hex(&s) {
                        set_style(app, sel, |st| set_solid(st, Color { a: c.a, ..nc }, None));
                    }
                }
                // Not offered while a character range is being edited: bindings are per layer.
                if !editing(app, sel) {
                    var_bind::hover_button(app, ui, hex_r, sel, &["fills/0"], VariableKind::Color);
                }
            }
            if let Some(v) = field::num_field(ui, op_r, "text-op", Prefix::None, Some((op * 100.0).round()), "%") {
                let o = (v / 100.0).clamp(0.0, 1.0);
                set_style(app, sel, |st| set_solid(st, c, Some(o)));
            }
        }
        (None, _) => {
            widgets::swatch(ui.painter(), sw, Color32::TRANSPARENT);
            field::field_bg(ui, hex_r, ui.id().with("text-mixed"));
            ui.painter().text(Pos2::new(hex_r.left() + 8.0, hex_r.center().y), Align2::LEFT_CENTER, "Mixed", theme::font_body(), theme::TEXT);
        }
        _ => {
            widgets::swatch(ui.painter(), sw, Color32::TRANSPARENT);
            field::field_bg(ui, hex_r, ui.id().with("text-other"));
            ui.painter().text(Pos2::new(hex_r.left() + 8.0, hex_r.center().y), Align2::LEFT_CENTER, "Gradient / image", theme::font_body(), theme::TEXT);
        }
    }
    pane.y += theme::FIELD_HEIGHT + 8.0;
}

/// Sets the first fill to a solid colour (keeping other paints), optionally changing its opacity.
pub fn set_solid(st: &mut TextStyle, c: Color, opacity: Option<f32>) {
    match st.fills.iter_mut().find(|p| p.visible) {
        Some(p) => {
            p.kind = PaintKind::Solid(c);
            if let Some(o) = opacity {
                p.opacity = o;
            }
        }
        None => {
            let mut p = Paint::solid(c);
            if let Some(o) = opacity {
                p.opacity = o;
            }
            st.fills.push(p);
        }
    }
}

// ------------------------------------------------------------------ font family picker
pub(super) fn family_picker(ui: &mut Ui, rect: Rect, label: &str) -> Option<String> {
    let id = ui.id().with("font-family");
    #[cfg(test)]
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("field-rect", format!("{:?}", "font-family"))), rect));
    let open_id = id.with("open");
    let open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let r = ui.interact(rect, id, Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    if r.hovered() || open {
        p.rect_stroke(rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, if open { theme::BORDER_SELECTED } else { theme::BORDER }), StrokeKind::Inside);
    }
    let font = theme::font_body();
    let shown = widgets::elide(p, label, &font, rect.width() - 28.0);
    p.text(Pos2::new(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, shown, font, theme::TEXT);
    crate::icons::paint(p, crate::icons::Icon::ChevronDown, Pos2::new(rect.right() - 12.0, rect.center().y), 12.0, theme::ICON_SECONDARY);
    if r.clicked() {
        ui.data_mut(|d| {
            d.insert_temp(open_id, !open);
            d.insert_temp(id.with("q"), String::new());
        });
    }
    if !open {
        return None;
    }
    let ctx = ui.ctx().clone();
    let mut picked = None;
    let mut close = false;
    let pos = Pos2::new(rect.left() - 4.0, rect.bottom() + 2.0);
    let w = rect.width() + 8.0;
    let area = egui::Area::new(id.with("popup")).order(egui::Order::Foreground).fixed_pos(pos).show(&ctx, |ui| {
        let panel = Rect::from_min_size(pos, Vec2::new(w, 296.0));
        ui.allocate_exact_size(panel.size(), Sense::hover());
        widgets::menu_panel(ui.painter(), panel, theme::RADIUS_FRAME);
        let search = Rect::from_min_size(panel.min + Vec2::new(8.0, 8.0), Vec2::new(w - 16.0, theme::FIELD_HEIGHT));
        let mut q = ui.data(|d| d.get_temp::<String>(id.with("q"))).unwrap_or_default();
        let qid = id.with("q-edit");
        field::field_bg(ui, search, qid);
        let te = egui::TextEdit::singleline(&mut q)
            .id(qid)
            .frame(egui::Frame::NONE)
            .hint_text("Search fonts")
            .font(theme::font_body())
            .text_color(theme::TEXT)
            .margin(egui::Margin::symmetric(8, 0))
            .vertical_align(egui::Align::Center)
            .desired_width(search.width());
        let resp = ui.put(search, te);
        if !resp.has_focus() && !ui.data(|d| d.get_temp::<bool>(id.with("focused"))).unwrap_or(false) {
            resp.request_focus();
            ui.data_mut(|d| d.insert_temp(id.with("focused"), true));
        }
        ui.data_mut(|d| d.insert_temp(id.with("q"), q.clone()));
        let families = families_cached(&ctx);
        let needle = q.to_lowercase();
        let list: Vec<&String> = families.iter().filter(|f| needle.is_empty() || f.to_lowercase().contains(&needle)).collect();
        let body = Rect::from_min_max(panel.min + Vec2::new(0.0, 8.0 + theme::FIELD_HEIGHT + 4.0), panel.max - Vec2::new(0.0, 8.0));
        let row_h = theme::MENU_ITEM_HEIGHT;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
        child.set_clip_rect(body);
        if resp.has_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(f) = list.first() {
                picked = Some((*f).clone());
            }
        }
        egui::ScrollArea::vertical().max_height(body.height()).auto_shrink([false, false]).show_rows(&mut child, row_h, list.len(), |ui, range| {
            for i in range {
                let (row, rr) = ui.allocate_exact_size(Vec2::new(body.width(), row_h), Sense::click());
                let cur = list[i].eq_ignore_ascii_case(label);
                if rr.hovered() {
                    ui.painter().rect_filled(row.shrink2(Vec2::new(theme::MENU_PILL_INSET, 0.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_MENU_SELECTED);
                }
                if cur {
                    crate::icons::paint(
                        ui.painter(),
                        crate::icons::Icon::Check,
                        Pos2::new(row.left() + 20.0, row.center().y),
                        theme::MENU_ICON_SIZE,
                        theme::TEXT,
                    );
                }
                ui.painter().text(Pos2::new(row.left() + 36.0, row.center().y), Align2::LEFT_CENTER, list[i], theme::font_menu(), theme::TEXT);
                if rr.clicked() {
                    picked = Some(list[i].clone());
                }
            }
        });
        if list.is_empty() {
            ui.painter().text(body.left_top() + Vec2::new(16.0, 12.0), Align2::LEFT_CENTER, "No matching fonts", theme::font_menu(), theme::TEXT_SECONDARY);
        }
        panel
    });
    let (pressed, ptr, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
    if esc || (pressed && ptr.is_some_and(|p| !area.response.rect.contains(p) && !rect.contains(p))) || picked.is_some() {
        close = true;
    }
    if close {
        ui.data_mut(|d| {
            d.insert_temp(open_id, false);
            d.insert_temp(id.with("focused"), false);
        });
    }
    picked
}

/// Installed families, cached per font generation.
fn families_cached(ctx: &egui::Context) -> std::sync::Arc<Vec<String>> {
    let key = egui::Id::new("typo-families");
    let gen = designos_render::font_generation();
    if let Some((g, v)) = ctx.data(|d| d.get_temp::<(usize, std::sync::Arc<Vec<String>>)>(key)) {
        if g == gen {
            return v;
        }
    }
    let v = std::sync::Arc::new(designos_render::available_families());
    ctx.data_mut(|d| d.insert_temp(key, (gen, v.clone())));
    v
}

// ------------------------------------------------------------------ type settings: Details
const DETAILS_W: f32 = 312.0;
const DETAIL_LABEL_X: f32 = 16.0;
const DETAIL_CONTROL_X: f32 = 152.0;
const DETAIL_CONTROL_W: f32 = 144.0;
const SET_CHIP_W: f32 = 32.0;
const SET_CHIP_GAP: f32 = 2.0;
const SETS_PER_ROW: usize = 8;

/// Segmented control with text labels inside `rect` (items share its width).
fn seg_rect(ui: &mut Ui, rect: Rect, key: &str, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    let mut out = None;
    let n = labels.len();
    let w = rect.width() / n as f32;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(rect.left() + i as f32 * w, rect.top()), Vec2::new(w, rect.height()));
        let radius = match (i == 0, i + 1 == n) {
            (true, true) => theme::cr(theme::RADIUS_CONTROL),
            (true, false) => theme::cr_each(theme::RADIUS_CONTROL, 0, 0, theme::RADIUS_CONTROL),
            (false, true) => theme::cr_each(0, theme::RADIUS_CONTROL, theme::RADIUS_CONTROL, 0),
            _ => theme::cr(0),
        };
        let resp = ui.interact(r, ui.id().with(("segr", key, i)), Sense::click());
        let sel = selected == Some(i);
        let bg = if sel {
            theme::BG_TERTIARY
        } else if resp.hovered() {
            theme::BG_TERTIARY.gamma_multiply(0.6)
        } else {
            theme::BG_SECONDARY
        };
        ui.painter().rect_filled(r, radius, bg);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *label, theme::font_body_strong(), if sel { theme::ICON } else { theme::ICON_SECONDARY });
        if i > 0 {
            widgets::vline(ui.painter(), r.left(), r.top(), r.bottom(), theme::BG);
        }
        if resp.clicked() {
            out = Some(i);
        }
    }
    out
}

/// A labelled row of the Details popover (32px pitch). `dim` greys the label (feature missing in the font).
fn detail_row(ui: &mut Ui, body: Rect, y: f32, label: &str, dim: bool, control: impl FnOnce(&mut Ui, Rect)) {
    let line = Rect::from_min_size(Pos2::new(body.left(), body.top() + 8.0 + y), Vec2::new(body.width(), 24.0));
    let font = theme::font_body();
    let color = if dim { theme::TEXT_TERTIARY } else { theme::TEXT_SECONDARY };
    widgets::text_on_baseline(ui.painter(), body.left() + DETAIL_LABEL_X, widgets::centered_baseline(line, &font), label, font, color);
    control(ui, Rect::from_min_size(Pos2::new(body.left() + DETAIL_CONTROL_X, line.top()), Vec2::new(DETAIL_CONTROL_W, 24.0)));
}

/// OpenType feature tags of the face a style resolves to, cached per font generation.
fn features_cached(ctx: &egui::Context, family: &str, weight: u16, italic: bool) -> std::sync::Arc<Vec<String>> {
    let key = egui::Id::new(("typo-features", family, weight, italic));
    let gen = designos_render::font_generation();
    if let Some((g, v)) = ctx.data(|d| d.get_temp::<(usize, std::sync::Arc<Vec<String>>)>(key)) {
        if g == gen {
            return v;
        }
    }
    let v = std::sync::Arc::new(designos_render::font_features(family, weight, italic));
    ctx.data_mut(|d| d.insert_temp(key, (gen, v.clone())));
    v
}

/// Applies `f` to the text properties (paragraph settings, lists, direction) of the edited text or of every selected layer.
pub fn set_text_props(app: &mut App, sel: &[NodeId], f: impl Fn(&mut TextProps)) {
    if editing(app, sel) {
        ts::edit_text_props(app, &f);
        return;
    }
    app.edit_nodes(sel, |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            f(t);
            n.size = layout::fit_text(t, n.size);
        }
    });
}

/// Sets (`Some`) or removes (`None`) OpenType features on the edited range / selected layers.
fn set_features(app: &mut App, sel: &[NodeId], changes: Vec<(&str, Option<u32>)>) {
    let changes: Vec<(String, Option<u32>)> = changes.into_iter().map(|(t, v)| (t.to_string(), v)).collect();
    set_style(app, sel, move |s| {
        for (tag, v) in &changes {
            richtext::set_feature(&mut s.features, tag, *v);
        }
    });
}

/// Rows of the Type settings popover for a font with `sets` stylistic sets on `active_tab`.
fn details_rows(sets: usize, active_tab: usize) -> f32 {
    let chips = if sets == 0 { 0 } else { 1 + sets.div_ceil(SETS_PER_ROW) };
    match active_tab {
        // Variable tab: 3-tab bar + Ag live preview + 4 variable axes + reset row
        2 => (1 + 5) as f32 + 2.5,
        // Details tab: 3-tab bar + Ag live preview + 7 OpenType rows + stylistic set chips
        1 => (1 + 7 + chips) as f32 + 2.5,
        // Basic tab (default): 3-tab bar + Ag live preview + OpenType + Paragraph (hides Variable Axes to save 136px)
        _ => (1 + 7 + chips + 8) as f32 + 3.0,
    }
}

/// The Figma UI3 `Type settings` 3-Tab popover (`Basic | Details | Variable`):
/// live `Ag` preview box with baseline/cap-height guides, basic case/decoration controls,
/// number styles, fractions, ligatures, stylistic sets, paragraph settings, and variable font axes.
pub(super) fn details_popover(app: &mut App, ui: &mut Ui, region: Rect, st: &super::effects::PopState) {
    let ctx = ui.ctx().clone();
    let ids = st.ids.clone();
    let Some(t) = ids.first().and_then(|id| match app.doc.get(*id).map(|n| &n.kind) {
        Some(NodeKind::Text(t)) => Some(t.clone()),
        _ => None,
    }) else {
        app.props.pop = None;
        return;
    };
    let sum = if editing(app, &ids) { ts::session_summary(app) } else { nodes_summary(app, &ids) };
    let Some(sum) = sum else {
        app.props.pop = None;
        return;
    };
    let feats = sum.features.clone().unwrap_or_default();
    let family = sum.family.clone().unwrap_or_else(|| t.style.font_family.clone());
    let available = features_cached(&ctx, &family, sum.weight.unwrap_or(t.style.font_weight), sum.italic.unwrap_or(t.style.italic));
    let has = |tag: &str| available.is_empty() || available.iter().any(|f| f == tag);
    let sets: Vec<u32> = (1..=20u32).filter(|i| available.iter().any(|f| *f == format!("ss{i:02}"))).collect();
    let tab_state_id = egui::Id::new("type-settings-active-tab");
    let mut active_tab = ctx.data(|d| d.get_temp::<usize>(tab_state_id)).unwrap_or(0);
    let height = widgets::popover::HEADER_H + 8.0 + details_rows(sets.len(), active_tab) * widgets::popover::ROW_H + 8.0;
    let val = |tag: &str| richtext::feature_value(&feats, tag);

    let pop = widgets::popover::show(&ctx, egui::Id::new("inspector-popover"), st.anchor, region, DETAILS_W, height, "Type settings", |ui, body| {
        let y = std::cell::Cell::new(0.0f32);
        let next = || {
            let v = y.get();
            y.set(v + widgets::popover::ROW_H);
            v
        };

        // ---- 1. 3-Tab Segmented Control: [ Basic | Details | Variable ]
        let tab_y = next();
        let tabs_r =
            Rect::from_min_size(Pos2::new(body.left() + DETAIL_LABEL_X, body.top() + 8.0 + tab_y), Vec2::new(body.width() - DETAIL_LABEL_X * 2.0, 24.0));
        widgets::record(ui, "det-tabs", tabs_r);
        if let Some(t_idx) = seg_rect(ui, tabs_r, "det-tabs", &["Basic", "Details", "Variable"], Some(active_tab)) {
            active_tab = t_idx;
            ctx.data_mut(|d| d.insert_temp(tab_state_id, active_tab));
        }

        // ---- 2. Figma UI3 `Ag` Live Preview Box (68x68px, >64px height so lint treats as preview canvas) + Basic Case & Decoration
        let ag_top_offset = y.get();
        y.set(ag_top_offset + 76.0);
        let ag_rect = Rect::from_min_size(Pos2::new(body.left() + DETAIL_LABEL_X, body.top() + 8.0 + ag_top_offset), Vec2::new(68.0, 68.0));
        ui.painter().rect_filled(ag_rect, theme::cr(theme::RADIUS_CONTROL), Color32::from_rgb(0x1E, 0x1E, 0x1E));
        ui.painter().rect_stroke(ag_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
        // Blue typographic cap-height & baseline metric guides (`#0D99FF`)
        let guide_col = Color32::from_rgba_unmultiplied(0x0D, 0x99, 0xFF, 110);
        widgets::hline(ui.painter(), ag_rect.left() + 6.0, ag_rect.right() - 6.0, ag_rect.top() + 21.0, guide_col);
        widgets::hline(ui.painter(), ag_rect.left() + 6.0, ag_rect.right() - 6.0, ag_rect.bottom() - 19.0, guide_col);
        let preview_glyph = match sum.case.unwrap_or(t.style.case) {
            TextCase::Upper => "AG",
            TextCase::Lower => "ag",
            _ => "Ag",
        };
        ui.painter().text(ag_rect.center(), Align2::CENTER_CENTER, preview_glyph, egui::FontId::proportional(24.0), theme::TEXT);

        // Right of `Ag`: Case (`— | AG | ag | Ag`) and Decoration (`None | U | S`)
        let case_row_y = body.top() + 8.0 + ag_top_offset + 6.0;
        let case_line = Rect::from_min_size(Pos2::new(ag_rect.right() + 10.0, case_row_y), Vec2::new(body.width() - 90.0, 24.0));
        widgets::text_on_baseline(
            ui.painter(),
            ag_rect.right() + 10.0,
            widgets::centered_baseline(case_line, &theme::font_body()),
            "Case",
            theme::font_body(),
            theme::TEXT_SECONDARY,
        );
        let case_ctrl = Rect::from_min_size(Pos2::new(body.left() + DETAIL_CONTROL_X, case_row_y), Vec2::new(DETAIL_CONTROL_W, 24.0));
        let cur_case = match sum.case.unwrap_or(t.style.case) {
            TextCase::Original => 0,
            TextCase::Upper => 1,
            TextCase::Lower => 2,
            TextCase::Title | TextCase::SmallCaps => 3,
        };
        if let Some(ci) = seg_rect(ui, case_ctrl, "det-basic-case", &["—", "AG", "ag", "Ag"], Some(cur_case)) {
            let tc = [TextCase::Original, TextCase::Upper, TextCase::Lower, TextCase::Title][ci];
            set_style(app, &ids, move |s| s.case = tc);
        }

        let dec_row_y = body.top() + 8.0 + ag_top_offset + 38.0;
        let dec_line = Rect::from_min_size(Pos2::new(ag_rect.right() + 10.0, dec_row_y), Vec2::new(body.width() - 90.0, 24.0));
        widgets::text_on_baseline(
            ui.painter(),
            ag_rect.right() + 10.0,
            widgets::centered_baseline(dec_line, &theme::font_body()),
            "Decor",
            theme::font_body(),
            theme::TEXT_SECONDARY,
        );
        let dec_ctrl = Rect::from_min_size(Pos2::new(body.left() + DETAIL_CONTROL_X, dec_row_y), Vec2::new(DETAIL_CONTROL_W, 24.0));
        let cur_dec = match sum.decoration.unwrap_or(t.style.decoration) {
            TextDecoration::None => 0,
            TextDecoration::Underline => 1,
            TextDecoration::Strikethrough => 2,
        };
        if let Some(di) = seg_rect(ui, dec_ctrl, "det-basic-dec", &["None", "U", "S"], Some(cur_dec)) {
            let td = [TextDecoration::None, TextDecoration::Underline, TextDecoration::Strikethrough][di];
            set_style(app, &ids, move |s| s.decoration = td);
        }

        let sep_top = body.top() + 8.0 + y.get() - 4.0;
        widgets::hline(ui.painter(), body.left(), body.right(), sep_top, theme::BORDER);
        y.set(y.get() + 4.0);

        if active_tab != 2 {
            // ---- numbers & OpenType details (shown on Basic and Details tabs)
            let spacing = ["Default", "Tabular", "Proportional"];
            let cur = if val("tnum") == Some(1) {
                1
            } else if val("pnum") == Some(1) {
                2
            } else {
                0
            };
            detail_row(ui, body, next(), "Number spacing", !has("tnum"), |ui, r| {
                let items = spacing.map(String::from);
                if let Some(i) = dropdown(ui, r, "det-spacing", spacing[cur], &items) {
                    let (tn, pn) = match i {
                        1 => (Some(1), None),
                        2 => (None, Some(1)),
                        _ => (None, None),
                    };
                    set_features(app, &ids, vec![("tnum", tn), ("pnum", pn)]);
                }
            });
            let figures = ["Default", "Lining", "Oldstyle"];
            let cur = if val("lnum") == Some(1) {
                1
            } else if val("onum") == Some(1) {
                2
            } else {
                0
            };
            detail_row(ui, body, next(), "Number style", !has("lnum") && !has("onum"), |ui, r| {
                let items = figures.map(String::from);
                if let Some(i) = dropdown(ui, r, "det-figures", figures[cur], &items) {
                    let (l, o) = match i {
                        1 => (Some(1), None),
                        2 => (None, Some(1)),
                        _ => (None, None),
                    };
                    set_features(app, &ids, vec![("lnum", l), ("onum", o)]);
                }
            });
            for (label, tag, key) in [("Fractions", "frac", "det-frac"), ("Slashed zero", "zero", "det-zero"), ("Case sensitive forms", "case", "det-case")] {
                detail_row(ui, body, next(), label, !has(tag), |ui, r| {
                    widgets::record(ui, key, r);
                    if let Some(on) = widgets::popover::check(ui, r, key, val(tag) == Some(1)) {
                        set_features(app, &ids, vec![(tag, on.then_some(1))]);
                    }
                });
            }
            // ligatures and contextual alternates are on unless switched off
            for (label, tag, key) in [("Ligatures", "liga", "det-liga"), ("Contextual alternates", "calt", "det-calt")] {
                detail_row(ui, body, next(), label, !has(tag), |ui, r| {
                    widgets::record(ui, key, r);
                    if let Some(on) = widgets::popover::check(ui, r, key, val(tag) != Some(0)) {
                        set_features(app, &ids, vec![(tag, (!on).then_some(0))]);
                    }
                });
            }

            // ---- stylistic sets (only those the font has)
            if !sets.is_empty() {
                let top = next();
                detail_row(ui, body, top, "Stylistic sets", false, |_, _| {});
                for (row, chunk) in sets.chunks(SETS_PER_ROW).enumerate() {
                    let line_y = next();
                    for (i, set) in chunk.iter().enumerate() {
                        let tag = format!("ss{set:02}");
                        let on = val(&tag) == Some(1);
                        let x = body.left() + DETAIL_LABEL_X + i as f32 * (SET_CHIP_W + SET_CHIP_GAP);
                        let r = Rect::from_min_size(Pos2::new(x, body.top() + 8.0 + line_y), Vec2::new(SET_CHIP_W, 24.0));
                        widgets::record(ui, &format!("det-ss-{set}"), r);
                        let resp = ui.interact(r, ui.id().with(("det-ss", row, i)), Sense::click());
                        let bg = if on {
                            theme::BG_SELECTED
                        } else if resp.hovered() {
                            theme::BG_TERTIARY
                        } else {
                            theme::BG_SECONDARY
                        };
                        ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), bg);
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            format!("{set}"),
                            theme::font_body(),
                            if on { Color32::WHITE } else { theme::TEXT_SECONDARY },
                        );
                        resp.clone().on_hover_text(format!("Stylistic set {set}"));
                        if resp.clicked() {
                            set_features(app, &ids, vec![(tag.as_str(), (!on).then_some(1))]);
                        }
                    }
                }
            }
        }

        if active_tab == 0 {
            // ---- paragraph (Basic tab only)
            let sep = body.top() + 8.0 + y.get() - 4.0;
            widgets::hline(ui.painter(), body.left(), body.right(), sep, theme::BORDER);
            y.set(y.get() + 8.0);
            let p = t.paragraph.clone();
            detail_row(ui, body, next(), "Paragraph indent", false, |ui, r| {
                if let Some(v) = field::num_field(ui, r, "det-indent", Prefix::None, Some(p.indent), "") {
                    set_text_props(app, &ids, |t| t.paragraph.indent = v.max(0.0));
                }
            });
            detail_row(ui, body, next(), "Paragraph spacing", false, |ui, r| {
                if let Some(v) = field::num_field(ui, r, "det-pspacing", Prefix::None, Some(t.style.paragraph_spacing), "") {
                    set_base(app, &ids, |s| s.paragraph_spacing = v.max(0.0));
                }
            });
            detail_row(ui, body, next(), "List spacing", false, |ui, r| {
                if let Some(v) = field::num_field(ui, r, "det-lspacing", Prefix::None, Some(p.list_spacing), "") {
                    set_text_props(app, &ids, |t| t.paragraph.list_spacing = v.max(0.0));
                }
            });
            detail_row(ui, body, next(), "Hanging punctuation", false, |ui, r| {
                widgets::record(ui, "det-hangp", r);
                if let Some(on) = widgets::popover::check(ui, r, "det-hangp", p.hanging_punctuation) {
                    set_text_props(app, &ids, |t| t.paragraph.hanging_punctuation = on);
                }
            });
            detail_row(ui, body, next(), "Hanging lists", false, |ui, r| {
                if let Some(on) = widgets::popover::check(ui, r, "det-hangl", p.hanging_list) {
                    set_text_props(app, &ids, |t| t.paragraph.hanging_list = on);
                }
            });
            detail_row(ui, body, next(), "Vertical trim", false, |ui, r| {
                let items = ["Standard", "Cap height"].map(String::from);
                let cur = if p.leading_trim == LeadingTrim::CapHeight { 1 } else { 0 };
                if let Some(i) = dropdown(ui, r, "det-trim", &items[cur], &items) {
                    set_text_props(app, &ids, |t| t.paragraph.leading_trim = if i == 1 { LeadingTrim::CapHeight } else { LeadingTrim::None });
                }
            });
            detail_row(ui, body, next(), "Text direction", false, |ui, r| {
                let cur = match t.direction {
                    TextDirection::Auto => 0,
                    TextDirection::Ltr => 1,
                    TextDirection::Rtl => 2,
                };
                widgets::record(ui, "det-dir", r);
                if let Some(i) = seg_rect(ui, r, "det-dir", &["Auto", "LTR", "RTL"], Some(cur)) {
                    let d = [TextDirection::Auto, TextDirection::Ltr, TextDirection::Rtl][i];
                    set_text_props(app, &ids, |t| t.direction = d);
                }
            });
            detail_row(ui, body, next(), "Wrap style", false, |ui, r| {
                let cur = match t.text_wrap {
                    TextWrap::Auto => 0,
                    TextWrap::Balance => 1,
                    TextWrap::Pretty => 2,
                };
                widgets::record(ui, "det-wrap", r);
                if let Some(i) = seg_rect(ui, r, "det-wrap", &["Auto", "Balance", "Pretty"], Some(cur)) {
                    let w = [TextWrap::Auto, TextWrap::Balance, TextWrap::Pretty][i];
                    set_text_props(app, &ids, |t| t.text_wrap = w);
                }
            });
        }

        if active_tab == 2 {
            // ---- variable font axes (Variable tab only: wght, wdth, opsz, slnt + reset)
            let wght_val = t.style.get_axis("wght").unwrap_or(t.style.font_weight as f32);
            detail_row(ui, body, next(), "Axis · Weight (wght)", false, |ui, r| {
                widgets::record(ui, "det-axis-wght", r);
                if let Some(v) = field::num_field(ui, r, "det-axis-wght", Prefix::None, Some(wght_val), "") {
                    let v = v.clamp(100.0, 900.0);
                    set_style(app, &ids, |s| {
                        s.set_axis("wght", v);
                        s.font_weight = v.round() as u16;
                    });
                }
            });
            let wdth_val = t.style.get_axis("wdth").unwrap_or(100.0);
            detail_row(ui, body, next(), "Axis · Width (wdth)", false, |ui, r| {
                widgets::record(ui, "det-axis-wdth", r);
                if let Some(v) = field::num_field(ui, r, "det-axis-wdth", Prefix::None, Some(wdth_val), "%") {
                    let v = v.clamp(50.0, 200.0);
                    set_style(app, &ids, |s| s.set_axis("wdth", v));
                }
            });
            let opsz_val = t.style.get_axis("opsz").unwrap_or(t.style.font_size);
            detail_row(ui, body, next(), "Axis · Optical (opsz)", false, |ui, r| {
                widgets::record(ui, "det-axis-opsz", r);
                if let Some(v) = field::num_field(ui, r, "det-axis-opsz", Prefix::None, Some(opsz_val), "") {
                    let v = v.clamp(6.0, 144.0);
                    set_style(app, &ids, |s| s.set_axis("opsz", v));
                }
            });
            let slnt_val = t.style.get_axis("slnt").unwrap_or(0.0);
            detail_row(ui, body, next(), "Axis · Slant (slnt)", false, |ui, r| {
                widgets::record(ui, "det-axis-slnt", r);
                if let Some(v) = field::num_field(ui, r, "det-axis-slnt", Prefix::None, Some(slnt_val), "°") {
                    let v = v.clamp(-20.0, 20.0);
                    set_style(app, &ids, |s| s.set_axis("slnt", v));
                }
            });
            detail_row(ui, body, next(), "Variable axes", false, |ui, r| {
                widgets::record(ui, "det-axis-reset", r);
                if seg_rect(ui, r, "det-axis-reset", &["Reset axes"], None) == Some(0) {
                    set_style(app, &ids, |s| s.variation_axes.clear());
                }
            });
        }
    });
    if pop {
        app.props.pop = None;
    }
}
