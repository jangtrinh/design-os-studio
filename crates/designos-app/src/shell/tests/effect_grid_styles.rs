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

    let row = style_picker::delete_style_row(s.app(), &sel, StyleProperty::Effect);
    assert!(row.disabled, "Delete style row is disabled when a node on the hidden library page uses the style");
    assert_eq!(row.label, "Delete style");
    assert_eq!(row.tooltip, style_edit::LIBRARY_STYLE_TIP);

    pick(&mut s, "effect-style-button", 4, 1);
    assert!(s.app().doc.styles.contains_key("Effect style 1"));
    assert_eq!(s.app().doc.get(welcome).unwrap().style_refs.get("effect").map(String::as_str), Some("Effect style 1"));
}
