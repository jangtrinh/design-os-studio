//! "Edit style": change the values of a shared style from the style picker and see every linked
//! layer follow, as one undo step.

use super::*;
use crate::panels::{style_edit, style_picker};
use designos_doc::{Effect, ExternalAsset, LayoutGrid, LibraryAssetKind, Style, StyleProperty};

fn shadow(radius: f32) -> Effect {
    Effect::DropShadow { color: Color::rgba(0, 0, 0, 64), offset: DV::new(0.0, 4.0), radius, spread: 0.0, visible: true, show_behind: false }
}

fn columns(count: u32) -> LayoutGrid {
    LayoutGrid::Columns { count, gutter: 20.0, offset: 0.0, color: Color::rgba(255, 0, 0, 26) }
}

/// Rect of a numeric field whose key is not a plain string (the key's `Debug` text).
fn raw_field_rect(s: &Sim, key_debug: &str) -> Rect {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", key_debug.to_string())))).unwrap_or_else(|| panic!("no field {key_debug}"))
}

fn type_into(s: &mut Sim, r: Rect, text: &str) {
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text(text);
    s.key(Key::Enter);
}

/// Hovers the local-styles row of `key` and clicks its "Edit style" button.
fn edit_from_list(s: &mut Sim, key: &str) {
    s.scroll_inspector_down();
    let row = s.rect_of(&format!("style:{key}"));
    s.mv(row.center());
    s.frame(vec![]);
    let button = s.rect_of(&format!("style-edit:{key}"));
    s.click(button.center());
    s.frame(vec![]);
}

fn fills_of(s: &Sim, id: NodeId) -> Vec<Paint> {
    s.app().doc.get(id).unwrap().fills.clone()
}

#[test]
fn editing_a_color_style_from_the_list_updates_every_linked_layer_in_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let (hero, card) = (s.by_name("Hero"), s.by_name("Card"));
    s.shell.app.apply_style(&[hero, card], "Brand/Primary");
    let before = fills_of(&s, hero);
    edit_from_list(&mut s, "Brand/Primary");
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some("Brand/Primary"));
    let field = s.field_rect("se-fill-opacity");
    type_into(&mut s, field, "50");
    for id in [hero, card] {
        assert!((fills_of(&s, id)[0].opacity - 0.5).abs() < 1e-4, "linked layer {id:?} follows the style");
    }
    assert!(matches!(&s.app().doc.styles["Brand/Primary"], Style::Fill(p) if (p[0].opacity - 0.5).abs() < 1e-4));
    s.cmd_key(Key::Z);
    assert_eq!(fills_of(&s, hero), before, "one undo takes the style and all of its layers back");
    assert_eq!(fills_of(&s, card), before);
    assert!(matches!(&s.app().doc.styles["Brand/Primary"], Style::Fill(p) if (p[0].opacity - 1.0).abs() < 1e-4));
    assert!(s.app().doc.get(hero).unwrap().style_refs.contains_key("fill"), "the link survives the edit and its undo");
}

#[test]
fn editing_a_text_style_from_the_list_restyles_every_linked_text_layer() {
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let t = s.by_name("Welcome back");
    s.shell.app.apply_style(&[t], "Body/Regular");
    let size_of = |s: &Sim| match &s.app().doc.get(t).unwrap().kind {
        NodeKind::Text(tp) => tp.style.font_size,
        k => panic!("{k:?}"),
    };
    assert_eq!(size_of(&s), 16.0);
    edit_from_list(&mut s, "Body/Regular");
    let field = s.field_rect("se-size");
    type_into(&mut s, field, "30");
    assert_eq!(size_of(&s), 30.0);
    assert!(matches!(&s.app().doc.styles["Body/Regular"], Style::Text(ts) if ts.font_size == 30.0));
    assert_eq!(s.app().doc.get(t).unwrap().style_refs.get("text").map(String::as_str), Some("Body/Regular"));
    s.cmd_key(Key::Z);
    assert_eq!(size_of(&s), 16.0);
    assert!(matches!(&s.app().doc.styles["Body/Regular"], Style::Text(ts) if ts.font_size == 16.0));
}

/// "Welcome" with a drop shadow and 12 column guides, both turned into styles, plus "Card" linked
/// to the effect style.
fn welcome_with_styles() -> (Sim, NodeId, NodeId) {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let (welcome, card) = (s.by_name("Welcome"), s.by_name("Card"));
    s.shell.app.edit_nodes(&[welcome], |n| {
        n.effects = vec![shadow(8.0)];
        if let Some(f) = n.kind.frame_props_mut() {
            f.grids = vec![columns(12)];
        }
    });
    assert!(s.shell.app.create_style_from(welcome, StyleProperty::Effect).is_some());
    assert!(s.shell.app.create_style_from(welcome, StyleProperty::Grid).is_some());
    s.shell.app.apply_style(&[card], "Effect style 1");
    s.scroll_inspector_down();
    (s, welcome, card)
}

#[test]
fn editing_an_effect_style_from_its_linked_row_updates_every_linked_layer_in_one_undo_step() {
    let (mut s, welcome, card) = welcome_with_styles();
    let button = s.rect_of("effect-style-edit");
    s.click(button.center());
    s.frame(vec![]);
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some("Effect style 1"));
    // Rows of a drop shadow: Name, X, Y, Blur, ...
    let blur = raw_field_rect(&s, "(\"fxp-b\", 0)");
    type_into(&mut s, blur, "20");
    for id in [welcome, card] {
        assert_eq!(s.app().doc.get(id).unwrap().effects, vec![shadow(20.0)], "linked layer {id:?} follows the style");
    }
    assert!(matches!(&s.app().doc.styles["Effect style 1"], Style::Effect(fx) if fx == &vec![shadow(20.0)]));
    s.cmd_key(Key::Z);
    for id in [welcome, card] {
        assert_eq!(s.app().doc.get(id).unwrap().effects, vec![shadow(8.0)]);
    }
    assert!(matches!(&s.app().doc.styles["Effect style 1"], Style::Effect(fx) if fx == &vec![shadow(8.0)]));
}

#[test]
fn editing_a_grid_style_from_the_style_menu_updates_the_linked_frame() {
    let (mut s, welcome, _card) = welcome_with_styles();
    // The menu rows: the style, then Create, Rename, Detach and Edit style below the separator.
    let r = s.rect_of("grid-style-button");
    s.click(r.center());
    let top = r.bottom() + 4.0 + theme::MENU_PADDING + 4.0 * theme::MENU_ITEM_HEIGHT + crate::widgets::menu::SEP_H;
    s.click(Pos2::new(r.right() - 60.0, top + theme::MENU_ITEM_HEIGHT / 2.0));
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some("Grid style 1"));
    s.frame(vec![]);
    let field = s.field_rect("se-grid-count");
    type_into(&mut s, field, "4");
    let grids = s.app().doc.get(welcome).unwrap().kind.frame_props().unwrap().grids.clone();
    assert_eq!(grids, vec![columns(4)]);
    assert!(matches!(&s.app().doc.styles["Grid style 1"], Style::Grid(g) if g == &vec![columns(4)]));
    s.cmd_key(Key::Z);
    let grids = s.app().doc.get(welcome).unwrap().kind.frame_props().unwrap().grids.clone();
    assert_eq!(grids, vec![columns(12)]);
}

#[test]
fn a_drag_is_recorded_once_when_the_pointer_is_released() {
    let (mut s, welcome, card) = welcome_with_styles();
    let button = s.rect_of("effect-style-edit");
    s.click(button.center());
    s.frame(vec![]);
    // Pointer down on the popover body (not on a control) while values change underneath.
    let body = raw_field_rect(&s, "(\"fxp-b\", 0)");
    s.mv(Pos2::new(body.left() - 60.0, body.center().y));
    s.button(true);
    for radius in [10.0, 14.0, 18.0] {
        style_edit::edit(&mut s.shell.app, |st| {
            if let Style::Effect(fx) = st {
                fx[0] = shadow(radius);
            }
        });
        s.frame(vec![]);
        assert_eq!(s.app().doc.get(card).unwrap().effects, vec![shadow(8.0)], "nothing is recorded while the pointer is down");
    }
    s.button(false);
    s.frame(vec![]);
    for id in [welcome, card] {
        assert_eq!(s.app().doc.get(id).unwrap().effects, vec![shadow(18.0)]);
    }
    s.cmd_key(Key::Z);
    for id in [welcome, card] {
        assert_eq!(s.app().doc.get(id).unwrap().effects, vec![shadow(8.0)], "one undo takes the whole drag back");
    }
}

/// Marks style `key` as the local copy of a style published by a shared library.
fn make_library_style(s: &mut Sim, key: &str) {
    s.shell.app.doc.library.external.insert(
        format!("L1/{key}"),
        ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some(key.into()) },
    );
}

#[test]
fn a_library_style_cannot_be_edited_from_the_list() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    s.shell.app.apply_style(&[hero], "Brand/Primary");
    make_library_style(&mut s, "Brand/Primary");
    let before = s.app().doc.styles["Brand/Primary"].clone();
    edit_from_list(&mut s, "Brand/Primary");
    assert!(s.app().props.style_edit.is_none(), "the popover stays closed for a library style");
    assert_eq!(s.app().doc.styles["Brand/Primary"], before);
}

#[test]
fn a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu() {
    let (mut s, ..) = welcome_with_styles();
    let sel = s.app().selection.clone();
    let props = [(StyleProperty::Effect, "Effect style 1"), (StyleProperty::Grid, "Grid style 1")];
    // A local style: the menu row reads "Edit style" and can be picked.
    for (prop, _) in props {
        let row = style_picker::edit_style_row(s.app(), &sel, prop);
        assert_eq!((row.label.as_str(), row.disabled), ("Edit style", false), "{prop:?}: a local style can be edited from the menu");
    }
    for (_, key) in props {
        make_library_style(&mut s, key);
    }
    // A library style: the same row is disabled and says where the style is edited.
    for (prop, _) in props {
        let row = style_picker::edit_style_row(s.app(), &sel, prop);
        assert!(row.disabled, "{prop:?}: the menu row is disabled for a library style");
        assert_eq!(row.label, "Edit in the library file", "{prop:?}: the row names where to edit");
    }
    // The linked row's edit button opens nothing either.
    let button = s.rect_of("effect-style-edit");
    s.click(button.center());
    s.frame(vec![]);
    assert!(s.app().props.style_edit.is_none(), "the linked row's edit button does nothing");
}
