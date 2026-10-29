//! Effect, grid and text styles in the inspector: the "Style" (four dots) button of the Effects,
//! Layout guide and Typography headers with its menu (apply, create, rename, detach, edit), and the
//! row that shows the style the selection is linked to (in place of the effect or grid rows, above
//! the type rows), like Figma's UI3.

use super::properties::Pane;
use super::style_edit;
use crate::app::App;
use crate::icons::{self, Icon};
use crate::styles::{shared_text_style, style_entries_of, StyleKind, TYPE_FIRST};
use crate::theme;
use crate::widgets::field;
use crate::widgets::menu::{popup_menu, MenuItem, MenuResult};
use crate::widgets::{self};
use designos_doc::*;
use egui::text::{CCursor, CCursorRange};
use egui::{Align2, Id, Pos2, Rect, Sense, TextEdit, Ui};

const MENU_W: f32 = 184.0;

const UNDO_SHORTCUT: &str = if cfg!(target_os = "macos") { "Cmd+Z" } else { "Ctrl+Z" };
const STYLE_DELETED_TOAST: &str = if cfg!(target_os = "macos") { "Style deleted · Cmd+Z to undo" } else { "Style deleted · Ctrl+Z to undo" };

fn style_deleted_toast(fallback_layers: usize) -> String {
    match fallback_layers {
        0 => STYLE_DELETED_TOAST.to_string(),
        1 => format!("Style deleted · 1 layer matches it in this mode only · {UNDO_SHORTCUT} to undo"),
        n => format!("Style deleted · {n} layers match it in this mode only · {UNDO_SHORTCUT} to undo"),
    }
}

fn delete_linked_style(app: &mut App, sel: &[NodeId], prop: StyleProperty) {
    let Some(key) = linked_style(app, sel, prop) else { return };
    app.scrub_end();
    if app.props.style_edit.as_ref().is_some_and(|s| s.key == key) {
        app.props.style_edit = None;
    }
    if app.props.style_rename == Some(prop) {
        app.props.style_rename = None;
    }
    let (op, fallback_layers) = match delete_style_with_fallbacks(&app.doc, &key) {
        Ok((cmd, fallbacks)) => {
            let mut ids: Vec<NodeId> = fallbacks.into_iter().map(|(id, _)| id).collect();
            ids.dedup();
            (Ok(cmd), ids.len())
        }
        Err(e) => (Err(e), 0),
    };
    if app.run_style_op(op) {
        app.show_toast(style_deleted_toast(fallback_layers), false);
    }
}

/// Fill and stroke styles have their own buttons, so anything but effect and text is a grid here.
fn kind_of(prop: StyleProperty) -> StyleKind {
    match prop {
        StyleProperty::Effect => StyleKind::Effect,
        StyleProperty::Text => StyleKind::Text,
        _ => StyleKind::Grid,
    }
}

/// Name the linked row and its test records of `prop` start with.
fn tag(prop: StyleProperty) -> &'static str {
    match prop {
        StyleProperty::Effect => "effect-style",
        StyleProperty::Text => "text-style",
        _ => "grid-style",
    }
}

fn button_key(prop: StyleProperty) -> &'static str {
    match prop {
        StyleProperty::Effect => "effect-style-button",
        StyleProperty::Text => "text-style-button",
        _ => "grid-style-button",
    }
}

/// Why the create row is off for a text layer whose runs differ, and for several layers that differ
/// from each other: shown as its tooltip.
const MIXED_TEXT_TIP: &str = "Text has mixed styles";
const MIXED_SELECTION_TIP: &str = "Selected text has mixed styles";

/// Style of `prop` the first selected node is linked to, when that style exists.
pub(super) fn linked_style(app: &App, sel: &[NodeId], prop: StyleProperty) -> Option<String> {
    let key = app.doc.get(*sel.first()?)?.style_refs.get(kind_of(prop).ref_key())?;
    app.doc.is_table_style(key).then(|| key.clone())
}

/// Whether the first selected node has effects or layout grids to turn into a style.
fn has_values(app: &App, sel: &[NodeId], prop: StyleProperty) -> bool {
    let Some(n) = sel.first().and_then(|id| app.doc.get(*id)) else { return false };
    match prop {
        StyleProperty::Effect => !n.effects.is_empty(),
        _ => n.kind.frame_props().is_some_and(|f| !f.grids.is_empty()),
    }
}

/// Whether the create row is off and, when there is a reason the user can act on, what it is.
fn create_state(app: &App, sel: &[NodeId], prop: StyleProperty, linked: bool) -> (bool, &'static str) {
    if linked {
        return (true, "");
    }
    match prop {
        // A new text box with no text has nothing to make a style from, and ending its session would delete it.
        StyleProperty::Text if app.is_new_empty_text(sel) => (true, TYPE_FIRST),
        StyleProperty::Text if shared_text_style(&app.doc, sel).is_none() => (true, if sel.len() > 1 { MIXED_SELECTION_TIP } else { MIXED_TEXT_TIP }),
        StyleProperty::Text => (false, ""),
        _ => (!has_values(app, sel, prop), ""),
    }
}

enum Pick {
    Apply(String),
    Create,
    Rename,
    RenameFolder(String),
    Detach,
    Delete,
    Edit,
}

/// Menu rows and, aligned with them, what each one does (None for a disabled placeholder).
fn menu_model(app: &App, sel: &[NodeId], prop: StyleProperty) -> (Vec<MenuItem>, Vec<Option<Pick>>) {
    let current = linked_style(app, sel, prop);
    let (none, create) = match prop {
        StyleProperty::Effect => ("No effect styles", "Create effect style"),
        StyleProperty::Text => ("No text styles", "Create text style"),
        _ => ("No grid styles", "Create grid style"),
    };
    let mut items = Vec::new();
    let mut picks = Vec::new();
    let mut last_folder: Option<String> = None;
    for e in style_entries_of(&app.doc, kind_of(prop)) {
        if !e.folder.is_empty() && last_folder.as_deref() != Some(e.folder.as_str()) {
            items.push(MenuItem::new(e.folder.clone()).disabled(true));
            picks.push(None);
        }
        last_folder = Some(e.folder.clone());
        let label = if e.folder.is_empty() { e.leaf } else { format!("  {}", e.leaf) };
        let tip = crate::styles::style_tooltip(&app.doc, &e.key);
        items.push(MenuItem::new(label).checked(current.as_deref() == Some(e.key.as_str())).tooltip(tip));
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
    // A library style is edited, renamed and deleted in its library file.
    let read_only = current.as_deref().is_some_and(|k| style_edit::is_read_only(app, k));
    let ro_tip = if read_only { style_edit::LIBRARY_STYLE_TIP } else { "" };
    let delete_ro = read_only || current.as_deref().is_some_and(|k| used_on_library_page(&app.doc, k));
    let delete_tip = if delete_ro { style_edit::LIBRARY_STYLE_TIP } else { "" };
    let linked_folder = current.as_deref().map(|k| crate::styles::split_name(app.doc.style_name(k)).0).filter(|f| !f.is_empty());
    let (create_off, create_tip) = create_state(app, sel, prop, linked);
    items.push(MenuItem::new(create).disabled(create_off).tooltip(create_tip));
    picks.push(Some(Pick::Create));
    items.push(MenuItem::new("Rename style").disabled(!linked || read_only).tooltip(ro_tip));
    picks.push(Some(Pick::Rename));
    if let Some(folder) = linked_folder {
        items.push(MenuItem::new("Rename folder").disabled(read_only).tooltip(ro_tip));
        picks.push(Some(Pick::RenameFolder(folder)));
    }
    items.push(MenuItem::new("Detach style").disabled(!linked));
    items.push(MenuItem::new("Delete style").disabled(!linked || delete_ro).tooltip(delete_tip));
    items.push(MenuItem::new("Edit style").disabled(!linked || read_only).tooltip(ro_tip));
    picks.extend([Some(Pick::Detach), Some(Pick::Delete), Some(Pick::Edit)]);
    (items, picks)
}

/// The "Edit style" row of the menu (always the last one), so tests can read its label and state.
#[cfg(test)]
pub(crate) fn edit_style_row(app: &App, sel: &[NodeId], prop: StyleProperty) -> MenuItem {
    menu_model(app, sel, prop).0.pop().expect("the menu always ends with the Edit style row")
}

/// The create row of the menu, so tests can read its label, state and tooltip.
#[cfg(test)]
pub(crate) fn create_style_row(app: &App, sel: &[NodeId], prop: StyleProperty) -> MenuItem {
    let (items, picks) = menu_model(app, sel, prop);
    items.into_iter().zip(picks).find_map(|(item, pick)| matches!(pick, Some(Pick::Create)).then_some(item)).expect("the menu always has a create row")
}

/// The "Rename style" row of the menu, so tests can read its label and state.
#[cfg(test)]
pub(crate) fn rename_style_row(app: &App, sel: &[NodeId], prop: StyleProperty) -> MenuItem {
    let (items, picks) = menu_model(app, sel, prop);
    items.into_iter().zip(picks).find_map(|(item, pick)| matches!(pick, Some(Pick::Rename)).then_some(item)).expect("the menu always has a Rename style row")
}

/// The "Delete style" row of the menu, so tests can read its label, tooltip and state.
#[cfg(test)]
pub(crate) fn delete_style_row(app: &App, sel: &[NodeId], prop: StyleProperty) -> MenuItem {
    let (items, picks) = menu_model(app, sel, prop);
    items.into_iter().zip(picks).find_map(|(item, pick)| matches!(pick, Some(Pick::Delete)).then_some(item)).expect("the menu always has a Delete style row")
}

static SCREENSHOT_HOVER_TIP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn set_screenshot_hover_tip(on: bool) {
    SCREENSHOT_HOVER_TIP.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The "Style" button left of the "+" of a section header whose title row was just laid out, and
/// its menu while open.
pub(super) fn header_button(app: &mut App, ui: &mut Ui, pane: &Pane, sel: &[NodeId], prop: StyleProperty) {
    let btn = pane.rect(theme::SECTION_TRAILING_X - 28.0, -theme::SECTION_TITLE_HEIGHT + 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    style_button(app, ui, sel, prop, btn, btn.right());
}

/// The "Style" button at `btn` and its menu while open; the menu's right edge sits at `menu_right`.
pub(crate) fn style_button(app: &mut App, ui: &mut Ui, sel: &[NodeId], prop: StyleProperty, btn: Rect, menu_right: f32) {
    let open = app.props.style_menu == Some(prop);
    let key = button_key(prop);
    widgets::record(ui, key, btn);
    if widgets::icon_button(ui, btn, key, Icon::Styles, open, "Apply style").clicked() {
        app.props.style_menu = (!open).then_some(prop);
    }
    if !open {
        return;
    }
    let (items, picks) = menu_model(app, sel, prop);
    let pos = Pos2::new(menu_right - MENU_W, btn.bottom() + 4.0);
    if SCREENSHOT_HOVER_TIP.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some((idx, it)) =
            items.iter().enumerate().find(|(i, it)| picks.get(*i).is_some_and(|p| matches!(p, Some(Pick::Apply(_)))) && !it.tooltip.is_empty())
        {
            crate::widgets::menu::preset_path(ui.ctx(), Id::new(("style-menu", key)), &[idx]);
            let screen_r = ui.ctx().content_rect();
            let est_h = items.len() as f32 * theme::MENU_ITEM_HEIGHT;
            let menu_top = pos.y.min(screen_r.bottom() - est_h).max(screen_r.top() + theme::MENU_PADDING);
            let row_y = menu_top + (idx as f32 + 1.0) * theme::MENU_ITEM_HEIGHT;
            let tip_text = it.tooltip.clone();
            egui::Area::new(Id::new(("style-menu-hover-tip", key)))
                .order(egui::Order::Tooltip)
                .fixed_pos(Pos2::new(pos.x + theme::MENU_PADDING, row_y + theme::MENU_PADDING))
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    crate::widgets::kit::tooltip_frame().show(ui, |ui| {
                        ui.label(tip_text);
                    });
                });
        }
    }
    match popup_menu(ui.ctx(), Id::new(("style-menu", key)), pos, MENU_W, &items, Some(btn)) {
        MenuResult::Picked(i) => {
            app.props.style_menu = None;
            match picks.into_iter().nth(i).flatten() {
                Some(Pick::Apply(style)) => app.apply_picked_style(sel, &style),
                Some(Pick::Create) if prop == StyleProperty::Text => {
                    app.create_text_style_from(sel);
                }
                Some(Pick::Create) => {
                    if let Some(id) = sel.first() {
                        app.create_style_from(*id, prop);
                    }
                }
                Some(Pick::Rename) => {
                    if linked_style(app, sel, prop).is_some_and(|k| !style_edit::is_read_only(app, &k)) {
                        ui.data_mut(|d| d.remove_temp::<String>(Id::new(("style-folder-rename", key))));
                        app.props.style_rename = Some(prop);
                    }
                }
                Some(Pick::RenameFolder(folder)) => {
                    if linked_style(app, sel, prop).is_some_and(|k| !style_edit::is_read_only(app, &k)) {
                        ui.data_mut(|d| d.insert_temp(Id::new(("style-folder-rename", key)), folder));
                        app.props.style_rename = Some(prop);
                    }
                }
                Some(Pick::Detach) => {
                    app.detach_style_of(sel, prop);
                }
                Some(Pick::Delete) => {
                    delete_linked_style(app, sel, prop);
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
    let tag = tag(prop);
    let btn_key = button_key(prop);
    let folder_mode_id = Id::new(("style-folder-rename", btn_key));
    let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W - theme::PANEL_ACTION_BUTTON, theme::FIELD_HEIGHT);
    widgets::record(ui, &format!("{tag}-row"), row);
    let name = app.doc.style_name(key).to_string();
    let id = Id::new((tag, "name"));
    let read_only = style_edit::is_read_only(app, key);
    if app.props.style_rename == Some(prop) && !read_only {
        let folder_target = ui.data(|d| d.get_temp::<String>(folder_mode_id));
        let initial = folder_target.as_deref().unwrap_or(&name);
        let started = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        field::field_bg(ui, row, id);
        let committed = field::edit_text(ui, row, id, initial, theme::font_body(), theme::TEXT, theme::FIELD_PLAIN_X);
        if !started {
            // Focus the field with its text selected once it exists; it ends the rename when it loses focus again.
            ui.memory_mut(|m| m.request_focus(id));
            let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(initial.chars().count()))));
            state.store(ui.ctx(), id);
            ui.data_mut(|d| d.insert_temp(id, true));
        }
        if committed.is_some() || (started && !ui.memory(|m| m.has_focus(id))) {
            ui.data_mut(|d| d.remove_temp::<bool>(id));
            ui.data_mut(|d| d.remove_temp::<String>(folder_mode_id));
            app.props.style_rename = None;
        }
        if let Some(text) = committed {
            if let Some(old_folder) = folder_target {
                app.rename_style_folder_to(&old_folder, &text);
            } else {
                app.rename_style_to(key, &text);
            }
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
        resp.on_hover_text(if app.doc.is_unnamed_style(key) { key.to_string() } else { name });
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The create row names the kind of style it makes, with the noun its own empty-state row uses.
    #[test]
    fn the_create_row_uses_the_noun_of_the_empty_state_row() {
        let mut doc = Document::new("t");
        doc.add_page("p");
        let app = App::new(doc);
        for (prop, empty, create) in [
            (StyleProperty::Effect, "No effect styles", "Create effect style"),
            (StyleProperty::Text, "No text styles", "Create text style"),
            (StyleProperty::Grid, "No grid styles", "Create grid style"),
        ] {
            let (items, picks) = menu_model(&app, &[], prop);
            assert_eq!(items[0].label, empty);
            let at = picks.iter().position(|p| matches!(p, Some(Pick::Create))).expect("every menu has a create row");
            assert_eq!(items[at].label, create);
        }
    }

    #[test]
    fn us01_ac02_orphan_styles_are_excluded_from_picker_and_reopened_designos_while_unpublished_stay_listed() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fx = Style::Effect(vec![Effect::LayerBlur { radius: 4.0, visible: true }]);
        // Orphans in `styles` with no `style_meta` (GUID pairs and consumer-seeded `S:` keys).
        doc.styles.insert("1186:100350".into(), fx.clone());
        doc.styles.insert("2:102".into(), fx.clone());
        doc.styles.insert("S:orphan-fx".into(), fx.clone());
        let nid = doc.alloc_id();
        let mut rect = Node::rectangle(nid, designos_doc::Rect::new(0.0, 0.0, 40.0, 40.0));
        rect.style_refs.insert("effect".into(), "S:orphan-fx".into());
        doc.insert(page, rect).unwrap();

        // Live local styles, including `_` and `.` prefixed names.
        for (k, name) in [("2:100", "Shadow/Card"), ("2:107", "_Private/Shadow"), ("2:108", ".Hidden/Shadow")] {
            doc.styles.insert(k.into(), fx.clone());
            doc.style_meta.insert(k.into(), StyleMeta { name: name.into(), property: StyleProperty::Effect, ..Default::default() });
        }

        let listed: Vec<String> = style_entries_of(&doc, StyleKind::Effect).into_iter().map(|e| e.key).collect();
        assert_eq!(listed, vec!["2:100".to_string(), "2:107".to_string(), "2:108".to_string()]);

        // Reopening an old `.designos` JSON with orphans still hides them at listing time without migration.
        let reopened = Document::from_json(&doc.to_json().unwrap()).unwrap();
        let reopened_listed: Vec<String> = style_entries_of(&reopened, StyleKind::Effect).into_iter().map(|e| e.key).collect();
        assert_eq!(reopened_listed, listed);
    }

    #[test]
    fn us01_ac03_layer_with_deleted_or_orphan_style_shows_no_style_chip() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fx = Style::Effect(vec![Effect::LayerBlur { radius: 4.0, visible: true }]);
        doc.styles.insert("1186:100350".into(), fx.clone());
        doc.styles.insert("2:106".into(), fx);
        doc.style_meta.insert("2:106".into(), StyleMeta { name: "Library/Elevation".into(), property: StyleProperty::Effect, ..Default::default() });

        let orphan_layer = doc.alloc_id();
        let mut n1 = Node::rectangle(orphan_layer, designos_doc::Rect::new(0.0, 0.0, 40.0, 40.0));
        n1.style_refs.insert("effect".into(), "1186:100350".into());
        doc.insert(page, n1).unwrap();

        let embedded_lib_layer = doc.alloc_id();
        let mut n2 = Node::rectangle(embedded_lib_layer, designos_doc::Rect::new(50.0, 0.0, 40.0, 40.0));
        n2.style_refs.insert("effect".into(), "2:106".into());
        doc.insert(page, n2).unwrap();

        let app = App::new(doc);
        assert_eq!(linked_style(&app, &[orphan_layer], StyleProperty::Effect), None, "deleted/orphan style shows no chip on the layer");
        assert_eq!(linked_style(&app, &[embedded_lib_layer], StyleProperty::Effect), Some("2:106".to_string()));
        assert_eq!(app.doc.style_name("2:106"), "Library/Elevation");
    }

    #[test]
    fn us01_ac04_unnamed_style_shows_fallback_label_and_id_only_in_tooltip() {
        let mut doc = Document::new("t");
        doc.add_page("p");
        let fx = Style::Effect(vec![Effect::LayerBlur { radius: 4.0, visible: true }]);
        doc.styles.insert("1186:100350".into(), fx.clone());
        doc.style_meta.insert("1186:100350".into(), StyleMeta { name: "   ".into(), property: StyleProperty::Effect, ..Default::default() });
        doc.styles.insert("2:100".into(), fx);
        doc.style_meta.insert("2:100".into(), StyleMeta { name: "Shadow/Card".into(), property: StyleProperty::Effect, ..Default::default() });

        let app = App::new(doc);
        let (items, _) = menu_model(&app, &[], StyleProperty::Effect);
        let unnamed = items.iter().find(|i| i.label == "Unnamed effect style").expect("unnamed style row present");
        assert_eq!(unnamed.tooltip, "1186:100350", "raw id appears only in the tooltip");
        let named = items.iter().find(|i| i.label == "  Card").expect("named style row present under its folder");
        assert_eq!(named.tooltip, "", "named style has no raw-id tooltip");
    }

    #[test]
    fn us01_ac02_round2_style_entries_of_on_200k_nodes_and_50_styles_is_o1_per_key_and_under_budget() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fx = Style::Effect(vec![Effect::LayerBlur { radius: 4.0, visible: true }]);

        // Populate 200,000 nodes in `doc.nodes` (1 page + 199,999 rectangles).
        let mut children = Vec::with_capacity(199_999);
        for i in 2..=200_000u64 {
            let id = NodeId(i);
            let mut n = Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
            n.parent = Some(page);
            if i > 199_981 {
                let idx = i - 199_982;
                n.style_refs.insert("effect".into(), format!("S:orphan-{idx:02}"));
            }
            doc.nodes.insert(id, n);
            children.push(id);
        }
        doc.reserve_ids(NodeId(200_000));
        doc.nodes.get_mut(&page).unwrap().children = children;
        assert_eq!(doc.nodes.len(), 200_000);

        // 50 styles: 25 live table styles (`S:live-..` and `2:..` with `style_meta`)
        // and 25 orphan styles without `style_meta` (20 `S:`-prefixed keys and 5 GUID keys).
        let mut all_keys = Vec::with_capacity(50);
        for i in 0..25 {
            let k = if i < 15 { format!("S:live-{i:02}") } else { format!("2:{}", 100 + i) };
            doc.styles.insert(k.clone(), fx.clone());
            doc.style_meta.insert(k.clone(), StyleMeta { name: format!("Elevation/Level-{i:02}"), property: StyleProperty::Effect, ..Default::default() });
            all_keys.push((k, true));
        }
        for i in 0..20 {
            let k = format!("S:orphan-{i:02}");
            doc.styles.insert(k.clone(), fx.clone());
            all_keys.push((k, false));
        }
        for i in 0..5 {
            let k = format!("1186:{}", 100_350 + i);
            doc.styles.insert(k.clone(), fx.clone());
            all_keys.push((k, false));
        }
        assert_eq!(doc.styles.len(), 50);

        let budget = if cfg!(debug_assertions) { std::time::Duration::from_millis(50) } else { std::time::Duration::from_millis(5) };
        let t0 = std::time::Instant::now();
        let listed = style_entries_of(&doc, StyleKind::Effect);
        let elapsed = t0.elapsed();

        assert!(elapsed < budget, "style_entries_of on 200,000 nodes and 50 styles took {elapsed:?} (budget {budget:?}); must not walk nodes");
        for (k, expected_table) in &all_keys {
            assert_eq!(doc.is_table_style(k), *expected_table, "is_table_style({k:?}) mismatch");
            assert_eq!(doc.is_orphan_style(k), !*expected_table, "is_orphan_style({k:?}) mismatch");
        }
        assert_eq!(listed.len(), 25, "only the 25 styles with style_meta must be listed");
    }
}
