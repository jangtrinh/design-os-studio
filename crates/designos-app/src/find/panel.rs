//! The Find and replace panel: it takes the place of the layers tree in the left sidebar.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::panels::layers::kind_icon;
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::widgets::{self, field};
use designos_doc::find::MatchField;
use designos_doc::{Document, NodeId, NodeKind};
use egui::text::{CCursor, CCursorRange};
use egui::{Align, Align2, Key, Margin, Modifiers, Pos2, Rect, Sense, Stroke, TextEdit, Ui, Vec2};
use std::ops::Range;

/// A result line: a bit of the matched text with the match marked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Excerpt {
    pub text: String,
    /// Byte range of the match inside `text`.
    pub hl: Range<usize>,
}

const EXCERPT_TAIL_CHARS: usize = 120;

/// The line of `s` holding the match `r`, trimmed to `lead` chars before it
/// (with an ellipsis) and a bounded tail after it.
pub fn excerpt(s: &str, r: &Range<usize>, lead: usize) -> Excerpt {
    let line_start = s[..r.start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = s[r.start..].find('\n').map_or(s.len(), |i| r.start + i);
    let end = r.end.min(line_end);
    let before = s[line_start..r.start].chars().count();
    let (begin, prefix) = if before > lead {
        let skip = before - lead;
        (s[line_start..].char_indices().nth(skip).map_or(line_start, |(i, _)| line_start + i), "…")
    } else {
        (line_start, "")
    };
    let mut tail_end = line_end;
    let mut suffix = "";
    if let Some((i, _)) = s[begin..line_end].char_indices().nth(EXCERPT_TAIL_CHARS) {
        tail_end = (begin + i).max(end);
        suffix = if tail_end < line_end { "…" } else { "" };
    }
    let text = format!("{prefix}{}{suffix}", &s[begin..tail_end]);
    let hl_start = prefix.len() + (r.start - begin);
    Excerpt { text, hl: hl_start..hl_start + (end - r.start) }
}

enum RowKind {
    Page(NodeId),
    Group(NodeId),
    /// A result, and whether it is listed under a frame row.
    Hit(usize, bool),
}

struct ListRow {
    kind: RowKind,
    top: f32,
    h: f32,
}

/// The ancestor of `id` that sits directly under its page, unless `id` is itself top level.
fn top_container(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = id;
    loop {
        let parent = doc.get(cur)?.parent?;
        if doc.get(parent).is_some_and(|p| matches!(p.kind, NodeKind::Page { .. })) {
            return (cur != id).then_some(cur);
        }
        cur = parent;
    }
}

fn build_rows(app: &App) -> Vec<ListRow> {
    let mut rows = Vec::new();
    let mut y = 0.0;
    let mut last_page = None;
    let mut last_group: Option<Option<NodeId>> = None;
    let mut push = |rows: &mut Vec<ListRow>, kind: RowKind, h: f32| {
        rows.push(ListRow { kind, top: y, h });
        y += h;
    };
    for (i, m) in app.find.results.iter().enumerate() {
        let page = app.doc.page_of(m.node);
        if app.find.search_in == crate::find::SearchIn::AllPages && page != last_page {
            if let Some(p) = page {
                push(&mut rows, RowKind::Page(p), theme::FIND_GROUP_ROW_H);
            }
            last_page = page;
            last_group = None;
        }
        let group = top_container(&app.doc, m.node);
        if last_group != Some(group) {
            if let Some(g) = group {
                push(&mut rows, RowKind::Group(g), theme::FIND_GROUP_ROW_H);
            }
            last_group = Some(group);
        }
        push(&mut rows, RowKind::Hit(i, group.is_some()), theme::LAYER_ROW_HEIGHT);
    }
    rows
}

fn text_button(ui: &mut Ui, rect: Rect, key: &str, label: &str, enabled: bool) -> bool {
    let r = ui.interact(rect, ui.id().with(("find-btn", key)), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    let fill = if enabled && r.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY };
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), fill);
    p.text(rect.center(), Align2::CENTER_CENTER, label, theme::font_body(), if enabled { theme::TEXT } else { theme::TEXT_DISABLED });
    widgets::record(ui, &format!("find:{key}"), rect);
    enabled && r.clicked()
}

fn toggle(ui: &mut Ui, rect: Rect, key: &str, label: &str, tip: &str, on: bool) -> bool {
    let r = ui.interact(rect, ui.id().with(("find-toggle", key)), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, theme::cr(theme::RADIUS_MARK), theme::BG_SELECTED);
    } else if r.hovered() {
        p.rect_filled(rect, theme::cr(theme::RADIUS_MARK), theme::BG_TRANSPARENT_HOVER);
    }
    let col = if on {
        theme::ICON_SELECTED
    } else if r.hovered() {
        theme::ICON
    } else {
        theme::ICON_SECONDARY
    };
    p.text(rect.center(), Align2::CENTER_CENTER, label, theme::font_body_strong(), col);
    widgets::record(ui, &format!("find:{key}"), rect);
    r.on_hover_text(tip).clicked()
}

fn chevron(ui: &mut Ui, rect: Rect, key: &str, up: bool, enabled: bool, tip: &str) -> bool {
    let r = ui.interact(rect, ui.id().with(("find-step", key)), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled && r.hovered() {
        p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_TRANSPARENT_HOVER);
    }
    let col = if !enabled {
        theme::ICON_TERTIARY
    } else if r.hovered() {
        theme::ICON
    } else {
        theme::ICON_SECONDARY
    };
    let c = rect.center();
    let (w, h) = (4.5, 2.5);
    let (a, b) = if up { (c.y + h, c.y - h) } else { (c.y - h, c.y + h) };
    let stroke = Stroke::new(1.5, col);
    p.line_segment([Pos2::new(c.x - w, a), Pos2::new(c.x, b)], stroke);
    p.line_segment([Pos2::new(c.x, b), Pos2::new(c.x + w, a)], stroke);
    widgets::record(ui, &format!("find:{key}"), rect);
    enabled && r.on_hover_text(tip).clicked()
}

/// A live single-line input (the text changes as you type, unlike `field::text_input`).
fn input(ui: &mut Ui, rect: Rect, id: egui::Id, text: &mut String, hint: &str, focus: bool) -> egui::Response {
    field::field_bg(ui, rect, id);
    if focus {
        ui.memory_mut(|m| m.request_focus(id));
        let mut st = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(text.chars().count()))));
        st.store(ui.ctx(), id);
    }
    let font = theme::font_body();
    let te = TextEdit::singleline(text)
        .id(id)
        .font(font.clone())
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .margin(Margin::symmetric(theme::FIELD_PLAIN_X as i8, 0))
        .vertical_align(Align::Center)
        .hint_text(egui::RichText::new(hint).font(font).color(theme::TEXT_TERTIARY))
        .desired_width(rect.width());
    ui.put(rect, te)
}

pub fn show(app: &mut App, ui: &mut Ui, region: Rect) {
    app.find_refresh(false);
    let p = ui.painter().clone();
    let (x0, x1) = (region.left(), region.right());
    let mut y = region.top();

    // ---- title
    let title = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(region.width(), theme::LAYER_HEADER_HEIGHT));
    p.text(Pos2::new(x0 + theme::PANEL_PADDING_X, title.center().y), Align2::LEFT_CENTER, "Find and replace", theme::font_body_strong(), theme::TEXT);
    let close =
        Rect::from_min_size(Pos2::new(x1 - theme::PANEL_ACTION_RIGHT_PAD - theme::PANEL_ACTION_BUTTON, y + 8.0), Vec2::splat(theme::PANEL_ACTION_BUTTON));
    if widgets::icon_button(ui, close, "find-close", Icon::Close, false, "Close (Esc)").clicked() {
        app.close_find();
        return;
    }
    y += theme::LAYER_HEADER_HEIGHT;

    // ---- search row: replace toggle, field, option toggles
    // Every field starts on the panel inset; the replace toggle sits beside the close button.
    let field_x = x0 + theme::INSET_PANEL;
    let fr = Rect::from_min_max(Pos2::new(field_x, y), Pos2::new(x1 - theme::INSET_PANEL, y + theme::FIND_FIELD_H));
    let tg =
        Rect::from_min_size(Pos2::new(close.left() - theme::SPACE_FACT - theme::PANEL_ACTION_BUTTON, close.top()), Vec2::splat(theme::PANEL_ACTION_BUTTON));
    let chev = if app.find.show_replace { Icon::ChevronDown } else { Icon::ChevronRight };
    if widgets::icon_button(ui, tg, "find-replace-toggle", chev, false, "Toggle replace").clicked() {
        app.find.show_replace = !app.find.show_replace;
    }
    widgets::record(ui, "find:replace-toggle", tg);
    let sid = ui.id().with("find-search");
    let focus = std::mem::take(&mut app.find.focus);
    let focused = ui.memory(|m| m.has_focus(sid));
    let (mut esc, mut next, mut prev) = (false, false, false);
    if focused {
        ui.input_mut(|i| {
            esc = i.consume_key(Modifiers::NONE, Key::Escape);
            prev = i.consume_key(Modifiers::SHIFT, Key::Enter);
            next = !prev && i.consume_key(Modifiers::NONE, Key::Enter);
        });
    }
    let toggles_w = theme::FIND_TOGGLE_SIZE * 3.0 + 4.0;
    let text_rect = Rect::from_min_max(fr.min, Pos2::new(fr.right() - toggles_w, fr.bottom()));
    input(ui, text_rect, sid, &mut app.find.query, "Find", focus);
    widgets::record(ui, "find:search", fr);
    let ty = fr.center().y - theme::FIND_TOGGLE_SIZE / 2.0;
    let tx = |k: f32| Pos2::new(fr.right() - 2.0 - theme::FIND_TOGGLE_SIZE * (3.0 - k), ty);
    let tsz = Vec2::splat(theme::FIND_TOGGLE_SIZE);
    if toggle(ui, Rect::from_min_size(tx(0.0), tsz), "case", "Aa", "Match case", app.find.opts.match_case) {
        app.find.opts.match_case = !app.find.opts.match_case;
    }
    if toggle(ui, Rect::from_min_size(tx(1.0), tsz), "word", "ab", "Match whole word", app.find.opts.whole_word) {
        app.find.opts.whole_word = !app.find.opts.whole_word;
    }
    if toggle(ui, Rect::from_min_size(tx(2.0), tsz), "regex", ".*", "Use regular expression", app.find.opts.regex) {
        app.find.opts.regex = !app.find.opts.regex;
    }
    y += theme::FIND_FIELD_H + theme::FIND_ROW_GAP;

    // ---- replace row and buttons
    let mut replace_one = false;
    let mut replace_all = false;
    if app.find.show_replace {
        let rr = Rect::from_min_max(Pos2::new(field_x, y), Pos2::new(x1 - theme::INSET_PANEL, y + theme::FIND_FIELD_H));
        let rid = ui.id().with("find-replace");
        let rfocused = ui.memory(|m| m.has_focus(rid));
        let (mut resc, mut renter) = (false, false);
        if rfocused {
            ui.input_mut(|i| {
                resc = i.consume_key(Modifiers::NONE, Key::Escape);
                renter = i.consume_key(Modifiers::NONE, Key::Enter);
            });
        }
        input(ui, rr, rid, &mut app.find.replacement, "Replace with", false);
        widgets::record(ui, "find:replace", rr);
        esc |= resc;
        replace_one |= renter;
        y += theme::FIND_FIELD_H + theme::FIND_ROW_GAP;
        let bw = (rr.width() - theme::FIND_ROW_GAP) / 2.0;
        let has = !app.find.results.is_empty();
        let b1 = Rect::from_min_size(Pos2::new(rr.left(), y), Vec2::new(bw, theme::FIND_BUTTON_H));
        let b2 = Rect::from_min_size(Pos2::new(rr.left() + bw + theme::FIND_ROW_GAP, y), Vec2::new(bw, theme::FIND_BUTTON_H));
        replace_one |= text_button(ui, b1, "replace-one", "Replace", has);
        replace_all |= text_button(ui, b2, "replace-all", "Replace all", has);
        y += theme::FIND_BUTTON_H + theme::FIND_ROW_GAP;
    }

    // ---- status row: count, scope, previous / next
    let status = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(region.width(), theme::FIND_STATUS_H));
    let n = app.find.results.len();
    let (msg, col) = match (&app.find.error, app.find.query.is_empty(), n) {
        (Some(e), _, _) => (e.clone(), theme::DANGER),
        (None, true, _) => ("Type to search".to_string(), theme::TEXT_TERTIARY),
        (None, false, 0) => ("No results".to_string(), theme::TEXT_SECONDARY),
        (None, false, 1) => ("1 result".to_string(), theme::TEXT_SECONDARY),
        (None, false, n) => (format!("{n} results"), theme::TEXT_SECONDARY),
    };
    let nx = Rect::from_min_size(Pos2::new(x1 - 8.0 - 24.0, status.center().y - 12.0), Vec2::splat(24.0));
    let pv = nx.translate(Vec2::new(-24.0, 0.0));
    let font = theme::font_caption();
    let scope_label = app.find.search_in.label();
    let sw = widgets::text_width(&p, scope_label, &font) + 12.0;
    let chip = Rect::from_min_size(Pos2::new(pv.left() - 4.0 - sw, status.center().y - 11.0), Vec2::new(sw, 22.0));
    let msg_rect = Rect::from_min_max(Pos2::new(x0 + theme::PANEL_PADDING_X, status.top()), Pos2::new(chip.left() - 4.0, status.bottom()));
    clipped(ui, msg_rect, |ui| {
        ui.painter().text(Pos2::new(msg_rect.left(), status.center().y), Align2::LEFT_CENTER, &msg, font.clone(), col);
    });
    let cr = ui.interact(chip, ui.id().with("find-scope"), Sense::click());
    if cr.hovered() {
        p.rect_filled(chip, theme::cr(theme::RADIUS_MARK), theme::BG_TRANSPARENT_HOVER);
    }
    p.text(chip.center(), Align2::CENTER_CENTER, scope_label, font, if cr.hovered() { theme::TEXT } else { theme::TEXT_SECONDARY });
    widgets::record(ui, "find:scope", chip);
    if cr.on_hover_text("Where to search").clicked() {
        app.cycle_find_scope();
    }
    prev |= chevron(ui, pv, "prev", true, n > 0, "Previous result (Shift+Enter)");
    next |= chevron(ui, nx, "next", false, n > 0, "Next result (Enter)");
    y += theme::FIND_STATUS_H;
    widgets::hline(&p, x0, x1, y - 1.0, theme::BORDER);

    // ---- actions (after layout, so the results they change are not drawn stale)
    if esc {
        app.close_find();
        return;
    }
    if replace_all {
        app.find_replace_all();
    } else if replace_one {
        app.find_replace_one();
    } else if prev {
        app.find_step(-1);
    } else if next {
        app.find_step(1);
    }

    results(app, ui, Rect::from_min_max(Pos2::new(x0, y), region.max));
}

fn results(app: &mut App, ui: &mut Ui, list: Rect) {
    let rows = build_rows(app);
    let content_h = rows.last().map_or(0.0, |r| r.top + r.h) + 8.0;
    let mut scroll = app.find.scroll;
    wheel_scroll(ui, list, &mut scroll, content_h);
    // Keep the current result in view when stepping through them.
    if let Some(cur) = app.find.current.filter(|c| app.find.scrolled_to != Some(*c)) {
        if let Some(r) = rows.iter().find(|r| matches!(r.kind, RowKind::Hit(i, _) if i == cur)) {
            if r.top < scroll {
                scroll = r.top;
            } else if r.top + r.h > scroll + list.height() {
                scroll = r.top + r.h - list.height();
            }
        }
        app.find.scrolled_to = Some(cur);
    } else if app.find.current.is_none() {
        app.find.scrolled_to = None;
    }
    scroll = scroll.clamp(0.0, (content_h - list.height()).max(0.0));
    app.find.scroll = scroll;

    let mut goto = None;
    clipped(ui, list, |ui| {
        let p = ui.painter().clone();
        for row in &rows {
            let rr = Rect::from_min_size(Pos2::new(list.left(), list.top() + row.top - scroll), Vec2::new(list.width(), row.h));
            if rr.bottom() < list.top() || rr.top() > list.bottom() {
                continue;
            }
            match row.kind {
                RowKind::Page(id) => {
                    let name = app.doc.get(id).map(|n| n.name.clone()).unwrap_or_default();
                    icons::paint(
                        &p,
                        Icon::Page,
                        Pos2::new(rr.left() + theme::INSET_PANEL + theme::LAYER_ICON_SIZE / 2.0, rr.center().y),
                        theme::LAYER_ICON_SIZE,
                        theme::ICON_SECONDARY,
                    );
                    p.text(
                        Pos2::new(rr.left() + theme::INSET_PANEL + theme::LAYER_ICON_SIZE + theme::SPACE_INLINE, rr.center().y),
                        Align2::LEFT_CENTER,
                        name,
                        theme::font_body_strong(),
                        theme::TEXT,
                    );
                }
                RowKind::Group(id) => {
                    let Some(n) = app.doc.get(id) else { continue };
                    let indent = if app.find.search_in == crate::find::SearchIn::AllPages { 16.0 } else { 0.0 };
                    icons::paint(
                        &p,
                        kind_icon(&n.kind),
                        Pos2::new(rr.left() + theme::INSET_PANEL + theme::LAYER_ICON_SIZE / 2.0 + indent, rr.center().y),
                        theme::LAYER_ICON_SIZE,
                        theme::ICON_TERTIARY,
                    );
                    let name = widgets::elide(&p, &n.name, &theme::font_body(), rr.width() - 60.0 - indent);
                    p.text(
                        Pos2::new(rr.left() + theme::INSET_PANEL + theme::LAYER_ICON_SIZE + theme::SPACE_INLINE + indent, rr.center().y),
                        Align2::LEFT_CENTER,
                        name,
                        theme::font_body(),
                        theme::TEXT_SECONDARY,
                    );
                }
                RowKind::Hit(i, in_group) => {
                    let m = &app.find.results[i];
                    let Some(n) = app.doc.get(m.node) else { continue };
                    let resp = ui.interact(rr, ui.id().with(("find-hit", i)), Sense::click());
                    let is_cur = app.find.current == Some(i);
                    let pill = rr.shrink2(Vec2::new(theme::INSET_FRAME, 0.0));
                    if is_cur {
                        p.rect_filled(pill, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_SELECTED_HOVER } else { theme::BG_SELECTED });
                    } else if resp.hovered() {
                        p.rect_filled(pill, theme::cr(theme::RADIUS_CONTROL), theme::BG_TRANSPARENT_HOVER);
                    }
                    let indent = (if app.find.search_in == crate::find::SearchIn::AllPages { 16.0 } else { 0.0 }) + if in_group { 16.0 } else { 0.0 };
                    let icon_c = Pos2::new(rr.left() + theme::INSET_PANEL + theme::LAYER_ICON_SIZE / 2.0 + indent, rr.center().y);
                    let comp = matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. }) || n.component.source.is_some();
                    let icon_col = if comp {
                        theme::TEXT_COMPONENT
                    } else if is_cur {
                        theme::ICON_SELECTED
                    } else {
                        theme::ICON_TERTIARY
                    };
                    icons::paint(&p, kind_icon(&n.kind), icon_c, theme::LAYER_ICON_SIZE, icon_col);
                    let source = match (&n.kind, m.field) {
                        (NodeKind::Text(t), MatchField::Text) => t.content.as_str(),
                        _ => n.name.as_str(),
                    };
                    let ex = excerpt(source, &m.range, theme::FIND_EXCERPT_LEAD);
                    let tx = icon_c.x + theme::LAYER_ICON_SIZE / 2.0 + 8.0;
                    let text_clip = Rect::from_min_max(Pos2::new(tx, rr.top()), Pos2::new(rr.right() - 12.0, rr.bottom()));
                    let font = theme::font_body();
                    let pp = p.with_clip_rect(text_clip.intersect(list));
                    let a = tx + widgets::text_width(&pp, &ex.text[..ex.hl.start], &font);
                    let b = tx + widgets::text_width(&pp, &ex.text[..ex.hl.end], &font);
                    let hl = Rect::from_min_max(Pos2::new(a, rr.center().y - 9.0), Pos2::new(b, rr.center().y + 9.0));
                    pp.rect_filled(hl, theme::cr(theme::RADIUS_MARK), if is_cur { theme::FIND_HIGHLIGHT_SELECTED } else { theme::FIND_HIGHLIGHT });
                    pp.text(Pos2::new(tx, rr.center().y), Align2::LEFT_CENTER, &ex.text, font, if n.visible { theme::TEXT } else { theme::TEXT_TERTIARY });
                    widgets::record(ui, &format!("find:hit{i}"), rr);
                    if resp.clicked() {
                        goto = Some(i);
                    }
                }
            }
        }
    });
    scrollbar(ui, list, scroll, content_h);
    if let Some(i) = goto {
        app.find_goto(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_marks_the_match_and_trims_long_leads() {
        let s = "Revenue grew 18% over the last quarter.";
        let at = s.find("quarter").unwrap();
        let e = excerpt(s, &(at..at + 7), 12);
        assert_eq!(&e.text[e.hl.clone()], "quarter");
        assert!(e.text.starts_with('…'));
        assert!(e.text.chars().count() < 40);
        let e = excerpt("short", &(0..5), 12);
        assert_eq!((e.text.as_str(), e.hl), ("short", 0..5));
    }

    #[test]
    fn excerpt_stays_on_the_matched_line_and_on_char_boundaries() {
        let s = "first line\nsecond 🎉 line é\nthird";
        let at = s.find('🎉').unwrap();
        let e = excerpt(s, &(at..at + 4), 3);
        assert_eq!(&e.text[e.hl.clone()], "🎉");
        assert!(e.text.ends_with("line é"));
        assert!(!e.text.contains('\n'));
        // A match that spans lines is cut at the end of its first line.
        let e = excerpt(s, &(6..16), 12);
        assert_eq!(&e.text[e.hl.clone()], "line");
    }

    #[test]
    fn excerpt_bounds_very_long_lines() {
        let long = format!("{}needle{}", "a".repeat(500), "b".repeat(500));
        let e = excerpt(&long, &(500..506), 12);
        assert_eq!(&e.text[e.hl.clone()], "needle");
        assert!(e.text.chars().count() < 160, "{}", e.text.chars().count());
        assert!(e.text.ends_with('…'));
    }
}
