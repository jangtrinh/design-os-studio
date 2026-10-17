//! Effect and grid styles in the inspector: the "Style" (four dots) button of the Effects and
//! Layout guide headers with its menu (apply, create, rename, detach, edit), and the row that
//! stands in for the effect or grid rows while the selection is linked to a style, like Figma's UI3.

use super::properties::Pane;
use super::style_edit;
use crate::app::App;
use crate::icons::{self, Icon};
use crate::styles::{style_entries_of, StyleKind};
use crate::theme;
use crate::widgets::field;
use crate::widgets::menu::{popup_menu, MenuItem, MenuResult};
use crate::widgets::{self};
use designos_doc::*;
use egui::text::{CCursor, CCursorRange};
use egui::{Align2, Id, Pos2, Sense, TextEdit, Ui};

const MENU_W: f32 = 184.0;

fn kind_of(prop: StyleProperty) -> StyleKind {
    if prop == StyleProperty::Effect {
        StyleKind::Effect
    } else {
        StyleKind::Grid
    }
}

/// Style of `prop` the first selected node is linked to, when that style exists.
pub(super) fn linked_style(app: &App, sel: &[NodeId], prop: StyleProperty) -> Option<String> {
    let key = app.doc.get(*sel.first()?)?.style_refs.get(kind_of(prop).ref_key())?;
    app.doc.styles.contains_key(key).then(|| key.clone())
}

/// Whether the first selected node has effects (or layout grids) to turn into a style.
fn has_values(app: &App, sel: &[NodeId], prop: StyleProperty) -> bool {
    let Some(n) = sel.first().and_then(|id| app.doc.get(*id)) else { return false };
    match prop {
        StyleProperty::Effect => !n.effects.is_empty(),
        _ => n.kind.frame_props().is_some_and(|f| !f.grids.is_empty()),
    }
}

enum Pick {
    Apply(String),
    Create,
    Rename,
    Detach,
    Edit,
}

/// Menu rows and, aligned with them, what each one does (None for a disabled placeholder).
fn menu_model(app: &App, sel: &[NodeId], prop: StyleProperty) -> (Vec<MenuItem>, Vec<Option<Pick>>) {
    let current = linked_style(app, sel, prop);
    let (none, create) =
        if prop == StyleProperty::Effect { ("No effect styles", "Create style from effects") } else { ("No grid styles", "Create style from guides") };
    let mut items = Vec::new();
    let mut picks = Vec::new();
    for e in style_entries_of(&app.doc, kind_of(prop)) {
        items.push(MenuItem::new(app.doc.style_name(&e.key)).checked(current.as_deref() == Some(e.key.as_str())));
        picks.push(Some(Pick::Apply(e.key)));
    }
    if items.is_empty() {
        items.push(MenuItem::new(none).disabled(true));
        picks.push(None);
    }
    if let Some(last) = items.last_mut() {
        last.separator_after = true;
    }
    let linked = current.is_some();
    // A library style is edited and renamed in its library file.
    let read_only = current.as_deref().is_some_and(|k| style_edit::is_read_only(app, k));
    items.push(MenuItem::new(create).disabled(linked || !has_values(app, sel, prop)));
    items.push(MenuItem::new("Rename style").disabled(!linked || read_only));
    items.push(MenuItem::new("Detach style").disabled(!linked));
    items.push(MenuItem::new(if read_only { style_edit::LIBRARY_STYLE_TIP } else { "Edit style" }).disabled(!linked || read_only));
    picks.extend([Some(Pick::Create), Some(Pick::Rename), Some(Pick::Detach), Some(Pick::Edit)]);
    (items, picks)
}

/// The "Edit style" row of the menu (always the last one), so tests can read its label and state.
#[cfg(test)]
pub(crate) fn edit_style_row(app: &App, sel: &[NodeId], prop: StyleProperty) -> MenuItem {
    menu_model(app, sel, prop).0.pop().expect("the menu always ends with the Edit style row")
}

/// The "Rename style" row of the menu, so tests can read its state.
#[cfg(test)]
pub(crate) fn rename_style_row(app: &App, sel: &[NodeId], prop: StyleProperty) -> MenuItem {
    menu_model(app, sel, prop).0.into_iter().find(|item| item.label == "Rename style").expect("the menu always has a Rename style row")
}

/// The "Style" button left of the "+" of a section header whose title row was just laid out, and
/// its menu while open.
pub(super) fn header_button(app: &mut App, ui: &mut Ui, pane: &Pane, sel: &[NodeId], prop: StyleProperty) {
    let btn = pane.rect(theme::SECTION_TRAILING_X - 28.0, -theme::SECTION_TITLE_HEIGHT + 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    let open = app.props.style_menu == Some(prop);
    let key = if prop == StyleProperty::Effect { "effect-style-button" } else { "grid-style-button" };
    widgets::record(ui, key, btn);
    if widgets::icon_button(ui, btn, key, Icon::Styles, open, "Apply style").clicked() {
        app.props.style_menu = (!open).then_some(prop);
    }
    if !open {
        return;
    }
    let (items, picks) = menu_model(app, sel, prop);
    let pos = Pos2::new(btn.right() - MENU_W, btn.bottom() + 4.0);
    match popup_menu(ui.ctx(), Id::new(("style-menu", key)), pos, MENU_W, &items, Some(btn)) {
        MenuResult::Picked(i) => {
            app.props.style_menu = None;
            match picks.into_iter().nth(i).flatten() {
                Some(Pick::Apply(style)) => app.apply_style(sel, &style),
                Some(Pick::Create) => {
                    if let Some(id) = sel.first() {
                        app.create_style_from(*id, prop);
                    }
                }
                Some(Pick::Rename) => {
                    if linked_style(app, sel, prop).is_some_and(|k| !style_edit::is_read_only(app, &k)) {
                        app.props.style_rename = Some(prop);
                    }
                }
                Some(Pick::Detach) => {
                    app.detach_style_of(sel, prop);
                }
                Some(Pick::Edit) => {
                    if let Some(style) = linked_style(app, sel, prop) {
                        style_edit::toggle(app, &style, btn);
                    }
                }
                None => {}
            }
        }
        MenuResult::Closed => app.props.style_menu = None,
        MenuResult::Open => {}
    }
}

/// The row of the style `key` the selection is linked to: its name (double click renames) and a
/// button that detaches it. Leaves `pane` below the row.
pub(super) fn linked_row(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], prop: StyleProperty, key: &str) {
    let tag = if prop == StyleProperty::Effect { "effect-style" } else { "grid-style" };
    let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W - theme::PANEL_ACTION_BUTTON, theme::FIELD_HEIGHT);
    widgets::record(ui, &format!("{tag}-row"), row);
    let name = app.doc.style_name(key).to_string();
    let id = Id::new((tag, "name"));
    let read_only = style_edit::is_read_only(app, key);
    if app.props.style_rename == Some(prop) && !read_only {
        let started = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        field::field_bg(ui, row, id);
        let committed = field::edit_text(ui, row, id, &name, theme::font_body(), theme::TEXT, theme::FIELD_PLAIN_X);
        if !started {
            // Focus the field with its text selected once it exists; it ends the rename when it loses focus again.
            ui.memory_mut(|m| m.request_focus(id));
            let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(name.chars().count()))));
            state.store(ui.ctx(), id);
            ui.data_mut(|d| d.insert_temp(id, true));
        }
        if committed.is_some() || (started && !ui.memory(|m| m.has_focus(id))) {
            ui.data_mut(|d| d.remove_temp::<bool>(id));
            app.props.style_rename = None;
        }
        if let Some(text) = committed {
            app.rename_style_to(key, &text);
        }
    } else {
        let resp = ui.interact(row, id, Sense::click());
        field::field_bg(ui, row, id);
        icons::paint(ui.painter(), Icon::Styles, Pos2::new(row.left() + 14.0, row.center().y), 16.0, theme::ICON_SECONDARY);
        let shown = widgets::elide(ui.painter(), &name, &theme::font_body(), row.width() - 36.0);
        ui.painter().text(Pos2::new(row.left() + 28.0, row.center().y), Align2::LEFT_CENTER, shown, theme::font_body(), theme::TEXT);
        if resp.double_clicked() && !read_only {
            app.props.style_rename = Some(prop);
        }
        resp.on_hover_text(name);
    }
    let edit = pane.rect(theme::SECTION_TRAILING_X - theme::PANEL_ACTION_BUTTON - 4.0, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, &format!("{tag}-edit"), edit);
    if style_edit::edit_button(app, ui, edit, (tag, "edit"), key) {
        style_edit::toggle(app, key, edit);
    }
    let minus = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, &format!("{tag}-detach"), minus);
    if widgets::icon_button(ui, minus, (tag, "detach"), Icon::Minus, false, "Detach style").clicked() {
        app.detach_style_of(sel, prop);
    }
    pane.y += theme::FIELD_HEIGHT + 8.0;
}
