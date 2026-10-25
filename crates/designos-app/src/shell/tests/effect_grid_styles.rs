//! Effect and grid styles in the inspector: create, apply, rename and detach through the
//! "Style" button of the Effects and Layout guide headers, and the local styles list.

use super::*;
use designos_doc::{Effect, LayoutGrid, Style, StyleProperty};

fn shadow() -> Effect {
    Effect::DropShadow { color: Color::rgba(0, 0, 0, 64), offset: DV::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false }
}

fn columns() -> LayoutGrid {
    LayoutGrid::columns(12, 20.0, 0.0, Color::rgba(255, 0, 0, 26))
}

/// "Welcome" selected, with a drop shadow and a 12 column grid, inspector scrolled to its end.
fn welcome_sim() -> (Sim, NodeId) {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let welcome = s.by_name("Welcome");
    s.shell.app.edit_nodes(&[welcome], |n| {
        n.effects = vec![shadow()];
        if let Some(f) = n.kind.frame_props_mut() {
            f.grids = vec![columns()];
        }
    });
    s.scroll_inspector_down();
    (s, welcome)
}

fn grids_of(s: &Sim, id: NodeId) -> Vec<LayoutGrid> {
    s.app().doc.get(id).unwrap().kind.frame_props().expect("Welcome is a frame").grids.clone()
}

fn collect_shape_texts(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
    match shape {
        egui::epaint::Shape::Text(t) => out.push(t.galley.job.text.clone()),
        egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| collect_shape_texts(s, out)),
        _ => {}
    }
}

fn painted_texts(s: &Sim) -> Vec<String> {
    let mut out = Vec::new();
    for cs in &s.shapes {
        collect_shape_texts(&cs.shape, &mut out);
    }
    out
}

/// Opens the menu behind `button` and clicks its `row`-th row, `separators` of which sit above it
/// (the menu opens 4 px below the button, right-aligned to it).
fn pick(s: &mut Sim, button: &str, row: usize, separators: usize) {
    let r = s.rect_of(button);
    s.click(r.center());
    let top = r.bottom() + 4.0 + theme::MENU_PADDING + row as f32 * theme::MENU_ITEM_HEIGHT + separators as f32 * crate::widgets::menu::SEP_H;
    s.click(Pos2::new(r.right() - 60.0, top + theme::MENU_ITEM_HEIGHT / 2.0));
}

#[test]
fn creating_an_effect_style_links_the_layer_in_one_undo_step() {
    let (mut s, welcome) = welcome_sim();
    pick(&mut s, "effect-style-button", 1, 1);
    let key = s.app().doc.get(welcome).unwrap().style_refs.get("effect").cloned().expect("linked to the new style");
    assert_eq!(key, "Effect style 1");
    assert!(matches!(&s.app().doc.styles[&key], Style::Effect(fx) if fx == &vec![shadow()]));
    assert_eq!(s.app().doc.style_meta[&key].property, StyleProperty::Effect);
    s.frame(vec![]);
    s.rect_of("effect-style-row");
    s.cmd_key(Key::Z);
    assert!(!s.app().doc.styles.contains_key("Effect style 1"), "undo takes the style away again");
    assert!(!s.app().doc.get(welcome).unwrap().style_refs.contains_key("effect"));
    assert_eq!(s.app().doc.get(welcome).unwrap().effects, vec![shadow()], "the layer keeps its effect");
}

#[test]
fn detaching_and_applying_an_effect_style_from_the_menu() {
    let (mut s, welcome) = welcome_sim();
    pick(&mut s, "effect-style-button", 1, 1);
    s.frame(vec![]);
    let detach = s.rect_of("effect-style-detach");
    s.click(detach.center());
    let n = s.app().doc.get(welcome).unwrap();
    assert!(!n.style_refs.contains_key("effect"));
    assert_eq!(n.effects, vec![shadow()], "detaching keeps the values");
    assert!(s.app().doc.styles.contains_key("Effect style 1"), "the style stays in the file");
    // Apply it again from the list: the first enabled row is the style itself.
    s.scroll_inspector_down();
    pick(&mut s, "effect-style-button", 0, 0);
    assert_eq!(s.app().doc.get(welcome).unwrap().style_refs.get("effect").map(String::as_str), Some("Effect style 1"));
}

#[test]
fn renaming_an_effect_style_from_the_menu() {
    let (mut s, welcome) = welcome_sim();
    pick(&mut s, "effect-style-button", 1, 1);
    s.frame(vec![]);
    // Rows: the style, Create (disabled while linked), Rename, Detach.
    pick(&mut s, "effect-style-button", 2, 1);
    assert_eq!(s.app().props.style_rename, Some(StyleProperty::Effect));
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().props.style_rename, Some(StyleProperty::Effect), "the name field keeps the focus");
    s.type_text("Shadow/Card");
    s.key(Key::Enter);
    s.frame(vec![]);
    assert_eq!(s.app().doc.style_name("Effect style 1"), "Shadow/Card");
    assert_eq!(s.app().props.style_rename, None);
    assert_eq!(s.app().doc.get(welcome).unwrap().style_refs.get("effect").map(String::as_str), Some("Effect style 1"), "the link is kept");
}

#[test]
fn creating_detaching_and_applying_a_grid_style() {
    let (mut s, welcome) = welcome_sim();
    pick(&mut s, "grid-style-button", 1, 1);
    let key = s.app().doc.get(welcome).unwrap().style_refs.get("grid").cloned().expect("linked to the new style");
    assert_eq!(key, "Grid style 1");
    assert!(matches!(&s.app().doc.styles[&key], Style::Grid(g) if g == &vec![columns()]));
    assert_eq!(s.app().doc.style_meta[&key].property, StyleProperty::Grid);
    s.frame(vec![]);
    let detach = s.rect_of("grid-style-detach");
    s.click(detach.center());
    assert!(!s.app().doc.get(welcome).unwrap().style_refs.contains_key("grid"));
    assert_eq!(grids_of(&s, welcome), vec![columns()], "detaching keeps the guides");
    s.scroll_inspector_down();
    pick(&mut s, "grid-style-button", 0, 0);
    assert_eq!(s.app().doc.get(welcome).unwrap().style_refs.get("grid").map(String::as_str), Some("Grid style 1"));
}

#[test]
fn local_styles_list_effect_and_grid_groups() {
    let (mut s, welcome) = welcome_sim();
    s.shell.app.create_style_from(welcome, StyleProperty::Effect);
    s.shell.app.create_style_from(welcome, StyleProperty::Grid);
    s.shell.app.rename_style_to("Effect style 1", "Shadow/Card");
    // nothing selected: the Styles section is part of the page panel
    s.shell.app.selection.clear();
    s.shell.app.props.scroll = 1.0e6;
    s.frame(vec![]);
    s.frame(vec![]);
    s.rect_of("folder:Effect styles/Shadow");
    s.rect_of("style:Grid style 1");
    s.rect_of("style:Effect style 1");
}

#[test]
fn deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step() {
    let (mut s, welcome) = welcome_sim();
    let card = s.by_name("Card");
    pick(&mut s, "effect-style-button", 1, 1);
    s.shell.app.apply_style(&[card], "Effect style 1");
    s.scroll_inspector_down();
    s.frame(vec![]);

    // Rows: the style (0), Create (1), Rename (2), Detach (3), Delete (4), Edit (5).
    pick(&mut s, "effect-style-button", 4, 1);

    let expected_toast = if cfg!(target_os = "macos") { "Style deleted · Cmd+Z to undo" } else { "Style deleted · Ctrl+Z to undo" };
    let toast = s.app().toast.as_ref().expect("deleting a style shows a toast");
    assert!(!toast.error, "delete toast is informational: {}", toast.text);
    assert_eq!(toast.text, expected_toast);
    assert!(!s.app().doc.styles.contains_key("Effect style 1"), "the style is removed from doc.styles");
    for id in [welcome, card] {
        let n = s.app().doc.get(id).unwrap();
        assert!(!n.style_refs.contains_key("effect"), "{id:?} lost its style ref");
        assert_eq!(n.effects, vec![shadow()], "{id:?} kept its resolved effect values");
    }

    // One undo step restores the style and every linked layer's reference.
    s.cmd_key(Key::Z);
    assert!(s.app().doc.styles.contains_key("Effect style 1"), "undo restores the deleted style");
    for id in [welcome, card] {
        let n = s.app().doc.get(id).unwrap();
        assert_eq!(n.style_refs.get("effect").map(String::as_str), Some("Effect style 1"), "{id:?} link is restored");
        assert_eq!(n.effects, vec![shadow()]);
    }
}

#[test]
fn a_library_style_cannot_be_deleted_from_the_style_menu() {
    use crate::panels::{style_edit, style_picker};
    use designos_doc::{ExternalAsset, LibraryAssetKind};

    let (mut s, welcome) = welcome_sim();
    s.shell.app.create_style_from(welcome, StyleProperty::Effect);
    s.shell.app.create_style_from(welcome, StyleProperty::Grid);
    s.scroll_inspector_down();
    s.frame(vec![]);
    let sel = s.app().selection.clone();
    let props = [(StyleProperty::Effect, "Effect style 1"), (StyleProperty::Grid, "Grid style 1")];

    for (prop, _) in props {
        let row = style_picker::delete_style_row(s.app(), &sel, prop);
        assert_eq!((row.label.as_str(), row.disabled), ("Delete style", false), "{prop:?}: a local style can be deleted from the menu");
    }

    for (_, key) in props {
        s.shell.app.doc.library.external.insert(
            format!("L1/{key}"),
            ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some(key.into()) },
        );
    }

    for (prop, _) in props {
        let row = style_picker::delete_style_row(s.app(), &sel, prop);
        assert!(row.disabled, "{prop:?}: the Delete style row is disabled for a library style");
        assert_eq!(row.label, "Delete style", "{prop:?}: the Delete style row keeps its normal label");
        assert_eq!(row.tooltip, style_edit::LIBRARY_STYLE_TIP, "{prop:?}: the Delete style row carries the A13 tooltip");
    }

    // Clicking the disabled Delete style row in the menu leaves the library style and its link intact.
    pick(&mut s, "effect-style-button", 4, 1);
    assert!(s.app().doc.styles.contains_key("Effect style 1"));
    assert_eq!(s.app().doc.get(welcome).unwrap().style_refs.get("effect").map(String::as_str), Some("Effect style 1"));

    // Hovering the disabled rows in the open menu paints the LIBRARY_STYLE_TIP tooltip.
    s.shell.app.props.style_menu = Some(StyleProperty::Effect);
    s.frame(vec![]);
    for row_idx in [2usize, 4, 5] {
        let pt = s.rect_of(&format!("menu-row-0-{row_idx}")).center();
        s.mv(pt + Vec2::new(0.0, 1.0));
        s.mv(pt);
        for _ in 0..30 {
            s.frame(vec![]);
        }
        let painted = painted_texts(&s);
        assert!(
            painted.iter().any(|t| t == style_edit::LIBRARY_STYLE_TIP),
            "row {row_idx} hover should paint {:?}, got {painted:?}",
            style_edit::LIBRARY_STYLE_TIP
        );
    }
    s.shell.app.props.style_menu = None;
}

#[test]
fn a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu() {
    use crate::panels::{style_edit, style_picker};

    let (mut s, welcome) = welcome_sim();
    s.shell.app.create_style_from(welcome, StyleProperty::Effect);
    s.scroll_inspector_down();
    s.frame(vec![]);
    let sel = s.app().selection.clone();

    let lp = designos_doc::ensure_library_page(&mut s.shell.app.doc);
    let lib_node_id = s.shell.app.doc.alloc_id();
    let mut lib_node = designos_doc::Node::rectangle(lib_node_id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
    lib_node.style_refs.insert("effect".into(), "Effect style 1".into());
    s.shell.app.doc.insert(lp, lib_node).unwrap();

    let rename_row = style_picker::rename_style_row(s.app(), &sel, StyleProperty::Effect);
    assert!(!rename_row.disabled && rename_row.tooltip.is_empty(), "Rename style stays enabled for library-page style");
    let edit_row = style_picker::edit_style_row(s.app(), &sel, StyleProperty::Effect);
    assert!(!edit_row.disabled && edit_row.tooltip.is_empty(), "Edit style stays enabled for library-page style");

    let row = style_picker::delete_style_row(s.app(), &sel, StyleProperty::Effect);
    assert!(row.disabled, "Delete style row is disabled when a node on the hidden library page uses the style");
    assert_eq!(row.label, "Delete style");
    assert_eq!(row.tooltip, style_edit::LIBRARY_STYLE_TIP);

    // In the open menu, hovering Delete style paints the tip, while hovering Rename style does not.
    s.shell.app.props.style_menu = Some(StyleProperty::Effect);
    s.frame(vec![]);
    let del_pt = s.rect_of("menu-row-0-4").center();
    s.mv(del_pt + Vec2::new(0.0, 1.0));
    s.mv(del_pt);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    assert!(painted_texts(&s).iter().any(|t| t == style_edit::LIBRARY_STYLE_TIP));
    let ren_pt = s.rect_of("menu-row-0-2").center();
    s.mv(ren_pt + Vec2::new(0.0, 1.0));
    s.mv(ren_pt);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    assert!(!painted_texts(&s).iter().any(|t| t == style_edit::LIBRARY_STYLE_TIP));
    s.shell.app.props.style_menu = None;

    pick(&mut s, "effect-style-button", 4, 1);
    assert!(s.app().doc.styles.contains_key("Effect style 1"));
    assert_eq!(s.app().doc.get(welcome).unwrap().style_refs.get("effect").map(String::as_str), Some("Effect style 1"));
}

#[test]
fn library_page_scan_never_runs_on_idle_frame_and_at_most_once_when_menu_open() {
    let (mut s, welcome) = welcome_sim();
    s.shell.app.create_style_from(welcome, StyleProperty::Effect);
    s.shell.app.create_style_from(welcome, StyleProperty::Grid);
    let lp = designos_doc::ensure_library_page(&mut s.shell.app.doc);
    let lib_node_id = s.shell.app.doc.alloc_id();
    let lib_node = designos_doc::Node::rectangle(lib_node_id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
    s.shell.app.doc.insert(lp, lib_node).unwrap();
    s.scroll_inspector_down();
    s.frame(vec![]);

    designos_doc::take_library_page_scans();
    s.frame(vec![]);
    let closed_scans = designos_doc::take_library_page_scans();
    assert_eq!(closed_scans, 0, "expected 0 library-page scans per frame with the style menu closed, got {closed_scans}");

    s.shell.app.props.style_menu = Some(StyleProperty::Effect);
    designos_doc::take_library_page_scans();
    s.frame(vec![]);
    let open_scans = designos_doc::take_library_page_scans();
    assert!(open_scans <= 1, "expected at most 1 library-page scan per frame with the style menu open, got {open_scans}");
}

#[test]
fn deleting_a_style_shows_fallback_toast_for_one_and_three_layers_and_plain_toast_when_none() {
    use designos_doc::{Variable, VariableCollection, VariableKind, VariableMode, VariableValue};

    let run_case = |fallback_layer_count: usize| -> String {
        let (mut s, welcome) = welcome_sim();
        let card = s.by_name("Card");
        let page = s.app().page;
        let third_id = s.shell.app.doc.alloc_id();
        let mut third_node = designos_doc::Node::rectangle(third_id, designos_doc::Rect::new(0.0, 0.0, 20.0, 20.0));
        third_node.effects = vec![shadow()];
        let third = s.shell.app.doc.insert(page, third_node).unwrap();
        s.shell.app.edit_nodes(&[card], |n| {
            n.effects = vec![shadow()];
        });
        pick(&mut s, "effect-style-button", 1, 1);

        s.shell.app.doc.variable_collections.insert(
            "c".into(),
            VariableCollection {
                id: "c".into(),
                name: "Theme".into(),
                modes: vec![
                    VariableMode { id: "light".into(), name: "Light".into() },
                    VariableMode { id: "dim".into(), name: "Dim".into() },
                    VariableMode { id: "dark".into(), name: "Dark".into() },
                ],
                default_mode: "light".into(),
                variables: vec!["v_fx_col".into(), "v_fx_op".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        s.shell.app.doc.variables.insert(
            "v_fx_col".into(),
            Variable {
                id: "v_fx_col".into(),
                name: "v_fx_col".into(),
                collection: "c".into(),
                kind: VariableKind::Color,
                values: [("dark".to_string(), VariableValue::Color(Color::rgba(0, 0, 0, 128)))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        let dim_op = if fallback_layer_count == 0 { 80.0 } else { 20.0 };
        s.shell.app.doc.variables.insert(
            "v_fx_op".into(),
            Variable {
                id: "v_fx_op".into(),
                name: "v_fx_op".into(),
                collection: "c".into(),
                kind: VariableKind::Number,
                values: [
                    ("light".to_string(), VariableValue::Number(80.0)),
                    ("dim".to_string(), VariableValue::Number(dim_op)),
                    ("dark".to_string(), VariableValue::Number(5.0)),
                ]
                .into_iter()
                .collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        s.shell.app.doc.style_meta.get_mut("Effect style 1").unwrap().bindings.insert("effects/0/opacity".into(), "v_fx_op".into());

        let targets = match fallback_layer_count {
            0 | 1 => vec![welcome],
            3 => vec![welcome, card, third],
            _ => unreachable!(),
        };
        for &id in &targets {
            if id != welcome {
                s.shell.app.apply_style(&[id], "Effect style 1");
            }
            s.shell.app.doc.get_mut(id).unwrap().variables.bindings.insert("effects/0/color".into(), "v_fx_col".into());
        }
        s.scroll_inspector_down();
        s.frame(vec![]);
        pick(&mut s, "effect-style-button", 4, 1);
        let toast = s.app().toast.as_ref().expect("deleting a style shows a toast");
        assert!(!toast.error);
        toast.text.clone()
    };

    let undo = if cfg!(target_os = "macos") { "Cmd+Z" } else { "Ctrl+Z" };
    assert_eq!(run_case(0), format!("Style deleted · {undo} to undo"));
    assert_eq!(run_case(1), format!("Style deleted · 1 layer matches it in this mode only · {undo} to undo"));
    assert_eq!(run_case(3), format!("Style deleted · 3 layers match it in this mode only · {undo} to undo"));
}

#[test]
fn styles_order_by_sort_position_group_by_folder_and_rename_folder_in_one_undo_step() {
    use crate::styles::{style_entries_of, StyleKind};
    use designos_doc::{Style, StyleMeta};

    let (mut s, welcome) = welcome_sim();
    // Insert 4 effect styles in 2 folders whose sort_position order is opposite to alphabetical / key order.
    for (key, name, pos) in [
        ("2:10", "Shadows/shadow-lg", "!~~p~oLx"),
        ("2:20", "Focus rings/focus-ring", "\"'#"),
        ("2:30", "Shadows/shadow-xs", "!~~p~oL"),
        ("2:40", "Shadows/shadow-sm", "!~~p~oLf"),
    ] {
        s.shell.app.doc.styles.insert(key.into(), Style::Effect(vec![shadow()]));
        s.shell
            .app
            .doc
            .style_meta
            .insert(key.into(), StyleMeta { name: name.into(), property: StyleProperty::Effect, sort_position: Some(pos.into()), ..Default::default() });
    }

    let entries = style_entries_of(&s.app().doc, StyleKind::Effect);
    let ordered_names: Vec<(&str, &str)> = entries.iter().map(|e| (e.folder.as_str(), e.leaf.as_str())).collect();
    assert_eq!(
        ordered_names,
        vec![("Shadows", "shadow-xs"), ("Shadows", "shadow-sm"), ("Shadows", "shadow-lg"), ("Focus rings", "focus-ring"),],
        "styles must be ordered by sort_position and grouped by folder, not sorted alphabetically by name"
    );

    // Link welcome to 2:30 ("Shadows/shadow-xs"), open the effect style picker menu, verify
    // folder headers and ordered leaves are painted, pick "Rename folder" (row 8), and rename
    // "Shadows" -> "Elevation".
    s.shell.app.apply_style(&[welcome], "2:30");
    s.frame(vec![]);
    let btn = s.rect_of("effect-style-button");
    s.click(btn.center());
    s.frame(vec![]);
    let painted = painted_texts(&s);
    for expected in ["Shadows", "  shadow-xs", "  shadow-sm", "  shadow-lg", "Focus rings", "  focus-ring", "Rename folder"] {
        assert!(painted.iter().any(|t| t == expected), "expected {expected:?} in open style picker menu, got {painted:?}");
    }
    // Menu rows when 4 styles in 2 folders are listed:
    // 0: Shadows (disabled header), 1: shadow-xs, 2: shadow-sm, 3: shadow-lg, 4: Focus rings (disabled header), 5: focus-ring
    // separator
    // 6: Create, 7: Rename style, 8: Rename folder, 9: Detach, 10: Delete, 11: Edit
    let rename_folder_row = s.rect_of("menu-row-0-8");
    s.click(rename_folder_row.center());
    assert_eq!(s.app().props.style_rename, Some(StyleProperty::Effect));
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().props.style_rename, Some(StyleProperty::Effect));
    s.type_text("Elevation");
    s.key(Key::Enter);
    s.frame(vec![]);

    assert_eq!(s.app().doc.style_name("2:30"), "Elevation/shadow-xs");
    assert_eq!(s.app().doc.style_name("2:40"), "Elevation/shadow-sm");
    assert_eq!(s.app().doc.style_name("2:10"), "Elevation/shadow-lg");
    assert_eq!(s.app().doc.style_name("2:20"), "Focus rings/focus-ring");

    // One undo restores every member style's prefix.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.style_name("2:30"), "Shadows/shadow-xs");
    assert_eq!(s.app().doc.style_name("2:40"), "Shadows/shadow-sm");
    assert_eq!(s.app().doc.style_name("2:10"), "Shadows/shadow-lg");
}
