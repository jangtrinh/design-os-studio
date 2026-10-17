//! UI tests for variables: the view (floating panel and full view), the inspector binding UI and mode switching.

use super::*;
use crate::panels::rail;
use designos_doc::{PaintKind, VariableKind, VariableValue};

/// (collection id, light mode id, dark mode id) of the demo Theme collection.
fn theme_ids(app: &App) -> (String, String, String) {
    let c = app.doc.variable_collections.values().find(|c| c.name == "Theme").expect("Theme collection");
    (c.id.clone(), c.modes[0].id.clone(), c.modes[1].id.clone())
}

fn var<'a>(app: &'a App, name: &str) -> &'a designos_doc::Variable {
    app.doc.variables.values().find(|v| v.name == name).unwrap_or_else(|| panic!("no variable {name}"))
}

/// Pixel of the Card rendered through the document renderer.
fn card_pixel(s: &Sim) -> [u8; 4] {
    let pm = designos_render::render_node(&s.app().doc, s.by_name("Card"), 1.0).unwrap();
    let p = pm.pixel(268, 40).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

#[test]
fn demo_has_a_theme_collection_bound_to_the_card() {
    let s = Sim::new();
    let (c, light, dark) = theme_ids(s.app());
    let coll = &s.app().doc.variable_collections[&c];
    assert_eq!(coll.modes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Light", "Dark"]);
    assert_eq!(coll.default_mode, light);
    let card = s.app().doc.get(s.by_name("Card")).unwrap();
    assert_eq!(card.variables.bindings["fills/0"], var(s.app(), "Surface/Card").id);
    let title = s.app().doc.get(s.by_name("Quarterly report")).unwrap();
    assert_eq!(title.variables.bindings["fills/0"], var(s.app(), "Text/Primary").id);
    assert!(var(s.app(), "Surface/Card").values.contains_key(&dark));
}

#[test]
fn switching_the_card_to_dark_changes_rendered_pixels() {
    let mut s = Sim::with(Options { select: Some("Card".into()), ..Default::default() });
    let card = s.by_name("Card");
    let (c, _, dark) = theme_ids(s.app());
    let light_px = card_pixel(&s);
    assert!(light_px[0] > 240, "light card is white-ish: {light_px:?}");

    // Appearance -> mode dropdown of the Theme collection: Auto, Light, Dark.
    let dd = s.rect_of("mode:Theme");
    s.click(dd.center());
    // menu rows: Auto, Light, Dark (24px rows under the field, 8px padding)
    let row = |i: f32| Pos2::new(dd.left() + 20.0, dd.bottom() + 2.0 + theme::MENU_PADDING + i * theme::MENU_ITEM_HEIGHT + 12.0);
    s.click(row(2.0));
    assert_eq!(s.app().doc.get(card).unwrap().variables.modes.get(&c), Some(&dark));
    let dark_px = card_pixel(&s);
    assert!(dark_px[0] < 80, "dark card is dark: {dark_px:?}");
    assert_ne!(light_px, dark_px);

    // the canvas tile cache follows too: render the page the way the canvas does
    let page = s.app().page;
    let mut cache = designos_render::TileCache::new();
    let vp = designos_render::Viewport { offset: DV::new(540.0, 100.0), zoom: 1.0, dpr: 1.0 };
    let tiles = cache.render_tiles(&s.app().doc, page, &vp, (256, 256), 1, designos_render::TileBudget::unlimited());
    assert!(tiles.pending() == 0);

    // one undo step brings the light theme back
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(card).unwrap().variables.modes.is_empty());
    assert_eq!(card_pixel(&s), light_px);
    s.cmd_key(Key::Y);
    assert_eq!(card_pixel(&s), dark_px);
}

/// Top of the Variables rail cell, looked up so rail changes don't break the tests.
fn variables_rail_top() -> f32 {
    let i = rail::ITEMS.iter().position(|(item, ..)| *item == crate::app::RailItem::Variables).expect("Variables rail item");
    rail::cell_tops()[i]
}

/// Opens the variables view with the rail item: it starts as the floating panel; `full` expands it.
fn open_view(s: &mut Sim, full: bool) {
    let top = variables_rail_top();
    s.click(Pos2::new(28.0, top + 20.0));
    assert!(s.app().vars.open, "the Variables rail item opens the view");
    assert!(!s.app().vars.full, "it opens as the floating panel first");
    if full {
        s.click(s.rect_of("vars-expand").center());
        assert!(s.app().vars.full);
        s.frame(vec![]);
    }
}

fn row_order(s: &Sim) -> Vec<String> {
    let c = s.app().vars.collection.clone().unwrap();
    s.app().doc.variable_collections[&c].variables.iter().map(|id| s.app().doc.variables[id].name.clone()).collect()
}

fn select_row(s: &mut Sim, name: &str) {
    let r = s.rect_of(&format!("vars-row:{name}"));
    s.click(Pos2::new(r.left() + 120.0, r.center().y));
}

#[test]
fn rail_opens_floating_panel_then_full_view_and_back() {
    let mut s = Sim::new();
    open_view(&mut s, false);
    assert_eq!(s.app().vars.collection.as_deref(), s.app().doc.variable_collections.values().find(|c| c.name == "Theme").map(|c| c.id.as_str()));
    // expand -> full view, collapse -> floating again (the form is remembered)
    s.click(s.rect_of("vars-expand").center());
    assert!(s.app().vars.full);
    s.click(s.rect_of("vars-collapse").center());
    assert!(!s.app().vars.full && s.app().vars.open);
    // X closes, the rail item reopens in the last form
    s.click(s.rect_of("vars-close").center());
    assert!(!s.app().vars.open);
    let top = variables_rail_top();
    s.click(Pos2::new(28.0, top + 20.0));
    assert!(s.app().vars.open && !s.app().vars.full);
    s.key(Key::Escape);
    assert!(!s.app().vars.open, "Escape closes when nothing is selected");
}

#[test]
fn floating_panel_drags_by_its_header() {
    let mut s = Sim::new();
    open_view(&mut s, false);
    assert!(s.app().vars.float_pos.is_none());
    s.drag(Pos2::new(110.0, 28.0), Pos2::new(190.0, 108.0));
    let p = s.app().vars.float_pos.expect("moved");
    assert!(p.x > 60.0 && p.y > 60.0, "{p:?}");
}

#[test]
fn editing_a_number_cell_is_one_undo_step() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let (_, light, _) = theme_ids(s.app());
    let r = s.rect_of(&format!("vars-num:Radius/Card:{light}"));
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("20");
    s.key(Key::Enter);
    assert_eq!(var(s.app(), "Radius/Card").values[&light], VariableValue::Number(20.0));
    s.cmd_key(Key::Z);
    assert_eq!(var(s.app(), "Radius/Card").values[&light], VariableValue::Number(12.0));
    s.cmd_key(Key::Y);
    assert_eq!(var(s.app(), "Radius/Card").values[&light], VariableValue::Number(20.0));
}

#[test]
fn create_variable_menu_adds_a_typed_variable() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let before = s.app().doc.variables.len();
    s.click(s.rect_of("vars-create").center());
    assert!(s.app().vars.menu.is_some());
    s.key(Key::ArrowDown);
    s.key(Key::ArrowDown); // Color, Number
    s.key(Key::Enter);
    s.frame(vec![]);
    assert_eq!(s.app().doc.variables.len(), before + 1);
    let v = s.app().doc.variables.values().find(|v| v.name == "Number").expect("new Number variable");
    assert_eq!(v.kind, VariableKind::Number);
    assert_eq!(v.values.len(), 2, "one value per mode");
    s.shell.app.undo();
    assert_eq!(s.app().doc.variables.len(), before);
}

#[test]
fn add_collection_and_mode() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let colls = s.app().doc.variable_collections.len();
    s.click(s.rect_of("vars-add-collection").center());
    assert_eq!(s.app().doc.variable_collections.len(), colls + 1);
    s.type_text("Spacing");
    s.key(Key::Enter);
    assert!(s.app().doc.variable_collections.values().any(|c| c.name == "Spacing"));
    let id = s.app().vars.collection.clone().unwrap();
    s.click(s.rect_of("vars-add-mode").center());
    assert_eq!(s.app().doc.variable_collections[&id].modes.len(), 2);
}

#[test]
fn single_default_mode_header_reads_value_and_modes_rename() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    s.click(s.rect_of("vars-add-collection").center());
    s.type_text("Spacing");
    s.key(Key::Enter);
    s.frame(vec![]);
    s.rect_of("vars-mode:Value");
    // a second mode switches the header to the mode names
    s.click(s.rect_of("vars-add-mode").center());
    s.frame(vec![]);
    s.rect_of("vars-mode:Mode 1");
    s.rect_of("vars-mode:Mode 2");
}

#[test]
fn bind_button_opens_picker_and_binds_a_fill() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let sw = s.rect_of("sw:false:0");
    s.mv(Pos2::new(sw.right() + 40.0, sw.center().y));
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0").center());
    assert!(s.app().vars.bind_picker.is_some());
    s.frame(vec![]);
    let item = s.rect_of("pick:Brand/Primary");
    s.click(item.center());
    let brand = var(s.app(), "Brand/Primary").id.clone();
    let n = s.app().doc.get(hero).unwrap();
    assert_eq!(n.variables.bindings["fills/0"], brand);
    assert!(s.app().vars.bind_picker.is_none());
    // the field now shows the variable pill with a detach action
    s.frame(vec![]);
    let pill = s.rect_of("bound:fills/0");
    s.mv(pill.center());
    s.frame(vec![]);
    s.click(Pos2::new(pill.right() - 13.0, pill.center().y));
    let n = s.app().doc.get(hero).unwrap();
    assert!(n.variables.bindings.is_empty());
    assert!(matches!(n.fills[0].kind, PaintKind::Solid(_)));
}

#[test]
fn numeric_fields_bind_and_unbind() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    // add a number variable to bind the width to
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (c, light, _) = theme_ids(s.app());
    let w = store.add_variable(&c, "Size/Wide", VariableKind::Number).unwrap();
    store.set_value(&w, &light, VariableValue::Number(222.0));
    s.shell.app.commit_store(store, &[], &[]);
    s.frame(vec![]);
    let field = s.field_rect("w");
    s.mv(field.center());
    s.frame(vec![]);
    s.click(s.rect_of("bind:width").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Size/Wide").center());
    let n = s.app().doc.get(hero).unwrap();
    assert_eq!(n.variables.bindings["width"], w);
    assert_eq!(n.size.x, 222.0);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(hero).unwrap().variables.is_empty());
}

#[test]
fn enter_renames_the_selected_row_keeping_its_group() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    select_row(&mut s, "Text/Primary");
    assert_eq!(s.app().vars.selected.len(), 1);
    s.key(Key::Enter);
    assert!(s.app().vars.renaming.is_some());
    s.type_text("Ink");
    s.key(Key::Enter);
    assert!(s.app().doc.variables.values().any(|v| v.name == "Text/Ink"));
    let m = s.rect_of("vars-mode:Dark");
    s.click(Pos2::new(m.left() + 20.0, m.center().y));
    // double click renames a mode; here the header only selects, so use the app state directly
    s.shell.app.vars.renaming = Some(('m', theme_ids(s.app()).2));
    s.shell.app.vars.rename_frames = 0;
    s.frame(vec![]);
    s.type_text("Night");
    s.key(Key::Enter);
    let (c, _, dark) = theme_ids(s.app());
    assert_eq!(s.app().doc.variable_collections[&c].modes.iter().find(|m| m.id == dark).unwrap().name, "Night");
}

#[test]
fn shift_and_ctrl_click_multi_select() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let order: Vec<String> = row_order(&s);
    let a = order[0].clone();
    select_row(&mut s, &a);
    let c = order.last().unwrap().clone();
    assert_ne!(a, c);
    s.mods = Modifiers::SHIFT;
    select_row(&mut s, &c);
    s.mods = Modifiers::NONE;
    let n = s.app().vars.selected.len();
    assert!(n >= 2, "shift selects a range: {n}");
    s.mods = Modifiers::COMMAND;
    select_row(&mut s, &c);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().vars.selected.len(), n - 1, "ctrl toggles a row");
}

#[test]
fn delete_and_ctrl_d_work_on_the_selection_in_single_undo_steps() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let n0 = s.app().doc.variables.len();
    select_row(&mut s, "Text/Primary");
    s.cmd_key(Key::D);
    assert_eq!(s.app().doc.variables.len(), n0 + 1);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.variables.len(), n0);
    let ink = var(s.app(), "Text/Primary").id.clone();
    let title = s.by_name("Quarterly report");
    assert_eq!(s.app().doc.get(title).unwrap().variables.bindings["fills/0"], ink);
    select_row(&mut s, "Text/Primary");
    s.key(Key::Delete);
    assert!(!s.app().doc.variables.contains_key(&ink));
    assert!(!s.app().doc.get(title).unwrap().variables.bindings.contains_key("fills/0"));
    s.shell.app.undo();
    assert!(s.app().doc.variables.contains_key(&ink));
    assert_eq!(s.app().doc.get(title).unwrap().variables.bindings["fills/0"], ink);
}

#[test]
fn row_menu_has_figmas_items_and_deletes() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let ink = var(s.app(), "Text/Primary").id.clone();
    let r = s.rect_of("vars-row:Text/Primary");
    s.right_click(Pos2::new(r.left() + 120.0, r.center().y));
    assert!(s.app().vars.menu.is_some());
    // Edit variable, Duplicate, New group with selection, Delete
    for _ in 0..4 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::Enter);
    s.frame(vec![]);
    assert!(!s.app().doc.variables.contains_key(&ink));
    s.shell.app.undo();
    assert!(s.app().doc.variables.contains_key(&ink));
}

#[test]
fn new_group_with_selection_prefixes_the_names() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let r = s.rect_of("vars-row:Text/Primary");
    s.right_click(Pos2::new(r.left() + 120.0, r.center().y));
    for _ in 0..3 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::Enter);
    s.frame(vec![]);
    assert!(s.app().doc.variables.values().any(|v| v.name == "Text/Group 1/Primary"), "{:?}", row_order(&s));
}

#[test]
fn edit_variable_popover_edits_scoping_and_publishing() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let r = s.rect_of("vars-row:Text/Primary");
    s.right_click(Pos2::new(r.left() + 120.0, r.center().y));
    s.key(Key::ArrowDown);
    s.key(Key::Enter); // Edit variable
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().vars.edit.is_some());
    s.click(s.rect_of("var-pop:hide").center());
    assert!(var(s.app(), "Text/Primary").hidden_from_publishing);
    s.frame(vec![]);
    // unchecking one property turns "all" into an explicit list without it
    s.click(s.rect_of("var-pop:STROKE_COLOR").center());
    let scopes = var(s.app(), "Text/Primary").scopes.clone();
    assert!(!scopes.is_empty() && !scopes.contains(&"STROKE_COLOR".to_string()) && scopes.contains(&"ALL_FILLS".to_string()), "{scopes:?}");
    s.frame(vec![]);
    s.click(s.rect_of("var-pop:all").center());
    assert!(var(s.app(), "Text/Primary").scopes.is_empty(), "the checkbox restores all properties");
    s.key(Key::Escape);
    s.frame(vec![]);
    assert!(s.app().vars.edit.is_none());
}

#[test]
fn alias_button_picks_another_variable() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let (_, light, _) = theme_ids(s.app());
    let cell = s.rect_of(&format!("vars-hex:Text/Primary:{light}"));
    s.mv(cell.center());
    s.frame(vec![]);
    s.click(s.rect_of(&format!("vars-bind:Text/Primary:{light}")).center());
    assert!(s.app().vars.alias_picker.is_some());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Gray/900").center());
    let target = var(s.app(), "Gray/900").id.clone();
    assert_eq!(var(s.app(), "Text/Primary").values[&light], VariableValue::Alias(target.clone()));
    // the alias shows as a pill; its detach button bakes the resolved colour back
    s.frame(vec![]);
    let pill = s.rect_of(&format!("vars-alias:Text/Primary:{light}"));
    s.mv(pill.center());
    s.frame(vec![]);
    s.click(Pos2::new(pill.right() - 13.0, pill.center().y));
    assert!(matches!(var(s.app(), "Text/Primary").values[&light], VariableValue::Color(_)));
    let store = designos_doc::VariableStore::from_doc(&s.app().doc);
    assert!(!store.can_alias(&target, &var(s.app(), "Radius/Card").id));
}

#[test]
fn dragging_a_row_reorders_and_drops_into_groups() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    let before = row_order(&s);
    let pos = |o: &[String], n: &str| o.iter().position(|x| x == n).unwrap();
    assert!(pos(&before, "Text/Primary") < pos(&before, "Text/Secondary"));
    let from = s.rect_of("vars-row:Text/Secondary");
    let target = s.rect_of("vars-row:Text/Primary");
    s.drag(Pos2::new(from.left() + 120.0, from.center().y), Pos2::new(target.left() + 120.0, target.top() + 4.0));
    let after = row_order(&s);
    assert!(pos(&after, "Text/Secondary") < pos(&after, "Text/Primary"), "{after:?}");
    s.shell.app.undo();
    assert_eq!(row_order(&s), before);

    // onto another group's header row: the variable joins that group
    s.frame(vec![]);
    let from = s.rect_of("vars-row:Text/Secondary");
    let hdr = s.rect_of("vars-header:Surface");
    s.drag(Pos2::new(from.left() + 120.0, from.center().y), hdr.center());
    assert!(s.app().doc.variables.values().any(|v| v.name == "Surface/Secondary"), "{:?}", row_order(&s));
    s.shell.app.undo();
    assert_eq!(row_order(&s), before);
}

#[test]
fn groups_come_from_slashes_and_filter_the_table() {
    let s = Sim::new();
    let c = s.app().doc.variable_collections.values().find(|c| c.name == "Theme").unwrap().clone();
    let store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let groups: Vec<String> = crate::variables::view::group_paths(&store, &c).into_iter().map(|(p, _)| p).collect();
    assert!(groups.contains(&"Text".to_string()) && groups.contains(&"Surface".to_string()), "{groups:?}");
    let mut s = s;
    open_view(&mut s, true);
    s.click(s.rect_of("vars-group:Text").center());
    assert_eq!(s.app().vars.group.as_deref(), Some("Text"));
    s.click(s.rect_of("vars-group:All").center());
    assert_eq!(s.app().vars.group, None);
}

#[test]
fn text_fill_edits_only_the_selected_range_and_binds_the_layer() {
    let mut s = Sim::new();
    s.key(Key::T);
    s.click(s.screen(300.0, 820.0));
    s.type_text("hello world");
    let id = s.app().selection[0];
    s.cmd_key(Key::Home);
    s.mods = Modifiers::SHIFT;
    for _ in 0..5 {
        s.key(Key::ArrowRight);
    }
    s.mods = Modifiers::NONE;
    s.frame(vec![]);
    // the Fill section shows the text colour; editing it recolours just "hello"
    s.scroll_inspector_down();
    let hex = s.rect_of("text-hex");
    s.click(Pos2::new(hex.left() + 30.0, hex.center().y));
    s.type_text("FF0000");
    s.key(Key::Enter);
    let t = text_of(&s, id);
    assert_eq!(t.runs.len(), 1, "{:?}", t.runs);
    assert_eq!((t.runs[0].start, t.runs[0].end), (0, 5));
    assert!(matches!(&t.runs[0].style.fills[0].kind, PaintKind::Solid(c) if c.to_hex() == "FF0000"));
    assert!(!matches!(&t.style.fills[0].kind, PaintKind::Solid(c) if c.to_hex() == "FF0000"), "the rest keeps its colour");
    s.key(Key::Escape);
    assert!(s.app().canvas.text.is_none(), "Escape leaves the text session");

    // with one colour, the whole layer's colour can be bound to a variable (`fills/0`)
    s.key(Key::T);
    s.click(s.screen(300.0, 900.0));
    s.type_text("plain");
    let id = s.app().selection[0];
    s.key(Key::Escape);
    s.shell.app.selection = vec![id];
    s.scroll_inspector_down();
    let sw = s.rect_of("text-sw");
    s.mv(Pos2::new(sw.right() + 40.0, sw.center().y));
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Text/Primary").center());
    let ink = var(s.app(), "Text/Primary").id.clone();
    let n = s.app().doc.get(id).unwrap();
    assert_eq!(n.variables.bindings["fills/0"], ink);
    let t = text_of(&s, id);
    assert!(matches!(&t.style.fills[0].kind, PaintKind::Solid(c) if c.to_hex() == "1E1E1E"), "the raw colour takes the variable's value");
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(id).unwrap().variables.is_empty(), "binding is one undo step");
}

// ------------------------------------------------------------------ library collections

/// Adds a subscribed-library collection "Shared brand" with one colour variable "Brand/Sea" to the demo
/// document; returns (collection id, variable id, the variable's default mode).
fn add_library_collection(s: &mut Sim) -> (String, String, String) {
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let coll = store.add_collection("Shared brand");
    let mode = store.collections[&coll].default_mode.clone();
    let var = store.add_variable(&coll, "Brand/Sea", VariableKind::Color).unwrap();
    store.set_value(&var, &mode, VariableValue::Color(designos_doc::Color::rgb(0x0C, 0x8C, 0xE9)));
    store.collections.get_mut(&coll).unwrap().source_library_key = Some("lk-shared".into());
    assert!(s.shell.app.commit_store(store, &[], &[]), "a new library collection can be added");
    (coll, var, mode)
}

fn recorded(s: &Sim, name: &str) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string())))).is_some()
}

#[test]
fn library_collections_are_listed_under_a_from_library_heading() {
    let mut s = Sim::new();
    let (lib, ..) = add_library_collection(&mut s);
    open_view(&mut s, true);
    let heading = s.rect_of("vars-heading:From library");
    let library_row = s.rect_of("vars-coll:Shared brand");
    for own in ["Theme", "Primitives"] {
        let row = s.rect_of(&format!("vars-coll:{own}"));
        assert!(row.bottom() <= heading.top(), "{own} sits above the heading");
    }
    assert!(library_row.top() >= heading.bottom(), "the library collection sits under the heading");
    assert_ne!(s.app().vars.collection.as_deref(), Some(lib.as_str()), "the panel opens on a collection the file owns");
}

#[test]
fn no_heading_without_library_collections() {
    let mut s = Sim::new();
    open_view(&mut s, true);
    assert!(!recorded(&s, "vars-heading:From library"));
}

#[test]
fn a_library_collection_cannot_be_edited_from_the_panel() {
    let mut s = Sim::new();
    let (lib, var, mode) = add_library_collection(&mut s);
    s.shell.app.vars.collection = Some(lib.clone());
    open_view(&mut s, true);
    let before = (s.app().doc.variables.clone(), s.app().doc.variable_collections.clone());
    assert!(!recorded(&s, "vars-create") && !recorded(&s, "vars-add-mode"), "no create-variable or add-mode controls");
    assert!(recorded(&s, "vars-readonly"), "a note says the collection is read-only");

    // typing a new hex into the value cell
    let hex = s.rect_of(&format!("vars-hex:Brand/Sea:{mode}"));
    s.click(Pos2::new(hex.left() + 30.0, hex.center().y));
    s.type_text("00FF00");
    s.key(Key::Enter);
    // double-clicking the name, and the collection in the sidebar, do not start a rename
    let name = s.rect_of("vars-row:Brand/Sea");
    s.click(name.center());
    s.click(name.center());
    assert!(s.app().vars.renaming.is_none(), "the variable name stays read-only");
    let side = s.rect_of("vars-coll:Shared brand");
    s.click(side.center());
    s.click(side.center());
    assert!(s.app().vars.renaming.is_none(), "the collection name stays read-only");
    // right-clicking a row or a mode column opens no editing menu
    s.right_click(s.rect_of("vars-row:Brand/Sea").center());
    assert!(s.app().vars.menu.is_none());
    s.right_click(s.rect_of("vars-mode:Value").center());
    assert!(s.app().vars.menu.is_none());
    assert_eq!((s.app().doc.variables.clone(), s.app().doc.variable_collections.clone()), before);
    assert_eq!(s.app().doc.variables[&var].values[&mode], VariableValue::Color(designos_doc::Color::rgb(0x0C, 0x8C, 0xE9)));
}

#[test]
fn committing_a_variable_edit_leaves_library_collections_untouched() {
    let mut s = Sim::new();
    let (lib, var, mode) = add_library_collection(&mut s);
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    // everything a panel path could do to the maps directly
    store.variables.get_mut(&var).unwrap().values.insert(mode, VariableValue::Color(designos_doc::Color::WHITE));
    store.variables.get_mut(&var).unwrap().name = "Hacked".into();
    store.collections.get_mut(&lib).unwrap().name = "Hacked".into();
    let local = s.app().doc.variables.values().find(|v| v.name == "Radius/Card").unwrap().id.clone();
    store.variables.get_mut(&local).unwrap().name = "Radius/Edited".into();
    let before = s.app().doc.variable_collections[&lib].clone();
    assert!(s.shell.app.commit_store(store, &[], &[]), "the local edit still commits");
    assert_eq!(s.app().doc.variable_collections[&lib], before);
    assert_eq!(s.app().doc.variables[&var].name, "Brand/Sea");
    assert!(s.app().doc.variables.values().any(|v| v.name == "Radius/Edited"));
    // a store whose only change is to the library collection is no change at all
    let mut only_library = designos_doc::VariableStore::from_doc(&s.app().doc);
    only_library.collections.remove(&lib);
    assert!(!s.shell.app.commit_store(only_library, &[], &[]));
    assert!(s.app().doc.variable_collections.contains_key(&lib));
}
