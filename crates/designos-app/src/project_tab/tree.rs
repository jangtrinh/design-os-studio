//! The doc tree panel: header, "Documents" title row, search, the rows, drag to reorder, rename and context menus.
//! It paints and reports what the user asked for ([`TreeAction`]); the Project tab runs the actions.

use super::menu::{self, Entry, Outcome};
use super::tree_model::{build_rows, group_takes_pages, page_order, Row, RowKey, RowKind};
use crate::icons::{self, Icon};
use crate::tabs::project_tab::ops::TreeSnapshot;
use crate::theme;
use crate::widgets::{self, kit};
use designos_docs::Group;
use egui::{Align2, Color32, Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Ui, Vec2};
use std::collections::HashSet;

const ROW_H: f32 = theme::LAYER_ROW_HEIGHT;
const ROW_INSET: f32 = theme::PAGE_ROW_INSET;
const ICON_BOX: f32 = 16.0;
const SEARCH_ROW_H: f32 = theme::CONTROL_XS + theme::SPACE_INLINE;
const BUTTON: f32 = theme::PANEL_ACTION_BUTTON;
const ACTION_GAP: f32 = theme::SPACE_INLINE;
const COUNT_PAD: f32 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub enum TreeAction {
    Open(String),
    Rename { rel: String, title: String },
    Reorder { group: Group, order: Vec<String> },
    AskDelete(String),
    NewPage(Group),
    OpenInHome,
    RevealDocs,
    CopyLink(String),
    EditSource(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuTarget {
    Page(String, Group),
    Group(Group),
}

#[derive(Clone, Debug)]
pub struct RenameState {
    pub rel: String,
    pub text: String,
    fresh: bool,
}

#[derive(Clone, Debug)]
struct Drag {
    rel: String,
    group: Group,
}

#[derive(Default)]
pub struct TreeUi {
    pub collapsed: HashSet<RowKey>,
    pub search_open: bool,
    pub query: String,
    pub scroll: f32,
    pub rename: Option<RenameState>,
    pub menu: Option<(MenuTarget, Pos2)>,
    pub header_menu: Option<Pos2>,
    pub new_menu: Option<Pos2>,
    drag: Option<Drag>,
    /// The row keyboard navigation is on.
    pub focus: Option<RowKey>,
    /// The tree owns the arrow keys (the last click was inside it).
    pub keyboard: bool,
    search_fresh: bool,
}

impl TreeUi {
    pub fn start_rename(&mut self, rel: &str, title: &str) {
        self.rename = Some(RenameState { rel: rel.to_string(), text: title.to_string(), fresh: true });
    }

    pub fn open_search(&mut self) {
        self.search_open = true;
        self.search_fresh = true;
    }

    pub fn close_search(&mut self) {
        self.search_open = false;
        self.query.clear();
    }
}

/// What the tree needs from the tab each frame.
pub struct TreeInput<'a> {
    pub project_name: &'a str,
    pub snapshot: &'a TreeSnapshot,
    pub selected: Option<&'a str>,
    /// A dialog is open over the tree: it owns the keyboard.
    pub modal: bool,
}

fn files_line(n: usize) -> String {
    format!("{n} file{}", if n == 1 { "" } else { "s" })
}

/// The label of the "Reveal" row: the platform's own name for its file manager.
fn reveal_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Reveal in Finder"
    } else if cfg!(target_os = "windows") {
        "Show in Explorer"
    } else {
        "Show in file manager"
    }
}

/// Rectangles of the tree's own regions, from the panel rect. Exposed so tests and the tab agree with the painter.
pub struct TreeRects {
    pub header: Rect,
    pub title: Rect,
    pub more: Rect,
    pub search: Rect,
    pub collapse: Rect,
    pub plus: Rect,
    pub list: Rect,
}

pub fn regions(panel: Rect, search_open: bool) -> TreeRects {
    let inner = Rect::from_min_max(panel.min, Pos2::new(panel.right() - theme::BORDER_WIDTH, panel.bottom()));
    let header = Rect::from_min_size(inner.min, Vec2::new(inner.width(), theme::LEFT_HEADER_HEIGHT));
    let title = Rect::from_min_size(Pos2::new(inner.left(), header.bottom()), Vec2::new(inner.width(), theme::PANEL_TITLE_ROW_HEIGHT));
    let by = title.center().y - BUTTON / 2.0;
    let plus = Rect::from_min_size(Pos2::new(inner.right() - theme::SPACE_INLINE - BUTTON, by), Vec2::splat(BUTTON));
    let collapse = plus.translate(Vec2::new(-(BUTTON + ACTION_GAP), 0.0));
    let search = collapse.translate(Vec2::new(-(BUTTON + ACTION_GAP), 0.0));
    let more = Rect::from_min_size(Pos2::new(inner.right() - theme::SPACE_INLINE - BUTTON, header.top() + theme::LEFT_ACTION_Y), Vec2::splat(BUTTON));
    let list_top = title.bottom() + if search_open { SEARCH_ROW_H } else { 0.0 };
    let list = Rect::from_min_max(Pos2::new(inner.left(), list_top), inner.max);
    TreeRects { header, title, more, search, collapse, plus, list }
}

fn row_rect(list: Rect, index: usize, scroll: f32) -> Rect {
    Rect::from_min_size(Pos2::new(list.left() + ROW_INSET, list.top() + index as f32 * ROW_H - scroll), Vec2::new(list.width() - ROW_INSET * 2.0, ROW_H))
}

/// Paints the tree in `panel` and returns what the user did.
pub fn show(ui: &mut Ui, panel: Rect, input: &TreeInput, st: &mut TreeUi) -> Vec<TreeAction> {
    let mut actions = Vec::new();
    let p = ui.painter().clone();
    p.rect_filled(panel, 0.0, theme::BG);
    widgets::vline(&p, panel.right() - theme::BORDER_WIDTH, panel.top(), panel.bottom(), theme::BORDER);
    let r = regions(panel, st.search_open);
    header(ui, &r, input, st);
    title_row(ui, &r, st);
    if st.search_open {
        search_field(ui, &r, st);
    }
    let rows = build_rows(input.snapshot, &st.collapsed, &st.query);
    list(ui, &r, &rows, input, st, &mut actions);
    menus(ui.ctx(), &rows, input, st, &mut actions);
    if ui.input(|i| i.pointer.any_pressed()) {
        st.keyboard = ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| panel.contains(pos));
    }
    keys(ui.ctx(), &rows, input, st, &mut actions);
    actions
}

fn header(ui: &mut Ui, r: &TreeRects, input: &TreeInput, st: &mut TreeUi) {
    let p = ui.painter().clone();
    let x = r.header.left() + theme::INSET_PANEL;
    let name_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let shown = widgets::elide(&p, input.project_name, &name_font, r.more.left() - x - theme::SPACE_INLINE);
    p.text(Pos2::new(x, r.header.top() + theme::LEFT_FILE_NAME_Y + 10.0), Align2::LEFT_CENTER, shown, name_font, theme::TEXT);
    p.text(
        Pos2::new(x, r.header.top() + theme::LEFT_SUBLABEL_Y + 9.0),
        Align2::LEFT_CENTER,
        files_line(input.snapshot.files.len()),
        theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
        theme::TEXT_TERTIARY,
    );
    widgets::hline(&p, r.header.left(), r.header.right(), r.header.bottom() - theme::BORDER_WIDTH, theme::BORDER);
    widgets::record(ui, "project-tree-header", r.header);
    widgets::record(ui, "project-tree-more", r.more);
    if widgets::icon_button(ui, r.more, "project-tree-more", Icon::Ellipsis, st.header_menu.is_some(), "Project menu").clicked() {
        st.header_menu = if st.header_menu.is_some() { None } else { Some(r.more.left_bottom() + Vec2::new(0.0, theme::SPACE_FACT)) };
    }
}

fn title_row(ui: &mut Ui, r: &TreeRects, st: &mut TreeUi) {
    let p = ui.painter().clone();
    kit::panel_header(&p, r.title, "Documents");
    widgets::record(ui, "project-tree-title", r.title);
    widgets::record(ui, "project-tree-search", r.search);
    widgets::record(ui, "project-tree-collapse", r.collapse);
    widgets::record(ui, "project-tree-plus", r.plus);
    if widgets::icon_button(ui, r.search, "project-tree-search", Icon::Search, st.search_open, "Search pages").clicked() {
        if st.search_open {
            st.close_search();
        } else {
            st.open_search();
        }
    }
    if widgets::icon_button(ui, r.collapse, "project-tree-collapse", Icon::ChevronDown, false, "Collapse all").clicked() {
        for g in Group::ALL {
            if g != Group::Overview {
                st.collapsed.insert(RowKey::Group(g));
            }
        }
    }
    if widgets::icon_button(ui, r.plus, "project-tree-plus", Icon::Plus, st.new_menu.is_some(), "New page").clicked() {
        st.new_menu = if st.new_menu.is_some() { None } else { Some(r.plus.left_bottom() + Vec2::new(0.0, theme::SPACE_FACT)) };
    }
}

fn search_field(ui: &mut Ui, r: &TreeRects, st: &mut TreeUi) {
    let p = ui.painter().clone();
    let field = Rect::from_min_size(
        Pos2::new(r.list.left() + theme::SPACE_INLINE, r.title.bottom()),
        Vec2::new(r.list.width() - theme::SPACE_INLINE * 2.0, theme::CONTROL_XS),
    );
    let id = Id::new("project-tree-search-field");
    let focused = ui.memory(|m| m.has_focus(id));
    kit::paint_frame(&p, field, theme::RADIUS_CONTROL, &kit::field_look(if focused { kit::State::Focused } else { kit::State::Default }, false));
    if focused {
        p.rect_stroke(field, theme::cr(theme::RADIUS_CONTROL), egui::Stroke::new(2.0, theme::BORDER_SELECTED), egui::StrokeKind::Inside);
    }
    icons::paint(&p, Icon::Search, Pos2::new(field.left() + theme::PROJECT_SEARCH_ICON_INSET + 8.0, field.center().y), 16.0, theme::ICON_QUATERNARY);
    let clear = Rect::from_center_size(Pos2::new(field.right() - theme::INSET_CONTROL - 8.0, field.center().y), Vec2::splat(16.0));
    let text_rect = Rect::from_min_max(
        Pos2::new(field.left() + theme::PROJECT_SEARCH_TEXT_INSET, field.top()),
        Pos2::new(clear.left() - theme::FIELD_SUFFIX_GAP, field.bottom()),
    );
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular);
    if st.query.is_empty() {
        p.text(Pos2::new(text_rect.left(), field.center().y), Align2::LEFT_CENTER, "Search pages", font.clone(), theme::TEXT_PLACEHOLDER);
    }
    let mut edit = egui::TextEdit::singleline(&mut st.query)
        .id(id)
        .font(font)
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .vertical_align(egui::Align::Center)
        .desired_width(text_rect.width());
    edit = edit.min_size(text_rect.size());
    let resp = ui.put(text_rect, edit);
    widgets::record(ui, "project-tree-search-field", field);
    if st.search_fresh {
        resp.request_focus();
        st.search_fresh = false;
    }
    if !st.query.is_empty() {
        icons::paint(&p, Icon::Close, clear.center(), 12.0, theme::ICON_QUATERNARY);
        let c = ui.interact(clear, id.with("clear"), Sense::click());
        widgets::labelled(ui, c.id, egui::accesskit::Role::Button, "Clear search");
        if c.clicked() {
            st.query.clear();
            resp.request_focus();
        }
    }
    if resp.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
        st.close_search();
    }
}

fn list(ui: &mut Ui, r: &TreeRects, rows: &[Row], input: &TreeInput, st: &mut TreeUi, actions: &mut Vec<TreeAction>) {
    let list = r.list;
    let p = ui.painter().clone();
    if rows.is_empty() && !st.query.trim().is_empty() {
        let cx = list.center().x;
        p.text(
            Pos2::new(cx, list.top() + theme::INSET_PANEL + 9.0),
            Align2::CENTER_CENTER,
            "No pages match",
            theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
            theme::TEXT_TERTIARY,
        );
        let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
        let w = widgets::text_width(&p, "Clear search", &font) + 16.0;
        let btn = Rect::from_center_size(Pos2::new(cx, list.top() + theme::INSET_PANEL + 18.0 + 8.0 + 12.0), Vec2::new(w, 24.0));
        let resp = ui.interact(btn, Id::new("project-tree-clear-search"), Sense::click());
        let color = if resp.hovered() { theme::TEXT_SECONDARY_HOVER } else { theme::TEXT_TERTIARY };
        p.text(btn.center(), Align2::CENTER_CENTER, "Clear search", font, color);
        widgets::record(ui, "project-tree-clear-search", btn);
        if resp.clicked() {
            st.close_search();
        }
        return;
    }
    let content_h = rows.len() as f32 * ROW_H;
    crate::panels::wheel_scroll(ui, list, &mut st.scroll, content_h);
    let scroll = st.scroll;
    let dragging = st.drag.clone();
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let released = ui.input(|i| i.pointer.primary_released());
    let mut drop_line: Option<f32> = None;
    let mut drop_order: Option<(Group, Vec<String>)> = None;
    let mut renaming_done: Option<(String, Option<String>)> = None;
    let mut started: Option<Drag> = None;
    let mut clicked_row: Option<usize> = None;
    let mut toggled: Option<RowKey> = None;
    let mut menu_request: Option<(MenuTarget, Pos2)> = None;
    crate::panels::clipped(ui, list, |ui| {
        for (i, row) in rows.iter().enumerate() {
            let rect = row_rect(list, i, scroll);
            if rect.bottom() < list.top() || rect.top() > list.bottom() {
                continue;
            }
            match &row.kind {
                RowKind::Group(_) => {
                    let resp = ui.interact(rect, ui.id().with(("project-tree-row", i)), Sense::click());
                    paint_group(ui, rect, row);
                    widgets::record(ui, &format!("project-tree-group-{}", row.label), rect);
                    widgets::labelled(ui, resp.id, egui::accesskit::Role::Button, &row.label);
                    if resp.clicked() && row.chevron.is_some() {
                        toggled = Some(row.key.clone());
                    }
                    if resp.secondary_clicked() {
                        if let (RowKind::Group(g), Some(at)) = (&row.kind, ui.input(|i| i.pointer.interact_pos())) {
                            menu_request = Some((MenuTarget::Group(*g), at));
                        }
                    }
                    if st.focus.as_ref() == Some(&row.key) && st.keyboard {
                        kit::focus_ring(&ui.painter().clone(), rect.shrink(theme::INSET_SEGMENT), theme::RADIUS_CONTROL as f32);
                    }
                }
                RowKind::Page { rel, group } => {
                    let selected = input.selected == Some(rel.as_str());
                    let renaming = st.rename.as_ref().is_some_and(|rn| rn.rel == *rel);
                    let resp = ui.interact(rect, ui.id().with(("project-tree-row", i)), Sense::click_and_drag());
                    widgets::record(ui, &format!("project-tree-page-{rel}"), rect);
                    widgets::labelled(ui, resp.id, egui::accesskit::Role::TreeItem, &row.label);
                    let dim = dragging.as_ref().is_some_and(|d| d.rel == *rel);
                    paint_item(ui, rect, row, selected, resp.hovered() && dragging.is_none(), dim, &st.query, renaming);
                    if renaming {
                        if let Some(rn) = st.rename.as_mut() {
                            renaming_done = rename_field(ui, rect, row, rn);
                        }
                    } else {
                        if resp.clicked() {
                            clicked_row = Some(i);
                        }
                        if resp.drag_started() && st.query.trim().is_empty() {
                            started = Some(Drag { rel: rel.clone(), group: *group });
                        }
                        if resp.secondary_clicked() {
                            if let Some(at) = ui.input(|i| i.pointer.interact_pos()) {
                                menu_request = Some((MenuTarget::Page(rel.clone(), *group), at));
                            }
                        }
                    }
                    if st.focus.as_ref() == Some(&row.key) && st.keyboard {
                        kit::focus_ring(&ui.painter().clone(), rect.shrink(theme::INSET_SEGMENT), theme::RADIUS_CONTROL as f32);
                    }
                }
                RowKind::Pin => {
                    paint_item(ui, rect, row, false, false, false, &st.query, false);
                    if row.chevron.is_some() {
                        let resp = ui.interact(
                            Rect::from_min_size(rect.min + Vec2::new(row.depth as f32 * theme::LAYER_INDENT, 0.0), Vec2::new(theme::LAYER_CHEVRON_BOX, ROW_H)),
                            ui.id().with(("project-tree-chevron", i)),
                            Sense::click(),
                        );
                        if resp.clicked() {
                            toggled = Some(row.key.clone());
                        }
                    }
                }
                RowKind::Placeholder(group) => {
                    let hot = pointer.is_some_and(|q| rect.contains(q)) && ui.rect_contains_pointer(rect);
                    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular);
                    ui.painter().text(
                        Pos2::new(rect.left() + theme::PROJECT_TREE_LABEL_INSET, rect.center().y),
                        Align2::LEFT_CENTER,
                        &row.label,
                        font,
                        theme::TEXT_TERTIARY,
                    );
                    if hot && group_takes_pages(*group) {
                        let plus = Rect::from_min_size(Pos2::new(rect.right() - COUNT_PAD - BUTTON, rect.center().y - BUTTON / 2.0), Vec2::splat(BUTTON));
                        widgets::record(ui, &format!("project-tree-add-{}", row.label), plus);
                        if widgets::icon_button(ui, plus, ("project-tree-add", i), Icon::Plus, false, "New page").clicked() {
                            actions.push(TreeAction::NewPage(*group));
                        }
                    }
                }
            }
        }
    });
    if let Some(d) = &dragging {
        if let Some(q) = pointer {
            let (line, order) = drop_slot(rows, d, q, list, scroll);
            drop_line = line;
            drop_order = order;
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            drag_ghost(ui.ctx(), rows, d, q);
        }
        if let Some(y) = drop_line {
            let line = Rect::from_min_max(
                Pos2::new(list.left() + ROW_INSET, y - theme::LAYER_DROP_THICKNESS / 2.0),
                Pos2::new(list.right() - ROW_INSET, y + theme::LAYER_DROP_THICKNESS / 2.0),
            );
            ui.painter().with_clip_rect(list).rect_filled(line, 0.0, theme::TEXT);
        }
        if released {
            if let Some((group, order)) = drop_order {
                if order != page_order(rows, group) {
                    actions.push(TreeAction::Reorder { group, order });
                }
            }
            st.drag = None;
        }
    }
    if let Some(d) = started {
        st.drag = Some(d);
    }
    crate::panels::scrollbar(ui, list, st.scroll, content_h);
    if let Some(s) = crate::panels::take_scrollbar_drag(ui, list) {
        st.scroll = s;
    }
    if let Some(key) = toggled {
        if !st.collapsed.remove(&key) {
            st.collapsed.insert(key);
        }
    }
    if let Some(i) = clicked_row {
        if let RowKind::Page { rel, .. } = &rows[i].kind {
            st.focus = Some(rows[i].key.clone());
            actions.push(TreeAction::Open(rel.clone()));
        }
    }
    if let Some(m) = menu_request {
        st.menu = Some(m);
    }
    if let Some((rel, title)) = renaming_done {
        st.rename = None;
        if let Some(title) = title {
            actions.push(TreeAction::Rename { rel, title });
        }
    }
}

fn paint_group(ui: &mut Ui, rect: Rect, row: &Row) {
    let p = ui.painter().clone();
    if let Some(open) = row.chevron {
        let center = Pos2::new(rect.left() + theme::LAYER_CHEVRON_BOX / 2.0, rect.center().y);
        icons::paint(&p, if open { Icon::ChevronDown } else { Icon::ChevronRight }, center, 12.0, theme::ICON_QUATERNARY);
    }
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
    p.text(Pos2::new(rect.left() + theme::LAYER_CHEVRON_BOX, rect.center().y), Align2::LEFT_CENTER, &row.label, font, theme::TEXT_SECONDARY);
    if let Some(n) = row.count {
        let text = n.to_string();
        let w = (widgets::text_width(&p, &text, &theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium)) + 12.0).max(theme::BADGE_SM);
        let pill = Rect::from_min_size(Pos2::new(rect.right() - COUNT_PAD - w, rect.center().y - theme::BADGE_SM / 2.0), Vec2::new(w, theme::BADGE_SM));
        kit::count_badge(&p, pill, &text, kit::Tone::Neutral);
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_item(ui: &mut Ui, rect: Rect, row: &Row, selected: bool, hovered: bool, dim: bool, query: &str, renaming: bool) {
    let p = ui.painter().clone();
    let state = if selected {
        kit::State::Selected
    } else if hovered {
        kit::State::Hover
    } else {
        kit::State::Default
    };
    let mut look = kit::row_look(state, kit::Surface::Primary);
    let fade = |c: Color32| if dim { c.gamma_multiply(0.6) } else { c };
    look.fill = fade(look.fill);
    kit::paint_frame(&p, rect, theme::RADIUS_CONTROL, &look);
    let indent = row.depth as f32 * theme::LAYER_INDENT;
    let icon_left = if let Some(open) = row.chevron {
        let center = Pos2::new(rect.left() + indent + theme::LAYER_CHEVRON_BOX / 2.0, rect.center().y);
        let glyph = if open { Icon::ChevronDown } else { Icon::ChevronRight };
        icons::paint(&p, glyph, center, 12.0, fade(theme::ICON_QUATERNARY));
        rect.left() + indent + theme::LAYER_CHEVRON_BOX
    } else {
        rect.left() + theme::LAYER_CHEVRON_BOX + indent
    };
    icons::paint(&p, row.icon, Pos2::new(icon_left + ICON_BOX / 2.0, rect.center().y), ICON_BOX, fade(look.icon));
    if renaming {
        return;
    }
    let label_x = icon_left + ICON_BOX + theme::SPACE_FACT;
    let weight = if selected { theme::TextWeight::Semibold } else { theme::TextWeight::Regular };
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, weight);
    let shown = widgets::elide(&p, &row.label, &font, rect.right() - label_x - theme::SPACE_INLINE);
    paint_label(&p, Pos2::new(label_x, rect.center().y), &shown, &font, fade(look.text), query);
}

/// Draws `text`, with the first match of `query` in `TEXT` Semibold.
fn paint_label(p: &egui::Painter, left: Pos2, text: &str, font: &egui::FontId, color: Color32, query: &str) {
    let q = query.trim().to_lowercase();
    let lower = text.to_lowercase();
    let at = (!q.is_empty()).then(|| lower.find(&q)).flatten().filter(|i| text.is_char_boundary(*i) && text.is_char_boundary(i + q.len()));
    let Some(start) = at else {
        p.text(left, Align2::LEFT_CENTER, text, font.clone(), color);
        return;
    };
    let (head, rest) = text.split_at(start);
    let (hit, tail) = rest.split_at(q.len());
    let strong = theme::font_text(font.size, theme::TextWeight::Semibold);
    let mut x = left.x;
    for (part, f, c) in [(head, font, color), (hit, &strong, theme::TEXT), (tail, font, color)] {
        if part.is_empty() {
            continue;
        }
        let r = p.text(Pos2::new(x, left.y), Align2::LEFT_CENTER, part, f.clone(), c);
        x = r.right();
    }
}

fn rename_field(ui: &mut Ui, rect: Rect, row: &Row, rn: &mut RenameState) -> Option<(String, Option<String>)> {
    let p = ui.painter().clone();
    let icon_left = rect.left() + theme::LAYER_CHEVRON_BOX + row.depth as f32 * theme::LAYER_INDENT;
    let field = Rect::from_min_max(
        Pos2::new(icon_left + ICON_BOX + 1.0, rect.top() + theme::SPACE_ROW),
        Pos2::new(rect.right() - theme::SPACE_INLINE, rect.bottom() - theme::SPACE_ROW),
    );
    kit::paint_frame(&p, field, theme::RADIUS_CONTROL, &kit::field_look(kit::State::Focused, false));
    p.rect_stroke(field, theme::cr(theme::RADIUS_CONTROL), egui::Stroke::new(2.0, theme::BORDER_SELECTED), egui::StrokeKind::Inside);
    let id = Id::new(("project-tree-rename", &rn.rel));
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular);
    let edit = egui::TextEdit::singleline(&mut rn.text)
        .id(id)
        .font(font)
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::symmetric(theme::PROJECT_RENAME_TEXT_PAD as i8, 0))
        .vertical_align(egui::Align::Center)
        .desired_width(field.width())
        .min_size(field.size());
    let resp = ui.put(field, edit);
    widgets::record(ui, "project-tree-rename-field", field);
    if rn.fresh {
        resp.request_focus();
        let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        let n = rn.text.chars().count();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
        state.store(ui.ctx(), id);
        rn.fresh = false;
        return None;
    }
    let esc = ui.input(|i| i.key_pressed(Key::Escape));
    if esc && (resp.has_focus() || resp.lost_focus()) {
        return Some((rn.rel.clone(), None));
    }
    if resp.lost_focus() {
        let title = rn.text.trim().to_string();
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        let blur_commit = enter || !ui.memory(|m| m.has_focus(id));
        if blur_commit {
            return Some((rn.rel.clone(), (!title.is_empty()).then_some(title)));
        }
    }
    None
}

/// Where a dragged page would land: the y of the insertion line and the group's new order, or none when the
/// pointer is outside the page's own group (the drop is refused and the row snaps back).
fn drop_slot(rows: &[Row], drag: &Drag, pointer: Pos2, list: Rect, scroll: f32) -> (Option<f32>, Option<(Group, Vec<String>)>) {
    let Some(gi) = rows.iter().position(|r| matches!(r.kind, RowKind::Group(g) if g == drag.group)) else { return (None, None) };
    let end = rows[gi + 1..].iter().position(|r| matches!(r.kind, RowKind::Group(_))).map(|n| gi + 1 + n).unwrap_or(rows.len());
    let band_top = row_rect(list, gi, scroll).top();
    let band_bottom = row_rect(list, end.saturating_sub(1), scroll).bottom();
    if pointer.y < band_top || pointer.y > band_bottom || pointer.x < list.left() || pointer.x > list.right() {
        return (None, None);
    }
    let pages: Vec<(usize, &String)> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match &r.kind {
            RowKind::Page { rel, group } if *group == drag.group => Some((i, rel)),
            _ => None,
        })
        .collect();
    let Some(from) = pages.iter().position(|(_, rel)| **rel == drag.rel) else { return (None, None) };
    let mut slot = pages.len();
    for (n, (i, _)) in pages.iter().enumerate() {
        if pointer.y < row_rect(list, *i, scroll).center().y {
            slot = n;
            break;
        }
    }
    let y = match pages.get(slot) {
        Some((i, _)) => row_rect(list, *i, scroll).top(),
        None => pages.last().map(|(i, _)| row_rect(list, *i, scroll).bottom()).unwrap_or(band_top),
    };
    let mut order: Vec<String> = pages.iter().map(|(_, rel)| (*rel).clone()).collect();
    let moved = order.remove(from);
    let to = if slot > from { slot - 1 } else { slot };
    order.insert(to.min(order.len()), moved);
    (Some(y), Some((drag.group, order)))
}

fn drag_ghost(ctx: &Context, rows: &[Row], drag: &Drag, pointer: Pos2) {
    let Some(row) = rows.iter().find(|r| matches!(&r.kind, RowKind::Page { rel, .. } if *rel == drag.rel)) else { return };
    let p = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("project-tree-ghost")));
    let rect = Rect::from_min_size(pointer + Vec2::new(12.0, 8.0), Vec2::new(theme::LEFT_PANEL_WIDTH - 24.0, ROW_H));
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
    let ghost = |c: Color32| c.gamma_multiply(0.8);
    icons::paint(&p, row.icon, Pos2::new(rect.left() + theme::LAYER_CHEVRON_BOX + 8.0, rect.center().y), ICON_BOX, ghost(theme::ICON_QUATERNARY_HOVER));
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Semibold);
    p.text(
        Pos2::new(rect.left() + theme::LAYER_CHEVRON_BOX + ICON_BOX + theme::SPACE_FACT, rect.center().y),
        Align2::LEFT_CENTER,
        &row.label,
        font,
        ghost(theme::TEXT_SECONDARY_HOVER),
    );
}

fn page_menu_entries(rows: &[Row], rel: &str, group: Group) -> Vec<Entry> {
    let order = page_order(rows, group);
    let at = order.iter().position(|r| r == rel).unwrap_or(0);
    vec![
        Entry::new("Open").icon(Icon::BookOpen),
        Entry::new("Rename").icon(Icon::Pencil).shortcut("F2"),
        Entry::new("Move up").icon(Icon::ChevronUp).disabled(at == 0),
        Entry::new("Move down").icon(Icon::ChevronDown).disabled(at + 1 >= order.len()),
        Entry::new("Copy page link").icon(Icon::Link),
        Entry::new("Edit source").icon(Icon::Dev).sep(),
        Entry::new("Delete").icon(Icon::Trash).destructive(),
    ]
}

fn menus(ctx: &Context, rows: &[Row], input: &TreeInput, st: &mut TreeUi, actions: &mut Vec<TreeAction>) {
    if let Some((target, at)) = st.menu.clone() {
        match target {
            MenuTarget::Page(rel, group) => {
                let entries = page_menu_entries(rows, &rel, group);
                match menu::show(ctx, Id::new("project-tree-menu"), at, &entries) {
                    Outcome::Open => {}
                    Outcome::Closed => st.menu = None,
                    Outcome::Picked(i) => {
                        st.menu = None;
                        let title = input.snapshot.title_of(&rel);
                        match i {
                            0 => actions.push(TreeAction::Open(rel)),
                            1 => st.start_rename(&rel, &title),
                            2 | 3 => {
                                let mut order = page_order(rows, group);
                                if let Some(from) = order.iter().position(|r| *r == rel) {
                                    let to = if i == 2 { from.saturating_sub(1) } else { (from + 1).min(order.len() - 1) };
                                    let moved = order.remove(from);
                                    order.insert(to, moved);
                                    actions.push(TreeAction::Reorder { group, order });
                                }
                            }
                            4 => actions.push(TreeAction::CopyLink(rel)),
                            5 => actions.push(TreeAction::EditSource(rel)),
                            _ => actions.push(TreeAction::AskDelete(rel)),
                        }
                    }
                }
            }
            MenuTarget::Group(group) => {
                let collapsible = group != Group::Overview;
                let closed = st.collapsed.contains(&RowKey::Group(group));
                let mut entries = Vec::new();
                let adds = group_takes_pages(group);
                if adds {
                    entries.push(Entry::new("New page").icon(Icon::Plus));
                }
                if collapsible {
                    entries.push(Entry::new(if closed { "Expand" } else { "Collapse" }).icon(if closed { Icon::ChevronDown } else { Icon::ChevronUp }));
                }
                if entries.is_empty() {
                    st.menu = None;
                } else {
                    match menu::show(ctx, Id::new("project-tree-menu"), at, &entries) {
                        Outcome::Open => {}
                        Outcome::Closed => st.menu = None,
                        Outcome::Picked(i) => {
                            st.menu = None;
                            if adds && i == 0 {
                                actions.push(TreeAction::NewPage(group));
                            } else if closed {
                                st.collapsed.remove(&RowKey::Group(group));
                            } else {
                                st.collapsed.insert(RowKey::Group(group));
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(at) = st.header_menu {
        let entries = vec![Entry::new("Open in Home").icon(Icon::Home), Entry::new(reveal_label()).icon(Icon::Folder)];
        match menu::show(ctx, Id::new("project-tree-header-menu"), at, &entries) {
            Outcome::Open => {}
            Outcome::Closed => st.header_menu = None,
            Outcome::Picked(i) => {
                st.header_menu = None;
                actions.push(if i == 0 { TreeAction::OpenInHome } else { TreeAction::RevealDocs });
            }
        }
    }
    if let Some(at) = st.new_menu {
        let entries = vec![Entry::new("New overview page"), Entry::new("New spec page"), Entry::new("New discovery page")];
        match menu::show(ctx, Id::new("project-tree-new-menu"), at, &entries) {
            Outcome::Open => {}
            Outcome::Closed => st.new_menu = None,
            Outcome::Picked(i) => {
                st.new_menu = None;
                actions.push(TreeAction::NewPage([Group::Overview, Group::Spec, Group::Discovery][i]));
            }
        }
    }
}

/// Arrow keys, Enter and F2 while the tree owns the keyboard.
fn keys(ctx: &Context, rows: &[Row], input: &TreeInput, st: &mut TreeUi, actions: &mut Vec<TreeAction>) {
    if input.modal
        || !st.keyboard
        || st.rename.is_some()
        || st.menu.is_some()
        || st.header_menu.is_some()
        || st.new_menu.is_some()
        || ctx.egui_wants_keyboard_input()
    {
        return;
    }
    let (up, down, left, right, enter, f2) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowLeft),
            i.consume_key(Modifiers::NONE, Key::ArrowRight),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::F2),
        )
    });
    if rows.is_empty() || !(up || down || left || right || enter || f2) {
        return;
    }
    let navigable: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| matches!(r.kind, RowKind::Group(_) | RowKind::Page { .. })).map(|(i, _)| i).collect();
    let current = st
        .focus
        .as_ref()
        .and_then(|k| navigable.iter().position(|i| rows[*i].key == *k))
        .or_else(|| navigable.iter().position(|i| matches!(&rows[*i].key, RowKey::Page(r) if input.selected == Some(r.as_str()))));
    let step = |from: Option<usize>, delta: i32| -> Option<RowKey> {
        let n = navigable.len() as i32;
        let at = from.map(|f| f as i32).unwrap_or(if delta > 0 { -1 } else { n });
        let next = (at + delta).clamp(0, n - 1);
        navigable.get(next as usize).map(|i| rows[*i].key.clone())
    };
    if up {
        st.focus = step(current, -1);
    }
    if down {
        st.focus = step(current, 1);
    }
    let focused = st.focus.clone();
    match focused {
        Some(RowKey::Group(g)) if g != Group::Overview => {
            if left {
                st.collapsed.insert(RowKey::Group(g));
            }
            if right {
                st.collapsed.remove(&RowKey::Group(g));
            }
            if enter && !st.collapsed.remove(&RowKey::Group(g)) {
                st.collapsed.insert(RowKey::Group(g));
            }
        }
        Some(RowKey::Page(rel)) => {
            if enter {
                actions.push(TreeAction::Open(rel.clone()));
            }
            if f2 {
                let title = input.snapshot.title_of(&rel);
                st.start_rename(&rel, &title);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_regions_follow_the_spec_at_1440() {
        let panel = Rect::from_min_size(Pos2::new(48.0, 32.0), Vec2::new(240.0, 868.0));
        let r = regions(panel, false);
        assert_eq!(r.header.height(), 65.0);
        assert_eq!(r.title.top(), 97.0);
        assert_eq!(r.title.height(), 40.0);
        assert_eq!((r.plus.center().x, r.plus.center().y), (267.0, 117.0));
        assert_eq!(r.collapse.center().x, 235.0);
        assert_eq!(r.search.center().x, 203.0);
        assert_eq!(r.more.center(), Pos2::new(267.0, 56.0));
        assert_eq!(r.list.top(), 137.0);
        assert_eq!(regions(panel, true).list.top(), 177.0, "the search field takes a 32 row and 8 below");
    }

    #[test]
    fn the_page_menu_has_the_mockup_rows_and_icons() {
        let entries = page_menu_entries(&[], "a.md", Group::Spec);
        let rows: Vec<(&str, Option<Icon>)> = entries.iter().map(|e| (e.label.as_str(), e.icon)).collect();
        assert_eq!(
            rows,
            vec![
                ("Open", Some(Icon::BookOpen)),
                ("Rename", Some(Icon::Pencil)),
                ("Move up", Some(Icon::ChevronUp)),
                ("Move down", Some(Icon::ChevronDown)),
                ("Copy page link", Some(Icon::Link)),
                ("Edit source", Some(Icon::Dev)),
                ("Delete", Some(Icon::Trash)),
            ]
        );
    }

    #[test]
    fn a_page_drag_inside_its_group_reorders_and_outside_it_is_refused() {
        use crate::tabs::project_tab::ops::TreeSnapshot;
        use designos_docs::Manifest;
        let mut manifest = Manifest::new();
        manifest.group_mut(Group::Spec).pages = vec!["a.md".into(), "b.md".into(), "c.md".into()];
        manifest.group_mut(Group::Discovery).pages = vec!["d.md".into()];
        let mut snap = TreeSnapshot::empty();
        snap.manifest = manifest;
        let rows = build_rows(&snap, &HashSet::new(), "");
        let list = Rect::from_min_size(Pos2::new(48.0, 137.0), Vec2::new(239.0, 700.0));
        let drag = Drag { rel: "c.md".into(), group: Group::Spec };
        // Rows: Overview, its placeholder, Spec, a, b, c, ... ; a pointer above row "a" puts c first.
        let a = rows.iter().position(|r| matches!(&r.kind, RowKind::Page { rel, .. } if rel == "a.md")).unwrap();
        let above_a = Pos2::new(100.0, row_rect(list, a, 0.0).top() + 2.0);
        let (line, order) = drop_slot(&rows, &drag, above_a, list, 0.0);
        assert_eq!(line, Some(row_rect(list, a, 0.0).top()));
        assert_eq!(order, Some((Group::Spec, vec!["c.md".into(), "a.md".into(), "b.md".into()])));
        // Over the Discovery group: refused.
        let disc = rows.iter().position(|r| r.label == "d" || matches!(&r.kind, RowKind::Page { rel, .. } if rel == "d.md")).unwrap();
        let over_other = Pos2::new(100.0, row_rect(list, disc, 0.0).center().y);
        assert_eq!(drop_slot(&rows, &drag, over_other, list, 0.0), (None, None));
    }
}
